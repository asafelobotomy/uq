# uQ's kits for apps

Files that Bindle and uQ's native apps copy into their own repositories, so every app reads uQ's
theme, logs its problems and names uQ's paths the same way, without depending on uQ to build or
run. Apps vendor them unchanged and pin a version; CI checks the copies.

| Kit | Version | What it is |
|---|---|---|
| [`uq-consts`](uq-consts/) | 1.0.0 | the shared values (paths, install folders, D-Bus names, `uq_api`, motion, shape, type, fallback palette) for Rust, C++, QML, Python, JS and shell, and `UqThemeBase.qml` |
| [`uq-theme`](uq-theme/) | 1.1.0 | reading and following `tokens.json` (Rust, C++, Python), and following uQ's KDE colour scheme (C++) |
| [`uq-diag`](uq-diag/) | 1.0.1 | problem codes, the log format and crash records (Rust, C++) |
| [`../web/uq-tokens`](../web/uq-tokens/) | 1.0.0 | `tokens.json` as CSS custom properties for web front ends |

## In an app

List what you vendor in `uq/native.toml`:

```toml
# uQ kits this repo vendors. CI: scripts/check-uq-native.sh
[uq-consts]
version = "1.0.0"
files = ["qml/UqThemeBase.qml", "src/uq_consts.rs"]

[uq-theme]
version = "1.1.0"
files = ["src/uq_theme.rs"]

[uq-diag]
version = "1.0.1"
files = ["src/uq_diag.rs"]
codes = "uq/diag/myapp.toml"   # optional: the codes the sources log and the list agree
sources = ["src", "qml"]
```

Copy [`check-uq-native.sh`](check-uq-native.sh) to `scripts/check-uq-native.sh` and run it in CI.
It checks each file's version header and that its contents are exactly that release's (it knows
every release's checksums), and, for uq-diag, the problem codes. It replaces the older per-kit
scripts (`check-uq-theme.sh`, `check-uq-diag.sh`, `check-uq-tokens.sh`, `check-diag-codes.sh`).

To update, from a uQ checkout:

```sh
scripts/check-uq-native.sh --sync ../uq
```

copies every listed file's current version (and the script itself) and bumps the pins.

## Changing a kit here

Change the files, bump the version on their first lines (uq-consts: in `consts.toml`, then run
`uq-consts/gen.py`), and run `./gen-check.sh`. It adds the new checksums to the kit's `SUMS`,
which keeps every earlier release's, and rewrites `check-uq-native.sh`. CI fails when either is
stale.
