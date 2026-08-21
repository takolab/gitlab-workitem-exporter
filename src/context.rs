use std::error::Error;
use std::fmt;

use reqwest::Client;

use crate::gitlab::{fetch_all_comments, fetch_work_item};
use crate::models::{Comment, MultiExport, MultiExportSource, MultiExportWorkItem};

const SCHEMA_VERSION: &str = "1.0";

/// Wraps a fetch failure with the Work Item IID it happened for, so a
/// multi-Work-Item export fails with a message that names the offending
/// IID instead of a bare underlying error.
pub struct WorkItemFetchError {
    iid: u64,
    source: Box<dyn Error>,
}

impl fmt::Display for WorkItemFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Failed to fetch Work Item {}: {}", self.iid, self.source)
    }
}

impl fmt::Debug for WorkItemFetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl Error for WorkItemFetchError {}

/// Fetches every requested Work Item (in the given order) along with its
/// most recent non-system comments, and assembles them into a single
/// [`MultiExport`]. Nothing is returned until every Work Item has been
/// fetched successfully, so a failure partway through never produces a
/// partial result.
pub async fn build_multi_export(
    client: &Client,
    token: &str,
    base_url: &str,
    project: &str,
    iids: &[u64],
    recent_comments_limit: usize,
) -> Result<MultiExport, Box<dyn Error>> {
    let mut work_items = Vec::with_capacity(iids.len());

    for &iid in iids {
        let work_item = fetch_work_item(client, token, base_url, project, iid)
            .await
            .map_err(|source| WorkItemFetchError { iid, source })?;

        let all_comments = fetch_all_comments(client, token, base_url, project, iid)
            .await
            .map_err(|source| WorkItemFetchError { iid, source })?;

        let non_system_comments: Vec<Comment> = all_comments
            .into_iter()
            .filter(|comment| !comment.system)
            .collect();

        let total_comment_count = non_system_comments.len();
        let comments_truncated = total_comment_count > recent_comments_limit;

        // `non_system_comments` is already oldest-to-newest (the REST API is
        // requested with sort=asc), so skipping the earliest entries keeps
        // the selected comments in the required oldest-to-newest order.
        let skip = total_comment_count.saturating_sub(recent_comments_limit);
        let recent_comments: Vec<Comment> = non_system_comments.into_iter().skip(skip).collect();

        work_items.push(MultiExportWorkItem {
            id: work_item.id,
            iid,
            title: work_item.title,
            description: work_item.description,
            state: work_item.state,
            created_at: work_item.created_at,
            updated_at: work_item.updated_at,
            web_url: work_item.web_url,
            total_comment_count,
            comments_truncated,
            recent_comments,
        });
    }

    Ok(MultiExport {
        schema_version: SCHEMA_VERSION.to_string(),
        generated_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        source: MultiExportSource {
            gitlab_base_url: base_url.to_string(),
            project: project.to_string(),
            work_item_iids: iids.to_vec(),
            recent_comments_limit,
        },
        work_items,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_item_fetch_error_names_the_iid() {
        let error = WorkItemFetchError {
            iid: 42,
            source: std::io::Error::other("boom").into(),
        };

        assert_eq!(error.to_string(), "Failed to fetch Work Item 42: boom");
    }
}
