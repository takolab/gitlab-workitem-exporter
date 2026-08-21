use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct GraphqlResponse {
    pub data: Option<Data>,
    pub errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
pub struct GraphqlError {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct Data {
    pub project: Option<Project>,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    #[serde(rename = "workItems")]
    pub work_items: WorkItems,
}

#[derive(Debug, Deserialize)]
pub struct WorkItems {
    pub nodes: Vec<WorkItem>,
}

#[derive(Debug, Deserialize)]
pub struct WorkItem {
    pub id: String,
    pub iid: String,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    #[serde(rename = "webUrl")]
    pub web_url: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Author {
    pub name: String,
    pub username: String,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Comment {
    pub id: u64,
    pub body: String,
    pub created_at: String,
    pub system: bool,
    pub author: Author,
}

#[derive(Debug, Serialize)]
pub struct ExportWorkItem {
    pub id: String,
    pub iid: String,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub comments: Vec<Comment>,
}

/// The `source` block of a multi Work Item export: the GitLab project the
/// Work Items came from and the request parameters used to build the file.
#[derive(Debug, Serialize)]
pub struct MultiExportSource {
    pub gitlab_base_url: String,
    pub project: String,
    pub work_item_iids: Vec<u64>,
    pub recent_comments_limit: usize,
}

/// A single Work Item within a multi Work Item export, including only its
/// most recent non-system comments (see [`MultiExportSource::recent_comments_limit`]).
#[derive(Debug, Serialize)]
pub struct MultiExportWorkItem {
    pub id: String,
    pub iid: u64,
    pub title: String,
    pub description: Option<String>,
    pub state: String,
    pub created_at: String,
    pub updated_at: String,
    pub web_url: String,
    pub total_comment_count: usize,
    pub comments_truncated: bool,
    pub recent_comments: Vec<Comment>,
}

/// The top-level JSON document produced by a multi Work Item export.
#[derive(Debug, Serialize)]
pub struct MultiExport {
    pub schema_version: String,
    pub generated_at: String,
    pub source: MultiExportSource,
    pub work_items: Vec<MultiExportWorkItem>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_graphql_work_item_response() {
        let input = r#"
        {
            "data": {
                "project": {
                    "workItems": {
                        "nodes": [
                            {
                                "id": "gid://gitlab/WorkItem/123456789",
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
        }
        "#;

        let response: GraphqlResponse =
            serde_json::from_str(input).expect("GraphQL response should deserialize");

        assert!(response.errors.is_none());

        let data = response.data.expect("data should exist");

        let project = data.project.expect("project should exist");

        assert_eq!(project.work_items.nodes.len(), 1);

        let work_item = &project.work_items.nodes[0];

        assert_eq!(work_item.id, "gid://gitlab/WorkItem/123456789");
        assert_eq!(work_item.iid, "30");
        assert_eq!(work_item.title, "Example Work Item Title");
        assert_eq!(work_item.description.as_deref(), Some("Test description"));
        assert_eq!(work_item.state, "OPEN");
        assert_eq!(work_item.created_at, "2026-08-01T10:00:00Z");
        assert_eq!(work_item.updated_at, "2026-08-20T14:00:00Z");
        assert_eq!(
            work_item.web_url,
            "https://gitlab.example.com/example-group/example-project/-/work_items/30"
        );
    }

    #[test]
    fn deserializes_rest_comments() {
        let input = r#"
        [
            {
                "id": 987654321,
                "body": "First comment",
                "created_at": "2026-08-11T10:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username": "example-user"
                }
            },
            {
                "id": 987654322,
                "body": "Second comment",
                "created_at": "2026-08-11T11:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username": "example-user"
                }
            }
        ]
        "#;

        let comments: Vec<Comment> =
            serde_json::from_str(input).expect("Comments should deserialize");

        assert_eq!(comments.len(), 2);

        assert_eq!(comments[0].id, 987654321);
        assert_eq!(comments[0].body, "First comment");
        assert!(!comments[0].system);
        assert_eq!(comments[0].author.username, "example-user");

        assert_eq!(comments[1].body, "Second comment");
    }

    #[test]
    fn serializes_export_work_item() {
        let work_item = ExportWorkItem {
            id: "gid://gitlab/WorkItem/123456789".to_string(),
            iid: "30".to_string(),
            title: "Example Work Item Title".to_string(),
            description: Some("Test description".to_string()),
            state: "OPEN".to_string(),
            comments: vec![Comment {
                id: 987654321,
                body: "First comment".to_string(),
                created_at: "2026-08-11T10:00:00.000Z".to_string(),
                system: false,
                author: Author {
                    name: "Example User".to_string(),
                    username: "example-user".to_string(),
                },
            }],
        };

        let actual = serde_json::to_value(&work_item).expect("ExportWorkItem should serialize");

        let expected = json!({
            "id": "gid://gitlab/WorkItem/123456789",
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
                }
            ]
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn serializes_multi_export() {
        let export = MultiExport {
            schema_version: "1.0".to_string(),
            generated_at: "2026-08-21T09:00:00Z".to_string(),
            source: MultiExportSource {
                gitlab_base_url: "https://gitlab.example.com".to_string(),
                project: "example-group/example-project".to_string(),
                work_item_iids: vec![101, 102],
                recent_comments_limit: 10,
            },
            work_items: vec![MultiExportWorkItem {
                id: "gid://gitlab/WorkItem/123456789".to_string(),
                iid: 101,
                title: "Example Application".to_string(),
                description: Some("Current goals, tasks, and notes...".to_string()),
                state: "OPEN".to_string(),
                created_at: "2026-08-01T10:00:00Z".to_string(),
                updated_at: "2026-08-20T14:00:00Z".to_string(),
                web_url:
                    "https://gitlab.example.com/example-group/example-project/-/work_items/101"
                        .to_string(),
                total_comment_count: 14,
                comments_truncated: true,
                recent_comments: vec![Comment {
                    id: 987654321,
                    body: "Example progress update".to_string(),
                    created_at: "2026-08-20T10:00:00Z".to_string(),
                    system: false,
                    author: Author {
                        name: "Example User".to_string(),
                        username: "example-user".to_string(),
                    },
                }],
            }],
        };

        let actual = serde_json::to_value(&export).expect("MultiExport should serialize");

        let expected = json!({
            "schema_version": "1.0",
            "generated_at": "2026-08-21T09:00:00Z",
            "source": {
                "gitlab_base_url": "https://gitlab.example.com",
                "project": "example-group/example-project",
                "work_item_iids": [101, 102],
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
                    "web_url": "https://gitlab.example.com/example-group/example-project/-/work_items/101",
                    "total_comment_count": 14,
                    "comments_truncated": true,
                    "recent_comments": [
                        {
                            "id": 987654321u64,
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
        });

        assert_eq!(actual, expected);
    }
}
