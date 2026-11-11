# Integrating an app with uQ

An app can plug into uQ in five ways. None of them makes uQ a dependency of the app, or the app a
dependency of uQ: the files are inert where uQ isn't installed, and uQ runs fine without them.

| Extension point | What the app ships | What the user sees |
|---|---|---|
| [Theme](#theme) | nothing; it reads uQ's `tokens.json` | the app follows uQ's colours, fonts and light/dark switch |
| [Settings page](#settings-pages) | `page.toml` and a QML file | a page under **Apps** in uQ Settings |
| [Widget](#widgets) | `widget.toml` and a QML file | an icon in the bar or a tile in quick settings |
| [Launcher provider](#launcher-providers) and [app actions](#app-actions) | a `.provider` file and a D-Bus service; `.appaction` files | results in Spotlight; entries in an app's right-click menu |
| [Window rules](#window-rules) | a `rules.toml` file | rules under **Rules from apps** on the Window rules page, each off until turned on |

Design: `docs/superpowers/specs/2026-11-10-uq-phase4-integrations-design.md` (Phase 4).

## Where the files live

Keep everything in a `uq/` folder in your own repository, installed by your own package:

```
<repo>/uq/
  widgets/<id>/widget.toml, Widget.qml      → /usr/share/uq/shell/widgets/<id>/
  pages/<id>/page.toml, Page.qml            → /usr/share/uq/settings/pages/<id>/
  launcher/<name>.provider                  → /usr/share/uq/launcher/
  launcher/actions/<id>.appaction           → /usr/share/uq/launcher/actions/
  dbus/<bus name>.service                   → /usr/share/dbus-1/services/
  integrations/<name>/rules.toml            → /usr/share/uq/integrations/<name>/
  README.md                                 what the folder adds and how you tested it
```

- uQ reads integrations only from `/usr/share/uq/` (root-owned, installed by packages), never
  from a folder the user can write to. The one exception, launcher `.provider` files in
  `~/.local/share/uq/launcher/`, stays as it always was.
- Install the folder unconditionally from your app's main package and list `uq` as an
  `optdepends` with a one-line reason.
- uQ watches these directories. What a package installs shows up within a second or two, and
  goes when the package is removed; nobody logs out or restarts anything.
- Every manifest says `uq_api = 1`. A manifest that needs a newer uQ is skipped, and says so.

Tools:

- `uqctl integrations check uq/` checks the whole folder before packaging: the layout, every
  manifest and file, the plain-text rule for QML, that each command's program exists, that
  each provider's bus name is valid, and every `rules.toml`. Exit 0 means clean. Run it in your CI, pinned to a uQ
  release.
- `uqctl integrations list [--json]` shows what an installed uQ found, whether each item shows,
  and why not.
- uQ's own gate test, `tests/shell/integrations_gate.sh`, installs fixture `uq/` folders into an
  empty tree while the bridge runs and checks that each appears and goes again.

## Threat model

Integration code is trusted code from installed packages. uQ's checks stop **mistakes**: a typo
that would run the wrong program, an argument that turns into `--force`, a hung service, a
command that prints forever, markup in a name. They are not a sandbox against a hostile package,
which could install anything anywhere anyway. Nothing stops widget or page QML from importing
other modules; the templates and `uqctl integrations check` steer away from it.

What uQ enforces:

- Files are read only from `/usr/share/uq/`. A folder or manifest that is a symlink pointing
  outside its directory is refused. Manifests are parsed strictly: unknown keys are errors.
- Every string an integration produces (names, results, command output, property values) is
  shown as plain text through `UqText` / `UqLabel`, never as rich text.
- **Commands** are only the declared ones. `argv[0]` is an absolute path under `/usr/bin` or
  `/usr/lib`; the fixed argv is never changed; the caller's arguments are appended one per entry,
  exactly as many as declared, each matching its anchored pattern and never starting with `-`.
  No shell, a scrubbed environment, stdin closed, its own process group, a time limit (10 s by
  default, at most 10 minutes), 4 MiB of output and 64 KiB of errors; then `SIGTERM`, and
  `SIGKILL` 2 seconds later.
- **D-Bus** goes to one name, path and interface per widget or page, listed members only.
  Argument types come from the service's introspection data, never from the QML. Only `b y n q i
  u x t d s o`, arrays of those, `a{sv}` and `a{ss}` cross, both ways. Arguments are at most
  16 KiB of JSON, results at most 256 KiB, and every request ends within 2 seconds.
- **Window rules** are data, never Lua: uqd checks each one against a closed schema and writes
  it into `generated.lua` itself, with every value quoted, then has Hyprland check the result.
  None applies until the user turns it on.
- **App actions** have one placeholder, `%d`, filled with a validated desktop id as one whole
  argument.
- Nothing runs as root on an integration's behalf. Admin work goes through your app's own polkit
  actions; uQ's polkit agent shows the prompt.

## Theme

uQ writes the current theme's design tokens to `~/.local/state/uq/theme/tokens.json` (under
`$XDG_STATE_HOME` if set). The `theme/` directory is replaced atomically on every theme or
light/dark change, so **watch the parent directory** (`~/.local/state/uq/`), not the file: a
watch on the file is lost after the first change.

- **Web apps** (Tauri, Electron): vendor `web/uq-tokens/` from this repository. It maps the
  tokens to CSS custom properties, validates every value, and falls back to your own theme when
  uQ isn't there. Its README has the Tauri and Electron recipes.
- **Qt and GTK apps** need nothing: `uqctl setup --apps` points GTK, qt6ct and Kvantum at uQ's
  theme.
- **QML in a settings page or widget** gets the tokens as `ctx.theme` / `ctx.Theme`.

Checklist:

- [ ] Your app looks right with uQ's light and dark variants of a few themes.
- [ ] A theme change while the app runs is picked up (watching the parent directory).
- [ ] With no `tokens.json` (uQ not installed), the app uses its own theme and shows no error.

## Settings pages

A page under **Apps** in uQ Settings: `/usr/share/uq/settings/pages/<id>/page.toml` and one QML
file. The full reference, including `ctx` for pages, `[[command]]` with `confirm`, `[requires]`
and `[dbus]`, is [`docs/settings-pages.md`](settings-pages.md); the starting point is
`templates/settings-page/`.

Checklist:

- [ ] `id` equals the folder name; `uq_api = 1`.
- [ ] All text goes through `UqLabel`; no `Text` or `Label`.
- [ ] Every `[[command]]` has the smallest `args` patterns that work, and `confirm` on anything
      destructive.
- [ ] `[requires]` hides the page when your program or service isn't there.
- [ ] The page shows a sensible state while a command runs and when it fails.
- [ ] `uqctl integrations check uq/` is clean.

## Widgets

A widget is a small piece of QML in the bar or in quick settings:
`/usr/share/uq/shell/widgets/<id>/widget.toml` and one QML file. Start from
`templates/widget/`.

### `widget.toml`

```toml
uq_api = 1
id = "hauler"                 # [a-z0-9-]{1,40}, must equal the folder name
name = "Downloads"            # shown in the "…" list and in Settings
slot = "bar"                  # "bar" | "quick"
qml = "Widget.qml"            # a file in this folder
order = 50                    # 0-100, ties break on id

[requires]                    # all must hold for the widget to show (both optional)
bus_name = "org.hauler.Hauler1"   # shown only while this name has an owner
bins = ["hauler"]                 # shown only if /usr/bin/hauler exists

[dbus]                        # optional: what ctx.dbus may reach
name = "org.hauler.Hauler1"
path = "/org/hauler/Hauler1"
interface = "org.hauler.Hauler1"
methods = ["PauseAll", "ResumeAll"]
properties = ["ActiveCount", "BytesDone", "BytesTotal"]
signals = ["Finished"]
# bus = "system"              # default "session"

[[command]]                   # optional: what ctx.command.run may run
id = "status"
argv = ["/usr/bin/hauler", "status", "--json"]
output = "json"               # "json" | "text" | "none"
timeout_ms = 3000
# args = [{ name = "url", pattern = "^https?://[^ ]{1,2000}$" }]
```

`[requires]` and `[[command]]` mean exactly what they mean in `page.toml`, except that a
widget's command may not use `confirm` (the bar has no confirmation dialog; put such commands on
a settings page). A `requires.bus_name` is watched on the `[dbus]` table's bus: the widget hides
when the name loses its owner and comes back when it returns. `bins` is checked again whenever
anything in the widgets directory changes.

### Slots

- **`bar`**: a 26 × 26 px cell in the Status island, left of the status icons. Up to four show;
  the rest are listed under a "…" button.
- **`quick`**: a full-width 32 px row in the **Apps** group at the bottom of quick settings, the
  size of the toggle rows. The group is hidden while no tile has loaded.

The slot size is fixed: draw inside it. A widget may define `property Component popup`; calling
`ctx.openPopup()` shows it under the bar icon, or as a detail view under the quick tile, with the
same `ctx`. Popups close on a click outside, on Esc and when the screen locks. Widgets are never
drawn over the lock screen.

### `ctx`

The root item gets one property, `ctx` (declare `property var ctx`). It is set before
`Component.onCompleted`.

| Member | What it does |
|---|---|
| `ctx.Theme` (also `ctx.theme`) | the design tokens: `accent`, `text`, `textMuted`, `surface`, `overlay`, `radius`, `fontUi`, `fontSize`… |
| `ctx.UqText`, `ctx.UqInput` | Components for the shell's plain-text label and single-line input. Use them through a `Loader` (`sourceComponent: ctx.UqText`) or `createObject` |
| `ctx.slot` | `"bar"` or `"quick"` |
| `ctx.dbus.call(method, args, cb)` | a listed method; `cb({value})` or `cb({error})`. No outputs is `null`, one is the value, several are a list |
| `ctx.dbus.get(cb)` | every listed property as one object: `cb({value: {...}})` or `cb({error})`; also fills `ctx.dbus.props` |
| `ctx.dbus.props` | the listed properties, kept current from `PropertiesChanged` (at most 4 updates a second) |
| `ctx.dbus.onProps(cb)` | `cb(changed)` on each update |
| `ctx.dbus.onSignal(cb)` | `cb(member, args)` for each listed signal |
| `ctx.command.run(id, args, cb)` | a declared command: `cb({code: 0, output})`, `cb({code, error, output})` for a non-zero exit, `cb({error})` otherwise |
| `ctx.openPopup()`, `ctx.closePopup()` | show or hide the widget's `popup` |
| `ctx.fail(reason)` | give up: the widget is unloaded (see below) |

There is no handle on the bridge, `Quickshell.Io` or the shell's modules in `ctx`, and a widget
should not import them: everything it does goes through the calls above, which the bridge checks
against the manifest. At most 8 D-Bus requests per widget are in flight at once; more get
`{error: "busy"}`. A second run of a command that is still running gets `{error: "busy"}`.
Requests use `NO_AUTO_START`: a service is never started by a widget, unless the widget has no
`requires.bus_name` and the name has a D-Bus service file.

### When a widget breaks

A widget whose QML doesn't load, or whose callback (from `ctx.dbus`, `ctx.command`, `onProps`,
`onSignal`) throws, or which calls `ctx.fail()`, is unloaded: its slot takes no room and the rest
of the bar is unaffected. The reason is logged once per session (`journalctl --user -u
uq-shell`), and the widget is tried again at most once a minute. A D-Bus service that hangs
answers `{error}` after 2 seconds. An invalid `widget.toml` is skipped, logged once by the
bridge, and listed with the reason by `uqctl integrations list`.

Checklist:

- [ ] `id` equals the folder name; `uq_api = 1`; `slot` is the one you mean.
- [ ] All text goes through `ctx.UqText` (or `ctx.UqInput`); no `Text`, `Label` or imports of
      the shell's modules.
- [ ] The widget fits its fixed slot (26 × 26 for `bar`, a 32 px row for `quick`).
- [ ] `[requires]` hides it when your program or service isn't there.
- [ ] `[dbus]` lists only the members the widget uses; `[[command]]` only the commands, with tight
      `args` patterns.
- [ ] Every callback handles `{error}` and shows its own error state.
- [ ] `uqctl integrations check uq/` is clean.

## Launcher providers

A provider adds rows to Spotlight. It is a D-Bus service on the session bus that owns
`/org/uq/Launcher1` with the interface `org.uq.Launcher1.Provider`:

- `Search(s query) → a(sssssd)`: rows of `(id, title, subtitle, icon, actions, score)`.
  `actions` is a JSON list of strings (the first is what Enter does); `score` is 0–1. `icon` is
  an icon-theme name or an absolute local path.
- `Activate(s id, s action)`: the user chose a row.

Register it with a `.provider` file in `/usr/share/uq/launcher/`:

```ini
[Launcher Provider]
BusName=org.example.Provider
Name=Example            # shown next to each of its rows
Prefix=:                # optional, 1-4 characters: only queries starting with it, stripped
Activatable=true        # optional: start it when the launcher opens (needs a service file)
```

What uQ does with it:

- Only providers whose name has an owner are asked, with `NO_AUTO_START`, and each answer has
  300 ms. A slow provider simply misses that query; Spotlight never waits.
- At most 20 rows count. Titles are cut to 200 bytes, subtitles to 300, actions to 2 KiB; a bad
  icon becomes a default one. Scores are scaled so a provider can never outrank an app whose
  name matches.
- Queries starting with `>` or `=` (commands and maths) never reach providers. Without a
  `Prefix`, a provider gets queries of two characters or more that don't start with another
  provider's prefix.
- **`Activatable=true`**: with a session D-Bus service file for the bus name
  (`/usr/share/dbus-1/services/<name>.service`), uQ starts the provider when Spotlight or the app
  menu opens, if it isn't running: at most once every 30 seconds, without waiting, never per
  keystroke. Without a service file the key is ignored. A provider that fails to start is logged
  at most every 10 minutes and search carries on without it.
- The provider should exit after about 60 seconds without a call. `templates/provider/` does all
  of this in about a hundred lines of Rust.

Checklist:

- [ ] `Search` answers well within 300 ms, and with an empty list when it has nothing.
- [ ] `Activate` never runs anything taken from the query or the id through a shell.
- [ ] With `Activatable=true`, the service file's `Name=` equals `BusName=`, and the provider
      exits when idle.
- [ ] A `Prefix` that doesn't clash with `>`, `=` or another provider you know of.
- [ ] `uqctl integrations check uq/` is clean.

## App actions

App actions add entries to the right-click menu of app rows in Spotlight and the app menu, such
as "Uninstall…" or "Check for update". One file each:
`/usr/share/uq/launcher/actions/<id>.appaction`, in desktop-entry syntax:

```ini
[App Action]
Name=Uninstall…
Icon=edit-delete
Match=*                          # a glob (* and ?) on the desktop id, e.g. appofholding-*.desktop
Exec=/usr/bin/crumbs uninstall --desktop-id %d
Terminal=true
```

- `%d` is the only placeholder: the row's desktop id (`firefox.desktop`), checked against
  `^[A-Za-z0-9_][A-Za-z0-9._-]{0,199}\.desktop$` and passed as one whole argument. It never
  starts with `-`, but prefer a flag before it anyway.
- `Exec` is split with desktop-entry quoting rules and starts with an absolute path; there is no
  shell. An action whose program is missing is hidden.
- `Terminal=true` runs the command in the terminal chosen in Settings → Default apps, so
  interactive tools keep their own prompts. Otherwise it runs detached in its own scope, like an
  app launch.
- The shell only names the action and the row; the bridge reads the file, checks the desktop id
  (it must be an installed app the action matches) and builds the command line.

Checklist:

- [ ] `Match` is as narrow as it can be.
- [ ] The program confirms anything destructive itself (`Terminal=true` for an interactive
      prompt).
- [ ] The program treats the desktop id as data and looks the app up itself.
- [ ] `uqctl integrations check uq/` is clean.

## Window rules

An app can offer window rules, such as "open the emulators full screen on the game workspace".
They are off until the user turns each one on under **Rules from apps** on the Window rules page
of uQ Settings, and the user can turn them off again or undo the change from History. One file
per app: `/usr/share/uq/integrations/<name>/rules.toml`, where `<name>` is `[a-z0-9-]{1,40}`.

```toml
uq_api = 1                                  # optional

[[window_rule]]
id = "romforge.emulators"                   # "<name>.<rule>", the folder name first
name = "Emulators open full screen"         # optional: what Settings shows (else the id)
class = ["^(retroarch|duckstation-qt|PPSSPPSDL|dolphin-emu|pcsx2-qt|rpcs3|azahar)$"]
title = "^.*$"                              # optional: one window title pattern
workspace = "name:game"                     # optional: "name:<name>" or a number 1-100
fullscreen = true                           # optional
float = false                               # optional
```

- `id` is required and must start with the folder name and a dot; the rest is
  `[a-z0-9-]{1,40}`. Ids are unique in the file. Turned-on rules are remembered by id, so keep an
  id stable across releases and give a rule whose meaning changes a new one.
- `class` is required: 1 to 8 app ID patterns. Patterns use Hyprland's syntax (RE2): each is at
  most 256 characters, must compile, and is matched against the whole app ID. Look-arounds and
  back-references are refused because Hyprland can't use them. Each pattern becomes its own
  Hyprland rule with the same effects.
- `workspace` names are letters, digits, spaces, `_` and `-`, up to 32, after `name:`.
- At least one of `workspace`, `fullscreen` and `float` is set. A file holds at most 16 rules
  and 64 KiB. Unknown keys, wrong types and bad patterns refuse the whole file.
- uqd reads the file at every change it renders. When the user turns a rule on, uqd checks it
  again, renders it as one `hl.window_rule` call per pattern (named `uq:app:<id>`, after the
  workspace rules and before the user's own window rules, so their rules win), and has Hyprland
  verify the config. Any refusal leaves the rule off and shows the reason.
- A rule that is on but whose file was removed, or no longer passes the checks, is left out of
  the config at the next change. Its id stays on, so it applies again when the package is back;
  Settings lists it as not installed.
- Nothing else changes when a rule is turned on. If the rule wants a workspace to look a certain
  way (say, no gaps for games), say so in your README; the user sets that on the Look and Window
  rules pages.

Checklist:

- [ ] Every pattern is anchored (`^…$`) and as narrow as it can be.
- [ ] `name` says what the rule does in plain words.
- [ ] `uqctl integrations check uq/` is clean.
