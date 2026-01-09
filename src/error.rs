use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to parse org file: {0}")]
    OrgParse(String),

    #[error("Failed to write org file: {0}")]
    OrgWrite(String),

    #[error("GitHub API error: {0}")]
    GitHub(#[from] octocrab::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("TOML parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Sync conflict on issue #{issue}: {field} changed in both org and GitHub")]
    Conflict { issue: u64, field: String },

    #[error("No GitHub repository configured for this file")]
    NoRepo,

    #[error("GitHub authentication failed: {0}")]
    Auth(String),

    #[error("Heading not found: {0}")]
    HeadingNotFound(String),

    #[error("Issue not found: #{0}")]
    IssueNotFound(u64),
}

pub type Result<T> = std::result::Result<T, Error>;
