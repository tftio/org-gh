//! Common test utilities and fixtures

use std::fs;
use std::path::Path;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

pub const TEST_REPO: &str = "tftio/org-gh-test-fixture";
pub const TEST_OWNER: &str = "tftio";
pub const TEST_REPO_NAME: &str = "org-gh-test-fixture";

/// Load a fixture file as a string
pub fn load_fixture(name: &str) -> String {
    let path = Path::new("tests/fixtures").join(name);
    fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "Failed to load fixture {}: {}. Run `cargo test --test record_fixtures -- --ignored` first.",
            path.display(),
            e
        )
    })
}

/// Load a fixture file as JSON
pub fn load_fixture_json(name: &str) -> serde_json::Value {
    serde_json::from_str(&load_fixture(name)).expect("Invalid JSON in fixture")
}

/// Set up a mock server with standard GitHub API responses
pub async fn setup_mock_github() -> MockServer {
    let server = MockServer::start().await;

    // Mock: GET /repos/{owner}/{repo}/issues?state=open
    if Path::new("tests/fixtures/issues_open.json").exists() {
        let body = load_fixture("issues_open.json");
        Mock::given(method("GET"))
            .and(path(format!("/repos/{}/issues", TEST_REPO)))
            .and(query_param("state", "open"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }

    // Mock: GET /repos/{owner}/{repo}/issues?state=closed
    if Path::new("tests/fixtures/issues_closed.json").exists() {
        let body = load_fixture("issues_closed.json");
        Mock::given(method("GET"))
            .and(path(format!("/repos/{}/issues", TEST_REPO)))
            .and(query_param("state", "closed"))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }

    // Mock individual issues
    for i in 1..=5 {
        let fixture_name = format!("issue_{}.json", i);
        if Path::new("tests/fixtures").join(&fixture_name).exists() {
            let body = load_fixture(&fixture_name);
            Mock::given(method("GET"))
                .and(path(format!("/repos/{}/issues/{}", TEST_REPO, i)))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&server)
                .await;
        }
    }

    // Mock: GET /repos/{owner}/{repo}/issues/5/comments
    if Path::new("tests/fixtures/issue_5_comments.json").exists() {
        let body = load_fixture("issue_5_comments.json");
        Mock::given(method("GET"))
            .and(path(format!("/repos/{}/issues/5/comments", TEST_REPO)))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }

    // Mock: GET /repos/{owner}/{repo}/labels
    if Path::new("tests/fixtures/labels.json").exists() {
        let body = load_fixture("labels.json");
        Mock::given(method("GET"))
            .and(path(format!("/repos/{}/labels", TEST_REPO)))
            .respond_with(ResponseTemplate::new(200).set_body_string(body))
            .mount(&server)
            .await;
    }

    server
}

/// Create a test org file content
pub fn sample_org_content() -> String {
    format!(
        r#"#+TITLE: Test Roadmap
#+GH_REPO: {}

* TODO Test issue open simple
:PROPERTIES:
:GH_ISSUE: 1
:END:
Simple body text

* TODO Test issue with labels
:PROPERTIES:
:GH_ISSUE: 2
:ASSIGNEE: tftio
:LABELS: bug,enhancement
:END:
Has labels and assignee

* DONE Test issue closed
:PROPERTIES:
:GH_ISSUE: 3
:LABELS: documentation
:END:
This one is done

* TODO New item without issue
This should be created on push
"#,
        TEST_REPO
    )
}

/// Create a minimal org file for testing
pub fn minimal_org_content() -> String {
    format!(
        r#"#+GH_REPO: {}

* TODO Simple task
Task body
"#,
        TEST_REPO
    )
}
