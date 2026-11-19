use {
    anyhow::Result,
    clap::{Parser, Subcommand},
    std::path::PathBuf,
};

/// Package self-contained Python applications.
#[derive(Parser)]
#[command(name = "pyoxide", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the contents of a Python distribution archive.
    DistInfo {
        /// Python distribution `.tar.zst` archive.
        archive: PathBuf,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::DistInfo { archive } => {
            let env = pyoxidizerlib::environment::Environment::new()?;
            pyoxidizerlib::projectmgmt::python_distribution_info(&env, &archive)
        }
    }
}
