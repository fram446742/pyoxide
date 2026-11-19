# pyoxide

Packaging and running self-contained Python applications, on top of the
PyOxidizer crates.

This is a clean downstream: instead of forking PyOxidizer's ~69 k lines, this
workspace depends on them at a pinned revision (`Cargo.toml`, one line) and
keeps only the logic that PyOxidizer does not expose. See `PORTING.md` for the
feature inventory and what still has to move here.

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
