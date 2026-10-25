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
| Bamboo       | `bamboo`     | `/rest/api/latest`  |

This first version only **reads**. The HTTP client has no method other than
`GET`, and a test checks that no other method is ever sent.

## Install

```sh
cargo install --path .
```

## Authentication

Only HTTP Basic auth is supported. Put your username and password on one line
in `~/.atlassian_credentials`:

```sh
printf '%s:%s\n' 'alice' 'my-password' > ~/.atlassian_credentials
chmod 600 ~/.atlassian_credentials
```

Everything after the first `:` is the password, so passwords may contain `:`.
The CLI warns if the file can be read by other users. Use
`--credentials-file <PATH>` to read a different file.

## Server URLs

Pass `--base-url`, or set one environment variable per product:

```sh
export JIRA_URL=https://jira.example.com          # also used by `agile`
export CONFLUENCE_URL=https://confluence.example.com
export BITBUCKET_URL=https://bitbucket.example.com
export BAMBOO_URL=https://bamboo.example.com
```

Include any context path, e.g. `https://example.com/jira`. For a server with a
private CA, the OS trust store is used; you can also pass `--ca-cert ca.pem`.

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
atlassian bamboo result latest PROJ-PLAN

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
parameters (`startAt`/`maxResults`, `start`/`limit`, `start-index`/`max-result`).
`--all` follows every page and prints the items as one JSON array. Bamboo
doesn't support `--all`.

## Commands

| Product | Resources |
|---------|-----------|
| `jira` | `server-info`, `myself`, `issue` (get, comments, worklogs, transitions, watchers, votes, remote-links), `search`, `project` (list, get, components, versions, statuses, roles), `user` (get, search), `group` (members, search), `field`, `issue-type`, `issue-link-type`, `priority`, `status`, `resolution`, `filter` (get, favourite), `dashboard`, `raw` |
| `agile` | `board` (list, get, configuration, sprints, issues, backlog, epics), `sprint` (get, issues), `epic` (get, issues), `raw` |
| `confluence` | `space` (list, get, content), `content` (list, get, children, descendants, attachments, comments, labels, history), `search`, `user` (current, get), `group` (list, members), `raw` |
| `bitbucket` | `application-properties`, `project`, `repo`, `branch` (list, default), `tag`, `commit` (list, get, changes), `pull-request` (list, get, activities, changes, commits), `file` (list, browse, content), `user`, `group`, `raw` |
| `bamboo` | `info`, `server`, `current-user`, `project`, `plan` (list, get, branches), `result` (list, get, latest), `deploy` (project list/get, environment-results), `queue`, `agent`, `raw` |

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
| `tests/cli.rs` | Runs the real binary against hand-built wiremock servers: auth, env vars, `--all` for each paging style, `raw`, text output, errors, and that only `GET` is ever sent. |
| `tests/{jira,agile,confluence,bitbucket,bamboo}.rs` | Runs every command against a mock server built from the product's OpenAPI spec in `specs/`. |

The mock server (`tests/support/mod.rs`) reads an OpenAPI 3 or Swagger 2 file
and registers a route for every `GET` operation. Each route:

- requires the test user's Basic auth header
- requires the operation's mandatory query parameters
- answers with the spec's example, or a value generated from the response schema

Missing or wrong credentials get `401`, and paths that aren't in the spec get
`404`. A command passes only if it hits a real endpoint and prints exactly
what the spec says it returns.

> **Note:** the official spec files aren't committed yet, so the five
> spec-driven tests fail until they are added. See
> [`specs/README.md`](specs/README.md) and run `scripts/fetch-specs.sh`.

## Suggested future products

- **Jira Service Management**: service desks, queues, requests, customers,
  organizations (`/rest/servicedeskapi`).
- **Assets (formerly Insight)**: object schemas, objects, AQL search
  (`/rest/insight/1.0` or `/rest/assets/1.0`).
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
