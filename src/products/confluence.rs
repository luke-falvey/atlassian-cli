//! Confluence Data Center REST API (`/rest/api`).

use anyhow::Result;
use clap::{Subcommand, ValueEnum};

use super::RawArgs;
use crate::request::{ApiRequest, PageArgs, PageStyle, seg};

const API: &str = "/rest/api";

#[derive(Debug, Clone, Subcommand)]
pub enum ConfluenceCommand {
    /// Spaces.
    #[command(subcommand)]
    Space(SpaceCmd),
    /// Pages, blog posts, comments and attachments.
    #[command(subcommand)]
    Content(ContentCmd),
    /// Search content with CQL.
    Search {
        /// CQL query, e.g. `type = page AND space = DEV`.
        #[arg(long)]
        cql: String,
        /// Comma-separated properties to expand.
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Users.
    #[command(subcommand)]
    User(UserCmd),
    /// Groups.
    #[command(subcommand)]
    Group(GroupCmd),
    /// GET any Confluence REST path.
    Raw(RawArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ContentType {
    Page,
    Blogpost,
    Comment,
    Attachment,
}

impl ContentType {
    fn as_str(self) -> &'static str {
        match self {
            ContentType::Page => "page",
            ContentType::Blogpost => "blogpost",
            ContentType::Comment => "comment",
            ContentType::Attachment => "attachment",
        }
    }
}

#[derive(Debug, Clone, Subcommand)]
pub enum SpaceCmd {
    /// List spaces.
    List {
        /// `global` or `personal`.
        #[arg(long = "type")]
        space_type: Option<String>,
        /// Only these space keys. Repeat for several.
        #[arg(long = "key")]
        keys: Vec<String>,
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get a space by key.
    Get {
        key: String,
        #[arg(long)]
        expand: Option<String>,
    },
    /// Content in a space.
    Content {
        key: String,
        /// Only content of this type.
        #[arg(long = "type", value_enum)]
        content_type: Option<ContentType>,
        /// `all` (default) or `root` for top-level pages only.
        #[arg(long)]
        depth: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum ContentCmd {
    /// List content, optionally filtered by space, type and title.
    List {
        #[arg(long)]
        space: Option<String>,
        #[arg(long = "type", value_enum)]
        content_type: Option<ContentType>,
        #[arg(long)]
        title: Option<String>,
        /// `current` (default), `trashed`, `draft` or `any`.
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Get content by id.
    Get {
        id: String,
        /// Comma-separated properties to expand, e.g. `body.storage,version`.
        #[arg(long)]
        expand: Option<String>,
        /// A historical version number.
        #[arg(long)]
        version: Option<u64>,
        #[arg(long)]
        status: Option<String>,
    },
    /// Direct children of a piece of content.
    Children {
        id: String,
        /// Type of children to list.
        #[arg(long = "type", value_enum, default_value = "page")]
        content_type: ContentType,
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// All descendants of a piece of content.
    Descendants {
        id: String,
        #[arg(long = "type", value_enum, default_value = "page")]
        content_type: ContentType,
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Attachments on a piece of content.
    Attachments {
        id: String,
        /// Only the attachment with this file name.
        #[arg(long)]
        filename: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Comments on a piece of content.
    Comments {
        id: String,
        /// `all` to include nested replies.
        #[arg(long)]
        depth: Option<String>,
        #[arg(long)]
        expand: Option<String>,
        #[command(flatten)]
        page: PageArgs,
    },
    /// Labels on a piece of content.
    Labels {
        id: String,
        #[command(flatten)]
        page: PageArgs,
    },
    /// History (creator, versions, contributors).
    History { id: String },
}

#[derive(Debug, Clone, Subcommand)]
pub enum UserCmd {
    /// The user whose credentials are in use.
    Current,
    /// Get a user by username or user key.
    Get {
        #[arg(long, required_unless_present = "key", conflicts_with = "key")]
        username: Option<String>,
        #[arg(long)]
        key: Option<String>,
    },
}

#[derive(Debug, Clone, Subcommand)]
pub enum GroupCmd {
    /// List groups.
    List {
        #[command(flatten)]
        page: PageArgs,
    },
    /// Members of a group.
    Members {
        name: String,
        #[command(flatten)]
        page: PageArgs,
    },
}

fn p(rest: &str) -> String {
    format!("{API}{rest}")
}

impl ConfluenceCommand {
    pub fn to_request(&self) -> Result<ApiRequest> {
        use PageStyle::Confluence as C;
        Ok(match self {
            ConfluenceCommand::Space(cmd) => match cmd {
                SpaceCmd::List {
                    space_type,
                    keys,
                    expand,
                    page,
                } => {
                    let mut r = ApiRequest::get(p("/space")).opt("type", space_type.as_ref());
                    for k in keys {
                        r = r.param("spaceKey", k);
                    }
                    r.opt("expand", expand.as_ref()).paged(C, page)
                }
                SpaceCmd::Get { key, expand } => {
                    ApiRequest::get(p(&format!("/space/{}", seg(key))))
                        .opt("expand", expand.as_ref())
                }
                SpaceCmd::Content {
                    key,
                    content_type,
                    depth,
                    page,
                } => {
                    let path = match content_type {
                        Some(t) => format!("/space/{}/content/{}", seg(key), t.as_str()),
                        None => format!("/space/{}/content", seg(key)),
                    };
                    ApiRequest::get(p(&path))
                        .opt("depth", depth.as_ref())
                        .paged(C, page)
                }
            },
            ConfluenceCommand::Content(cmd) => cmd.to_request(),
            ConfluenceCommand::Search { cql, expand, page } => {
                ApiRequest::get(p("/content/search"))
                    .param("cql", cql)
                    .opt("expand", expand.as_ref())
                    .paged(C, page)
            }
            ConfluenceCommand::User(cmd) => match cmd {
                UserCmd::Current => ApiRequest::get(p("/user/current")),
                UserCmd::Get { username, key } => ApiRequest::get(p("/user"))
                    .opt("username", username.as_ref())
                    .opt("key", key.as_ref()),
            },
            ConfluenceCommand::Group(cmd) => match cmd {
                GroupCmd::List { page } => ApiRequest::get(p("/group")).paged(C, page),
                GroupCmd::Members { name, page } => {
                    ApiRequest::get(p(&format!("/group/{}/member", seg(name)))).paged(C, page)
                }
            },
            ConfluenceCommand::Raw(raw) => return raw.to_request(),
        })
    }
}

impl ContentCmd {
    fn to_request(&self) -> ApiRequest {
        use PageStyle::Confluence as C;
        let content = |id: &str, rest: &str| p(&format!("/content/{}{rest}", seg(id)));
        match self {
            ContentCmd::List {
                space,
                content_type,
                title,
                status,
                expand,
                page,
            } => ApiRequest::get(p("/content"))
                .opt("spaceKey", space.as_ref())
                .opt("type", content_type.map(ContentType::as_str))
                .opt("title", title.as_ref())
                .opt("status", status.as_ref())
                .opt("expand", expand.as_ref())
                .paged(C, page),
            ContentCmd::Get {
                id,
                expand,
                version,
                status,
            } => ApiRequest::get(content(id, ""))
                .opt("expand", expand.as_ref())
                .opt("version", *version)
                .opt("status", status.as_ref()),
            ContentCmd::Children {
                id,
                content_type,
                expand,
                page,
            } => ApiRequest::get(content(id, &format!("/child/{}", content_type.as_str())))
                .opt("expand", expand.as_ref())
                .paged(C, page),
            ContentCmd::Descendants {
                id,
                content_type,
                expand,
                page,
            } => ApiRequest::get(content(
                id,
                &format!("/descendant/{}", content_type.as_str()),
            ))
            .opt("expand", expand.as_ref())
            .paged(C, page),
            ContentCmd::Attachments { id, filename, page } => {
                ApiRequest::get(content(id, "/child/attachment"))
                    .opt("filename", filename.as_ref())
                    .paged(C, page)
            }
            ContentCmd::Comments {
                id,
                depth,
                expand,
                page,
            } => ApiRequest::get(content(id, "/child/comment"))
                .opt("depth", depth.as_ref())
                .opt("expand", expand.as_ref())
                .paged(C, page),
            ContentCmd::Labels { id, page } => {
                ApiRequest::get(content(id, "/label")).paged(C, page)
            }
            ContentCmd::History { id } => ApiRequest::get(content(id, "/history")),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::cli::Cli;
    use crate::request::ApiRequest;
    use clap::Parser;

    fn req(args: &[&str]) -> ApiRequest {
        let mut argv = vec!["atlassian", "confluence"];
        argv.extend_from_slice(args);
        Cli::try_parse_from(argv).unwrap().to_request().unwrap()
    }

    #[test]
    fn space_list_with_keys() {
        let r = req(&[
            "space", "list", "--key", "DEV", "--key", "OPS", "--limit", "5",
        ]);
        assert_eq!(r.path, "/rest/api/space");
        assert_eq!(
            r.first_query(),
            vec![
                ("spaceKey".into(), "DEV".into()),
                ("spaceKey".into(), "OPS".into()),
                ("limit".into(), "5".into()),
            ]
        );
    }

    #[test]
    fn space_content_by_type() {
        assert_eq!(
            req(&["space", "content", "DEV"]).path,
            "/rest/api/space/DEV/content"
        );
        assert_eq!(
            req(&["space", "content", "DEV", "--type", "blogpost"]).path,
            "/rest/api/space/DEV/content/blogpost"
        );
    }

    #[test]
    fn content_paths() {
        assert_eq!(
            req(&["content", "get", "123"]).path,
            "/rest/api/content/123"
        );
        assert_eq!(
            req(&["content", "children", "123"]).path,
            "/rest/api/content/123/child/page"
        );
        assert_eq!(
            req(&["content", "children", "123", "--type", "comment"]).path,
            "/rest/api/content/123/child/comment"
        );
        assert_eq!(
            req(&["content", "descendants", "123"]).path,
            "/rest/api/content/123/descendant/page"
        );
        assert_eq!(
            req(&["content", "attachments", "123"]).path,
            "/rest/api/content/123/child/attachment"
        );
        assert_eq!(
            req(&["content", "labels", "123"]).path,
            "/rest/api/content/123/label"
        );
        assert_eq!(
            req(&["content", "history", "123"]).path,
            "/rest/api/content/123/history"
        );
    }

    #[test]
    fn content_get_query() {
        let r = req(&[
            "content",
            "get",
            "1",
            "--expand",
            "body.storage",
            "--version",
            "3",
        ]);
        assert_eq!(
            r.query,
            vec![
                ("expand".into(), "body.storage".into()),
                ("version".into(), "3".into())
            ]
        );
    }

    #[test]
    fn search_uses_cql() {
        let r = req(&["search", "--cql", "type = page", "--start", "25"]);
        assert_eq!(r.path, "/rest/api/content/search");
        assert_eq!(
            r.first_query(),
            vec![
                ("cql".into(), "type = page".into()),
                ("start".into(), "25".into())
            ]
        );
    }

    #[test]
    fn user_get_requires_username_or_key() {
        assert!(Cli::try_parse_from(["atlassian", "confluence", "user", "get"]).is_err());
        let r = req(&["user", "get", "--key", "abc"]);
        assert_eq!(r.query, vec![("key".into(), "abc".into())]);
    }

    #[test]
    fn group_members_encodes_name() {
        assert_eq!(
            req(&["group", "members", "confluence users"]).path,
            "/rest/api/group/confluence%20users/member"
        );
    }
}
