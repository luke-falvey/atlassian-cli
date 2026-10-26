//! Jira Data Center platform REST API (`/rest/api/2`).

use anyhow::Result;
use clap::Subcommand;

use clap::ValueEnum;

use super::{ListGet, RawArgs};
use crate::request::{ApiRequest, Guard, PageArgs, PageStyle, seg};
use crate::scope::{IssueQuery, jql_string};

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
    /// List issues, filtered by project, people, status, type and dates.
    List(IssueListArgs),
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

/// Fixed filters for `jira issue list`. There is no free-form JQL: the CLI
/// builds the query itself, which keeps it inside the allowed projects.
#[derive(Debug, Clone, clap::Args)]
pub struct IssueListArgs {
    /// Only issues in this project key, e.g. `ABC`. Repeat for several.
    #[arg(long = "project", value_name = "KEY", value_parser = parse_project_key)]
    pub projects: Vec<String>,
    /// Only issues assigned to this username, or `me`.
    #[arg(long, value_name = "USER")]
    pub assignee: Option<String>,
    /// Only issues reported by this username, or `me`.
    #[arg(long, value_name = "USER")]
    pub reporter: Option<String>,
    /// Only issues with this status, e.g. `In Progress`. Repeat for several.
    #[arg(long = "status", value_name = "STATUS")]
    pub statuses: Vec<String>,
    /// Only issues of this type, e.g. `Bug`. Repeat for several.
    #[arg(long = "type", value_name = "TYPE")]
    pub types: Vec<String>,
    /// Only unresolved issues.
    #[arg(long)]
    pub open: bool,
    /// Only issues updated within this period: a number and `m`, `h`, `d` or
    /// `w`, e.g. `7d`.
    #[arg(long, value_name = "PERIOD", value_parser = parse_period)]
    pub updated_since: Option<String>,
    /// Only issues created within this period, e.g. `30d`.
    #[arg(long, value_name = "PERIOD", value_parser = parse_period)]
    pub created_since: Option<String>,
    /// Sort order (newest or highest first, except `key`).
    #[arg(long, value_enum, default_value = "updated")]
    pub order_by: IssueOrder,
    /// Comma-separated fields to return, e.g. `summary,status,assignee`.
    #[arg(long)]
    pub fields: Option<String>,
    /// Comma-separated entities to expand, e.g. `changelog`.
    #[arg(long)]
    pub expand: Option<String>,
    #[command(flatten)]
    pub page: PageArgs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum IssueOrder {
    Updated,
    Created,
    Priority,
    Key,
}

fn parse_project_key(s: &str) -> Result<String, String> {
    let ok = s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if ok {
        Ok(s.to_string())
    } else {
        Err(format!("`{s}` is not a project key such as ABC"))
    }
}

fn parse_period(s: &str) -> Result<String, String> {
    let (number, unit) = s.split_at(s.len().saturating_sub(1));
    if !number.is_empty()
        && number.chars().all(|c| c.is_ascii_digit())
        && matches!(unit, "m" | "h" | "d" | "w")
    {
        Ok(s.to_string())
    } else {
        Err(format!("`{s}` is not a period such as 30m, 12h, 7d or 2w"))
    }
}

/// `currentUser()` for `me`, otherwise a quoted username.
fn jql_user(user: &str) -> String {
    if user.eq_ignore_ascii_case("me") {
        "currentUser()".into()
    } else {
        jql_string(user)
    }
}

fn jql_list(values: &[String]) -> String {
    values
        .iter()
        .map(|v| jql_string(v))
        .collect::<Vec<_>>()
        .join(", ")
}

impl IssueListArgs {
    /// The query, minus the project clause, which the scope check adds.
    pub fn query(&self) -> IssueQuery {
        let mut clauses = Vec::new();
        if let Some(user) = &self.assignee {
            clauses.push(format!("assignee = {}", jql_user(user)));
        }
        if let Some(user) = &self.reporter {
            clauses.push(format!("reporter = {}", jql_user(user)));
        }
        if !self.statuses.is_empty() {
            clauses.push(format!("status in ({})", jql_list(&self.statuses)));
        }
        if !self.types.is_empty() {
            clauses.push(format!("issuetype in ({})", jql_list(&self.types)));
        }
        if self.open {
            clauses.push("resolution = Unresolved".into());
        }
        if let Some(period) = &self.updated_since {
            clauses.push(format!("updated >= -{period}"));
        }
        if let Some(period) = &self.created_since {
            clauses.push(format!("created >= -{period}"));
        }
        let order_by = match self.order_by {
            IssueOrder::Updated => "updated DESC",
            IssueOrder::Created => "created DESC",
            IssueOrder::Priority => "priority DESC",
            IssueOrder::Key => "key ASC",
        };
        IssueQuery {
            projects: self.projects.clone(),
            clauses,
            order_by: order_by.into(),
        }
    }
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
            Project(cmd) => cmd.to_request(),
            // Users, groups, filters and dashboards span the whole instance.
            User(_) | Group(_) | Filter(_) | Dashboard(_) => {
                self.instance_request().guard(Guard::Instance)
            }
            Field(FieldCmd::List) => ApiRequest::get(p("/field")),
            IssueType(cmd) => cmd.to_request(&p("/issuetype")),
            IssueLinkType(cmd) => cmd.to_request(&p("/issueLinkType")),
            Priority(cmd) => cmd.to_request(&p("/priority")),
            Status(cmd) => cmd.to_request(&p("/status")),
            Resolution(cmd) => cmd.to_request(&p("/resolution")),
            Raw(raw) => return raw.to_request(),
        })
    }

    fn instance_request(&self) -> ApiRequest {
        use JiraCommand::*;
        match self {
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
            _ => unreachable!("only instance-wide commands"),
        }
    }
}

impl IssueCmd {
    fn to_request(&self) -> ApiRequest {
        let issue = |key: &str, rest: &str| {
            ApiRequest::get(p(&format!("/issue/{}{rest}", seg(key))))
                .guard(Guard::JiraIssue(key.to_string()))
        };
        match self {
            IssueCmd::List(args) => ApiRequest::get(p("/search"))
                .opt("fields", args.fields.as_ref())
                .opt("expand", args.expand.as_ref())
                .paged(PageStyle::Jira, &args.page)
                .guard(Guard::JiraIssueSearch(args.query())),
            IssueCmd::Get {
                key,
                fields,
                expand,
            } => issue(key, "")
                .opt("fields", fields.as_ref())
                .opt("expand", expand.as_ref()),
            IssueCmd::Comments { key, page } => issue(key, "/comment").paged(PageStyle::Jira, page),
            IssueCmd::Worklogs { key } => issue(key, "/worklog"),
            IssueCmd::Transitions { key } => issue(key, "/transitions"),
            IssueCmd::Watchers { key } => issue(key, "/watchers"),
            IssueCmd::Votes { key } => issue(key, "/votes"),
            IssueCmd::RemoteLinks { key } => issue(key, "/remotelink"),
        }
    }
}

impl ProjectCmd {
    fn to_request(&self) -> ApiRequest {
        let project = |key: &str, rest: &str| {
            ApiRequest::get(p(&format!("/project/{}{rest}", seg(key))))
                .guard(Guard::JiraProject(key.to_string()))
        };
        match self {
            ProjectCmd::List { expand } => ApiRequest::get(p("/project"))
                .opt("expand", expand.as_ref())
                .guard(Guard::JiraProjectList),
            ProjectCmd::Get { key, expand } => project(key, "").opt("expand", expand.as_ref()),
            ProjectCmd::Components { key } => project(key, "/components"),
            ProjectCmd::Versions { key } => project(key, "/versions"),
            ProjectCmd::Statuses { key } => project(key, "/statuses"),
            ProjectCmd::Roles { key } => project(key, "/role"),
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
    fn issue_list_builds_a_fixed_query() {
        let r = req(&[
            "issue",
            "list",
            "--project",
            "ABC",
            "--assignee",
            "me",
            "--status",
            "In Progress",
            "--status",
            "To Do",
            "--type",
            "Bug",
            "--open",
            "--updated-since",
            "7d",
            "--order-by",
            "priority",
            "--start",
            "10",
            "--limit",
            "5",
        ]);
        assert_eq!(r.path, "/rest/api/2/search");
        // The JQL itself is added by the scope check, which knows the projects.
        assert_eq!(q(&r), kv(&[("startAt", "10"), ("maxResults", "5")]));
        let crate::request::Guard::JiraIssueSearch(query) = r.guard else {
            panic!("expected an issue search guard, got {:?}", r.guard);
        };
        assert_eq!(query.projects, vec!["ABC".to_string()]);
        assert_eq!(
            query.jql(&query.projects),
            "project in (\"ABC\") AND assignee = currentUser() AND \
             status in (\"In Progress\", \"To Do\") AND issuetype in (\"Bug\") AND \
             resolution = Unresolved AND updated >= -7d ORDER BY priority DESC"
        );
    }

    #[test]
    fn issue_list_quotes_user_values() {
        let r = req(&[
            "issue",
            "list",
            "--reporter",
            "o\"brien",
            "--created-since",
            "2w",
        ]);
        let crate::request::Guard::JiraIssueSearch(query) = r.guard else {
            panic!("expected an issue search guard");
        };
        assert_eq!(
            query.jql(&[]),
            "reporter = \"o\\\"brien\" AND created >= -2w ORDER BY updated DESC"
        );
    }

    #[test]
    fn issue_list_rejects_bad_values() {
        use clap::Parser;
        for args in [
            vec!["--project", "ABC) OR project = XYZ"],
            vec!["--updated-since", "7 days"],
            vec!["--updated-since", "d"],
        ] {
            let mut argv = vec!["atlassian", "jira", "issue", "list"];
            argv.extend(args.iter().copied());
            assert!(Cli::try_parse_from(&argv).is_err(), "{argv:?}");
        }
    }

    #[test]
    fn search_command_is_gone() {
        use clap::Parser;
        assert!(Cli::try_parse_from(["atlassian", "jira", "search", "--jql", "x"]).is_err());
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
