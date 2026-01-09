use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Represents a parsed org file with its syncable items
#[derive(Debug, Clone)]
pub struct OrgFile {
    /// Path to the org file
    pub path: std::path::PathBuf,
    /// GitHub repository (from #+GH_REPO:)
    pub repo: Option<String>,
    /// Raw content for writing back
    pub content: String,
    /// Syncable headings (those with TODO keywords)
    pub items: Vec<OrgItem>,
}

/// A syncable org heading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgItem {
    /// Stable identifier (CUSTOM_ID or slugified heading)
    pub id: String,
    /// Heading text (without TODO keyword)
    pub title: String,
    /// Body content (everything before first subheading)
    pub body: String,
    /// TODO state
    pub state: TodoState,
    /// GitHub issue number (from :GH_ISSUE:)
    pub gh_issue: Option<u64>,
    /// GitHub issue URL (from :GH_URL:)
    pub gh_url: Option<String>,
    /// Assignees (from :ASSIGNEE:, comma-separated)
    pub assignees: Vec<String>,
    /// Labels (from :LABELS:, comma-separated)
    pub labels: Vec<String>,
    /// When the item was created
    pub created: Option<DateTime<Utc>>,
    /// Last update timestamp
    pub updated: Option<DateTime<Utc>>,
    /// Byte range in the original content (for modifications)
    pub span: std::ops::Range<usize>,
    /// Property drawer byte range (for property updates)
    pub properties_span: Option<std::ops::Range<usize>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TodoState {
    /// Open, not started (TODO)
    Todo,
    /// In progress (DOING)
    Doing,
    /// Blocked by something (BLOCKED)
    Blocked,
    /// Waiting for external input (WAITING)
    Waiting,
    /// Completed (DONE)
    Done,
    /// Cancelled/won't do (CANCELLED, WONTFIX)
    Cancelled,
}

impl TodoState {
    pub fn is_open(&self) -> bool {
        matches!(
            self,
            TodoState::Todo | TodoState::Doing | TodoState::Blocked | TodoState::Waiting
        )
    }

    pub fn is_closed(&self) -> bool {
        matches!(self, TodoState::Done | TodoState::Cancelled)
    }

    pub fn from_keyword(keyword: &str) -> Option<Self> {
        match keyword.to_uppercase().as_str() {
            "TODO" => Some(TodoState::Todo),
            "DOING" => Some(TodoState::Doing),
            "BLOCKED" => Some(TodoState::Blocked),
            "WAITING" => Some(TodoState::Waiting),
            "DONE" => Some(TodoState::Done),
            "CANCELLED" | "CANCELED" | "WONTFIX" => Some(TodoState::Cancelled),
            _ => None,
        }
    }

    pub fn to_keyword(&self) -> &'static str {
        match self {
            TodoState::Todo => "TODO",
            TodoState::Doing => "DOING",
            TodoState::Blocked => "BLOCKED",
            TodoState::Waiting => "WAITING",
            TodoState::Done => "DONE",
            TodoState::Cancelled => "CANCELLED",
        }
    }
}
