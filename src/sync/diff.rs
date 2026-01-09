use crate::github::model::GhIssue;
use crate::org::model::OrgItem;
use crate::sync::state::{hash_body, SyncedItem};

/// Changes detected for a single field
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldChange {
    /// No change
    None,
    /// Only org changed
    OrgChanged,
    /// Only GitHub changed
    GitHubChanged,
    /// Both changed (conflict)
    Conflict,
}

/// Result of comparing org item, GitHub issue, and base state
#[derive(Debug, Clone)]
pub struct DiffResult {
    pub title: FieldChange,
    pub body: FieldChange,
    pub state: FieldChange,
    pub assignees: FieldChange,
    pub labels: FieldChange,
}

impl DiffResult {
    pub fn has_conflicts(&self) -> bool {
        self.title == FieldChange::Conflict
            || self.body == FieldChange::Conflict
            || self.state == FieldChange::Conflict
            || self.assignees == FieldChange::Conflict
    }

    pub fn has_changes(&self) -> bool {
        self.title != FieldChange::None
            || self.body != FieldChange::None
            || self.state != FieldChange::None
            || self.assignees != FieldChange::None
            || self.labels != FieldChange::None
    }

    pub fn conflict_fields(&self) -> Vec<&'static str> {
        let mut fields = Vec::new();
        if self.title == FieldChange::Conflict {
            fields.push("title");
        }
        if self.body == FieldChange::Conflict {
            fields.push("body");
        }
        if self.state == FieldChange::Conflict {
            fields.push("state");
        }
        if self.assignees == FieldChange::Conflict {
            fields.push("assignees");
        }
        if self.labels == FieldChange::Conflict {
            fields.push("labels");
        }
        fields
    }
}

/// Compute three-way diff between org item, GitHub issue, and base state
pub fn three_way_diff(org: &OrgItem, gh: &GhIssue, base: &SyncedItem) -> DiffResult {
    DiffResult {
        title: diff_string(&org.title, &gh.title, &base.title),
        body: diff_body(&org.body, gh.body.as_deref().unwrap_or(""), &base.body_hash),
        state: diff_state(org, gh, base),
        assignees: diff_vec(&org.assignees, &gh.assignees, &base.assignees),
        labels: diff_vec(&org.labels, &gh.labels, &base.labels),
    }
}

fn diff_string(org: &str, gh: &str, base: &str) -> FieldChange {
    let org_changed = org != base;
    let gh_changed = gh != base;

    match (org_changed, gh_changed) {
        (false, false) => FieldChange::None,
        (true, false) => FieldChange::OrgChanged,
        (false, true) => FieldChange::GitHubChanged,
        (true, true) => {
            if org == gh {
                // Both changed to same value - no conflict
                FieldChange::OrgChanged
            } else {
                FieldChange::Conflict
            }
        }
    }
}

fn diff_body(org_body: &str, gh_body: &str, base_hash: &str) -> FieldChange {
    let org_hash = hash_body(org_body);
    let gh_hash = hash_body(gh_body);

    let org_changed = org_hash != base_hash;
    let gh_changed = gh_hash != base_hash;

    match (org_changed, gh_changed) {
        (false, false) => FieldChange::None,
        (true, false) => FieldChange::OrgChanged,
        (false, true) => FieldChange::GitHubChanged,
        (true, true) => {
            if org_hash == gh_hash {
                FieldChange::OrgChanged
            } else {
                FieldChange::Conflict
            }
        }
    }
}

fn diff_state(org: &OrgItem, gh: &GhIssue, base: &SyncedItem) -> FieldChange {
    let org_state = if org.state.is_open() {
        "open"
    } else {
        "closed"
    };
    let gh_state = if gh.state.is_open() { "open" } else { "closed" };

    diff_string(org_state, gh_state, &base.state)
}

fn diff_vec(org: &[String], gh: &[String], base: &[String]) -> FieldChange {
    let org_changed = !vec_eq(org, base);
    let gh_changed = !vec_eq(gh, base);

    match (org_changed, gh_changed) {
        (false, false) => FieldChange::None,
        (true, false) => FieldChange::OrgChanged,
        (false, true) => FieldChange::GitHubChanged,
        (true, true) => {
            if vec_eq(org, gh) {
                FieldChange::OrgChanged
            } else {
                // For labels/assignees, we can often auto-merge (union)
                // but report as conflict for now - engine decides
                FieldChange::Conflict
            }
        }
    }
}

fn vec_eq(a: &[String], b: &[String]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut a_sorted: Vec<_> = a.iter().collect();
    let mut b_sorted: Vec<_> = b.iter().collect();
    a_sorted.sort();
    b_sorted.sort();
    a_sorted == b_sorted
}

/// Merge two label lists (union)
pub fn merge_labels(org: &[String], gh: &[String]) -> Vec<String> {
    let mut result: Vec<String> = org.to_vec();
    for label in gh {
        if !result.iter().any(|l| l == label) {
            result.push(label.clone());
        }
    }
    result.sort();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_diff_string_no_change() {
        assert_eq!(diff_string("hello", "hello", "hello"), FieldChange::None);
    }

    #[test]
    fn test_diff_string_org_changed() {
        assert_eq!(
            diff_string("new", "hello", "hello"),
            FieldChange::OrgChanged
        );
    }

    #[test]
    fn test_diff_string_gh_changed() {
        assert_eq!(
            diff_string("hello", "new", "hello"),
            FieldChange::GitHubChanged
        );
    }

    #[test]
    fn test_diff_string_conflict() {
        assert_eq!(
            diff_string("org_new", "gh_new", "hello"),
            FieldChange::Conflict
        );
    }

    #[test]
    fn test_merge_labels() {
        let org = vec!["a".to_string(), "b".to_string()];
        let gh = vec!["b".to_string(), "c".to_string()];
        let merged = merge_labels(&org, &gh);
        assert_eq!(merged, vec!["a", "b", "c"]);
    }
}
