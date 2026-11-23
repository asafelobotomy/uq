# uq-consts

The values uQ, Bindle and uQ's native apps share, in one file: `consts.toml`. `gen.py` writes it out
for each language, and apps vendor those files unchanged. uQ compiles the same Rust file
(`uq_core::uq_consts`), and `crates/uq-core/tests/native_consts.rs` holds the shell's `Motion.qml`
and `Theme.qml`, the Graphite theme and uqd's paths to it, so uQ can't drift from what it hands out.

Version: **1.0.0** (the first line of each output says `uq-consts 1.0.0`).

| File | For | Needs |
|---|---|---|
| `uq_consts.rs` | Rust apps (editions 2021 and 2024) | nothing |
| `uqconsts.h` | Qt 6 C++ apps (header-only) | Qt Core, for the path functions |
| `UqThemeBase.qml` | a QML app's `Theme` singleton | QtQuick |
| `uq_consts.py` | Python apps | the standard library |
| `uq-consts.js` | web front ends (Tauri, Electron), an ES module | nothing |
| `uq-consts.sh` | shell scripts and PKGBUILDs (`. uq-consts.sh`) | POSIX sh |

## What's in it

| Group | Values |
|---|---|
| `paths.state` | `uq/`, `uq/theme/`, `uq/theme/tokens.json`, `uq/crashes/` under `$XDG_STATE_HOME` (absolute only) or `~/.local/state` |
| `paths.cache` | `quickshell/crashes/` under `$XDG_CACHE_HOME` or `~/.cache` |
| `install` | `/usr/share/uq` and the folders packages install integrations into: settings pages, widgets, launcher providers and actions, window rules, problem codes, themes |
| `dbus` | `org.uq.Desktop1` (name, path, interface), the launcher provider's path and interface |
| `api` | the newest `uq_api` a manifest may declare, and a page's default |
| `motion` | fast 100, base 140, slow 220 ms, pop scale 0.96, OutCubic in, InCubic out |
| `shape` | fallback radius 10; controls use radius − 2, chips radius − 4; gap 8 |
| `type` | fallback size 11 pt; small = body − 1 (at least 8); heading = body × 1.5 |
| `palette.dark`, `palette.light` | uQ's Graphite theme, the palette to fall back to off uQ |

Not in it: each app's own app id, desktop id, bus name, socket and problem-code prefix. Those
belong to the app.

## Rust

```rust
#[rustfmt::skip]
mod uq_consts;

let tokens = uq_consts::paths::tokens();                       // Option<PathBuf>
let dir = uq_consts::install::PAGES_DIR;                       // "/usr/share/uq/settings/pages"
let bus = uq_consts::dbus::DESKTOP_NAME;
```

## C++

```cpp
#include "uqconsts.h"
const QString tokens = uqconsts::paths::tokensPath();
QDBusInterface uq(uqconsts::dbus::desktopName, uqconsts::dbus::desktopPath, uqconsts::dbus::desktopInterface);
```

## QML: the theme base

`UqThemeBase.qml` is a whole `Theme` singleton: uQ's colours (with Graphite fallbacks), `radius`,
`radiusControl`, `radiusInner`, `gapIn`, `gapOut`, `borderWidth`, `gap`, `fontUi`, `fontMono`,
`bodySize`, `smallSize`, `headingSize`, `motionFast`, `motionBase`, `motionSlow`, `easing`,
`easingExit`, `popScale` and `reduceMotion`. Copy it next to your `Theme.qml`, add it to the same
QML module, and feed it uq-theme's tokens:

```qml
pragma Singleton
import QtQuick
import MyApp

UqThemeBase {
    tokens: App.theme.tokens          // uq-theme's load() / Watcher.tokens
    readonly property color scrim: "#000000"   // the app's own extras
}
```

On uQ it follows the theme and Reduce motion live; off uQ it is Graphite dark with the app font.

## Python, JS and shell

```python
import uq_consts
uq_consts.tokens()                    # Path or None
uq_consts.INSTALL_PAGES_DIR, uq_consts.PALETTE_DARK["accent"]
```

```js
import { motion, palette, statePaths } from "./vendor/uq-consts.js";
```

```sh
. scripts/uq-consts.sh
install -Dm644 page.toml "$pkgdir$UQ_INSTALL_PAGES_DIR/myapp/page.toml"
cat "$(uq_tokens)"
```

## Changing it

Edit `consts.toml`, bump `version` (a new value is a minor bump; a changed or removed one is
major), then run `./gen.py` and `../gen-check.sh`, and commit the outputs with it. uQ's CI fails
when an output or the check script is stale, and runs each language's tests in `test/`.
