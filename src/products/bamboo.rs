//! Bamboo Data Center REST API (`/rest/api/latest`).

use anyhow::Result;
use clap::{Args, Subcommand};

use super::RawArgs;
use crate::request::{ApiRequest, PageArgs, PageStyle, seg};

const API: &str = "/rest/api/latest";

#[derive(Debug, Clone, Subcommand)]
pub enum BambooCommand {
    /// Version and build information.
    Info,
    /// Server state (running, paused, ...).
    Server,
    /// The user whose credentials are in use.
    CurrentUser,
    /// Build projects.
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Build plans.
    #[command(subcommand)]
    Plan(PlanCmd),
    /// Build results.
    #[command(subcommand)]
    Result(ResultCmd),
    /// Deployment projects and environments.
    #[command(subcommand)]
    Deploy(DeployCmd),
    /// Builds waiting in the queue.
    Queue,
    /// Build agents (requires admin permission).
    #[command(subcommand)]
    Agent(AgentCmd),
    /// GET any Bamboo REST path.
    Raw(RawArgs),
}

/// Paging flags for Bamboo, which reports totals in a nested object and so
/// doesn't support `--all`.
#[derive(Debug, Clone, Default, Args)]
pub struct BambooPage {
    /// Index of the first result to return.
    #[arg(long)]
    pub start: Option<u64>,
    /// Maximum number of results to return.
    #[arg(long)]
    pub limit: Option<u64>,
}

impl BambooPage {
    fn args(&self) -> PageArgs {
        PageArgs {
            start: self.start,
            limit: self.limit,
            all: false,
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectCmd {
    /// List projects.
    List {
        /// Comma-separated entities to expand, e.g. `projects.project.plans`.
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: BambooPage,
    },
    /// Get a project by key.
    Get {
        key: String,
        #[arg(long)]
        expand: Option<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum PlanCmd {
    /// List plans.
    List {
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: BambooPage,
    },
    /// Get a plan by key, e.g. `PROJ-PLAN`.
    Get {
        key: String,
        #[arg(long)]
        expand: Option<String>,
    },
    /// Branches of a plan.
    Branches {
        key: String,
        #[command(flatten)]
        page: BambooPage,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ResultCmd {
    /// List the latest results, for all plans or one plan.
    List {
        /// Plan key, e.g. `PROJ-PLAN`.
        plan: Option<String>,
        /// `Successful`, `Failed` or `Unknown`.
        #[arg(long)]
        build_state: Option<String>,
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: BambooPage,
    },
    /// Get a build result by key, e.g. `PROJ-PLAN-42`.
    Get {
        key: String,
        #[arg(long)]
        expand: Option<String>,
    },
    /// The latest result of a plan.
    Latest {
        plan: String,
        #[arg(long)]
        expand: Option<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum DeployCmd {
    /// Deployment projects.
    #[command(subcommand)]
    Project(DeployProjectCmd),
    /// Deployment results of an environment.
    EnvironmentResults {
        id: String,
        #[command(flatten)]
        page: BambooPage,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum DeployProjectCmd {
    /// List all deployment projects.
    List,
    /// Get a deployment project by id.
    Get { id: String },
}

#[derive(Debug, Clone, Subcommand)]
pub enum AgentCmd {
    /// List build agents.
    List,
}

fn p(rest: &str) -> String {
    format!("{API}{rest}")
}

impl BambooCommand {
    pub fn to_request(&self) -> Result<ApiRequest> {
        use PageStyle::Bamboo as B;
        Ok(match self {
            BambooCommand::Info => ApiRequest::get(p("/info")),
            BambooCommand::Server => ApiRequest::get(p("/server")),
            BambooCommand::CurrentUser => ApiRequest::get(p("/currentUser")),
            BambooCommand::Project(cmd) => match cmd {
                ProjectCmd::List { expand, page } => ApiRequest::get(p("/project"))
                    .opt("expand", expand.as_ref())
                    .paged(B, &page.args()),
                ProjectCmd::Get { key, expand } => {
                    ApiRequest::get(p(&format!("/project/{}", seg(key))))
                        .opt("expand", expand.as_ref())
                }
            },
            BambooCommand::Plan(cmd) => match cmd {
                PlanCmd::List { expand, page } => ApiRequest::get(p("/plan"))
                    .opt("expand", expand.as_ref())
                    .paged(B, &page.args()),
                PlanCmd::Get { key, expand } => ApiRequest::get(p(&format!("/plan/{}", seg(key))))
                    .opt("expand", expand.as_ref()),
                PlanCmd::Branches { key, page } => {
                    ApiRequest::get(p(&format!("/plan/{}/branch", seg(key)))).paged(B, &page.args())
                }
            },
            BambooCommand::Result(cmd) => match cmd {
                ResultCmd::List {
                    plan,
                    build_state,
                    expand,
                    page,
                } => {
                    let path = match plan {
                        Some(plan) => p(&format!("/result/{}", seg(plan))),
                        None => p("/result"),
                    };
                    ApiRequest::get(path)
                        .opt("buildstate", build_state.as_ref())
                        .opt("expand", expand.as_ref())
                        .paged(B, &page.args())
                }
                ResultCmd::Get { key, expand } => {
                    ApiRequest::get(p(&format!("/result/{}", seg(key))))
                        .opt("expand", expand.as_ref())
                }
                ResultCmd::Latest { plan, expand } => {
                    ApiRequest::get(p(&format!("/result/{}/latest", seg(plan))))
                        .opt("expand", expand.as_ref())
                }
            },
            BambooCommand::Deploy(cmd) => match cmd {
                DeployCmd::Project(DeployProjectCmd::List) => {
                    ApiRequest::get(p("/deploy/project/all"))
                }
                DeployCmd::Project(DeployProjectCmd::Get { id }) => {
                    ApiRequest::get(p(&format!("/deploy/project/{}", seg(id))))
                }
                DeployCmd::EnvironmentResults { id, page } => {
                    ApiRequest::get(p(&format!("/deploy/environment/{}/results", seg(id))))
                        .paged(B, &page.args())
                }
            },
            BambooCommand::Queue => ApiRequest::get(p("/queue")),
            BambooCommand::Agent(AgentCmd::List) => ApiRequest::get(p("/agent")),
            BambooCommand::Raw(raw) => return raw.to_request(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::request::ApiRequest;
    use clap::Parser;

    fn req(args: &[&str]) -> ApiRequest {
        let mut argv = vec!["atlassian", "bamboo"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap().to_request().unwrap()
    }

    #[test]
    fn simple_paths() {
        assert_eq!(req(&["info"]).path, "/rest/api/latest/info");
        assert_eq!(req(&["current-user"]).path, "/rest/api/latest/currentUser");
        assert_eq!(req(&["queue"]).path, "/rest/api/latest/queue");
        assert_eq!(req(&["agent", "list"]).path, "/rest/api/latest/agent");
    }

    #[test]
    fn plan_and_result_paths() {
        assert_eq!(
            req(&["plan", "get", "PROJ-PLAN"]).path,
            "/rest/api/latest/plan/PROJ-PLAN"
        );
        assert_eq!(
            req(&["plan", "branches", "PROJ-PLAN"]).path,
            "/rest/api/latest/plan/PROJ-PLAN/branch"
        );
        assert_eq!(req(&["result", "list"]).path, "/rest/api/latest/result");
        assert_eq!(
            req(&["result", "latest", "PROJ-PLAN"]).path,
            "/rest/api/latest/result/PROJ-PLAN/latest"
        );
    }

    #[test]
    fn bamboo_paging_params() {
        let r = req(&[
            "result",
            "list",
            "PROJ-PLAN",
            "--start",
            "25",
            "--limit",
            "25",
        ]);
        assert_eq!(r.path, "/rest/api/latest/result/PROJ-PLAN");
        assert_eq!(
            r.first_query(),
            vec![
                ("start-index".into(), "25".into()),
                ("max-result".into(), "25".into())
            ]
        );
    }

    #[test]
    fn no_all_flag() {
        assert!(Cli::try_parse_from(["atlassian", "bamboo", "plan", "list", "--all"]).is_err());
    }

    #[test]
    fn deploy_paths() {
        assert_eq!(
            req(&["deploy", "project", "list"]).path,
            "/rest/api/latest/deploy/project/all"
        );
        assert_eq!(
            req(&["deploy", "environment-results", "9"]).path,
            "/rest/api/latest/deploy/environment/9/results"
        );
    }
}
