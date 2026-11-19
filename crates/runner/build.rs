use std::path::Path;

fn main() {
    println!("cargo:rerun-if-env-changed=PYO3_CONFIG_FILE");
    println!("cargo:rerun-if-env-changed=PYOXIDE_PACKED_RESOURCES_SPLIT");
    println!("cargo:rustc-check-cfg=cfg(stdlib_packed_resources)");
    println!("cargo:rustc-check-cfg=cfg(stdlib_packed_resources_split)");

    // By default Rust will not export dynamic symbols from built executables.
    // If we're linking libpython, we need its symbols to be exported in order
    // to load Python extension modules.
    if let Ok(os) = std::env::var("CARGO_CFG_TARGET_OS") {
        match os.as_str() {
            "linux" => {
                println!("cargo:rustc-link-arg=-Wl,-export-dynamic");
            }
            "macos" => {
                println!("cargo:rustc-link-arg=-rdynamic");
            }
            _ => {}
        }
    }

    // build-runner points PYO3_CONFIG_FILE at the embedding artifacts.
    if let Ok(config_path) = std::env::var("PYO3_CONFIG_FILE") {
        let config_path = std::path::PathBuf::from(config_path);
        println!("cargo:rerun-if-changed={}", config_path.display());

        emit_pyo3_config_link_args(&config_path);

        let artifact_dir = config_path
            .parent()
            .expect("could not resolve parent directory of PyO3 config file");

        let packed_resources_path = artifact_dir.join("packed-resources");

        if packed_resources_path.exists() {
            println!("cargo:rerun-if-changed={}", packed_resources_path.display());

            // Split builds read the blob at run time; do not compile it in.
            let split = std::env::var("PYOXIDE_PACKED_RESOURCES_SPLIT")
                .map(|v| v == "1")
                .unwrap_or(false);

            if split {
                println!("cargo:rustc-cfg=stdlib_packed_resources_split");
            } else {
                println!("cargo:rustc-cfg=stdlib_packed_resources");
                println!(
                    "cargo:rustc-env=PYTHON_PACKED_RESOURCES_PATH={}",
                    packed_resources_path.display()
                );
            }
        }

        // The interpreter version the binary is built against, used to reject
        // `pyoxide-modules` files compiled by a different Python.
        if let Some(version) = pyo3_config_python_version(&config_path) {
            println!("cargo:rustc-env=PYOXIDE_PYTHON_VERSION={}", version);
        }
    }
}

/// Extract `version=` from a PyO3 configuration file, e.g. `3.11`.
fn pyo3_config_python_version(config_path: &Path) -> Option<String> {
    let config = std::fs::read_to_string(config_path).ok()?;

    config
        .lines()
        .find_map(|line| line.strip_prefix("version="))
        .map(|version| version.trim().to_string())
}

/// Re-emit the linker arguments PyOxidizer recorded in the PyO3 config.
/// Cargo ignores `rustc-link-arg` from dependencies like `pyo3-ffi`, so the
/// binary's own build script has to pass them on.
fn emit_pyo3_config_link_args(config_path: &Path) {
    let config = std::fs::read_to_string(config_path)
        .unwrap_or_else(|e| panic!("could not read {}: {}", config_path.display(), e));

    for line in config.lines() {
        if let Some(value) = line.strip_prefix("extra_build_script_line=") {
            if value.starts_with("cargo:rustc-link-arg=") {
                println!("{}", value);
            }
        }
    }
}
