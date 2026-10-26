//! Scope guardrails: limit which Jira projects, Confluence spaces and
//! Bitbucket repositories the CLI may read.
//!
//! Every command carries a [`Guard`] saying what it touches. Before the
//! request is sent, [`authorize`] checks the guard against the configured
//! [`Scope`]. Some checks need a lookup first: a Confluence page id doesn't
//! say which space it's in, so the page is fetched and its space checked.
//! After the request, a [`PostFilter`] drops list items outside the scope.
//!
//! Each product has its own scope (`[jira.scope]`, `[confluence.scope]`,
//! `[bitbucket.scope]`); a product without one is not restricted.

use anyhow::{Result, bail};
use serde::Deserialize;
use serde_json::Value;

use crate::client::{Client, Output, items_mut, page_items};
use crate::request::{ApiRequest, seg};

/// Cap on items fetched by `--all` when a scope is set and no `max_results`
/// is configured.
pub const DEFAULT_MAX_RESULTS: usize = 1000;

/// A product's `scope` table: `[jira.scope]`, `[confluence.scope]` or
/// `[bitbucket.scope]`. Each takes its own list (`projects`, `spaces` or
/// `repos`) plus the shared `max_results` and `allow_raw`.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProductScope {
    /// `[jira.scope]`: Jira project keys. Also limits `agile`.
    pub projects: Option<Vec<String>>,
    /// `[confluence.scope]`: Confluence space keys.
    pub spaces: Option<Vec<String>>,
    /// `[bitbucket.scope]`: `PROJECT/repo`, or `PROJECT/*` for every
    /// repository in a project.
    pub repos: Option<Vec<String>>,
    /// Most items `--all` may fetch (default 1000).
    pub max_results: Option<usize>,
    /// Allow the `raw` command, which can reach any endpoint.
    #[serde(default)]
    pub allow_raw: bool,
}

impl ProductScope {
    /// Check the values are well formed, and that `[<section>.scope]` only
    /// uses its own list key.
    pub fn validate(&self, section: &str) -> std::result::Result<(), String> {
        let own = match section {
            "jira" => "projects",
            "confluence" => "spaces",
            "bitbucket" => "repos",
            other => return Err(format!("unknown product `{other}`")),
        };
        for (key, set) in [
            ("projects", self.projects.is_some()),
            ("spaces", self.spaces.is_some()),
            ("repos", self.repos.is_some()),
        ] {
            if set && key != own {
                return Err(format!("[{section}.scope] takes `{own}`, not `{key}`"));
            }
        }
        for repo in self.repos.iter().flatten() {
            match repo.split_once('/') {
                Some((p, r)) if !p.is_empty() && !r.is_empty() && !r.contains('/') => {}
                _ => {
                    return Err(format!(
                        "repos entry `{repo}` must be `PROJECT/repo` or `PROJECT/*`"
                    ));
                }
            }
        }
        if self.max_results == Some(0) {
            return Err(format!("[{section}.scope] max_results must be at least 1"));
        }
        Ok(())
    }
}

/// What a command may read: the scope of the product it runs against, or
/// unrestricted when that product has no `scope` table.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Scope {
    config: Option<ProductScope>,
}

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

impl Scope {
    pub fn unrestricted() -> Self {
        Self { config: None }
    }

    pub fn restricted(config: ProductScope) -> Self {
        Self {
            config: Some(config),
        }
    }

    pub fn is_restricted(&self) -> bool {
        self.config.is_some()
    }

    /// Most items `--all` may return, when restricted.
    pub fn max_results(&self) -> Option<usize> {
        self.config
            .as_ref()
            .map(|c| c.max_results.unwrap_or(DEFAULT_MAX_RESULTS))
    }

    fn jira_projects(&self) -> &[String] {
        self.config
            .as_ref()
            .and_then(|c| c.projects.as_deref())
            .unwrap_or(&[])
    }

    fn confluence_spaces(&self) -> &[String] {
        self.config
            .as_ref()
            .and_then(|c| c.spaces.as_deref())
            .unwrap_or(&[])
    }

    fn bitbucket_repos(&self) -> &[String] {
        self.config
            .as_ref()
            .and_then(|c| c.repos.as_deref())
            .unwrap_or(&[])
    }

    pub fn allows_jira_project(&self, key: &str) -> bool {
        !self.is_restricted() || self.jira_projects().iter().any(|p| same(p, key))
    }

    pub fn allows_confluence_space(&self, key: &str) -> bool {
        !self.is_restricted() || self.confluence_spaces().iter().any(|s| same(s, key))
    }

    fn repo_patterns(&self) -> impl Iterator<Item = (&str, &str)> {
        self.bitbucket_repos()
            .iter()
            .filter_map(|r| r.split_once('/'))
    }

    pub fn allows_bitbucket_repo(&self, project: &str, repo: &str) -> bool {
        !self.is_restricted()
            || self
                .repo_patterns()
                .any(|(p, r)| same(p, project) && (r == "*" || same(r, repo)))
    }

    /// A project is visible when any allowed repository is in it.
    pub fn allows_bitbucket_project(&self, project: &str) -> bool {
        !self.is_restricted() || self.repo_patterns().any(|(p, _)| same(p, project))
    }

    fn allows_raw(&self) -> bool {
        self.config.as_ref().is_none_or(|c| c.allow_raw)
    }

    fn list(items: &[String]) -> String {
        if items.is_empty() {
            "none".into()
        } else {
            items.join(", ")
        }
    }
}

/// A `jira issue list` query, built from fixed filters rather than free
/// JQL. The project clause is added by [`authorize`], so the query can never
/// reach outside the allowed projects.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueQuery {
    pub projects: Vec<String>,
    /// Already-escaped JQL conditions, joined with AND.
    pub clauses: Vec<String>,
    /// `ORDER BY` clause, e.g. `updated DESC`.
    pub order_by: String,
}

/// Quote a value as a JQL string literal.
pub fn jql_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

impl IssueQuery {
    /// The final JQL, limited to `projects` when any are given.
    pub fn jql(&self, projects: &[String]) -> String {
        let mut clauses = Vec::new();
        if !projects.is_empty() {
            let list: Vec<String> = projects.iter().map(|p| jql_string(p)).collect();
            clauses.push(format!("project in ({})", list.join(", ")));
        }
        clauses.extend(self.clauses.iter().cloned());
        let filter = clauses.join(" AND ");
        let order = format!("ORDER BY {}", self.order_by);
        if filter.is_empty() {
            order
        } else {
            format!("{filter} {order}")
        }
    }
}

/// What a command touches, so the scope can be checked.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum Guard {
    /// Instance metadata or the caller's own account: always allowed.
    #[default]
    Open,
    /// Searches or lists across the whole instance (users, groups,
    /// dashboards, filters): blocked when a scope is set.
    Instance,
    /// The `raw` command: blocked when a scope is set, unless `allow_raw`.
    Raw,
    JiraProject(String),
    JiraIssue(String),
    JiraProjectList,
    JiraIssueSearch(IssueQuery),
    AgileBoardList(Option<String>),
    AgileBoard(String),
    AgileSprint(String),
    AgileEpic(String),
    /// Agile issue lists: the board, sprint or epic is checked, and issues
    /// from other projects are dropped from the result.
    AgileIssues(Box<Guard>),
    ConfluenceSpace(String),
    ConfluenceSpaceList(Vec<String>),
    ConfluenceContentList(Option<String>),
    ConfluenceContent(String),
    BitbucketProject(String),
    BitbucketProjectList,
    BitbucketRepo {
        project: String,
        repo: String,
    },
    BitbucketRepoList,
}

/// How to drop out-of-scope items from a list response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostFilter {
    None,
    /// Items with an issue `key` such as `ABC-1`.
    IssueKeys,
    /// Items with a Jira project `key`.
    JiraProjectKeys,
    /// Items with a Confluence space `key`.
    ConfluenceSpaceKeys,
    /// Items with a Bitbucket project `key`.
    BitbucketProjectKeys,
    /// Items with `slug` and `project.key`.
    BitbucketRepos,
}

/// Split an issue key `ABC-123` into its project key.
pub fn issue_project(key: &str) -> Option<&str> {
    let (project, number) = key.rsplit_once('-')?;
    let valid_project = project
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
        && project
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_');
    let valid_number = !number.is_empty() && number.chars().all(|c| c.is_ascii_digit());
    (valid_project && valid_number).then_some(project)
}

fn denied(message: String) -> anyhow::Error {
    anyhow::anyhow!("blocked by scope: {message}")
}

impl Scope {
    fn check_jira_project(&self, key: &str) -> Result<()> {
        if self.allows_jira_project(key) {
            Ok(())
        } else {
            Err(denied(format!(
                "Jira project `{key}` is not allowed (allowed: {})",
                Self::list(self.jira_projects())
            )))
        }
    }

    fn check_issue(&self, key: &str) -> Result<()> {
        if !self.is_restricted() {
            return Ok(());
        }
        match issue_project(key) {
            Some(project) => self.check_jira_project(project),
            None => Err(denied(format!(
                "`{key}` is not an issue key such as ABC-123; use the key, not the numeric id"
            ))),
        }
    }

    fn check_space(&self, key: &str) -> Result<()> {
        if self.allows_confluence_space(key) {
            Ok(())
        } else {
            Err(denied(format!(
                "Confluence space `{key}` is not allowed (allowed: {})",
                Self::list(self.confluence_spaces())
            )))
        }
    }

    fn repos(&self) -> String {
        Self::list(self.bitbucket_repos())
    }
}

/// Check `req` against `scope`, finish any request the scope shapes (the JQL
/// of `jira issue list`, the spaces of `confluence space list`), and say how
/// to filter the response. Fails without sending `req` when it is out of
/// scope; may send lookup requests through `client` to find out.
pub async fn authorize(req: &mut ApiRequest, scope: &Scope, client: &Client) -> Result<PostFilter> {
    let guard = req.guard.clone();
    authorize_guard(&guard, req, scope, client).await
}

async fn authorize_guard(
    guard: &Guard,
    req: &mut ApiRequest,
    scope: &Scope,
    client: &Client,
) -> Result<PostFilter> {
    let restricted = scope.is_restricted();
    // A page may not be bigger than the whole allowance.
    if let Some(max) = scope.max_results()
        && let Some(paging) = req.paging.as_mut()
        && let Some(limit) = paging.limit
        && limit > max as u64
    {
        paging.limit = Some(max as u64);
    }
    match guard {
        Guard::Open => Ok(PostFilter::None),
        Guard::Instance if restricted => Err(denied(
            "this command reads across the whole instance (users, groups, filters or \
             dashboards) and is disabled when a scope is set"
                .into(),
        )),
        Guard::Instance => Ok(PostFilter::None),
        Guard::Raw if !scope.allows_raw() => Err(denied(
            "`raw` can reach any endpoint and is disabled when a scope is set".into(),
        )),
        Guard::Raw => Ok(PostFilter::None),
        Guard::JiraProject(key) => {
            scope.check_jira_project(key)?;
            Ok(PostFilter::None)
        }
        Guard::JiraIssue(key) => {
            scope.check_issue(key)?;
            Ok(PostFilter::None)
        }
        Guard::JiraProjectList => Ok(if restricted {
            PostFilter::JiraProjectKeys
        } else {
            PostFilter::None
        }),
        Guard::JiraIssueSearch(query) => {
            let projects = if restricted && query.projects.is_empty() {
                scope.jira_projects().to_vec()
            } else {
                for p in &query.projects {
                    scope.check_jira_project(p)?;
                }
                query.projects.clone()
            };
            if restricted && projects.is_empty() {
                return Err(denied("no Jira projects are allowed".into()));
            }
            req.query.insert(0, ("jql".into(), query.jql(&projects)));
            Ok(if restricted {
                PostFilter::IssueKeys
            } else {
                PostFilter::None
            })
        }
        Guard::AgileBoardList(project) => {
            if restricted {
                match project {
                    Some(p) => scope.check_jira_project(p)?,
                    None => {
                        return Err(denied(format!(
                            "pass --project with one of the allowed projects ({})",
                            Scope::list(scope.jira_projects())
                        )));
                    }
                }
            }
            Ok(PostFilter::None)
        }
        Guard::AgileBoard(id) => {
            if restricted {
                check_board(id, scope, client).await?;
            }
            Ok(PostFilter::None)
        }
        Guard::AgileSprint(id) => {
            if restricted {
                let sprint =
                    fetch_json(client, &format!("/rest/agile/1.0/sprint/{}", seg(id))).await?;
                let board = sprint
                    .get("originBoardId")
                    .and_then(|v| v.as_u64().map(|n| n.to_string()))
                    .ok_or_else(|| {
                        denied(format!("can't tell which board sprint {id} belongs to"))
                    })?;
                check_board(&board, scope, client).await?;
            }
            Ok(PostFilter::None)
        }
        Guard::AgileEpic(id) => {
            if restricted {
                let key = if issue_project(id).is_some() {
                    id.clone()
                } else {
                    let epic =
                        fetch_json(client, &format!("/rest/agile/1.0/epic/{}", seg(id))).await?;
                    epic.get("key")
                        .and_then(Value::as_str)
                        .map(String::from)
                        .ok_or_else(|| {
                            denied(format!("can't tell which project epic {id} is in"))
                        })?
                };
                scope.check_issue(&key)?;
            }
            Ok(PostFilter::None)
        }
        Guard::AgileIssues(inner) => {
            Box::pin(authorize_guard(inner, req, scope, client)).await?;
            Ok(if restricted {
                PostFilter::IssueKeys
            } else {
                PostFilter::None
            })
        }
        Guard::ConfluenceSpace(key) => {
            scope.check_space(key)?;
            Ok(PostFilter::None)
        }
        Guard::ConfluenceSpaceList(keys) => {
            if !restricted {
                return Ok(PostFilter::None);
            }
            if keys.is_empty() {
                let allowed = scope.confluence_spaces().to_vec();
                if allowed.is_empty() {
                    return Err(denied("no Confluence spaces are allowed".into()));
                }
                for key in allowed {
                    req.query.push(("spaceKey".into(), key));
                }
            } else {
                for key in keys {
                    scope.check_space(key)?;
                }
            }
            Ok(PostFilter::ConfluenceSpaceKeys)
        }
        Guard::ConfluenceContentList(space) => {
            if restricted {
                match space {
                    Some(s) => scope.check_space(s)?,
                    None => {
                        return Err(denied("pass --space with one of the allowed spaces".into()));
                    }
                }
            }
            Ok(PostFilter::None)
        }
        Guard::ConfluenceContent(id) => {
            if restricted {
                let path = format!("/rest/api/content/{}", seg(id));
                let content = match client
                    .get(&path, &[("expand".into(), "space".into())])
                    .await?
                {
                    Output::Json(v) => v,
                    Output::Text(_) => Value::Null,
                };
                let space = content
                    .pointer("/space/key")
                    .and_then(Value::as_str)
                    .ok_or_else(|| denied(format!("can't tell which space content {id} is in")))?;
                scope.check_space(space)?;
            }
            Ok(PostFilter::None)
        }
        Guard::BitbucketProject(key) => {
            if scope.allows_bitbucket_project(key) {
                Ok(PostFilter::None)
            } else {
                Err(denied(format!(
                    "Bitbucket project `{key}` has no allowed repositories (allowed: {})",
                    scope.repos()
                )))
            }
        }
        Guard::BitbucketProjectList => Ok(if restricted {
            PostFilter::BitbucketProjectKeys
        } else {
            PostFilter::None
        }),
        Guard::BitbucketRepo { project, repo } => {
            if scope.allows_bitbucket_repo(project, repo) {
                Ok(PostFilter::None)
            } else {
                Err(denied(format!(
                    "Bitbucket repository `{project}/{repo}` is not allowed (allowed: {})",
                    scope.repos()
                )))
            }
        }
        Guard::BitbucketRepoList => Ok(if restricted {
            PostFilter::BitbucketRepos
        } else {
            PostFilter::None
        }),
    }
}

async fn fetch_json(client: &Client, path: &str) -> Result<Value> {
    match client.get(path, &[]).await? {
        Output::Json(v) => Ok(v),
        Output::Text(_) => bail!("GET {path} did not return JSON"),
    }
}

/// Every project a board shows must be allowed.
async fn check_board(id: &str, scope: &Scope, client: &Client) -> Result<()> {
    let page = fetch_json(
        client,
        &format!("/rest/agile/1.0/board/{}/project", seg(id)),
    )
    .await?;
    let projects: Vec<&Value> = match (&page, page_items(&page)) {
        (_, Some(items)) => items.iter().collect(),
        (Value::Array(items), None) => items.iter().collect(),
        (single @ Value::Object(_), None) => vec![single],
        _ => Vec::new(),
    };
    let keys: Vec<&str> = projects
        .iter()
        .filter_map(|p| p.get("key").and_then(Value::as_str))
        .collect();
    if keys.is_empty() {
        return Err(denied(format!(
            "can't tell which projects board {id} shows"
        )));
    }
    for key in keys {
        scope.check_jira_project(key)?;
    }
    Ok(())
}

impl PostFilter {
    fn keep(self, item: &Value, scope: &Scope) -> bool {
        let str_at = |ptr: &str| item.pointer(ptr).and_then(Value::as_str);
        match self {
            PostFilter::None => true,
            PostFilter::IssueKeys => str_at("/key")
                .and_then(issue_project)
                .is_some_and(|p| scope.allows_jira_project(p)),
            PostFilter::JiraProjectKeys => {
                str_at("/key").is_some_and(|k| scope.allows_jira_project(k))
            }
            PostFilter::ConfluenceSpaceKeys => {
                str_at("/key").is_some_and(|k| scope.allows_confluence_space(k))
            }
            PostFilter::BitbucketProjectKeys => {
                str_at("/key").is_some_and(|k| scope.allows_bitbucket_project(k))
            }
            PostFilter::BitbucketRepos => match (str_at("/project/key"), str_at("/slug")) {
                (Some(p), Some(r)) => scope.allows_bitbucket_repo(p, r),
                _ => false,
            },
        }
    }

    /// Drop items outside `scope` from a list response (a page or an array).
    pub fn apply(self, output: Output, scope: &Scope) -> Output {
        if self == PostFilter::None {
            return output;
        }
        match output {
            Output::Json(mut v) => {
                if let Some(items) = items_mut(&mut v) {
                    items.retain(|item| self.keep(item, scope));
                }
                Output::Json(v)
            }
            text => text,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn scope() -> Scope {
        // One scope with every list, so each check can be tested here; in
        // a config file each product's table holds only its own list.
        Scope::restricted(ProductScope {
            projects: Some(vec!["ABC".into(), "ops".into()]),
            spaces: Some(vec!["DEV".into()]),
            repos: Some(vec!["PROJ/api".into(), "WEB/*".into()]),
            max_results: None,
            allow_raw: false,
        })
    }

    #[test]
    fn issue_keys() {
        assert_eq!(issue_project("ABC-123"), Some("ABC"));
        assert_eq!(issue_project("A_B2-1"), Some("A_B2"));
        assert_eq!(issue_project("10001"), None);
        assert_eq!(issue_project("ABC-"), None);
        assert_eq!(issue_project("-1"), None);
        assert_eq!(issue_project("1AB-1"), None);
    }

    #[test]
    fn matching_is_case_insensitive() {
        let s = scope();
        assert!(s.allows_jira_project("abc") && s.allows_jira_project("OPS"));
        assert!(!s.allows_jira_project("XYZ"));
        assert!(s.allows_confluence_space("dev") && !s.allows_confluence_space("HR"));
    }

    #[test]
    fn bitbucket_patterns() {
        let s = scope();
        assert!(s.allows_bitbucket_repo("PROJ", "api"));
        assert!(!s.allows_bitbucket_repo("PROJ", "secrets"));
        assert!(s.allows_bitbucket_repo("web", "anything"));
        assert!(s.allows_bitbucket_project("PROJ") && s.allows_bitbucket_project("WEB"));
        assert!(!s.allows_bitbucket_project("HR"));
    }

    #[test]
    fn unrestricted_allows_everything() {
        let s = Scope::unrestricted();
        assert!(s.allows_jira_project("X") && s.allows_confluence_space("X"));
        assert!(s.allows_bitbucket_repo("X", "y") && s.allows_raw());
        assert_eq!(s.max_results(), None);
    }

    #[test]
    fn max_results_defaults_when_restricted() {
        assert_eq!(scope().max_results(), Some(DEFAULT_MAX_RESULTS));
    }

    #[test]
    fn validate_checks_keys_and_patterns() {
        let ok = ProductScope {
            repos: Some(vec!["PROJ/api".into(), "PROJ/*".into()]),
            ..Default::default()
        };
        assert!(ok.validate("bitbucket").is_ok());
        for bad in ["PROJ", "/api", "PROJ/", "A/b/c"] {
            let s = ProductScope {
                repos: Some(vec![bad.into()]),
                ..Default::default()
            };
            assert!(s.validate("bitbucket").is_err(), "{bad}");
        }
        let wrong_key = ProductScope {
            spaces: Some(vec!["DEV".into()]),
            ..Default::default()
        };
        let err = wrong_key.validate("jira").unwrap_err();
        assert!(err.contains("takes `projects`, not `spaces`"), "{err}");
        let zero = ProductScope {
            max_results: Some(0),
            ..Default::default()
        };
        assert!(zero.validate("jira").is_err());
    }

    #[test]
    fn jql_quotes_values_and_scopes_projects() {
        let q = IssueQuery {
            projects: vec![],
            clauses: vec![format!("status in ({})", jql_string(r#"Say "hi" \o/"#))],
            order_by: "updated DESC".into(),
        };
        assert_eq!(
            q.jql(&["ABC".into(), "OPS".into()]),
            r#"project in ("ABC", "OPS") AND status in ("Say \"hi\" \\o/") ORDER BY updated DESC"#
        );
        let empty = IssueQuery {
            order_by: "key ASC".into(),
            ..Default::default()
        };
        assert_eq!(empty.jql(&[]), "ORDER BY key ASC");
    }

    #[test]
    fn post_filters_drop_out_of_scope_items() {
        let s = scope();
        let page = json!({"issues": [{"key": "ABC-1"}, {"key": "XYZ-2"}, {"key": "OPS-3"}]});
        let out = PostFilter::IssueKeys.apply(Output::Json(page), &s);
        assert_eq!(
            out,
            Output::Json(json!({"issues": [{"key": "ABC-1"}, {"key": "OPS-3"}]}))
        );

        let repos = json!([
            {"slug": "api", "project": {"key": "PROJ"}},
            {"slug": "secrets", "project": {"key": "PROJ"}},
            {"slug": "site", "project": {"key": "WEB"}},
            {"slug": "x"}
        ]);
        let out = PostFilter::BitbucketRepos.apply(Output::Json(repos), &s);
        assert_eq!(
            out,
            Output::Json(json!([
                {"slug": "api", "project": {"key": "PROJ"}},
                {"slug": "site", "project": {"key": "WEB"}}
            ]))
        );

        let projects = json!([{"key": "ABC"}, {"key": "HR"}]);
        let out = PostFilter::JiraProjectKeys.apply(Output::Json(projects), &s);
        assert_eq!(out, Output::Json(json!([{"key": "ABC"}])));
    }
}
