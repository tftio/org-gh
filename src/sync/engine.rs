use crate::config::{Config, ConflictResolution};
use crate::error::{Error, Result};
use crate::github::model::{CreateIssueRequest, GhIssue, GhIssueState, UpdateIssueRequest};
use crate::github::GitHubClient;
use crate::org::model::{OrgFile, OrgItem, TodoState};
use crate::sync::diff::{merge_labels, three_way_diff, DiffResult, FieldChange};
use crate::sync::state::SyncState;
use std::collections::HashMap;

/// Actions to be executed during sync
#[derive(Debug)]
pub enum SyncAction {
    /// Create a new GitHub issue
    CreateIssue { org_item: OrgItem },
    /// Update GitHub issue from org
    UpdateGitHub {
        issue_number: u64,
        request: UpdateIssueRequest,
    },
    /// Update org item from GitHub
    UpdateOrg {
        issue_number: u64,
        gh_issue: GhIssue,
    },
    /// Update both sides (merged)
    UpdateBoth {
        issue_number: u64,
        gh_request: UpdateIssueRequest,
        org_changes: OrgChanges,
    },
    /// Conflict requiring user resolution
    Conflict {
        issue_number: u64,
        fields: Vec<&'static str>,
        org_item: OrgItem,
        gh_issue: GhIssue,
    },
    /// No action needed
    NoOp { issue_number: u64 },
    /// Warning (e.g., issue removed from org)
    Warn { message: String },
}

/// Changes to apply to an org item
#[derive(Debug, Clone)]
pub struct OrgChanges {
    pub state: Option<TodoState>,
    pub assignees: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
    pub comments: Vec<String>,
}

pub struct SyncEngine {
    config: Config,
    client: GitHubClient,
    state: SyncState,
    dry_run: bool,
    force: bool,
}

impl SyncEngine {
    pub async fn new(
        config: Config,
        org_file: &OrgFile,
        dry_run: bool,
        force: bool,
    ) -> Result<Self> {
        let repo = org_file.repo.as_ref().ok_or(Error::NoRepo)?;

        let token = config.github_token()?;
        let client = GitHubClient::new(&token, repo).await?;
        let state = SyncState::load(&org_file.path)?;

        Ok(Self {
            config,
            client,
            state,
            dry_run,
            force,
        })
    }

    /// Plan sync actions by comparing org, GitHub, and base state
    pub async fn plan(&self, org_file: &OrgFile) -> Result<Vec<SyncAction>> {
        let gh_issues = self.client.fetch_issues().await?;
        let gh_map: HashMap<u64, &GhIssue> = gh_issues.iter().map(|i| (i.number, i)).collect();

        let mut actions = Vec::new();

        // Process each org item
        for item in &org_file.items {
            let action = if let Some(issue_num) = item.gh_issue {
                // Linked item - reconcile
                if let Some(gh) = gh_map.get(&issue_num) {
                    if let Some(base) = self.state.items.get(&issue_num) {
                        self.reconcile(item, gh, base)
                    } else {
                        // No base state - treat as GitHub being source of truth
                        SyncAction::UpdateOrg {
                            issue_number: issue_num,
                            gh_issue: (*gh).clone(),
                        }
                    }
                } else {
                    SyncAction::Warn {
                        message: format!(
                            "Issue #{} linked in org but not found in GitHub",
                            issue_num
                        ),
                    }
                }
            } else {
                // Unlinked item - try to match or create
                if let Some(matched) = self.find_matching_issue(item, &gh_issues).await {
                    // Found matching issue - this is initial link
                    SyncAction::UpdateOrg {
                        issue_number: matched.number,
                        gh_issue: matched,
                    }
                } else {
                    // No match - create new issue
                    SyncAction::CreateIssue {
                        org_item: item.clone(),
                    }
                }
            };
            actions.push(action);
        }

        // Check for issues in state that are no longer in org
        for (issue_num, synced) in &self.state.items {
            let in_org = org_file
                .items
                .iter()
                .any(|i| i.gh_issue == Some(*issue_num));
            if !in_org {
                actions.push(SyncAction::Warn {
                    message: format!(
                        "Issue #{} ({}) was in org but heading removed",
                        issue_num, synced.title
                    ),
                });
            }
        }

        Ok(actions)
    }

    /// Reconcile an org item with a GitHub issue using three-way diff
    fn reconcile(
        &self,
        org: &OrgItem,
        gh: &GhIssue,
        base: &crate::sync::state::SyncedItem,
    ) -> SyncAction {
        let diff = three_way_diff(org, gh, base);

        if !diff.has_changes() {
            return SyncAction::NoOp {
                issue_number: gh.number,
            };
        }

        if diff.has_conflicts() && !self.force {
            return SyncAction::Conflict {
                issue_number: gh.number,
                fields: diff.conflict_fields(),
                org_item: org.clone(),
                gh_issue: gh.clone(),
            };
        }

        // Build update requests based on diff and resolution strategy
        let (gh_request, org_changes) = self.resolve_diff(org, gh, &diff);

        match (gh_request, org_changes) {
            (Some(gh_req), Some(org_ch)) => SyncAction::UpdateBoth {
                issue_number: gh.number,
                gh_request: gh_req,
                org_changes: org_ch,
            },
            (Some(req), None) => SyncAction::UpdateGitHub {
                issue_number: gh.number,
                request: req,
            },
            (None, Some(_)) => SyncAction::UpdateOrg {
                issue_number: gh.number,
                gh_issue: gh.clone(),
            },
            (None, None) => SyncAction::NoOp {
                issue_number: gh.number,
            },
        }
    }

    /// Resolve diff into concrete update requests
    fn resolve_diff(
        &self,
        org: &OrgItem,
        gh: &GhIssue,
        diff: &DiffResult,
    ) -> (Option<UpdateIssueRequest>, Option<OrgChanges>) {
        let mut gh_req = UpdateIssueRequest::default();
        let mut org_changes = OrgChanges {
            state: None,
            assignees: None,
            labels: None,
            comments: vec![],
        };
        let mut has_gh_changes = false;
        let mut has_org_changes = false;

        // Title: org wins (or force)
        match diff.title {
            FieldChange::OrgChanged | FieldChange::Conflict => {
                gh_req.title = Some(org.title.clone());
                has_gh_changes = true;
            }
            FieldChange::GitHubChanged => {
                // Title from GH - but we don't update org titles from GH
                // (org is authoring surface)
            }
            FieldChange::None => {}
        }

        // Body: org wins
        match diff.body {
            FieldChange::OrgChanged | FieldChange::Conflict => {
                gh_req.body = Some(org.body.clone());
                has_gh_changes = true;
            }
            FieldChange::GitHubChanged => {
                // Don't update org body from GH
            }
            FieldChange::None => {}
        }

        // State: configurable
        match diff.state {
            FieldChange::OrgChanged => {
                gh_req.state = Some(if org.state.is_open() {
                    GhIssueState::Open
                } else {
                    GhIssueState::Closed
                });
                has_gh_changes = true;
            }
            FieldChange::GitHubChanged => {
                org_changes.state = Some(if gh.state.is_open() {
                    TodoState::Todo
                } else {
                    TodoState::Done
                });
                has_org_changes = true;
            }
            FieldChange::Conflict => {
                // Force mode: org wins
                if self.force || self.config.sync.state_conflict == ConflictResolution::OrgWins {
                    gh_req.state = Some(if org.state.is_open() {
                        GhIssueState::Open
                    } else {
                        GhIssueState::Closed
                    });
                    has_gh_changes = true;
                }
            }
            FieldChange::None => {}
        }

        // Assignees: GitHub wins
        match diff.assignees {
            FieldChange::OrgChanged => {
                gh_req.assignees = Some(org.assignees.clone());
                has_gh_changes = true;
            }
            FieldChange::GitHubChanged | FieldChange::Conflict => {
                org_changes.assignees = Some(gh.assignees.clone());
                has_org_changes = true;
            }
            FieldChange::None => {}
        }

        // Labels: union merge
        match diff.labels {
            FieldChange::OrgChanged => {
                gh_req.labels = Some(org.labels.clone());
                has_gh_changes = true;
            }
            FieldChange::GitHubChanged => {
                org_changes.labels = Some(gh.labels.clone());
                has_org_changes = true;
            }
            FieldChange::Conflict => {
                // Union merge
                let merged = merge_labels(&org.labels, &gh.labels);
                gh_req.labels = Some(merged.clone());
                org_changes.labels = Some(merged);
                has_gh_changes = true;
                has_org_changes = true;
            }
            FieldChange::None => {}
        }

        (
            if has_gh_changes { Some(gh_req) } else { None },
            if has_org_changes {
                Some(org_changes)
            } else {
                None
            },
        )
    }

    /// Try to find an existing GitHub issue matching an org item by title
    async fn find_matching_issue(&self, item: &OrgItem, issues: &[GhIssue]) -> Option<GhIssue> {
        issues.iter().find(|i| i.title == item.title).cloned()
    }

    /// Execute planned actions
    pub async fn execute(
        &mut self,
        actions: Vec<SyncAction>,
        org_file: &mut OrgFile,
    ) -> Result<()> {
        if self.dry_run {
            for action in &actions {
                println!("{:?}", action);
            }
            return Ok(());
        }

        for action in actions {
            match action {
                SyncAction::CreateIssue { org_item } => {
                    let issue = self
                        .client
                        .create_issue(CreateIssueRequest {
                            title: org_item.title.clone(),
                            body: Some(org_item.body.clone()),
                            assignees: org_item.assignees.clone(),
                            labels: org_item.labels.clone(),
                        })
                        .await?;

                    // Update org file with new issue number
                    // TODO: Apply property updates to org_file.content

                    // Record in state
                    self.state.record_sync(
                        issue.number,
                        &org_item.id,
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

                    println!("Created issue #{}: {}", issue.number, issue.title);
                }

                SyncAction::UpdateGitHub {
                    issue_number,
                    request,
                } => {
                    let _issue = self.client.update_issue(issue_number, request).await?;
                    println!("Updated GitHub issue #{}", issue_number);
                }

                SyncAction::UpdateOrg {
                    issue_number,
                    gh_issue: _,
                } => {
                    // TODO: Apply changes to org_file.content
                    println!("Updated org from GitHub issue #{}", issue_number);
                }

                SyncAction::UpdateBoth {
                    issue_number,
                    gh_request,
                    org_changes: _,
                } => {
                    self.client.update_issue(issue_number, gh_request).await?;
                    // TODO: Apply org_changes to org_file.content
                    println!("Updated both sides for issue #{}", issue_number);
                }

                SyncAction::Conflict {
                    issue_number,
                    fields,
                    ..
                } => {
                    eprintln!(
                        "Conflict on issue #{}: {} (use --force to override)",
                        issue_number,
                        fields.join(", ")
                    );
                }

                SyncAction::NoOp { issue_number: _ } => {
                    // Nothing to do
                }

                SyncAction::Warn { message } => {
                    eprintln!("Warning: {}", message);
                }
            }
        }

        // Save updated state
        self.state.save(&org_file.path)?;

        Ok(())
    }
}
