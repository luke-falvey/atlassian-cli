//! Bitbucket Data Center REST API (`/rest/api/latest`).

use anyhow::Result;
use clap::{Args, Subcommand, ValueEnum};

use super::RawArgs;
use crate::request::{ApiRequest, Guard, PageArgs, PageStyle, path_segs, seg};

const API: &str = "/rest/api/latest";

#[derive(Debug, Clone, Subcommand)]
pub enum BitbucketCommand {
    /// Version and build information.
    ApplicationProperties,
    /// Projects.
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Repositories.
    #[command(subcommand)]
    Repo(RepoCmd),
    /// Branches of a repository.
    #[command(subcommand)]
    Branch(BranchCmd),
    /// Tags of a repository.
    #[command(subcommand)]
    Tag(TagCmd),
    /// Commits of a repository.
    #[command(subcommand)]
    Commit(CommitCmd),
    /// Pull requests of a repository.
    #[command(subcommand)]
    PullRequest(PullRequestCmd),
    /// Files in a repository.
    #[command(subcommand)]
    File(FileCmd),
    /// Users.
    #[command(subcommand)]
    User(UserCmd),
    /// Groups (requires admin permission).
    #[command(subcommand)]
    Group(GroupCmd),
    /// GET any Bitbucket REST path.
    Raw(RawArgs),
}

/// A repository, identified by project key and repository slug.
#[derive(Debug, Clone, Args)]
pub struct RepoRef {
    /// Project key, e.g. `PROJ` (or `~username` for a personal project).
    pub project: String,
    /// Repository slug.
    pub repo: String,
}

impl RepoRef {
    fn path(&self, rest: &str) -> String {
        format!(
            "{API}/projects/{}/repos/{}{rest}",
            seg(&self.project),
            seg(&self.repo)
        )
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum ProjectCmd {
    /// List projects.
    List {
        /// Only projects whose name contains this text.
        #[arg(long)]
        name: Option<String>,
        /// Only projects where the user has this permission, e.g. `PROJECT_READ`.
        #[arg(long)]
        permission: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a project by key.
    Get {
        /// Project key, e.g. `PROJ` (or `~username` for a personal project).
        key: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum RepoCmd {
    /// List repositories, in one project or across all projects.
    List {
        /// Only repositories in this project.
        project: Option<String>,
        /// Only repositories whose name contains this text.
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a repository.
    Get {
        #[command(flatten)]
        repo: RepoRef,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum BranchCmd {
    /// List branches.
    List {
        #[command(flatten)]
        repo: RepoRef,
        /// Only branches whose name contains this text.
        #[arg(long)]
        filter: Option<String>,
        /// `ALPHABETICAL` or `MODIFICATION`.
        #[arg(long)]
        order_by: Option<String>,
        /// Include ahead/behind and latest-commit metadata.
        #[arg(long)]
        details: bool,
        #[command(flatten)]
        page: PageArgs,
    },
    /// The default branch.
    Default {
        #[command(flatten)]
        repo: RepoRef,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum TagCmd {
    /// List tags.
    List {
        #[command(flatten)]
        repo: RepoRef,
        /// Only tags whose name contains this text.
        #[arg(long)]
        filter: Option<String>,
        /// `ALPHABETICAL` or `MODIFICATION`.
        #[arg(long)]
        order_by: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum CommitCmd {
    /// List commits, newest first.
    List {
        #[command(flatten)]
        repo: RepoRef,
        /// Commit or ref to start from (default: the default branch).
        #[arg(long)]
        until: Option<String>,
        /// Exclude commits reachable from this commit or ref.
        #[arg(long)]
        since: Option<String>,
        /// Only commits that touch this file path.
        #[arg(long)]
        path: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a commit.
    Get {
        #[command(flatten)]
        repo: RepoRef,
        /// Commit hash (full or abbreviated) or ref.
        id: String,
    },
    /// Files changed by a commit.
    Changes {
        #[command(flatten)]
        repo: RepoRef,
        /// Commit hash (full or abbreviated) or ref.
        id: String,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "UPPER")]
pub enum PrState {
    Open,
    Merged,
    Declined,
    All,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PullRequestCmd {
    /// List pull requests.
    List {
        #[command(flatten)]
        repo: RepoRef,
        /// Pull request state (default: OPEN).
        #[arg(long, value_enum)]
        state: Option<PrState>,
        /// `INCOMING` (default) or `OUTGOING`, relative to `--at`.
        #[arg(long)]
        direction: Option<String>,
        /// Only pull requests to (or from) this ref, e.g. `refs/heads/main`.
        #[arg(long)]
        at: Option<String>,
        /// `NEWEST` (default) or `OLDEST`.
        #[arg(long)]
        order: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a pull request.
    Get {
        #[command(flatten)]
        repo: RepoRef,
        /// Pull request id (the number in its URL).
        id: u64,
    },
    /// Comments, approvals and updates on a pull request.
    Activities {
        #[command(flatten)]
        repo: RepoRef,
        /// Pull request id (the number in its URL).
        id: u64,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Files changed by a pull request.
    Changes {
        #[command(flatten)]
        repo: RepoRef,
        /// Pull request id (the number in its URL).
        id: u64,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Commits in a pull request.
    Commits {
        #[command(flatten)]
        repo: RepoRef,
        /// Pull request id (the number in its URL).
        id: u64,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum FileCmd {
    /// List file paths, recursively, below a directory.
    List {
        #[command(flatten)]
        repo: RepoRef,
        /// Directory to list (default: repository root).
        path: Option<String>,
        /// Commit or ref to list at.
        #[arg(long)]
        at: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Browse a directory (children) or file (lines) as JSON.
    Browse {
        #[command(flatten)]
        repo: RepoRef,
        /// File or directory path (default: the repository root).
        path: Option<String>,
        /// Commit or ref to read at, e.g. `refs/heads/main` (default: the default branch).
        #[arg(long)]
        at: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Print the raw content of a file.
    Content {
        #[command(flatten)]
        repo: RepoRef,
        /// File path, e.g. `src/main.rs`.
        path: String,
        /// Commit or ref to read at, e.g. `refs/heads/main` (default: the default branch).
        #[arg(long)]
        at: Option<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum UserCmd {
    /// List users.
    List {
        /// Only users whose username, name or email contains this text.
        #[arg(long)]
        filter: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a user by slug.
    Get {
        /// User slug (usually the username), e.g. `alice`.
        slug: String,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum GroupCmd {
    /// List groups.
    List {
        /// Only groups whose name contains this text.
        #[arg(long)]
        filter: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
}

fn p(rest: &str) -> String {
    format!("{API}{rest}")
}

/// `/files`, `/files/a/b` etc. for an optional file path.
fn with_path(base: &str, path: Option<&String>) -> String {
    match path.map(|p| path_segs(p)).filter(|p| !p.is_empty()) {
        Some(p) => format!("{base}/{p}"),
        None => base.to_string(),
    }
}

impl BitbucketCommand {
    pub fn to_request(&self) -> Result<ApiRequest> {
        use PageStyle::Bitbucket as B;
        let req = match self {
            BitbucketCommand::ApplicationProperties => {
                ApiRequest::get(p("/application-properties"))
            }
            BitbucketCommand::Project(cmd) => match cmd {
                ProjectCmd::List {
                    name,
                    permission,
                    page,
                } => ApiRequest::get(p("/projects"))
                    .opt("name", name.as_ref())
                    .opt("permission", permission.as_ref())
                    .paged(B, page),
                ProjectCmd::Get { key } => ApiRequest::get(p(&format!("/projects/{}", seg(key)))),
            },
            BitbucketCommand::Repo(cmd) => match cmd {
                RepoCmd::List {
                    project: Some(project),
                    name,
                    page,
                } => ApiRequest::get(p(&format!("/projects/{}/repos", seg(project))))
                    .opt("name", name.as_ref())
                    .paged(B, page),
                RepoCmd::List {
                    project: None,
                    name,
                    page,
                } => ApiRequest::get(p("/repos"))
                    .opt("name", name.as_ref())
                    .paged(B, page),
                RepoCmd::Get { repo } => ApiRequest::get(repo.path("")),
            },
            BitbucketCommand::Branch(cmd) => match cmd {
                BranchCmd::List {
                    repo,
                    filter,
                    order_by,
                    details,
                    page,
                } => ApiRequest::get(repo.path("/branches"))
                    .opt("filterText", filter.as_ref())
                    .opt("orderBy", order_by.as_ref())
                    .flag("details", *details)
                    .paged(B, page),
                BranchCmd::Default { repo } => ApiRequest::get(repo.path("/branches/default")),
            },
            BitbucketCommand::Tag(TagCmd::List {
                repo,
                filter,
                order_by,
                page,
            }) => ApiRequest::get(repo.path("/tags"))
                .opt("filterText", filter.as_ref())
                .opt("orderBy", order_by.as_ref())
                .paged(B, page),
            BitbucketCommand::Commit(cmd) => match cmd {
                CommitCmd::List {
                    repo,
                    until,
                    since,
                    path,
                    page,
                } => ApiRequest::get(repo.path("/commits"))
                    .opt("until", until.as_ref())
                    .opt("since", since.as_ref())
                    .opt("path", path.as_ref())
                    .paged(B, page),
                CommitCmd::Get { repo, id } => {
                    ApiRequest::get(repo.path(&format!("/commits/{}", seg(id))))
                }
                CommitCmd::Changes { repo, id, page } => {
                    ApiRequest::get(repo.path(&format!("/commits/{}/changes", seg(id))))
                        .paged(B, page)
                }
            },
            BitbucketCommand::PullRequest(cmd) => match cmd {
                PullRequestCmd::List {
                    repo,
                    state,
                    direction,
                    at,
                    order,
                    page,
                } => ApiRequest::get(repo.path("/pull-requests"))
                    .opt(
                        "state",
                        state
                            .and_then(|s| s.to_possible_value())
                            .map(|v| v.get_name().to_string()),
                    )
                    .opt("direction", direction.as_ref())
                    .opt("at", at.as_ref())
                    .opt("order", order.as_ref())
                    .paged(B, page),
                PullRequestCmd::Get { repo, id } => {
                    ApiRequest::get(repo.path(&format!("/pull-requests/{id}")))
                }
                PullRequestCmd::Activities { repo, id, page } => {
                    ApiRequest::get(repo.path(&format!("/pull-requests/{id}/activities")))
                        .paged(B, page)
                }
                PullRequestCmd::Changes { repo, id, page } => {
                    ApiRequest::get(repo.path(&format!("/pull-requests/{id}/changes")))
                        .paged(B, page)
                }
                PullRequestCmd::Commits { repo, id, page } => {
                    ApiRequest::get(repo.path(&format!("/pull-requests/{id}/commits")))
                        .paged(B, page)
                }
            },
            BitbucketCommand::File(cmd) => match cmd {
                FileCmd::List {
                    repo,
                    path,
                    at,
                    page,
                } => ApiRequest::get(with_path(&repo.path("/files"), path.as_ref()))
                    .opt("at", at.as_ref())
                    .paged(B, page),
                FileCmd::Browse {
                    repo,
                    path,
                    at,
                    page,
                } => ApiRequest::get(with_path(&repo.path("/browse"), path.as_ref()))
                    .opt("at", at.as_ref())
                    .paged(B, page),
                FileCmd::Content { repo, path, at } => {
                    ApiRequest::get(with_path(&repo.path("/raw"), Some(path)))
                        .opt("at", at.as_ref())
                }
            },
            BitbucketCommand::User(cmd) => match cmd {
                UserCmd::List { filter, page } => ApiRequest::get(p("/users"))
                    .opt("filter", filter.as_ref())
                    .paged(B, page),
                UserCmd::Get { slug } => ApiRequest::get(p(&format!("/users/{}", seg(slug)))),
            },
            BitbucketCommand::Group(GroupCmd::List { filter, page }) => {
                ApiRequest::get(p("/admin/groups"))
                    .opt("filter", filter.as_ref())
                    .paged(B, page)
            }
            BitbucketCommand::Raw(raw) => return raw.to_request(),
        };
        Ok(req.guard(self.guard()))
    }

    /// What the command touches, for the scope check.
    fn guard(&self) -> Guard {
        use BitbucketCommand as C;
        let repo = |r: &RepoRef| Guard::BitbucketRepo {
            project: r.project.clone(),
            repo: r.repo.clone(),
        };
        match self {
            C::ApplicationProperties => Guard::Open,
            C::Project(ProjectCmd::List { .. }) => Guard::BitbucketProjectList,
            C::Project(ProjectCmd::Get { key }) => Guard::BitbucketProject(key.clone()),
            C::Repo(RepoCmd::List { .. }) => Guard::BitbucketRepoList,
            C::Repo(RepoCmd::Get { repo: r }) => repo(r),
            C::Branch(BranchCmd::List { repo: r, .. } | BranchCmd::Default { repo: r }) => repo(r),
            C::Tag(TagCmd::List { repo: r, .. }) => repo(r),
            C::Commit(
                CommitCmd::List { repo: r, .. }
                | CommitCmd::Get { repo: r, .. }
                | CommitCmd::Changes { repo: r, .. },
            ) => repo(r),
            C::PullRequest(
                PullRequestCmd::List { repo: r, .. }
                | PullRequestCmd::Get { repo: r, .. }
                | PullRequestCmd::Activities { repo: r, .. }
                | PullRequestCmd::Changes { repo: r, .. }
                | PullRequestCmd::Commits { repo: r, .. },
            ) => repo(r),
            C::File(
                FileCmd::List { repo: r, .. }
                | FileCmd::Browse { repo: r, .. }
                | FileCmd::Content { repo: r, .. },
            ) => repo(r),
            // Listing users and groups spans the whole instance.
            C::User(_) | C::Group(_) => Guard::Instance,
            C::Raw(_) => Guard::Raw,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::request::ApiRequest;
    use clap::Parser;

    fn req(args: &[&str]) -> ApiRequest {
        let mut argv = vec!["atlassian", "bitbucket"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap().to_request().unwrap()
    }

    const REPO: &str = "/rest/api/latest/projects/PROJ/repos/my-repo";

    #[test]
    fn repo_list_scoped_and_global() {
        assert_eq!(
            req(&["repo", "list", "PROJ"]).path,
            "/rest/api/latest/projects/PROJ/repos"
        );
        let r = req(&["repo", "list", "--name", "api"]);
        assert_eq!(r.path, "/rest/api/latest/repos");
        assert_eq!(r.query, vec![("name".into(), "api".into())]);
    }

    #[test]
    fn repo_scoped_resources() {
        assert_eq!(req(&["repo", "get", "PROJ", "my-repo"]).path, REPO);
        assert_eq!(
            req(&["branch", "list", "PROJ", "my-repo"]).path,
            format!("{REPO}/branches")
        );
        assert_eq!(
            req(&["branch", "default", "PROJ", "my-repo"]).path,
            format!("{REPO}/branches/default")
        );
        assert_eq!(
            req(&["tag", "list", "PROJ", "my-repo"]).path,
            format!("{REPO}/tags")
        );
        assert_eq!(
            req(&["commit", "get", "PROJ", "my-repo", "abc123"]).path,
            format!("{REPO}/commits/abc123")
        );
        assert_eq!(
            req(&["commit", "changes", "PROJ", "my-repo", "abc123"]).path,
            format!("{REPO}/commits/abc123/changes")
        );
    }

    #[test]
    fn pull_request_list_state() {
        let r = req(&[
            "pull-request",
            "list",
            "PROJ",
            "my-repo",
            "--state",
            "MERGED",
            "--limit",
            "50",
        ]);
        assert_eq!(r.path, format!("{REPO}/pull-requests"));
        assert_eq!(
            r.first_query(),
            vec![
                ("state".into(), "MERGED".into()),
                ("limit".into(), "50".into())
            ]
        );
    }

    #[test]
    fn pull_request_sub_resources() {
        assert_eq!(
            req(&["pull-request", "activities", "PROJ", "my-repo", "7"]).path,
            format!("{REPO}/pull-requests/7/activities")
        );
        assert_eq!(
            req(&["pull-request", "commits", "PROJ", "my-repo", "7"]).path,
            format!("{REPO}/pull-requests/7/commits")
        );
    }

    #[test]
    fn file_paths_are_encoded_per_segment() {
        assert_eq!(
            req(&["file", "list", "PROJ", "my-repo"]).path,
            format!("{REPO}/files")
        );
        assert_eq!(
            req(&["file", "browse", "PROJ", "my-repo", "src/my file.rs"]).path,
            format!("{REPO}/browse/src/my%20file.rs")
        );
        let r = req(&[
            "file",
            "content",
            "PROJ",
            "my-repo",
            "README.md",
            "--at",
            "main",
        ]);
        assert_eq!(r.path, format!("{REPO}/raw/README.md"));
        assert_eq!(r.query, vec![("at".into(), "main".into())]);
    }

    #[test]
    fn personal_project_key() {
        assert_eq!(
            req(&["repo", "get", "~alice", "dotfiles"]).path,
            "/rest/api/latest/projects/~alice/repos/dotfiles"
        );
    }

    #[test]
    fn users_and_groups() {
        assert_eq!(
            req(&["user", "get", "alice"]).path,
            "/rest/api/latest/users/alice"
        );
        assert_eq!(
            req(&["group", "list"]).path,
            "/rest/api/latest/admin/groups"
        );
    }
}
