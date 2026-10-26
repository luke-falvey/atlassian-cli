//! Every `confluence` command against a mock built from the official Confluence DC spec.

mod support;

#[tokio::test]
async fn confluence_commands_match_spec() {
    support::check_commands_against_spec(
        "confluence.json",
        &[
            &[
                "confluence",
                "space",
                "list",
                "--type",
                "global",
                "--key",
                "DEV",
            ],
            &[
                "confluence",
                "space",
                "get",
                "DEV",
                "--expand",
                "description.plain",
            ],
            &["confluence", "space", "content", "DEV"],
            &["confluence", "space", "content", "DEV", "--type", "page"],
            &[
                "confluence",
                "content",
                "list",
                "--space",
                "DEV",
                "--type",
                "page",
            ],
            &[
                "confluence",
                "content",
                "get",
                "123",
                "--expand",
                "body.storage",
            ],
            &["confluence", "content", "children", "123"],
            &[
                "confluence",
                "content",
                "descendants",
                "123",
                "--type",
                "comment",
            ],
            &["confluence", "content", "attachments", "123"],
            &["confluence", "content", "comments", "123", "--depth", "all"],
            &["confluence", "content", "labels", "123"],
            &["confluence", "content", "history", "123"],
            &[
                "confluence",
                "content",
                "history",
                "123",
                "--expand",
                "previousVersion",
            ],
            &["confluence", "search", "--cql", "type = page"],
            &["confluence", "user", "current"],
            &["confluence", "user", "get", "--username", "alice"],
            &["confluence", "group", "list"],
            &["confluence", "group", "members", "confluence-users"],
            &["confluence", "raw", "/rest/api/space"],
        ],
    )
    .await;
}
