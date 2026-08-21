use std::error::Error;
use std::io::Error as IoError;

use reqwest::Client;
use serde_json::json;

use crate::models::{Comment, GraphqlResponse, WorkItem};

fn parse_work_item_response(response: GraphqlResponse) -> Result<WorkItem, Box<dyn Error>> {
    if let Some(errors) = &response.errors {
        for error in errors {
            eprintln!("GraphQL error: {}", error.message);
        }

        return Err(IoError::other("GitLab GraphQL request failed").into());
    }

    let data = response
        .data
        .ok_or_else(|| IoError::other("GraphQL response did not contain data"))?;

    let project = data
        .project
        .ok_or_else(|| IoError::other("GitLab project not found or access denied"))?;

    let work_item = project
        .work_items
        .nodes
        .into_iter()
        .next()
        .ok_or_else(|| IoError::other("Work Item not found"))?;

    Ok(work_item)
}

pub async fn fetch_work_item(
    client: &Client,
    token: &str,
    base_url: &str,
    project: &str,
    iid: u64,
) -> Result<WorkItem, Box<dyn Error>> {
    let query = r#"
        query($project: ID!, $iid: String!) {
            project(fullPath: $project) {
                workItems(iid: $iid) {
                    nodes {
                        id
                        iid
                        title
                        description
                        state
                    }
                }
            }
        }
    "#;

    let body = json!({
        "query": query,
        "variables": {
            "project": project,
            "iid": iid.to_string()
        }
    });

    let graphql_url = format!("{}/api/graphql", base_url.trim_end_matches('/'));

    let response = client
        .post(&graphql_url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?;

    let graphql_response: GraphqlResponse = response.json().await?;

    parse_work_item_response(graphql_response)
}

pub async fn fetch_all_comments(
    client: &Client,
    token: &str,
    base_url: &str,
    project: &str,
    iid: u64,
) -> Result<Vec<Comment>, Box<dyn Error>> {
    const PER_PAGE: usize = 100;

    let encoded_project = urlencoding::encode(project);

    let mut all_comments = Vec::new();
    let mut page = 1;

    loop {
        let url = format!(
            "{}/api/v4/projects/{}/issues/{}/notes\
            ?sort=asc\
            &order_by=created_at\
            &activity_filter=only_comments\
            &per_page={}\
            &page={}",
            base_url.trim_end_matches('/'),
            encoded_project,
            iid,
            PER_PAGE,
            page
        );

        let mut comments: Vec<Comment> = client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let comment_count = comments.len();

        all_comments.append(&mut comments);

        if comment_count < PER_PAGE {
            break;
        }

        page += 1;
    }

    Ok(all_comments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_work_item() -> WorkItem {
        WorkItem {
            id: "gid://gitlab/WorkItem/123456789".to_string(),
            iid: "30".to_string(),
            title: "Test Work Item".to_string(),
            description: Some("Test description".to_string()),
            state: "OPEN".to_string(),
        }
    }

    #[test]
    fn parses_valid_work_item_response() {
        let response = GraphqlResponse {
            data: Some(crate::models::Data {
                project: Some(crate::models::Project {
                    work_items: crate::models::WorkItems {
                        nodes: vec![sample_work_item()],
                    },
                }),
            }),
            errors: None,
        };

        let work_item = parse_work_item_response(response).expect("response should parse");

        assert_eq!(work_item.iid, "30");
        assert_eq!(work_item.title, "Test Work Item");
    }

    #[test]
    fn returns_error_when_project_is_null() {
        let response = GraphqlResponse {
            data: Some(crate::models::Data { project: None }),
            errors: None,
        };

        let error = parse_work_item_response(response).expect_err("missing project should fail");

        assert_eq!(
            error.to_string(),
            "GitLab project not found or access denied"
        );
    }

    #[test]
    fn returns_error_when_work_item_is_missing() {
        let response = GraphqlResponse {
            data: Some(crate::models::Data {
                project: Some(crate::models::Project {
                    work_items: crate::models::WorkItems { nodes: vec![] },
                }),
            }),
            errors: None,
        };

        let error = parse_work_item_response(response).expect_err("missing Work Item should fail");

        assert_eq!(error.to_string(), "Work Item not found");
    }
}
