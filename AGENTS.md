# Agent notes for uQ

A themeable Hyprland desktop for Arch and Manjaro: `uqd` (session daemon, `org.uq.Desktop1`), `uqctl` (CLI), a Quickshell shell (`shell/`, `lock/`, `greeter/`), the `uq-settings` app (cxx-qt, `apps/uq-settings`) and a root helper (`crates/uq-settings-helper`). Rust workspace, edition 2024. Design and plans are in `docs/superpowers/`; user-facing behaviour is in `README.md`.

## Verify

CI (`.github/workflows/ci.yml`) runs all of this in an `archlinux` container as an unprivileged user. Run the parts your change touches before committing.

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked                           # needs dbus and lua installed
cargo test --workspace --locked -- --include-ignored      # what CI runs; adds Hyprland --verify-config for every theme
cargo test -p uqd --test verify_config -- --ignored       # only the Hyprland config check
scripts/lint-shell.sh                                     # shell/, lock/, greeter/
scripts/lint-settings.sh                                  # apps/uq-settings/qml
cargo build --locked -p uq-settings                       # tests/shell/settings_pages_load.sh opens every page with this build
for t in tests/shell/*.sh; do bash "$t"; done
QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml
QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml
node --test 'web/uq-tokens/test/*.test.js'
cargo test --manifest-path native/uq-theme/test/Cargo.toml   # + cmake -S native/uq-theme/test for the C++ copy
cargo deny check advisories
```

Heavier checks, each safe for the real session:

- `scripts/smoke-shell.sh`: the shell on a headless Hyprland with a private bus and runtime dir. Needs Hyprland, quickshell, jq and a DRM card. Exit 77 means skipped, not passed.
- `tests/live/run.sh --list` shows the Settings live checks. On a real desktop they change settings and put them back, and ask first unless `--yes`. CI runs them in a sandbox (`tests/live/ci-in-container.sh`).
- `tests/vm/run.sh`: installs HEAD's packages in a QEMU/KVM Arch guest and runs `tests/vm/checks/`. Needs `/dev/kvm`; state in `~/.cache/uq-vm`.
- `scripts/check-companion-pages.sh target/debug/uqctl <app>/uq ...`: checks other apps' Settings pages against this uQ (`companions.yml` runs it weekly and when the page loader changes).

## Developing on a machine that runs uQ

Read `docs/developing.md`. In short:

- `scripts/uq-dev session` opens a nested desktop running this checkout's `uqd` and `shell/`, with sandboxed config and a private session bus. Use it instead of restarting the real `uqd`, `uq-shell` or `uq-lock` units.
- `scripts/uq-dev run <repo>` runs an app from `~/Projects/<repo>` with throwaway config dirs.
- `scripts/uq-dev promote <repo>` builds and installs a package from the checkout's HEAD; `rollback <package>` undoes it. Ask before promoting: it replaces what the user is running.
- `uq-dev promote`, `scripts/build-local-pkg.sh` and `tests/vm/run.sh` all pack HEAD with `git archive`. Uncommitted changes are not in the build.

## Conventions the linters enforce

- No `Text`, `Label`, `TextEdit`, `TextInput` or `Qt.createQmlObject` in QML. Shell text goes through `UqText` (`shell/core/UqText.qml`), Settings text through `UqLabel` (`apps/uq-settings/qml/components/UqLabel.qml`). Both render plain text only, so strings from apps and notifications can't inject markup.
- Settings controls show data through `UqLabel` content items: `text:` may only be a string literal, and a `ComboBox` needs `contentItem:` and `delegate:`.
- Text people read in Settings uses plain words. `scripts/settings-jargon.txt` lists the banned ones; end a line with `// jargon-ok: <reason>` for a real exception.
- Every `.qml` and `.js` under `apps/uq-settings/qml` must be listed in `apps/uq-settings/build.rs` (`QmlFile::from("...")`). A missing file passes `qmltestrunner` and fails in the real app with "X is not a type".
- Every `tracing::warn!`/`error!` starts with a code, `"[AREA-NNN] …"`, and every QML `console.warn`/`console.error` has `uq[AREA-NNN]` (Settings: `uq-settings[AREA-NNN]`), from `crates/uq-core/src/diag/codes.toml`. A new kind of problem gets a new code there; never reuse a number (retire it). `cargo test -p uq-core --test diag_codes` checks both ways. See `docs/diagnostics.md`.
- Golden files: `UQ_BLESS=1 cargo test -p uq-core` rewrites them (`render_golden`, `uqtheme_corpus`). Read the diff before committing it.

## Release

The full recipe and gate are in `docs/releasing.md`; follow it. What is easy to get wrong:

- The workspace `version` in `Cargo.toml` is the version. `scripts/bump-version.sh X.Y.Z` (or `X.Y.Z-rc.N`) sets it in `Cargo.toml`, `Cargo.lock` and `packaging/uq/PKGBUILD` (`pkgrel=1`), and regenerates `packaging/uq/.SRCINFO`. Never edit those by hand; `bash tests/shell/version_test.sh` checks them.
- A release candidate's `pkgver` has no dash: `1.0.0-rc.1` is `1.0.0rc1`.
- Move the `[Unreleased]` notes in `CHANGELOG.md` under `## [X.Y.Z] - YYYY-MM-DD` first: the GitHub Release notes are that section.
- Commit `release: vX.Y.Z`, merge to `main`, then tag by hand: `git tag -a vX.Y.Z -m "uQ X.Y.Z"` and push the tag. `release.yml` fails before publishing if the tag and `Cargo.toml` disagree.
- The AUR push is manual, to `ssh://aur@aur.archlinux.org/uq.git`. The package builds from the release asset `uq-X.Y.Z.tar.gz`, not GitHub's `/archive/` tarball, so `sha256sums` can only be filled in after the release is published. The run's `aur` artifact has the finished `PKGBUILD` and `.SRCINFO`. Commit the same two files back to `packaging/uq/` so the repo mirrors the AUR.
- `uq-hyprland-pin` (`packaging/uq-hyprland-pin/`) has its own `pkgver`: the lowest Hyprland allowed. Push it to the AUR only when the pin changes.

## Gotchas

- Hyprland is pinned to one series (0.56.2 to 0.57.x now). Moving the pin means changing `packaging/uq-hyprland-pin/PKGBUILD`, `_pin` in `packaging/uq/PKGBUILD` and the series check in `ci.yml` together, then re-running `verify_config`. CI fails when Arch ships a Hyprland outside the series.
- Hyprland refuses to run as root and needs `XDG_RUNTIME_DIR`, even for `--verify-config`.
- `uqd` writes `~/.config/hypr/uq/generated.lua` and the rendered theme files. Never edit generated output; change the templates (`templates/*.j2`) or `uqd`. The user's own settings live in `user.lua`, which uQ never rewrites.
- The launcher is written `look=see>do_` in text people read and `lookseedo` in code and IPC. Older docs call it Spotlight.
- `templates/provider` is a standalone example crate for other repos and is excluded from the workspace. `cargo` commands at the root don't build or test it.
- Companion apps (scdr, crumbs, noti-fi, XZram, scale_tail, emobie, bindle) ship Settings pages from their own `uq/` folders. A change to the page loader, `docs/settings-pages.md`, `docs/integrating.md` or `templates/` can break them: run `check-companion-pages.sh` against the checkouts in `~/Projects`.
- `deny.toml` ignores named advisories with a reason each. Remove an entry when its cause is gone instead of adding blanket ignores.
