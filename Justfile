set shell := ["bash", "-uc"]
set positional-arguments

pin := `grep -oE 'rev = "[0-9a-f]{40}"' Cargo.toml | head -1 | sed -E 's/rev = "(.*)"/\1/'`

default:
  @just --list

# Build the binaries against the pinned upstream revision.
build:
  cargo build

# Compile and run every test.
test:
  cargo test

# Remove generated artifacts.
clean:
  rm -rf dist build

# Remove generated artifacts and the cargo target directory.
clean-all: clean
  rm -rf target

# Move the pinned upstream revision; compile errors name what changed.
upstream-bump rev:
  sed -i -E 's/rev = "[0-9a-f]{40}"/rev = "{{rev}}"/' Cargo.toml
  grep -n 'rev = ' Cargo.toml
  cargo test
  cargo check -p pyoxide-runner

# Show the pin and the remote heads it is supposed to track.
upstream-status:
  #!/usr/bin/env bash
  set -eo pipefail
  echo "pin:   {{pin}}"
  echo "base:  $(git ls-remote https://github.com/ntamas/PyOxidizer main | cut -f1) (ntamas/PyOxidizer main)"
  echo "fixes: $(git ls-remote https://github.com/fram446742/PyOxidizer fixes/upstream | cut -f1) (fram446742 fixes/upstream)"

# Fail when the base fork moved past the pin. For the scheduled CI job.
upstream-check:
  #!/usr/bin/env bash
  set -eo pipefail
  git fetch --quiet --no-tags https://github.com/ntamas/PyOxidizer main:refs/remotes/base/main
  git fetch --quiet --no-tags https://github.com/fram446742/PyOxidizer fixes/upstream:refs/remotes/pin/fixes
  pin={{pin}}

  if ! git merge-base --is-ancestor refs/remotes/base/main refs/remotes/pin/fixes; then
    echo "the base fork moved: rebase fixes/upstream (just fixes-rebase in the fork) and bump the pin" >&2
    exit 1
  fi

  fixes=$(git rev-parse refs/remotes/pin/fixes)
  if [ "$pin" != "$fixes" ]; then
    echo "note: pin $pin lags fixes/upstream head $fixes"
  fi
  echo "ok: pin is on top of the base fork"

# Build the self-contained runner with Python embedded.
runner python_version="3.11" libraries="" split="":
  #!/usr/bin/env bash
  set -eo pipefail
  args=(--path . --dest dist/runner --python-version {{python_version}} --release)
  if [ -n "{{libraries}}" ]; then
    args+=(--requirements "{{libraries}}")
  fi
  if [ -n "{{split}}" ]; then
    args+=(--split-resources)
  else
    rm -f dist/runner/pyoxide-resources.packed-resources
  fi
  cargo run --quiet --bin pyoxide -- build-runner "${args[@]}"
  ls -lh dist/runner/
  ./dist/runner/pyoxide-runner -V
