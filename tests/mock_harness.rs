//! Tests for the OpenAPI mock server itself, using small fixture specs, so
//! the harness is known to behave before the product tests rely on it.

mod support;

use atlassian_cli::Output;
use serde_json::json;
use support::{SpecMock, TestConfig, fixture_path, run_cli};

async fn sample() -> SpecMock {
    SpecMock::start(&fixture_path("openapi3-sample.json")).await
}

async fn get(mock: &SpecMock, path: &str, query: &[&str]) -> anyhow::Result<Output> {
    let config = TestConfig::new();
    let mut args = vec!["jira", "raw", path];
    for q in query {
        args.extend(["-q", q]);
    }
    run_cli(&mock.uri(), &config, &args).await
}

#[tokio::test]
async fn serves_inline_examples_below_server_base_path() {
    let mock = sample().await;
    let out = get(&mock, "/rest/api/1/thing/42", &[]).await.unwrap();
    assert_eq!(out, Output::Json(json!({"id": "42", "name": "templated"})));
}

#[tokio::test]
async fn literal_paths_beat_templates() {
    let mock = sample().await;
    let out = get(&mock, "/rest/api/1/thing/special", &[]).await.unwrap();
    assert_eq!(out, Output::Json(json!({"special": true})));
    assert_eq!(
        mock.operation_for("/rest/api/1/thing/special")
            .unwrap()
            .template,
        "/rest/api/1/thing/special"
    );
}

#[tokio::test]
async fn synthesizes_from_schema_with_refs_allof_and_recursion() {
    let mock = sample().await;
    let out = get(&mock, "/rest/api/1/widgets", &["q=x"]).await.unwrap();
    let widget = json!({
        "name": "string",
        "kind": "GEAR",
        "created": "2024-01-01T00:00:00.000Z",
        "parent": null
    });
    assert_eq!(
        out,
        Output::Json(json!({"isLastPage": true, "start": 1, "values": [widget]}))
    );
}

#[tokio::test]
async fn required_query_parameters_are_enforced() {
    let mock = sample().await;
    let err = get(&mock, "/rest/api/1/widgets", &[]).await.unwrap_err();
    assert!(err.to_string().contains("404"), "{err:#}");
}

#[tokio::test]
async fn named_and_string_examples() {
    let mock = sample().await;
    assert_eq!(
        get(&mock, "/rest/api/1/examples", &[]).await.unwrap(),
        Output::Json(json!({"named": "example"}))
    );
    assert_eq!(
        get(&mock, "/rest/api/1/stringly", &[]).await.unwrap(),
        Output::Json(json!({"decoded": true}))
    );
}

#[tokio::test]
async fn text_responses_and_multi_segment_paths() {
    let mock = sample().await;
    let out = get(&mock, "/rest/api/1/files/src/main.rs", &[])
        .await
        .unwrap();
    assert_eq!(out, Output::Text("hello\n".into()));
}

#[tokio::test]
async fn no_content_responses() {
    let mock = sample().await;
    assert_eq!(
        get(&mock, "/rest/api/1/empty", &[]).await.unwrap(),
        Output::Json(serde_json::Value::Null)
    );
}

#[tokio::test]
async fn unknown_paths_and_non_get_operations_are_404() {
    let mock = sample().await;
    for path in [
        "/rest/api/1/nope",
        "/rest/api/1/write-only",
        "/api/1/thing/42",
    ] {
        let err = get(&mock, path, &[]).await.unwrap_err();
        assert!(err.to_string().contains("404"), "{path}: {err:#}");
    }
}

#[tokio::test]
async fn wrong_credentials_are_401() {
    let mock = sample().await;
    let config = TestConfig::with("username = \"test-user\"\npassword = \"wrong\"\n");
    let err = run_cli(
        &mock.uri(),
        &config,
        &["jira", "raw", "/rest/api/1/thing/1"],
    )
    .await
    .unwrap_err();
    let msg = format!("{err:#}");
    assert!(msg.contains("401"), "{msg}");
    assert!(msg.contains("not authenticated"), "{msg}");
}

#[tokio::test]
async fn swagger2_specs_are_supported() {
    let mock = SpecMock::start(&fixture_path("swagger2-sample.json")).await;
    let config = TestConfig::new();
    let out = run_cli(
        &mock.uri(),
        &config,
        &["bitbucket", "application-properties"],
    )
    .await
    .unwrap();
    assert_eq!(out, Output::Json(json!({"version": "9.0.0"})));
    let out = run_cli(
        &mock.uri(),
        &config,
        &["bitbucket", "project", "get", "PROJ"],
    )
    .await
    .unwrap();
    assert_eq!(out, Output::Json(json!({"key": "string", "public": true})));
}

#[test]
fn base_path_variants() {
    use support::base_path;
    assert_eq!(
        base_path(&json!({"servers": [{"url": "http://{baseurl}/rest"}]})),
        "/rest"
    );
    assert_eq!(base_path(&json!({"servers": [{"url": "/rest/"}]})), "/rest");
    assert_eq!(
        base_path(&json!({"servers": [{"url": "https://host"}]})),
        ""
    );
    assert_eq!(
        base_path(&json!({"basePath": "/rest/api/2"})),
        "/rest/api/2"
    );
    assert_eq!(base_path(&json!({})), "");
}

#[test]
fn template_regex_variants() {
    use support::template_regex;
    let (re, n) = template_regex("/rest/api/2/issue/{key}/comment");
    assert_eq!(n, 1);
    assert!(re.is_match("/rest/api/2/issue/ABC-1/comment"));
    assert!(!re.is_match("/rest/api/2/issue/a/b/comment"));
    let (re, _) = template_regex("/repos/{slug}/browse/{path:.*}");
    assert!(re.is_match("/repos/r/browse/a/b/c"));
    let (re, _) = template_regex("/api/v1.0/x");
    assert!(!re.is_match("/api/v1a0/x"), "literal dots must be escaped");
}
