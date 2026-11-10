# uQ Phase 4: repo integrations design

Date: 2026-11-10 · Status: draft for review (written from the roadmap without a brainstorming
session; the choices marked **Decision** in §9 are the ones to confirm)
Parent spec: `2026-11-08-uq-desktop-design.md` (§2.4 extension points, §3 components per repo,
§7 phase 4). Builds on: `2026-11-08-uq-phase2-shell-design.md` (launcher provider API, bridge) and
`2026-11-08-uq-phase3-settings-design.md` (plugin pages, §2.5). Roadmap: M2 in
`../plans/2026-11-09-uq-road-to-1.0.md`.

## 1. Intent

Phase 4 turns the author's other apps into native parts of uQ without making any of them a
dependency. uQ grows the missing host side of the four extension points, and each repo ships the
small files that plug into them.

**What the roadmap fixed**

- Integrations land one repo per slice, in design order: scale_tail, appOFholding, hauler,
  emobie, XZram, scdr, crumBS, oysterAV, romforge. noti-fi already landed in Phase 2.
- `uq-tokens` is a small, dependency-free JS/CSS snippet for web-based apps. It lives in this
  repo under `web/uq-tokens/` and is vendored into romforge, SonicSound, emobie, noveleditor and
  gutcheck.
- emobie becomes a headless `:` launcher provider over D-Bus, so uQ never opens its Tauri window
  for emoji search. Its standalone app stays for other desktops.
- Each repo keeps its own releases. uQ's side is the optional dependency and the docs.

**Assumptions**

- Phase 3 is complete, including slice 6 (plugin-page SDK, `docs/settings-pages.md`,
  `templates/settings-page/`). Phase 4 extends that SDK; it does not replace it.
- Every integrated repo belongs to the same author, so integration files live in each repo's own
  `uq/` folder and are installed by that repo's own package (§2.6). `uq-<name>` adapter packages
  are kept as a fallback only.
- The shell has no widget host yet. Phase 2 hard-codes Tailscale status in the Status island and
  the notification bell in the Notifications island; nothing loads third-party QML.

**Success criteria**

1. uQ starts and runs with none of the nine apps installed: no errors in the journal, no empty
   slots, no placeholder pages.
2. Installing any one of them adds its widget, page, launcher results or actions **without logging
   out**. Removing it takes them away again within a few seconds.
3. A widget, page or provider that crashes, hangs or returns junk never breaks the bar, the
   launcher or the settings app. It is hidden or shows an error card.
4. Every web-based app that vendors `uq-tokens` follows a theme or variant change live, and falls
   back to its own theme when uQ isn't there.
5. `:smile` in Spotlight lists emoji from emobie without starting emobie's window, and Enter
   copies the emoji.

**Non-goals**

- A sandbox for integration code. Widgets and pages are trusted package code; the helpers limit
  mistakes, not attacks (same threat model as Phase 3 §2.5).
- Integrations from user-writable directories. Everything is loaded from `/usr/share/uq/` only.
  (Launcher `.provider` files in `~/.local/share/uq/launcher/` stay as Phase 2 defined them.)
- New features inside the integrated apps beyond what their uQ pieces need.
- Packaging or AUR publishing of the integrated apps themselves. Each repo's own release process
  covers that; this spec only lists what each one must add.

## 2. Architecture

### 2.1 What exists and what's missing

| Extension point | Host today | Missing |
|---|---|---|
| Theme | `tokens.json` + `ThemeChanged`; noti-fi polls the file | a reusable reader for web apps (`uq-tokens`, §2.5); a short guide for Qt/GTK apps |
| Settings page | Phase 3 plugin pages, `dbus` helper only | a `command` helper for CLI-first repos (§2.3); live discovery of newly installed pages |
| Widget | none | the whole host: manifest, slots, loader, bridge-mediated D-Bus and commands (§2.2) |
| Launcher provider | Phase 2 D-Bus provider API, `.provider` files, `NO_AUTO_START` | starting activatable providers (§2.4.1); actions on app rows (§2.4.2) |

One shared Rust module, `uq_core::integration`, holds the manifest types, validation, the
command runner and the JSON↔D-Bus mapping. The bridge (`uqctl bridge`) and `uq-settings` both use
it, so a widget and a page with the same manifest keys behave the same way.

### 2.2 Widget host

**Location and manifest.** Each widget is a folder `/usr/share/uq/shell/widgets/<id>/` with a
`widget.toml` and a QML file. Only that directory is searched.

```toml
id = "hauler"                 # [a-z0-9-]{1,40}, must equal the folder name
name = "Downloads"            # shown in Settings → Shell → Widgets
slot = "bar"                  # "bar" (Status island icon) | "quick" (quick-settings tile)
qml = "Widget.qml"            # relative, no "..", must exist in the folder
order = 50                    # 0–100; ties break on id

[requires]                    # all listed conditions must hold for the widget to show
bus_name = "org.hauler.Hauler1"   # optional: shown only while this name has an owner
bins = ["hauler"]                 # optional: shown only if these exist in /usr/bin

[dbus]                        # optional: what the widget may reach through the bridge
name = "org.hauler.Hauler1"
path = "/org/hauler/Hauler1"
interface = "org.hauler.Hauler1"
methods = ["Pause", "Resume"]
properties = ["ActiveCount", "BytesDone", "BytesTotal"]
signals = ["Finished"]

[[command]]                   # optional: fixed commands the widget may run (§2.3)
id = "status"
argv = ["/usr/bin/tailscale", "status", "--json"]
output = "json"               # "json" | "text" | "none"
timeout_ms = 3000
```

**Slots.**

- `bar`: a 1-cell icon in the Status island, left of the existing status icons, with an optional
  popup. Room for at most 4; extra widgets are listed under a "…" icon.
- `quick`: a tile in a new **Apps** group at the bottom of quick settings, the same size and style
  as the Phase 2 toggle rows, with an optional detail view. The group is hidden when empty.

**Loading and isolation.**

- The bridge scans the widgets directory at start and watches it with inotify (debounced 1 s), so a
  package install or removal shows up without a relog. It validates each manifest and sends the
  list to the shell as a `Widgets` event. Invalid manifests are skipped and logged once.
- `requires.bus_name` is watched with `NameOwnerChanged`; `requires.bins` is re-checked on each
  rescan. The `Widgets` event carries a `visible` flag per widget.
- The shell loads each visible widget with a `Loader` inside a fixed-size slot. A load error or a
  runtime error from the component unloads it and logs the error once per widget per session.
  A widget is never retried more than once a minute.
- The widget's root item receives a single `ctx` object: `Theme`, `UqText`, `UqInput`, `slot`,
  `dbus` and `command`. It does **not** get `Quickshell.Io` handles or the bridge itself; nothing
  stops trusted package code from importing them, but the template and lint (§2.7) steer away
  from it.
- All text a widget shows goes through `UqText` (plain text), as everywhere else in the shell.
- On lock, popups close as in Phase 2; widgets never draw above the lock.

**Bridge protocol additions** (JSON lines, as Phase 2 §2.3):

| Command | Args | Returns |
|---|---|---|
| `WidgetCall` | widget id, method, args (JSON array) | result as JSON, or an error |
| `WidgetGet` | widget id | all listed properties as a JSON object |
| `WidgetRun` | widget id, command id, args (array of strings) | `{code, output}` (output parsed per `output`) |

| Event | Data |
|---|---|
| `Widgets` | `[{id, name, slot, order, qml_url, visible}]`, on start and on every change |
| `WidgetProps` | `{widget, props}` from `PropertiesChanged`, coalesced to at most 4 per second |
| `WidgetSignal` | `{widget, member, args}` for listed signals only |

**Limits** (enforced in the bridge, not the QML):

- Only the `name`, `path`, `interface` and the listed members in `[dbus]` are reachable. Calls use
  `NO_AUTO_START` unless `requires.bus_name` is empty and the name is activatable.
- D-Bus types are mapped through the interface's introspection data. Only `b y n q i u x t d s o
  as a{sv} a{ss}` and arrays of those are supported; anything else is an error.
- Arguments up to 16 KiB of JSON, results up to 256 KiB, a 2 s deadline per call.
- At most 8 calls in flight per widget; more are rejected with `busy`.

### 2.3 The `command` helper (widgets and pages)

Several repos are CLI-first and already print JSON (crumBS, oysterAV, appOFholding, romforge,
`tailscale`). Asking each of them to grow a D-Bus service only for uQ is a lot of work for little
gain, so widgets and settings pages may run **declared** commands.

- **Declared only.** A command is a `[[command]]` table in `widget.toml` or `page.toml`: `id`,
  `argv`, `output`, `timeout_ms` (default 10 000, max 600 000), and optionally `args` and
  `confirm`.
- **argv.** `argv[0]` must be an absolute path under `/usr/bin` or `/usr/lib`. The fixed argv is
  never changed. The caller's arguments are appended as separate argv entries, and only if the
  manifest declares them:

  ```toml
  args = [{ name = "app", pattern = "^[A-Za-z0-9._:@+-]{1,128}$" }]
  ```

  Each argument must match its anchored pattern in full, and may not start with `-`, so it can
  never become an option. The count must match exactly.
- **No shell.** Commands are spawned directly, never through `sh -c`.
- **Environment.** Scrubbed to `HOME`, `USER`, `LANG`, `LC_*`, `PATH=/usr/bin`, `XDG_*`,
  `DBUS_SESSION_BUS_ADDRESS`, `WAYLAND_DISPLAY` and `UQ_INTEGRATION=1`.
- **Running.** In its own process group with stdin closed. stdout is capped at 4 MiB and stderr
  at 64 KiB; a command over either cap or over its timeout is killed (`SIGTERM`, then `SIGKILL`
  after 2 s) and reported as an error.
- **Concurrency.** One run per command id per widget or page at a time; a second call while one
  runs returns `busy`.
- **`confirm`.** A command with `confirm = "Remove %s and its leftovers?"` makes the host show a
  confirmation dialog with that fixed text (the single `%s` is filled with the first argument,
  shown as plain text). The command runs only on an explicit Confirm.
- **Admin.** A command that needs root uses the app's own polkit flow. uQ's polkit agent shows the
  prompt; uQ never runs anything as root on an integration's behalf.

Phase 3 slice 6's `page.toml` gains `[[command]]` and `[requires]` with the same meaning as above.
Its `dbus` helper keeps its Phase 3 shape. The plugin-page list in `uq-settings` also starts
watching `/usr/share/uq/settings/pages/` with inotify, so a newly installed page appears without
restarting the app.

### 2.4 Launcher additions

#### 2.4.1 Activatable providers

Phase 2 queries only providers whose bus name already has an owner, with `NO_AUTO_START`, so
search never waits for a process to start. That keeps search fast but means a provider must
already be running.

- A `.provider` file may add `Activatable=true`. The bus name must then have a session D-Bus
  service file; the bridge checks with `ListActivatableNames` and ignores the key otherwise.
- When Spotlight or the app menu opens, the bridge calls `StartServiceByName` for each
  activatable provider that has no owner, asynchronously, with a 2 s timeout, at most once per
  30 s per provider. Queries still go only to owned names with `NO_AUTO_START`, so the first
  keystrokes may miss a cold provider, but no search ever waits.
- Providers should exit after about 60 s without a call. The template does this.

#### 2.4.2 App actions

Some integrations act on *apps* rather than adding results: crumBS's "Uninstall…" and
appOFholding's "Check for update". These become **app actions**, shown in the right-click menu
of an app row in Spotlight and in the app menu.

- **Files.** `/usr/share/uq/launcher/actions/<id>.appaction`, in desktop-entry syntax:

  ```ini
  [App Action]
  Name=Uninstall…
  Icon=edit-delete
  Match=*                          # glob on the desktop id; e.g. appofholding-*.desktop
  Exec=/usr/bin/crumbs uninstall --desktop-id %d
  Terminal=true
  ```

- **`%d`** is the only placeholder. It is the desktop id of the row, which must match
  `^[A-Za-z0-9._-]{1,200}\.desktop$`, and it is always one whole argv entry. `Exec` is split with
  desktop-entry quoting rules; no shell is involved.
- **`Terminal=true`** runs it in the terminal chosen in Settings → Default apps (Phase 3
  `shell.terminal`), so interactive tools like crumBS keep their own confirmation prompts.
  `Terminal=false` runs it detached in a `systemd-run --user --scope`, like a launch.
- Actions are read by the bridge, watched with inotify like widgets, and sent to the shell in a
  `AppActions` event. Launching goes through the bridge's existing `Launch` path with a new
  `{app_action, desktop_id}` form; the shell never builds a command line.

### 2.5 `uq-tokens`

A single ES module plus a CSS file, with no dependencies, in `web/uq-tokens/`:

```
web/uq-tokens/uq-tokens.js     ~150 lines: map, apply, watch
web/uq-tokens/uq-tokens.css    fallback custom properties (the app's own defaults go here)
web/uq-tokens/README.md        vendoring, the property list, the Tauri and Electron recipes
web/uq-tokens/test/            node --test unit tests
```

- **Mapping.** `applyTokens(tokens, root = document.documentElement)` sets CSS custom properties
  from `tokens.json`: every colour as `--uq-<name>` (`--uq-bg`, `--uq-surface-1`,
  `--uq-text`, `--uq-accent`, `--uq-success` …), plus `--uq-radius`, `--uq-gap`,
  `--uq-font-ui`, `--uq-font-mono`, and `data-uq-variant="dark|light"` on the root. Unknown keys
  are ignored, and each value is checked (colours `#rrggbb[aa]`, lengths numeric, font names
  `[^;{}]{1,100}`) before it's set, so a broken file can't inject CSS.
- **Reading.** The module never touches the file system itself. The app passes a `read()`
  function that returns the file text (or `null`), and a `subscribe(callback)` that fires on
  change. The README gives a recipe for each runtime:
  - **Tauri v2:** a 40-line Rust command in the app's backend that reads
    `$XDG_STATE_HOME/uq/theme/tokens.json` and watches the **parent** `theme/` directory with
    `notify` (the directory is swapped atomically, so watching the file loses it after the first
    change), emitting a `uq-tokens-changed` event.
  - **Electron:** the same in the main process, exposed through `contextBridge`.
  - **Plain browser / no backend:** none; the fallback CSS applies.
- **Fallback.** If `read()` returns `null` or the JSON fails to parse, the module removes its
  properties and the app's own theme (from `uq-tokens.css` or its own CSS) applies. It never
  throws.
- **Versioning.** The file header carries a version (`uq-tokens 1.0.0`). Vendored copies are
  checked by a small `scripts/check-uq-tokens.sh` in each consuming repo that compares the header
  with the version it pinned, so drift is visible.
- **Consumers:** romforge and emobie in their slices; SonicSound, noveleditor and gutcheck in the
  last slice. gutcheck only if it has a web UI at that time.

### 2.6 Where integration files live and how they install

**Decision: in-repo `uq/` folders, installed by each repo's own package.**

```
<repo>/uq/
  widgets/<id>/widget.toml, Widget.qml      → /usr/share/uq/shell/widgets/<id>/
  pages/<id>/page.toml, Page.qml            → /usr/share/uq/settings/pages/<id>/
  launcher/<id>.provider                    → /usr/share/uq/launcher/
  launcher/actions/<id>.appaction           → /usr/share/uq/launcher/actions/
  dbus/<name>.service                       → /usr/share/dbus-1/services/   (activatable providers)
  README.md                                 what the folder adds and how it was tested
```

- These files are inert without uQ: nothing reads `/usr/share/uq/` unless uQ is installed. So the
  app's main package installs them unconditionally, and no separate package is needed.
- Each repo's PKGBUILD lists `uq` as an `optdepends` with a one-line reason. uQ's PKGBUILD lists
  each app as an `optdepends`, as it already does for `scdr` and `tailscale`.
- A repo that can't ship the folder in its own package (none today) gets a `uq-<name>` adapter
  package under `packaging/` in this repo, built from the same folder layout.
- The QML in a repo's `uq/` folder targets uQ's widget and page context, so it tracks uQ's API
  version: each manifest declares `uq_api = 1`. The host skips manifests with a newer major
  version and says so in Settings → About → Integrations.

### 2.7 Tooling for integration authors

- **`uqctl integrations list`**: every widget, page, provider and app action uQ found, with its
  source path, `visible` state and any validation error. `--json` for scripts.
- **`uqctl integrations check <dir>`**: validates a `uq/` folder before packaging. It runs the
  manifest validation, the text lint (no bare `Text`/`Label`; `UqText` only), checks that every
  `[[command]]` `argv[0]` path exists, and for `.provider` files that the bus name is valid.
  Each repo's CI runs it, pinned to a uQ release.
- **Templates:** `templates/widget/`, `templates/settings-page/` (from Phase 3, extended with a
  `[[command]]` example), `templates/provider/` (a Rust zbus provider with idle exit and a
  session D-Bus service file).
- **Docs:** `docs/integrating.md` covers all four extension points, the threat model, and a
  checklist per extension point. It links `docs/settings-pages.md`.
- **Settings → About → Integrations** lists what's installed and what failed, using
  `uqctl integrations list --json`.

## 3. Per-repo integration

Each slice lands in **two repos**: the app's repo (its `uq/` folder, any new D-Bus or CLI surface,
PKGBUILD changes, its own release), and this repo (the optional dependency, docs, the gate test).

| # | Repo | Toolkit and IPC today | Widget | Settings page | Launcher | Work in the app's repo |
|---|---|---|---|---|---|---|
| 1 | scale_tail | Rust tray (ksni), drives the `tailscale` CLI | `quick`: Tailscale on/off, current exit node, exit-node picker | Tailscale: operator status, auto-connect, accept routes, peers list | — | Package it for the AUR (it ships only an AppImage today); the `uq/` folder. The tray stays for other desktops and is not started under uQ. |
| 2 | appOFholding | Rust + cxx-qt, CLI with subcommands | `quick`: "N updates", opens the page | AppImages: list, update all, update policy, schedule | App actions on `appofholding-*.desktop` rows: "Check for update", "Launch options…" | `--json` for `list` and `check`; confirm its desktop-id prefix; the `uq/` folder |
| 3 | hauler | Rust daemon `haulerd`, JSON-RPC on a unix socket, cxx-qt app | `bar`: progress ring while anything downloads, popup with the queue | — (the app has its own settings) | Provider: a query that is an `http(s)` URL offers "Download with hauler" | A small `org.hauler.Hauler1` D-Bus interface in `haulerd` (§3.1); the `uq/` folder |
| 4 | emobie | Tauri v2 + React, `emobie-inputd` | — | — | Activatable provider for `:` (§3.2) | `emobie-provider` binary, shared emoji data, the `uq/` folder, `uq-tokens` in the app |
| 5 | XZram | CLI, Qt6 GUI, system daemon `xzramd` on `io.github.XZram1` | — | "Memory and swap": zram size and algorithm, swap files, swappiness, over the existing `io.github.XZram.Manager` | — | The `uq/` folder only; the page's admin actions use xzramd's existing polkit actions |
| 6 | scdr | Rust, system daemon `org.scdr.Daemon1`, session agent `org.scdr.Session1` | — | "Scheduling": daemon status, profiles, per-app rules, sched_ext scheduler choice, `scdr-session` on/off | — | The `uq/` folder; uQ's side checks that uQ's own units sit in `session.slice` (design §3) |
| 7 | crumBS | CLI-first, `--json` everywhere, polkit for system work | — | "Storage and cleanup": reclaimable space by category, clean, orphans, history and restore, through `[[command]]` | App action "Uninstall…" on every app row, in a terminal | `crumbs uninstall --desktop-id <id>` (or an equivalent lookup); the `uq/` folder |
| 8 | oysterAV | Python CLI `oyst-cli` with `--json`, GTK4 GUI, polkit helper | `quick`: security status (ok / warning / threat), last scan time | "Security": status, scan a folder, quarantine list, schedules | — | A cheap `status --json` that doesn't start a scan; the `uq/` folder |
| 9 | romforge | Tauri v2 app + `romforge-cli` | — | — | Activatable provider: games by name, Enter starts the game; game-mode rule (§3.3) | `romforge-provider` (or `romforge-cli provider`), `uq-tokens` in the app, AUR packaging (none today), the `uq/` folder |
| 10 | SonicSound, noveleditor, gutcheck | Tauri / web | — | — | — | Vendor `uq-tokens` and add the backend reader |

### 3.1 hauler's status interface

`haulerd` owns `org.hauler.Hauler1` on the session bus while it runs (it's socket-activated, so
the widget's `requires.bus_name` hides it when hauler is idle and stopped).

- Properties: `ActiveCount u`, `QueuedCount u`, `BytesDone t`, `BytesTotal t` (0 when unknown).
  `PropertiesChanged` at most once a second.
- Methods: `PauseAll()`, `ResumeAll()`, `Add(s url)`. `Add` validates the URL as the CLI does.
- Signals: `Finished(s id, s name, b ok)`. Completion notifications stay hauler's own, through
  `org.freedesktop.Notifications` (noti-fi under uQ).

The provider in row 3 is the same process: `org.hauler.Hauler1` also serves
`org.uq.Launcher1.Provider` at `/org/uq/Launcher1`, so it needs no separate binary.

### 3.2 emobie as a headless provider

- **Binary:** `emobie-provider`, a small Rust zbus service on `org.emobie.Provider` with a session
  D-Bus service file (`Activatable=true`). It exits after 60 s idle.
- **Data:** the same emojibase data the app uses, converted at build time into one compact file,
  `/usr/share/emobie/emoji.json` (codepoints, name, keywords, group, skin-tone variants). The app
  and the provider both read that file, so search results match.
- **Search:** the `.provider` file sets `Prefix=:`. The query (prefix stripped) is matched against
  names and keywords; favourites and recents from emobie's prefs file rank first. Up to 20 items,
  with the emoji itself as the title and its name as the subtitle.
- **Actions:** `copy` (default, Enter) puts the emoji on the clipboard with `wl-copy`; `type`
  (Shift+Enter) types it into the focused window through `emobie-inputd` if it's running, else
  falls back to copy.
- **Hotkey:** uQ adds a default bind `uq.emoji` (Super+.) that opens Spotlight prefilled with `:`.
  emobie's own global hotkey stays off under uQ to avoid a double bind; emobie detects uQ by the
  `org.uq.Desktop1` name.

### 3.3 romforge's game-mode rule

The design asks for an "optional game-mode workspace rule". It is opt-in and goes through uqd,
never through a packaged Lua file, so a broken integration can't break `generated.lua`.

- romforge ships `/usr/share/uq/integrations/romforge/rules.toml`:

  ```toml
  [[window_rule]]
  id = "romforge.emulators"
  class = ["^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$"]
  workspace = "name:game"
  fullscreen = true
  ```

- uqd gains a `integration_rules` domain: a list of rule ids the user turned on in Settings →
  Look and layouts → "Rules from apps". It reads the packaged file, validates each field against
  a closed schema (regex syntax checked, workspace names `[A-Za-z0-9 _-]{1,32}`), renders the rule
  through `lua_string`, and verifies `generated.lua` as in Phase 3. A rule whose file disappears
  is dropped on the next render.
- The `game` workspace also gets layout `master`, no gaps and animations off, set through the
  Phase 3 `layouts` and `look` domains when the rule is turned on.

## 4. Data flows

**Install while logged in.** `pacman -S hauler` → files land in `/usr/share/uq/shell/widgets/hauler/`
and `/usr/share/uq/launcher/hauler.provider` → the bridge's inotify watch fires → after 1 s it
re-scans, validates, and emits `Widgets` and its updated provider list → the shell loads the
widget once `org.hauler.Hauler1` has an owner. `uq-settings`, if open, adds any new page to the
Apps category.

**Widget call.** Widget QML → `ctx.dbus.call("PauseAll")` → shell → bridge `WidgetCall` →
manifest check → D-Bus call with a 2 s deadline → result back to the widget's callback.

**Theme change in a web app.** uqd swaps `~/.local/state/uq/theme/` → the app backend's watch on
the parent directory fires → it re-reads `tokens.json` and emits `uq-tokens-changed` →
`applyTokens()` updates the custom properties.

## 5. Error handling

| Failure | Behaviour |
|---|---|
| Invalid `widget.toml`, `page.toml`, `.provider` or `.appaction` | Skipped; one journal line naming the file and the reason; listed with the error in `uqctl integrations list` and Settings → About → Integrations |
| Manifest declares a newer `uq_api` | Skipped with "needs a newer uQ" |
| Widget QML fails to load or throws | Slot unloads; logged once per session; retried at most once a minute |
| Widget's bus name disappears | Widget hidden; reappears when the name returns |
| D-Bus call slow or failing | Error to the widget after 2 s; the widget shows its own error state; the bar is unaffected |
| `[[command]]` times out, overflows or exits non-zero | Killed if needed; `{code, error}` to the caller; stderr (first 2 KiB) in the journal |
| Activatable provider fails to start | Logged once per 10 min; search continues without it |
| App action's program missing | The action is hidden (checked on scan) |
| `uq-tokens` file missing or broken | The app's own theme applies; no error shown |
| Integration rule invalid | uqd rejects it, the toggle returns to off, the page shows the reason |

## 6. Security

- Integration files come only from `/usr/share/uq/` (root-owned, package-managed). The existing
  `~/.local/share/uq/launcher/` `.provider` directory stays, as in Phase 2.
- Commands: fixed absolute `argv[0]`, appended arguments checked against anchored patterns and
  never allowed to start with `-`, no shell, scrubbed environment, timeouts and output caps
  (§2.3). App actions: one placeholder, validated, one argv entry (§2.4.2).
- D-Bus: one name, path and interface per widget or page; listed members only; type allowlist;
  size limits (§2.2).
- Untrusted text: widget and page text, provider results, app-action names, emoji names, game
  titles and every command output shown to the user go through `UqText`/`UqLabel` as plain
  text.
- `uq-tokens`: values validated before they become CSS; no `innerHTML`, no `eval`.
- No integration code runs as root. Admin work goes through each app's own polkit actions.
- The M4 security pass reviews the command runner and the JSON↔D-Bus mapping as root-adjacent
  input handling, with a fuzz target for each.

## 7. Testing

- **`uq_core::integration`:** manifest validation (every key, path traversal in `qml`, bad ids,
  unknown keys), the argument patterns (anchoring, leading `-`, counts), the command runner
  (timeout, caps, process-group kill, scrubbed env), the JSON↔D-Bus mapping for every allowed
  type and rejection of the rest. A `cargo fuzz` target for manifests and for the mapping.
- **Bridge:** integration tests on a private bus with a fake widget service and a fake
  activatable provider: discovery, inotify add and remove, `requires` gating, the member
  allowlist, deadlines, `StartServiceByName` throttling.
- **Shell:** `qmltestrunner` tests for the slot host: a good widget, one that fails to load, one
  that throws later, overflow into "…", the empty Apps group hidden.
- **Settings:** the `command` helper and `confirm` dialog in offscreen QML tests; live discovery of
  a page added while the app is open.
- **`uq-tokens`:** `node --test` for mapping, validation (CSS injection attempts), the fallback
  and repeated changes.
- **Gate test (success criteria 1–2):** a CI job that starts the shell headless with an empty
  `/usr/share/uq/` integration tree, asserts no errors, then copies each repo's `uq/` folder in
  turn (from fixtures pinned per slice) and asserts the widget, page, provider or action appears;
  then removes it and asserts it goes. On a real machine, the slice's live checklist does the same
  with real packages.
- **Per repo:** each repo's CI runs `uqctl integrations check uq/`.

## 8. Code layout (this repo)

```
crates/uq-core/src/integration/   manifest.rs, command.rs, dbus_json.rs, watch.rs, mod.rs
crates/uqctl/src/bridge/          widgets.rs, appactions.rs (new); providers.rs (activation)
crates/uqctl/src/integrations.rs  `uqctl integrations list|check`
crates/uqd/src/rules.rs           integration_rules domain (§3.3)
shell/widgets/                    WidgetHost.qml, Slot.qml, WidgetContext.qml
shell/quick/AppsGroup.qml
apps/uq-settings/src/plugins.rs   (extended: [[command]], [requires], inotify)
web/uq-tokens/                    uq-tokens.js, uq-tokens.css, README.md, test/
templates/widget/, templates/provider/
docs/integrating.md
```

## 9. Decisions to confirm

1. **In-repo `uq/` folders, installed by each app's own package** (§2.6), instead of a
   `uq-<name>` adapter per repo. Simpler to keep in step with each app; the cost is that each app
   tracks uQ's integration API (`uq_api`).
2. **A `command` helper for CLI-first repos** (§2.3), instead of requiring a D-Bus service from
   crumBS, oysterAV, appOFholding and romforge. Commands are declared, fixed and never go through
   a shell.
3. **Activatable providers start when the launcher opens**, never per keystroke (§2.4.1). The
   first keystrokes after a cold start may miss results from that provider.
4. **App actions** (§2.4.2) for crumBS's "Uninstall…" and appOFholding's per-app actions, instead
   of separate provider results.
5. **appOFholding adds no duplicate launcher results.** Its AppImages already appear as apps
   through their `.desktop` entries; it contributes actions and the update tile.
6. **hauler as "default download manager"** means: a URL typed or pasted into Spotlight offers
   "Download with hauler", and it is pre-selected in bindle's uQ app list. There is no system-wide
   download-handler standard to register with.
7. **emobie's hotkey** becomes uQ's `Super+.` bind opening Spotlight with `:`; emobie's own hotkey
   is off under uQ.
8. **romforge's game-mode rule** is opt-in and rendered by uqd from a validated TOML file, not a
   packaged Lua file.
9. **scale_tail and romforge get AUR packaging** in their slices; today neither has a PKGBUILD.

## 10. Slices (one plan each)

0. **Integration foundation (uQ only):** `uq_core::integration`, the widget host and slots, bridge
   widget calls and commands, the settings `command` helper and live page discovery, activatable
   providers, app actions, `uqctl integrations`, templates, `docs/integrating.md`, `uq-tokens`,
   and the gate test with fixture integrations. Tag `v0.4.0`.
1. scale_tail (and its AUR package). 2. appOFholding. 3. hauler. 4. emobie (with `uq-tokens`).
5. XZram. 6. scdr. 7. crumBS. 8. oysterAV. 9. romforge (with `uq-tokens`, AUR package and the
   game-mode rule). 10. `uq-tokens` in SonicSound, noveleditor and gutcheck.

Slices 1–10 each end with a release of the app's repo and a `v0.4.x` uQ tag when uQ itself
changed (optdepends, docs). They can run in parallel once slice 0 has shipped, since each touches a
different repo; the design order sets priority, not a dependency.
