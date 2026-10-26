//! Jira Data Center platform REST API (`/rest/api/2`).

use anyhow::Result;
use clap::Subcommand;

use super::{ListGet, RawArgs};
use crate::request::{ApiRequest, PageArgs, PageStyle, seg};

const API: &str = "/rest/api/2";

#[derive(Debug, Clone, Subcommand)]
pub enum JiraCommand {
    /// Server version and build information.
    ServerInfo,
    /// The user whose credentials are in use.
    Myself,
    /// Issues and their sub-resources.
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Search for issues with JQL.
    Search {
        /// JQL query, e.g. `project = ABC AND status = Open`.
        #[arg(long)]
        jql: String,
        /// Comma-separated fields to return (default: navigable fields).
        #[arg(long)]
        fields: Option<String>,
        /// Comma-separated entities to expand, e.g. `changelog`.
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Projects and their components, versions, statuses and roles.
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Users.
    #[command(subcommand)]
    User(UserCmd),
    /// Groups.
    #[command(subcommand)]
    Group(GroupCmd),
    /// System and custom fields.
    #[command(subcommand)]
    Field(FieldCmd),
    /// Issue types.
    #[command(subcommand)]
    IssueType(ListGet),
    /// Issue link types.
    #[command(subcommand)]
    IssueLinkType(ListGet),
    /// Priorities.
    #[command(subcommand)]
    Priority(ListGet),
    /// Statuses.
    #[command(subcommand)]
    Status(ListGet),
    /// Resolutions.
    #[command(subcommand)]
    Resolution(ListGet),
    /// Saved filters.
    #[command(subcommand)]
    Filter(FilterCmd),
    /// Dashboards.
    #[command(subcommand)]
    Dashboard(DashboardCmd),
    /// GET any Jira REST path.
    Raw(RawArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum IssueCmd {
    /// Get an issue by key or id.
    Get {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
        /// Comma-separated fields to return.
        #[arg(long)]
        fields: Option<String>,
        /// Comma-separated entities to expand, e.g. `changelog,renderedFields`.
        #[arg(long)]
        expand: Option<String>,
    },
    /// Comments on an issue.
    Comments {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Work logs on an issue.
    Worklogs {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
    },
    /// Transitions available to the current user.
    Transitions {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
    },
    /// Users watching an issue.
    Watchers {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
    },
    /// Votes on an issue.
    Votes {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
    },
    /// Remote links (e.g. Confluence pages) on an issue.
    RemoteLinks {
        /// Issue key or id, e.g. `ABC-123`.
        key: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectCmd {
    /// List projects visible to the current user.
    List {
        /// Comma-separated entities to expand, e.g. `description,lead`.
        #[arg(long)]
        expand: Option<String>,
    },
    /// Get a project by key or id.
    Get {
        /// Project key or id, e.g. `ABC`.
        key: String,
        /// Comma-separated entities to expand, e.g. `description,lead`.
        #[arg(long)]
        expand: Option<String>,
    },
    /// Components of a project.
    Components {
        /// Project key or id, e.g. `ABC`.
        key: String,
    },
    /// Versions of a project.
    Versions {
        /// Project key or id, e.g. `ABC`.
        key: String,
    },
    /// Issue types and their statuses for a project.
    Statuses {
        /// Project key or id, e.g. `ABC`.
        key: String,
    },
    /// Roles defined for a project.
    Roles {
        /// Project key or id, e.g. `ABC`.
        key: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum UserCmd {
    /// Get a user by username.
    Get {
        /// Username (login name), e.g. `alice`.
        username: String,
    },
    /// Find users whose username, name or email matches.
    Search {
        /// Text to match against username, display name or email.
        query: String,
        /// Include inactive users.
        #[arg(long)]
        include_inactive: bool,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum GroupCmd {
    /// Members of a group.
    Members {
        /// Group name, e.g. `jira-users`.
        name: String,
        /// Include inactive users.
        #[arg(long)]
        include_inactive: bool,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Find groups whose name matches.
    Search {
        /// Text to match against group names.
        query: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum FieldCmd {
    /// List all fields.
    List,
}

#[derive(Debug, Clone, Subcommand)]
pub enum FilterCmd {
    /// Get a filter by id.
    Get {
        /// Filter id (from `jira filter favourite` or the filter's URL).
        id: String,
    },
    /// The current user's favourite filters.
    Favourite,
}

#[derive(Debug, Clone, Subcommand)]
pub enum DashboardCmd {
    /// List dashboards.
    List {
        /// `favourite` or `my`.
        #[arg(long)]
        filter: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a dashboard by id.
    Get {
        /// Dashboard id (from `jira dashboard list`).
        id: String,
    },
}

fn p(rest: &str) -> String {
    format!("{API}{rest}")
}

impl JiraCommand {
    pub fn to_request(&self) -> Result<ApiRequest> {
        use JiraCommand::*;
        Ok(match self {
            ServerInfo => ApiRequest::get(p("/serverInfo")),
            Myself => ApiRequest::get(p("/myself")),
            Issue(cmd) => cmd.to_request(),
            Search {
                jql,
                fields,
                expand,
                page,
            } => ApiRequest::get(p("/search"))
                .param("jql", jql)
                .opt("fields", fields.as_ref())
                .opt("expand", expand.as_ref())
                .paged(PageStyle::Jira, page),
            Project(cmd) => cmd.to_request(),
            User(cmd) => match cmd {
                UserCmd::Get { username } => {
                    ApiRequest::get(p("/user")).param("username", username)
                }
                UserCmd::Search {
                    query,
                    include_inactive,
                    page,
                } => ApiRequest::get(p("/user/search"))
                    .param("username", query)
                    .flag("includeInactive", *include_inactive)
                    .paged(PageStyle::Jira, page),
            },
            Group(cmd) => match cmd {
                GroupCmd::Members {
                    name,
                    include_inactive,
                    page,
                } => ApiRequest::get(p("/group/member"))
                    .param("groupname", name)
                    .flag("includeInactiveUsers", *include_inactive)
                    .paged(PageStyle::Jira, page),
                GroupCmd::Search { query } => {
                    ApiRequest::get(p("/groups/picker")).param("query", query)
                }
            },
            Field(FieldCmd::List) => ApiRequest::get(p("/field")),
            IssueType(cmd) => cmd.to_request(&p("/issuetype")),
            IssueLinkType(cmd) => cmd.to_request(&p("/issueLinkType")),
            Priority(cmd) => cmd.to_request(&p("/priority")),
            Status(cmd) => cmd.to_request(&p("/status")),
            Resolution(cmd) => cmd.to_request(&p("/resolution")),
            Filter(cmd) => match cmd {
                FilterCmd::Get { id } => ApiRequest::get(p(&format!("/filter/{}", seg(id)))),
                FilterCmd::Favourite => ApiRequest::get(p("/filter/favourite")),
            },
            Dashboard(cmd) => match cmd {
                DashboardCmd::List { filter, page } => ApiRequest::get(p("/dashboard"))
                    .opt("filter", filter.as_ref())
                    .paged(PageStyle::Jira, page),
                DashboardCmd::Get { id } => ApiRequest::get(p(&format!("/dashboard/{}", seg(id)))),
            },
            Raw(raw) => return raw.to_request(),
        })
    }
}

impl IssueCmd {
    fn to_request(&self) -> ApiRequest {
        let issue = |key: &str, rest: &str| p(&format!("/issue/{}{rest}", seg(key)));
        match self {
            IssueCmd::Get {
                key,
                fields,
                expand,
            } => ApiRequest::get(issue(key, ""))
                .opt("fields", fields.as_ref())
                .opt("expand", expand.as_ref()),
            IssueCmd::Comments { key, page } => {
                ApiRequest::get(issue(key, "/comment")).paged(PageStyle::Jira, page)
            }
            IssueCmd::Worklogs { key } => ApiRequest::get(issue(key, "/worklog")),
            IssueCmd::Transitions { key } => ApiRequest::get(issue(key, "/transitions")),
            IssueCmd::Watchers { key } => ApiRequest::get(issue(key, "/watchers")),
            IssueCmd::Votes { key } => ApiRequest::get(issue(key, "/votes")),
            IssueCmd::RemoteLinks { key } => ApiRequest::get(issue(key, "/remotelink")),
        }
    }
}

impl ProjectCmd {
    fn to_request(&self) -> ApiRequest {
        let project = |key: &str, rest: &str| p(&format!("/project/{}{rest}", seg(key)));
        match self {
            ProjectCmd::List { expand } => {
                ApiRequest::get(p("/project")).opt("expand", expand.as_ref())
            }
            ProjectCmd::Get { key, expand } => {
                ApiRequest::get(project(key, "")).opt("expand", expand.as_ref())
            }
            ProjectCmd::Components { key } => ApiRequest::get(project(key, "/components")),
            ProjectCmd::Versions { key } => ApiRequest::get(project(key, "/versions")),
            ProjectCmd::Statuses { key } => ApiRequest::get(project(key, "/statuses")),
            ProjectCmd::Roles { key } => ApiRequest::get(project(key, "/role")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::request::ApiRequest;
    use clap::Parser;

    fn req(args: &[&str]) -> ApiRequest {
        let mut argv = vec!["atlassian", "jira"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap().to_request().unwrap()
    }

    fn q(r: &ApiRequest) -> Vec<(String, String)> {
        r.first_query()
    }

    fn kv(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn simple_paths() {
        assert_eq!(req(&["server-info"]).path, "/rest/api/2/serverInfo");
        assert_eq!(req(&["myself"]).path, "/rest/api/2/myself");
        assert_eq!(req(&["field", "list"]).path, "/rest/api/2/field");
        assert_eq!(
            req(&["priority", "get", "3"]).path,
            "/rest/api/2/priority/3"
        );
        assert_eq!(req(&["issue-type", "list"]).path, "/rest/api/2/issuetype");
        assert_eq!(
            req(&["issue-link-type", "list"]).path,
            "/rest/api/2/issueLinkType"
        );
        assert_eq!(
            req(&["filter", "favourite"]).path,
            "/rest/api/2/filter/favourite"
        );
    }

    #[test]
    fn issue_get_with_fields_and_expand() {
        let r = req(&[
            "issue",
            "get",
            "ABC-1",
            "--fields",
            "summary",
            "--expand",
            "changelog",
        ]);
        assert_eq!(r.path, "/rest/api/2/issue/ABC-1");
        assert_eq!(q(&r), kv(&[("fields", "summary"), ("expand", "changelog")]));
    }

    #[test]
    fn issue_sub_resources() {
        assert_eq!(
            req(&["issue", "comments", "ABC-1"]).path,
            "/rest/api/2/issue/ABC-1/comment"
        );
        assert_eq!(
            req(&["issue", "remote-links", "ABC-1"]).path,
            "/rest/api/2/issue/ABC-1/remotelink"
        );
    }

    #[test]
    fn search_paging() {
        let r = req(&[
            "search",
            "--jql",
            "project = ABC",
            "--start",
            "10",
            "--limit",
            "5",
        ]);
        assert_eq!(r.path, "/rest/api/2/search");
        assert_eq!(
            q(&r),
            kv(&[
                ("jql", "project = ABC"),
                ("startAt", "10"),
                ("maxResults", "5")
            ])
        );
    }

    #[test]
    fn search_all_sets_paging_flag() {
        let r = req(&["search", "--jql", "x", "--all"]);
        assert!(r.paging.unwrap().all);
    }

    #[test]
    fn user_and_group_queries() {
        let r = req(&["user", "get", "alice"]);
        assert_eq!(
            (r.path.as_str(), q(&r)),
            ("/rest/api/2/user", kv(&[("username", "alice")]))
        );
        let r = req(&["group", "members", "jira-users", "--include-inactive"]);
        assert_eq!(r.path, "/rest/api/2/group/member");
        assert_eq!(
            q(&r),
            kv(&[
                ("groupname", "jira-users"),
                ("includeInactiveUsers", "true")
            ])
        );
    }

    #[test]
    fn project_paths() {
        assert_eq!(
            req(&["project", "versions", "ABC"]).path,
            "/rest/api/2/project/ABC/versions"
        );
        assert_eq!(
            req(&["project", "roles", "ABC"]).path,
            "/rest/api/2/project/ABC/role"
        );
    }

    #[test]
    fn raw_path() {
        let r = req(&["raw", "/rest/api/2/configuration", "-q", "a=b"]);
        assert_eq!(r.path, "/rest/api/2/configuration");
        assert_eq!(r.query, kv(&[("a", "b")]));
    }
}
