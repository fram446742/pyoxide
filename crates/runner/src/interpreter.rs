use {
    anyhow::Result,
    pyembed::{MainPythonInterpreter, OxidizedPythonInterpreterConfig, PackedResourcesSource},
    serde::{Deserialize, Serialize},
    std::{
        ffi::{OsStr, OsString},
        ops::{Deref, DerefMut},
        path::{Path, PathBuf},
    },
};

/// The packed resources file(s) compiled into this executable.
#[cfg(stdlib_packed_resources)]
const STDLIB_RESOURCES_DATA: Option<&[u8]> =
    Some(include_bytes!(env!("PYTHON_PACKED_RESOURCES_PATH")));

#[cfg(not(stdlib_packed_resources))]
const STDLIB_RESOURCES_DATA: Option<&[u8]> = None;

/// Blob read from next to the executable in split builds.
#[cfg(stdlib_packed_resources_split)]
const STDLIB_RESOURCES_FILE: &str = "pyoxide-resources.packed-resources";

/// Directory next to the executable that holds libraries: every
/// `*.packed-resources` file is a resource source, every package directory a
/// `sys.path` entry.
const MODULES_DIR: &str = "pyoxide-modules";

const MODULES_FILE_SUFFIX: &str = ".packed-resources";

/// Separator used when joining `sys.path` entries into a PYTHONPATH value.
const PATH_LIST_SEPARATOR: char = if cfg!(windows) { ';' } else { ':' };

/// Python version this binary was built against, if the build recorded one.
/// A dynamic build against the host's libpython has none, and then sidecars are
/// not checked.
const PYTHON_VERSION: Option<&str> = option_env!("PYOXIDE_PYTHON_VERSION");

/// Metadata written next to each packed resources file by `pyoxide package`.
#[derive(Debug, Deserialize)]
struct ModuleFileMetadata {
    python_version: Option<String>,
}

/// Bytecode from another Python fails with opaque errors, so a mismatched file
/// is skipped. Files without metadata are accepted.
fn module_file_compatible(path: &Path) -> bool {
    let Some(expected) = PYTHON_VERSION else {
        return true;
    };

    let metadata_path = path.with_extension("packed-resources.json");
    let Ok(raw) = std::fs::read_to_string(&metadata_path) else {
        return true;
    };

    match serde_json::from_str::<ModuleFileMetadata>(&raw) {
        Ok(metadata) => match metadata.python_version {
            Some(version) if version != expected => {
                eprintln!(
                    "pyoxide-runner: skipping {}: built for Python {}, this runner is Python {}",
                    path.display(),
                    version,
                    expected
                );
                false
            }
            _ => true,
        },
        Err(e) => {
            eprintln!(
                "pyoxide-runner: ignoring unreadable {}: {}",
                metadata_path.display(),
                e
            );
            true
        }
    }
}

fn add_module_files(builder: &mut OxidizedPythonInterpreterConfig<'_>, dir: &Path) {
    let mut paths: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .map(|n| n.to_string_lossy().ends_with(MODULES_FILE_SUFFIX))
                        .unwrap_or(false)
            })
            .collect(),
        // The directory does not have to exist.
        Err(_) => return,
    };
    paths.sort();

    if paths.is_empty() {
        return;
    }

    for path in paths {
        // A directory wins: it is the form that can carry extension modules.
        let stem = path
            .file_name()
            .map(|n| {
                n.to_string_lossy()
                    .trim_end_matches(MODULES_FILE_SUFFIX)
                    .to_string()
            })
            .unwrap_or_default();

        if !stem.is_empty() && dir.join(&stem).is_dir() {
            eprintln!(
                "pyoxide-runner: using {}/ and ignoring {}{}",
                stem, stem, MODULES_FILE_SUFFIX
            );
            continue;
        }

        if !module_file_compatible(&path) {
            continue;
        }

        // Memory-mapped: a large library set costs no Rust memory.
        builder
            .packed_resources
            .push(PackedResourcesSource::MemoryMappedPath(path));
    }

    builder.oxidized_importer = true;
}

/// Metadata written next to an unpacked package directory by
/// `pyoxide package --unpack`.
#[derive(Debug, Deserialize)]
struct PackageMetadata {
    python_version: Option<String>,
}

/// Package directories under `dir`: each becomes a `sys.path` entry. Same
/// version check as the packed files.
fn module_dirs(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<_> = match std::fs::read_dir(dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok().map(|e| e.path()))
            .filter(|path| path.is_dir())
            .collect(),
        // The directory does not have to exist.
        Err(_) => return Vec::new(),
    };

    dirs.retain(|path| {
        let Some(expected) = PYTHON_VERSION else {
            return true;
        };

        let metadata_path = path.with_extension("json");
        let Ok(raw) = std::fs::read_to_string(&metadata_path) else {
            return true;
        };

        match serde_json::from_str::<PackageMetadata>(&raw) {
            Ok(metadata) => match metadata.python_version {
                Some(version) if version != expected => {
                    eprintln!(
                        "pyoxide-runner: skipping {}: built for Python {}, this runner is Python {}",
                        path.display(),
                        version,
                        expected
                    );
                    false
                }
                _ => true,
            },
            Err(e) => {
                eprintln!(
                    "pyoxide-runner: ignoring unreadable {}: {}",
                    metadata_path.display(),
                    e
                );
                true
            }
        }
    });

    dirs.sort();
    dirs
}

/// An embedded Python interpreter configuration.
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default, transparent)]
pub struct Config<'a> {
    inner: OxidizedPythonInterpreterConfig<'a>,
}

impl<'a> Deref for Config<'a> {
    type Target = OxidizedPythonInterpreterConfig<'a>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'a> DerefMut for Config<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'a> From<Config<'a>> for OxidizedPythonInterpreterConfig<'a> {
    fn from(c: Config<'a>) -> Self {
        c.inner
    }
}

impl<'a> Config<'a> {
    /// Apply the current environment's settings to this config.
    pub fn apply_environment(&mut self) {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()));

        // Resources are either compiled in or, with --split-resources, loaded
        // from a file next to the executable. Both make the binary
        // self-contained with respect to the host's Python installation.
        let mut self_contained = false;

        if let Some(resources) = STDLIB_RESOURCES_DATA {
            // Register the packed resources and enable oxidized_importer to load them.
            self.inner
                .packed_resources
                .push(PackedResourcesSource::Memory(resources));
            self_contained = true;
        }

        #[cfg(stdlib_packed_resources_split)]
        if let Some(exe_dir) = &exe_dir {
            let path = exe_dir.join(STDLIB_RESOURCES_FILE);

            if path.is_file() {
                // Memory-mapped: the standard library blob is over 100 MB and
                // should never be read into anonymous memory.
                self.inner
                    .packed_resources
                    .push(PackedResourcesSource::MemoryMappedPath(path));
                self_contained = true;
            } else {
                eprintln!(
                    "pyoxide-runner: {} not found next to the executable; the standard library and baked packages are unavailable",
                    path.display()
                );
            }
        }

        if self_contained {
            self.inner.oxidized_importer = true;

            // Self-contained binaries have no default sys.path; keep a custom
            // one if it is already set.
            if self.interpreter_config.module_search_paths.is_none() {
                self.interpreter_config.module_search_paths = Some(vec![]);
            }

            // Otherwise Python derives these and warns that they do not exist.
            if let Some(exe_dir) = &exe_dir {
                self.interpreter_config.home = Some(exe_dir.to_path_buf());
                self.interpreter_config.prefix = Some(exe_dir.to_path_buf());
                self.interpreter_config.exec_prefix = Some(exe_dir.to_path_buf());
            }
        }

        // Libraries next to the executable also work in a dynamic build, which
        // is how they are tried out during development.
        if let Some(exe_dir) = &exe_dir {
            let modules_dir = exe_dir.join(MODULES_DIR);

            add_module_files(&mut self.inner, &modules_dir);

            // Unpacked packages are plain directories imported through
            // sys.path, which is how extension modules and real files work.
            let dirs = module_dirs(&modules_dir);

            if !dirs.is_empty() {
                if self_contained {
                    // Self-contained builds start with an empty search path.
                    self.interpreter_config
                        .module_search_paths
                        .get_or_insert_with(Vec::new)
                        .extend(dirs);
                } else {
                    // A dynamic build keeps its own paths and appends this.
                    let mut paths = dirs;
                    if let Some(existing) = std::env::var_os("PYTHONPATH") {
                        paths.extend(std::env::split_paths(&existing));
                    }

                    if let Ok(joined) = std::env::join_paths(paths) {
                        if let Ok(joined) = joined.into_string() {
                            self.interpreter_config.python_path_env =
                                Some(match &self.interpreter_config.python_path_env {
                                    Some(current) => {
                                        format!("{}{}{}", joined, PATH_LIST_SEPARATOR, current)
                                    }
                                    None => joined,
                                });
                        }
                    }
                }
            }
        }

        self.set_missing_path_configuration = false;
    }
}

/// CPython answers some arguments, `--help` and `--version` among them, by
/// asking to exit instead of initializing. That comes back as an error carrying
/// a code, which is a normal outcome here.
pub fn run_interpreter(config: OxidizedPythonInterpreterConfig) -> Result<i32> {
    let interp = match MainPythonInterpreter::new(config) {
        Ok(interp) => interp,
        Err(err) => {
            let Some(code) = err.exit_code() else {
                return Err(anyhow::Error::new(err).context("initializing Python interpreter"));
            };

            // The interpreter never finished starting, so nothing flushed the
            // streams for us. `--help` writes to stdout, which would otherwise
            // be lost.
            let _ = std::io::Write::flush(&mut std::io::stdout());
            let _ = std::io::Write::flush(&mut std::io::stderr());

            return Ok(code);
        }
    };

    Ok(interp.run())
}

/// Runs an embedded Python interpreter in `python` mode.
pub fn run_python<T>(exe: &Path, args: &[T]) -> Result<i32>
where
    T: Into<OsString> + AsRef<OsStr>,
{
    let mut config = Config::default();
    config.apply_environment();
    config.exe = Some(exe.to_path_buf());
    config.argv = Some(
        vec![exe.as_os_str().to_os_string()]
            .into_iter()
            .chain(args.iter().map(|x| x.into()))
            .collect::<Vec<_>>(),
    );

    let config: OxidizedPythonInterpreterConfig = config.into();

    run_interpreter(config)
}
