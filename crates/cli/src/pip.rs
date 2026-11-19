//! Run pip and keep each package's wheels apart, so packaging can write one
//! file or directory per package.

use {
    anyhow::{anyhow, Context, Result},
    duct::cmd,
    pyoxidizerlib::py_packaging::distribution::PythonDistribution,
    python_packaging::{
        policy::PythonPackagingPolicy, resource::PythonResource, wheel::WheelArchive,
    },
    std::{io::Write, path::PathBuf},
    tempfile::TempDir,
};

/// Run `pip download` into a temporary directory that the caller keeps, since
/// `--unpack` reads the wheels afterwards.
pub fn download_wheels(
    env: &pyoxidizerlib::environment::Environment,
    host_dist: &dyn PythonDistribution,
    target_dist: &dyn PythonDistribution,
    policy: &PythonPackagingPolicy,
    verbose: bool,
    args: &[String],
) -> Result<(TempDir, Vec<(String, PathBuf)>)> {
    let temp_dir = env.temporary_directory("pyoxide-pip-download")?;

    host_dist.ensure_pip()?;

    let target_dir = temp_dir.path();

    let mut pip_args = vec![
        "-m".to_string(),
        "pip".to_string(),
        "--disable-pip-version-check".to_string(),
    ];

    if verbose {
        pip_args.push("--verbose".to_string());
    }

    pip_args.extend(vec![
        "download".to_string(),
        "--dest".to_string(),
        format!("{}", target_dir.display()),
        // Only download wheels.
        "--only-binary=:all:".to_string(),
    ]);

    // We download files compatible with the distribution we're targeting.
    // pip accepts multiple --platform arguments and takes wheels matching
    // any of them.
    let platform_tags = match policy.python_platform_compatibility_tag_overrides() {
        [] => vec![target_dist.python_platform_compatibility_tag().to_string()],
        tags => tags.to_vec(),
    };
    pip_args.extend(platform_tags.iter().map(|tag| format!("--platform={}", tag)));

    pip_args.extend(vec![
        format!("--python-version={}", target_dist.python_version()),
        format!(
            "--implementation={}",
            target_dist.python_implementation_short()
        ),
    ]);

    if let Some(abi) = target_dist.python_abi_tag() {
        pip_args.push(format!("--abi={}", abi));
    }

    pip_args.extend(args.iter().cloned());

    eprintln!("downloading dependencies with pip");

    let output = cmd(host_dist.python_exe_path(), &pip_args)
        .stderr_to_stdout()
        .unchecked()
        .run()?;

    // pip's output only matters on failure or with --verbose.
    if !output.status.success() {
        std::io::stderr().write_all(&output.stdout)?;
        return Err(anyhow!("error running pip"));
    }
    if verbose {
        std::io::stdout().write_all(&output.stdout)?;
    }

    let mut files = std::fs::read_dir(target_dir)?
        .map(|entry| Ok(entry?.path()))
        .collect::<Result<Vec<_>>>()?;
    files.sort();

    let mut wheels = Vec::new();

    for path in &files {
        wheels.push((package_name_from_wheel(path)?, path.clone()));
    }

    Ok((temp_dir, wheels))
}

/// `download_wheels`, then scan each wheel into resources.
pub fn download_wheel_resources<'a>(
    env: &pyoxidizerlib::environment::Environment,
    host_dist: &dyn PythonDistribution,
    target_dist: &dyn PythonDistribution,
    policy: &PythonPackagingPolicy,
    verbose: bool,
    args: &[String],
) -> Result<Vec<(String, Vec<PythonResource<'a>>)>> {
    let (temp_dir, wheels) = download_wheels(env, host_dist, target_dist, policy, verbose, args)?;

    let mut res = Vec::new();

    for (group_name, path) in &wheels {
        let wheel = WheelArchive::from_path(path)?;

        let resources = wheel.python_resources(
            target_dist.cache_tag(),
            &target_dist.python_module_suffixes()?,
            policy.file_scanner_emit_files(),
            policy.file_scanner_classify_files(),
        )?;

        res.push((group_name.clone(), resources));
    }

    temp_dir.close().context("closing temporary directory")?;

    Ok(res)
}

/// The distribution name of a wheel: everything before the first `-`, with
/// hyphens escaped as underscores by the wheel format.
fn package_name_from_wheel(path: &std::path::Path) -> Result<String> {
    let file_name = path
        .file_name()
        .ok_or_else(|| anyhow!("could not derive file name"))?
        .to_string_lossy();
    let name = file_name
        .strip_suffix(".whl")
        .ok_or_else(|| anyhow!("{} is not a wheel", path.display()))?
        .split('-')
        .next()
        .ok_or_else(|| anyhow!("empty wheel name"))?;

    if name.is_empty() {
        return Err(anyhow!("empty wheel name in {}", path.display()));
    }

    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::package_name_from_wheel;
    use std::path::Path;

    #[test]
    fn wheel_names() {
        for (file, name) in [
            ("idna-3.20-py3-none-any.whl", "idna"),
            // Build tags add a `-` after the version; the name still precedes it.
            ("pyyaml-6.0.2-cp311-cp311-manylinux_2_17_x86_64.whl", "pyyaml"),
            ("zope_interface-7.0-py3-none-any.whl", "zope_interface"),
        ] {
            assert_eq!(package_name_from_wheel(Path::new(file)).unwrap(), name);
        }

        assert!(package_name_from_wheel(Path::new("idna-3.20.tar.gz")).is_err());
    }
}
