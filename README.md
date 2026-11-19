# pyoxide

A `python` executable with the interpreter, the standard library and its native
modules inside it, plus a tool for dropping libraries next to it.

It is built on PyOxidizer's crates, pinned by revision, and only adds what they
do not do: per-package library files and a prebuilt interpreter.

```console
$ cargo build
$ just runner
$ ./dist/runner/pyoxide-runner -c 'import sys; print(sys.version)'
3.11.18 (main, Nov 10 2026, ...)
```

You need Rust, `clang` and `lld`. No Python installation: everything the runner
reports comes from the embedded distribution.

## The runner

`pyoxide-runner` is `python`. Arguments go to the interpreter, unchanged —
`-c`, `-m`, a script, `-` for stdin, `-O`, `-OO`, `-B`, `-E`, `-s`, `-X`, `-W`,
`-V`, `--help` — with the usual exit codes. No arguments starts a REPL. Rename
it to `python3` and nothing changes except the name in `sys.argv[0]`.

Two things `python` does not have:

```
pyoxide-runner --runner-help          # what this executable can also do
pyoxide-runner --yaml app.yaml        # interpreter described by YAML
```

The YAML is the document a PyOxidizer project uses, with an `interpreter_config`
block. It needs a `---` line; `...` is optional:

```yaml
---
interpreter_config:
  run_command: 'print("hello, world")'
...
```

## Libraries

Both forms live in `pyoxide-modules/`, next to the executable. The runner scans
it at startup, so adding a library is copying a file and removing one is
deleting it. No rebuild, no environment variables.

One file per package:

```console
$ pyoxide package --dest dist/runner/pyoxide-modules requests idna
wrote dist/runner/pyoxide-modules/requests.packed-resources
wrote dist/runner/pyoxide-modules/idna.packed-resources
packaging complete: 2 package(s), nothing was skipped
```

That file is loaded from memory, and on Linux a C extension cannot be loaded
from memory. Those modules are skipped and reported, so you find out at build
time instead of at import time:

```console
$ pyoxide package --dest dist/runner/pyoxide-modules psutil
wrote dist/runner/pyoxide-modules/psutil.packed-resources
  incomplete: 1 C extension module (s) skipped
packaging incomplete: 1 of 1 package(s) are missing resources
  psutil: 1 C extension module (s) skipped

C extension modules cannot be loaded from a packed resources file on this platform, so these packages will fail at import. Ship them as real files instead:
  pyoxide package --unpack --python-version 3.11 --dest dist/runner/pyoxide-modules psutil
```

The other form writes a directory of real files, which is what `.so` modules and
`__file__` need:

```console
$ pyoxide package --unpack --dest dist/runner/pyoxide-modules numpy
wrote dist/runner/pyoxide-modules/numpy/
$ pyoxide-runner -c 'import numpy, numpy.linalg as l; print(numpy.__version__, l.det([[1,2],[3,4]]))'
2.2.6 -2.0
```

Every package gets a sidecar recording the Python version it was built for (and
the ABI tag for directories). The runner skips a file or directory whose sidecar
names another version, with a warning. A directory wins over a packed file of
the same name.

## Commands

`pyoxide package [--dest DIR] [--unpack] [--python-version X.Y] [--verbose] REQ...`

Wheels only, resolved by pip: `requests`, `lxml>=5`, `numpy==2.2.6`. A package
without a wheel for the target is an error.

`pyoxide build-runner [--dest DIR] [--python-version X.Y] [--requirements "a b"] [--split-resources] [--release]`

Builds the runner. It runs in two stages:

1. evaluate the `resources` target of `pyoxidizer.bzl` with the pinned library —
   this resolves or downloads the Python distribution and writes
   `build/<triple>/<profile>/resources/`, containing a PyO3 config and the packed
   resources;
2. compile `crates/runner` with `PYO3_CONFIG_FILE` pointing at that config, then
   copy the executable to `--dest`.

`--split-resources` keeps the payload in a sibling
`pyoxide-resources.packed-resources` instead of embedding it: 24 MB executable
plus a 132 MB blob that must stay next to it, rather than one 154 MB file.
`--requirements` bakes packages in, subject to the same limit as packed files.

`pyoxide dist-info ARCHIVE` prints the target triple, tags, version, extension
modules and files of a cached Python distribution.

During development, `cargo build`/`cargo test` skip the runner: it cannot compile
without the PyO3 artifacts from stage 1, so it is outside `default-members`.

## Layout

```
crates/cli      pyoxide: dist-info, package, package --unpack, build-runner
crates/runner   pyoxide-runner: interpreter, --yaml, module scanning
pyoxidizer.bzl  which modules go in and at which bytecode levels
```

## Updating PyOxidizer

The dependency is one commit, written three times in `[workspace.dependencies]`.
It points at `fram446742/PyOxidizer#fixes/upstream`: the base fork plus two fixes
it does not carry yet.

```console
$ just upstream-check      # 1 if the base fork moved past the pin
$ just fixes-rebase        # rebase the fixes onto it, prints the new revision
$ just upstream-bump <rev> # move the pin; compile errors say what changed
```

If the fixes ever land upstream, point the revision at
`https://github.com/ntamas/PyOxidizer` and drop the branch. The fork keeps the
changes this project was built from as tags, `port/01` … `port/17`.

## Notes

- `package --unpack` writes no `.dist-info`, so `importlib.metadata` does not see
  unpacked packages. Importing them works.
- `_tkinter` is left out: it needs tcl shared libraries the distribution bundles
  but cannot link statically.
- Only Linux x86_64 has been exercised. The Windows and macOS paths are ported,
  not tested.

When something goes wrong:

| Symptom | Cause |
|---|---|
| `unable to find PyOxidizer config file` | the config must be named `pyoxidizer.bzl` |
| `could not find a package in Cargo manifest` | `add_cargo_manifest_licensing` must point at `crates/runner/Cargo.toml`, not the workspace root |
| `resolving ar header ... No such file or directory` | the pin lost the `libpython` fix |
| `could not format PyStatus` on `--help` | the pin lost the `pyembed` fix |
| `git fetch failed for <url>` | the pinned commit is not pushed |

## License

MPL-2.0, like the PyOxidizer code this is derived from. See `LICENSE`.
