//! Every `jira` command against a mock built from the official Jira DC spec.

mod support;

#[tokio::test]
async fn jira_commands_match_spec() {
    support::check_commands_against_spec(
        "jira.json",
        &[
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
            &[
                "jira",
                "search",
                "--jql",
                "project = ABC",
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
        ],
    )
    .await;
}
