# Contributing

## Build

Pop!_OS 24.04 / Ubuntu 24.04 with COSMIC:

```sh
sudo apt install build-essential pkg-config libxkbcommon-dev libwayland-dev
cargo run -p ember                    # the app
cargo run -p ember -- --page cpu      # open on a page
cargo run -p ember -- --preview sound # one panel applet's popup, for layout work
cargo run -p ember-applet -- sound    # an applet (needs to run inside cosmic-panel to dock)
cargo run -p emberd                   # history + alerts service
```

Workspace: `core` (readings and controls, no UI), `ui` (comic drawing kit, shared widgets,
Quick panel), `app`, `applet`, `widget`, `daemon`, `helper` (root helper).

## Before a pull request

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

- Keep the look in `DESIGN.md`: ink outlines, hard shadows, stepped motion, no blur.
- Readings are `Option`; show `--` when something can't be measured, never a fake value.
- Anything touching `crates/helper` needs tests for the new argument's bad inputs.
- If you rename or drop a file Ember installs in a user's home, add the old path to
  `OBSOLETE` in `install.sh` so `--update` cleans it up.

## Release

Bump `version` in `Cargo.toml`, add a `CHANGELOG.md` entry, tag `vX.Y.Z`. CI builds the
packages with `packaging/build-deb.sh` and attaches them to the GitHub release.
