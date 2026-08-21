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
                                "id": "gid://gitlab/WorkItem/197173799",
                                "iid": "30",
                                "title": "Example Work Item Title",
                                "description": "Test description",
                                "state": "OPEN"
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

        assert_eq!(work_item.id, "gid://gitlab/WorkItem/197173799");
        assert_eq!(work_item.iid, "30");
        assert_eq!(work_item.title, "Example Work Item Title");
        assert_eq!(work_item.description.as_deref(), Some("Test description"));
        assert_eq!(work_item.state, "OPEN");
    }

    #[test]
    fn deserializes_rest_comments() {
        let input = r#"
        [
            {
                "id": 3670758947,
                "body": "First comment",
                "created_at": "2026-08-11T10:00:00.000Z",
                "system": false,
                "author": {
                    "name": "Example User",
                    "username": "example-user"
                }
            },
            {
                "id": 3670759000,
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

        assert_eq!(comments[0].id, 3670758947);
        assert_eq!(comments[0].body, "First comment");
        assert!(!comments[0].system);
        assert_eq!(comments[0].author.username, "example-user");

        assert_eq!(comments[1].body, "Second comment");
    }

    #[test]
    fn serializes_export_work_item() {
        let work_item = ExportWorkItem {
            id: "gid://gitlab/WorkItem/197173799".to_string(),
            iid: "30".to_string(),
            title: "Example Work Item Title".to_string(),
            description: Some("Test description".to_string()),
            state: "OPEN".to_string(),
            comments: vec![Comment {
                id: 3670758947,
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
            "id": "gid://gitlab/WorkItem/197173799",
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
                }
            ]
        });

        assert_eq!(actual, expected);
    }
}
