# Vendored OpenAPI specifications

The test suite builds its mock servers from these files (see
`tests/support/mod.rs`). Every CLI command is run against the mock, and a
command whose URL isn't in the spec fails its test.

| File | Product | Version | Source |
|------|---------|---------|--------|
| `jira.json` | Jira Software DC (`/rest/api/2` + `/rest/agile/1.0`) | 10.0.0 | https://dac-static.atlassian.com/server/jira/platform/jira_software_dc_10000_swagger.v3.json |
| `jira-9.12.json` | Jira DC (`/rest/api/2`), GET operations only | 9.12.0 | Converted from https://docs.atlassian.com/software/jira/docs/api/REST/9.12.0/ by `scripts/jira-docs-to-openapi.py` |
| `confluence.json` | Confluence DC | 10.2.15 | https://dac-static.atlassian.com/server/confluence/10.2.15.swagger.v3.json (page: https://developer.atlassian.com/server/confluence/rest/v10215/) |
| `bitbucket.json` | Bitbucket DC | 10.2 | https://dac-static.atlassian.com/server/bitbucket/10.2.swagger.v3.json (page: https://developer.atlassian.com/server/bitbucket/rest/v1002/) |

## Notes

- `tests/jira.rs` runs every jira command against both Jira specs.
  `tests/agile.rs` uses `jira.json`, which also covers the agile API.
- Jira 9.x has no OpenAPI file. The 9.12.0 reference is a single HTML page, and
  the WADL it links to returns "NoSuchKey". `scripts/jira-docs-to-openapi.py`
  turns that page into `jira-9.12.json`, keeping paths, query parameters, the
  success status and the documented example or schema for each GET operation.
- The Confluence spec uses its context path (`/confluence`) as the server URL
  and full `/rest/api/...` paths. The mock passes that context path to the CLI
  as part of the base URL, as a real user would.
- Spec quirks that the CLI handles correctly:
  - Jira 10.0 `GET /api/2/group/member` lists `groupname` as a path parameter,
    but the path has no placeholder. Jira takes it as a query parameter, which
    is what the CLI sends.
  - Some operations (e.g. Bitbucket `browse/{path}` and `raw/{path}`, Jira 9.12
    `dashboard`) document no response body. For those, the mock checks only the
    path and parameters.
- Example credentials (sample access tokens, passwords) in the official specs
  are replaced with `REDACTED-EXAMPLE` by `scripts/redact-spec-examples.py`,
  which `scripts/fetch-specs.sh` runs on every download. They are only
  documentation samples, but they match real token formats, so GitHub push
  protection rejects them.

## Adding or refreshing a spec

```sh
scripts/fetch-specs.sh            # all products
scripts/fetch-specs.sh bitbucket  # one product
```

Then fill in the version and URL above and run `cargo test`. If a test now
fails, the spec has diverged from the CLI. Fix the command, or remove it if
the endpoint no longer exists.

Both OpenAPI 3 and Swagger 2 files are supported. If Atlassian doesn't publish
an OpenAPI file for a product, write a small OpenAPI 3 file covering the endpoints the CLI uses. Mark
it as **unofficial** in the table above.
