pub mod init;
pub mod pull;
pub mod push;
pub mod status;
pub mod sync;
pub mod unlink;

use crate::output::Format;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "org-gh")]
#[command(about = "Bidirectional sync between org-mode and GitHub Issues")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path to config file
    #[arg(long, global = true)]
    pub config: Option<PathBuf>,

    /// Override GitHub token
    #[arg(long, global = true)]
    pub token: Option<String>,

    /// Suppress non-error output
    #[arg(long, short, global = true)]
    pub quiet: bool,

    /// Output as s-expressions (for Emacs/elisp)
    #[arg(long, global = true)]
    pub sexp: bool,

    /// Output as JSON (for scripting)
    #[arg(long, global = true)]
    pub json: bool,
}

impl Cli {
    /// Get the output format based on flags
    pub fn output_format(&self) -> Format {
        if self.sexp {
            Format::Sexp
        } else if self.json {
            Format::Json
        } else {
            Format::Human
        }
    }
}

#[derive(Subcommand)]
pub enum Command {
    /// Initialize sync for an org file
    Init(init::Args),
    /// Push org changes to GitHub
    Push(push::Args),
    /// Pull GitHub changes to org
    Pull(pull::Args),
    /// Bidirectional sync
    Sync(sync::Args),
    /// Show sync status
    Status(status::Args),
    /// Remove sync link without closing issue
    Unlink(unlink::Args),
}
