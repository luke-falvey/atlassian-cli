//! Every `bitbucket` command against a mock built from the official Bitbucket DC spec.

mod support;

#[tokio::test]
async fn bitbucket_commands_match_spec() {
    support::check_commands_against_spec(
        "bitbucket.json",
        &[
            &["bitbucket", "application-properties"],
            &["bitbucket", "project", "list", "--name", "core"],
            &["bitbucket", "project", "get", "PROJ"],
            &["bitbucket", "repo", "list"],
            &["bitbucket", "repo", "list", "PROJ"],
            &["bitbucket", "repo", "get", "PROJ", "repo"],
            &["bitbucket", "branch", "list", "PROJ", "repo", "--details"],
            &["bitbucket", "branch", "default", "PROJ", "repo"],
            &["bitbucket", "tag", "list", "PROJ", "repo"],
            &[
                "bitbucket",
                "commit",
                "list",
                "PROJ",
                "repo",
                "--until",
                "main",
            ],
            &["bitbucket", "commit", "get", "PROJ", "repo", "abc123"],
            &["bitbucket", "commit", "changes", "PROJ", "repo", "abc123"],
            &[
                "bitbucket",
                "pull-request",
                "list",
                "PROJ",
                "repo",
                "--state",
                "ALL",
            ],
            &["bitbucket", "pull-request", "get", "PROJ", "repo", "1"],
            &[
                "bitbucket",
                "pull-request",
                "activities",
                "PROJ",
                "repo",
                "1",
            ],
            &["bitbucket", "pull-request", "changes", "PROJ", "repo", "1"],
            &["bitbucket", "pull-request", "commits", "PROJ", "repo", "1"],
            &["bitbucket", "file", "list", "PROJ", "repo"],
            &["bitbucket", "file", "list", "PROJ", "repo", "src"],
            &["bitbucket", "file", "browse", "PROJ", "repo", "src/main.rs"],
            &["bitbucket", "file", "content", "PROJ", "repo", "README.md"],
            &["bitbucket", "user", "list"],
            &["bitbucket", "user", "get", "alice"],
            &["bitbucket", "group", "list"],
            &["bitbucket", "raw", "/rest/api/latest/projects"],
        ],
    )
    .await;
}
