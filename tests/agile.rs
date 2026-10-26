//! Every `agile` command against a mock built from the official Jira Software DC spec (which also covers the agile API).

mod support;

#[tokio::test]
async fn agile_commands_match_spec() {
    support::check_commands_against_spec(
        "jira.json",
        &[
            &[
                "agile",
                "board",
                "list",
                "--type",
                "scrum",
                "--project",
                "ABC",
            ],
            &["agile", "board", "get", "1"],
            &["agile", "board", "configuration", "1"],
            &["agile", "board", "sprints", "1", "--state", "active"],
            &["agile", "board", "issues", "1", "--fields", "summary"],
            &["agile", "board", "backlog", "1"],
            &["agile", "board", "epics", "1", "--done", "false"],
            &["agile", "sprint", "get", "2"],
            &["agile", "sprint", "issues", "2"],
            &["agile", "epic", "get", "ABC-1"],
            &["agile", "epic", "issues", "ABC-1"],
            &["agile", "raw", "/rest/agile/1.0/board/1"],
        ],
    )
    .await;
}
