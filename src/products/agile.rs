//! Jira Software (Agile) REST API (`/rest/agile/1.0`). Served by the Jira
//! instance, so it uses the Jira base URL.

use anyhow::Result;
use clap::Subcommand;

use super::RawArgs;
use crate::request::{ApiRequest, Guard, PageArgs, PageStyle, seg};

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
    Get {
        /// Board id (from `agile board list`).
        id: String,
    },
    /// Board configuration (columns, estimation, ranking).
    Configuration {
        /// Board id (from `agile board list`).
        id: String,
    },
    /// Sprints on a board.
    Sprints {
        /// Board id (from `agile board list`).
        id: String,
        /// Comma-separated states: `future`, `active`, `closed`.
        #[arg(long)]
        state: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Issues on a board.
    Issues {
        /// Board id (from `agile board list`).
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Issues in a board's backlog.
    Backlog {
        /// Board id (from `agile board list`).
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Epics on a board.
    Epics {
        /// Board id (from `agile board list`).
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
    Get {
        /// Sprint id (from `agile board sprints`).
        id: String,
    },
    /// Issues in a sprint.
    Issues {
        /// Sprint id (from `agile board sprints`).
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
    Get {
        /// Epic id or issue key, e.g. `ABC-42`.
        id: String,
    },
    /// Issues in an epic.
    Issues {
        /// Epic id or issue key, e.g. `ABC-42`.
        id: String,
        #[command(flatten)]
        filter: IssueFilter,
        #[command(flatten)]
        page: PageArgs,
    },
}

/// Options shared by the agile issue listings. There is no JQL filter: the
/// board, sprint or epic decides which issues are listed.
#[derive(Debug, Clone, Default, clap::Args)]
pub struct IssueFilter {
    /// Comma-separated fields to return.
    #[arg(long)]
    pub fields: Option<String>,
}

fn p(rest: &str) -> String {
    format!("{API}{rest}")
}

/// An issue listing under `owner` (a board, sprint or epic).
fn issues(path: String, owner: Guard, filter: &IssueFilter, page: &PageArgs) -> ApiRequest {
    ApiRequest::get(path)
        .opt("fields", filter.fields.as_ref())
        .paged(PageStyle::Jira, page)
        .guard(Guard::AgileIssues(Box::new(owner)))
}

impl AgileCommand {
    pub fn to_request(&self) -> Result<ApiRequest> {
        let board = |id: &str, rest: &str| p(&format!("/board/{}{rest}", seg(id)));
        let on_board = |id: &str| Guard::AgileBoard(id.to_string());
        let sprint = |id: &str| Guard::AgileSprint(id.to_string());
        let epic = |id: &str| Guard::AgileEpic(id.to_string());
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
                    .paged(PageStyle::Jira, page)
                    .guard(Guard::AgileBoardList(project.clone())),
                BoardCmd::Get { id } => ApiRequest::get(board(id, "")).guard(on_board(id)),
                BoardCmd::Configuration { id } => {
                    ApiRequest::get(board(id, "/configuration")).guard(on_board(id))
                }
                BoardCmd::Sprints { id, state, page } => ApiRequest::get(board(id, "/sprint"))
                    .opt("state", state.as_ref())
                    .paged(PageStyle::Jira, page)
                    .guard(on_board(id)),
                BoardCmd::Issues { id, filter, page } => {
                    issues(board(id, "/issue"), on_board(id), filter, page)
                }
                BoardCmd::Backlog { id, filter, page } => {
                    issues(board(id, "/backlog"), on_board(id), filter, page)
                }
                BoardCmd::Epics { id, done, page } => {
                    let mut req = ApiRequest::get(board(id, "/epic"))
                        .opt("done", *done)
                        .paged(PageStyle::Jira, page);
                    // Epics are issues: drop any from other projects.
                    req.guard = Guard::AgileIssues(Box::new(on_board(id)));
                    req
                }
            },
            AgileCommand::Sprint(cmd) => match cmd {
                SprintCmd::Get { id } => {
                    ApiRequest::get(p(&format!("/sprint/{}", seg(id)))).guard(sprint(id))
                }
                SprintCmd::Issues { id, filter, page } => issues(
                    p(&format!("/sprint/{}/issue", seg(id))),
                    sprint(id),
                    filter,
                    page,
                ),
            },
            AgileCommand::Epic(cmd) => match cmd {
                EpicCmd::Get { id } => {
                    ApiRequest::get(p(&format!("/epic/{}", seg(id)))).guard(epic(id))
                }
                EpicCmd::Issues { id, filter, page } => issues(
                    p(&format!("/epic/{}/issue", seg(id))),
                    epic(id),
                    filter,
                    page,
                ),
            },
            AgileCommand::Raw(raw) => return raw.to_request(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::request::{ApiRequest, Guard};
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
    fn jql_filter_is_gone() {
        assert!(
            Cli::try_parse_from(["atlassian", "agile", "sprint", "issues", "1", "--jql", "x"])
                .is_err()
        );
    }

    #[test]
    fn sprint_and_epic_issues() {
        let r = req(&["sprint", "issues", "12", "--fields", "summary"]);
        assert_eq!(r.path, "/rest/agile/1.0/sprint/12/issue");
        assert_eq!(r.query, vec![("fields".into(), "summary".into())]);
        assert_eq!(
            r.guard,
            Guard::AgileIssues(Box::new(Guard::AgileSprint("12".into())))
        );
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
