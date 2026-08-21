# GitLab Work Item Exporter

A small Rust CLI application that exports a GitLab Work Item and its comments to a single JSON file.

This project was created as a hands-on Rust learning project, covering HTTP APIs, GraphQL, REST, JSON serialization/deserialization, CLI argument parsing, error handling, pagination, testing, and basic Rust project organization.

**Status: experimental.** This is a personal learning project. The current implementation focuses on a narrow use case (see [Current Scope](#current-scope)) and does not yet implement the broader "LifeOS context snapshot" idea described in [Roadmap](#roadmap).

## Features

* Fetches a single GitLab Work Item using the GitLab GraphQL API
* Fetches comments associated with the Work Item using the GitLab REST Notes API
* Fetches all comments across multiple REST API pages
* Combines the Work Item and its comments into a single JSON file
* Supports selecting a GitLab project and Work Item IID from CLI arguments
* Supports a custom output path with `--output`
* When running under WSL without `--output`, writes the JSON file to the Windows Downloads folder
* Handles HTTP and GraphQL errors
* Includes unit and integration tests
* Includes mocked GitLab API tests without requiring network access

## Example Output

```json
{
  "id": "gid://gitlab/WorkItem/197173799",
  "iid": "30",
  "title": "Example Work Item Title",
  "description": "Work Item description...",
  "state": "OPEN",
  "comments": [
    {
      "id": 3670758947,
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

The token used during development required the following read permissions:

* `User: Read`
* `Project: Read`
* `Work Item: Read`

Do not store the token directly in the source code.

### Option 1: `.env` file (recommended for local development)

Copy the example file and fill in your token:

```bash
cp .env.example .env
```

Edit `.env` and set `GITLAB_TOKEN`:

```dotenv
GITLAB_TOKEN=your-token
```

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

Export a Work Item:

```bash
./target/release/gitlab-workitem-exporter \
  --project example-group/example-project \
  --iid 30
```

The required arguments are:

```text
--project <PROJECT>
    GitLab project path

--iid <IID>
    GitLab Work Item IID
```

### Custom output path

Use `--output` to explicitly specify the output file:

```bash
./target/release/gitlab-workitem-exporter \
  --project example-group/example-project \
  --iid 30 \
  --output ./workitem-30.json
```

### Default output path

When `--output` is omitted, the application detects the Windows user profile from WSL and writes the file to the Windows Downloads folder.

For Work Item `30`, the output file name is:

```text
workitem-30.json
```

For example:

```text
C:\Users\<username>\Downloads\workitem-30.json
```

From WSL, this appears similar to:

```text
/mnt/c/Users/<username>/Downloads/workitem-30.json
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

## Project Structure

```text
gitlab-workitem-exporter/
├── Cargo.toml
├── .env.example
├── src/
│   ├── main.rs
│   ├── config.rs
│   ├── gitlab.rs
│   └── models.rs
└── tests/
    └── cli.rs
```

### `main.rs`

Responsible for the overall CLI workflow:

* Parse CLI arguments
* Load configuration (`.env` file plus environment variables)
* Create the HTTP client
* Fetch the Work Item
* Fetch comments
* Combine the data
* Serialize the result
* Determine the output path
* Write the JSON file

### `config.rs`

Responsible for configuration and environment loading:

* Load a local `.env` file if present, without overriding variables already set in the process environment
* Read and validate `GITLAB_TOKEN`
* Read `GITLAB_BASE_URL`, defaulting to `https://gitlab.com`
* Return a clear error when `GITLAB_TOKEN` is missing or empty, without ever including the token value

### `gitlab.rs`

Responsible for communication with GitLab:

* Fetch a Work Item through GraphQL
* Parse the GraphQL response
* Fetch comments through the REST Notes API
* Handle comment pagination
* Handle HTTP and GraphQL errors

### `models.rs`

Contains the Rust data models used for:

* GitLab GraphQL responses
* GitLab REST Note responses
* Exported Work Item JSON

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
            └── state

GitLab REST Notes API
        │
        └── Comments
            ├── id
            ├── body
            ├── created_at
            ├── system
            └── author

                ↓

        ExportWorkItem

                ↓

        workitem-<iid>.json
```

The Work Item itself is retrieved using:

```text
POST /api/graphql
```

Comments are retrieved using the REST Notes API:

```text
GET /api/v4/projects/:project/issues/:iid/notes
```

The comments API is requested with:

```text
activity_filter=only_comments
sort=asc
order_by=created_at
per_page=100
```

Additional pages are fetched until all comments have been retrieved.

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
* Export JSON serialization
* Valid Work Item response parsing
* Missing project handling
* Missing Work Item handling

### CLI integration tests

Integration tests cover:

* `--help`
* Missing `--iid`
* Missing `--project`
* Unknown CLI arguments
* Full Work Item + comments export using mocked GitLab APIs
* Comment pagination across multiple REST API pages

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
* Use a GitLab Fine-grained Personal Access Token scoped to the minimum permissions your use case needs (see [GitLab Personal Access Token](#gitlab-personal-access-token)).
* Exported JSON files can contain Work Item titles, descriptions, and comment text. Treat exported JSON as potentially sensitive and avoid committing it to a public repository.
* The application never logs or prints the token value. Error messages describe *that* the token is missing, never its contents.

## License

Licensed under the [MIT License](LICENSE).

## Current Scope

The current version intentionally focuses on a small use case:

> Export one GitLab Work Item and all of its comments into one JSON file.

It does not currently:

* Export multiple Work Items in one invocation
* Export Work Item attachments
* Export every Work Item widget
* Automatically authenticate without `GITLAB_TOKEN`
* Provide native default Downloads folder detection outside WSL
* Install the binary system-wide

These can be added later if needed.

## Development Status

The first implementation milestone is complete.

The application currently passes:

* Rust formatting checks
* Unit tests
* CLI integration tests
* Mock GitLab API integration tests
* Comment pagination tests
* Clippy with warnings treated as errors
* Release build and real GitLab Work Item export

## Roadmap

This exporter is intended as a foundation for a broader tool that builds a JSON "context snapshot" of a personal GitLab-based task system (referred to as LifeOS), including:

* Today's Must / Should / Bonus tasks
* Per-project progress
* Open tasks, blockers, and next actions
* Related Work Items, comments, and evidence URLs

This future work is **not implemented yet**. The current codebase is intentionally organized so it can grow along these lines without a major rewrite:

```text
GitLabClient
    │
    ▼
WorkItemExporter
    │
    ▼
JSON output
```

Today, `gitlab.rs` and `models.rs` play the role of the GitLab client and JSON export logic; a dedicated aggregation layer for LifeOS-specific context (combining multiple Work Items, projects, and comments into one snapshot) would sit on top of them in a future change.
