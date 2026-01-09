use crate::config::Config;
use crate::error::{Error, Result};
use crate::github::model::{CreateIssueRequest, GhIssueState, UpdateIssueRequest};
use crate::github::GitHubClient;
use crate::org::writer::set_property;
use crate::org::{parse_file, write_file};
use crate::output::{format, Format, PushItem, PushOutput};
use crate::sync::state::hash_body;
use crate::sync::SyncState;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to org file
    pub file: PathBuf,

    /// Force push - org wins all conflicts
    #[arg(long, short)]
    pub force: bool,

    /// Dry run - show what would happen
    #[arg(long)]
    pub dry_run: bool,

    /// Verbose output
    #[arg(long, short)]
    pub verbose: bool,
}

pub async fn run(args: Args, output_format: Format) -> Result<()> {
    // Parse org file
    let mut org_file = parse_file(&args.file)?;
    let repo = org_file.repo.clone().ok_or(Error::NoRepo)?;

    // Load sync state
    let mut state = SyncState::load(&args.file)?;
    if state.repo.is_empty() {
        state.repo = repo.clone();
    }

    // Load config and create client
    let config = Config::load()?;
    let token = config.github_token()?;
    let client = GitHubClient::new(&token, &repo).await?;

    let is_human = output_format == Format::Human;

    if args.dry_run && is_human {
        println!("Dry run - no changes will be made\n");
    }

    let mut created_items = Vec::new();
    let mut updated_items = Vec::new();
    let mut skipped = 0;

    // Process each org item
    for item in &org_file.items {
        if let Some(issue_num) = item.gh_issue {
            // Existing linked item - check if we need to update
            if let Some(base) = state.items.get(&issue_num) {
                let title_changed = item.title != base.title;
                let body_changed = hash_body(&item.body) != base.body_hash;
                let state_changed = {
                    let org_state = if item.state.is_open() {
                        "open"
                    } else {
                        "closed"
                    };
                    org_state != base.state
                };

                if title_changed || body_changed || state_changed {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("Update #{}: {}", issue_num, item.title);
                        if title_changed {
                            println!("  - title: {} -> {}", base.title, item.title);
                        }
                        if body_changed {
                            println!("  - body changed");
                        }
                        if state_changed {
                            let new_state = if item.state.is_open() {
                                "open"
                            } else {
                                "closed"
                            };
                            println!("  - state: {} -> {}", base.state, new_state);
                        }
                    }

                    if !args.dry_run {
                        let req = UpdateIssueRequest {
                            title: if title_changed {
                                Some(item.title.clone())
                            } else {
                                None
                            },
                            body: if body_changed {
                                Some(item.body.clone())
                            } else {
                                None
                            },
                            state: if state_changed {
                                Some(if item.state.is_open() {
                                    GhIssueState::Open
                                } else {
                                    GhIssueState::Closed
                                })
                            } else {
                                None
                            },
                            assignees: None,
                            labels: None,
                        };

                        let issue = client.update_issue(issue_num, req).await?;

                        // Update sync state
                        state.record_sync(
                            issue.number,
                            &item.id,
                            &issue.title,
                            issue.body.as_deref().unwrap_or(""),
                            if issue.state.is_open() {
                                "open"
                            } else {
                                "closed"
                            },
                            &issue.assignees,
                            &issue.labels,
                            issue.updated_at,
                        );

                        updated_items.push(PushItem {
                            title: issue.title,
                            issue_number: issue.number,
                            url: issue.html_url,
                            action: "updated".to_string(),
                        });
                    }
                } else {
                    skipped += 1;
                    if is_human && args.verbose {
                        println!("Skip #{}: {} (no changes)", issue_num, item.title);
                    }
                }
            } else {
                // Linked but no base state - update state from current
                skipped += 1;
                if is_human && args.verbose {
                    println!("Skip #{}: {} (no base state)", issue_num, item.title);
                }
            }
        } else {
            // New item - try to match or create
            if is_human && (args.verbose || args.dry_run) {
                println!("Create: {}", item.title);
            }

            if !args.dry_run {
                // Try to find existing issue by title first
                let existing = client.find_by_title(&item.title).await?;

                let (issue, matched) = if let Some(existing) = existing {
                    if is_human {
                        println!("  Matched existing issue #{}", existing.number);
                    }
                    (existing, true)
                } else {
                    // Create new issue
                    let req = CreateIssueRequest {
                        title: item.title.clone(),
                        body: if item.body.is_empty() {
                            None
                        } else {
                            Some(item.body.clone())
                        },
                        assignees: item.assignees.clone(),
                        labels: item.labels.clone(),
                    };

                    (client.create_issue(req).await?, false)
                };

                // Update org file with issue number
                org_file.content = set_property(
                    &org_file.content,
                    item,
                    "GH_ISSUE",
                    &issue.number.to_string(),
                );
                org_file.content = set_property(&org_file.content, item, "GH_URL", &issue.html_url);

                // Update sync state
                state.record_sync(
                    issue.number,
                    &item.id,
                    &issue.title,
                    issue.body.as_deref().unwrap_or(""),
                    if issue.state.is_open() {
                        "open"
                    } else {
                        "closed"
                    },
                    &issue.assignees,
                    &issue.labels,
                    issue.updated_at,
                );

                created_items.push(PushItem {
                    title: issue.title,
                    issue_number: issue.number,
                    url: issue.html_url,
                    action: if matched { "matched" } else { "created" }.to_string(),
                });
            }
        }
    }

    // Save changes
    if !args.dry_run {
        write_file(&org_file)?;
        state.save(&args.file)?;
    }

    if is_human {
        println!();
        println!(
            "Push complete: {} created, {} updated, {} unchanged",
            created_items.len(),
            updated_items.len(),
            skipped
        );
    } else {
        let output = PushOutput {
            created: created_items,
            updated: updated_items,
            errors: Vec::new(),
        };
        print!("{}", format(&output, output_format));
    }

    Ok(())
}
