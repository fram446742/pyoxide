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
| `port/14-wheel-bytecode` | bytecode-only wheels in the scanner | **`patches/`** | also sent upstream |
| `port/15-package-unpack` | `package --unpack` | `crates/cli` | |
| `port/16-output-verdict` | verdict at the end of packaging | `crates/cli` | |
| `port/17-just-clean` | `just clean` / `clean-all` | `Justfile` | |

## Ground rules

- **Crate first, patch last.** Everything reachable through
  `pyoxidizerlib`'s public modules (`py_packaging`, `projectmgmt`,
  `environment`, `shell`, `starlark`) goes in a crate here.
- **`patches/` means upstream internals.** A patch is a 3-way diff against the
  pinned revision, applied by `just upstream-bump` and by CI. If `patches/`
  grows past a couple of files, the previous rule was applied wrongly.
- **Send patches upstream.** Anything accepted upstream leaves `patches/`.

## Not ported

`tugger*` (13.9 k lines, installer/signing machinery for upstream's release
pipeline), `release` (upstream maintainer tool), `pyembed-bench`,
`starlark-dialect-build-targets` (dependency of `pyoxidizerlib`, arrives with
it), and `pyoxy` itself (replaced by `crates/runner`).

## Status

- [x] F0 — fork frozen: `port/*` tags, `base` = `origin/main`, `just sync`.
- [x] F1 — this skeleton compiles against the pinned revision.
- [ ] F2 — `package` / `package --unpack` in `crates/cli`.
- [ ] F3 — `crates/runner` (pyoxy equivalent) over `pyembed`.
- [ ] F4 — `patches/`, CI on pin bumps, weekly upstream check.
- [ ] F5 — retire the fork.
