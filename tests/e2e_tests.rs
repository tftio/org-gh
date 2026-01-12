//! End-to-end tests against live GitHub API
//!
//! These tests hit the real tftio/org-gh-test-fixture repository.
//! Run with: cargo test --test e2e_tests -- --ignored --nocapture
//!
//! Prerequisites:
//! - GITHUB_TOKEN env var or gh CLI authenticated
//! - Write access to tftio/org-gh-test-fixture
//!
//! Test isolation:
//! - Each test creates issues with unique prefixes
//! - Tests clean up after themselves by closing created issues
//! - Run cleanup_test_issues first if previous runs left orphans

use chrono::Utc;
use org_gh::github::model::{CreateIssueRequest, UpdateIssueRequest};
use org_gh::github::GitHubClient;
use org_gh::output::Format;
use org_gh::sync::SyncState;
use std::fs;
use tempfile::TempDir;

const E2E_REPO: &str = "tftio/org-gh-test-fixture";
const TEST_PREFIX: &str = "[E2E-TEST]";

/// Wait for GitHub API eventual consistency
async fn wait_for_consistency() {
    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
}

/// Wait longer for search/list operations which have slower consistency
async fn wait_for_search_consistency() {
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
}

/// Get GitHub token from env or gh CLI
fn get_token() -> String {
    std::env::var("GITHUB_TOKEN").unwrap_or_else(|_| {
        let output = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
            .expect("Failed to run gh auth token");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    })
}

/// Generate a unique test issue title
fn unique_title(base: &str) -> String {
    let ts = Utc::now().format("%Y%m%d-%H%M%S");
    format!("{} {} {}", TEST_PREFIX, base, ts)
}

// ============================================================================
// Cleanup
// ============================================================================

#[tokio::test]
#[ignore]
async fn cleanup_test_issues() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let issues = client.fetch_issues().await.expect("Failed to fetch issues");

    let test_issues: Vec<_> = issues
        .iter()
        .filter(|i| i.title.starts_with(TEST_PREFIX))
        .collect();

    println!("Found {} test issues to clean up", test_issues.len());

    for issue in test_issues {
        if issue.state.is_open() {
            println!("  Closing #{}: {}", issue.number, issue.title);
            client
                .close_issue(issue.number)
                .await
                .expect("Failed to close");
        }
    }

    println!("Cleanup complete");
}

// ============================================================================
// GitHub Client E2E Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn e2e_create_and_close_issue() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let title = unique_title("create-close-test");

    // Create issue
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: Some("E2E test issue - will be closed".to_string()),
            assignees: vec![],
            labels: vec!["e2e-test".to_string()],
        })
        .await
        .expect("Failed to create issue");

    println!("Created issue #{}: {}", created.number, created.title);
    assert_eq!(created.title, title);
    assert!(created.state.is_open());
    assert!(created.labels.contains(&"e2e-test".to_string()));

    // Verify it exists
    let fetched = client
        .get_issue(created.number)
        .await
        .expect("Failed to fetch issue");
    assert_eq!(fetched.number, created.number);

    // Close it
    let closed = client
        .close_issue(created.number)
        .await
        .expect("Failed to close issue");
    println!("Closed issue #{}", closed.number);
    assert!(!closed.state.is_open());
}

#[tokio::test]
#[ignore]
async fn e2e_update_issue_fields() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let title = unique_title("update-fields-test");

    // Create issue
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: Some("Original body".to_string()),
            assignees: vec![],
            labels: vec![],
        })
        .await
        .expect("Failed to create issue");

    println!("Created issue #{}", created.number);

    // Update title and body
    let updated = client
        .update_issue(
            created.number,
            UpdateIssueRequest {
                title: Some(format!("{} (updated)", title)),
                body: Some("Updated body content".to_string()),
                state: None,
                assignees: None,
                labels: Some(vec!["e2e-test".to_string(), "updated".to_string()]),
            },
        )
        .await
        .expect("Failed to update issue");

    println!("Updated issue #{}", updated.number);
    assert!(updated.title.contains("(updated)"));
    assert_eq!(updated.body, Some("Updated body content".to_string()));
    assert!(updated.labels.contains(&"updated".to_string()));

    // Cleanup
    client
        .close_issue(created.number)
        .await
        .expect("Failed to close");
}

#[tokio::test]
#[ignore]
async fn e2e_reopen_issue() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let title = unique_title("reopen-test");

    // Create and close
    let created = client
        .create_issue(CreateIssueRequest {
            title,
            body: None,
            assignees: vec![],
            labels: vec!["e2e-test".to_string()],
        })
        .await
        .expect("Failed to create");

    let closed = client
        .close_issue(created.number)
        .await
        .expect("Failed to close");
    assert!(!closed.state.is_open());

    // Reopen
    let reopened = client
        .reopen_issue(created.number)
        .await
        .expect("Failed to reopen");
    println!("Reopened issue #{}", reopened.number);
    assert!(reopened.state.is_open());

    // Cleanup
    client
        .close_issue(created.number)
        .await
        .expect("Failed to close");
}

#[tokio::test]
#[ignore]
async fn e2e_find_by_title() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let title = unique_title("find-by-title-test");

    // Create issue
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: None,
            assignees: vec![],
            labels: vec!["e2e-test".to_string()],
        })
        .await
        .expect("Failed to create");

    // Wait for GitHub API eventual consistency (search needs longer)
    wait_for_search_consistency().await;

    // Find by exact title
    let found = client
        .find_by_title(&title)
        .await
        .expect("Failed to search");

    assert!(found.is_some(), "Should find issue by title");
    assert_eq!(found.unwrap().number, created.number);

    // Should not find non-existent
    let not_found = client
        .find_by_title("definitely-not-a-real-issue-title-12345")
        .await
        .expect("Failed to search");
    assert!(not_found.is_none());

    // Cleanup
    client
        .close_issue(created.number)
        .await
        .expect("Failed to close");
}

// ============================================================================
// Full Sync E2E Tests
// ============================================================================

#[tokio::test]
#[ignore]
async fn e2e_sync_creates_issues() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let title1 = unique_title("sync-create-1");
    let title2 = unique_title("sync-create-2");

    // Create org file with new items
    let content = format!(
        r#"#+TITLE: E2E Test
#+GH_REPO: {}

* TODO {}
:PROPERTIES:
:LABELS: e2e-test
:END:

First test item body.

* TODO {}
:PROPERTIES:
:LABELS: e2e-test
:END:

Second test item body.
"#,
        E2E_REPO, title1, title2
    );

    fs::write(&file_path, &content).unwrap();

    // Run sync via CLI
    let args = org_gh::cli::sync::Args {
        file: file_path.clone(),
        force: false,
        dry_run: false,
        verbose: true,
    };

    org_gh::cli::sync::run(args, Format::Human)
        .await
        .expect("Sync failed");

    // Verify org file was updated with GH_ISSUE properties
    let updated_content = fs::read_to_string(&file_path).unwrap();

    // Extract issue numbers from org file
    let re = regex::Regex::new(r":GH_ISSUE: (\d+)").unwrap();
    let issue_numbers: Vec<u64> = re
        .captures_iter(&updated_content)
        .map(|c| c[1].parse().unwrap())
        .collect();

    assert_eq!(issue_numbers.len(), 2, "Should have 2 GH_ISSUE properties");
    println!("Created issues: {:?}", issue_numbers);

    // Verify issues exist on GitHub
    for num in &issue_numbers {
        let issue = client.get_issue(*num).await.expect("Failed to get issue");
        assert!(issue.title.contains(TEST_PREFIX), "Issue should have test prefix");
        println!("  Verified #{}: {}", issue.number, issue.title);
    }

    // Cleanup
    for num in issue_numbers {
        client.close_issue(num).await.ok();
    }
}

#[tokio::test]
#[ignore]
async fn e2e_sync_pushes_state_change() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let title = unique_title("sync-state-push");

    // Create issue on GitHub first
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: Some("Test body".to_string()),
            assignees: vec![],
            labels: vec!["e2e-test".to_string()],
        })
        .await
        .expect("Failed to create issue");

    println!("Created issue #{}", created.number);

    // Wait for GitHub API eventual consistency
    wait_for_consistency().await;

    // Create org file with DONE state (issue is open on GH)
    let content = format!(
        r#"#+TITLE: E2E Test
#+GH_REPO: {}

* DONE {}
:PROPERTIES:
:GH_ISSUE: {}
:GH_URL: {}
:LABELS: e2e-test
:END:

Test body
"#,
        E2E_REPO, title, created.number, created.html_url
    );

    fs::write(&file_path, &content).unwrap();

    // Initialize sync state (simulate first sync already happened)
    let mut state = SyncState::new(E2E_REPO);
    state.record_sync(
        created.number,
        "heading-1",
        &title,
        "Test body",
        "open", // Base state was open
        &[],
        &["e2e-test".to_string()],
        created.updated_at,
    );
    state.save(&file_path).unwrap();

    // Run sync - should push DONE state to GitHub
    let args = org_gh::cli::sync::Args {
        file: file_path.clone(),
        force: false,
        dry_run: false,
        verbose: true,
    };

    org_gh::cli::sync::run(args, Format::Human)
        .await
        .expect("Sync failed");

    // Verify issue is now closed on GitHub
    let updated = client
        .get_issue(created.number)
        .await
        .expect("Failed to fetch");

    assert!(
        !updated.state.is_open(),
        "Issue should be closed after sync"
    );
    println!("Issue #{} is now closed", created.number);
}

#[tokio::test]
#[ignore]
async fn e2e_sync_pulls_state_change() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let title = unique_title("sync-state-pull");

    // Create issue on GitHub
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: Some("Test body".to_string()),
            assignees: vec![],
            labels: vec!["e2e-test".to_string()],
        })
        .await
        .expect("Failed to create issue");

    // Close it on GitHub
    client
        .close_issue(created.number)
        .await
        .expect("Failed to close");

    println!("Created and closed issue #{}", created.number);

    // Wait for GitHub API eventual consistency
    wait_for_consistency().await;

    // Create org file with TODO state (issue is closed on GH)
    let content = format!(
        r#"#+TITLE: E2E Test
#+GH_REPO: {}

* TODO {}
:PROPERTIES:
:GH_ISSUE: {}
:GH_URL: {}
:LABELS: e2e-test
:END:

Test body
"#,
        E2E_REPO, title, created.number, created.html_url
    );

    fs::write(&file_path, &content).unwrap();

    // Initialize sync state with open state
    let mut state = SyncState::new(E2E_REPO);
    state.record_sync(
        created.number,
        "heading-1",
        &title,
        "Test body",
        "open", // Base state was open
        &[],
        &["e2e-test".to_string()],
        created.updated_at,
    );
    state.save(&file_path).unwrap();

    // Run sync - should pull closed state from GitHub
    let args = org_gh::cli::sync::Args {
        file: file_path.clone(),
        force: false,
        dry_run: false,
        verbose: true,
    };

    org_gh::cli::sync::run(args, Format::Human)
        .await
        .expect("Sync failed");

    // Verify org file now has DONE
    let updated_content = fs::read_to_string(&file_path).unwrap();
    assert!(
        updated_content.contains("* DONE"),
        "Org file should have DONE after pulling closed state"
    );
    println!("Org file updated to DONE");
}

#[tokio::test]
#[ignore]
async fn e2e_sync_pulls_labels() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let title = unique_title("sync-labels-pull");

    // Create issue with labels
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: Some("Test body".to_string()),
            assignees: vec![],
            labels: vec!["e2e-test".to_string(), "bug".to_string()],
        })
        .await
        .expect("Failed to create issue");

    // Add another label via update
    client
        .update_issue(
            created.number,
            UpdateIssueRequest {
                labels: Some(vec![
                    "e2e-test".to_string(),
                    "bug".to_string(),
                    "enhancement".to_string(),
                ]),
                ..Default::default()
            },
        )
        .await
        .expect("Failed to update");

    println!("Created issue #{} with labels", created.number);

    // Wait for GitHub API eventual consistency
    wait_for_consistency().await;

    // Create org file without the new label
    let content = format!(
        r#"#+TITLE: E2E Test
#+GH_REPO: {}

* TODO {}
:PROPERTIES:
:GH_ISSUE: {}
:GH_URL: {}
:LABELS: e2e-test
:END:

Test body
"#,
        E2E_REPO, title, created.number, created.html_url
    );

    fs::write(&file_path, &content).unwrap();

    // Initialize sync state with just e2e-test label
    let mut state = SyncState::new(E2E_REPO);
    state.record_sync(
        created.number,
        "heading-1",
        &title,
        "Test body",
        "open",
        &[],
        &["e2e-test".to_string()],
        created.created_at, // Use created_at so updated_at is newer
    );
    state.save(&file_path).unwrap();

    // Run sync - should pull new labels from GitHub
    let args = org_gh::cli::sync::Args {
        file: file_path.clone(),
        force: false,
        dry_run: false,
        verbose: true,
    };

    org_gh::cli::sync::run(args, Format::Human)
        .await
        .expect("Sync failed");

    // Verify org file has updated labels
    let updated_content = fs::read_to_string(&file_path).unwrap();
    assert!(
        updated_content.contains("bug") || updated_content.contains("enhancement"),
        "Org file should have pulled labels from GitHub"
    );
    println!("Labels synced to org file");

    // Cleanup
    client.close_issue(created.number).await.ok();
}

// ============================================================================
// Edge Cases
// ============================================================================

#[tokio::test]
#[ignore]
async fn e2e_sync_matches_existing_by_title() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let title = unique_title("match-by-title");

    // Create issue on GitHub first
    let created = client
        .create_issue(CreateIssueRequest {
            title: title.clone(),
            body: Some("Created on GitHub".to_string()),
            assignees: vec![],
            labels: vec!["e2e-test".to_string()],
        })
        .await
        .expect("Failed to create issue");

    println!("Created issue #{} on GitHub", created.number);

    // Wait for GitHub API eventual consistency (title matching needs longer)
    wait_for_search_consistency().await;

    // Create org file with same title but NO GH_ISSUE
    let content = format!(
        r#"#+TITLE: E2E Test
#+GH_REPO: {}

* TODO {}
:PROPERTIES:
:LABELS: e2e-test
:END:

Local description
"#,
        E2E_REPO, title
    );

    fs::write(&file_path, &content).unwrap();

    // Run sync - should match existing issue, not create duplicate
    let args = org_gh::cli::sync::Args {
        file: file_path.clone(),
        force: false,
        dry_run: false,
        verbose: true,
    };

    org_gh::cli::sync::run(args, Format::Human)
        .await
        .expect("Sync failed");

    // Verify org file was linked to existing issue
    let updated_content = fs::read_to_string(&file_path).unwrap();
    assert!(
        updated_content.contains(&format!(":GH_ISSUE: {}", created.number)),
        "Org file should be linked to existing issue"
    );

    // Verify no duplicate was created
    let all_matching = client
        .fetch_issues()
        .await
        .expect("Failed to fetch")
        .into_iter()
        .filter(|i| i.title == title)
        .collect::<Vec<_>>();

    assert_eq!(
        all_matching.len(),
        1,
        "Should only have one issue with this title"
    );

    println!("Matched existing issue #{}", created.number);

    // Cleanup
    client.close_issue(created.number).await.ok();
}

#[tokio::test]
#[ignore]
async fn e2e_dry_run_makes_no_changes() {
    let token = get_token();
    let client = GitHubClient::new(&token, E2E_REPO)
        .await
        .expect("Failed to create client");

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let title = unique_title("dry-run-test");

    // Create org file with new item
    let content = format!(
        r#"#+TITLE: E2E Test
#+GH_REPO: {}

* TODO {}
:PROPERTIES:
:LABELS: e2e-test
:END:

Should not be created
"#,
        E2E_REPO, title
    );

    fs::write(&file_path, &content).unwrap();
    let original_content = content.clone();

    // Run sync with dry_run
    let args = org_gh::cli::sync::Args {
        file: file_path.clone(),
        force: false,
        dry_run: true,
        verbose: true,
    };

    org_gh::cli::sync::run(args, Format::Human)
        .await
        .expect("Sync failed");

    // Verify no issue was created
    let found = client.find_by_title(&title).await.expect("Failed to search");
    assert!(found.is_none(), "Dry run should not create issues");

    // Verify org file unchanged
    let after_content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(
        after_content, original_content,
        "Dry run should not modify org file"
    );

    println!("Dry run made no changes");
}
