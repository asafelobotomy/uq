# Settings pages from other apps

An app can add its own page to uQ Settings, under **Apps** in the sidebar. The page is a folder
with a `page.toml` and one QML file, installed by the app's own package. This is the reference
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
  the fixed `argv` is never changed.
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
  2 seconds. At most 8 requests per page are in flight; more get a "busy" error.
- A page with `requires.bus_name` never starts its service: if the name has no owner, the
  request fails.

## Threat model

Pages are trusted code from installed packages, read only from root-owned `/usr/share/uq/`.
The limits above stop mistakes (a typo that would run the wrong program, an argument that
turns into `--force`, a hung command, markup in text), not a hostile page: nothing stops page
QML from importing other modules. A page must still never pass untrusted text to anything that
interprets it, and must show every piece of text through `UqLabel`.

## Testing

- `uqctl integrations check uq/` checks the whole folder before packaging: the layout, every
  manifest, the text rule, that each `[[command]]` program exists, and more. Exit 0 means clean.
  Run it in your CI, pinned to a uQ release.
- `uq-settings --plugin-dir <dir>` reads pages from `<dir>` instead of
  `/usr/share/uq/settings/pages/`, so you can try a page without installing it. Pages added to
  or removed from it come and go while it runs.
- `uqctl integrations list` shows what an installed uQ found, and why a page is hidden or
  refused.
