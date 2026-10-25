//! Every `bamboo` command against a mock built from the Bamboo DC spec.

mod support;

#[tokio::test]
async fn bamboo_commands_match_spec() {
    support::check_commands_against_spec(
        "bamboo.json",
        &[
            &["bamboo", "info"],
            &["bamboo", "server"],
            &["bamboo", "current-user"],
            &["bamboo", "project", "list"],
            &["bamboo", "project", "get", "PROJ"],
            &["bamboo", "plan", "list", "--limit", "25"],
            &["bamboo", "plan", "get", "PROJ-PLAN"],
            &["bamboo", "plan", "branches", "PROJ-PLAN"],
            &["bamboo", "result", "list"],
            &[
                "bamboo",
                "result",
                "list",
                "PROJ-PLAN",
                "--build-state",
                "Failed",
            ],
            &["bamboo", "result", "get", "PROJ-PLAN-42"],
            &["bamboo", "result", "latest", "PROJ-PLAN"],
            &["bamboo", "deploy", "project", "list"],
            &["bamboo", "deploy", "project", "get", "1"],
            &["bamboo", "deploy", "environment-results", "2"],
            &["bamboo", "queue"],
            &["bamboo", "agent", "list"],
            &["bamboo", "raw", "/rest/api/latest/info"],
        ],
    )
    .await;
}
