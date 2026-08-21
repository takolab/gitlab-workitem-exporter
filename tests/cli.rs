use assert_cmd::Command;
use predicates::prelude::*;

use std::fs;

use serde_json::json;
use tempfile::tempdir;
use wiremock::matchers::{bearer_token, body_partial_json, method, path, path_regex, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BIN_NAME: &str = "gitlab-workitem-exporter";

#[test]
fn help_succeeds() {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.arg("--help");

    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Export GitLab Work Items"))
        .stdout(predicate::str::contains("Single Work Item export"))
        .stdout(predicate::str::contains("Multi Work Item export"))
        .stdout(predicate::str::contains("mutually exclusive"))
        .stdout(predicate::str::contains("--iids"))
        .stdout(predicate::str::contains("GITLAB_WORK_ITEM_IIDS"));
}

#[test]
fn missing_iid_fails() {
    // Run from an isolated temp directory with no `.env` and no
    // GITLAB_WORK_ITEM_IIDS, so mode resolution has no IID from any source
    // and this test does not depend on the developer's local `.env`.
    let temp_dir = tempdir().expect("temporary directory should be created");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.current_dir(temp_dir.path())
        .env("GITLAB_TOKEN", "test-token")
        .env_remove("GITLAB_WORK_ITEM_IIDS")
        .args(["--project", "example-group/example-project"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--iid"))
        .stderr(predicate::str::contains("--iids"))
        .stderr(predicate::str::contains("GITLAB_WORK_ITEM_IIDS"));
}

#[test]
fn missing_project_fails() {
    // Run from an isolated temp directory with GITLAB_PROJECT unset so this
    // test does not depend on (or read) the developer's local `.env`,
    // which may set a default GITLAB_PROJECT.
    let temp_dir = tempdir().expect("temporary directory should be created");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.current_dir(temp_dir.path())
        .env_remove("GITLAB_PROJECT")
        .args(["--iid", "30"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--project"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uses_gitlab_project_env_var_when_flag_is_omitted() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .and(body_partial_json(json!({
            "variables": {
                "project": "example-group/example-project",
                "iid": "30"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "project": {
                    "workItems": {
                        "nodes": [
                            {
                                "id": "gid://gitlab/WorkItem/1",
                                "iid": "30",
                                "title": "Example Work Item Title",
                                "description": "Test description",
                                "state": "OPEN",
                                "createdAt": "2026-08-01T10:00:00Z",
                                "updatedAt": "2026-08-20T14:00:00Z",
                                "webUrl": "https://gitlab.example.com/example-group/example-project/-/work_items/30"
                            }
                        ]
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitem-30.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .env("GITLAB_PROJECT", "example-group/example-project")
        .args(["--iid", "30", "--output"])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    assert_eq!(actual["iid"], "30");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_project_flag_overrides_gitlab_project_env_var() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .and(body_partial_json(json!({
            "variables": {
                "project": "cli-group/cli-project",
                "iid": "30"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "project": {
                    "workItems": {
                        "nodes": [
                            {
                                "id": "gid://gitlab/WorkItem/1",
                                "iid": "30",
                                "title": "Example Work Item Title",
                                "description": "Test description",
                                "state": "OPEN",
                                "createdAt": "2026-08-01T10:00:00Z",
                                "updatedAt": "2026-08-20T14:00:00Z",
                                "webUrl": "https://gitlab.example.com/example-group/example-project/-/work_items/30"
                            }
                        ]
                    }
                }
            }
        })))
        .expect(1)
        .named("GraphQL Work Item request")
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitem-30.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .env("GITLAB_PROJECT", "env-group/env-project")
        .args([
            "--project",
            "cli-group/cli-project",
            "--iid",
            "30",
            "--output",
        ])
        .arg(&output_path);

    // The mocked GraphQL request only matches "cli-group/cli-project", so
    // success here proves the CLI flag won over the environment variable.
    cmd.assert().success();
}

#[test]
fn unknown_argument_fails() {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.args([
        "--project",
        "example-group/example-project",
        "--iid",
        "30",
        "--unknown",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("unexpected argument"));
}

#[test]
fn missing_token_fails_with_clear_error() {
    // Run from an isolated temp directory with no `.env` file so this test
    // does not depend on (or read) the developer's local `.env`, and never
    // reaches the network: the token check happens before any HTTP call.
    let temp_dir = tempdir().expect("temporary directory should be created");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.current_dir(temp_dir.path())
        .env_remove("GITLAB_TOKEN")
        .args(["--project", "example-group/example-project", "--iid", "30"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("GITLAB_TOKEN is not set"))
        .stderr(predicate::str::contains("your-token").not());
}

#[test]
fn empty_token_fails_with_clear_error() {
    let temp_dir = tempdir().expect("temporary directory should be created");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.current_dir(temp_dir.path())
        .env("GITLAB_TOKEN", "")
        .args(["--project", "example-group/example-project", "--iid", "30"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("GITLAB_TOKEN is not set"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fails_clearly_when_output_path_is_invalid() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "project": {
                    "workItems": {
                        "nodes": [
                            {
                                "id": "gid://gitlab/WorkItem/1",
                                "iid": "30",
                                "title": "Example Work Item Title",
                                "description": "Test description",
                                "state": "OPEN",
                                "createdAt": "2026-08-01T10:00:00Z",
                                "updatedAt": "2026-08-20T14:00:00Z",
                                "webUrl": "https://gitlab.example.com/example-group/example-project/-/work_items/30"
                            }
                        ]
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");

    // The parent directory does not exist, so writing the output file fails.
    let output_path = temp_dir
        .path()
        .join("no-such-directory")
        .join("workitem-30.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .args([
            "--project",
            "example-group/example-project",
            "--iid",
            "30",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().failure();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exports_work_item_with_comments_to_json() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .and(body_partial_json(json!({
            "variables": {
                "project": "example-group/example-project",
                "iid": "30"
            }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "project": {
                    "workItems": {
                        "nodes": [
                            {
                                "id":
                                    "gid://gitlab/WorkItem/123456789",
                                "iid": "30",
                                "title":
                                    "Example Work Item Title",
                                "description":
                                    "Test description",
                                "state": "OPEN",
                                "createdAt": "2026-08-01T10:00:00Z",
                                "updatedAt": "2026-08-20T14:00:00Z",
                                "webUrl": "https://gitlab.example.com/example-group/example-project/-/work_items/30"
                            }
                        ]
                    }
                }
            }
        })))
        .expect(1)
        .named("GraphQL Work Item request")
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .and(query_param("sort", "asc"))
        .and(query_param("order_by", "created_at"))
        .and(query_param("activity_filter", "only_comments"))
        .and(query_param("per_page", "100"))
        .and(query_param("page", "1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": 987654321u64,
                "body": "First comment",
                "created_at":
                    "2026-08-11T10:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username":
                        "example-user"
                }
            },
            {
                "id": 987654322u64,
                "body": "Second comment",
                "created_at":
                    "2026-08-11T11:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username":
                        "example-user"
                }
            },
            {
                "id": 987654323u64,
                "body": "Third comment",
                "created_at":
                    "2026-08-11T12:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username":
                        "example-user"
                }
            }
        ])))
        .expect(1)
        .named("REST Notes request")
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");

    let output_path = temp_dir.path().join("workitem-30.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .args([
            "--project",
            "example-group/example-project",
            "--iid",
            "30",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    let expected = json!({
        "id":
            "gid://gitlab/WorkItem/123456789",
        "iid": "30",
        "title": "Example Work Item Title",
        "description": "Test description",
        "state": "OPEN",
        "comments": [
            {
                "id": 987654321u64,
                "body": "First comment",
                "created_at":
                    "2026-08-11T10:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username":
                        "example-user"
                }
            },
            {
                "id": 987654322u64,
                "body": "Second comment",
                "created_at":
                    "2026-08-11T11:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username":
                        "example-user"
                }
            },
            {
                "id": 987654323u64,
                "body": "Third comment",
                "created_at":
                    "2026-08-11T12:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username":
                        "example-user"
                }
            }
        ]
    });

    assert_eq!(actual, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exports_all_comments_across_multiple_pages() {
    let server = MockServer::start().await;

    // The Work Item itself
    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": {
                "project": {
                    "workItems": {
                        "nodes": [
                            {
                                "id":
                                    "gid://gitlab/WorkItem/123456789",
                                "iid": "30",
                                "title": "Pagination Test",
                                "description":
                                    "Test description",
                                "state": "OPEN",
                                "createdAt": "2026-08-01T10:00:00Z",
                                "updatedAt": "2026-08-20T14:00:00Z",
                                "webUrl": "https://gitlab.example.com/example-group/example-project/-/work_items/30"
                            }
                        ]
                    }
                }
            }
        })))
        .expect(1)
        .named("GraphQL Work Item request")
        .mount(&server)
        .await;

    // Generate 100 comments for page 1
    let first_page_comments: Vec<_> = (1..=100)
        .map(|id| {
            json!({
                "id": id,
                "body": format!("Comment {id}"),
                "created_at":
                    "2026-08-11T10:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Test User",
                    "username": "test-user"
                }
            })
        })
        .collect();

    // Comments page 1
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .and(query_param("page", "1"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(first_page_comments))
        .expect(1)
        .named("Comments page 1")
        .mount(&server)
        .await;

    // Comments page 2
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .and(query_param("page", "2"))
        .and(query_param("per_page", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([
            {
                "id": 101,
                "body": "Comment 101",
                "created_at":
                    "2026-08-11T11:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Test User",
                    "username": "test-user"
                }
            }
        ])))
        .expect(1)
        .named("Comments page 2")
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");

    let output_path = temp_dir.path().join("workitem-30.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .args([
            "--project",
            "example-group/example-project",
            "--iid",
            "30",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    let comments = actual["comments"]
        .as_array()
        .expect("comments should be an array");

    assert_eq!(comments.len(), 101);

    assert_eq!(comments[0]["body"], "Comment 1");

    assert_eq!(comments[99]["body"], "Comment 100");

    assert_eq!(comments[100]["body"], "Comment 101");
}

// --- Multi Work Item export ---

fn work_item_node(iid: &str, title: &str) -> serde_json::Value {
    json!({
        "id": format!("gid://gitlab/WorkItem/{iid}"),
        "iid": iid,
        "title": title,
        "description": format!("Description for {title}"),
        "state": "OPEN",
        "createdAt": "2026-08-01T10:00:00Z",
        "updatedAt": "2026-08-20T14:00:00Z",
        "webUrl": format!(
            "https://gitlab.example.com/example-group/example-project/-/work_items/{iid}"
        )
    })
}

fn graphql_work_items_response(nodes: Vec<serde_json::Value>) -> serde_json::Value {
    json!({
        "data": {
            "project": {
                "workItems": {
                    "nodes": nodes
                }
            }
        }
    })
}

fn note(id: u64, body: &str, created_at: &str, system: bool) -> serde_json::Value {
    json!({
        "id": id,
        "body": body,
        "created_at": created_at,
        "system": system,
        "author": {
            "name": "Example User",
            "username": "example-user"
        }
    })
}

async fn mount_work_item_mock(server: &MockServer, iid: &str, node: serde_json::Value) {
    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .and(body_partial_json(json!({
            "variables": { "iid": iid }
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(graphql_work_items_response(vec![node])),
        )
        .mount(server)
        .await;
}

async fn mount_comments_mock(server: &MockServer, iid: &str, comments: Vec<serde_json::Value>) {
    Mock::given(method("GET"))
        .and(path_regex(format!(
            r"^/api/v4/projects/.+/issues/{iid}/notes$"
        )))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(comments))
        .mount(server)
        .await;
}

#[test]
fn iid_and_iids_together_fails() {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.args([
        "--project",
        "example-group/example-project",
        "--iid",
        "30",
        "--iids",
        "23,24",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("mutually exclusive"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exports_multiple_work_items_to_one_json_file() {
    let server = MockServer::start().await;

    mount_work_item_mock(&server, "23", work_item_node("23", "First Work Item")).await;
    mount_work_item_mock(&server, "24", work_item_node("24", "Second Work Item")).await;

    mount_comments_mock(
        &server,
        "23",
        vec![note(
            1,
            "Progress on first",
            "2026-08-20T10:00:00.000Z",
            false,
        )],
    )
    .await;
    mount_comments_mock(&server, "24", vec![]).await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitems-context.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .args([
            "--project",
            "example-group/example-project",
            "--iids",
            "23,24",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    assert_eq!(actual["schema_version"], "1.0");
    assert!(actual["generated_at"].as_str().is_some());
    assert_eq!(actual["source"]["project"], "example-group/example-project");
    assert_eq!(actual["source"]["work_item_iids"], json!([23, 24]));
    assert_eq!(actual["source"]["recent_comments_limit"], 10);

    let work_items = actual["work_items"]
        .as_array()
        .expect("work_items should be an array");

    assert_eq!(work_items.len(), 2);

    // Order follows the requested --iids order.
    assert_eq!(work_items[0]["iid"], 23);
    assert_eq!(work_items[0]["title"], "First Work Item");
    assert_eq!(work_items[0]["total_comment_count"], 1);
    assert_eq!(work_items[0]["comments_truncated"], false);
    assert_eq!(
        work_items[0]["recent_comments"][0]["body"],
        "Progress on first"
    );

    assert_eq!(work_items[1]["iid"], 24);
    assert_eq!(work_items[1]["title"], "Second Work Item");
    assert_eq!(work_items[1]["total_comment_count"], 0);
    assert_eq!(work_items[1]["comments_truncated"], false);
    assert_eq!(work_items[1]["recent_comments"], json!([]));

    // The token must never appear anywhere in the exported JSON.
    assert!(!output.contains("test-token"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_export_filters_system_comments_limits_and_orders_recent_comments() {
    let server = MockServer::start().await;

    mount_work_item_mock(&server, "23", work_item_node("23", "Busy Work Item")).await;

    // 5 non-system comments (oldest to newest) plus 2 system comments mixed
    // in; GITLAB_RECENT_COMMENTS_LIMIT below is set to 2, so only the two
    // newest non-system comments should be kept, oldest first.
    let comments = vec![
        note(1, "Comment 1", "2026-08-11T10:00:00.000Z", false),
        note(2, "System event", "2026-08-11T10:30:00.000Z", true),
        note(3, "Comment 2", "2026-08-11T11:00:00.000Z", false),
        note(4, "Comment 3", "2026-08-11T12:00:00.000Z", false),
        note(5, "System event 2", "2026-08-11T12:30:00.000Z", true),
        note(6, "Comment 4", "2026-08-11T13:00:00.000Z", false),
        note(7, "Comment 5", "2026-08-11T14:00:00.000Z", false),
    ];

    mount_comments_mock(&server, "23", comments).await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitems-context.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .env("GITLAB_RECENT_COMMENTS_LIMIT", "2")
        .args([
            "--project",
            "example-group/example-project",
            "--iids",
            "23",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    assert_eq!(actual["source"]["recent_comments_limit"], 2);

    let work_item = &actual["work_items"][0];

    // 5 non-system comments total, system comments excluded from the count.
    assert_eq!(work_item["total_comment_count"], 5);
    assert_eq!(work_item["comments_truncated"], true);

    let recent = work_item["recent_comments"]
        .as_array()
        .expect("recent_comments should be an array");

    assert_eq!(recent.len(), 2);
    // Oldest to newest among the selected comments.
    assert_eq!(recent[0]["body"], "Comment 4");
    assert_eq!(recent[1]["body"], "Comment 5");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_export_reports_which_iid_failed_and_writes_no_file() {
    let server = MockServer::start().await;

    mount_work_item_mock(&server, "23", work_item_node("23", "Existing Work Item")).await;
    mount_comments_mock(&server, "23", vec![]).await;

    // Work Item 24 does not exist: GraphQL returns no nodes.
    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .and(body_partial_json(json!({
            "variables": { "iid": "24" }
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(graphql_work_items_response(vec![])))
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitems-context.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .args([
            "--project",
            "example-group/example-project",
            "--iids",
            "23,24",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("24"));

    assert!(
        !output_path.exists(),
        "no output file should be written when a Work Item fetch fails"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_export_falls_back_to_gitlab_work_item_iids_env_var() {
    let server = MockServer::start().await;

    mount_work_item_mock(&server, "101", work_item_node("101", "Env Work Item")).await;
    mount_comments_mock(&server, "101", vec![]).await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitems-context.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.current_dir(temp_dir.path())
        .env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .env("GITLAB_WORK_ITEM_IIDS", "101")
        .args(["--project", "example-group/example-project", "--output"])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    assert_eq!(actual["work_items"][0]["iid"], 101);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_iid_ignores_invalid_gitlab_work_item_iids_env_var() {
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api/graphql"))
        .and(bearer_token("test-token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(graphql_work_items_response(vec![
                work_item_node("30", "Example Work Item Title"),
            ])),
        )
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path_regex(r"^/api/v4/projects/.+/issues/30/notes$"))
        .and(bearer_token("test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
        .mount(&server)
        .await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitem-30.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    // GITLAB_WORK_ITEM_IIDS is garbage, and GITLAB_RECENT_COMMENTS_LIMIT is
    // invalid too; neither should matter because --iid selects a single
    // export, which never reads either variable.
    cmd.current_dir(temp_dir.path())
        .env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .env("GITLAB_WORK_ITEM_IIDS", "not-a-valid-list")
        .env("GITLAB_RECENT_COMMENTS_LIMIT", "0")
        .args([
            "--project",
            "example-group/example-project",
            "--iid",
            "30",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    assert_eq!(actual["iid"], "30");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn explicit_iids_overrides_gitlab_work_item_iids_env_var() {
    let server = MockServer::start().await;

    mount_work_item_mock(&server, "23", work_item_node("23", "CLI Work Item")).await;
    mount_comments_mock(&server, "23", vec![]).await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitems-context.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.current_dir(temp_dir.path())
        .env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .env("GITLAB_WORK_ITEM_IIDS", "101,102")
        .args([
            "--project",
            "example-group/example-project",
            "--iids",
            "23",
            "--output",
        ])
        .arg(&output_path);

    // The mocked GraphQL request only matches iid "23", so success here
    // proves --iids won over GITLAB_WORK_ITEM_IIDS.
    cmd.assert().success();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_export_parses_whitespace_and_deduplicates_iids() {
    let server = MockServer::start().await;

    mount_work_item_mock(&server, "23", work_item_node("23", "First")).await;
    mount_comments_mock(&server, "23", vec![]).await;
    mount_work_item_mock(&server, "24", work_item_node("24", "Second")).await;
    mount_comments_mock(&server, "24", vec![]).await;

    let temp_dir = tempdir().expect("temporary directory should be created");
    let output_path = temp_dir.path().join("workitems-context.json");

    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token")
        .env("GITLAB_BASE_URL", server.uri())
        .args([
            "--project",
            "example-group/example-project",
            "--iids",
            " 23, 24,23,, 24 ",
            "--output",
        ])
        .arg(&output_path);

    cmd.assert().success();

    let output = fs::read_to_string(&output_path).expect("output JSON should exist");

    let actual: serde_json::Value =
        serde_json::from_str(&output).expect("output should be valid JSON");

    assert_eq!(actual["source"]["work_item_iids"], json!([23, 24]));

    let work_items = actual["work_items"]
        .as_array()
        .expect("work_items should be an array");

    assert_eq!(work_items.len(), 2);
}

#[test]
fn invalid_iids_value_fails_clearly() {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.env("GITLAB_TOKEN", "test-token").args([
        "--project",
        "example-group/example-project",
        "--iids",
        "23,not-a-number,25",
    ]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not-a-number"));
}
