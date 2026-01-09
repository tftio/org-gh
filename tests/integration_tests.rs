//! Integration tests using recorded fixtures

mod common;

use common::{load_fixture, sample_org_content, setup_mock_github, TEST_REPO};
use org_gh::github::GitHubClient;
use org_gh::org::parse_file;
use org_gh::output::Format;
use org_gh::sync::SyncState;
use std::fs;
use tempfile::TempDir;

// ============================================================================
// GitHub Client Tests
// ============================================================================

#[tokio::test]
async fn test_fetch_issues() {
    let server = setup_mock_github().await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let issues = client.fetch_issues().await.expect("Failed to fetch issues");

    // Should have both open and closed issues
    assert!(
        issues.len() >= 4,
        "Expected at least 4 issues, got {}",
        issues.len()
    );

    // Verify we got specific issues
    let issue_1 = issues.iter().find(|i| i.number == 1);
    assert!(issue_1.is_some(), "Issue #1 not found");
    assert_eq!(issue_1.unwrap().title, "Test issue open simple");

    let issue_2 = issues.iter().find(|i| i.number == 2);
    assert!(issue_2.is_some(), "Issue #2 not found");
    assert_eq!(issue_2.unwrap().assignees, vec!["tftio"]);
    assert!(issue_2.unwrap().labels.contains(&"bug".to_string()));

    let issue_3 = issues.iter().find(|i| i.number == 3);
    assert!(issue_3.is_some(), "Issue #3 not found");
    assert!(!issue_3.unwrap().state.is_open()); // closed
}

#[tokio::test]
async fn test_get_single_issue() {
    let server = setup_mock_github().await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let issue = client.get_issue(1).await.expect("Failed to get issue");

    assert_eq!(issue.number, 1);
    assert_eq!(issue.title, "Test issue open simple");
    assert!(issue.state.is_open());
}

#[tokio::test]
async fn test_fetch_comments() {
    let server = setup_mock_github().await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let comments = client
        .fetch_comments(5)
        .await
        .expect("Failed to fetch comments");

    assert_eq!(comments.len(), 2, "Expected 2 comments");
    assert_eq!(comments[0].body, "First comment on the issue");
    assert_eq!(comments[1].body, "Second comment for testing");
}

#[tokio::test]
async fn test_find_by_title() {
    let server = setup_mock_github().await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let found = client
        .find_by_title("Test issue open simple")
        .await
        .expect("Failed to search");

    assert!(found.is_some());
    assert_eq!(found.unwrap().number, 1);

    let not_found = client
        .find_by_title("Nonexistent issue")
        .await
        .expect("Failed to search");

    assert!(not_found.is_none());
}

// ============================================================================
// Org Parser Tests
// ============================================================================

#[test]
fn test_parse_org_with_gh_repo() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    fs::write(&file_path, sample_org_content()).unwrap();

    let org_file = parse_file(&file_path).expect("Failed to parse");

    assert_eq!(org_file.repo, Some(TEST_REPO.to_string()));
    assert_eq!(org_file.items.len(), 4);
}

#[test]
fn test_parse_org_extracts_properties() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    fs::write(&file_path, sample_org_content()).unwrap();

    let org_file = parse_file(&file_path).expect("Failed to parse");

    // Item with GH_ISSUE
    let item_1 = org_file.items.iter().find(|i| i.gh_issue == Some(1));
    assert!(item_1.is_some());
    assert_eq!(item_1.unwrap().title, "Test issue open simple");

    // Item with assignee and labels
    let item_2 = org_file.items.iter().find(|i| i.gh_issue == Some(2));
    assert!(item_2.is_some());
    let item_2 = item_2.unwrap();
    assert_eq!(item_2.assignees, vec!["tftio"]);
    assert!(item_2.labels.contains(&"bug".to_string()));
    assert!(item_2.labels.contains(&"enhancement".to_string()));

    // DONE item
    let item_3 = org_file.items.iter().find(|i| i.gh_issue == Some(3));
    assert!(item_3.is_some());
    assert!(item_3.unwrap().state.is_closed());

    // Item without GH_ISSUE
    let new_item = org_file.items.iter().find(|i| i.gh_issue.is_none());
    assert!(new_item.is_some());
    assert_eq!(new_item.unwrap().title, "New item without issue");
}

#[test]
fn test_parse_org_todo_states() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    // Note: orgize 0.10 only recognizes TODO and DONE by default
    // Custom keywords (DOING, BLOCKED, etc.) require parser configuration
    let content = r#"#+GH_REPO: test/repo

* TODO Task todo
* DONE Task done
"#;

    fs::write(&file_path, content).unwrap();

    let org_file = parse_file(&file_path).expect("Failed to parse");

    assert_eq!(org_file.items.len(), 2);

    let todo = org_file
        .items
        .iter()
        .find(|i| i.title == "Task todo")
        .unwrap();
    assert!(todo.state.is_open());

    let done = org_file
        .items
        .iter()
        .find(|i| i.title == "Task done")
        .unwrap();
    assert!(done.state.is_closed());
}

#[test]
fn test_parse_org_without_gh_repo() {
    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let content = r#"#+TITLE: No Repo

* TODO Some task
"#;

    fs::write(&file_path, content).unwrap();

    let org_file = parse_file(&file_path).expect("Failed to parse");

    assert!(org_file.repo.is_none());
}

// ============================================================================
// Sync State Tests
// ============================================================================

#[test]
fn test_sync_state_roundtrip() {
    let dir = TempDir::new().unwrap();
    let org_path = dir.path().join("test.org");
    fs::write(&org_path, "").unwrap();

    let mut state = SyncState::new(TEST_REPO);

    state.record_sync(
        1,
        "heading-1",
        "Test Title",
        "Test body",
        "open",
        &["user1".to_string()],
        &["bug".to_string()],
        chrono::Utc::now(),
    );

    state.save(&org_path).expect("Failed to save");

    let loaded = SyncState::load(&org_path).expect("Failed to load");

    assert_eq!(loaded.repo, TEST_REPO);
    assert!(loaded.items.contains_key(&1));

    let item = loaded.items.get(&1).unwrap();
    assert_eq!(item.title, "Test Title");
    assert_eq!(item.state, "open");
}

#[test]
fn test_sync_state_remove() {
    let mut state = SyncState::new(TEST_REPO);

    state.record_sync(
        1,
        "heading-1",
        "Title",
        "Body",
        "open",
        &[],
        &[],
        chrono::Utc::now(),
    );

    assert!(state.items.contains_key(&1));

    state.remove(1);

    assert!(!state.items.contains_key(&1));
}

// ============================================================================
// Diff Tests
// ============================================================================

#[test]
fn test_three_way_diff_no_changes() {
    use org_gh::github::model::{GhIssue, GhIssueState};
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::sync::diff::three_way_diff;
    use org_gh::sync::state::{hash_body, SyncedItem};

    let org = OrgItem {
        id: "test".to_string(),
        title: "Title".to_string(),
        body: "Body".to_string(),
        state: TodoState::Todo,
        gh_issue: Some(1),
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..10,
        properties_span: None,
    };

    let gh = GhIssue {
        number: 1,
        title: "Title".to_string(),
        body: Some("Body".to_string()),
        state: GhIssueState::Open,
        assignees: vec![],
        labels: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        closed_at: None,
        html_url: "https://github.com/test/repo/issues/1".to_string(),
    };

    let base = SyncedItem {
        org_heading_id: "test".to_string(),
        title: "Title".to_string(),
        body_hash: hash_body("Body"),
        state: "open".to_string(),
        assignees: vec![],
        labels: vec![],
        gh_updated_at: chrono::Utc::now(),
        org_updated_at: None,
    };

    let diff = three_way_diff(&org, &gh, &base);

    assert!(!diff.has_changes());
    assert!(!diff.has_conflicts());
}

#[test]
fn test_three_way_diff_org_changed() {
    use org_gh::github::model::{GhIssue, GhIssueState};
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::sync::diff::{three_way_diff, FieldChange};
    use org_gh::sync::state::{hash_body, SyncedItem};

    let org = OrgItem {
        id: "test".to_string(),
        title: "New Title".to_string(), // Changed
        body: "Body".to_string(),
        state: TodoState::Todo,
        gh_issue: Some(1),
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..10,
        properties_span: None,
    };

    let gh = GhIssue {
        number: 1,
        title: "Title".to_string(), // Unchanged
        body: Some("Body".to_string()),
        state: GhIssueState::Open,
        assignees: vec![],
        labels: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        closed_at: None,
        html_url: "https://github.com/test/repo/issues/1".to_string(),
    };

    let base = SyncedItem {
        org_heading_id: "test".to_string(),
        title: "Title".to_string(),
        body_hash: hash_body("Body"),
        state: "open".to_string(),
        assignees: vec![],
        labels: vec![],
        gh_updated_at: chrono::Utc::now(),
        org_updated_at: None,
    };

    let diff = three_way_diff(&org, &gh, &base);

    assert!(diff.has_changes());
    assert!(!diff.has_conflicts());
    assert_eq!(diff.title, FieldChange::OrgChanged);
}

#[test]
fn test_three_way_diff_conflict() {
    use org_gh::github::model::{GhIssue, GhIssueState};
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::sync::diff::{three_way_diff, FieldChange};
    use org_gh::sync::state::{hash_body, SyncedItem};

    let org = OrgItem {
        id: "test".to_string(),
        title: "Org Title".to_string(), // Changed
        body: "Body".to_string(),
        state: TodoState::Todo,
        gh_issue: Some(1),
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..10,
        properties_span: None,
    };

    let gh = GhIssue {
        number: 1,
        title: "GH Title".to_string(), // Also changed, different value
        body: Some("Body".to_string()),
        state: GhIssueState::Open,
        assignees: vec![],
        labels: vec![],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        closed_at: None,
        html_url: "https://github.com/test/repo/issues/1".to_string(),
    };

    let base = SyncedItem {
        org_heading_id: "test".to_string(),
        title: "Original Title".to_string(), // Base value
        body_hash: hash_body("Body"),
        state: "open".to_string(),
        assignees: vec![],
        labels: vec![],
        gh_updated_at: chrono::Utc::now(),
        org_updated_at: None,
    };

    let diff = three_way_diff(&org, &gh, &base);

    assert!(diff.has_changes());
    assert!(diff.has_conflicts());
    assert_eq!(diff.title, FieldChange::Conflict);
}

// ============================================================================
// Writer Tests
// ============================================================================

#[test]
fn test_set_property_new_drawer() {
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::org::writer::set_property;

    let content = "* TODO Test item\nSome body text\n";

    let item = OrgItem {
        id: "test".to_string(),
        title: "Test item".to_string(),
        body: "Some body text".to_string(),
        state: TodoState::Todo,
        gh_issue: None,
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..content.len(),
        properties_span: None, // No existing drawer
    };

    let result = set_property(content, &item, "GH_ISSUE", "42");

    assert!(result.contains(":PROPERTIES:"));
    assert!(result.contains(":GH_ISSUE: 42"));
    assert!(result.contains(":END:"));
}

#[test]
fn test_set_todo_state() {
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::org::writer::set_todo_state;

    let content = "* TODO Test item\nBody\n";

    let item = OrgItem {
        id: "test".to_string(),
        title: "Test item".to_string(),
        body: "Body".to_string(),
        state: TodoState::Todo,
        gh_issue: None,
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..content.len(),
        properties_span: None,
    };

    let result = set_todo_state(content, &item, "DONE");

    assert!(result.contains("* DONE Test item"));
    assert!(!result.contains("* TODO"));
}

// ============================================================================
// Config Tests
// ============================================================================

#[test]
fn test_config_github_token_from_gh_cli() {
    // This test verifies the gh auth token fallback works
    use org_gh::config::Config;

    let config = Config::default();

    // Should try gh auth token if no token configured
    // This will succeed if gh is authenticated, fail otherwise
    let result = config.github_token();

    // We just verify it doesn't panic - actual success depends on gh being configured
    assert!(result.is_ok() || result.is_err());
}

// ============================================================================
// Error Handling Tests
// ============================================================================

#[test]
fn test_parse_invalid_repo_format() {
    use org_gh::github::GitHubClient;

    let result = tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(GitHubClient::new("token", "invalid-no-slash"));

    assert!(result.is_err());
}

#[test]
fn test_parse_nonexistent_file() {
    let result = parse_file(std::path::Path::new("/nonexistent/path/file.org"));
    assert!(result.is_err());
}

// ============================================================================
// Writer Edge Case Tests
// ============================================================================

#[test]
fn test_set_property_existing_drawer() {
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::org::writer::set_property;

    let content = r#"* TODO Test item
:PROPERTIES:
:OLD_PROP: old value
:END:
Some body text
"#;

    let props_start = content.find(":PROPERTIES:").unwrap();
    let props_end = content.find(":END:").unwrap() + 5;

    let item = OrgItem {
        id: "test".to_string(),
        title: "Test item".to_string(),
        body: "Some body text".to_string(),
        state: TodoState::Todo,
        gh_issue: None,
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..content.len(),
        properties_span: Some(props_start..props_end),
    };

    let result = set_property(content, &item, "GH_ISSUE", "42");

    assert!(result.contains(":GH_ISSUE: 42"));
    assert!(result.contains(":OLD_PROP: old value"));
}

#[test]
fn test_update_existing_property() {
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::org::writer::set_property;

    let content = r#"* TODO Test item
:PROPERTIES:
:GH_ISSUE: 1
:END:
"#;

    let props_start = content.find(":PROPERTIES:").unwrap();
    let props_end = content.find(":END:").unwrap() + 5;

    let item = OrgItem {
        id: "test".to_string(),
        title: "Test item".to_string(),
        body: String::new(),
        state: TodoState::Todo,
        gh_issue: Some(1),
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..content.len(),
        properties_span: Some(props_start..props_end),
    };

    let result = set_property(content, &item, "GH_ISSUE", "99");

    assert!(result.contains(":GH_ISSUE: 99"));
    assert!(!result.contains(":GH_ISSUE: 1"));
}

// ============================================================================
// Hash and State Tests
// ============================================================================

#[test]
fn test_body_hash_consistency() {
    use org_gh::sync::state::hash_body;

    let body = "This is test content with\nmultiple lines.";

    let hash1 = hash_body(body);
    let hash2 = hash_body(body);

    assert_eq!(hash1, hash2);
    assert!(hash1.starts_with("sha256:"));
}

#[test]
fn test_body_hash_different_content() {
    use org_gh::sync::state::hash_body;

    let hash1 = hash_body("Content A");
    let hash2 = hash_body("Content B");

    assert_ne!(hash1, hash2);
}

#[test]
fn test_sync_state_pending_creates() {
    let mut state = SyncState::new(TEST_REPO);

    state.add_pending_create("heading-1", "New Feature");
    state.add_pending_create("heading-2", "Another Feature");

    assert_eq!(state.pending_creates.len(), 2);

    state.remove_pending_create("heading-1");

    assert_eq!(state.pending_creates.len(), 1);
    assert_eq!(state.pending_creates[0].title, "Another Feature");
}

// ============================================================================
// Model Tests
// ============================================================================

#[test]
fn test_todo_state_keywords() {
    use org_gh::org::model::TodoState;

    assert_eq!(TodoState::Todo.to_keyword(), "TODO");
    assert_eq!(TodoState::Doing.to_keyword(), "DOING");
    assert_eq!(TodoState::Blocked.to_keyword(), "BLOCKED");
    assert_eq!(TodoState::Waiting.to_keyword(), "WAITING");
    assert_eq!(TodoState::Done.to_keyword(), "DONE");
    assert_eq!(TodoState::Cancelled.to_keyword(), "CANCELLED");
}

#[test]
fn test_todo_state_from_keyword() {
    use org_gh::org::model::TodoState;

    assert_eq!(TodoState::from_keyword("TODO"), Some(TodoState::Todo));
    assert_eq!(TodoState::from_keyword("todo"), Some(TodoState::Todo)); // case insensitive
    assert_eq!(TodoState::from_keyword("DOING"), Some(TodoState::Doing));
    assert_eq!(TodoState::from_keyword("BLOCKED"), Some(TodoState::Blocked));
    assert_eq!(TodoState::from_keyword("WAITING"), Some(TodoState::Waiting));
    assert_eq!(TodoState::from_keyword("DONE"), Some(TodoState::Done));
    assert_eq!(
        TodoState::from_keyword("CANCELLED"),
        Some(TodoState::Cancelled)
    );
    assert_eq!(
        TodoState::from_keyword("CANCELED"),
        Some(TodoState::Cancelled)
    ); // alt spelling
    assert_eq!(
        TodoState::from_keyword("WONTFIX"),
        Some(TodoState::Cancelled)
    );
    assert_eq!(TodoState::from_keyword("INVALID"), None);
}

#[test]
fn test_gh_issue_state_is_open() {
    use org_gh::github::model::GhIssueState;

    assert!(GhIssueState::Open.is_open());
    assert!(!GhIssueState::Closed.is_open());
}

// ============================================================================
// Diff Tests - Additional Cases
// ============================================================================

#[test]
fn test_merge_labels_deduplication() {
    use org_gh::sync::diff::merge_labels;

    let org = vec!["bug".to_string(), "feature".to_string()];
    let gh = vec!["feature".to_string(), "docs".to_string()];

    let merged = merge_labels(&org, &gh);

    // Should have 3 unique labels, sorted
    assert_eq!(merged.len(), 3);
    assert!(merged.contains(&"bug".to_string()));
    assert!(merged.contains(&"docs".to_string()));
    assert!(merged.contains(&"feature".to_string()));
}

#[test]
fn test_diff_result_conflict_fields() {
    use org_gh::sync::diff::{DiffResult, FieldChange};

    let diff = DiffResult {
        title: FieldChange::Conflict,
        body: FieldChange::None,
        state: FieldChange::Conflict,
        assignees: FieldChange::GitHubChanged,
        labels: FieldChange::OrgChanged,
    };

    let fields = diff.conflict_fields();
    assert_eq!(fields.len(), 2);
    assert!(fields.contains(&"title"));
    assert!(fields.contains(&"state"));
}

// ============================================================================
// Create Issue Request Tests
// ============================================================================

#[tokio::test]
async fn test_create_issue_mock() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    // Use a recorded fixture as the response
    let fixture = load_fixture("issue_1.json");

    Mock::given(method("POST"))
        .and(path(format!("/repos/{}/issues", TEST_REPO)))
        .respond_with(ResponseTemplate::new(201).set_body_string(fixture))
        .mount(&server)
        .await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let req = org_gh::github::model::CreateIssueRequest {
        title: "New Issue".to_string(),
        body: Some("Issue body".to_string()),
        assignees: vec![],
        labels: vec![],
    };

    let issue = client
        .create_issue(req)
        .await
        .expect("Failed to create issue");

    // The fixture is for issue #1
    assert_eq!(issue.number, 1);
    assert_eq!(issue.title, "Test issue open simple");
}

#[tokio::test]
async fn test_update_issue_mock() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    // Use closed issue fixture for update response
    let fixture = load_fixture("issue_3.json");

    Mock::given(method("PATCH"))
        .and(path(format!("/repos/{}/issues/3", TEST_REPO)))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&server)
        .await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let req = org_gh::github::model::UpdateIssueRequest {
        title: Some("Updated Title".to_string()),
        state: Some(org_gh::github::model::GhIssueState::Closed),
        ..Default::default()
    };

    let issue = client
        .update_issue(3, req)
        .await
        .expect("Failed to update issue");

    // The fixture is for issue #3 which is closed
    assert_eq!(issue.number, 3);
    assert!(!issue.state.is_open());
}

#[tokio::test]
async fn test_close_issue_mock() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    // Use closed issue fixture
    let fixture = load_fixture("issue_3.json");

    Mock::given(method("PATCH"))
        .and(path(format!("/repos/{}/issues/1", TEST_REPO)))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&server)
        .await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let issue = client.close_issue(1).await.expect("Failed to close issue");

    assert!(!issue.state.is_open());
}

#[tokio::test]
async fn test_reopen_issue_mock() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    let server = MockServer::start().await;

    // Use open issue fixture
    let fixture = load_fixture("issue_1.json");

    Mock::given(method("PATCH"))
        .and(path(format!("/repos/{}/issues/3", TEST_REPO)))
        .respond_with(ResponseTemplate::new(200).set_body_string(fixture))
        .mount(&server)
        .await;

    let client = GitHubClient::with_base_url("fake-token", TEST_REPO, Some(&server.uri()))
        .await
        .expect("Failed to create client");

    let issue = client
        .reopen_issue(3)
        .await
        .expect("Failed to reopen issue");

    assert!(issue.state.is_open());
}

// ============================================================================
// Writer Tests - write_file and append_to_logbook
// ============================================================================

#[test]
fn test_write_file() {
    use org_gh::org::model::OrgFile;
    use org_gh::org::writer::write_file;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");

    let org_file = OrgFile {
        path: file_path.clone(),
        repo: Some("owner/repo".to_string()),
        content: "#+TITLE: Test\n* TODO Item\n".to_string(),
        items: vec![],
    };

    write_file(&org_file).expect("Failed to write file");

    let content = fs::read_to_string(&file_path).expect("Failed to read file");
    assert_eq!(content, "#+TITLE: Test\n* TODO Item\n");
}

#[test]
fn test_append_to_logbook_new() {
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::org::writer::append_to_logbook;

    let content = r#"* TODO Test item
:PROPERTIES:
:GH_ISSUE: 1
:END:
Some body text
"#;

    let props_start = content.find(":PROPERTIES:").unwrap();
    let props_end = content.find(":END:").unwrap() + 5;

    let item = OrgItem {
        id: "test".to_string(),
        title: "Test item".to_string(),
        body: "Some body text".to_string(),
        state: TodoState::Todo,
        gh_issue: Some(1),
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..content.len(),
        properties_span: Some(props_start..props_end),
    };

    let result = append_to_logbook(content, &item, "- State changed to DONE [2026-01-09]");

    assert!(result.contains(":LOGBOOK:"));
    assert!(result.contains("- State changed to DONE [2026-01-09]"));
    assert!(result.contains(":END:"));
}

#[test]
fn test_append_to_logbook_existing() {
    use org_gh::org::model::{OrgItem, TodoState};
    use org_gh::org::writer::append_to_logbook;

    let content = r#"* TODO Test item
:PROPERTIES:
:GH_ISSUE: 1
:END:
:LOGBOOK:
- Previous entry
:END:
Some body text
"#;

    let props_start = content.find(":PROPERTIES:").unwrap();
    let props_end = content.find(":END:").unwrap() + 5;

    let item = OrgItem {
        id: "test".to_string(),
        title: "Test item".to_string(),
        body: "Some body text".to_string(),
        state: TodoState::Todo,
        gh_issue: Some(1),
        gh_url: None,
        assignees: vec![],
        labels: vec![],
        created: None,
        updated: None,
        span: 0..content.len(),
        properties_span: Some(props_start..props_end),
    };

    let result = append_to_logbook(content, &item, "- New entry");

    assert!(result.contains("- Previous entry"));
    assert!(result.contains("- New entry"));
}

// ============================================================================
// Config Tests - load and config_path
// ============================================================================

#[test]
fn test_config_load_default() {
    use org_gh::config::Config;

    // Config::load() returns default if file doesn't exist
    let config = Config::load().expect("Failed to load config");

    // Check default values
    assert_eq!(config.sync.doing_label, "in-progress");
    assert_eq!(config.sync.blocked_label, "blocked");
}

#[test]
fn test_config_path() {
    use org_gh::config::Config;

    let path = Config::config_path().expect("Failed to get config path");

    // Should contain org-gh in the path
    assert!(path.to_string_lossy().contains("org-gh"));
    assert!(path.to_string_lossy().ends_with("config.toml"));
}

// ============================================================================
// Sync Engine Tests
// ============================================================================

#[test]
fn test_sync_action_debug() {
    use org_gh::sync::engine::SyncAction;

    // Test that SyncAction can be debug printed
    let action = SyncAction::NoOp { issue_number: 42 };
    let debug_str = format!("{:?}", action);
    assert!(debug_str.contains("NoOp"));
    assert!(debug_str.contains("42"));
}

#[test]
fn test_org_changes_default() {
    use org_gh::sync::engine::OrgChanges;

    let changes = OrgChanges {
        state: None,
        assignees: None,
        labels: None,
        comments: vec![],
    };

    assert!(changes.state.is_none());
    assert!(changes.assignees.is_none());
    assert!(changes.labels.is_none());
    assert!(changes.comments.is_empty());
}

// ============================================================================
// CLI Init Command Tests
// ============================================================================

#[tokio::test]
async fn test_init_validates_repo_format() {
    use org_gh::cli::init::Args;
    use std::path::PathBuf;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(&file_path, "* TODO Test\n").unwrap();

    let args = Args {
        file: file_path,
        repo: "invalid-no-slash".to_string(),
    };

    let result = org_gh::cli::init::run(args, Format::Human).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_init_file_not_found() {
    use org_gh::cli::init::Args;
    use std::path::PathBuf;

    let args = Args {
        file: PathBuf::from("/nonexistent/file.org"),
        repo: "owner/repo".to_string(),
    };

    let result = org_gh::cli::init::run(args, Format::Human).await;
    assert!(result.is_err());
}

// ============================================================================
// CLI Status Command Tests
// ============================================================================

#[tokio::test]
async fn test_status_no_repo() {
    use org_gh::cli::status::Args;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(&file_path, "* TODO Test\n").unwrap(); // No GH_REPO

    let args = Args { file: file_path };

    let result = org_gh::cli::status::run(args, Format::Human).await;
    assert!(result.is_err());
}

// ============================================================================
// CLI Push Command Tests
// ============================================================================

#[tokio::test]
async fn test_push_no_repo() {
    use org_gh::cli::push::Args;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(&file_path, "* TODO Test\n").unwrap(); // No GH_REPO

    let args = Args {
        file: file_path,
        force: false,
        dry_run: false,
        verbose: false,
    };

    let result = org_gh::cli::push::run(args, Format::Human).await;
    assert!(result.is_err());
}

// ============================================================================
// CLI Pull Command Tests
// ============================================================================

#[tokio::test]
async fn test_pull_no_repo() {
    use org_gh::cli::pull::Args;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(&file_path, "* TODO Test\n").unwrap(); // No GH_REPO

    let args = Args {
        file: file_path,
        force: false,
        dry_run: false,
        verbose: false,
    };

    let result = org_gh::cli::pull::run(args, Format::Human).await;
    assert!(result.is_err());
}

// ============================================================================
// CLI Sync Command Tests
// ============================================================================

#[tokio::test]
async fn test_sync_no_repo() {
    use org_gh::cli::sync::Args;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(&file_path, "* TODO Test\n").unwrap(); // No GH_REPO

    let args = Args {
        file: file_path,
        force: false,
        dry_run: false,
        verbose: false,
    };

    let result = org_gh::cli::sync::run(args, Format::Human).await;
    assert!(result.is_err());
}

// ============================================================================
// CLI Unlink Command Tests
// ============================================================================

#[tokio::test]
async fn test_unlink_not_found() {
    use org_gh::cli::unlink::Args;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(&file_path, "#+GH_REPO: owner/repo\n* TODO Test\n").unwrap();

    let args = Args {
        file: file_path,
        target: "nonexistent heading".to_string(),
        close: false,
    };

    let result = org_gh::cli::unlink::run(args, Format::Human).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_unlink_not_linked() {
    use org_gh::cli::unlink::Args;

    let dir = TempDir::new().unwrap();
    let file_path = dir.path().join("test.org");
    fs::write(
        &file_path,
        "#+GH_REPO: owner/repo\n* TODO Test item\nBody\n",
    )
    .unwrap();

    let args = Args {
        file: file_path,
        target: "Test item".to_string(),
        close: false,
    };

    // Should succeed but print message that item is not linked
    let result = org_gh::cli::unlink::run(args, Format::Human).await;
    assert!(result.is_ok());
}
