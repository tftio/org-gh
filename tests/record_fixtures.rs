//! One-time recording script for API fixtures
//! Run with: cargo test --test record_fixtures -- --ignored --nocapture
//!
//! This fetches real responses from tftio/org-gh-test-fixture and saves them
//! as JSON files for wiremock replay.

use std::fs;
use std::path::Path;

const FIXTURE_REPO: &str = "tftio/org-gh-test-fixture";
const FIXTURES_DIR: &str = "tests/fixtures";

#[tokio::test]
#[ignore] // Only run manually when re-recording
async fn record_fixtures() {
    let token = std::env::var("GITHUB_TOKEN")
        .or_else(|_| get_gh_token())
        .expect("Set GITHUB_TOKEN or have gh CLI configured");

    let client = octocrab::Octocrab::builder()
        .personal_token(token)
        .build()
        .expect("Failed to build octocrab client");

    let fixtures_path = Path::new(FIXTURES_DIR);
    fs::create_dir_all(fixtures_path).expect("Failed to create fixtures dir");

    // Record: GET /repos/{owner}/{repo}/issues?state=open
    println!("Recording open issues...");
    let open_issues: serde_json::Value = client
        .get(
            format!("/repos/{}/issues?state=open&per_page=100", FIXTURE_REPO),
            None::<&()>,
        )
        .await
        .expect("Failed to fetch open issues");
    write_fixture(fixtures_path, "issues_open.json", &open_issues);

    // Record: GET /repos/{owner}/{repo}/issues?state=closed
    println!("Recording closed issues...");
    let closed_issues: serde_json::Value = client
        .get(
            format!("/repos/{}/issues?state=closed&per_page=100", FIXTURE_REPO),
            None::<&()>,
        )
        .await
        .expect("Failed to fetch closed issues");
    write_fixture(fixtures_path, "issues_closed.json", &closed_issues);

    // Record: GET /repos/{owner}/{repo}/issues/1
    println!("Recording issue #1...");
    let issue_1: serde_json::Value = client
        .get(format!("/repos/{}/issues/1", FIXTURE_REPO), None::<&()>)
        .await
        .expect("Failed to fetch issue 1");
    write_fixture(fixtures_path, "issue_1.json", &issue_1);

    // Record: GET /repos/{owner}/{repo}/issues/2
    println!("Recording issue #2...");
    let issue_2: serde_json::Value = client
        .get(format!("/repos/{}/issues/2", FIXTURE_REPO), None::<&()>)
        .await
        .expect("Failed to fetch issue 2");
    write_fixture(fixtures_path, "issue_2.json", &issue_2);

    // Record: GET /repos/{owner}/{repo}/issues/3
    println!("Recording issue #3...");
    let issue_3: serde_json::Value = client
        .get(format!("/repos/{}/issues/3", FIXTURE_REPO), None::<&()>)
        .await
        .expect("Failed to fetch issue 3");
    write_fixture(fixtures_path, "issue_3.json", &issue_3);

    // Record: GET /repos/{owner}/{repo}/issues/5/comments
    println!("Recording comments for issue #5...");
    let comments: serde_json::Value = client
        .get(
            format!("/repos/{}/issues/5/comments?per_page=100", FIXTURE_REPO),
            None::<&()>,
        )
        .await
        .expect("Failed to fetch comments");
    write_fixture(fixtures_path, "issue_5_comments.json", &comments);

    // Record: GET /repos/{owner}/{repo}/labels
    println!("Recording labels...");
    let labels: serde_json::Value = client
        .get(format!("/repos/{}/labels", FIXTURE_REPO), None::<&()>)
        .await
        .expect("Failed to fetch labels");
    write_fixture(fixtures_path, "labels.json", &labels);

    println!("\nFixtures recorded to {}/", FIXTURES_DIR);
}

fn write_fixture(dir: &Path, name: &str, value: &serde_json::Value) {
    let path = dir.join(name);
    let content = serde_json::to_string_pretty(value).expect("Failed to serialize");
    fs::write(&path, content).expect("Failed to write fixture");
    println!("  Wrote {}", path.display());
}

fn get_gh_token() -> Result<String, std::env::VarError> {
    use std::process::Command;
    let output = Command::new("gh")
        .args(["auth", "token"])
        .output()
        .map_err(|_| std::env::VarError::NotPresent)?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(std::env::VarError::NotPresent)
    }
}
