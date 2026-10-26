//! Every `jira` command against a mock built from the official Jira DC spec.

mod support;

/// Every jira command, as one sample invocation each.
const COMMANDS: &[&[&str]] = &[
    &["jira", "server-info"],
    &["jira", "myself"],
    &[
        "jira",
        "issue",
        "get",
        "ABC-1",
        "--fields",
        "summary",
        "--expand",
        "changelog",
    ],
    &["jira", "issue", "comments", "ABC-1", "--limit", "10"],
    &["jira", "issue", "worklogs", "ABC-1"],
    &["jira", "issue", "transitions", "ABC-1"],
    &["jira", "issue", "watchers", "ABC-1"],
    &["jira", "issue", "votes", "ABC-1"],
    &["jira", "issue", "remote-links", "ABC-1"],
    &["jira", "issue", "list"],
    &[
        "jira",
        "issue",
        "list",
        "--project",
        "ABC",
        "--assignee",
        "me",
        "--status",
        "Open",
        "--type",
        "Bug",
        "--open",
        "--updated-since",
        "7d",
        "--order-by",
        "priority",
        "--start",
        "0",
        "--limit",
        "50",
    ],
    &["jira", "project", "list", "--expand", "lead"],
    &["jira", "project", "get", "ABC"],
    &["jira", "project", "components", "ABC"],
    &["jira", "project", "versions", "ABC"],
    &["jira", "project", "statuses", "ABC"],
    &["jira", "project", "roles", "ABC"],
    &["jira", "user", "get", "alice"],
    &["jira", "user", "search", "ali"],
    &["jira", "group", "members", "jira-users"],
    &["jira", "group", "search", "jira"],
    &["jira", "field", "list"],
    &["jira", "issue-type", "list"],
    &["jira", "issue-type", "get", "10000"],
    &["jira", "issue-link-type", "list"],
    &["jira", "issue-link-type", "get", "10000"],
    &["jira", "priority", "list"],
    &["jira", "priority", "get", "3"],
    &["jira", "status", "list"],
    &["jira", "status", "get", "1"],
    &["jira", "resolution", "list"],
    &["jira", "resolution", "get", "1"],
    &["jira", "filter", "get", "10000"],
    &["jira", "filter", "favourite"],
    &["jira", "dashboard", "list"],
    &["jira", "dashboard", "get", "10000"],
    &["jira", "raw", "/rest/api/2/serverInfo"],
];

/// Jira Software DC 10.0.0 (official OpenAPI file).
#[tokio::test]
async fn jira_commands_match_10_0_spec() {
    support::check_commands_against_spec("jira.json", COMMANDS).await;
}

/// Jira 9.12.0, converted from the HTML REST reference.
#[tokio::test]
async fn jira_commands_match_9_12_docs() {
    support::check_commands_against_spec("jira-9.12.json", COMMANDS).await;
}
