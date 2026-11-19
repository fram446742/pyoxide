//! Build the self-contained runner: evaluate `pyoxide.bzl` for its embedding
//! artifacts, then compile `pyoxide-runner` against them.
//!
//! Ported from the fork's `build_pyoxy_runner` (`port/08`, `port/11`).

use {
    anyhow::{anyhow, Context, Result},
    duct::cmd,
    pyoxidizerlib::environment::Environment,
    std::{collections::HashMap, fs::create_dir_all, path::{Path, PathBuf}},
};

#[allow(clippy::too_many_arguments)]
pub fn build_runner(
    env: &Environment,
    project_path: &Path,
    dest_path: &Path,
    exe_name: &str,
    target_triple: Option<&str>,
    extra_vars: HashMap<String, Option<String>>,
    release: bool,
    split_resources: bool,
    verbose: bool,
) -> Result<()> {
    // First stage: evaluate the project's `resources` target, which writes the
    // embedding artifacts - pyo3 config, packed resources, licensing - into
    // the build directory.
    let project_path = std::fs::canonicalize(project_path)
        .with_context(|| format!("resolving {}", project_path.display()))?;

    let mut vars = extra_vars;

    // The embedding artifacts for Python-linked executables always include the
    // stdlib resources. Pointing BUILD_PATH at a project-local directory keeps
    // them grouped with the binary that consumes them.
    vars.entry("BUILD_PATH".to_string()).or_insert_with(|| {
        Some(project_path.join("build").display().to_string())
    });

    // Starlark resolves a relative BUILD_PATH against the project directory.
    let build_dir = match vars.get("BUILD_PATH").cloned().flatten() {
        Some(x) if !x.is_empty() => {
            let path = PathBuf::from(x);
            if path.is_absolute() {
                path
            } else {
                project_path.join(path)
            }
        }
        _ => project_path.join("build"),
    };

    pyoxidizerlib::projectmgmt::build(
        env,
        &project_path,
        target_triple,
        Some(vec!["resources".to_string()]),
        vars,
        release,
        verbose,
    )
    .context("building Python embedding artifacts")?;

    // Second stage: build the crate's binary against those artifacts.
    let target_triple = pyoxidizerlib::projectmgmt::resolve_target(target_triple)?;

    let profile = if release { "release" } else { "debug" };

    // The artifacts land in <BUILD_PATH>/<triple>/<profile>/resources/.
    let artifacts_dir = find_pyo3_config_dir(&build_dir).ok_or_else(|| {
        anyhow!(
            "the resources target of {} did not write a pyo3-build-config-file.txt under {}",
            project_path.display(),
            build_dir.display()
        )
    })?;

    let pyo3_config = artifacts_dir.join("resources").join("pyo3-build-config-file.txt");

    let bin_name = "pyoxide-runner";

    // Cargo rebuilds when PYO3_CONFIG_FILE changes thanks to
    // `cargo:rerun-if-env-changed` in the runner's build script, so repeated
    // invocations with a different --python-version do produce a rebuild.
    let exe_path = project_path
        .join("target")
        .join(&target_triple)
        .join(profile)
        .join(exe_name_file_with_suffix(bin_name, &target_triple));

    let mut cmd_args = vec![
        "build".to_string(),
        // The runner is not a default member: plain builds must skip it.
        "-p".to_string(),
        bin_name.to_string(),
        "--target".to_string(),
        target_triple.clone(),
        "--bin".to_string(),
        bin_name.to_string(),
        "--target-dir".to_string(),
        project_path.join("target").display().to_string(),
    ];

    if release {
        cmd_args.push("--release".to_string());
    }

    let mut env_vars: HashMap<String, String> = std::env::vars().collect();
    env_vars.insert(
        "PYO3_CONFIG_FILE".to_string(),
        pyo3_config.display().to_string(),
    );

    // Read by the runner's build script: it decides whether the packed
    // resources are compiled in or loaded from a sibling file at run time.
    if split_resources {
        env_vars.insert(
            "PYOXIDE_PACKED_RESOURCES_SPLIT".to_string(),
            "1".to_string(),
        );
    }

    cmd("cargo", &cmd_args)
        .full_env(env_vars)
        .dir(&project_path)
        .run()
        .context("invoking cargo command")?;

    if !exe_path.exists() {
        return Err(anyhow!(
            "cargo build did not produce an executable at {}",
            exe_path.display()
        ));
    }

    create_dir_all(dest_path)
        .with_context(|| format!("creating destination directory {}", dest_path.display()))?;

    let dest_exe = dest_path.join(exe_name_file_with_suffix(exe_name, &target_triple));

    std::fs::copy(&exe_path, &dest_exe)
        .with_context(|| format!("copying executable to {}", dest_exe.display()))?;

    println!("wrote {}", dest_exe.display());

    // With --split-resources the blob is not embedded: it ships as a sibling
    // file that the runner memory-maps. The name is what interpreter.rs looks
    // for (STDLIB_RESOURCES_FILE).
    if split_resources {
        let source = artifacts_dir.join("resources").join("packed-resources");
        let dest_blob = dest_path.join("pyoxide-resources.packed-resources");

        std::fs::copy(&source, &dest_blob)
            .with_context(|| format!("copying packed resources to {}", dest_blob.display()))?;

        println!("wrote {} (sidecar resources)", dest_blob.display());
    }

    println!("artifacts: {}", artifacts_dir.display());

    Ok(())
}

/// Return the file name `exe_name` should have for `target_triple`.
fn exe_name_file_with_suffix(exe_name: &str, target_triple: &str) -> String {
    if target_triple.contains("windows") {
        format!("{}.exe", exe_name)
    } else {
        exe_name.to_string()
    }
}

/// Find the directory under `build_dir` containing a `resources/`
/// `pyo3-build-config-file.txt`.
///
/// The regular layout is `<build>/<triple>/<profile>/resources/`, but the
/// depth-limited walk keeps this working if the intermediate directory names
/// change. Returns None when nothing was found.
fn find_pyo3_config_dir(build_dir: &Path) -> Option<PathBuf> {
    const MAX_DEPTH: usize = 4;

    fn walk(dir: &Path, depth: usize, found: &mut Option<PathBuf>) -> bool {
        if found.is_some() {
            return true;
        }

        if depth > MAX_DEPTH {
            return false;
        }

        if dir
            .join("resources")
            .join("pyo3-build-config-file.txt")
            .exists()
        {
            *found = Some(dir.to_path_buf());
            return true;
        }

        let mut entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .filter_map(|x| x.ok().map(|e| e.path()))
                .filter(|p| p.is_dir())
                .collect::<Vec<_>>(),
            Err(_) => return false,
        };
        entries.sort();
        entries.reverse();

        for entry in entries {
            if walk(&entry, depth + 1, found) {
                return true;
            }
        }

        false
    }

    let mut found: Option<PathBuf> = None;
    walk(build_dir, 0, &mut found);
    found
}
