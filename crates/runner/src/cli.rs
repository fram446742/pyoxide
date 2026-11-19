use {
    crate::{interpreter::run_python, yaml::run_yaml_path},
    anyhow::{Context, Result},
    clap::Command,
    std::{
        ffi::OsString,
        path::{Path, PathBuf},
    },
};

const RUNNER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Prints the runner's own help.
///
/// Python's `-h`/`--help`/`-?` belong to the interpreter, so the runner's
/// features are reachable through a long option Python does not define. That
/// also means a runner option can never be mistaken for a program name.
const HELP_OPTION: &str = "--runner-help";

/// Runs an interpreter configured by a YAML file.
const YAML_OPTION: &str = "--yaml";
const YAML_OPTION_PREFIX: &str = "--yaml=";

/// Superseded spellings, still accepted.
const LEGACY_RUN_PYTHON: &str = "run-python";
const LEGACY_RUN_YAML: &str = "run-yaml";

const RUNNER_HELP: &str = "\
A self-contained Python interpreter.

The interpreter and its standard library are inside this executable. A build
made with --split-resources keeps them in a `pyoxide-resources.packed-resources`
file next to it, and the two must stay together.

Everything else is python: -c, -m, a script, - for stdin, -O, -OO, -B, -E, -s,
-X, -W, -V, --version, --help, with the usual exit codes. No arguments starts a
REPL. The name of the executable does not matter.

Options of this runner (the rest belong to the interpreter):

  --yaml FILE [args...]   Run an interpreter described by a YAML file. It also
                          works as a script interpreter:

                              #!/usr/bin/env -S pyoxide-runner --yaml

  --runner-help           Print this message. -h and --help are the
                          interpreter's arguments.

  run-yaml FILE -- ...    Deprecated spelling of --yaml.
  run-python -- ...       Deprecated spelling of the default.

Libraries go in `pyoxide-modules/` next to this executable: every
`*.packed-resources` file is imported like a built-in library, every package
directory is added to sys.path, and a sidecar naming another Python version
makes the runner skip the file with a warning. `pyoxide package` writes both
forms.
";

/// Remove the runner's own leading argument, returning the YAML file when the
/// `--yaml` form was used.
///
/// Only the first argument is considered, so everything after it - including
/// another `--yaml` - belongs to the interpreter.
fn take_runner_option(args: &mut Vec<OsString>) -> RunnerOption {
    let Some(first) = args.first() else {
        return RunnerOption::None;
    };

    let first = first.to_string_lossy().to_string();

    if first == HELP_OPTION {
        args.remove(0);
        return RunnerOption::Help;
    }

    if let Some(rest) = first.strip_prefix(YAML_OPTION_PREFIX) {
        args.remove(0);
        return RunnerOption::Yaml(PathBuf::from(rest));
    }

    if first == YAML_OPTION {
        args.remove(0);

        return match args.first() {
            Some(path) => {
                let path = PathBuf::from(path.as_os_str());
                args.remove(0);
                RunnerOption::Yaml(path)
            }
            None => RunnerOption::MissingYamlPath,
        };
    }

    RunnerOption::None
}

enum RunnerOption {
    /// No runner argument was given: behave like `python`.
    None,
    Help,
    Yaml(PathBuf),
    /// `--yaml` was given without the file it needs.
    MissingYamlPath,
}

/// Strip a `--` argument separator, as used by the deprecated subcommands.
fn strip_separator(args: &mut Vec<OsString>) {
    if args.first().map(|x| x.as_os_str() == "--").unwrap_or(false) {
        args.remove(0);
    }
}

fn print_runner_help() -> i32 {
    let mut app = Command::new("pyoxide-runner")
        .version(RUNNER_VERSION)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .override_usage("  pyoxide-runner [python arguments...]")
        .about("Run a self-contained Python interpreter")
        .long_about(RUNNER_HELP);

    // print_long_help() writes to stdout and renders long_about (print_help()
    // would print only the one-line about). Its errors are not actionable
    // here: there is nothing to do if the terminal goes away.
    let _ = app.print_long_help();

    0
}

pub fn run() -> Result<i32> {
    let exe = std::env::current_exe().context("resolving current executable")?;
    let mut args: Vec<OsString> = std::env::args_os().skip(1).collect();

    // Deprecated subcommand spellings. They are only recognized as the first
    // argument, so `pyoxide-runner run-yaml.py` still runs a program by that
    // name.
    match args
        .first()
        .map(|x| x.to_string_lossy().to_string())
        .as_deref()
    {
        Some(LEGACY_RUN_PYTHON) => {
            args.remove(0);
            strip_separator(&mut args);
            return run_python(&exe, &args);
        }
        Some(LEGACY_RUN_YAML) => {
            args.remove(0);

            let yaml_path = args
                .first()
                .cloned()
                .context("run-yaml requires a file; use --yaml FILE")?;
            args.remove(0);
            strip_separator(&mut args);

            return run_yaml_path(Path::new(&yaml_path), &args);
        }
        _ => {}
    }

    match take_runner_option(&mut args) {
        RunnerOption::Help => Ok(print_runner_help()),
        RunnerOption::Yaml(yaml_path) => run_yaml_path(&yaml_path, &args),
        RunnerOption::MissingYamlPath => Err(anyhow::anyhow!(
            "--yaml requires a file: pyoxide-runner --yaml FILE [args...]"
        )),
        // Everything else is Python's: no arguments means a REPL, and `-c`,
        // `-m`, a script path, `-`, `--help` and so on are the interpreter's
        // business, not ours.
        RunnerOption::None => run_python(&exe, &args),
    }
}
