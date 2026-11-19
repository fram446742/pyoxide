#!/usr/bin/env bash
#
# Build the self-contained runner (Python embedded) into dist/runner/.
#
# Usage: scripts/runner.sh [python-version] [requirements] [split]
#
#   python-version  X.Y to embed, default 3.11
#   requirements    space separated packages to bake in, e.g. "requests pyyaml"
#   split           non-empty ships the payload as a sibling file (24 MB binary
#                   + 132 MB blob) instead of one 155 MB executable
#
# The first run downloads the Python distribution (~35 MB) into PyOxidizer's
# cache and compiles the payload; later runs reuse both.
set -eo pipefail

cd "$(dirname "$0")/.."

python_version=${1:-3.11}
requirements=${2:-}
split=${3:-}

args=(--path . --dest dist/runner --python-version "$python_version" --release)

if [ -n "$requirements" ]; then
  args+=(--requirements "$requirements")
fi

if [ -n "$split" ]; then
  args+=(--split-resources)
else
  # Switching back from split mode must not leave a stale payload behind,
  # which the runner would happily load instead of the embedded resources.
  rm -f dist/runner/pyoxide-resources.packed-resources
fi

cargo run --quiet --bin pyoxide -- build-runner "${args[@]}"

ls -lh dist/runner/
./dist/runner/pyoxide-runner -V
