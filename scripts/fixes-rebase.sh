#!/usr/bin/env bash
#
# Rebase the fork's fixes/upstream branch onto the base fork's main and print
# the revision to pin in Cargo.toml. Run it from anywhere.
#
# Usage: scripts/fixes-rebase.sh [path-to-fork-checkout]
set -eo pipefail

fork=${1:-../PyOxidizer}

cd "$fork"
git fetch origin --prune
git checkout fixes/upstream
git rebase origin/main
git log --oneline -3
echo "pin: $(git rev-parse HEAD)"
echo "publish: git push --force-with-lease myrepo fixes/upstream"
