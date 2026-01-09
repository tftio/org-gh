use crate::config::Config;
use crate::error::{Error, Result};
use crate::github::GitHubClient;
use crate::org::parse_file;
use crate::output::{format, Format, StatusOutput};
use crate::sync::SyncState;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to org file
    pub file: PathBuf,
}

pub async fn run(args: Args, output_format: Format) -> Result<()> {
    // Parse org file
    let org_file = parse_file(&args.file)?;

    let repo = org_file.repo.as_ref().ok_or(Error::NoRepo)?;

    // Load sync state
    let state = SyncState::load(&args.file)?;

    // Load config and fetch GitHub data
    let config = Config::load()?;
    let token = config.github_token()?;
    let client = GitHubClient::new(&token, repo).await?;

    // Count items by status
    let synced_count = state.items.len();
    let pending_creates: Vec<String> = org_file
        .items
        .iter()
        .filter(|item| item.gh_issue.is_none())
        .map(|item| item.title.clone())
        .collect();

    // Fetch remote state and compare
    let gh_issues = client.fetch_issues().await?;

    // Find local changes (items that differ from base state)
    let mut local_changes = Vec::new();
    let mut remote_changes = Vec::new();

    for item in &org_file.items {
        if let Some(issue_num) = item.gh_issue {
            if let Some(base) = state.items.get(&issue_num) {
                // Check if org changed from base
                if item.title != base.title {
                    local_changes.push(format!("#{}: title changed", issue_num));
                }
                let org_state = if item.state.is_open() {
                    "open"
                } else {
                    "closed"
                };
                if org_state != base.state {
                    local_changes.push(format!(
                        "#{}: marked {}",
                        issue_num,
                        org_state.to_uppercase()
                    ));
                }
            }

            // Check if GitHub changed from base
            if let Some(gh) = gh_issues.iter().find(|i| i.number == issue_num) {
                if let Some(base) = state.items.get(&issue_num) {
                    if gh.title != base.title {
                        remote_changes.push(format!("#{}: title changed", issue_num));
                    }
                    let gh_state = if gh.state.is_open() { "open" } else { "closed" };
                    if gh_state != base.state {
                        remote_changes.push(format!("#{}: now {}", issue_num, gh_state));
                    }
                }
            }
        }
    }

    let output = StatusOutput {
        file: args.file.display().to_string(),
        repo: repo.clone(),
        last_sync: state
            .last_sync
            .map(|t| t.format("%Y-%m-%d %H:%M:%S UTC").to_string()),
        synced_count,
        pending_creates,
        local_changes,
        remote_changes,
    };

    print!("{}", format(&output, output_format));
    Ok(())
}
