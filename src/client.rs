//! A GET-only HTTP client. There is deliberately no way to send any other
//! HTTP method: this first version of the CLI is read-only.

use std::path::Path;

use anyhow::{Context, Result, bail};
use reqwest::header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::Value;

use crate::config::Credentials;
use crate::log::RequestLog;
use crate::request::{ApiRequest, PageStyle};

/// What a command printed: parsed JSON or plain text (e.g. raw file content).
#[derive(Debug, Clone, PartialEq)]
pub enum Output {
    Json(Value),
    Text(String),
}

impl Output {
    /// The JSON value, if this output is JSON.
    pub fn json(&self) -> Option<&Value> {
        match self {
            Output::Json(v) => Some(v),
            Output::Text(_) => None,
        }
    }
}

/// Upper bound on pages fetched by `--all`, as a guard against servers that
/// never report the last page.
const MAX_PAGES: usize = 10_000;

pub struct Client {
    http: reqwest::Client,
    base_url: String,
    credentials: Credentials,
    log: Option<RequestLog>,
    max_items: Option<usize>,
}

impl Client {
    pub fn new(base_url: &str, credentials: Credentials, ca_cert: Option<&Path>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        // Stops XSRF checks from rejecting requests on some DC endpoints.
        headers.insert("X-Atlassian-Token", HeaderValue::from_static("no-check"));

        let mut builder = reqwest::Client::builder()
            .default_headers(headers)
            .user_agent(concat!("atlassian-cli/", env!("CARGO_PKG_VERSION")));
        if let Some(path) = ca_cert {
            let pem = std::fs::read(path)
                .with_context(|| format!("could not read CA certificate {}", path.display()))?;
            let cert = reqwest::Certificate::from_pem(&pem)
                .with_context(|| format!("invalid PEM certificate {}", path.display()))?;
            builder = builder.add_root_certificate(cert);
        }

        let base_url = base_url.trim_end_matches('/').to_string();
        if !(base_url.starts_with("http://") || base_url.starts_with("https://")) {
            bail!("base URL must start with http:// or https://, got `{base_url}`");
        }

        Ok(Self {
            http: builder.build().context("could not build HTTP client")?,
            base_url,
            credentials,
            log: None,
            max_items: None,
        })
    }

    /// Record every request in `log`.
    pub fn with_log(mut self, log: Option<RequestLog>) -> Self {
        self.log = log;
        self
    }

    /// Stop `--all` after this many items.
    pub fn with_max_items(mut self, max: Option<usize>) -> Self {
        self.max_items = max;
        self
    }

    fn record(&self, url: &str, status: Option<u16>, items: Option<usize>) -> Result<()> {
        match &self.log {
            Some(log) => log.request(url, status, items),
            None => Ok(()),
        }
    }

    /// Run the request, following pagination when `--all` was given.
    pub async fn execute(&self, req: &ApiRequest) -> Result<Output> {
        match &req.paging {
            Some(p) if p.all => self.get_all(req, p.style, p.limit).await,
            _ => self.get(&req.path, &req.first_query()).await,
        }
    }

    /// Send a single GET request.
    pub async fn get(&self, path: &str, query: &[(String, String)]) -> Result<Output> {
        let url = format!("{}{}", self.base_url, path);
        let request = self
            .http
            .get(&url)
            .basic_auth(&self.credentials.username, Some(&self.credentials.password))
            .query(query)
            .build()
            .with_context(|| format!("invalid request to {url}"))?;
        let full_url = request.url().to_string();
        let resp = match self.http.execute(request).await {
            Ok(resp) => resp,
            Err(e) => {
                self.record(&full_url, None, None)?;
                return Err(e).with_context(|| format!("request to {url} failed"));
            }
        };

        let status = resp.status();
        let is_json = resp
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|ct| ct.contains("json"));
        let body = resp
            .text()
            .await
            .with_context(|| format!("could not read response from {url}"))?;

        if !status.is_success() {
            self.record(&full_url, Some(status.as_u16()), None)?;
            bail!("GET {path} returned {status}{}", error_detail(&body));
        }
        let output = if body.is_empty() {
            Output::Json(Value::Null)
        } else if is_json {
            let v = serde_json::from_str(&body)
                .with_context(|| format!("GET {path} returned invalid JSON"))?;
            Output::Json(v)
        } else {
            Output::Text(body)
        };
        let items = match &output {
            Output::Json(Value::Array(a)) => Some(a.len()),
            Output::Json(v) => page_items(v).map(Vec::len),
            Output::Text(_) => None,
        };
        self.record(&full_url, Some(status.as_u16()), items)?;
        Ok(output)
    }

    async fn get_all(
        &self,
        req: &ApiRequest,
        style: PageStyle,
        limit: Option<u64>,
    ) -> Result<Output> {
        let mut items = Vec::new();
        let mut start = 0u64;
        for _ in 0..MAX_PAGES {
            let mut query = req.query.clone();
            query.push((style.start_param().into(), start.to_string()));
            if let Some(l) = limit {
                query.push((style.limit_param().into(), l.to_string()));
            }
            let page = match self.get(&req.path, &query).await? {
                Output::Json(v) => v,
                Output::Text(_) => bail!("GET {} did not return JSON", req.path),
            };
            let Some(page_items) = page_items(&page) else {
                // Not a paged collection: return what the server sent.
                return Ok(Output::Json(page));
            };
            let count = page_items.len() as u64;
            items.extend(page_items.iter().cloned());
            if let Some(max) = self.max_items
                && items.len() >= max
            {
                let more = items.len() > max || next_start(style, &page, start, count).is_some();
                items.truncate(max);
                if more {
                    eprintln!(
                        "warning: stopped at {max} results (the configured maximum); \
                         there are more. Narrow the request to see the rest."
                    );
                }
                return Ok(Output::Json(Value::Array(items)));
            }
            match next_start(style, &page, start, count) {
                Some(next) if next > start => start = next,
                _ => return Ok(Output::Json(Value::Array(items))),
            }
        }
        bail!("stopped after {MAX_PAGES} pages; narrow the query or use --start/--limit")
    }
}

/// Keys that hold the items of a paged collection, in lookup order.
const ITEM_KEYS: &[&str] = &[
    "values",
    "issues",
    "results",
    "comments",
    "worklogs",
    "dashboards",
];

/// The items on one page of a paged collection, if `page` is one.
pub fn page_items(page: &Value) -> Option<&Vec<Value>> {
    let obj = page.as_object()?;
    ITEM_KEYS.iter().find_map(|k| obj.get(*k)?.as_array())
}

/// The items of a list response: the array itself, or the items on a page.
pub fn items_mut(value: &mut Value) -> Option<&mut Vec<Value>> {
    if value.is_array() {
        return value.as_array_mut();
    }
    let obj = value.as_object_mut()?;
    let key = ITEM_KEYS
        .iter()
        .find(|k| obj.get(**k).is_some_and(Value::is_array))?;
    obj.get_mut(*key)?.as_array_mut()
}

/// Where the next page starts, or `None` when `page` is the last one.
pub fn next_start(style: PageStyle, page: &Value, start: u64, count: u64) -> Option<u64> {
    if count == 0 {
        return None;
    }
    let num = |k: &str| page.get(k).and_then(Value::as_u64);
    let flag = |k: &str| page.get(k).and_then(Value::as_bool);
    match style {
        PageStyle::Jira => {
            if flag("isLast") == Some(true) {
                return None;
            }
            let next = num("startAt").unwrap_or(start) + count;
            match num("total") {
                Some(total) if next >= total => None,
                _ => Some(next),
            }
        }
        PageStyle::Confluence => {
            page.pointer("/_links/next")?;
            Some(num("start").unwrap_or(start) + num("size").unwrap_or(count))
        }
        PageStyle::Bitbucket => {
            if flag("isLastPage") != Some(false) {
                return None;
            }
            num("nextPageStart")
        }
    }
}

/// Pull a readable message out of an Atlassian error body.
pub fn error_detail(body: &str) -> String {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        let text = body.trim();
        return if text.is_empty() || text.starts_with('<') {
            String::new()
        } else {
            format!(": {}", text.chars().take(500).collect::<String>())
        };
    };
    let mut msgs: Vec<String> = Vec::new();
    // Jira: {"errorMessages": [...], "errors": {"field": "msg"}}
    if let Some(arr) = v.get("errorMessages").and_then(Value::as_array) {
        msgs.extend(arr.iter().filter_map(Value::as_str).map(String::from));
    }
    match v.get("errors") {
        // Bitbucket: {"errors": [{"message": "..."}]}
        Some(Value::Array(arr)) => msgs.extend(
            arr.iter()
                .filter_map(|e| e.get("message")?.as_str())
                .map(String::from),
        ),
        Some(Value::Object(map)) => msgs.extend(
            map.iter()
                .filter_map(|(k, e)| Some(format!("{k}: {}", e.as_str()?))),
        ),
        _ => {}
    }
    // Confluence: {"message": "..."}
    if let Some(m) = v.get("message").and_then(Value::as_str) {
        msgs.push(m.to_string());
    }
    if msgs.is_empty() {
        String::new()
    } else {
        format!(": {}", msgs.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn jira_next_page_uses_total() {
        let p = json!({"startAt": 0, "maxResults": 2, "total": 5, "issues": [1, 2]});
        assert_eq!(next_start(PageStyle::Jira, &p, 0, 2), Some(2));
        let p = json!({"startAt": 4, "maxResults": 2, "total": 5, "issues": [5]});
        assert_eq!(next_start(PageStyle::Jira, &p, 4, 1), None);
    }

    #[test]
    fn jira_agile_is_last_stops() {
        let p = json!({"startAt": 0, "isLast": true, "values": [1]});
        assert_eq!(next_start(PageStyle::Jira, &p, 0, 1), None);
        let p = json!({"startAt": 0, "isLast": false, "values": [1]});
        assert_eq!(next_start(PageStyle::Jira, &p, 0, 1), Some(1));
    }

    #[test]
    fn confluence_follows_next_link() {
        let p = json!({"start": 0, "size": 2, "results": [1, 2], "_links": {"next": "/x"}});
        assert_eq!(next_start(PageStyle::Confluence, &p, 0, 2), Some(2));
        let p = json!({"start": 2, "size": 1, "results": [3], "_links": {}});
        assert_eq!(next_start(PageStyle::Confluence, &p, 2, 1), None);
    }

    #[test]
    fn bitbucket_uses_next_page_start() {
        let p = json!({"isLastPage": false, "nextPageStart": 25, "values": [1]});
        assert_eq!(next_start(PageStyle::Bitbucket, &p, 0, 1), Some(25));
        let p = json!({"isLastPage": true, "values": [1]});
        assert_eq!(next_start(PageStyle::Bitbucket, &p, 0, 1), None);
    }

    #[test]
    fn empty_page_stops() {
        let p = json!({"isLastPage": false, "nextPageStart": 25, "values": []});
        assert_eq!(next_start(PageStyle::Bitbucket, &p, 0, 0), None);
    }

    #[test]
    fn page_items_finds_known_keys() {
        assert_eq!(page_items(&json!({"values": [1]})).unwrap().len(), 1);
        assert_eq!(page_items(&json!({"issues": [1, 2]})).unwrap().len(), 2);
        assert!(page_items(&json!([1, 2])).is_none());
        assert!(page_items(&json!({"key": "ABC"})).is_none());
    }

    #[test]
    fn error_detail_formats() {
        assert_eq!(
            error_detail(r#"{"errorMessages":["Issue does not exist"],"errors":{}}"#),
            ": Issue does not exist"
        );
        assert_eq!(
            error_detail(r#"{"errors":[{"message":"Repository not found"}]}"#),
            ": Repository not found"
        );
        assert_eq!(
            error_detail(r#"{"statusCode":404,"message":"No space"}"#),
            ": No space"
        );
        assert_eq!(error_detail("<html>oops</html>"), "");
        assert_eq!(error_detail("plain text"), ": plain text");
    }

    #[test]
    fn rejects_non_http_base_url() {
        let creds = Credentials {
            username: "u".into(),
            password: "p".into(),
        };
        assert!(Client::new("ftp://x", creds.clone(), None).is_err());
        assert!(Client::new("https://jira.example.com/", creds, None).is_ok());
    }
}
