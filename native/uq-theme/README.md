# uq-theme

Makes a native app (Qt 6 C++ or Rust) follow the uQ theme live. One copy of the code that reads
uQ's `tokens.json` and follows it with a file watch, instead of one per app. Web apps (Tauri,
Electron) use [`web/uq-tokens`](../../web/uq-tokens) instead.

Version: **1.0.0** (the first line of each file says `uq-theme 1.0.0`).

| File | For | Needs |
|---|---|---|
| `uq_theme.rs` | Rust apps (cxx-qt and others) | `serde_json`, `libc` |
| `uqtheme.h`, `uqtheme.cpp` | Qt 6 C++ apps | Qt Core |
| `check-uq-theme.sh` | every app's CI | bash, sha256sum |

## What you get

Both languages give the same object, the shape uQ native apps' QML `Theme` singletons read:

```
{present: true, name, variant: "dark"|"light",
 colors: {bg, surface, surface_alt, overlay, text, text_muted, accent, accent_text, border,
          success, warning, error},
 radius, gapIn, gapOut, borderWidth, fontUi, fontMono, fontSize, iconTheme}
```

or `{present: false}` when uQ isn't running there, the file doesn't parse, or any colour is
missing or not `#rrggbb` / `#rrggbbaa` (then follow the desktop's own theme). Numbers are clamped
(radius 0–32 and so on), so a hostile file can't make a window unusable.

The file is `$XDG_STATE_HOME/uq/theme/tokens.json` (only an absolute `XDG_STATE_HOME` counts),
else `~/.local/state/uq/theme/tokens.json`. Each app may name its own override variable for tests
and screenshots (for example `BINDLE_UQ_TOKENS`).

**No polling.** uqd replaces the whole `theme/` folder at once (it renames a filled staging folder
over it), so the watch is on that folder and its parent, or on the nearest folder that exists when
uQ hasn't written a theme yet. A burst of writes is read once (150 ms later), and only a real change
is reported.

## Rust

Copy `uq_theme.rs` to `src/uq_theme.rs`, add `libc = "0.2"` and `serde_json = "1"`, and declare it
so your own rustfmt settings leave it alone:

```rust
#[rustfmt::skip]
mod uq_theme;

let path = uq_theme::tokens_path("MYAPP_UQ_TOKENS");
let now = uq_theme::load(path.as_deref());           // serde_json::Value
uq_theme::watch(path, move |json| qt.queue(move |b| b.set_theme_json(json.into())).is_ok());
```

`watch` runs on a thread of its own and calls back with the JSON text on each change, until the
callback returns false.

## C++ (Qt 6)

Copy `uqtheme.h` and `uqtheme.cpp` into your sources (they need `AUTOMOC`):

```cpp
#include "uqtheme.h"

auto *theme = new uqtheme::Watcher(uqtheme::tokensPath("MYAPP_UQ_TOKENS"), &app);
engine.rootContext()->setContextProperty("uqTheme", theme);   // QML: uqTheme.tokens.colors.accent
QObject::connect(theme, &uqtheme::Watcher::tokensChanged, [&] { applyPalette(theme->tokens()); });
```

`uqtheme::load(path)` and `uqtheme::parse(bytes)` are there for a one-off read.

## Keeping copies in step

Don't edit the vendored files; to update, copy the new release over them and bump the pin. In CI:

```sh
scripts/check-uq-theme.sh 1.0.0 src/uq_theme.rs
```

It checks each file's header and that its contents are exactly that release's (the checksums are
in the script), and exits 1 otherwise.

Changing these files here: bump the version on the first line of all three, run `./gen-check.sh`
(it writes `check-uq-theme.sh` with the new checksums and keeps the old ones in `SUMS`), and copy
the new script into the apps along with the files. uQ's CI runs the tests in `test/` and fails when
`check-uq-theme.sh` is out of date.

## Who uses it

| App | Copy |
|---|---|
| Bindle (bindle-gui, bindleUP) | Rust |
| bindaru | Rust |
| uQview | Rust |
| appOFholding | Rust |
| kyko | Rust |
| reelo | C++ |
| XZram | C++ |

hauler and noti-fi's panel follow uQ's generated KDE colour scheme instead (so Kirigami and
qqc2-desktop-style controls match), with their own file watch. AV Morning Star and oysterAV
(Python) have their own copies for now; romforge and emobie use `web/uq-tokens`.
