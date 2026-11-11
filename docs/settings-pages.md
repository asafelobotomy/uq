# Settings pages from other apps

An app can add its own page to uQ Settings, under **Apps** in the sidebar. The page is a folder
with a `page.toml` and a QML file (plus, if you like, a `logic.js` and other component files),
installed by the app's own package. This is the reference
for writing one; `templates/settings-page/` is a working starting point.

Design: Phase 3 spec §2.5 (plugin pages) and Phase 4 spec §2.3 (`[[command]]`, `[requires]`,
live discovery), in `docs/superpowers/specs/`.

## Where pages live

```
<your repo>/uq/pages/<id>/page.toml, Page.qml   →   /usr/share/uq/settings/pages/<id>/
```

- Only `/usr/share/uq/settings/pages/` is read. Pages are never read from a directory a user
  can write to.
- Settings watches that directory. A page appears a second or two after your package installs
  it and goes away when it is removed; nobody has to restart Settings or log out.
- A page that fails to load is skipped with one line in the journal and listed with the reason
  by `uqctl integrations list`. A page whose QML fails shows a "This page couldn't load" card.

### Files in the page folder

The folder holds `page.toml`, the QML file it names, and any other files the page uses. The QML
file is loaded from its place in the folder, so relative imports work as in any QML:

```
uq/pages/<id>/page.toml
uq/pages/<id>/Page.qml        import "logic.js" as Logic
uq/pages/<id>/logic.js        .pragma library: parsing, argument building, messages
uq/pages/<id>/StatusRow.qml   a component, used from Page.qml as StatusRow { }
```

- Keep pure logic (parsing command output, building arguments, checking input) in a `logic.js`
  that starts `.pragma library` and makes no `Qt.*` calls; then it can be tested with Node
  without Qt.
- `uqctl integrations check` reads every `.qml` and `.js` file in the folder, so the text rule
  covers them all.
- Install the whole folder (`install -Dm644 -t …/pages/<id>/ uq/pages/<id>/*`).

## `page.toml`

```toml
uq_api = 1
id = "tailscale"                # [a-z0-9-]{1,40}, equal to the folder name
name = "Tailscale"              # 1-100 characters
icon = "network-vpn"            # optional; a theme icon name, not a path
category = "apps"               # optional; "apps" is the only value
qml = "Page.qml"                # a file name in this folder (no directories, no "..")
keywords = ["vpn", "network"]   # optional; at most 32, found by Settings search
dbus_name = "org.example.App1"  # optional; the one bus name ctx.dbus may reach

[[setting]]                     # optional, up to 200: rows Settings search finds
id = "exit-node"                # [A-Za-z0-9._-]{1,64}; your page's `highlight` gets it
label = "Exit node"
keywords = ["route"]

[requires]                      # optional: the page shows only while all of these hold
bus_name = "org.example.App1"   # this name has an owner (checked live)
bins = ["tailscale"]            # these exist in /usr/bin

[dbus]                          # optional: pins ctx.dbus to one object (see below)
name = "org.example.App1"
path = "/org/example/App1"
interface = "org.example.App1"
methods = ["Refresh"]
properties = ["State"]
bus = "session"                 # or "system" for a system service; default "session"
timeout_ms = { Refresh = 120000 }   # optional: longer deadlines for listed methods, at most 120000

[[command]]                     # optional, up to 64: what ctx.command.run may run
id = "status"                   # [a-z0-9_-]{1,40}
argv = ["/usr/bin/tailscale", "status", "--json"]
output = "json"                 # "json" | "text" | "none"
timeout_ms = 3000               # default 10000, at most 600000
args = [{ name = "peer", pattern = "^[A-Za-z0-9._-]{1,64}$" }]   # optional
confirm = "Disconnect from %s?" # optional
```

Every key is checked strictly. An unknown key, a bad value or a QML file that resolves outside
the folder (through a symlink, say) refuses the whole page, with the reason. `uq_api` may be
left out (it means 1); a page with a newer `uq_api` than this uQ supports is skipped with
"needs a newer uQ".

`[requires]`: `bins` is checked whenever the directory changes; `bus_name` is followed live, so a
page tied to a daemon appears when the daemon starts and goes when it stops. The name is
watched on the `[dbus]` table's bus (the session bus without one).

## What the page gets

The page's root item may declare any of these; Settings sets the ones it declares.

```qml
import QtQuick
import org.uq.settings          // UqLabel

Item {
    property var ctx            // the page context, below
    property var theme          // the design tokens (same as ctx.theme; Phase 3)
    property string highlight   // the [[setting]] id Settings search jumped to
}
```

**Text.** Show every piece of text with `UqLabel` from `import org.uq.settings`. It is a
`Label` that never interprets markup, so text from your app's output or from D-Bus can't turn
into links or formatting. `Text`, `Label`, `TextEdit`, `TextInput` and `Qt.createQmlObject` are
refused by `uqctl integrations check`. Button texts should be fixed strings.

### Components pages may use

These components of `import org.uq.settings` are part of `uq_api = 1`: their names and the
properties and signals listed here stay as they are while `uq_api` is 1. Anything else the module
exports is Settings' own and may change. All of them show text as plain text.

| Component | What it is | Properties and signals |
|---|---|---|
| `UqLabel` | a `Label` that never interprets markup; use it for every piece of text | everything `QtQuick.Controls` `Label` has (`textFormat` is fixed to plain text) |
| `UqComboBox` | a `ComboBox` whose value and rows are plain text | everything `ComboBox` has; give it a list of strings as `model` |
| `SettingRow` | one setting: a label on the left, the controls you put inside on the right; flashes when Settings search jumps to it | `settingId`, `label`, `highlight` (bind it to the page's `highlight`), `flashing` (read-only); children are the controls |
| `SwitchRow` | a `SettingRow` with a switch; after a flip the switch shows `value` again until you change `value`, so a refused change snaps back | `value`, `chosen(bool value)`, `toggle` (the switch, read-only) |
| `ValueSlider` | a `SettingRow` with a slider; `chosen` comes 400 ms after the last move | `value`, `from`, `to`, `stepSize`, `lowLabel`, `highLabel`, `busy` (when it turns false the slider shows `value` again), `chosen(real value)`, `slider` (read-only) |
| `ErrorCard` | a card for something that failed | `title`, `detail` |

Beside these, a page may use stock `QtQuick`, `QtQuick.Layouts` and `QtQuick.Controls` items
(`Button`, `Switch`, `CheckBox`, `SpinBox`, `Slider`, `TextField` for the person's own input,
…), with fixed strings as their `text`. Don't import Kirigami: Settings' style may change.

### Testing a page

`templates/settings-page-tests/` is a test kit to copy into your repository as `tests/uq/`: a stub
`org.uq.settings` module with the components above (no Kirigami needed), a `FakeContext.qml`
that stands in for `ctx` with canned answers and a log of every request, and an example
`tst_page.qml` that loads your page and hands it the fake `ctx`. Run it offscreen:

```
QT_QPA_PLATFORM=offscreen qmltestrunner -import tests/uq/stub -input tests/uq
```

Its README has the details. uQ's own tests run the kit against the template page and check
that every property and signal of the stub exists on the real component.

### `ctx.theme`

The current `tokens.json`: `colors` (`bg`, `surface`, `text`, `text_muted`, `accent`, `error`,
…), `ui`, `fonts`. It is `{}` when no theme has been written yet, so always have a fallback.

### `ctx.command.run(id, args, callback)`

Runs the `[[command]]` with that `id`, with `args` (a list of strings) appended to its `argv`.

- `callback(result)` gets `{code: 0, output}` on success. `output` is the parsed JSON
  (`output = "json"`), the text (`"text"`) or `null` (`"none"`).
- A non-zero exit gives `{code, error, output}`: `error` names the code and the first line the
  program wrote to stderr; `output` is the parsed JSON if it parses, else the text.
- Anything else gives `{error}`: an unknown command, wrong arguments, already running, timed out,
  more than 4 MiB of output or 64 KiB of errors, killed, or JSON that doesn't parse.
- With `confirm`, Settings first shows a dialog with that exact text, `%s` replaced by the first
  argument as plain text, and **Cancel** and **Confirm**. The command runs only on Confirm; Cancel
  or Escape gives `{error: "Cancelled", cancelled: true}`.

What is enforced, whatever the page does:

- Only declared commands run. `argv[0]` is an absolute path under `/usr/bin` or `/usr/lib`, and
  the fixed `argv` is never changed. The check is on the path as written (no `.`, `..` or `//`),
  not on where it leads: symlinks are not resolved, so a link under `/usr/lib` that points
  elsewhere is accepted. Those directories are root's, so only a package (or root) can put one
  there.
- Arguments are appended as separate entries, exactly as many as `args` declares. Each must
  match its pattern in full (the pattern must start with `^` and end with `$`) and may never
  start with `-`, so it can't become an option.
- No shell. The environment is only `HOME`, `USER`, `LANG`, `LC_*`, `XDG_*`,
  `DBUS_SESSION_BUS_ADDRESS`, `WAYLAND_DISPLAY`, `PATH=/usr/bin` and `UQ_INTEGRATION=1`. stdin is
  closed and the command runs in its own process group.
- Over its timeout or an output cap, the whole process group gets `SIGTERM`, then `SIGKILL` two
  seconds later.
- The run ends when the command exits. Output still arriving 200 ms after that is dropped, so a
  process it started in the background (and left holding stdout or stderr) doesn't hold up the
  result; that process is left running.
- One run per command at a time: a second `run` of the same `id` while one runs gets
  `{error: "already running"}`.
- Nothing runs as root. A command that needs it uses your app's own polkit action; uQ's polkit
  agent shows the prompt.

### `ctx.dbus.call(method, args, callback[, target])` and `ctx.dbus.get(property, callback[, target])`

Calls a method or reads a property on the page's own bus name. `callback` gets `{value}` or
`{error}`. A method with no results gives `null`, one result gives that value, several give a
list.

- **With a `[dbus]` table:** only its `path` and `interface`, and only the listed `methods` and
  `properties`. Leave `target` out. `bus = "system"` reaches a system service.
- **With only `dbus_name`** (the Phase 3 form): pass `target = {path, interface}`; any method or
  property on that name, on the session bus, is allowed.
- Argument types come from the service's introspection data, never from the page. Only
  `b y n q i u x t d s o`, arrays of those, `a{sv}` and `a{ss}` are supported, both ways;
  anything else is an error. In `a{sv}`, `true` is `b`, whole numbers are `x`, other numbers
  `d`, strings `s`, lists of one kind `ab`/`ax`/`ad`/`as`, objects `a{sv}`.
- Arguments are at most 16 KiB of JSON and results at most 256 KiB. Every request ends within
  2 seconds, except a method given its own deadline in `[dbus] timeout_ms`. At most 8 requests
  per page are in flight; more get a "busy" error.
- **Writes that may ask for a password.** A service that checks polkit with user interaction
  allowed may wait for the person to type their password, which takes longer than 2 seconds; the
  page would get `{error}` while the change still goes ahead. List such a method in
  `timeout_ms = { Method = <ms> }` (1 to 120000) under `[dbus]`: its calls may take that long.
  Only methods in `methods` may be listed, and only pages have it (a widget's calls always end
  within 2 seconds). Property reads always keep 2 seconds.
- A page with `requires.bus_name` never starts its service: if the name has no owner, the
  request fails.

### `ctx.openApp(desktopId[, callback])`

Opens an app, such as your own full app, the way the app launcher does: through `uqctl launch`,
so it gets its fix from **Apps and scaling**, runs in its own scope apart from Settings, and
counts as a launch. Use it for a "More in <app>" button instead of a `[[command]]`: a command's
output is read until the program ends, so a GUI app started by a command holds it up until its
timeout and is then stopped.

- `desktopId` is a plain desktop file name, `org.example.App.desktop`: letters, digits, `.`, `_`
  and `-`, ending `.desktop`, not starting with `.` or `-`. No paths.
- `callback(result)` (optional) gets `{ok: true}` once the app was handed over, or `{error}` (a
  bad id, or no such app).

## Threat model

Pages are trusted code from installed packages, read only from root-owned `/usr/share/uq/`.
The limits above stop mistakes (a typo that would run the wrong program, an argument that
turns into `--force`, a hung command, markup in text), not a hostile page: nothing stops page
QML from importing other modules. A page must still never pass untrusted text to anything that
interprets it, and must show every piece of text through `UqLabel`.

## Testing

- `uqctl integrations check uq/` checks the whole folder before packaging: the layout, every
  manifest, the text rule, that each `[[command]]` program exists, and more. Exit 0 means clean.
  Run it in your CI, pinned to a uQ release. `--no-programs` skips only the "program exists"
  check, for a CI that doesn't install your app.
- `uq-settings --plugin-dir <dir>` reads pages from `<dir>` instead of
  `/usr/share/uq/settings/pages/`, so you can try a page without installing it. Pages added to
  or removed from it come and go while it runs.
- `uqctl integrations list` shows what an installed uQ found, and why a page is hidden or
  refused.
