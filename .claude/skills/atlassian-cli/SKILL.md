---
name: atlassian-cli
description: Read data from Atlassian Data Center (self-hosted Jira, Jira Software, Confluence, Bitbucket) with the `atlassian` CLI. Use when asked to look up, search, list or summarise Jira issues, JQL results, projects, users, boards, sprints or epics; Confluence pages, spaces, CQL search results, comments, attachments or page history; or Bitbucket projects, repositories, branches, commits, pull requests or file contents. Read-only: it cannot create or change anything.
---

# Using the `atlassian` CLI

`atlassian` is a read-only client for Atlassian **Data Center** (self-hosted)
products. Every command sends HTTP GETs and prints JSON.

```
atlassian <product> <resource> <action> [args] [options]
```

| Product | Covers | REST API |
|---|---|---|
| `jira` | issues, JQL search, projects, users, groups, fields, filters, dashboards | `/rest/api/2` |
| `agile` | Jira Software boards, sprints, epics, backlog (alias `jira-software`) | `/rest/agile/1.0` |
| `confluence` | spaces, pages/blog posts, CQL search, comments, attachments, labels, history | `/rest/api` |
| `bitbucket` | projects, repos, branches, tags, commits, pull requests, files, users | `/rest/api/latest` |

**Full list of commands and options:** [reference.md](reference.md), generated
from the code. You can also run `atlassian <product> --help`, or `--help` at any
level.

## Rules

- **Read-only.** There are no create, update, delete, comment or transition
  commands. If asked to change something, say the CLI can't, and give the
  information needed to do it by hand. Never invent a command.
- **Boards, sprints and epics are under `agile`**, not `jira`. They use the
  `[jira]` config section.
- **`--all` for complete lists.** List commands return one page by default
  (`--start`, `--limit`). `--all` follows every page and prints a single
  **JSON array** of the items, without the page wrapper.
- **Pipe `--compact` into `jq`** to pick out fields:
  `atlassian jira search --jql '...' --all --compact | jq -r '.[] | "\(.key) \(.fields.summary)"'`
- **Narrow responses with `--fields` (Jira) or `--expand`.** Confluence omits
  page bodies unless asked: `--expand body.storage`.
- **Use `raw` for endpoints without a command:**
  `atlassian jira raw /rest/api/2/issue/ABC-1/editmeta -q expand=projects`.
  The path is relative to the base URL and includes the `/rest/...` prefix.
- **IDs:**
  - Jira issues use keys (`ABC-123`).
  - Confluence content uses numeric ids (the `pageId` in a page URL). Find
    them with `confluence search` or `content list --space KEY --title "..."`.
  - Bitbucket repos are `PROJECT REPO-SLUG`; personal projects are
    `~username`.

## Setup

Settings come from `~/.atlassian.toml` (or `--config PATH`):

```toml
username = "alice"          # default Basic-auth credentials
password = "secret"

[jira]                      # also used by `agile`
base_url = "https://jira.example.com"

[confluence]
base_url = "https://wiki.example.com/confluence"   # include the context path
# username/password/ca_cert can be overridden per section

[bitbucket]
base_url = "https://bitbucket.example.com"
```

`--base-url URL` and `--ca-cert PEM` override the file for one command.
**Don't print or echo the config file**: it contains a password.

## Worked tasks

| Task | Command(s) |
|---|---|
| Who am I logged in as? | `atlassian jira myself` · `atlassian confluence user current` |
| My open issues | `atlassian jira search --jql 'assignee = currentUser() AND resolution = Unresolved ORDER BY priority DESC' --fields summary,status,priority --all` |
| One issue in full, with history | `atlassian jira issue get ABC-123 --expand changelog,renderedFields` |
| Issues updated in the last week in a project | `atlassian jira search --jql 'project = ABC AND updated >= -7d' --fields summary,updated --all` |
| Comments on an issue | `atlassian jira issue comments ABC-123 --all` |
| What a project contains | `atlassian jira project get ABC` · `jira project components ABC` · `jira project versions ABC` |
| Find a user | `atlassian jira user search ali` |
| Find a board, then its active sprint's issues | `atlassian agile board list --project ABC` → `atlassian agile board sprints <boardId> --state active` → `atlassian agile sprint issues <sprintId> --fields summary,status --all` |
| A board's backlog | `atlassian agile board backlog <boardId> --all` |
| Pages edited in the last week in a space | `atlassian confluence search --cql 'space = DEV AND type = page AND lastmodified > now("-7d")' --all` |
| Read a page's content | `atlassian confluence content get <pageId> --expand body.storage,version` |
| A page by title | `atlassian confluence content list --space DEV --title "Release checklist" --expand version` |
| Child pages | `atlassian confluence content children <pageId> --all` |
| Who changed a page | `atlassian confluence content history <pageId> --expand contributors.publishers.users,previousVersion` |
| Repos in a project | `atlassian bitbucket repo list PROJ --all` |
| Open PRs into main | `atlassian bitbucket pull-request list PROJ repo --state OPEN --at refs/heads/main --all` |
| Review state of a PR | `atlassian bitbucket pull-request get PROJ repo 42` · `bitbucket pull-request activities PROJ repo 42 --all` |
| Files changed by a PR | `atlassian bitbucket pull-request changes PROJ repo 42 --all` |
| A file at a branch or commit | `atlassian bitbucket file content PROJ repo path/to/file --at refs/heads/main` (prints raw text, not JSON) |
| Recent commits touching a path | `atlassian bitbucket commit list PROJ repo --path src/ --limit 20` |

## Output shapes

Without `--all`, list commands print the server's page object. With `--all`
they print only the items as one array. Where the items are on a page:

| Product | Page fields | Items in |
|---|---|---|
| Jira search | `startAt`, `maxResults`, `total` | `issues` |
| Jira comments, worklogs | `startAt`, `maxResults`, `total` | `comments`, `worklogs` |
| Jira dashboards | `startAt`, `maxResults`, `total` | `dashboards` |
| Jira group members, agile | `startAt`, `maxResults`, `isLast` | `values` (agile issue lists: `issues`) |
| Confluence | `start`, `limit`, `size`, `_links.next` | `results` |
| Bitbucket | `start`, `limit`, `isLastPage`, `nextPageStart` | `values` |

Some endpoints return a plain array, e.g. `jira project list`,
`jira user search`, `jira field list` and most `list` lookups. `--all` leaves
those unchanged.

## Troubleshooting

| Error | Meaning and fix |
|---|---|
| `no base URL for <product>` | Add `base_url` to that section of `~/.atlassian.toml`, or pass `--base-url`. |
| `no username/password for <product>` | Set `username`/`password` at the top of the config or in the section. |
| `invalid config file ... unknown field` | Misspelled key, or an unsupported section (only `jira`, `confluence`, `bitbucket`). |
| `returned 401 Unauthorized` | Wrong credentials, or the account is locked or needs a CAPTCHA. Check by logging in to the web UI. |
| `returned 403` | The user can't see that resource. |
| `returned 404` | Wrong key or id, or a wrong base URL (e.g. a missing `/confluence` context path). |
| TLS or certificate errors | Set `ca_cert = "/path/ca.pem"` in the section, or pass `--ca-cert`. |

Exit status is 0 on success and 1 on any error. Errors go to stderr, prefixed
with `error:`.
