mod config;
mod context;
mod gitlab;
mod models;

use std::error::Error;
use std::fs;
use std::io::Error as IoError;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;
use reqwest::Client;

use config::{Config, ExportMode};
use context::build_multi_export;
use gitlab::{fetch_all_comments, fetch_work_item};
use models::ExportWorkItem;

#[derive(Parser, Debug)]
#[command(
    name = "gitlab-workitem-exporter",
    version,
    about = "Export GitLab Work Items and their comments to JSON",
    long_about = "Export GitLab Work Items and their comments to JSON.\n\n\
        Single Work Item export (--iid): exports one Work Item and ALL of its \
        comments, using the original JSON schema.\n\n\
        Multi Work Item export (--iids, or GITLAB_WORK_ITEM_IIDS when neither \
        --iid nor --iids is given): exports multiple Work Items from the same \
        GitLab project into one JSON file. Each Work Item includes only its \
        most recent non-system comments (GITLAB_RECENT_COMMENTS_LIMIT, \
        default 10).\n\n\
        --iid and --iids are mutually exclusive."
)]
struct Args {
    /// GitLab project path, e.g. your-group/your-project.
    /// Falls back to GITLAB_PROJECT from the environment or `.env` file
    /// when omitted; an explicit --project always takes priority. Used by
    /// both single and multi Work Item export.
    #[arg(long, env = "GITLAB_PROJECT")]
    project: String,

    /// Export a single GitLab Work Item and ALL of its comments (existing
    /// behavior, unchanged JSON schema). Mutually exclusive with --iids.
    #[arg(long)]
    iid: Option<u64>,

    /// Export multiple GitLab Work Items into one JSON file: a comma
    /// separated list of IIDs, e.g. --iids 23,24,25. Each Work Item includes
    /// only its most recent non-system comments. Mutually exclusive with
    /// --iid. Falls back to GITLAB_WORK_ITEM_IIDS when neither --iid nor
    /// --iids is given.
    #[arg(long)]
    iids: Option<String>,

    /// Output JSON file path. Defaults to workitem-<iid>.json for a single
    /// export, or workitems-context.json for a multi export.
    #[arg(long)]
    output: Option<PathBuf>,
}

fn default_output_dir() -> Result<PathBuf, Box<dyn Error>> {
    let profile_output = Command::new("cmd.exe")
        .args(["/C", "echo", "%USERPROFILE%"])
        .output()?;

    if !profile_output.status.success() {
        return Err(IoError::other("Failed to get Windows user profile").into());
    }

    let windows_profile = String::from_utf8(profile_output.stdout)?;

    let windows_downloads = format!(r"{}\Downloads", windows_profile.trim());

    let wslpath_output = Command::new("wslpath")
        .args(["-u", &windows_downloads])
        .output()?;

    if !wslpath_output.status.success() {
        return Err(IoError::other("Failed to convert Windows Downloads path to WSL path").into());
    }

    let downloads_path = String::from_utf8(wslpath_output.stdout)?;

    Ok(PathBuf::from(downloads_path.trim()))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    config::load_dotenv();

    let args = Args::parse();

    // Resolved before the token check so CLI argument errors (e.g. --iid
    // and --iids given together) are reported on their own, rather than
    // being masked by an unrelated missing-token error.
    let mode = config::resolve_export_mode(args.iid, args.iids.as_deref())?;

    let config = Config::from_env()?;

    let client = Client::new();

    match mode {
        ExportMode::Single(iid) => {
            let work_item =
                fetch_work_item(&client, &config.token, &config.base_url, &args.project, iid)
                    .await?;

            let comments =
                fetch_all_comments(&client, &config.token, &config.base_url, &args.project, iid)
                    .await?;

            let export_work_item = ExportWorkItem {
                id: work_item.id,
                iid: work_item.iid,
                title: work_item.title,
                description: work_item.description,
                state: work_item.state,
                comments,
            };

            let pretty_json = serde_json::to_string_pretty(&export_work_item)?;

            let output_path = match args.output {
                Some(path) => path,
                None => default_output_dir()?.join(format!("workitem-{iid}.json")),
            };

            fs::write(&output_path, &pretty_json)?;

            println!("Saved to {}", output_path.display());
        }
        ExportMode::Multiple(iids) => {
            let recent_comments_limit = config::recent_comments_limit_from_env()?;

            let export = build_multi_export(
                &client,
                &config.token,
                &config.base_url,
                &args.project,
                &iids,
                recent_comments_limit,
            )
            .await?;

            let pretty_json = serde_json::to_string_pretty(&export)?;

            let output_path = match args.output {
                Some(path) => path,
                None => default_output_dir()?.join("workitems-context.json"),
            };

            fs::write(&output_path, &pretty_json)?;

            println!("Saved to {}", output_path.display());
        }
    }

    Ok(())
}
