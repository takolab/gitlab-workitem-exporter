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
        .stdout(predicate::str::contains("Export a GitLab Work Item"));
}

#[test]
fn missing_iid_fails() {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.args(["--project", "example-group/example-project"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--iid"));
}

#[test]
fn missing_project_fails() {
    let mut cmd = Command::cargo_bin(BIN_NAME).expect("binary should exist");

    cmd.args(["--iid", "30"]);

    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("--project"));
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
                                "state": "OPEN"
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
                                    "gid://gitlab/WorkItem/197173799",
                                "iid": "30",
                                "title":
                                    "Example Work Item Title",
                                "description":
                                    "Test description",
                                "state": "OPEN"
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
                "id": 3670758947u64,
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
                "id": 3670759000u64,
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
                "id": 3670759100u64,
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
            "gid://gitlab/WorkItem/197173799",
        "iid": "30",
        "title": "Example Work Item Title",
        "description": "Test description",
        "state": "OPEN",
        "comments": [
            {
                "id": 3670758947u64,
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
                "id": 3670759000u64,
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
                "id": 3670759100u64,
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
                                    "gid://gitlab/WorkItem/197173799",
                                "iid": "30",
                                "title": "Pagination Test",
                                "description":
                                    "Test description",
                                "state": "OPEN"
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
