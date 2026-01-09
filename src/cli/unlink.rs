use crate::error::{Error, Result};
use crate::org::{parse_file, write_file};
use crate::output::{format, Format, UnlinkOutput};
use crate::sync::SyncState;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to org file
    pub file: PathBuf,

    /// Heading text or issue number to unlink
    pub target: String,

    /// Also close the issue on GitHub
    #[arg(long)]
    pub close: bool,
}

pub async fn run(args: Args, output_format: Format) -> Result<()> {
    // Parse org file
    let mut org_file = parse_file(&args.file)?;

    // Load sync state
    let mut state = SyncState::load(&args.file)?;

    // Find the item to unlink - either by title or by issue number
    let target_issue_num: Option<u64> = args.target.parse().ok();

    let item = org_file.items.iter().find(|item| {
        if let Some(num) = target_issue_num {
            item.gh_issue == Some(num)
        } else {
            item.title
                .to_lowercase()
                .contains(&args.target.to_lowercase())
        }
    });

    let item = match item {
        Some(item) => item.clone(),
        None => {
            return Err(Error::Config(format!(
                "No item found matching '{}'",
                args.target
            )));
        }
    };

    let issue_num = match item.gh_issue {
        Some(num) => num,
        None => {
            if output_format == Format::Human {
                println!("Item '{}' is not linked to a GitHub issue", item.title);
            }
            return Ok(());
        }
    };

    let is_human = output_format == Format::Human;

    if is_human {
        println!("Unlinking '{}' from issue #{}", item.title, issue_num);
    }

    // Close the issue if requested
    if args.close {
        let repo = org_file.repo.clone().ok_or(Error::NoRepo)?;

        let config = crate::config::Config::load()?;
        let token = config.github_token()?;
        let client = crate::github::GitHubClient::new(&token, &repo).await?;

        use crate::github::model::{GhIssueState, UpdateIssueRequest};
        let req = UpdateIssueRequest {
            title: None,
            body: None,
            state: Some(GhIssueState::Closed),
            assignees: None,
            labels: None,
        };
        client.update_issue(issue_num, req).await?;
        if is_human {
            println!("  Closed issue #{} on GitHub", issue_num);
        }
    } else if is_human {
        println!("  Issue #{} remains open on GitHub", issue_num);
    }

    // Remove GH_ISSUE and GH_URL properties from org file
    org_file.content = remove_property(&org_file.content, &item, "GH_ISSUE");
    org_file.content = remove_property(&org_file.content, &item, "GH_URL");

    // Remove from sync state
    state.remove(issue_num);

    // Save changes
    write_file(&org_file)?;
    state.save(&args.file)?;

    if is_human {
        println!("  Removed sync link from org file");
        println!("\nUnlink complete.");
    } else {
        let output = UnlinkOutput {
            title: item.title,
            issue_number: issue_num,
            closed: args.close,
        };
        print!("{}", format(&output, output_format));
    }

    Ok(())
}

/// Remove a property from an item's property drawer
fn remove_property(content: &str, item: &crate::org::model::OrgItem, key: &str) -> String {
    if let Some(ref props_span) = item.properties_span {
        let before = &content[..props_span.start];
        let drawer = &content[props_span.start..props_span.end];
        let after = &content[props_span.end..];

        let key_upper = key.to_uppercase();
        let new_drawer: String = drawer
            .lines()
            .filter(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with(':')
                    && !trimmed.starts_with(":END:")
                    && !trimmed.starts_with(":PROPERTIES:")
                {
                    if let Some(colon_pos) = trimmed[1..].find(':') {
                        let prop_key = &trimmed[1..colon_pos + 1];
                        return prop_key.to_uppercase() != key_upper;
                    }
                }
                true
            })
            .collect::<Vec<_>>()
            .join("\n");

        format!("{}{}{}", before, new_drawer, after)
    } else {
        content.to_string()
    }
}
