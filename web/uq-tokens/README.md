# uq-tokens

Makes a web app (Tauri, Electron, or a plain page) follow the uQ theme. `uq-tokens.js` turns
uQ's `tokens.json` into CSS custom properties on `<html>`; `uq-tokens.css` holds the fallbacks
your app uses when uQ isn't there. No dependencies, no build step.

Version: **1.0.0** (the first line of `uq-tokens.js` says `// uq-tokens 1.0.0`).

## Vendoring

Copy three files into your app and commit them:

```
your-app/src/vendor/uq-tokens/uq-tokens.js
your-app/src/vendor/uq-tokens/uq-tokens.css
your-app/scripts/check-uq-tokens.sh        (from web/uq-tokens/check-uq-tokens.sh)
```

Edit the values in `uq-tokens.css` to your app's own theme (keep the property names). Don't edit
`uq-tokens.js`; to update, copy the new release over it and bump the pinned version.

Pin the version in CI so drift is visible:

```sh
scripts/check-uq-tokens.sh 1.0.0 src/vendor/uq-tokens/uq-tokens.js
```

It exits 0 when the header matches, 1 on a mismatch, a missing header or a missing file, and 2
on bad usage.

## Properties

Set inline on the root element (so they win over `uq-tokens.css`):

| Property | From `tokens.json` | Example |
|---|---|---|
| `--uq-bg` | `colors.bg` | `#1f1f22` |
| `--uq-surface` | `colors.surface` | `#2b2b2e` |
| `--uq-surface-alt` | `colors.surface_alt` | `#353538` |
| `--uq-overlay` | `colors.overlay` | `#464649` |
| `--uq-text` | `colors.text` | `#e4e4e7` |
| `--uq-text-muted` | `colors.text_muted` | `#a9a9ac` |
| `--uq-accent` | `colors.accent` | `#8ab4f8` |
| `--uq-accent-text` | `colors.accent_text` | `#1f1f22` |
| `--uq-border` | `colors.border` | `#5a5a5d` |
| `--uq-success` | `colors.success` | `#81c995` |
| `--uq-warning` | `colors.warning` | `#fdd663` |
| `--uq-error` | `colors.error` | `#f28b82` |
| `--uq-radius` | `ui.radius` | `10px` |
| `--uq-gap` | `ui.gap_in` (spacing between elements) | `5px` |
| `--uq-gap-in` | `ui.gap_in` | `5px` |
| `--uq-gap-out` | `ui.gap_out` (padding to an edge) | `12px` |
| `--uq-border-width` | `ui.border_width` | `2px` |
| `--uq-font-size` | `fonts.size` | `11px` |
| `--uq-font-ui` | `fonts.ui` | `"Inter"` |
| `--uq-font-mono` | `fonts.mono` | `"JetBrains Mono"` |

And the attribute `data-uq-variant="dark"` or `"light"` from `variant`.

Each value is checked before it is set: colours must be `#rrggbb` or `#rrggbbaa`, lengths finite
numbers from 0 to 1000 (written as px), font names 1–100 characters with no `; { } " ' \`,
control characters, `url(` or `/*` (written quoted). A value that fails is left out, and
`uq-tokens.css` (or your own CSS) supplies it. Keys it doesn't know (`icons`, `terminal`, extra
colours) are ignored. Font properties hold one family, so add your own fallbacks:

```css
body {
    background: var(--uq-bg);
    color: var(--uq-text);
    font: var(--uq-font-size) var(--uq-font-ui), system-ui, sans-serif;
}
code { font-family: var(--uq-font-mono), ui-monospace, monospace; }
```

## API

```js
import { applyTokens, clearTokens, parseTokens, watchTokens } from "./vendor/uq-tokens/uq-tokens.js";
```

- `parseTokens(text)`: the parsed object, or `null` for anything that isn't a JSON object. Never throws.
- `applyTokens(tokens, root = document.documentElement)`: sets every valid property and removes
  the rest, so nothing stale stays from an earlier call. Anything but an object clears. Returns
  the number of properties set.
- `clearTokens(root = document.documentElement)`: removes every property and the attribute.
- `watchTokens({ read, subscribe, root })`: applies now and whenever `subscribe`'s callback
  fires. `read()` returns the file text or `null`, or a Promise of either; `subscribe(cb)` may
  return an unsubscribe function or a Promise of one. If `read()` returns `null`, throws, rejects
  or returns bad JSON, the properties are removed and your own theme applies. Returns
  `{ ready, refresh, stop }`.
- `VERSION`, `PROPERTIES` (the property names above), `mapTokens(tokens)`, and the validators
  `validColor`, `validLength`, `validFont`.

The module never reads files itself and never throws. It only calls
`root.style.setProperty/removeProperty` and `setAttribute/removeAttribute`.

## Where the file is

`$XDG_STATE_HOME/uq/theme/tokens.json`, with `$XDG_STATE_HOME` defaulting to `~/.local/state`.
uqd replaces the whole `theme/` directory atomically when the theme changes, so watch the
**parent** directory (`$XDG_STATE_HOME/uq/`): a watch on the file or on `theme/` itself is lost
after the first change.

## Recipe: Tauri v2

`src-tauri/Cargo.toml`: `notify = "8"` and `dirs = "6"`.

```rust
// src-tauri/src/uq_tokens.rs
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

fn uq_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| dirs::home_dir().map(|h| h.join(".local/state")))
        .map(|s| s.join("uq"))
}

/// The tokens.json text, or None when uQ isn't installed or the file is unreadable.
#[tauri::command]
pub fn uq_tokens() -> Option<String> {
    let text = std::fs::read_to_string(uq_dir()?.join("theme/tokens.json")).ok()?;
    (text.len() < 64 * 1024).then_some(text)
}

/// Watches the parent of theme/ (it is swapped atomically) and emits `uq-tokens-changed`.
pub fn watch(app: &AppHandle) {
    let Some(dir) = uq_dir() else { return };
    let theme = dir.join("theme");
    let handle = app.clone();
    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if res.is_ok_and(|ev| ev.paths.iter().any(|p| p.starts_with(&theme))) {
            let _ = handle.emit("uq-tokens-changed", ());
        }
    });
    let Ok(mut watcher): notify::Result<RecommendedWatcher> = watcher else { return };
    if watcher.watch(&dir, RecursiveMode::NonRecursive).is_ok() {
        app.manage(std::sync::Mutex::new(watcher)); // keep it alive
    }
}
```

In `main.rs`: `.invoke_handler(tauri::generate_handler![uq_tokens::uq_tokens])` and
`.setup(|app| { uq_tokens::watch(app.handle()); Ok(()) })`. If `~/.local/state/uq` doesn't exist
yet, the app keeps its own theme until it is restarted.

Front end:

```js
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { watchTokens } from "./vendor/uq-tokens/uq-tokens.js";

watchTokens({
    read: () => invoke("uq_tokens"),
    subscribe: (cb) => listen("uq-tokens-changed", cb)
});
```

## Recipe: Electron

Main process:

```js
const { app, ipcMain, BrowserWindow } = require("electron");
const fs = require("node:fs");
const path = require("node:path");

const state = process.env.XDG_STATE_HOME && path.isAbsolute(process.env.XDG_STATE_HOME)
    ? process.env.XDG_STATE_HOME : path.join(app.getPath("home"), ".local/state");
const uqDir = path.join(state, "uq");

ipcMain.handle("uq-tokens:read", () => {
    try { return fs.readFileSync(path.join(uqDir, "theme/tokens.json"), "utf8"); }
    catch { return null; }
});

app.whenReady().then(() => {
    try {
        // The parent directory: theme/ is replaced atomically.
        fs.watch(uqDir, (_event, name) => {
            if (name === "theme" || name === null)
                for (const w of BrowserWindow.getAllWindows()) w.webContents.send("uq-tokens-changed");
        });
    } catch { /* no uQ here: the app's own theme applies */ }
});
```

Preload (with `contextIsolation: true`):

```js
const { contextBridge, ipcRenderer } = require("electron");
contextBridge.exposeInMainWorld("uqTokens", {
    read: () => ipcRenderer.invoke("uq-tokens:read"),
    subscribe: (cb) => {
        const f = () => cb();
        ipcRenderer.on("uq-tokens-changed", f);
        return () => ipcRenderer.removeListener("uq-tokens-changed", f);
    }
});
```

Renderer:

```js
import { watchTokens } from "./vendor/uq-tokens/uq-tokens.js";
if (window.uqTokens) watchTokens({ read: window.uqTokens.read, subscribe: window.uqTokens.subscribe });
```

## Recipe: plain browser

Nothing to do: link `uq-tokens.css` and use the properties. A browser page can't read
`~/.local/state`, so the fallback values apply. `applyTokens()` works if you get the JSON some
other way (for example from your own server).

```html
<link rel="stylesheet" href="vendor/uq-tokens/uq-tokens.css">
```

## Tests

```sh
node --test 'web/uq-tokens/test/*.test.js'
```
