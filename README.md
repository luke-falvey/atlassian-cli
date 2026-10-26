# atlassian-cli

A read-only command line client for Atlassian **Data Center** products, written
in Rust with [clap](https://docs.rs/clap) and [reqwest](https://docs.rs/reqwest).

```
atlassian <product> <resource> <action> [args]
```

| Product      | Command      | REST API            |
|--------------|--------------|---------------------|
| Jira         | `jira`       | `/rest/api/2`       |
| Jira Software (Agile) | `agile` (alias `jira-software`) | `/rest/agile/1.0` |
| Confluence   | `confluence` | `/rest/api`         |
| Bitbucket    | `bitbucket`  | `/rest/api/latest`  |

This first version only **reads**. The HTTP client has no method other than
`GET`, and a test checks that no other method is ever sent.

## Install

```sh
cargo install --path .
```

## Configuration

Server URLs and credentials live in `~/.atlassian.toml`, with one section per
product:

```toml
# Default Basic-auth credentials, used by every product without its own.
username = "alice"
password = "my-password"

[jira]          # also used by `agile` (Jira Software)
base_url = "https://jira.example.com"

[confluence]
base_url = "https://wiki.example.com/confluence"   # include any context path
username = "svc-confluence"                        # optional per-product override
password = "other-password"

[bitbucket]
base_url = "https://bitbucket.example.com"
ca_cert = "/etc/ssl/corp-ca.pem"                   # optional extra CA certificate
```

```sh
chmod 600 ~/.atlassian.toml
```

- **Sections:** each product reads its own section: `[jira]`, `[confluence]`
  or `[bitbucket]`. `agile` uses `[jira]`, because Jira serves the Jira
  Software API.
- **Authentication:** only HTTP Basic auth is supported. A section's
  `username`/`password` override the top-level ones.
- **Unknown keys:** a misspelled key is an error, not silently ignored.
- **Permissions:** the CLI warns if other users can read the file.
- **Certificates:** the OS trust store is used for TLS. `ca_cert` adds one
  more PEM certificate for that product.

Flags override the file for one run:

| Flag | Overrides |
|------|-----------|
| `--config <PATH>` | the file location (default `~/.atlassian.toml`) |
| `--base-url <URL>` | the section's `base_url` |
| `--ca-cert <PATH>` | the section's `ca_cert` |

## Examples

```sh
atlassian jira myself
atlassian jira issue get ABC-123 --fields summary,status --expand changelog
atlassian jira search --jql 'project = ABC AND resolution = Unresolved' --all
atlassian agile board sprints 42 --state active
atlassian confluence search --cql 'space = DEV AND type = page' --limit 10
atlassian confluence content get 123456 --expand body.storage,version
atlassian bitbucket pull-request list PROJ my-repo --state OPEN
atlassian bitbucket file content PROJ my-repo README.md --at refs/heads/main

# Any GET endpoint that has no command yet:
atlassian jira raw /rest/api/2/application-properties -q key=jira.title
```

Use `--help` at any level (`atlassian bitbucket --help`,
`atlassian jira issue --help`) to see the resources, actions and options.

### Output

Responses are printed as pretty JSON, or compact JSON with `--compact`, so
they pipe into `jq`. Raw file content (`bitbucket file content`) is printed
as-is.

### Pagination

List commands take `--start` and `--limit`, mapped onto each product's own
parameters (`startAt`/`maxResults` or `start`/`limit`). `--all` follows every
page and prints the items as one JSON array.

## Commands

| Product | Resources |
|---------|-----------|
| `jira` | `server-info`, `myself`, `issue` (get, comments, worklogs, transitions, watchers, votes, remote-links), `search`, `project` (list, get, components, versions, statuses, roles), `user` (get, search), `group` (members, search), `field`, `issue-type`, `issue-link-type`, `priority`, `status`, `resolution`, `filter` (get, favourite), `dashboard`, `raw` |
| `agile` | `board` (list, get, configuration, sprints, issues, backlog, epics), `sprint` (get, issues), `epic` (get, issues), `raw` |
| `confluence` | `space` (list, get, content), `content` (list, get, children, descendants, attachments, comments, labels, history), `search`, `user` (current, get), `group` (list, members), `raw` |
| `bitbucket` | `application-properties`, `project`, `repo`, `branch` (list, default), `tag`, `commit` (list, get, changes), `pull-request` (list, get, activities, changes, commits), `file` (list, browse, content), `user`, `group`, `raw` |

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### How the tests work

| Where | What |
|-------|------|
| `src/**` unit tests | Credentials parsing, URL and query building for every command, pagination logic, error messages. |
| `tests/mock_harness.rs` | Tests the OpenAPI mock server itself against small fixture specs in `tests/fixtures/`. |
| `tests/cli.rs` | Runs the real binary against hand-built wiremock servers: auth, the config file, `--all` for each paging style, `raw`, text output, errors, and that only `GET` is ever sent. |
| `tests/{jira,agile,confluence,bitbucket}.rs` | Runs every command against a mock server built from the product's OpenAPI spec in `specs/`. |

The mock server (`tests/support/mod.rs`) reads an OpenAPI 3 or Swagger 2 file
and registers a route for every `GET` operation. Each route:

- requires the test user's Basic auth header
- requires the operation's mandatory query parameters
- answers with the spec's example, or a value generated from the response schema

Missing or wrong credentials get `401`, and paths that aren't in the spec get
`404`. A command passes only if it hits a real endpoint and prints exactly
what the spec says it returns.

The specs are Jira 9.12.0 and 10.0, Confluence 10.2.15 and Bitbucket 10.2. See
[`specs/README.md`](specs/README.md) for their sources and how to refresh them.

## Suggested future products

- **Jira Service Management**: service desks, queues, requests, customers,
  organizations (`/rest/servicedeskapi`).
- **Assets (formerly Insight)**: object schemas, objects, AQL search
  (`/rest/insight/1.0` or `/rest/assets/1.0`).
- **Bamboo**: plans, build results, deployments (`/rest/api/latest`). Atlassian
  publishes no OpenAPI spec for it, so its commands would need a hand-written
  spec to test against.
- **Crowd**: users, groups, memberships, directories (`/rest/usermanagement/1`).
- **Fisheye / Crucible**: repositories, changesets, code reviews
  (`/rest-service`, `/rest-service-fe`).
- **Bitbucket Mesh / Confluence Team Calendars / Jira Portfolio (Advanced
  Roadmaps)**: smaller APIs, worth adding when needed.

## Possible next steps

- Write commands (create issue, comment, approve pull request, ...), behind an
  explicit flag.
- Personal access token (Bearer) auth, which every DC product now supports.
- Named profiles for several servers of the same product.
- Field selection or table output.
