# pyoxide

Packaging and running self-contained Python applications, on top of the
PyOxidizer crates.

This is a clean downstream: instead of forking PyOxidizer's ~69 k lines, this
workspace depends on them at a pinned revision (`Cargo.toml`, one line per
crate) and keeps only the logic that PyOxidizer does not expose. Today the pin
points at the fork's `fixes/upstream` branch — `origin/main` plus two fixes
that are not upstream yet; `PORTING.md` says why and how to shrink it to zero.

```console
$ just build
$ cargo run --bin pyoxide -- dist-info some-distribution.tar.zst
```

## Updating upstream

```console
$ just upstream-status          # pinned rev vs base fork head
$ just upstream-bump <sha>      # move the pin; compile errors name what broke
```

A pin bump is a normal commit. Nothing is vendored, so there is no merge
conflict to resolve and no second copy of upstream to keep alive.
