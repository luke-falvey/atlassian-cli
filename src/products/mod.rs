//! One module per Atlassian product. Each defines a clap `Subcommand` tree
//! (resource → action) whose leaves turn into an [`ApiRequest`].

pub mod agile;
pub mod bitbucket;
pub mod confluence;
pub mod jira;

use anyhow::{Result, bail};
use clap::{Args, Subcommand};

use crate::request::{ApiRequest, parse_key_val};

/// `raw`: GET any path below the base URL. A fallback for endpoints that
/// don't have their own command yet.
#[derive(Debug, Clone, Args)]
pub struct RawArgs {
    /// Path below the base URL, starting with `/`: e.g. `/rest/api/2/serverInfo` (Jira),
    /// `/rest/api/space` (Confluence) or `/rest/api/latest/projects` (Bitbucket).
    pub path: String,
    /// Query parameter as key=value. Repeat for several parameters.
    #[arg(short, long = "query", value_name = "KEY=VALUE", value_parser = parse_key_val)]
    pub query: Vec<(String, String)>,
}

impl RawArgs {
    pub fn to_request(&self) -> Result<ApiRequest> {
        if !self.path.starts_with('/') {
            bail!("raw path must start with `/`, got `{}`", self.path);
        }
        let mut req = ApiRequest::get(self.path.clone());
        req.query = self.query.clone();
        Ok(req)
    }
}

/// Actions for small lookup resources that can only be listed or fetched by id.
#[derive(Debug, Clone, Subcommand)]
pub enum ListGet {
    /// List all.
    List,
    /// Get one by id.
    Get {
        /// Id of the item, as shown by `list`.
        id: String,
    },
}

impl ListGet {
    /// `/base` for `list`, `/base/{id}` for `get`.
    pub fn to_request(&self, base: &str) -> ApiRequest {
        match self {
            ListGet::List => ApiRequest::get(base),
            ListGet::Get { id } => ApiRequest::get(format!("{base}/{}", crate::request::seg(id))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_requires_leading_slash() {
        let raw = RawArgs {
            path: "rest/api/2/myself".into(),
            query: vec![],
        };
        assert!(raw.to_request().is_err());
    }

    #[test]
    fn raw_passes_query_through() {
        let raw = RawArgs {
            path: "/rest/x".into(),
            query: vec![("a".into(), "1".into())],
        };
        let r = raw.to_request().unwrap();
        assert_eq!(r.path, "/rest/x");
        assert_eq!(r.query, vec![("a".into(), "1".into())]);
    }

    #[test]
    fn list_get_paths() {
        assert_eq!(ListGet::List.to_request("/p").path, "/p");
        assert_eq!(
            ListGet::Get { id: "1 2".into() }.to_request("/p").path,
            "/p/1%202"
        );
    }
}
