use clap::Parser;
use org_gh::cli::{Cli, Command};
use org_gh::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let format = cli.output_format();

    match cli.command {
        Command::Init(args) => org_gh::cli::init::run(args, format).await,
        Command::Push(args) => org_gh::cli::push::run(args, format).await,
        Command::Pull(args) => org_gh::cli::pull::run(args, format).await,
        Command::Sync(args) => org_gh::cli::sync::run(args, format).await,
        Command::Status(args) => org_gh::cli::status::run(args, format).await,
        Command::Unlink(args) => org_gh::cli::unlink::run(args, format).await,
    }
}
