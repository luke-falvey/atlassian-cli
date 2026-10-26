//! Scope guardrails end to end: what a `[scope]` config lets through, what it
//! refuses (without sending the request), how lists are filtered, and what
//! the request log records.

mod support;

use atlassian_cli::Output;
use serde_json::{Value, json};
use support::{TestConfig, credentials_toml, run_cli};
use wiremock::matchers::{any, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SCOPE: &str = r#"
[scope]
jira_projects = ["ABC"]
confluence_spaces = ["DEV"]
bitbucket_repos = ["PROJ/api", "WEB/*"]
"#;

fn scoped(extra: &str) -> TestConfig {
    TestConfig::with(&format!("{}{SCOPE}{extra}", credentials_toml()))
}

fn ok(body: Value) -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_json(body)
}

/// Fails the test (on drop) if any request reaches the server.
async fn no_requests(server: &MockServer) {
    Mock::given(any())
        .respond_with(ResponseTemplate::new(500))
        .expect(0)
        .named("no request should be sent")
        .mount(server)
        .await;
}

async fn get(path_str: &str, body: Value, times: u64, server: &MockServer) {
    Mock::given(method("GET"))
        .and(path(path_str))
        .respond_with(ok(body))
        .expect(times)
        .mount(server)
        .await;
}

async fn run(server: &MockServer, config: &TestConfig, args: &[&str]) -> anyhow::Result<Output> {
    run_cli(&server.uri(), config, args).await
}

fn json(out: Output) -> Value {
    out.json().cloned().expect("JSON output")
}

fn assert_blocked(result: anyhow::Result<Output>, needle: &str) {
    let msg = format!("{:#}", result.expect_err("should be blocked"));
    assert!(msg.contains("blocked by scope"), "{msg}");
    assert!(msg.contains(needle), "expected `{needle}` in: {msg}");
}

// ---------- Jira ----------

#[tokio::test]
async fn jira_issue_in_scope_is_fetched() {
    let server = MockServer::start().await;
    get(
        "/rest/api/2/issue/ABC-1",
        json!({"key": "ABC-1"}),
        1,
        &server,
    )
    .await;
    let out = run(&server, &scoped(""), &["jira", "issue", "get", "ABC-1"]).await;
    assert_eq!(json(out.unwrap()), json!({"key": "ABC-1"}));
}

#[tokio::test]
async fn jira_issue_out_of_scope_is_never_requested() {
    let server = MockServer::start().await;
    no_requests(&server).await;
    let config = scoped("");
    for args in [
        &["jira", "issue", "get", "XYZ-1"][..],
        &["jira", "issue", "comments", "xyz-9"],
        &["jira", "project", "get", "XYZ"],
    ] {
        assert_blocked(run(&server, &config, args).await, "is not allowed");
    }
    // Numeric ids can't be checked, so they're refused.
    assert_blocked(
        run(&server, &config, &["jira", "issue", "get", "10001"]).await,
        "use the key",
    );
}

#[tokio::test]
async fn jira_issue_list_is_limited_to_allowed_projects() {
    let server = MockServer::start().await;
    Mock::given(path("/rest/api/2/search"))
        .and(query_param(
            "jql",
            r#"project in ("ABC") AND assignee = currentUser() ORDER BY updated DESC"#,
        ))
        .respond_with(ok(json!({
            "startAt": 0, "total": 2,
            "issues": [{"key": "ABC-1"}, {"key": "XYZ-2"}]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let out = run(
        &server,
        &scoped(""),
        &["jira", "issue", "list", "--assignee", "me"],
    )
    .await;
    // The XYZ issue slips past the server-side query in this mock, and the
    // post-filter still drops it.
    assert_eq!(
        json(out.unwrap()),
        json!({"startAt": 0, "total": 2, "issues": [{"key": "ABC-1"}]})
    );
}

#[tokio::test]
async fn jira_issue_list_refuses_other_projects() {
    let server = MockServer::start().await;
    no_requests(&server).await;
    let out = run(
        &server,
        &scoped(""),
        &["jira", "issue", "list", "--project", "XYZ"],
    )
    .await;
    assert_blocked(out, "XYZ");
}

#[tokio::test]
async fn jira_project_list_is_filtered() {
    let server = MockServer::start().await;
    get(
        "/rest/api/2/project",
        json!([{"key": "ABC"}, {"key": "HR"}]),
        1,
        &server,
    )
    .await;
    let out = run(&server, &scoped(""), &["jira", "project", "list"]).await;
    assert_eq!(json(out.unwrap()), json!([{"key": "ABC"}]));
}

#[tokio::test]
async fn instance_wide_commands_are_refused() {
    let server = MockServer::start().await;
    no_requests(&server).await;
    let config = scoped("");
    for args in [
        &["jira", "user", "search", "a"][..],
        &["jira", "group", "members", "jira-users"],
        &["jira", "filter", "favourite"],
        &["jira", "dashboard", "list"],
        &["confluence", "group", "list"],
        &["confluence", "user", "get", "--username", "a"],
        &["bitbucket", "user", "list"],
        &["bitbucket", "group", "list"],
    ] {
        assert_blocked(run(&server, &config, args).await, "whole instance");
    }
}

#[tokio::test]
async fn metadata_commands_are_allowed() {
    let server = MockServer::start().await;
    get("/rest/api/2/myself", json!({"name": "me"}), 1, &server).await;
    get("/rest/api/2/priority", json!([{"id": "1"}]), 1, &server).await;
    let config = scoped("");
    run(&server, &config, &["jira", "myself"]).await.unwrap();
    run(&server, &config, &["jira", "priority", "list"])
        .await
        .unwrap();
}

#[tokio::test]
async fn raw_is_refused_unless_allowed() {
    let server = MockServer::start().await;
    get("/rest/api/2/configuration", json!({}), 1, &server).await;
    let args = ["jira", "raw", "/rest/api/2/configuration"];
    assert_blocked(run(&server, &scoped(""), &args).await, "`raw`");
    run(&server, &scoped("allow_raw = true\n"), &args)
        .await
        .unwrap();
}

// ---------- Jira Software ----------

#[tokio::test]
async fn agile_board_is_checked_through_its_projects() {
    let server = MockServer::start().await;
    get(
        "/rest/agile/1.0/board/1/project",
        json!({"values": [{"key": "ABC"}]}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/agile/1.0/board/2/project",
        json!({"values": [{"key": "ABC"}, {"key": "HR"}]}),
        1,
        &server,
    )
    .await;
    get("/rest/agile/1.0/board/1", json!({"id": 1}), 1, &server).await;
    get("/rest/agile/1.0/board/2", json!({"id": 2}), 0, &server).await;
    let config = scoped("");
    run(&server, &config, &["agile", "board", "get", "1"])
        .await
        .unwrap();
    // Board 2 also shows HR issues, so it's refused.
    assert_blocked(
        run(&server, &config, &["agile", "board", "get", "2"]).await,
        "HR",
    );
}

#[tokio::test]
async fn agile_sprint_is_checked_through_its_board() {
    let server = MockServer::start().await;
    get(
        "/rest/agile/1.0/sprint/5",
        json!({"id": 5, "originBoardId": 9}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/agile/1.0/board/9/project",
        json!({"values": [{"key": "XYZ"}]}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/agile/1.0/sprint/5/issue",
        json!({"issues": []}),
        0,
        &server,
    )
    .await;
    let out = run(&server, &scoped(""), &["agile", "sprint", "issues", "5"]).await;
    assert_blocked(out, "XYZ");
}

#[tokio::test]
async fn agile_epic_by_id_is_looked_up() {
    let server = MockServer::start().await;
    get(
        "/rest/agile/1.0/epic/77",
        json!({"id": 77, "key": "ABC-3"}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/agile/1.0/epic/77/issue",
        json!({"issues": [{"key": "ABC-4"}, {"key": "HR-1"}]}),
        1,
        &server,
    )
    .await;
    let out = run(&server, &scoped(""), &["agile", "epic", "issues", "77"]).await;
    assert_eq!(json(out.unwrap()), json!({"issues": [{"key": "ABC-4"}]}));
}

#[tokio::test]
async fn agile_board_list_needs_an_allowed_project() {
    let server = MockServer::start().await;
    get("/rest/agile/1.0/board", json!({"values": []}), 1, &server).await;
    let config = scoped("");
    assert_blocked(
        run(&server, &config, &["agile", "board", "list"]).await,
        "--project",
    );
    assert_blocked(
        run(
            &server,
            &config,
            &["agile", "board", "list", "--project", "XYZ"],
        )
        .await,
        "XYZ",
    );
    run(
        &server,
        &config,
        &["agile", "board", "list", "--project", "ABC"],
    )
    .await
    .unwrap();
}

// ---------- Confluence ----------

async fn content_lookup(id: &str, space: &str, server: &MockServer) {
    Mock::given(path(format!("/rest/api/content/{id}")))
        .and(query_param("expand", "space"))
        .respond_with(ok(json!({"id": id, "space": {"key": space}})))
        .with_priority(1)
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn confluence_content_is_checked_through_its_space() {
    let server = MockServer::start().await;
    content_lookup("1", "DEV", &server).await;
    content_lookup("2", "HR", &server).await;
    get(
        "/rest/api/content/1/child/page",
        json!({"results": [{"id": "3"}]}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/api/content/2/child/page",
        json!({"results": []}),
        0,
        &server,
    )
    .await;
    let config = scoped("");
    let out = run(
        &server,
        &config,
        &["confluence", "content", "children", "1"],
    )
    .await;
    assert_eq!(json(out.unwrap()), json!({"results": [{"id": "3"}]}));
    assert_blocked(
        run(
            &server,
            &config,
            &["confluence", "content", "children", "2"],
        )
        .await,
        "`HR`",
    );
}

#[tokio::test]
async fn confluence_space_list_asks_only_for_allowed_spaces() {
    let server = MockServer::start().await;
    Mock::given(path("/rest/api/space"))
        .and(query_param("spaceKey", "DEV"))
        .respond_with(ok(json!({"results": [{"key": "DEV"}, {"key": "HR"}]})))
        .expect(1)
        .mount(&server)
        .await;
    let out = run(&server, &scoped(""), &["confluence", "space", "list"]).await;
    assert_eq!(json(out.unwrap()), json!({"results": [{"key": "DEV"}]}));
}

#[tokio::test]
async fn confluence_content_list_needs_an_allowed_space() {
    let server = MockServer::start().await;
    no_requests(&server).await;
    let config = scoped("");
    assert_blocked(
        run(&server, &config, &["confluence", "content", "list"]).await,
        "--space",
    );
    assert_blocked(
        run(
            &server,
            &config,
            &["confluence", "content", "list", "--space", "HR"],
        )
        .await,
        "HR",
    );
}

// ---------- Bitbucket ----------

#[tokio::test]
async fn bitbucket_repos_match_exact_names_and_wildcards() {
    let server = MockServer::start().await;
    get(
        "/rest/api/latest/projects/PROJ/repos/api",
        json!({"slug": "api"}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/api/latest/projects/WEB/repos/site/branches",
        json!({"values": []}),
        1,
        &server,
    )
    .await;
    get(
        "/rest/api/latest/projects/PROJ/repos/secrets",
        json!({}),
        0,
        &server,
    )
    .await;
    let config = scoped("");
    run(
        &server,
        &config,
        &["bitbucket", "repo", "get", "PROJ", "api"],
    )
    .await
    .unwrap();
    run(
        &server,
        &config,
        &["bitbucket", "branch", "list", "WEB", "site"],
    )
    .await
    .unwrap();
    assert_blocked(
        run(
            &server,
            &config,
            &["bitbucket", "repo", "get", "PROJ", "secrets"],
        )
        .await,
        "PROJ/secrets",
    );
}

#[tokio::test]
async fn bitbucket_repo_list_is_filtered() {
    let server = MockServer::start().await;
    get(
        "/rest/api/latest/repos",
        json!({"isLastPage": true, "values": [
            {"slug": "api", "project": {"key": "PROJ"}},
            {"slug": "secrets", "project": {"key": "PROJ"}},
            {"slug": "site", "project": {"key": "WEB"}}
        ]}),
        1,
        &server,
    )
    .await;
    let out = run(
        &server,
        &scoped(""),
        &["bitbucket", "repo", "list", "--all"],
    )
    .await;
    assert_eq!(
        json(out.unwrap()),
        json!([
            {"slug": "api", "project": {"key": "PROJ"}},
            {"slug": "site", "project": {"key": "WEB"}}
        ])
    );
}

// ---------- Limits and logging ----------

#[tokio::test]
async fn all_stops_at_max_results() {
    let server = MockServer::start().await;
    Mock::given(path("/rest/api/latest/repos"))
        .respond_with(ok(json!({
            "isLastPage": false, "nextPageStart": 2,
            "values": [
                {"slug": "api", "project": {"key": "PROJ"}},
                {"slug": "site", "project": {"key": "WEB"}}
            ]
        })))
        .expect(1)
        .mount(&server)
        .await;
    let config = scoped("max_results = 2\n");
    let out = run(&server, &config, &["bitbucket", "repo", "list", "--all"]).await;
    assert_eq!(json(out.unwrap()).as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn without_a_scope_nothing_is_checked() {
    let server = MockServer::start().await;
    // No board/project lookup and no filtering.
    get("/rest/agile/1.0/board/2", json!({"id": 2}), 1, &server).await;
    get(
        "/rest/api/2/project",
        json!([{"key": "ABC"}, {"key": "HR"}]),
        1,
        &server,
    )
    .await;
    let config = TestConfig::new();
    run(&server, &config, &["agile", "board", "get", "2"])
        .await
        .unwrap();
    let out = run(&server, &config, &["jira", "project", "list"]).await;
    assert_eq!(json(out.unwrap()), json!([{"key": "ABC"}, {"key": "HR"}]));
}

#[tokio::test]
async fn requests_and_refusals_are_logged() {
    let server = MockServer::start().await;
    content_lookup("1", "DEV", &server).await;
    get("/rest/api/content/1", json!({"id": "1"}), 1, &server).await;
    let config = scoped("");
    run(&server, &config, &["confluence", "content", "get", "1"])
        .await
        .unwrap();
    let _ = run(&server, &config, &["jira", "issue", "get", "XYZ-1"]).await;

    let log = config.log_entries();
    assert_eq!(log.len(), 3, "{log:#?}");
    // The space lookup, then the content itself.
    assert_eq!(log[0]["outcome"], "ok");
    assert!(
        log[0]["url"]
            .as_str()
            .unwrap()
            .contains("/rest/api/content/1?expand=space")
    );
    assert_eq!(log[1]["status"], 200);
    assert_eq!(log[1]["product"], "confluence");
    // The refused command, with why.
    assert_eq!(log[2]["outcome"], "denied");
    assert_eq!(log[2]["product"], "jira");
    assert!(log[2]["reason"].as_str().unwrap().contains("XYZ"));
    // No credentials anywhere in the log.
    let text = std::fs::read_to_string(config.log_path()).unwrap();
    assert!(!text.contains(support::PASS) && !text.contains("Basic "));
}

#[tokio::test]
async fn logging_can_be_turned_off() {
    let server = MockServer::start().await;
    get("/rest/api/2/myself", json!({}), 1, &server).await;
    let config = TestConfig::with(&format!("{}[log]\nenabled = false\n", credentials_toml()));
    run(&server, &config, &["jira", "myself"]).await.unwrap();
    assert!(!config.log_path().exists());
}
