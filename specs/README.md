# Vendored OpenAPI specifications

The test suite builds its mock servers from these files (see
`tests/support/mod.rs`). Every CLI command is run against the mock, and a
command whose URL isn't in the spec fails its test.

| File                 | Product                     | Version | Source URL |
|----------------------|-----------------------------|---------|------------|
| `jira.json`          | Jira Software Data Center (platform `/rest/api/2` + agile `/rest/agile/1.0`) | 10.0.0 | https://dac-static.atlassian.com/server/jira/platform/jira_software_dc_10000_swagger.v3.json |
| `confluence.json`    | Confluence Data Center      | _todo_  | _todo_     |
| `bitbucket.json`     | Bitbucket Data Center       | _todo_  | _todo_     |
| `bamboo.json`        | Bamboo Data Center          | _todo_  | _todo_     |

## Status

`jira.json` is vendored and used by both `tests/jira.rs` and `tests/agile.rs`.
Atlassian publishes OpenAPI files only for Jira 10.0 and later; Jira 9.x
(e.g. 9.12) is documented only as HTML at docs.atlassian.com, so 10.0.0 is the
oldest official spec available. Its `/rest/api/2` endpoints used by the CLI
are unchanged from 9.x.

Known spec quirk: `GET /api/2/group/member` declares `groupname` as a *path*
parameter although the path has no placeholder. Jira takes it as a query
parameter, which is what the CLI sends.

The Confluence, Bitbucket and Bamboo specs are not committed yet. Until they
are, their spec tests fail with a message pointing here.

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
