use crate::config::Config;
use crate::error::{Error, Result};
use crate::github::GitHubClient;
use crate::org::writer::set_todo_state;
use crate::org::{parse_file, write_file};
use crate::output::{format, Conflict, Format, PullItem, PullOutput};
use crate::sync::state::hash_body;
use crate::sync::SyncState;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to org file
    pub file: PathBuf,

    /// Force pull - GitHub wins all conflicts
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

    // Fetch all issues from GitHub
    let gh_issues = client.fetch_issues().await?;

    let mut pulled_items = Vec::new();
    let mut conflict_items = Vec::new();
    let mut skipped = 0;

    // Process each org item that has a linked issue
    for item in &org_file.items {
        if let Some(issue_num) = item.gh_issue {
            // Find the corresponding GitHub issue
            let gh_issue = match gh_issues.iter().find(|i| i.number == issue_num) {
                Some(issue) => issue,
                None => {
                    if is_human && args.verbose {
                        println!("Skip #{}: {} (not found on GitHub)", issue_num, item.title);
                    }
                    skipped += 1;
                    continue;
                }
            };

            // Get base state if we have it
            let base = state.items.get(&issue_num);

            // Check what changed on GitHub side
            let mut changes: Vec<(&str, String, String)> = Vec::new();

            if let Some(base) = base {
                // Check if org also changed (conflict detection)
                let org_title_changed = item.title != base.title;
                let org_body_changed = hash_body(&item.body) != base.body_hash;
                let org_state_str = if item.state.is_open() {
                    "open"
                } else {
                    "closed"
                };
                let org_state_changed = org_state_str != base.state;

                let gh_title_changed = gh_issue.title != base.title;
                let gh_body_changed =
                    hash_body(gh_issue.body.as_deref().unwrap_or("")) != base.body_hash;
                let gh_state_str = if gh_issue.state.is_open() {
                    "open"
                } else {
                    "closed"
                };
                let gh_state_changed = gh_state_str != base.state;

                // Detect conflicts (both sides changed)
                if gh_title_changed && org_title_changed && gh_issue.title != item.title {
                    if args.force {
                        changes.push(("title", base.title.clone(), gh_issue.title.clone()));
                    } else {
                        if is_human {
                            println!(
                                "Conflict #{}: title (org: {}, gh: {})",
                                issue_num, item.title, gh_issue.title
                            );
                        }
                        conflict_items.push(Conflict {
                            issue_number: issue_num,
                            field: "title".to_string(),
                            local: item.title.clone(),
                            remote: gh_issue.title.clone(),
                        });
                        continue;
                    }
                } else if gh_title_changed {
                    changes.push(("title", base.title.clone(), gh_issue.title.clone()));
                }

                if gh_body_changed
                    && org_body_changed
                    && gh_issue.body.as_deref().unwrap_or("") != item.body
                {
                    if args.force {
                        changes.push(("body", "(changed)".to_string(), "(changed)".to_string()));
                    } else {
                        if is_human {
                            println!("Conflict #{}: body changed on both sides", issue_num);
                        }
                        conflict_items.push(Conflict {
                            issue_number: issue_num,
                            field: "body".to_string(),
                            local: "(changed)".to_string(),
                            remote: "(changed)".to_string(),
                        });
                        continue;
                    }
                } else if gh_body_changed {
                    changes.push(("body", "(changed)".to_string(), "(changed)".to_string()));
                }

                if gh_state_changed && org_state_changed && gh_state_str != org_state_str {
                    if args.force {
                        changes.push(("state", base.state.clone(), gh_state_str.to_string()));
                    } else {
                        if is_human {
                            println!(
                                "Conflict #{}: state (org: {}, gh: {})",
                                issue_num, org_state_str, gh_state_str
                            );
                        }
                        conflict_items.push(Conflict {
                            issue_number: issue_num,
                            field: "state".to_string(),
                            local: org_state_str.to_string(),
                            remote: gh_state_str.to_string(),
                        });
                        continue;
                    }
                } else if gh_state_changed {
                    changes.push(("state", base.state.clone(), gh_state_str.to_string()));
                }
            } else {
                // No base state - this is first sync for this item
                // Pull everything from GitHub
                if gh_issue.title != item.title {
                    changes.push(("title", item.title.clone(), gh_issue.title.clone()));
                }
                let gh_state_str = if gh_issue.state.is_open() {
                    "open"
                } else {
                    "closed"
                };
                let org_state_str = if item.state.is_open() {
                    "open"
                } else {
                    "closed"
                };
                if gh_state_str != org_state_str {
                    changes.push(("state", org_state_str.to_string(), gh_state_str.to_string()));
                }
            }

            if changes.is_empty() {
                skipped += 1;
                if is_human && args.verbose {
                    println!("Skip #{}: {} (no changes)", issue_num, item.title);
                }
            } else {
                if is_human && (args.verbose || args.dry_run) {
                    println!("Update #{}: {}", issue_num, item.title);
                    for (field, from, to) in &changes {
                        println!("  - {}: {} -> {}", field, from, to);
                    }
                }

                if !args.dry_run {
                    // Apply changes to org file
                    for (field, _, to) in &changes {
                        match *field {
                            "title" => {
                                // Title changes would require editing headline text
                                // This is complex - for now we skip title pulls
                                // The sync command can handle bidirectional title sync
                            }
                            "state" => {
                                let new_keyword = if to == "open" { "TODO" } else { "DONE" };
                                org_file.content =
                                    set_todo_state(&org_file.content, item, new_keyword);
                            }
                            "body" => {
                                // Body sync is complex - skip for pull, handle in sync
                            }
                            _ => {}
                        }
                    }

                    // Update sync state
                    state.record_sync(
                        gh_issue.number,
                        &item.id,
                        &gh_issue.title,
                        gh_issue.body.as_deref().unwrap_or(""),
                        if gh_issue.state.is_open() {
                            "open"
                        } else {
                            "closed"
                        },
                        &gh_issue.assignees,
                        &gh_issue.labels,
                        gh_issue.updated_at,
                    );
                }

                pulled_items.push(PullItem {
                    issue_number: issue_num,
                    title: gh_issue.title.clone(),
                    changes: changes.iter().map(|(f, _, _)| f.to_string()).collect(),
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
            "Pull complete: {} updated, {} unchanged",
            pulled_items.len(),
            skipped
        );
        if !conflict_items.is_empty() {
            println!(
                "  {} conflicts (use --force to override with GitHub values)",
                conflict_items.len()
            );
        }
    } else {
        let output = PullOutput {
            pulled: pulled_items,
            conflicts: conflict_items,
        };
        print!("{}", format(&output, output_format));
    }

    Ok(())
}
