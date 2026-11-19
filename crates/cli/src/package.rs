//! Write Python package resources: one packed file per package, or a directory
//! of real files with `--unpack`.

use {
    anyhow::{anyhow, Context, Result},
    crate::pip,
    pyoxidizerlib::{
        environment::Environment,
        py_packaging::distribution::{
            default_distribution_location, resolve_distribution, DistributionFlavor,
            PythonDistribution,
        },
    },
    python_packaging::{
        bytecode::BytecodeCompiler,
        location::{AbstractResourceLocation, ConcreteResourceLocation},
        policy::ExtensionModuleFilter,
        resource::PythonResource,
        resource_collection::PythonResourceCollector,
        wheel::WheelArchive,
    },
    std::{
        fs::create_dir_all,
        io::Write,
        path::{Path, PathBuf},
    },
};

struct Resolved {
    dist: Box<dyn PythonDistribution>,
}

fn resolve(
    env: &Environment,
    target_triple: &str,
    flavor: &str,
    python_version: Option<&str>,
) -> Result<Resolved> {
    let flavor = DistributionFlavor::try_from(flavor).map_err(|e| anyhow!("invalid --flavor: {}", e))?;
    let location = default_distribution_location(&flavor, target_triple, python_version)?;
    let cache_dir = env.python_distributions_dir();
    let dist = resolve_distribution(&location, &cache_dir)
        .context("resolving Python distribution")?;

    Ok(Resolved { dist })
}

/// Write one packed resources file per requirement, plus a `.json` sidecar.
pub fn generate_packed_resources(
    env: &Environment,
    dest_path: &Path,
    requirements: &[String],
    target_triple: &str,
    flavor: &str,
    python_version: Option<&str>,
    verbose: bool,
) -> Result<()> {
    if requirements.is_empty() {
        return Err(anyhow!("no requirements to package"));
    }

    let Resolved { dist } = resolve(env, target_triple, flavor, python_version)?;

    let mut policy = dist.create_packaging_policy()?;
    policy.set_extension_module_filter(ExtensionModuleFilter::All);
    policy.set_include_distribution_sources(true);
    policy.set_include_distribution_resources(true);
    policy.set_resources_location(ConcreteResourceLocation::InMemory);
    // Without every level, imports fail under -O and -OO.
    policy.set_bytecode_optimize_level_zero(true);
    policy.set_bytecode_optimize_level_one(true);
    policy.set_bytecode_optimize_level_two(true);

    let wheels = pip::download_wheel_resources(
        env,
        dist.as_ref(),
        dist.as_ref(),
        &policy,
        verbose,
        requirements,
    )
    .context("downloading Python packages with pip")?;

    create_dir_all(dest_path)
        .with_context(|| format!("creating destination directory {}", dest_path.display()))?;

    // Bytecode compilation is driven by the distribution's own Python.
    let compiler_temp = env.temporary_directory("python-package-bytecode")?;
    let mut compiler = BytecodeCompiler::new(dist.python_exe_path(), compiler_temp.path())?;

    let python_version = dist.python_major_minor_version();
    let package_count = wheels.len();
    // (package, C extension modules, untyped files) whose output is partial.
    let mut incomplete: Vec<(String, usize, usize)> = Vec::new();

    for (name, resources) in wheels {
        let mut skipped_extension_modules = 0;
        let mut skipped_files = 0;

        let mut collector = PythonResourceCollector::new(
            vec![AbstractResourceLocation::InMemory],
            // Extension modules are never written here.
            vec![],
            false,
            false,
        );

        for resource in &resources {
            match resource {
                PythonResource::ModuleSource(r) => {
                    collector.add_python_module_source(r, &ConcreteResourceLocation::InMemory)?;
                }
                // Some wheels ship modules only as .pyc (numpy does), and
                // others as source that still needs compiling.
                PythonResource::ModuleBytecode(r) => {
                    collector.add_python_module_bytecode(r, &ConcreteResourceLocation::InMemory)?;
                }
                PythonResource::ModuleBytecodeRequest(r) => {
                    collector
                        .add_python_module_bytecode_from_source(r, &ConcreteResourceLocation::InMemory)?;
                }
                PythonResource::PackageResource(r) => {
                    collector
                        .add_python_package_resource(r, &ConcreteResourceLocation::InMemory)?;
                }
                PythonResource::PackageDistributionResource(r) => {
                    collector.add_python_package_distribution_resource(
                        r,
                        &ConcreteResourceLocation::InMemory,
                    )?;
                }
                // Not loadable from memory; skipped and reported below.
                PythonResource::ExtensionModule(_) | PythonResource::File(_) => {
                    let kind = match resource {
                        PythonResource::ExtensionModule(_) => {
                            skipped_extension_modules += 1;
                            "C extension module"
                        }
                        _ => {
                            skipped_files += 1;
                            "untyped file"
                        }
                    };
                    if verbose {
                        eprintln!(
                            "skipping {} from {} because it cannot be loaded from memory",
                            kind, name
                        );
                    }
                }
                _ => {
                    return Err(anyhow!(
                        "{} contains a resource that cannot be written to a single packed resources file: {:?}",
                        name,
                        resource
                    ));
                }
            }
        }

        let compiled = collector
            .compile_resources(&mut compiler)
            .with_context(|| format!("compiling resources of {}", name))?;

        // Anything else written to disk would make this file incomplete.
        if !compiled.extra_files.is_empty() {
            return Err(anyhow!(
                "{} produced extra files that cannot be part of its packed resources file",
                name
            ));
        }

        let dest_file = dest_path.join(format!("{}.packed-resources", name));

        let mut fh = std::fs::File::create(&dest_file)
            .with_context(|| format!("opening {} for writing", dest_file.display()))?;
        let mut writer = std::io::BufWriter::new(&mut fh);

        compiled
            .write_packed_resources(&mut writer)
            .with_context(|| format!("writing {}", dest_file.display()))?;

        writer.flush()?;
        drop(writer);

        // The runner checks this before loading: bytecode from another Python
        // fails with opaque import errors. No sidecar means "load it anyway".
        let metadata_file = dest_path.join(format!("{}.packed-resources.json", name));
        let metadata = serde_json::json!({
            "python_version": python_version,
            "target_triple": target_triple,
        });

        std::fs::write(&metadata_file, serde_json::to_string_pretty(&metadata)?)
            .with_context(|| format!("writing {}", metadata_file.display()))?;

        println!("wrote {}", dest_file.display());

        // Name what the file is missing, per package.
        if skipped_extension_modules + skipped_files > 0 {
            println!(
                "  incomplete: {} skipped",
                describe_skipped(skipped_extension_modules, skipped_files)
            );
            incomplete.push((name, skipped_extension_modules, skipped_files));
        }
    }

    compiler_temp
        .close()
        .context("closing temporary directory")?;

    // Some packages only lose an accelerator, others something they need.
    if incomplete.is_empty() {
        println!(
            "packaging complete: {} package(s), nothing was skipped",
            package_count
        );
    } else {
        incomplete.sort();
        println!(
            "packaging incomplete: {} of {} package(s) are missing resources",
            incomplete.len(),
            package_count
        );

        for (name, extension_modules, files) in &incomplete {
            println!(
                "  {name}: {} skipped",
                describe_skipped(*extension_modules, *files)
            );
        }

        // The only fix is the other mode, and the caller has every value.
        println!();
        println!(
            "C extension modules cannot be loaded from a packed resources file on this platform, so these packages will fail at import. Ship them as real files instead:"
        );
        println!(
            "  pyoxide package --unpack --python-version {} --dest {} {}",
            python_version,
            dest_path.display(),
            requirements.join(" ")
        );
    }

    Ok(())
}

/// Write each package as a directory under `dest_path`. The runner adds
/// `<dest>/<name>` to `sys.path`, which is what `.so` modules and `__file__`
/// need.
pub fn unpack_wheels(
    env: &Environment,
    dest_path: &Path,
    requirements: &[String],
    target_triple: &str,
    flavor: &str,
    python_version: Option<&str>,
    verbose: bool,
) -> Result<()> {
    if requirements.is_empty() {
        return Err(anyhow!("no requirements to package"));
    }

    let Resolved { dist } = resolve(env, target_triple, flavor, python_version)?;

    let mut policy = dist.create_packaging_policy()?;
    policy.set_extension_module_filter(ExtensionModuleFilter::All);

    let (temp_dir, wheels) = pip::download_wheels(
        env,
        dist.as_ref(),
        dist.as_ref(),
        &policy,
        verbose,
        requirements,
    )
    .context("downloading Python packages with pip")?;

    create_dir_all(dest_path)
        .with_context(|| format!("creating destination directory {}", dest_path.display()))?;

    let package_python_version = dist.python_major_minor_version();
    let abi_tag = dist.python_abi_tag().unwrap_or_default();

    for (name, path) in wheels {
        let wheel = WheelArchive::from_path(&path)?;
        let package_path = dest_path.join(&name);

        // Replace whatever was there, so an upgrade leaves nothing stale.
        if package_path.exists() {
            std::fs::remove_dir_all(&package_path)
                .with_context(|| format!("removing {}", package_path.display()))?;
        }

        // The wheel helpers return install-layout paths; putting all of them
        // under one directory makes it a sys.path entry.
        let mut files = wheel.regular_files();
        files.extend(wheel.purelib_files());
        files.extend(wheel.platlib_files());
        files.extend(wheel.data_files());

        for file in files {
            let relative = file.path();
            let target: PathBuf = package_path.join(relative);

            if let Some(parent) = target.parent() {
                create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
            }

            let entry = file.entry();
            std::fs::write(&target, entry.file_data().resolve_content()?)
                .with_context(|| format!("writing {}", target.display()))?;

            #[cfg(unix)]
            if entry.is_executable() {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = std::fs::metadata(&target)?.permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(&target, permissions)?;
            }
        }

        // A packed file for the same package would shadow the directory.
        for stale in [
            dest_path.join(format!("{}.packed-resources", name)),
            dest_path.join(format!("{}.packed-resources.json", name)),
        ] {
            if stale.exists() {
                std::fs::remove_file(&stale)
                    .with_context(|| format!("removing {}", stale.display()))?;
                println!(
                    "removed {} (replaced by an unpacked directory)",
                    stale.display()
                );
            }
        }

        let metadata_file = dest_path.join(format!("{}.json", name));
        let metadata = serde_json::json!({
            "kind": "unpacked",
            "python_version": package_python_version,
            "target_triple": target_triple,
            "abi_tag": abi_tag,
        });

        std::fs::write(&metadata_file, serde_json::to_string_pretty(&metadata)?)
            .with_context(|| format!("writing {}", metadata_file.display()))?;

        println!("wrote {}/", package_path.display());
    }

    temp_dir
        .close()
        .context("closing temporary directory")?;

    println!("unpacked libraries are imported from real files");

    Ok(())
}

/// e.g. `1 C extension module(s), 2 untyped file(s)`.
fn describe_skipped(extension_modules: usize, files: usize) -> String {
    let mut parts = Vec::new();

    if extension_modules > 0 {
        parts.push(format!("{} C extension module(s)", extension_modules));
    }
    if files > 0 {
        parts.push(format!("{} untyped file(s)", files));
    }

    parts.join(", ")
}
