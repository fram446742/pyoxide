# Porting inventory

Source of truth for what moves out of `fram446742/PyOxidizer`. The fork keeps
each commit as a `port/NN-*` tag; the fork's `main` carries no logic and must
fast-forward to `origin/main`.

| Tag | What | Destination | Done when |
|---|---|---|---|
| `port/01-deps` | dependency bumps | — | superseded upstream |
| `port/02-build-fix` | build fixes for those bumps | — | superseded upstream |
| `port/03-snmalloc` | `pyembed`/snmalloc-sys adaptation | — | superseded upstream |
| `port/04-cli-layout` | subcommand layout | `crates/cli` | `--help` matches `tests/cmd/help.trycmd` |
| `port/05-pyoxy-exit-yaml` | interpreter exit codes, `--yaml` | `crates/runner` | |
| `port/06-docs-layout` | command docs | `README.md` | |
| `port/07-pyoxy-default-python` | `pyoxy` behaves like `python` | `crates/runner` | `-c`, script, `-` all work |
| `port/08-pyoxy-libs` | per-package libraries, static runner | `crates/runner` + bzl | |
| `port/09-just-entry` | `just` as the only entry point | `Justfile` | every recipe runs twice clean |
| `port/10-release-profile` | thin LTO, `cgu=1`, strip | `Cargo.toml` | done here |
| `port/11-pyoxy-split` | resources beside the executable, versioned libs | `crates/runner` | mismatch warns and skips |
| `port/12-docs-split` | split-resource docs | `README.md` | |
| `port/13-package-verdict` | per-package verdict | `crates/cli` | |
| `port/14-wheel-bytecode` | bytecode-only wheels in the scanner | **`[patch]` in `Cargo.toml`** | delete once upstream takes it |
| `port/15-package-unpack` | `package --unpack` | `crates/cli` | |
| `port/16-output-verdict` | verdict at the end of packaging | `crates/cli` | |
| `port/17-just-clean` | `just clean` / `clean-all` | `Justfile` | |

## F2 reality check (verified against the pinned revision)

The fork's `package` layer is **fork-only**: `python_package_generate_packed_resources`,
`python_package_unpack_wheels` and `build_pyoxy_runner` do not exist upstream. F2
*moves* ~900 lines, it does not call them.

What upstream does expose, and the port must build on:

| Needed | Upstream location | Public |
|---|---|---|
| resolve/download a distribution | `py_packaging::distribution::{resolve_distribution, default_distribution_location, download_distribution}` | yes |
| pip into resources | `py_packaging::packaging_tool::pip_download_wheel_resources` | yes |
| bytecode compiler | `PythonDistribution::python_bytecode_compiler` (trait method) | yes |
| policy knobs | `python_packaging::policy::{ExtensionModuleFilter, ConcreteResourceLocation}` | yes |
| environment, temp dirs, cache | `environment::Environment` | yes |
| distribution list | `python_distributions::PythonDistributionCollection` | yes (the `default_python_distributions` registry itself is private; go through `default_distribution_location`) |

## Ground rules

- **Crate first, patch last.** Everything reachable through
  `pyoxidizerlib`'s public modules (`py_packaging`, `projectmgmt`,
  `environment`, `shell`, `starlark`) goes in a crate here.
- **`[patch]` means upstream internals.** Cargo's `[patch]` block replaces one
  crate at a revision, with no vendoring: today exactly one crate
  (`python-packaging`, for port/14). If the block grows past a couple of
  crates, the previous rule was applied wrongly.
- **Send patches upstream.** Anything accepted upstream leaves `patches/`.

## Not ported

`tugger*` (13.9 k lines, installer/signing machinery for upstream's release
pipeline), `release` (upstream maintainer tool), `pyembed-bench`,
`starlark-dialect-build-targets` (dependency of `pyoxidizerlib`, arrives with
it), and `pyoxy` itself (replaced by `crates/runner`).

## Status

- [x] F0 — fork frozen: `port/*` tags, `base` = `origin/main`, `just sync`.
- [x] F1 — this skeleton compiles against the pinned revision; `pyoxide
  dist-info` is a real call into `pyoxidizerlib` (it also caught the first
  signature difference: upstream takes `&str`, the fork had `&Path`).
  `crates/runner` is deferred to F3 because it cannot compile without PyO3
  embedding artifacts.
- [x] F2 — `package` / `package --unpack` in `crates/cli`, with per-package
  verdicts. Verified end to end: `idna` packed; `psutil` reports the skipped C
  extension; `numpy` packs only with the bytecode-only-wheel fix and its
  unpacked form imports in CPython 3.11 (`numpy 2.2.6`, `det= -2.0`).
  Two things upstream lacks had to be ported too:
  `pip_download_wheels`/`pip_download_wheel_resources` (now `crates/cli/src/pip.rs`)
  and the `.pyc` scanner fix, which needs a `[patch]` on `python-packaging`
  until it lands upstream. `--unpack` writes no `.dist-info`, so
  `importlib.metadata.version()` does not work for unpacked packages —
  same behaviour as the fork.
- [ ] F3 — `crates/runner` (pyoxy equivalent) over `pyembed`.
- [ ] F4 — `patches/`, CI on pin bumps, weekly upstream check.
- [ ] F5 — retire the fork.
