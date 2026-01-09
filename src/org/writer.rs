use crate::error::Result;
use crate::org::model::{OrgFile, OrgItem};

/// Write modifications back to an org file
pub fn write_file(file: &OrgFile) -> Result<()> {
    std::fs::write(&file.path, &file.content)?;
    Ok(())
}

/// Update a property in an item's property drawer
/// Returns the new content with the property updated
pub fn set_property(content: &str, item: &OrgItem, key: &str, value: &str) -> String {
    // If item has a property drawer, update or add the property
    if let Some(ref props_span) = item.properties_span {
        let before = &content[..props_span.start];
        let drawer = &content[props_span.start..props_span.end];
        let after = &content[props_span.end..];

        let new_drawer = update_property_in_drawer(drawer, key, value);
        format!("{}{}{}", before, new_drawer, after)
    } else {
        // Need to insert a new property drawer after the headline
        insert_property_drawer(content, item, key, value)
    }
}

/// Update or add a property within an existing drawer
fn update_property_in_drawer(drawer: &str, key: &str, value: &str) -> String {
    let key_upper = key.to_uppercase();
    let mut lines: Vec<&str> = drawer.lines().collect();
    let mut found = false;

    for line in &mut lines {
        if line.trim().starts_with(':') && !line.trim().starts_with(":END:") {
            // Check if this line is our property
            let trimmed = line.trim();
            if let Some(colon_pos) = trimmed[1..].find(':') {
                let prop_key = &trimmed[1..colon_pos + 1];
                if prop_key.to_uppercase() == key_upper {
                    // This allocation is fine since we're rebuilding anyway
                    found = true;
                    // We'll handle this in the join below
                }
            }
        }
    }

    let mut result = Vec::new();
    for line in lines.iter() {
        let trimmed = line.trim();
        if trimmed.starts_with(':')
            && !trimmed.starts_with(":END:")
            && !trimmed.starts_with(":PROPERTIES:")
        {
            if let Some(colon_pos) = trimmed[1..].find(':') {
                let prop_key = &trimmed[1..colon_pos + 1];
                if prop_key.to_uppercase() == key_upper {
                    // Replace this property
                    result.push(format!(":{}: {}", key_upper, value));
                    continue;
                }
            }
        }

        if trimmed == ":END:" && !found {
            // Insert new property before :END:
            result.push(format!(":{}: {}", key_upper, value));
        }

        result.push((*line).to_string());
    }

    result.join("\n")
}

/// Insert a new property drawer after a headline
fn insert_property_drawer(content: &str, item: &OrgItem, key: &str, value: &str) -> String {
    // Find the end of the headline line
    let headline_start = item.span.start;
    let headline_end = content[headline_start..]
        .find('\n')
        .map(|i| headline_start + i)
        .unwrap_or(content.len());

    let before = &content[..headline_end];
    let after = &content[headline_end..];

    let drawer = format!("\n:PROPERTIES:\n:{}: {}\n:END:", key.to_uppercase(), value);

    format!("{}{}{}", before, drawer, after)
}

/// Update the TODO state keyword for an item
pub fn set_todo_state(content: &str, item: &OrgItem, new_state: &str) -> String {
    // Find the headline line and replace the TODO keyword
    let headline_start = item.span.start;
    let headline_end = content[headline_start..]
        .find('\n')
        .map(|i| headline_start + i)
        .unwrap_or(content.len());

    let headline = &content[headline_start..headline_end];

    // Find and replace the TODO keyword
    let old_keyword = item.state.to_keyword();
    if let Some(pos) = headline.find(old_keyword) {
        let before = &content[..headline_start + pos];
        let after = &content[headline_start + pos + old_keyword.len()..];
        format!("{}{}{}", before, new_state, after)
    } else {
        content.to_string()
    }
}

/// Append an entry to the LOGBOOK drawer
pub fn append_to_logbook(content: &str, item: &OrgItem, entry: &str) -> String {
    // Find existing LOGBOOK drawer or insert new one
    let section_start = item
        .properties_span
        .as_ref()
        .map(|s| s.end)
        .unwrap_or(item.span.start);

    let search_area = &content[section_start..item.span.end.min(content.len())];

    if let Some(logbook_start) = search_area.find(":LOGBOOK:") {
        // Find :END: after :LOGBOOK:
        let abs_start = section_start + logbook_start;
        if let Some(end_pos) = content[abs_start..].find(":END:") {
            let insert_pos = abs_start + end_pos;
            let before = &content[..insert_pos];
            let after = &content[insert_pos..];
            return format!("{}{}\n{}", before, entry, after);
        }
    }

    // No LOGBOOK found, create one after properties
    let insert_pos = item
        .properties_span
        .as_ref()
        .map(|s| s.end)
        .unwrap_or_else(|| {
            // After headline line
            content[item.span.start..]
                .find('\n')
                .map(|i| item.span.start + i + 1)
                .unwrap_or(item.span.end)
        });

    let before = &content[..insert_pos];
    let after = &content[insert_pos..];

    format!("{}:LOGBOOK:\n{}\n:END:\n{}", before, entry, after)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::org::model::TodoState;

    #[allow(dead_code)]
    fn make_test_item() -> OrgItem {
        OrgItem {
            id: "test".to_string(),
            title: "Test item".to_string(),
            body: String::new(),
            state: TodoState::Todo,
            gh_issue: None,
            gh_url: None,
            assignees: vec![],
            labels: vec![],
            created: None,
            updated: None,
            span: 0..50,
            properties_span: Some(20..45),
        }
    }

    #[test]
    fn test_update_property_in_drawer() {
        let drawer = ":PROPERTIES:\n:GH_ISSUE: 42\n:END:";
        let result = update_property_in_drawer(drawer, "GH_ISSUE", "99");
        assert!(result.contains(":GH_ISSUE: 99"));
    }

    #[test]
    fn test_add_property_to_drawer() {
        let drawer = ":PROPERTIES:\n:END:";
        let result = update_property_in_drawer(drawer, "GH_ISSUE", "42");
        assert!(result.contains(":GH_ISSUE: 42"));
    }
}
