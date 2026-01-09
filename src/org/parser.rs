use crate::error::Result;
use crate::org::model::{OrgFile, OrgItem, TodoState};
use orgize::ast::Headline;
use orgize::export::{Container, Event, TraversalContext, Traverser};
use orgize::rowan::ast::AstNode;
use orgize::Org;
use std::path::Path;

/// Parse an org file and extract syncable items
pub fn parse_file(path: &Path) -> Result<OrgFile> {
    let content = std::fs::read_to_string(path)?;
    parse_content(path, &content)
}

/// Parse org content string
pub fn parse_content(path: &Path, content: &str) -> Result<OrgFile> {
    let org = Org::parse(content);

    // Extract file-level properties
    let repo = extract_file_property(&org, "GH_REPO");

    // Extract syncable items (headings with TODO keywords)
    let items = extract_items(&org, content);

    Ok(OrgFile {
        path: path.to_path_buf(),
        repo,
        content: content.to_string(),
        items,
    })
}

/// Extract a file-level property (#+KEY: value)
fn extract_file_property(org: &Org, key: &str) -> Option<String> {
    // Use traverse to find keywords at document level
    struct KeywordFinder<'a> {
        key: &'a str,
        result: Option<String>,
    }

    impl Traverser for KeywordFinder<'_> {
        fn event(&mut self, event: Event, _ctx: &mut TraversalContext) {
            if let Event::Enter(Container::Keyword(kw)) = event {
                let kw_key: &str = &kw.key();
                if kw_key.to_uppercase() == self.key.to_uppercase() {
                    let value: &str = &kw.value();
                    self.result = Some(value.trim().to_string());
                }
            }
        }
    }

    let mut finder = KeywordFinder { key, result: None };
    org.traverse(&mut finder);
    finder.result
}

/// Extract all syncable items from the org document
fn extract_items(org: &Org, content: &str) -> Vec<OrgItem> {
    let mut items = Vec::new();

    struct HeadlineCollector<'a> {
        content: &'a str,
        items: &'a mut Vec<OrgItem>,
    }

    impl Traverser for HeadlineCollector<'_> {
        fn event(&mut self, event: Event, _ctx: &mut TraversalContext) {
            if let Event::Enter(Container::Headline(headline)) = event {
                if let Some(todo_kw) = headline.todo_keyword() {
                    // Token derefs to str
                    let kw_text: &str = &todo_kw;
                    if let Some(state) = TodoState::from_keyword(kw_text) {
                        if let Some(item) = parse_headline(&headline, state, self.content) {
                            self.items.push(item);
                        }
                    }
                }
            }
        }
    }

    let mut collector = HeadlineCollector {
        content,
        items: &mut items,
    };
    org.traverse(&mut collector);
    items
}

/// Parse a single headline into an OrgItem
fn parse_headline(headline: &Headline, state: TodoState, content: &str) -> Option<OrgItem> {
    // title() returns an iterator of syntax elements, collect to string
    let title: String = headline
        .title()
        .map(|t| t.to_string())
        .collect::<String>()
        .trim()
        .to_string();

    // Generate stable ID from CUSTOM_ID property or slugified title
    let id = get_property(headline, "CUSTOM_ID").unwrap_or_else(|| slugify(&title));

    // Extract properties
    let gh_issue = get_property(headline, "GH_ISSUE").and_then(|s| s.parse().ok());
    let gh_url = get_property(headline, "GH_URL");
    let assignees = get_property(headline, "ASSIGNEE")
        .map(|s| s.split(',').map(|a| a.trim().to_string()).collect())
        .unwrap_or_default();
    let labels = get_property(headline, "LABELS")
        .map(|s| s.split(',').map(|l| l.trim().to_string()).collect())
        .unwrap_or_default();
    let created = get_property(headline, "CREATED").and_then(|s| parse_datetime(&s));
    let updated = get_property(headline, "UPDATED").and_then(|s| parse_datetime(&s));

    // Get text range for the headline
    let range = headline.syntax().text_range();
    let span = usize::from(range.start())..usize::from(range.end());

    // Extract body (section content)
    let body = extract_body(headline, content);

    // Get property drawer span
    let properties_span = headline.properties().map(|props| {
        let pr = props.syntax().text_range();
        usize::from(pr.start())..usize::from(pr.end())
    });

    Some(OrgItem {
        id,
        title,
        body,
        state,
        gh_issue,
        gh_url,
        assignees,
        labels,
        created,
        updated,
        span,
        properties_span,
    })
}

/// Get a property value from a headline's property drawer
fn get_property(headline: &Headline, key: &str) -> Option<String> {
    let drawer = headline.properties()?;
    for prop in drawer.iter() {
        // Property key and value are Tokens (deref to str)
        let prop_key: &str = &prop.0;
        if prop_key.to_uppercase() == key.to_uppercase() {
            return Some(prop.1.trim().to_string());
        }
    }
    None
}

/// Extract body content from a headline section
fn extract_body(headline: &Headline, content: &str) -> String {
    if let Some(section) = headline.section() {
        let range = section.syntax().text_range();
        let start = usize::from(range.start());
        let end = usize::from(range.end());
        if end <= content.len() {
            return content[start..end].trim().to_string();
        }
    }
    String::new()
}

/// Convert a title to a URL-safe slug
fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

/// Parse various datetime formats
fn parse_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::{NaiveDate, NaiveDateTime, TimeZone, Utc};

    // Try ISO 8601 with timezone
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }

    // Try ISO 8601 without timezone
    if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S") {
        return Some(Utc.from_utc_datetime(&dt));
    }

    // Try date only
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        if let Some(dt) = d.and_hms_opt(0, 0, 0) {
            return Some(Utc.from_utc_datetime(&dt));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Hello World"), "hello-world");
        assert_eq!(
            slugify("Add user authentication!"),
            "add-user-authentication"
        );
        assert_eq!(slugify("  Multiple   Spaces  "), "multiple-spaces");
    }

    #[test]
    fn test_parse_simple_org() {
        let content = r#"#+TITLE: Test
#+GH_REPO: owner/repo

* TODO First task
Some body content.

* DONE Completed task
"#;
        let file = parse_content(Path::new("test.org"), content).unwrap();
        assert_eq!(file.repo, Some("owner/repo".to_string()));
        assert_eq!(file.items.len(), 2);
        assert_eq!(file.items[0].title, "First task");
        assert_eq!(file.items[0].state, TodoState::Todo);
        assert_eq!(file.items[1].state, TodoState::Done);
    }
}
