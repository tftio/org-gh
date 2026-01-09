//! Output formatting for CLI commands
//!
//! Supports three output formats:
//! - Human-readable (default)
//! - S-expressions (for Emacs/elisp)
//! - JSON (for other scripting)

use serde::Serialize;
use std::fmt::Write;

/// Output format for CLI commands
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Format {
    #[default]
    Human,
    Sexp,
    Json,
}

/// Trait for types that can be output in multiple formats
pub trait Output: Serialize {
    /// Human-readable output
    fn human(&self) -> String;
}

/// Format a value according to the specified format
pub fn format<T: Output>(value: &T, format: Format) -> String {
    match format {
        Format::Human => value.human(),
        Format::Sexp => to_sexp(value),
        Format::Json => serde_json::to_string_pretty(value).unwrap_or_else(|_| "null".to_string()),
    }
}

/// Convert a serializable value to s-expression format
pub fn to_sexp<T: Serialize>(value: &T) -> String {
    let json = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
    json_to_sexp(&json)
}

fn json_to_sexp(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => "nil".to_string(),
        serde_json::Value::Bool(b) => if *b { "t" } else { "nil" }.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => format!("\"{}\"", escape_string(s)),
        serde_json::Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(json_to_sexp).collect();
            format!("({})", items.join(" "))
        }
        serde_json::Value::Object(obj) => {
            let mut result = String::from("(");
            let mut first = true;
            for (key, val) in obj {
                if !first {
                    result.push_str("\n ");
                }
                first = false;
                // Convert snake_case to kebab-case for elisp conventions
                let elisp_key = key.replace('_', "-");
                write!(result, "({} . {})", elisp_key, json_to_sexp(val)).unwrap();
            }
            result.push(')');
            result
        }
    }
}

fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

// ============================================================================
// Output types for each command
// ============================================================================

/// Output for `org-gh init`
#[derive(Debug, Serialize)]
pub struct InitOutput {
    pub file: String,
    pub repo: String,
    pub initialized: bool,
}

impl Output for InitOutput {
    fn human(&self) -> String {
        if self.initialized {
            format!("Initialized {} for repo {}", self.file, self.repo)
        } else {
            format!("File {} already initialized for {}", self.file, self.repo)
        }
    }
}

/// Output for `org-gh status`
#[derive(Debug, Serialize)]
pub struct StatusOutput {
    pub file: String,
    pub repo: String,
    pub last_sync: Option<String>,
    pub synced_count: usize,
    pub pending_creates: Vec<String>,
    pub local_changes: Vec<String>,
    pub remote_changes: Vec<String>,
}

impl Output for StatusOutput {
    fn human(&self) -> String {
        let mut out = String::new();
        writeln!(out, "Repository: {}", self.repo).unwrap();
        writeln!(
            out,
            "Last sync: {}",
            self.last_sync.as_deref().unwrap_or("never")
        )
        .unwrap();
        writeln!(out).unwrap();
        writeln!(out, "Synced items: {}", self.synced_count).unwrap();
        writeln!(
            out,
            "Pending creates: {} (new headings without GH_ISSUE)",
            self.pending_creates.len()
        )
        .unwrap();

        if !self.pending_creates.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "New items to create:").unwrap();
            for item in &self.pending_creates {
                writeln!(out, "  - {}", item).unwrap();
            }
        }

        if !self.local_changes.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "Local changes (not pushed):").unwrap();
            for change in &self.local_changes {
                writeln!(out, "  - {}", change).unwrap();
            }
        }

        if !self.remote_changes.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "Remote changes (not pulled):").unwrap();
            for change in &self.remote_changes {
                writeln!(out, "  - {}", change).unwrap();
            }
        }

        if self.local_changes.is_empty()
            && self.remote_changes.is_empty()
            && self.pending_creates.is_empty()
        {
            writeln!(out).unwrap();
            writeln!(out, "Everything is in sync.").unwrap();
        }

        out
    }
}

/// Output for `org-gh push`
#[derive(Debug, Serialize)]
pub struct PushOutput {
    pub created: Vec<PushItem>,
    pub updated: Vec<PushItem>,
    pub errors: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PushItem {
    pub title: String,
    pub issue_number: u64,
    pub url: String,
    pub action: String, // "created" or "updated"
}

impl Output for PushOutput {
    fn human(&self) -> String {
        let mut out = String::new();
        for item in &self.created {
            writeln!(out, "Created #{}: {}", item.issue_number, item.title).unwrap();
            writeln!(out, "  {}", item.url).unwrap();
        }
        for item in &self.updated {
            writeln!(out, "Updated #{}: {}", item.issue_number, item.title).unwrap();
        }
        for error in &self.errors {
            writeln!(out, "Error: {}", error).unwrap();
        }
        if self.created.is_empty() && self.updated.is_empty() && self.errors.is_empty() {
            writeln!(out, "Nothing to push.").unwrap();
        }
        out
    }
}

/// Output for `org-gh pull`
#[derive(Debug, Serialize)]
pub struct PullOutput {
    pub pulled: Vec<PullItem>,
    pub conflicts: Vec<Conflict>,
}

#[derive(Debug, Serialize)]
pub struct PullItem {
    pub issue_number: u64,
    pub title: String,
    pub changes: Vec<String>,
}

impl Output for PullOutput {
    fn human(&self) -> String {
        let mut out = String::new();
        for item in &self.pulled {
            writeln!(out, "Pulled #{}: {}", item.issue_number, item.title).unwrap();
            for change in &item.changes {
                writeln!(out, "  - {}", change).unwrap();
            }
        }
        if !self.conflicts.is_empty() {
            writeln!(out).unwrap();
            writeln!(out, "Conflicts:").unwrap();
            for conflict in &self.conflicts {
                writeln!(
                    out,
                    "  #{} {}: local='{}' remote='{}'",
                    conflict.issue_number, conflict.field, conflict.local, conflict.remote
                )
                .unwrap();
            }
        }
        if self.pulled.is_empty() && self.conflicts.is_empty() {
            writeln!(out, "Nothing to pull.").unwrap();
        }
        out
    }
}

/// Output for `org-gh sync`
#[derive(Debug, Serialize)]
pub struct SyncOutput {
    pub pushed: Vec<PushItem>,
    pub pulled: Vec<PullItem>,
    pub conflicts: Vec<Conflict>,
}

impl Output for SyncOutput {
    fn human(&self) -> String {
        let mut out = String::new();

        if !self.pushed.is_empty() {
            writeln!(out, "Pushed:").unwrap();
            for item in &self.pushed {
                writeln!(
                    out,
                    "  #{}: {} ({})",
                    item.issue_number, item.title, item.action
                )
                .unwrap();
            }
        }

        if !self.pulled.is_empty() {
            writeln!(out, "Pulled:").unwrap();
            for item in &self.pulled {
                writeln!(out, "  #{}: {}", item.issue_number, item.title).unwrap();
            }
        }

        if !self.conflicts.is_empty() {
            writeln!(out, "Conflicts:").unwrap();
            for conflict in &self.conflicts {
                writeln!(
                    out,
                    "  #{} {}: local='{}' remote='{}'",
                    conflict.issue_number, conflict.field, conflict.local, conflict.remote
                )
                .unwrap();
            }
        }

        if self.pushed.is_empty() && self.pulled.is_empty() && self.conflicts.is_empty() {
            writeln!(out, "Everything is in sync.").unwrap();
        }

        out
    }
}

#[derive(Debug, Serialize)]
pub struct Conflict {
    pub issue_number: u64,
    pub field: String,
    pub local: String,
    pub remote: String,
}

/// Output for `org-gh unlink`
#[derive(Debug, Serialize)]
pub struct UnlinkOutput {
    pub title: String,
    pub issue_number: u64,
    pub closed: bool,
}

impl Output for UnlinkOutput {
    fn human(&self) -> String {
        if self.closed {
            format!("Unlinked and closed #{}: {}", self.issue_number, self.title)
        } else {
            format!("Unlinked #{}: {}", self.issue_number, self.title)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_sexp_simple() {
        #[derive(Serialize)]
        struct Test {
            name: String,
            count: i32,
        }

        let t = Test {
            name: "hello".to_string(),
            count: 42,
        };

        let sexp = to_sexp(&t);
        assert!(sexp.contains("(name . \"hello\")"));
        assert!(sexp.contains("(count . 42)"));
    }

    #[test]
    fn test_to_sexp_snake_to_kebab() {
        #[derive(Serialize)]
        struct Test {
            issue_number: i32,
        }

        let t = Test { issue_number: 1 };
        let sexp = to_sexp(&t);
        assert!(sexp.contains("issue-number"));
        assert!(!sexp.contains("issue_number"));
    }

    #[test]
    fn test_to_sexp_array() {
        let arr = vec!["a", "b", "c"];
        let sexp = to_sexp(&arr);
        assert_eq!(sexp, "(\"a\" \"b\" \"c\")");
    }

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("say \"hi\""), "say \\\"hi\\\"");
        assert_eq!(escape_string("line1\nline2"), "line1\\nline2");
    }
}
