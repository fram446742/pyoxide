use {
    anyhow::Result,
    clap::{Parser, Subcommand},
    std::{collections::HashMap, path::PathBuf},
};

mod package;
mod pip;
mod runner;

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

    /// Build the self-contained runner against embedded Python resources.
    BuildRunner {
        /// Project directory containing pyoxide.bzl.
        #[arg(long, default_value = ".")]
        path: PathBuf,

        /// Directory to write the executable (and the resources blob) to.
        #[arg(long, default_value = "dist")]
        dest: PathBuf,

        /// Name of the produced executable.
        #[arg(long, default_value = "pyoxide-runner")]
        exe_name: String,

        /// Python version (X.Y) to embed.
        #[arg(long)]
        python_version: Option<String>,

        /// Space separated requirements to embed, e.g. "requests pyyaml".
        #[arg(long)]
        requirements: Option<String>,

        /// Ship the packed resources as a file next to the executable.
        #[arg(long)]
        split_resources: bool,

        /// Build the runner with optimizations.
        #[arg(long)]
        release: bool,

        /// Rust target triple to use.
        #[arg(long, default_value_t = pyoxidizerlib::environment::default_target_triple().to_string())]
        target_triple: String,

        /// Print what the embedding step does.
        #[arg(long, short)]
        verbose: bool,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::DistInfo { archive } => {
            let env = pyoxidizerlib::environment::Environment::new()?;
            pyoxidizerlib::projectmgmt::python_distribution_info(&env, &archive)
        }

        Command::BuildRunner {
            path,
            dest,
            exe_name,
            python_version,
            requirements,
            split_resources,
            release,
            target_triple,
            verbose,
        } => {
            let env = pyoxidizerlib::environment::Environment::new()?;

            let mut vars: HashMap<String, Option<String>> = HashMap::new();
            if let Some(version) = python_version {
                vars.insert("PYTHON_VERSION".to_string(), Some(version));
            }
            if let Some(requirements) = requirements {
                vars.insert("REQUIREMENTS".to_string(), Some(requirements));
            }

            runner::build_runner(
                &env,
                &path,
                &dest,
                &exe_name,
                Some(&target_triple),
                vars,
                release,
                split_resources,
                verbose,
            )
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
