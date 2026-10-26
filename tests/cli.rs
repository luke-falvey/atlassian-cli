//! End-to-end tests of the `atlassian` binary.

mod support;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::{Value, json};
use support::{TestConfig, credentials_toml, expected_auth};
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn atlassian() -> Command {
    Command::cargo_bin("atlassian").unwrap()
}

/// Run the binary off the async runtime and return stdout as JSON.
async fn run_json(args: Vec<String>) -> Value {
    let out = tokio::task::spawn_blocking(move || atlassian().args(&args).assert().success())
        .await
        .unwrap();
    serde_json::from_slice(&out.get_output().stdout).unwrap()
}

fn args(server: &MockServer, config: &TestConfig, rest: &[&str]) -> Vec<String> {
    let mut v = vec![
        "--base-url".to_string(),
        server.uri(),
        "--config".to_string(),
        config.path.display().to_string(),
    ];
    v.extend(rest.iter().map(|s| s.to_string()));
    v
}

/// Fails the test (on drop) if anything other than GET reaches the server.
async fn forbid_non_get(server: &MockServer) {
    Mock::given(|req: &Request| req.method != wiremock::http::Method::GET)
        .respond_with(ResponseTemplate::new(405))
        .expect(0)
        .named("non-GET request")
        .mount(server)
        .await;
}

#[test]
fn help_lists_products() {
    atlassian().arg("--help").assert().success().stdout(
        predicate::str::contains("jira")
            .and(predicate::str::contains("agile"))
            .and(predicate::str::contains("confluence"))
            .and(predicate::str::contains("bitbucket"))
            .and(predicate::str::contains("bamboo")),
    );
}

#[test]
fn product_help_lists_resources() {
    atlassian()
        .args(["bitbucket", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("pull-request").and(predicate::str::contains("repo")));
    atlassian()
        .args(["jira", "issue", "--help"])
        .assert()
        .success()
        .stdout(predicate::str::contains("comments").and(predicate::str::contains("transitions")));
}

#[test]
fn version_flag() {
    atlassian()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn missing_config_file_is_a_clear_error() {
    atlassian()
        .args(["--config", "/no/such/atlassian.toml", "jira", "myself"])
        .assert()
        .failure()
        .stderr(predicate::str::contains(
            "could not read config file /no/such/atlassian.toml",
        ));
}

#[test]
fn invalid_config_file_is_a_clear_error() {
    let config = TestConfig::with("[jira]\nurl = \"https://jira.example.com\"\n");
    atlassian()
        .arg("--config")
        .arg(&config.path)
        .args(["jira", "myself"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("invalid config file").and(predicate::str::contains("url")),
        );
}

#[test]
fn missing_base_url_names_the_config_section() {
    let config = TestConfig::new();
    atlassian()
        .arg("--config")
        .arg(&config.path)
        .args(["confluence", "user", "current"])
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("[confluence]").and(predicate::str::contains("--base-url")),
        );
}

#[test]
fn missing_credentials_are_a_clear_error() {
    let config = TestConfig::with("[bitbucket]\nbase_url = \"http://127.0.0.1:9\"\n");
    atlassian()
        .arg("--config")
        .arg(&config.path)
        .args(["bitbucket", "project", "list"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("no username for bitbucket"));
}

/// Mount a `GET <path>` that requires `auth` and returns `{"name": "test-user"}`.
async fn mount_myself(server: &MockServer, path_str: &str, auth: &str) {
    Mock::given(method("GET"))
        .and(path(path_str))
        .and(header("authorization", auth))
        .and(header("accept", "application/json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"name": "test-user"})))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn base_url_and_credentials_from_config_sections() {
    let server = MockServer::start().await;
    forbid_non_get(&server).await;
    mount_myself(&server, "/jira/rest/api/2/myself", &expected_auth()).await;
    mount_myself(
        &server,
        "/wiki/rest/api/user/current",
        &support::basic_auth("conf-user", "conf-pass"),
    )
    .await;

    // Jira (and agile) use the top-level credentials; Confluence overrides them.
    let config = TestConfig::with(&format!(
        "{}\n[jira]\nbase_url = \"{uri}/jira/\"\n\n[confluence]\nbase_url = \"{uri}/wiki\"\nusername = \"conf-user\"\npassword = \"conf-pass\"\n",
        credentials_toml(),
        uri = server.uri()
    ));
    let path = config.path.clone();
    let (jira, conf) = tokio::task::spawn_blocking(move || {
        let jira = atlassian()
            .arg("--config")
            .arg(&path)
            .args(["jira", "myself", "--compact"])
            .assert()
            .success();
        let conf = atlassian()
            .arg("--config")
            .arg(&path)
            .args(["confluence", "user", "current", "--compact"])
            .assert()
            .success();
        (jira, conf)
    })
    .await
    .unwrap();
    for out in [jira, conf] {
        assert_eq!(
            String::from_utf8_lossy(&out.get_output().stdout),
            "{\"name\":\"test-user\"}\n"
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn agile_uses_the_jira_section() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/rest/agile/1.0/board/1"))
        .and(header("authorization", expected_auth().as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"id": 1})))
        .expect(1)
        .mount(&server)
        .await;
    let config = TestConfig::with(&format!(
        "{}[jira]\nbase_url = \"{}\"\n",
        credentials_toml(),
        server.uri()
    ));
    let path = config.path.clone();
    tokio::task::spawn_blocking(move || {
        atlassian()
            .arg("--config")
            .arg(&path)
            .args(["agile", "board", "get", "1"])
            .assert()
            .success();
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn base_url_flag_overrides_config() {
    let server = MockServer::start().await;
    mount_myself(&server, "/rest/api/2/myself", &expected_auth()).await;
    let config = TestConfig::with(&format!(
        "{}[jira]\nbase_url = \"http://127.0.0.1:9\"\n",
        credentials_toml()
    ));
    let out = run_json(args(&server, &config, &["jira", "myself"])).await;
    assert_eq!(out, json!({"name": "test-user"}));
}

#[tokio::test(flavor = "multi_thread")]
async fn http_errors_exit_non_zero_with_server_message() {
    let server = MockServer::start().await;
    Mock::given(path("/rest/api/2/issue/NOPE-1"))
        .respond_with(ResponseTemplate::new(404).set_body_json(json!({
            "errorMessages": ["Issue Does Not Exist"], "errors": {}
        })))
        .mount(&server)
        .await;
    let config = TestConfig::new();
    let argv = args(&server, &config, &["jira", "issue", "get", "NOPE-1"]);
    tokio::task::spawn_blocking(move || {
        atlassian().args(&argv).assert().failure().stderr(
            predicate::str::contains("404").and(predicate::str::contains("Issue Does Not Exist")),
        );
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn jira_all_follows_start_at_and_total() {
    let server = MockServer::start().await;
    forbid_non_get(&server).await;
    for (start, issues) in [
        ("0", json!([{"key": "A-1"}, {"key": "A-2"}])),
        ("2", json!([{"key": "A-3"}])),
    ] {
        Mock::given(method("GET"))
            .and(path("/rest/api/2/search"))
            .and(query_param("jql", "project = A"))
            .and(query_param("startAt", start))
            .and(query_param("maxResults", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "startAt": start.parse::<u64>().unwrap(), "maxResults": 2, "total": 3, "issues": issues
            })))
            .expect(1)
            .mount(&server)
            .await;
    }
    let config = TestConfig::new();
    let out = run_json(args(
        &server,
        &config,
        &[
            "jira",
            "search",
            "--jql",
            "project = A",
            "--limit",
            "2",
            "--all",
        ],
    ))
    .await;
    assert_eq!(out, json!([{"key": "A-1"}, {"key": "A-2"}, {"key": "A-3"}]));
}

#[tokio::test(flavor = "multi_thread")]
async fn confluence_all_follows_next_links() {
    let server = MockServer::start().await;
    forbid_non_get(&server).await;
    Mock::given(path("/rest/api/space"))
        .and(query_param("start", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"key": "A"}], "start": 0, "limit": 1, "size": 1,
            "_links": {"next": "/rest/api/space?start=1"}
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path("/rest/api/space"))
        .and(query_param("start", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{"key": "B"}], "start": 1, "limit": 1, "size": 1, "_links": {}
        })))
        .expect(1)
        .mount(&server)
        .await;
    let config = TestConfig::new();
    let out = run_json(args(
        &server,
        &config,
        &["confluence", "space", "list", "--all"],
    ))
    .await;
    assert_eq!(out, json!([{"key": "A"}, {"key": "B"}]));
}

#[tokio::test(flavor = "multi_thread")]
async fn bitbucket_all_follows_next_page_start() {
    let server = MockServer::start().await;
    forbid_non_get(&server).await;
    let repos = "/rest/api/latest/projects/PROJ/repos";
    Mock::given(path(repos))
        .and(query_param("start", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [{"slug": "a"}], "isLastPage": false, "nextPageStart": 25
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(path(repos))
        .and(query_param("start", "25"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "values": [{"slug": "b"}], "isLastPage": true
        })))
        .expect(1)
        .mount(&server)
        .await;
    let config = TestConfig::new();
    let out = run_json(args(
        &server,
        &config,
        &["bitbucket", "repo", "list", "PROJ", "--all"],
    ))
    .await;
    assert_eq!(out, json!([{"slug": "a"}, {"slug": "b"}]));
}

#[tokio::test(flavor = "multi_thread")]
async fn all_on_unpaged_response_returns_it_unchanged() {
    let server = MockServer::start().await;
    Mock::given(path("/rest/api/2/dashboard"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"unexpected": "shape"})))
        .expect(1)
        .mount(&server)
        .await;
    let config = TestConfig::new();
    let out = run_json(args(
        &server,
        &config,
        &["jira", "dashboard", "list", "--all"],
    ))
    .await;
    assert_eq!(out, json!({"unexpected": "shape"}));
}

#[tokio::test(flavor = "multi_thread")]
async fn bitbucket_file_content_prints_raw_text() {
    let server = MockServer::start().await;
    Mock::given(path(
        "/rest/api/latest/projects/P/repos/r/raw/docs/README.md",
    ))
    .and(query_param("at", "refs/heads/main"))
    .respond_with(ResponseTemplate::new(200).set_body_raw("# Title\n", "text/plain"))
    .mount(&server)
    .await;
    let config = TestConfig::new();
    let argv = args(
        &server,
        &config,
        &[
            "bitbucket",
            "file",
            "content",
            "P",
            "r",
            "docs/README.md",
            "--at",
            "refs/heads/main",
        ],
    );
    tokio::task::spawn_blocking(move || {
        atlassian()
            .args(&argv)
            .assert()
            .success()
            .stdout("# Title\n");
    })
    .await
    .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn raw_subcommand_passes_query_params() {
    let server = MockServer::start().await;
    forbid_non_get(&server).await;
    Mock::given(method("GET"))
        .and(path("/rest/api/2/configuration"))
        .and(query_param("a", "1"))
        .and(query_param("b", "x=y"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"ok": true})))
        .expect(1)
        .mount(&server)
        .await;
    let config = TestConfig::new();
    let out = run_json(args(
        &server,
        &config,
        &[
            "jira",
            "raw",
            "/rest/api/2/configuration",
            "-q",
            "a=1",
            "--query",
            "b=x=y",
        ],
    ))
    .await;
    assert_eq!(out, json!({"ok": true}));
}

#[tokio::test(flavor = "multi_thread")]
async fn reads_atlassian_toml_from_home_by_default() {
    let server = MockServer::start().await;
    mount_myself(&server, "/rest/api/2/myself", &expected_auth()).await;

    let home = tempfile::tempdir().unwrap();
    std::fs::write(
        home.path().join(".atlassian.toml"),
        format!(
            "{}[jira]\nbase_url = \"{}\"\n",
            credentials_toml(),
            server.uri()
        ),
    )
    .unwrap();
    let home_path = home.path().to_path_buf();
    tokio::task::spawn_blocking(move || {
        atlassian()
            .env("HOME", &home_path)
            .args(["jira", "myself"])
            .assert()
            .success()
            .stdout(predicate::str::contains("test-user"));
    })
    .await
    .unwrap();
}
