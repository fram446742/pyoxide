use {
    anyhow::Result,
    clap::{Parser, Subcommand},
    std::path::PathBuf,
};

mod package;
mod pip;

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
        archive: String,
    },

    /// Write one packed resources file per Python package.
    Package {
        /// Directory to write the resources to.
        #[arg(long, default_value = ".")]
        dest: PathBuf,

        /// Write each package as a directory of real files instead of a
        /// packed resources file.
        #[arg(long)]
        unpack: bool,

        /// Python distribution flavor.
        #[arg(long, default_value = "standalone")]
        flavor: String,

        /// Python version (X.Y) to use.
        #[arg(long)]
        python_version: Option<String>,

        /// Rust target triple to use.
        #[arg(long, default_value_t = pyoxidizerlib::environment::default_target_triple().to_string())]
        target_triple: String,

        /// Print what is skipped and why.
        #[arg(long, short)]
        verbose: bool,

        /// Requirements pip can resolve, e.g. `requests` or `lxml>=5`.
        #[arg(required = true, num_args = 1.., value_name = "REQ")]
        requirements: Vec<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::DistInfo { archive } => {
            let env = pyoxidizerlib::environment::Environment::new()?;
            pyoxidizerlib::projectmgmt::python_distribution_info(&env, &archive)
        }

        Command::Package {
            dest,
            unpack,
            flavor,
            python_version,
            target_triple,
            verbose,
            requirements,
        } => {
            let env = pyoxidizerlib::environment::Environment::new()?;
            let python_version = python_version.as_deref();

            if unpack {
                package::unpack_wheels(
                    &env,
                    &dest,
                    &requirements,
                    &target_triple,
                    &flavor,
                    python_version,
                    verbose,
                )
            } else {
                package::generate_packed_resources(
                    &env,
                    &dest,
                    &requirements,
                    &target_triple,
                    &flavor,
                    python_version,
                    verbose,
                )
            }
        }
    }
}
