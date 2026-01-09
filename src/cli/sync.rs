use crate::config::Config;
use crate::error::{Error, Result};
use crate::github::model::{CreateIssueRequest, GhIssueState, UpdateIssueRequest};
use crate::github::GitHubClient;
use crate::org::writer::{set_property, set_todo_state};
use crate::org::{parse_file, write_file};
use crate::output::{format, Conflict, Format, PullItem, PushItem, SyncOutput};
use crate::sync::diff::{three_way_diff, FieldChange};
use crate::sync::SyncState;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to org file
    pub file: PathBuf,

    /// Force sync - org wins all conflicts
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

    let mut pushed_items = Vec::new();
    let mut pulled_items = Vec::new();
    let mut conflict_items = Vec::new();
    let mut skipped = 0;

    // Process each org item
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

            // Get base state - required for three-way diff
            let base = match state.items.get(&issue_num) {
                Some(base) => base,
                None => {
                    // No base state - record current state and skip
                    if is_human && args.verbose {
                        println!(
                            "Skip #{}: {} (initializing sync state)",
                            issue_num, item.title
                        );
                    }
                    if !args.dry_run {
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
                    skipped += 1;
                    continue;
                }
            };

            // Perform three-way diff
            let diff = three_way_diff(item, gh_issue, base);

            if !diff.has_changes() {
                skipped += 1;
                if is_human && args.verbose {
                    println!("Skip #{}: {} (no changes)", issue_num, item.title);
                }
                continue;
            }

            // Check for conflicts
            if diff.has_conflicts() && !args.force {
                let conflict_fields = diff.conflict_fields();
                if is_human {
                    println!(
                        "Conflict #{}: {} (fields: {})",
                        issue_num,
                        item.title,
                        conflict_fields.join(", ")
                    );
                }
                for field in &conflict_fields {
                    conflict_items.push(Conflict {
                        issue_number: issue_num,
                        field: field.to_string(),
                        local: "(changed)".to_string(),
                        remote: "(changed)".to_string(),
                    });
                }
                continue;
            }

            if is_human && (args.verbose || args.dry_run) {
                println!("Sync #{}: {}", issue_num, item.title);
            }

            // Apply changes based on which side changed
            let mut gh_updates = UpdateIssueRequest {
                title: None,
                body: None,
                state: None,
                assignees: None,
                labels: None,
            };
            let mut org_changes: Vec<(&str, String)> = Vec::new();

            // Title
            match diff.title {
                FieldChange::OrgChanged | FieldChange::Conflict => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - title: push to GitHub");
                    }
                    gh_updates.title = Some(item.title.clone());
                }
                FieldChange::GitHubChanged => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - title: pull from GitHub (title changes not fully supported)");
                    }
                    // Title pull not implemented - would need to edit headline text
                }
                FieldChange::None => {}
            }

            // Body
            match diff.body {
                FieldChange::OrgChanged | FieldChange::Conflict => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - body: push to GitHub");
                    }
                    gh_updates.body = Some(item.body.clone());
                }
                FieldChange::GitHubChanged => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - body: pull from GitHub (body changes not fully supported)");
                    }
                    // Body pull not implemented - would need complex content editing
                }
                FieldChange::None => {}
            }

            // State
            match diff.state {
                FieldChange::OrgChanged | FieldChange::Conflict => {
                    if is_human && (args.verbose || args.dry_run) {
                        let new_state = if item.state.is_open() {
                            "open"
                        } else {
                            "closed"
                        };
                        println!("  - state: push {} to GitHub", new_state);
                    }
                    gh_updates.state = Some(if item.state.is_open() {
                        GhIssueState::Open
                    } else {
                        GhIssueState::Closed
                    });
                }
                FieldChange::GitHubChanged => {
                    let new_keyword = if gh_issue.state.is_open() {
                        "TODO"
                    } else {
                        "DONE"
                    };
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - state: pull {} from GitHub", new_keyword);
                    }
                    org_changes.push(("state", new_keyword.to_string()));
                }
                FieldChange::None => {}
            }

            // Assignees
            match diff.assignees {
                FieldChange::OrgChanged | FieldChange::Conflict => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - assignees: push to GitHub");
                    }
                    gh_updates.assignees = Some(item.assignees.clone());
                }
                FieldChange::GitHubChanged => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - assignees: pull from GitHub");
                    }
                    org_changes.push(("assignees", gh_issue.assignees.join(",")));
                }
                FieldChange::None => {}
            }

            // Labels
            match diff.labels {
                FieldChange::OrgChanged | FieldChange::Conflict => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - labels: push to GitHub");
                    }
                    gh_updates.labels = Some(item.labels.clone());
                }
                FieldChange::GitHubChanged => {
                    if is_human && (args.verbose || args.dry_run) {
                        println!("  - labels: pull from GitHub");
                    }
                    org_changes.push(("labels", gh_issue.labels.join(",")));
                }
                FieldChange::None => {}
            }

            if !args.dry_run {
                // Capture what we're updating before moving gh_updates
                let updating_title = gh_updates.title.is_some();
                let updating_body = gh_updates.body.is_some();
                let updating_state = gh_updates.state.is_some();
                let updating_assignees = gh_updates.assignees.is_some();
                let updating_labels = gh_updates.labels.is_some();

                // Apply GitHub updates if any
                let has_gh_updates = updating_title
                    || updating_body
                    || updating_state
                    || updating_assignees
                    || updating_labels;

                if has_gh_updates {
                    let updated_issue = client.update_issue(issue_num, gh_updates).await?;
                    pushed_items.push(PushItem {
                        title: updated_issue.title,
                        issue_number: issue_num,
                        url: updated_issue.html_url,
                        action: "updated".to_string(),
                    });
                }

                // Apply org updates
                for (field, value) in &org_changes {
                    match *field {
                        "state" => {
                            org_file.content = set_todo_state(&org_file.content, item, value);
                        }
                        "assignees" => {
                            org_file.content =
                                set_property(&org_file.content, item, "ASSIGNEE", value);
                        }
                        "labels" => {
                            org_file.content =
                                set_property(&org_file.content, item, "LABELS", value);
                        }
                        _ => {}
                    }
                }
                if !org_changes.is_empty() {
                    pulled_items.push(PullItem {
                        issue_number: issue_num,
                        title: gh_issue.title.clone(),
                        changes: org_changes.iter().map(|(f, _)| f.to_string()).collect(),
                    });
                }

                // Update sync state with latest values
                let final_state = if updating_state {
                    if item.state.is_open() {
                        "open"
                    } else {
                        "closed"
                    }
                } else if gh_issue.state.is_open() {
                    "open"
                } else {
                    "closed"
                };

                state.record_sync(
                    issue_num,
                    &item.id,
                    if updating_title {
                        &item.title
                    } else {
                        &gh_issue.title
                    },
                    if updating_body {
                        &item.body
                    } else {
                        gh_issue.body.as_deref().unwrap_or("")
                    },
                    final_state,
                    if updating_assignees {
                        &item.assignees
                    } else {
                        &gh_issue.assignees
                    },
                    if updating_labels {
                        &item.labels
                    } else {
                        &gh_issue.labels
                    },
                    gh_issue.updated_at,
                );
            }
        } else {
            // New item - create in GitHub
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

                pushed_items.push(PushItem {
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
        let created = pushed_items
            .iter()
            .filter(|i| i.action == "created")
            .count();
        let updated = pushed_items
            .iter()
            .filter(|i| i.action == "updated")
            .count();
        println!(
            "Sync complete: {} created, {} pushed, {} pulled, {} unchanged",
            created,
            updated,
            pulled_items.len(),
            skipped
        );
        if !conflict_items.is_empty() {
            println!(
                "  {} conflicts (use --force to let org win)",
                conflict_items.len()
            );
        }
    } else {
        let output = SyncOutput {
            pushed: pushed_items,
            pulled: pulled_items,
            conflicts: conflict_items,
        };
        print!("{}", format(&output, output_format));
    }

    Ok(())
}
