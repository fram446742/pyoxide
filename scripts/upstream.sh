#!/usr/bin/env bash
#
# Inspect and move the pinned PyOxidizer revision.
#
# Usage: scripts/upstream.sh <command>
#
#   status          show the pin and the heads it tracks
#   check           exit 1 when the base fork moved past the pin (CI)
#   bump <rev>      move the pin, then compile the CLI and the runner
#   fixes-rebase    rebase the fork's fixes/upstream onto its origin/main
#
# The pin is the git revision recorded in Cargo.toml, one line per crate.
set -eo pipefail

cd "$(dirname "$0")/.."

BASE_REPO=https://github.com/ntamas/PyOxidizer
PIN_REPO=https://github.com/fram446742/PyOxidizer
FORK_CHECKOUT=${FORK_CHECKOUT:-../PyOxidizer}

pin() {
  grep -oE 'rev = "[0-9a-f]{40}"' Cargo.toml | head -1 | sed -E 's/rev = "(.*)"/\1/'
}

case "${1:-}" in
  status)
    printf 'pin:   %s\n' "$(pin)"
    printf 'base:  %s (ntamas/PyOxidizer main)\n' "$(git ls-remote "$BASE_REPO" main | cut -f1)"
    printf 'fixes: %s (fram446742 fixes/upstream)\n' "$(git ls-remote "$PIN_REPO" fixes/upstream | cut -f1)"
    ;;

  check)
    git fetch --quiet --no-tags "$BASE_REPO" main:refs/remotes/base/main
    git fetch --quiet --no-tags "$PIN_REPO" fixes/upstream:refs/remotes/pin/fixes

    if ! git merge-base --is-ancestor refs/remotes/base/main refs/remotes/pin/fixes; then
      printf 'the base fork moved: rebase fixes/upstream (%s fixes-rebase) and bump the pin\n' "$0" >&2
      exit 1
    fi

    fixes=$(git rev-parse refs/remotes/pin/fixes)
    if [ "$(pin)" != "$fixes" ]; then
      printf 'note: pin %s lags fixes/upstream head %s\n' "$(pin)" "$fixes"
    fi
    printf 'ok: pin is on top of the base fork\n'
    ;;

  bump)
    rev=${2:-}
    if [ -z "$rev" ]; then
      printf 'usage: %s bump <rev>\n' "$0" >&2
      exit 2
    fi

    sed -i -E 's/rev = "[0-9a-f]{40}"/rev = "'"$rev"'"/' Cargo.toml
    grep -n 'rev = ' Cargo.toml
    # Compile errors here name every upstream API this project uses that
    # changed. The runner needs no PyO3 artifacts to type-check.
    cargo test
    cargo check -p pyoxide-runner
    ;;

  fixes-rebase)
    cd "$FORK_CHECKOUT"
    git fetch origin --prune
    git checkout fixes/upstream
    git rebase origin/main
    git log --oneline -3
    printf 'pin: %s\n' "$(git rev-parse HEAD)"
    printf 'publish: git push --force-with-lease myrepo fixes/upstream\n'
    ;;

  *)
    printf 'usage: %s <status|check|bump <rev>|fixes-rebase>\n' "$0" >&2
    exit 2
    ;;
esac
