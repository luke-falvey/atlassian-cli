# Vendored OpenAPI specifications

The test suite builds its mock servers from these files (see
`tests/support/mod.rs`). Every CLI command is run against the mock, and a
command whose URL isn't in the spec fails its test.

| File                 | Product                     | Version | Source URL |
|----------------------|-----------------------------|---------|------------|
| `jira.json`          | Jira Data Center (platform) | _todo_  | _todo_     |
| `jira-software.json` | Jira Software (Agile)       | _todo_  | _todo_     |
| `confluence.json`    | Confluence Data Center      | _todo_  | _todo_     |
| `bitbucket.json`     | Bitbucket Data Center       | _todo_  | _todo_     |
| `bamboo.json`        | Bamboo Data Center          | _todo_  | _todo_     |

## Status

**These files are not committed yet.** `dac-static.atlassian.com` was not
reachable from the environment the CLI was built in. Until they are added,
the five spec tests (`tests/{jira,agile,confluence,bitbucket,bamboo}.rs`)
fail with a message pointing here. All other tests run without them.

## Adding or refreshing a spec

```sh
scripts/fetch-specs.sh            # all products
scripts/fetch-specs.sh bitbucket  # one product
```

Then fill in the version and URL above and run `cargo test`. If a test now
fails, the spec has diverged from the CLI. Fix the command, or remove it if
the endpoint no longer exists.

Both OpenAPI 3 and Swagger 2 files are supported. If Atlassian doesn't publish
an OpenAPI file for a product (Bamboo's REST docs have historically been WADL
only), write a small OpenAPI 3 file covering the endpoints the CLI uses. Mark
it as **unofficial** in the table above.
