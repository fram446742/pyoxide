# Describes the Python embedding artifacts for the pyoxide runner: a PyO3
# config file and a packed resources blob, written under BUILD_PATH.
#
# `pyoxide build-runner` evaluates this target and then compiles the runner
# against what it produced.

BUILD_PATH = VARS.get("BUILD_PATH", CWD + "/build")
PYTHON_VERSION = VARS.get("PYTHON_VERSION", "3.11")

# Space separated Python requirements to embed in the produced resources,
# e.g. --var REQUIREMENTS "requests pyyaml". Empty by default.
REQUIREMENTS = VARS.get("REQUIREMENTS", "")

set_build_path(BUILD_PATH)


def make_resources():
    dist = default_python_distribution(python_version=PYTHON_VERSION)

    policy = dist.make_python_packaging_policy()

    policy.extension_module_filter = "all"
    policy.include_distribution_sources = True
    policy.include_distribution_resources = True
    policy.resources_location = "in-memory"

    # Bytecode for every optimization level. Without the -O1 and -O2 variants,
    # imports performed during interpreter startup fail when the executable is
    # invoked with -O or -OO.
    policy.bytecode_optimize_level_zero = True
    policy.bytecode_optimize_level_one = True
    policy.bytecode_optimize_level_two = True

    exe = dist.to_python_executable(
        name="pyoxide-runner",
        packaging_policy=policy,
    )

    if REQUIREMENTS:
        # One pip invocation for every requirement so shared dependencies are
        # resolved and installed exactly once.
        resources = exe.pip_install(REQUIREMENTS.split())

        # A packed resources blob loaded from memory cannot carry extension
        # modules or untyped files. Skip them, as `pyoxide package` does, so
        # e.g. requests embeds despite its C accelerator wheels.
        kept = [r for r in resources if type(r) not in ("PythonExtensionModule", "File")]

        skipped = len(resources) - len(kept)
        if skipped:
            print(
                "pyoxide: {} resource(s) skipped; the baked executable may be missing modules - check imports".format(skipped)
            )

        exe.add_python_resources(kept)

    exe.add_cargo_manifest_licensing(CWD + "/crates/runner/Cargo.toml")

    return exe.to_embedded_resources()


register_target("resources", make_resources)
resolve_targets()
