# GitLab Work Item Exporter

[![CI](https://github.com/takolab/gitlab-workitem-exporter/actions/workflows/ci.yml/badge.svg)](https://github.com/takolab/gitlab-workitem-exporter/actions/workflows/ci.yml)

A small Rust CLI application that exports GitLab Work Items and their comments to JSON. It supports two modes: exporting a single Work Item with all of its comments, and exporting multiple selected GitLab Work Items from one GitLab project into a single Work Item context snapshot for external analysis tools.

This project was created as a hands-on Rust learning project, covering HTTP APIs, GraphQL, REST, JSON serialization/deserialization, CLI argument parsing, error handling, pagination, testing, and basic Rust project organization.

**Status: experimental.** This is a personal learning project. The current implementation focuses on the use case described in [Current Scope](#current-scope): exporting either one Work Item, or several selected Work Items from a single GitLab project, as plain JSON for external review. It does not perform any AI summarization, prioritization, or task extraction; see [Roadmap](#roadmap) for ideas not yet implemented.

## Features

* Fetches a single GitLab Work Item using the GitLab GraphQL API
* Fetches multiple selected GitLab Work Items from the same GitLab project and combines them into one Work Item context snapshot (multi-Work Item export)
* Fetches comments associated with each Work Item using the GitLab Issues REST Notes API (see [Supported Work Item types](#supported-work-item-types))
* Fetches all comments across multiple REST API pages
* Single export: combines the Work Item and *all* of its comments into one JSON file (unchanged, original behavior)
* Multi export: combines multiple Work Items into one JSON file, each with only its most recent non-system comments
* Supports selecting a GitLab project, and one or more Work Item IIDs, from CLI arguments
* Supports a `GITLAB_WORK_ITEM_IIDS` environment fallback for multi export when no CLI IID is given
* Supports a custom output path with `--output`
* When running under WSL without `--output`, writes the JSON file to the Windows Downloads folder
* Handles HTTP and GraphQL errors, and reports which Work Item IID failed on a partial failure
* Includes unit and integration tests
* Includes mocked GitLab API tests without requiring network access

## Example Output

### Single Work Item export

```json
{
  "id": "gid://gitlab/WorkItem/123456789",
  "iid": "30",
  "title": "Example Work Item Title",
  "description": "Work Item description...",
  "state": "OPEN",
  "comments": [
    {
      "id": 987654321,
      "body": "Comment body...",
      "created_at": "2026-08-13T10:00:00.000Z",
      "system": false,
      "author": {
        "name": "Example User",
        "username": "example-user"
      }
    }
  ]
}
```

This is the original, unchanged single Work Item export schema: it always includes *all* comments for the Work Item.

### Multi Work Item export

```json
{
  "schema_version": "1.0",
  "generated_at": "2026-08-21T09:00:00Z",
  "source": {
    "gitlab_base_url": "https://gitlab.com",
    "project": "example-group/example-project",
    "work_item_iids": [101, 102, 103],
    "recent_comments_limit": 10
  },
  "work_items": [
    {
      "id": "gid://gitlab/WorkItem/123456789",
      "iid": 101,
      "title": "Example Application",
      "description": "Current goals, tasks, and notes...",
      "state": "OPEN",
      "created_at": "2026-08-01T10:00:00Z",
      "updated_at": "2026-08-20T14:00:00Z",
      "web_url": "https://gitlab.com/example-group/example-project/-/work_items/101",
      "total_comment_count": 14,
      "comments_truncated": true,
      "recent_comments": [
        {
          "id": 987654321,
          "body": "Example progress update",
          "created_at": "2026-08-20T10:00:00Z",
          "system": false,
          "author": {
            "name": "Example User",
            "username": "example-user"
          }
        }
      ]
    }
  ]
}
```

Notes on the multi-Work Item export schema:

* `work_items` is ordered exactly as the requested IIDs (from `--iids` or `GITLAB_WORK_ITEM_IIDS`), not by GitLab's own ordering.
* `total_comment_count` counts all *non-system* comments for the Work Item, regardless of how many are included in `recent_comments`.
* `comments_truncated` is `true` when `total_comment_count` is greater than `recent_comments_limit`.
* `recent_comments` holds only the most recent non-system comments (`recent_comments_limit` per Work Item, default 10), ordered oldest to newest.
* `description` and comment `body` fields always hold the original text from GitLab. Nothing is summarized, prioritized, or extracted — that is left to whatever tool reads the JSON.
* Field order in the JSON is not part of the schema contract; do not write code or tests that depend on it.

## Requirements

* Rust
* Cargo
* GitLab account
* GitLab Fine-grained Personal Access Token
* WSL is required for the current automatic Windows Downloads folder detection

Check your Rust installation:

```bash
rustc --version
cargo --version
```

## GitLab Personal Access Token

Create a GitLab Fine-grained Personal Access Token that can access the target project.

The token used during development required the following **read-only** permissions:

* `User: Read`
* `Project: Read`
* `Work Item: Read`

This application only ever reads from GitLab: it never creates, updates, or deletes Work Items, comments, or anything else. Scope the token to read access only.

Do not store the token directly in the source code.

### Option 1: `.env` file (recommended for local development)

Copy the example file and fill in your token:

```bash
cp .env.example .env
```

Edit `.env` and set `GITLAB_TOKEN` (and optionally `GITLAB_PROJECT`, see [Setting a default project](#setting-a-default-project)):

```dotenv
GITLAB_TOKEN=your-token
GITLAB_PROJECT=your-group/your-project
GITLAB_WORK_ITEM_IIDS=101,102,103
GITLAB_RECENT_COMMENTS_LIMIT=10
```

`GITLAB_WORK_ITEM_IIDS` and `GITLAB_RECENT_COMMENTS_LIMIT` are only used by multi Work Item export; see [Exporting multiple Work Items](#exporting-multiple-work-items).

`.env` is listed in `.gitignore` and is never committed. The application loads it automatically on startup if present. A value already set in your shell environment always takes priority over `.env`.

### Option 2: shell environment variable

```bash
export GITLAB_TOKEN='your-token'
```

You can confirm that the environment variable exists without printing the token:

```bash
if [ -n "$GITLAB_TOKEN" ]; then
    echo "GITLAB_TOKEN is set"
else
    echo "GITLAB_TOKEN is NOT set"
fi
```

If `GITLAB_TOKEN` is missing or empty, the application exits with a clear error message and never prints the token value.

## Build

Development build:

```bash
cargo build
```

Release build:

```bash
cargo build --release
```

The release binary is created at:

```text
target/release/gitlab-workitem-exporter
```

## Usage

Show help:

```bash
./target/release/gitlab-workitem-exporter --help
```

The arguments are:

```text
--project <PROJECT>
    GitLab project path.
    Falls back to GITLAB_PROJECT from the environment or `.env` file
    when omitted. An explicit --project always overrides it. Used by
    both single and multi Work Item export.

--iid <IID>
    Export a single Work Item and ALL of its comments (original
    behavior, unchanged JSON schema). Mutually exclusive with --iids.

--iids <IIDS>
    Export multiple Work Items into one JSON file: a comma-separated
    list of IIDs, e.g. --iids 23,24,25. Each Work Item includes only
    its most recent non-system comments. Mutually exclusive with --iid.
    Falls back to GITLAB_WORK_ITEM_IIDS when neither --iid nor --iids
    is given.

--output <OUTPUT>
    Output JSON file path. Defaults to workitem-<iid>.json for a
    single export, or workitems-context.json for a multi export.
```

Specifying both `--iid` and `--iids` at the same time is an error.

### Exporting a single Work Item

```bash
./target/release/gitlab-workitem-exporter \
  --project example-group/example-project \
  --iid 30
```

This exports one Work Item and *all* of its comments, using the original JSON schema (see [Example Output](#example-output)). This behavior and schema are unchanged from earlier versions.

### Exporting multiple Work Items

Use `--iids` to combine several Work Items from the *same* GitLab project into one JSON file:

```bash
./target/release/gitlab-workitem-exporter \
  --project example-group/example-project \
  --iids 23,24,25,26 \
  --output ./workitems-context.json
```

* IIDs are parsed as a comma-separated list of positive integers.
* Surrounding whitespace around each entry is trimmed (`--iids "23, 24, 25"` works).
* Empty entries are ignored.
* Duplicate IIDs are removed, keeping the first occurrence.
* The exported `work_items` array preserves the order the IIDs were given in.
* An invalid entry (not a positive integer) fails with an error naming the offending value.

This is still a single GitLab project, multiple Work Items export — it does not fetch across multiple GitLab projects.

#### `GITLAB_WORK_ITEM_IIDS` fallback

When neither `--iid` nor `--iids` is given on the command line, the application falls back to `GITLAB_WORK_ITEM_IIDS` from `.env` or the environment and performs a multi Work Item export:

```dotenv
GITLAB_WORK_ITEM_IIDS=101,102,103
```

```bash
./target/release/gitlab-workitem-exporter --output ./workitems-context.json
```

An explicit `--iid` always ignores `GITLAB_WORK_ITEM_IIDS` (single export wins outright). An explicit `--iids` always takes priority over `GITLAB_WORK_ITEM_IIDS`. If none of `--iid`, `--iids`, or `GITLAB_WORK_ITEM_IIDS` provide an IID, the application exits with a clear error.

#### Limiting comments per Work Item

Multi Work Item export includes only each Work Item's most recent non-system comments, not the full comment history. The number of comments kept per Work Item is controlled by `GITLAB_RECENT_COMMENTS_LIMIT` (default `10`, must be a positive integer):

```dotenv
GITLAB_RECENT_COMMENTS_LIMIT=10
```

This setting, and `GITLAB_WORK_ITEM_IIDS`, are only read when a multi Work Item export is actually selected — an invalid or missing value in either never affects an explicit single Work Item `--iid` export.

For each Work Item, the exported JSON also reports `total_comment_count` (all non-system comments, regardless of how many are included) and `comments_truncated` (`true` when there were more non-system comments than `recent_comments_limit`). Single Work Item export is unaffected by this limit and always includes every comment.

### Setting a default project

If you mostly work with a single GitLab project, set `GITLAB_PROJECT` in `.env` so you can omit `--project`. This applies to both single and multi Work Item export:

```dotenv
GITLAB_PROJECT=your-group/your-project
```

```bash
./target/release/gitlab-workitem-exporter --iid 30
```

An explicit `--project` on the command line always takes priority over `GITLAB_PROJECT`.

### Custom output path

Use `--output` to explicitly specify the output file, for either export mode:

```bash
./target/release/gitlab-workitem-exporter \
  --project example-group/example-project \
  --iid 30 \
  --output ./workitem-30.json
```

```bash
./target/release/gitlab-workitem-exporter \
  --project example-group/example-project \
  --iids 23,24,25,26 \
  --output ./workitems-context.json
```

### Default output path

When `--output` is omitted, the application detects the Windows user profile from WSL and writes the file to the Windows Downloads folder. The same directory detection is used for both export modes; only the default file name differs.

For a single Work Item export of Work Item `30`, the output file name is:

```text
workitem-30.json
```

For a multi Work Item export, the output file name is:

```text
workitems-context.json
```

For example:

```text
C:\Users\<username>\Downloads\workitem-30.json
C:\Users\<username>\Downloads\workitems-context.json
```

From WSL, this appears similar to:

```text
/mnt/c/Users/<username>/Downloads/workitem-30.json
/mnt/c/Users/<username>/Downloads/workitems-context.json
```

## Development

Run the application through Cargo:

```bash
cargo run -- \
  --project example-group/example-project \
  --iid 30
```

With an explicit output file:

```bash
cargo run -- \
  --project example-group/example-project \
  --iid 30 \
  --output ./test.json
```

Multi Work Item export:

```bash
cargo run -- \
  --project example-group/example-project \
  --iids 23,24,25 \
  --output ./workitems-context.json
```

## Project Structure

```text
gitlab-workitem-exporter/
├── Cargo.toml
├── .env.example
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── gitlab.rs
│   ├── models.rs
│   └── context.rs
└── tests/
    └── cli.rs
```

### `main.rs`

Responsible for the overall CLI workflow:

* Parse CLI arguments
* Load configuration (`.env` file plus environment variables)
* Resolve the export mode (single vs. multi, see `config.rs`)
* Create the HTTP client
* For single export: fetch the Work Item and all of its comments, combine, serialize
* For multi export: build the aggregated export via `context.rs`, serialize
* Determine the output path
* Write the JSON file

### `config.rs`

Responsible for configuration, environment loading, and CLI mode resolution:

* Load a local `.env` file if present, without overriding variables already set in the process environment
* Read and validate `GITLAB_TOKEN`
* Read `GITLAB_BASE_URL`, defaulting to `https://gitlab.com`
* Return a clear error when `GITLAB_TOKEN` is missing or empty, without ever including the token value
* Resolve whether the run is a single or multi Work Item export from `--iid`, `--iids`, and `GITLAB_WORK_ITEM_IIDS` (see [`GITLAB_WORK_ITEM_IIDS` fallback](#gitlab_work_item_iids-fallback))
* Parse and validate comma-separated Work Item IID lists (whitespace trimming, empty-entry skipping, deduplication, and clear errors on invalid entries)
* Read and validate `GITLAB_RECENT_COMMENTS_LIMIT`, only when a multi Work Item export is selected

### `gitlab.rs`

Responsible for communication with GitLab:

* Fetch a Work Item through GraphQL (shared by both single and multi export)
* Parse the GraphQL response
* Fetch comments through the REST Notes API (shared by both single and multi export)
* Handle comment pagination
* Handle HTTP and GraphQL errors

### `models.rs`

Contains the Rust data models used for:

* GitLab GraphQL responses
* GitLab REST Note responses
* Exported single Work Item JSON
* Exported multi Work Item JSON (Work Item context snapshot)

### `context.rs`

Responsible for multi Work Item aggregation:

* Fetch every requested Work Item and its comments, in the requested order
* Filter out system comments and keep only the most recent `GITLAB_RECENT_COMMENTS_LIMIT` per Work Item, oldest to newest
* Compute `total_comment_count` and `comments_truncated` per Work Item
* Assemble the final multi Work Item export document
* Fail with an error naming the Work Item IID if any fetch fails, without writing a partial file

## API Flow

The application currently uses both GitLab GraphQL and REST APIs.

```text
GitLab GraphQL API
        │
        └── Work Item
            ├── id
            ├── iid
            ├── title
            ├── description
            ├── state
            ├── createdAt
            ├── updatedAt
            └── webUrl

GitLab REST Notes API
        │
        └── Comments
            ├── id
            ├── body
            ├── created_at
            ├── system
            └── author

                ↓

    ┌───────────┴────────────┐
    │                         │
ExportWorkItem          MultiExport
(single, all comments)  (multiple Work Items,
    │                    recent comments only)
    ▼                         ▼
workitem-<iid>.json   workitems-context.json
```

`createdAt`, `updatedAt`, and `webUrl` are fetched for every Work Item but are only included in the multi Work Item export JSON; the single Work Item export JSON schema is unchanged.

The Work Item itself is retrieved using:

```text
POST /api/graphql
```

Comments are retrieved using the Issues REST Notes API:

```text
GET /api/v4/projects/:project/issues/:iid/notes
```

Because comments are fetched through the *Issues* Notes API, this currently works reliably for issue-type Work Items. Other Work Item types (e.g. Epics, Tasks) may not expose their comments through this endpoint and are unverified; see [Current Scope](#current-scope) and [Supported Work Item types](#supported-work-item-types). This applies to both single and multi Work Item export.

The comments API is requested with:

```text
activity_filter=only_comments
sort=asc
order_by=created_at
per_page=100
```

Additional pages are fetched until all comments have been retrieved. Single Work Item export keeps every comment retrieved this way. Multi Work Item export additionally filters out system comments and keeps only the most recent `GITLAB_RECENT_COMMENTS_LIMIT` per Work Item.

## Tests

Run all tests:

```bash
cargo test
```

The project currently contains unit tests and CLI integration tests.

### Unit tests

Unit tests cover:

* GraphQL JSON deserialization
* REST Note JSON deserialization
* Export JSON serialization (single and multi Work Item export)
* Valid Work Item response parsing
* Missing project handling
* Missing Work Item handling
* Work Item IID list parsing: whitespace trimming, empty entries, deduplication, invalid entries
* Export mode resolution: `--iid` vs. `--iids` vs. `GITLAB_WORK_ITEM_IIDS`, and their priority order
* `GITLAB_RECENT_COMMENTS_LIMIT` parsing and validation

### CLI integration tests

Integration tests cover:

* `--help`
* Missing `--iid`/`--iids` (and no `GITLAB_WORK_ITEM_IIDS` fallback)
* `--iid` and `--iids` given together (mutually exclusive error)
* Missing `--project` (and no `GITLAB_PROJECT` fallback)
* Unknown CLI arguments
* Missing or empty `GITLAB_TOKEN`
* Invalid output path (file write failure)
* `GITLAB_PROJECT` used as a fallback when `--project` is omitted
* Explicit `--project` overriding `GITLAB_PROJECT`
* Full single Work Item + comments export using mocked GitLab APIs
* Comment pagination across multiple REST API pages
* Multi Work Item export combining several Work Items into one JSON file, in the requested IID order
* Multi Work Item export filtering system comments, limiting to the most recent N, and ordering them oldest to newest
* `total_comment_count` and `comments_truncated` in multi Work Item export
* Multi Work Item export failing clearly (and writing no output file) when one Work Item IID cannot be fetched
* `GITLAB_WORK_ITEM_IIDS` used as a fallback when neither `--iid` nor `--iids` is given
* Explicit `--iid` ignoring an invalid `GITLAB_WORK_ITEM_IIDS` and `GITLAB_RECENT_COMMENTS_LIMIT`
* Explicit `--iids` overriding `GITLAB_WORK_ITEM_IIDS`
* Whitespace trimming and deduplication in `--iids`
* Invalid `--iids` values failing with a clear error
* The GitLab token never appearing in exported JSON

The mocked API tests do not access GitLab.com and do not require a real GitLab token.

Run only CLI integration tests:

```bash
cargo test --test cli
```

## Formatting and Static Analysis

Check formatting:

```bash
cargo fmt --check
```

Run Clippy:

```bash
cargo clippy --all-targets --all-features
```

Treat all Clippy warnings as errors:

```bash
cargo clippy --all-targets --all-features -- -D warnings
```

Before committing changes, the recommended local verification is:

```bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo build --release
```

## Continuous Integration

A [GitHub Actions workflow](.github/workflows/ci.yml) runs on every push and pull request to `main`, executing the same four checks listed above. It does not require `GITLAB_TOKEN` or any other secret, since all tests use mocked GitLab API responses.

## Testing With a Mock GitLab Server

For integration testing, the application supports overriding the GitLab base URL with:

```text
GITLAB_BASE_URL
```

Normal users do not need to set this variable.

When it is not set, the application uses:

```text
https://gitlab.com
```

Integration tests set `GITLAB_BASE_URL` to a local WireMock server so the complete CLI workflow can be tested without making requests to GitLab.com.

## Security Notes

* Never commit `.env` or any file containing a real `GITLAB_TOKEN`. `.env` is listed in `.gitignore`.
* Use a GitLab Fine-grained Personal Access Token scoped to the minimum, **read-only** permissions your use case needs (see [GitLab Personal Access Token](#gitlab-personal-access-token)). The application never writes back to GitLab.
* Exported JSON files can contain Work Item titles, descriptions, and comment text, which may include sensitive or confidential information. A multi Work Item export can aggregate this across several Work Items in one file. Treat exported JSON as potentially sensitive: do not commit it to a public repository, and be mindful of where you upload it (e.g. to an external analysis tool).
* The application never logs or prints the token value, and the token is never included in the exported JSON. Error messages describe *that* the token is missing, never its contents.

## License

Licensed under the [MIT License](LICENSE).

## Current Scope

The current version intentionally focuses on two related use cases:

> Export one GitLab Work Item and all of its comments into one JSON file.
>
> Export several selected GitLab Work Items from one GitLab project — each with its most recent non-system comments — into one Work Item context snapshot JSON file, for manual upload to an external analysis tool.

It does not currently:

* Export across multiple GitLab projects in one invocation
* Export Work Item attachments
* Export every Work Item widget
* Summarize, prioritize, or extract tasks from Descriptions or comments (see [Roadmap](#roadmap))
* Automatically authenticate without `GITLAB_TOKEN`
* Provide native default Downloads folder detection outside WSL
* Install the binary system-wide

These can be added later if needed.

### Supported Work Item types

The Work Item itself is fetched through the generic GitLab GraphQL `workItems` query, but comments are fetched through the *Issues* REST Notes API (`/api/v4/projects/:project/issues/:iid/notes`). This means the exporter currently supports **issue-type GitLab Work Items whose comments are available through the Issues Notes API**. Other Work Item types (Epics, Tasks, and others) have not been verified and may not return comments correctly through this endpoint.

## Development Status

The first two implementation milestones are complete: single Work Item export, and multi-Work Item context snapshot export.

The application currently passes:

* Rust formatting checks
* Unit tests
* CLI integration tests
* Mock GitLab API integration tests
* Comment pagination tests
* Clippy with warnings treated as errors
* Release build, and mock/real GitLab Work Item export in both single and multi export modes

## Roadmap

This exporter is intended as a small, general-purpose building block for a GitLab-based task or project tracking workflow: generate a plain JSON export of one or more GitLab Work Items — including their raw Descriptions and recent comments — and let an external tool (e.g. an LLM-based analysis tool) interpret it. The exporter itself intentionally stays "dumb": it fetches and structures data, but does not interpret it.

Multi Work Item export (Work Item context aggregation) is implemented today: given a GitLab project and a list of Work Item IIDs, `context.rs` builds a `MultiExport` combining `gitlab.rs` (the GitLab client) and `models.rs` (the JSON export types).

```text
GitLabClient (gitlab.rs)
    │
    ▼
WorkItemExporter (models.rs)
    │
    ├── single export ──────► workitem-<iid>.json
    │
    └── multi export
         via context.rs   ──► workitems-context.json
```

Ideas that are **explicitly out of scope** for this exporter, and are left to whatever external tool reads the exported JSON:

* AI-generated summaries or prioritization
* Task extraction from Descriptions or comments
* Markdown checklist parsing
* Per-date comment extraction
* Automatic relationships between Work Items
* Writing back to GitLab (creating or updating Work Items or comments)
* Aggregating across multiple GitLab projects
* A web UI, database, or scheduler/automatic runs
