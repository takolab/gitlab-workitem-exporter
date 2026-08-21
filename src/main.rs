mod config;
mod gitlab;
mod models;

use std::error::Error;
use std::fs;
use std::io::Error as IoError;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;
use reqwest::Client;

use config::Config;
use gitlab::{fetch_all_comments, fetch_work_item};
use models::ExportWorkItem;

#[derive(Parser, Debug)]
#[command(
    name = "gitlab-workitem-exporter",
    version,
    about = "Export a GitLab Work Item and its comments to JSON"
)]
struct Args {
    /// GitLab project path, e.g. your-group/your-project
    #[arg(long)]
    project: String,

    /// GitLab Work Item IID
    #[arg(long)]
    iid: u64,

    /// Output JSON file path
    #[arg(long)]
    output: Option<PathBuf>,
}

fn default_output_path(iid: u64) -> Result<PathBuf, Box<dyn Error>> {
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

    Ok(PathBuf::from(downloads_path.trim()).join(format!("workitem-{iid}.json")))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    config::load_dotenv();

    let args = Args::parse();

    let config = Config::from_env()?;

    let client = Client::new();

    let work_item = fetch_work_item(
        &client,
        &config.token,
        &config.base_url,
        &args.project,
        args.iid,
    )
    .await?;

    let comments = fetch_all_comments(
        &client,
        &config.token,
        &config.base_url,
        &args.project,
        args.iid,
    )
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
        None => default_output_path(args.iid)?,
    };

    fs::write(&output_path, &pretty_json)?;

    println!("Saved to {}", output_path.display());

    Ok(())
}
