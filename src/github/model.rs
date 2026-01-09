use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhIssue {
    pub number: u64,
    pub title: String,
    pub body: Option<String>,
    pub state: GhIssueState,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub closed_at: Option<DateTime<Utc>>,
    pub html_url: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GhIssueState {
    Open,
    Closed,
}

impl GhIssueState {
    pub fn is_open(&self) -> bool {
        matches!(self, GhIssueState::Open)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GhComment {
    pub id: u64,
    pub author: String,
    pub body: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new issue
#[derive(Debug, Clone)]
pub struct CreateIssueRequest {
    pub title: String,
    pub body: Option<String>,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
}

/// Request to update an existing issue
#[derive(Debug, Clone, Default)]
pub struct UpdateIssueRequest {
    pub title: Option<String>,
    pub body: Option<String>,
    pub state: Option<GhIssueState>,
    pub assignees: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
}
