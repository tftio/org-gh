use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub github: GitHubConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub org: OrgConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GitHubConfig {
    pub token: Option<String>,
    pub default_repo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    pub doing_label: String,
    pub blocked_label: String,
    #[serde(default)]
    pub default_labels: Vec<String>,
    pub title_conflict: ConflictResolution,
    pub body_conflict: ConflictResolution,
    pub state_conflict: ConflictResolution,
    pub assignee_conflict: ConflictResolution,
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            doing_label: "in-progress".to_string(),
            blocked_label: "blocked".to_string(),
            default_labels: vec![],
            title_conflict: ConflictResolution::OrgWins,
            body_conflict: ConflictResolution::OrgWins,
            state_conflict: ConflictResolution::Prompt,
            assignee_conflict: ConflictResolution::GitHubWins,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrgConfig {
    pub todo_keywords: Vec<String>,
    pub done_keywords: Vec<String>,
}

impl Default for OrgConfig {
    fn default() -> Self {
        Self {
            todo_keywords: vec![
                "TODO".to_string(),
                "DOING".to_string(),
                "BLOCKED".to_string(),
                "WAITING".to_string(),
            ],
            done_keywords: vec![
                "DONE".to_string(),
                "CANCELLED".to_string(),
                "WONTFIX".to_string(),
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictResolution {
    Prompt,
    OrgWins,
    GitHubWins,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            Ok(Self::default())
        }
    }

    pub fn config_path() -> Result<PathBuf> {
        let dirs = directories::ProjectDirs::from("", "", "org-gh")
            .ok_or_else(|| Error::Config("Could not determine config directory".into()))?;
        Ok(dirs.config_dir().join("config.toml"))
    }

    /// Get GitHub token from (in order): env var, gh CLI, config file
    pub fn github_token(&self) -> Result<String> {
        // 1. Environment variable
        if let Ok(token) = std::env::var("GITHUB_TOKEN") {
            return Ok(token);
        }

        // 2. gh CLI
        if let Ok(output) = std::process::Command::new("gh")
            .args(["auth", "token"])
            .output()
        {
            if output.status.success() {
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !token.is_empty() {
                    return Ok(token);
                }
            }
        }

        // 3. Config file
        if let Some(token) = &self.github.token {
            return Ok(token.clone());
        }

        Err(Error::Auth(
            "No GitHub token found. Set GITHUB_TOKEN, run 'gh auth login', or add token to config"
                .into(),
        ))
    }
}
