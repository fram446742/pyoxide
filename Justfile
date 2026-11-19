set shell := ["bash", "-uc"]
set positional-arguments

default:
  @just --list

# Build the binaries against the pinned upstream revision.
build:
  cargo build

# Compile and run every test.
test:
  cargo test

# Move the pinned upstream revision; compile errors name what changed.
upstream-bump rev:
  sed -i -E 's/rev = "[0-9a-f]{40}"/rev = "{{rev}}"/' Cargo.toml
  grep -n 'rev = ' Cargo.toml
  cargo test

# Compare the pinned revision with the head of the base fork.
upstream-status:
  git ls-remote https://github.com/ntamas/PyOxidizer main | cut -f1
  grep -oE 'rev = "[0-9a-f]{40}"' Cargo.toml
