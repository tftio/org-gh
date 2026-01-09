use crate::config::Config;
use crate::error::{Error, Result};
use crate::github::GitHubClient;
use crate::output::{format, Format, InitOutput};
use crate::sync::SyncState;
use clap::Args as ClapArgs;
use std::path::PathBuf;

#[derive(ClapArgs)]
pub struct Args {
    /// Path to org file
    pub file: PathBuf,

    /// GitHub repository (owner/repo)
    #[arg(long, short)]
    pub repo: String,
}

pub async fn run(args: Args, output_format: Format) -> Result<()> {
    // Validate file exists
    if !args.file.exists() {
        return Err(Error::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("File not found: {}", args.file.display()),
        )));
    }

    // Validate repo format
    if !args.repo.contains('/') || args.repo.split('/').count() != 2 {
        return Err(Error::Config(format!(
            "Invalid repository format: {}. Expected owner/repo",
            args.repo
        )));
    }

    let is_human = output_format == Format::Human;

    if is_human {
        println!(
            "Initializing {} for repo {}",
            args.file.display(),
            args.repo
        );
    }

    // Load config and validate GitHub access
    let config = Config::load()?;
    let token = config.github_token()?;

    if is_human {
        print!("Validating GitHub access... ");
    }
    let client = GitHubClient::new(&token, &args.repo).await?;

    // Verify repo exists by fetching issues (will error if no access)
    match client.fetch_issues().await {
        Ok(_) => {
            if is_human {
                println!("OK");
            }
        }
        Err(e) => {
            if is_human {
                println!("FAILED");
            }
            return Err(Error::Auth(format!(
                "Cannot access repository {}: {}",
                args.repo, e
            )));
        }
    }

    // Read file and check for existing GH_REPO header
    let content = std::fs::read_to_string(&args.file)?;
    let has_repo_header = content
        .lines()
        .any(|line| line.trim().to_uppercase().starts_with("#+GH_REPO:"));

    let initialized = if has_repo_header {
        if is_human {
            println!("File already has #+GH_REPO: header");
        }
        false
    } else {
        // Add GH_REPO header at the top (after any existing #+TITLE: line)
        let new_content = add_repo_header(&content, &args.repo);
        std::fs::write(&args.file, new_content)?;
        if is_human {
            println!("Added #+GH_REPO: {} header", args.repo);
        }
        true
    };

    // Create sync state file
    let state = SyncState::new(&args.repo);
    state.save(&args.file)?;

    if is_human {
        let state_path = SyncState::state_path(&args.file);
        println!("Created sync state: {}", state_path.display());
        println!(
            "\nInitialization complete. Run 'org-gh sync {}' to sync.",
            args.file.display()
        );
    } else {
        let output = InitOutput {
            file: args.file.display().to_string(),
            repo: args.repo,
            initialized,
        };
        print!("{}", format(&output, output_format));
    }

    Ok(())
}

/// Add #+GH_REPO: header to org file content
fn add_repo_header(content: &str, repo: &str) -> String {
    let mut lines: Vec<&str> = content.lines().collect();
    let header_line = format!("#+GH_REPO: {}", repo);

    // Find insertion point: after #+TITLE: if present, otherwise at top
    let insert_pos = lines
        .iter()
        .position(|line| {
            let upper = line.trim().to_uppercase();
            upper.starts_with("#+") && !upper.starts_with("#+TITLE:")
        })
        .unwrap_or(0);

    // If there's a TITLE line, insert after it
    let insert_pos = lines
        .iter()
        .take(insert_pos.saturating_add(5)) // Look in first few lines
        .position(|line| line.trim().to_uppercase().starts_with("#+TITLE:"))
        .map(|p| p + 1)
        .unwrap_or(insert_pos);

    lines.insert(insert_pos, &header_line);

    // Preserve original line endings
    if content.contains("\r\n") {
        lines.join("\r\n")
    } else {
        lines.join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_repo_header_empty() {
        let content = "";
        let result = add_repo_header(content, "owner/repo");
        assert!(result.contains("#+GH_REPO: owner/repo"));
    }

    #[test]
    fn test_add_repo_header_with_title() {
        let content = "#+TITLE: My File\n* Heading";
        let result = add_repo_header(content, "owner/repo");
        assert!(result.starts_with("#+TITLE: My File\n#+GH_REPO: owner/repo"));
    }

    #[test]
    fn test_add_repo_header_no_title() {
        let content = "* Heading\nSome content";
        let result = add_repo_header(content, "owner/repo");
        assert!(result.starts_with("#+GH_REPO: owner/repo"));
    }
}
