use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Sync state stored alongside the org file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncState {
    pub version: u32,
    pub repo: String,
    pub last_sync: Option<DateTime<Utc>>,
    pub items: HashMap<u64, SyncedItem>,
    #[serde(default)]
    pub pending_creates: Vec<PendingCreate>,
}

/// State of a synced item (last known values from both sides)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedItem {
    pub org_heading_id: String,
    pub title: String,
    pub body_hash: String,
    pub state: String,
    pub assignees: Vec<String>,
    pub labels: Vec<String>,
    pub gh_updated_at: DateTime<Utc>,
    pub org_updated_at: Option<DateTime<Utc>>,
}

/// An org heading pending creation in GitHub
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCreate {
    pub org_heading_id: String,
    pub title: String,
}

impl SyncState {
    pub fn new(repo: &str) -> Self {
        Self {
            version: 1,
            repo: repo.to_string(),
            last_sync: None,
            items: HashMap::new(),
            pending_creates: Vec::new(),
        }
    }

    /// Load sync state from file, or create empty if not exists
    pub fn load(org_path: &Path) -> Result<Self> {
        let state_path = Self::state_path(org_path);
        if state_path.exists() {
            let content = std::fs::read_to_string(&state_path)?;
            Ok(serde_json::from_str(&content)?)
        } else {
            // Return empty state - caller should initialize with repo
            Ok(Self {
                version: 1,
                repo: String::new(),
                last_sync: None,
                items: HashMap::new(),
                pending_creates: Vec::new(),
            })
        }
    }

    /// Save sync state to file
    pub fn save(&self, org_path: &Path) -> Result<()> {
        let state_path = Self::state_path(org_path);
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(state_path, content)?;
        Ok(())
    }

    /// Get the sync state file path for an org file
    pub fn state_path(org_path: &Path) -> PathBuf {
        let mut path = org_path.to_path_buf();
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        path.set_file_name(format!("{}.org-gh.json", file_name));
        path
    }

    /// Record a synced item's current state
    #[allow(clippy::too_many_arguments)]
    pub fn record_sync(
        &mut self,
        issue_number: u64,
        org_heading_id: &str,
        title: &str,
        body: &str,
        state: &str,
        assignees: &[String],
        labels: &[String],
        gh_updated_at: DateTime<Utc>,
    ) {
        let body_hash = hash_body(body);
        self.items.insert(
            issue_number,
            SyncedItem {
                org_heading_id: org_heading_id.to_string(),
                title: title.to_string(),
                body_hash,
                state: state.to_string(),
                assignees: assignees.to_vec(),
                labels: labels.to_vec(),
                gh_updated_at,
                org_updated_at: Some(Utc::now()),
            },
        );
        self.last_sync = Some(Utc::now());
    }

    /// Remove an item from sync state
    pub fn remove(&mut self, issue_number: u64) {
        self.items.remove(&issue_number);
    }

    /// Add a pending create
    pub fn add_pending_create(&mut self, heading_id: &str, title: &str) {
        self.pending_creates.push(PendingCreate {
            org_heading_id: heading_id.to_string(),
            title: title.to_string(),
        });
    }

    /// Remove a pending create by heading ID
    pub fn remove_pending_create(&mut self, heading_id: &str) {
        self.pending_creates
            .retain(|p| p.org_heading_id != heading_id);
    }
}

/// Hash body content for change detection
pub fn hash_body(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());
    format!("sha256:{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_path() {
        let org_path = Path::new("/home/user/notes/roadmap.org");
        let state_path = SyncState::state_path(org_path);
        assert_eq!(
            state_path,
            PathBuf::from("/home/user/notes/roadmap.org.org-gh.json")
        );
    }

    #[test]
    fn test_hash_body() {
        let hash1 = hash_body("Hello world");
        let hash2 = hash_body("Hello world");
        let hash3 = hash_body("Different content");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
        assert!(hash1.starts_with("sha256:"));
    }
}
