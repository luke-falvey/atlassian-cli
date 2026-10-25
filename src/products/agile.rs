//! Jira Software (Agile) REST API (`/rest/agile/1.0`). Served by the Jira
//! instance, so it uses the Jira base URL.

use anyhow::Result;
use clap::Subcommand;

use super::RawArgs;
use crate::request::{ApiRequest, PageArgs, PageStyle, seg};

const API: &str = "/rest/agile/1.0";

#[derive(Debug, Clone, Subcommand)]
pub enum AgileCommand {
    /// Scrum and Kanban boards.
    #[command(subcommand)]
    Board(BoardCmd),
    /// Sprints.
    #[command(subcommand)]
    Sprint(SprintCmd),
    /// Epics.
    #[command(subcommand)]
    Epic(EpicCmd),
    /// GET any Jira Software REST path.
    Raw(RawArgs),
}

#[derive(Debug, Clone, Subcommand)]
pub enum BoardCmd {
    /// List boards.
    List {
        /// `scrum` or `kanban`.
        #[arg(long = "type")]
        board_type: Option<String>,
        /// Only boards whose name contains this text.
        #[arg(long)]
        name: Option<String>,
        /// Only boards for this project key or id.
        #[arg(long)]
        project: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a board by id.
    Get { id: String },
    /// Board configuration (columns, estimation, ranking).
    Configuration { id: String },
    /// Sprints on a board.
    Sprints {
        id: String,
        /// Comma-separated states: `future`, `active`, `closed`.
        #[arg(long)]
        state: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Issues on a board.
    Issues {
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Issues in a board's backlog.
    Backlog {
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Epics on a board.
    Epics {
        id: String,
        /// Only epics that are done (`true`) or not done (`false`).
        #[arg(long)]
        done: Option<bool>,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum SprintCmd {
    /// Get a sprint by id.
    Get { id: String },
    /// Issues in a sprint.
    Issues {
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum EpicCmd {
    /// Get an epic by id or key.
    Get { id: String },
    /// Issues in an epic.
    Issues {
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
}

/// Filters shared by the agile issue listings.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct IssueFilter {
    /// Extra JQL to filter issues.
    #[arg(long)]
    pub jql: Option<String>,
    /// Comma-separated fields to return.
    #[arg(long)]
    pub fields: Option<String>,
}

fn p(rest: &str) -> String {
    format!("{API}{rest}")
}

fn issues(path: String, filter: &IssueFilter, page: &PageArgs) -> ApiRequest {
    ApiRequest::get(path)
        .opt("jql", filter.jql.as_ref())
        .opt("fields", filter.fields.as_ref())
        .paged(PageStyle::Jira, page)
}

impl AgileCommand {
    pub fn to_request(&self) -> Result<ApiRequest> {
        let board = |id: &str, rest: &str| p(&format!("/board/{}{rest}", seg(id)));
        Ok(match self {
            AgileCommand::Board(cmd) => match cmd {
                BoardCmd::List {
                    board_type,
                    name,
                    project,
                    page,
                } => ApiRequest::get(p("/board"))
                    .opt("type", board_type.as_ref())
                    .opt("name", name.as_ref())
                    .opt("projectKeyOrId", project.as_ref())
                    .paged(PageStyle::Jira, page),
                BoardCmd::Get { id } => ApiRequest::get(board(id, "")),
                BoardCmd::Configuration { id } => ApiRequest::get(board(id, "/configuration")),
                BoardCmd::Sprints { id, state, page } => ApiRequest::get(board(id, "/sprint"))
                    .opt("state", state.as_ref())
                    .paged(PageStyle::Jira, page),
                BoardCmd::Issues { id, filter, page } => issues(board(id, "/issue"), filter, page),
                BoardCmd::Backlog { id, filter, page } => {
                    issues(board(id, "/backlog"), filter, page)
                }
                BoardCmd::Epics { id, done, page } => ApiRequest::get(board(id, "/epic"))
                    .opt("done", *done)
                    .paged(PageStyle::Jira, page),
            },
            AgileCommand::Sprint(cmd) => match cmd {
                SprintCmd::Get { id } => ApiRequest::get(p(&format!("/sprint/{}", seg(id)))),
                SprintCmd::Issues { id, filter, page } => {
                    issues(p(&format!("/sprint/{}/issue", seg(id))), filter, page)
                }
            },
            AgileCommand::Epic(cmd) => match cmd {
                EpicCmd::Get { id } => ApiRequest::get(p(&format!("/epic/{}", seg(id)))),
                EpicCmd::Issues { id, filter, page } => {
                    issues(p(&format!("/epic/{}/issue", seg(id))), filter, page)
                }
            },
            AgileCommand::Raw(raw) => return raw.to_request(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::request::ApiRequest;
    use clap::Parser;

    fn req(args: &[&str]) -> ApiRequest {
        let mut argv = vec!["atlassian", "agile"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap().to_request().unwrap()
    }

    #[test]
    fn board_list_filters() {
        let r = req(&[
            "board",
            "list",
            "--type",
            "scrum",
            "--project",
            "ABC",
            "--limit",
            "10",
        ]);
        assert_eq!(r.path, "/rest/agile/1.0/board");
        assert_eq!(
            r.first_query(),
            vec![
                ("type".into(), "scrum".into()),
                ("projectKeyOrId".into(), "ABC".into()),
                ("maxResults".into(), "10".into()),
            ]
        );
    }

    #[test]
    fn board_sub_resources() {
        assert_eq!(
            req(&["board", "sprints", "7"]).path,
            "/rest/agile/1.0/board/7/sprint"
        );
        assert_eq!(
            req(&["board", "backlog", "7"]).path,
            "/rest/agile/1.0/board/7/backlog"
        );
        assert_eq!(
            req(&["board", "epics", "7"]).path,
            "/rest/agile/1.0/board/7/epic"
        );
        assert_eq!(
            req(&["board", "configuration", "7"]).path,
            "/rest/agile/1.0/board/7/configuration"
        );
    }

    #[test]
    fn sprint_and_epic_issues() {
        let r = req(&["sprint", "issues", "12", "--jql", "status = Done"]);
        assert_eq!(r.path, "/rest/agile/1.0/sprint/12/issue");
        assert_eq!(r.query, vec![("jql".into(), "status = Done".into())]);
        assert_eq!(
            req(&["epic", "issues", "ABC-1"]).path,
            "/rest/agile/1.0/epic/ABC-1/issue"
        );
    }

    #[test]
    fn jira_software_alias() {
        let r = Cli::try_parse_from(["atlassian", "jira-software", "board", "get", "1"])
            .unwrap()
            .to_request()
            .unwrap();
        assert_eq!(r.path, "/rest/agile/1.0/board/1");
    }
}
