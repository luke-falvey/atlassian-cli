//! Test support: a mock Atlassian server built from an OpenAPI specification.
//!
//! [`SpecMock::start`] reads a spec (OpenAPI 3 or Swagger 2) and registers one
//! wiremock route per `GET` operation. Each route:
//! - matches the templated path (`{param}` → one path segment),
//! - requires the expected Basic `Authorization` header,
//! - requires the operation's mandatory query parameters,
//! - answers with the spec's example response, or a value synthesized from
//!   the response schema when the spec has no example.
//!
//! Requests with missing or wrong credentials get `401`. Requests for paths
//! that aren't in the spec get wiremock's default `404`. So a CLI command that
//! builds a wrong URL fails its test.

#![allow(dead_code)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use base64::Engine;
use regex::Regex;
use serde_json::{Map, Value, json};
use tempfile::TempDir;
use wiremock::matchers::{method, path_regex};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

use atlassian_cli::{Cli, Output};
use clap::Parser;

/// Username written to the test credentials file.
pub const USER: &str = "test-user";
/// Password written to the test credentials file. Contains `:` on purpose.
pub const PASS: &str = "p@ss:word";

/// The `Authorization` header value the mock expects.
pub fn expected_auth() -> String {
    basic_auth(USER, PASS)
}

pub fn basic_auth(user: &str, pass: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Basic {token}")
}

/// Path of a vendored spec in `specs/`. Panics with instructions if missing.
pub fn spec_path(file: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("specs")
        .join(file);
    assert!(
        path.exists(),
        "{} is missing. Run `scripts/fetch-specs.sh` to download the official \
         Atlassian OpenAPI specs (see specs/README.md).",
        path.display()
    );
    path
}

/// Path of a test fixture in `tests/fixtures/`.
pub fn fixture_path(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(file)
}

pub fn load_spec(path: &Path) -> Value {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{} is not JSON: {e}", path.display()))
}

/// The response the mock gives for one operation.
#[derive(Debug, Clone, PartialEq)]
pub enum MockBody {
    Json(Value),
    Text(String),
    Empty,
}

/// One `GET` operation from the spec.
#[derive(Debug, Clone)]
pub struct Operation {
    /// Full path template, base path included, e.g. `/rest/api/2/issue/{key}`.
    pub template: String,
    pub regex: Regex,
    pub status: u16,
    pub body: MockBody,
    pub required_query: Vec<String>,
    /// Number of path parameters; literal paths win over templated ones.
    pub params: usize,
}

impl Operation {
    /// The output the CLI should print for this operation.
    pub fn expected_output(&self) -> Output {
        match &self.body {
            MockBody::Json(v) => Output::Json(v.clone()),
            MockBody::Text(t) => Output::Text(t.clone()),
            MockBody::Empty => Output::Json(Value::Null),
        }
    }
}

pub struct SpecMock {
    pub server: MockServer,
    pub operations: Vec<Operation>,
    /// Context path of the product (e.g. `/confluence`), part of the base URL.
    pub context_path: String,
}

impl SpecMock {
    pub async fn start(spec_file: &Path) -> Self {
        let spec = load_spec(spec_file);
        let operations = operations(&spec);
        assert!(
            !operations.is_empty(),
            "{} has no GET operations",
            spec_file.display()
        );
        let server = MockServer::start().await;
        let auth = expected_auth();

        for op in &operations {
            let required = op.required_query.clone();
            let mut response = ResponseTemplate::new(op.status);
            response = match &op.body {
                MockBody::Json(v) => response.set_body_json(v),
                MockBody::Text(t) => response.set_body_raw(t.clone(), "text/plain"),
                MockBody::Empty => response,
            };
            let expected = auth.clone();
            Mock::given(method("GET"))
                .and(path_regex(op.regex.as_str()))
                .and(move |req: &Request| auth_header(req) == Some(expected.as_str()))
                .and(move |req: &Request| {
                    required
                        .iter()
                        .all(|name| req.url.query_pairs().any(|(k, _)| k == name.as_str()))
                })
                .respond_with(response)
                // wiremock picks the lowest priority first: literal segments
                // (`/branches/default`) beat templates (`/branches/{name}`).
                .with_priority(1 + op.params.min(200) as u8)
                .mount(&server)
                .await;
        }

        let expected = auth.clone();
        Mock::given(move |req: &Request| auth_header(req) != Some(expected.as_str()))
            .respond_with(ResponseTemplate::new(401).set_body_json(json!({
                "errorMessages": ["You are not authenticated. Authentication required to perform this operation."],
                "errors": {}
            })))
            .with_priority(250)
            .mount(&server)
            .await;

        Self {
            server,
            operations,
            context_path: context_path(&spec),
        }
    }

    pub fn uri(&self) -> String {
        self.server.uri()
    }

    /// What a user would pass as `--base-url`: server plus context path.
    pub fn base_url(&self) -> String {
        format!("{}{}", self.server.uri(), self.context_path)
    }

    /// The operation the mock serves for `path`, mirroring wiremock's
    /// priority order.
    pub fn operation_for(&self, path: &str) -> Option<&Operation> {
        self.operations
            .iter()
            .filter(|op| op.regex.is_match(path))
            .min_by_key(|op| op.params)
    }
}

fn auth_header(req: &Request) -> Option<&str> {
    req.headers.get("authorization")?.to_str().ok()
}

/// All `GET` operations in a spec.
pub fn operations(spec: &Value) -> Vec<Operation> {
    let base = base_path(spec);
    let Some(paths) = spec.get("paths").and_then(Value::as_object) else {
        return Vec::new();
    };
    let mut ops = Vec::new();
    for (path, item) in paths {
        let Some(get) = item.get("get") else { continue };
        let template = format!("{}{}", base, path);
        let (regex, params) = template_regex(&template);
        let (status, body) = response(spec, get);
        // Parameters can sit on the path item and on the operation.
        let mut required_query = Vec::new();
        for list in [item.get("parameters"), get.get("parameters")]
            .into_iter()
            .flatten()
            .filter_map(Value::as_array)
        {
            for param in list {
                let param = resolve(spec, param);
                if param.get("in").and_then(Value::as_str) == Some("query")
                    && param.get("required").and_then(Value::as_bool) == Some(true)
                    && let Some(name) = param.get("name").and_then(Value::as_str)
                {
                    required_query.push(name.to_string());
                }
            }
        }
        ops.push(Operation {
            template,
            regex,
            status,
            body,
            required_query,
            params,
        });
    }
    ops
}

/// The path prefix of the API: OpenAPI 3 `servers[0].url` or Swagger 2 `basePath`.
pub fn base_path(spec: &Value) -> String {
    let raw = spec
        .pointer("/servers/0/url")
        .or_else(|| spec.get("basePath"))
        .and_then(Value::as_str)
        .unwrap_or("");
    // Drop scheme and host, e.g. `http://{baseurl}/rest` → `/rest`.
    let path = match raw.find("://") {
        Some(i) => match raw[i + 3..].find('/') {
            Some(j) => &raw[i + 3 + j..],
            None => "",
        },
        None => raw,
    };
    path.trim_end_matches('/').to_string()
}

/// The server path is a context path (part of the user's base URL) when the
/// spec's own paths already start with `/rest/`, as in the Confluence spec
/// (`servers: http://{baseurl}/confluence`, paths `/rest/api/...`).
/// Otherwise it is the REST prefix (`/rest`) that the CLI adds itself.
pub fn context_path(spec: &Value) -> String {
    let paths = spec.get("paths").and_then(Value::as_object);
    let all_rest =
        paths.is_some_and(|p| !p.is_empty() && p.keys().all(|k| k.starts_with("/rest/")));
    if all_rest {
        base_path(spec)
    } else {
        String::new()
    }
}

/// Regex matching a path template. `{x}` matches one segment; a trailing
/// `{path}` or a JAX-RS style `{x:.*}` matches the rest of the path.
pub fn template_regex(template: &str) -> (Regex, usize) {
    let param = Regex::new(r"\{([^}:]+)(:[^}]*)?\}").unwrap();
    let mut out = String::from("^");
    let mut last = 0;
    let mut count = 0;
    for m in param.captures_iter(template) {
        let whole = m.get(0).unwrap();
        out.push_str(&regex::escape(&template[last..whole.start()]));
        let name = &m[1];
        let rest_of_path = m.get(2).is_some() || (name == "path" && whole.end() == template.len());
        out.push_str(if rest_of_path { ".+" } else { "[^/]+" });
        last = whole.end();
        count += 1;
    }
    out.push_str(&regex::escape(&template[last..]));
    out.push_str("/?$");
    (Regex::new(&out).unwrap(), count)
}

/// Follow a local `$ref` (`#/components/...`, `#/definitions/...`).
pub fn resolve<'a>(spec: &'a Value, v: &'a Value) -> &'a Value {
    let mut current = v;
    for _ in 0..16 {
        match current.get("$ref").and_then(Value::as_str) {
            Some(r) => match r.strip_prefix('#').and_then(|p| spec.pointer(p)) {
                Some(target) => current = target,
                None => return current,
            },
            None => return current,
        }
    }
    current
}

/// Status code and body of an operation's success response.
fn response(spec: &Value, op: &Value) -> (u16, MockBody) {
    let Some(responses) = op.get("responses").and_then(Value::as_object) else {
        return (200, MockBody::Empty);
    };
    let mut codes: Vec<&String> = responses.keys().filter(|k| k.starts_with('2')).collect();
    codes.sort();
    let (code, resp) = match codes.first() {
        Some(c) => (c.as_str(), &responses[c.as_str()]),
        None => match responses.get("default") {
            Some(r) => ("200", r),
            None => return (200, MockBody::Empty),
        },
    };
    let status = code.replace(['X', 'x'], "0").parse().unwrap_or(200);
    let resp = resolve(spec, resp);
    if status == 204 {
        return (204, MockBody::Empty);
    }

    // OpenAPI 3: responses.<code>.content.<media type>
    if let Some(content) = resp.get("content").and_then(Value::as_object) {
        if let Some((_, media)) = content.iter().find(|(ct, _)| ct.contains("json")) {
            return (status, json_body(spec, media));
        }
        if let Some((ct, media)) = content.iter().next() {
            if ct == "*/*" {
                return (status, json_body(spec, media));
            }
            return (status, MockBody::Text(text_example(media)));
        }
        return (status, MockBody::Empty);
    }

    // Swagger 2: responses.<code>.examples / schema
    if let Some(ex) = resp.pointer("/examples/application~1json") {
        return (status, MockBody::Json(ex.clone()));
    }
    if let Some(schema) = resp.get("schema") {
        return (
            status,
            MockBody::Json(synthesize(spec, schema, &mut Vec::new(), 0)),
        );
    }
    (status, MockBody::Empty)
}

/// The body for a JSON media type. A media type with no example and no
/// schema documents nothing to serve, so the mock answers with no body.
fn json_body(spec: &Value, media: &Value) -> MockBody {
    let documented = ["example", "examples", "schema"]
        .iter()
        .any(|k| media.get(*k).is_some_and(|v| !v.is_null()));
    if documented {
        MockBody::Json(media_example(spec, media))
    } else {
        MockBody::Empty
    }
}

/// Some specs store JSON examples as strings; decode those.
fn decode_example(ex: &Value) -> Value {
    if let Value::String(s) = ex
        && let Ok(v @ (Value::Object(_) | Value::Array(_))) = serde_json::from_str::<Value>(s)
    {
        return v;
    }
    ex.clone()
}

fn media_example(spec: &Value, media: &Value) -> Value {
    if let Some(ex) = media.get("example").filter(|v| !v.is_null()) {
        return decode_example(ex);
    }
    if let Some(examples) = media.get("examples").and_then(Value::as_object)
        && let Some(first) = examples.values().next()
    {
        let first = resolve(spec, first);
        if let Some(v) = first.get("value") {
            return decode_example(v);
        }
    }
    match media.get("schema") {
        Some(schema) => synthesize(spec, schema, &mut Vec::new(), 0),
        None => Value::Null,
    }
}

fn text_example(media: &Value) -> String {
    match media.get("example") {
        Some(Value::String(s)) => s.clone(),
        _ => "mock file content\n".to_string(),
    }
}

const MAX_DEPTH: usize = 8;

/// Build a sample value from a JSON schema.
pub fn synthesize(spec: &Value, schema: &Value, refs: &mut Vec<String>, depth: usize) -> Value {
    if depth > MAX_DEPTH {
        return Value::Null;
    }
    if let Some(r) = schema.get("$ref").and_then(Value::as_str) {
        if refs.iter().any(|seen| seen == r) {
            return Value::Null; // recursive schema
        }
        refs.push(r.to_string());
        let v = synthesize(spec, resolve(spec, schema), refs, depth + 1);
        refs.pop();
        return v;
    }
    if let Some(ex) = schema.get("example").filter(|v| !v.is_null()) {
        return ex.clone();
    }
    if let Some(all) = schema.get("allOf").and_then(Value::as_array) {
        let mut merged = Map::new();
        for part in all {
            if let Value::Object(m) = synthesize(spec, part, refs, depth + 1) {
                merged.extend(m);
            }
        }
        return Value::Object(merged);
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(first) = schema
            .get(key)
            .and_then(Value::as_array)
            .and_then(|a| a.first())
        {
            return synthesize(spec, first, refs, depth + 1);
        }
    }
    if let Some(first) = schema
        .get("enum")
        .and_then(Value::as_array)
        .and_then(|a| a.first())
    {
        return first.clone();
    }
    let ty = schema.get("type").and_then(Value::as_str);
    match ty {
        Some("object") | None if schema.get("properties").is_some() => {
            let props = schema["properties"]
                .as_object()
                .cloned()
                .unwrap_or_default();
            let obj: BTreeMap<String, Value> = props
                .iter()
                .map(|(k, v)| (k.clone(), synthesize(spec, v, refs, depth + 1)))
                .collect();
            Value::Object(obj.into_iter().collect())
        }
        Some("object") => json!({}),
        Some("array") => match schema.get("items") {
            Some(items) => {
                let item = synthesize(spec, items, refs, depth + 1);
                if item.is_null() {
                    json!([])
                } else {
                    json!([item])
                }
            }
            None => json!([]),
        },
        Some("string") => match schema.get("format").and_then(Value::as_str) {
            Some("date-time") => json!("2024-01-01T00:00:00.000Z"),
            Some("date") => json!("2024-01-01"),
            Some("uri") | Some("url") => json!("https://example.com"),
            _ => json!("string"),
        },
        Some("integer") | Some("number") => json!(1),
        Some("boolean") => json!(true),
        _ => Value::Null,
    }
}

/// A temporary credentials file for the test user.
pub struct Creds {
    _dir: TempDir,
    pub path: PathBuf,
}

impl Creds {
    pub fn new() -> Self {
        Self::with(&format!("{USER}:{PASS}\n"))
    }

    pub fn with(contents: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("atlassian_credentials");
        std::fs::write(&path, contents).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        Self { _dir: dir, path }
    }
}

/// Parse `args` (without the program name) and run them against `base_url`.
pub async fn run_cli(base_url: &str, creds: &Creds, args: &[&str]) -> anyhow::Result<Output> {
    let mut argv = vec![
        "atlassian".to_string(),
        "--base-url".to_string(),
        base_url.to_string(),
        "--credentials-file".to_string(),
        creds.path.display().to_string(),
    ];
    argv.extend(args.iter().map(|s| s.to_string()));
    let cli = Cli::try_parse_from(argv)?;
    atlassian_cli::execute(&cli).await
}

/// Run every invocation against a mock built from `spec_file`. Each one must
/// succeed and print exactly the response the spec describes. All failures
/// are reported together.
pub async fn check_commands_against_spec(spec_file: &str, invocations: &[&[&str]]) {
    let mock = SpecMock::start(&spec_path(spec_file)).await;
    let creds = Creds::new();
    let mut failures = Vec::new();
    for args in invocations {
        let cli = Cli::try_parse_from(std::iter::once(&"atlassian").chain(args.iter()))
            .unwrap_or_else(|e| panic!("`{}` does not parse: {e}", args.join(" ")));
        let path = format!("{}{}", mock.context_path, cli.to_request().unwrap().path);
        let Some(op) = mock.operation_for(&path) else {
            failures.push(format!(
                "`{}`: GET {path} is not in {spec_file}",
                args.join(" ")
            ));
            continue;
        };
        // A null body would let any command "match"; insist on real content.
        if op.body == MockBody::Json(Value::Null) {
            failures.push(format!(
                "`{}`: {} has no example or schema to serve",
                args.join(" "),
                op.template
            ));
            continue;
        }
        match run_cli(&mock.base_url(), &creds, args).await {
            Ok(out) if out == op.expected_output() => {}
            Ok(out) => failures.push(format!(
                "`{}`: output does not match the {} example: {out:?}",
                args.join(" "),
                op.template
            )),
            Err(e) => failures.push(format!("`{}`: {e:#}", args.join(" "))),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} commands failed against {spec_file}:\n  {}",
        failures.len(),
        invocations.len(),
        failures.join("\n  ")
    );
}
