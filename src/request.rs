//! A product-independent description of a single read-only REST call.

use clap::Args;

pub use crate::scope::Guard;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// Characters that are percent-encoded inside a single path segment.
const SEGMENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'~')
    .remove(b'@');

/// Percent-encode one user-supplied path segment (issue key, repo slug, ...).
pub fn seg(s: &str) -> String {
    utf8_percent_encode(s, SEGMENT).to_string()
}

/// Percent-encode a `/`-separated file path, keeping the slashes.
pub fn path_segs(s: &str) -> String {
    s.split('/')
        .filter(|p| !p.is_empty())
        .map(seg)
        .collect::<Vec<_>>()
        .join("/")
}

/// How a product paginates its collection resources.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageStyle {
    /// Jira and Jira Software: `startAt` / `maxResults`, `total` / `isLast` in the body.
    Jira,
    /// Confluence: `start` / `limit`, `size` and `_links.next` in the body.
    Confluence,
    /// Bitbucket: `start` / `limit`, `isLastPage` / `nextPageStart` in the body.
    Bitbucket,
}

impl PageStyle {
    pub fn start_param(self) -> &'static str {
        match self {
            PageStyle::Jira => "startAt",
            PageStyle::Confluence | PageStyle::Bitbucket => "start",
        }
    }

    pub fn limit_param(self) -> &'static str {
        match self {
            PageStyle::Jira => "maxResults",
            PageStyle::Confluence | PageStyle::Bitbucket => "limit",
        }
    }
}

/// Paging flags shared by every list command.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct PageArgs {
    /// Index of the first result to return.
    #[arg(long)]
    pub start: Option<u64>,
    /// Maximum number of results per page.
    #[arg(long)]
    pub limit: Option<u64>,
    /// Follow pagination and print every result as one JSON array.
    #[arg(long, conflicts_with = "start")]
    pub all: bool,
}

/// Paging state attached to a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paging {
    pub style: PageStyle,
    pub start: Option<u64>,
    pub limit: Option<u64>,
    pub all: bool,
}

/// A GET request relative to the product's base URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRequest {
    /// Absolute path below the base URL, e.g. `/rest/api/2/myself`.
    pub path: String,
    pub query: Vec<(String, String)>,
    pub paging: Option<Paging>,
    /// What the request touches, checked against the configured scope.
    pub guard: Guard,
}

impl ApiRequest {
    pub fn get(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            query: Vec::new(),
            paging: None,
            guard: Guard::Open,
        }
    }

    /// Say what the request touches, for the scope check.
    pub fn guard(mut self, guard: Guard) -> Self {
        self.guard = guard;
        self
    }

    /// Add a query parameter.
    pub fn param(mut self, key: &str, value: impl ToString) -> Self {
        self.query.push((key.to_string(), value.to_string()));
        self
    }

    /// Add a query parameter only when a value is present.
    pub fn opt<T: ToString>(self, key: &str, value: Option<T>) -> Self {
        match value {
            Some(v) => self.param(key, v),
            None => self,
        }
    }

    /// Add a boolean flag parameter (`key=true`) only when set.
    pub fn flag(self, key: &str, set: bool) -> Self {
        if set { self.param(key, true) } else { self }
    }

    /// Mark the request as paginated in the given style.
    pub fn paged(mut self, style: PageStyle, page: &PageArgs) -> Self {
        self.paging = Some(Paging {
            style,
            start: page.start,
            limit: page.limit,
            all: page.all,
        });
        self
    }

    /// The query string for the first request, paging params included.
    pub fn first_query(&self) -> Vec<(String, String)> {
        let mut q = self.query.clone();
        if let Some(p) = &self.paging {
            if let Some(start) = p.start {
                q.push((p.style.start_param().to_string(), start.to_string()));
            }
            if let Some(limit) = p.limit {
                q.push((p.style.limit_param().to_string(), limit.to_string()));
            }
        }
        q
    }
}

/// Parse a `key=value` command line argument.
pub fn parse_key_val(s: &str) -> Result<(String, String), String> {
    s.split_once('=')
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .ok_or_else(|| format!("expected key=value, got `{s}`"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seg_encodes_reserved_characters() {
        assert_eq!(seg("ABC-123"), "ABC-123");
        assert_eq!(seg("my repo"), "my%20repo");
        assert_eq!(seg("a/b"), "a%2Fb");
        assert_eq!(seg("~user"), "~user");
    }

    #[test]
    fn path_segs_keeps_slashes() {
        assert_eq!(path_segs("src/main file.rs"), "src/main%20file.rs");
        assert_eq!(path_segs("/a//b/"), "a/b");
    }

    #[test]
    fn first_query_appends_paging_params() {
        let page = PageArgs {
            start: Some(5),
            limit: Some(10),
            all: false,
        };
        let r = ApiRequest::get("/x")
            .param("q", "1")
            .paged(PageStyle::Jira, &page);
        assert_eq!(
            r.first_query(),
            vec![
                ("q".into(), "1".into()),
                ("startAt".into(), "5".into()),
                ("maxResults".into(), "10".into()),
            ]
        );
    }

    #[test]
    fn page_param_names_per_style() {
        assert_eq!(PageStyle::Confluence.start_param(), "start");
        assert_eq!(PageStyle::Bitbucket.limit_param(), "limit");
    }

    #[test]
    fn opt_and_flag_skip_absent_values() {
        let r = ApiRequest::get("/x")
            .opt::<String>("a", None)
            .opt("b", Some("2"))
            .flag("c", false)
            .flag("d", true);
        assert_eq!(
            r.query,
            vec![("b".into(), "2".into()), ("d".into(), "true".into())]
        );
    }

    #[test]
    fn parse_key_val_splits_on_first_equals() {
        assert_eq!(
            parse_key_val("jql=a=b").unwrap(),
            ("jql".into(), "a=b".into())
        );
        assert!(parse_key_val("novalue").is_err());
    }
}
