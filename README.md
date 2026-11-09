# uQ

A themeable Hyprland desktop: stock Hyprland (pinned), system-wide theming, per-workspace layouts
and, in later phases, a Quickshell shell and integrated apps. See the
[design spec](docs/superpowers/specs/2026-11-08-uq-desktop-design.md) and the
[Phase 1 plan](docs/superpowers/plans/2026-11-08-uq-phase1-foundation.md).

## What Phase 1 gives you

- **`uqd`**: a session daemon that owns the theme, the dark/light schedule and per-workspace
  layouts, writes `~/.config/hypr/uq/generated.lua` and the rendered theme files, and serves
  `org.uq.Desktop1` on the session bus.
- **`uqctl`**: the command-line client.
- **Seven themes, each dark and light**: Gruvbox, Dracula, Matcha, Forest, Graphite, Rosewood,
  Velvet. Every pair of text and background colours meets WCAG AA contrast.
- **Base Hyprland config** with layout keys: `SUPER+ALT+D` dwindle, `SUPER+ALT+M` master,
  `SUPER+ALT+S` scrolling (per workspace, kept across reloads and reboots).

## Install (Arch, Manjaro)

```bash
cd packaging/uq-hyprland-pin && makepkg -si
cd ../uq && makepkg -si
uqctl setup          # add --apps to also theme GTK, qt6ct and terminals
```

`uqctl setup` writes `~/.config/hypr/hyprland.lua`. With `--apps` it also writes the GTK, qt6ct,
kitty, foot and alacritty files that load uQ's theme; those apply in every desktop you use, not
only Hyprland, so they are opt-in. It never overwrites a file you already have; it prints the line
to add instead. Then log in to Hyprland. uqd starts with the Hyprland session and stops when it
ends.

Hyprland is pinned to **0.56.2–0.57.x** (`uq-hyprland-pin`): Manjaro ships 0.56.2 and Arch
0.57.1 as of 2026-11-08. The Lua API uQ uses is identical in the 0.56.2 and 0.57 sources; CI runs
`Hyprland --verify-config` on every theme against Arch's current release.

## Use

```bash
uqctl theme list
uqctl theme set rosewood light
uqctl layout set 2 scrolling
uqctl schedule fixed 07:00 19:00   # or: uqctl schedule sun 51.5 -0.13, or: uqctl schedule off
```

Shell settings (all go through `uqd`; values are validated there):

```bash
uqctl pin add org.mozilla.firefox      # also: pin rm <id>, pin list
uqctl weather set "Paris" [--pick 2]   # geocodes, prints the top 5, stores one; weather off
uqctl idle set lock 300                # dim | lock | screen-off | suspend | locked-screen-off; secs or off
uqctl notif backend builtin            # auto | notifi | builtin | swaync | mako | dunst
uqctl privacy public-ip on             # or: privacy launches off
uqctl mouse speed -0.3                 # -1..1; also: accel default|adaptive|flat,
                                       # natural-scroll on|off, left-handed on|off,
                                       # scroll-factor 0.1..10, show
uqctl launches clear
```

The shell runs as `uq-shell.service` (Quickshell, started with the session). `SUPER+CTRL+L`
restarts the lock surface (`uq-lock.service`, else `hyprlock`) if it ever dies.

### Launchers

- `SUPER+Space` opens or closes Spotlight: apps, a calculator (`=2+2`), a command runner
  (`>cmd`) and any installed search providers.
- Tapping `SUPER` (left SUPER, pressed and released alone) opens or closes the app menu. It
  doesn't open when you press and release another key or mouse button while SUPER is held. On
  Hyprland 0.56 it can still open if you release SUPER while that other key is still down.
- `SUPER+A` opens or closes quick settings (volume, mic, brightness, Wi-Fi, Bluetooth and more).
- None of them works over the lock screen. All open on the focused monitor.

Scripts can drive them through the shell's IPC (ignored while the screen is locked):

```bash
qs ipc -p /usr/share/uq/shell call uq open spotlight    # or: close, toggle; spotlight | appmenu | quicksettings
```

`open` only opens and `close` only closes that surface; any other name is ignored. Each call
prints `ok` or `ignored`.

#### Volume, mic and brightness keys

`XF86AudioRaiseVolume`, `XF86AudioLowerVolume`, `XF86AudioMute`, `XF86AudioMicMute`,
`XF86MonBrightnessUp` and `XF86MonBrightnessDown` go through the shell (`uq:vol-up`, `uq:vol-down`,
`uq:mute`, `uq:mic-mute`, `uq:bright-up`, `uq:bright-down`). They work over the lock screen; the
volume and brightness keys repeat while held, and the mute keys don't. Each press shows an OSD: noti-fi's when it answers, otherwise uQ's own on the
focused monitor. Audio changes in the first 2 s after login are quiet (no OSD). With no output,
input or backlight device, the matching key does nothing.

#### Search providers

Any app can add rows to Spotlight by owning a D-Bus name on the session bus and dropping a
`.provider` file in `/usr/share/uq/launcher/` or `~/.local/share/uq/launcher/`
(`$XDG_DATA_HOME/uq/launcher/`). If two files name the same `BusName`, the system one wins.

```ini
[Launcher Provider]
BusName=org.example.Notes
# 1-64 characters, no control characters; shown next to every row from this provider.
Name=Notes
# Optional, 1-4 characters, no whitespace or control characters.
Prefix=n:
```

A file with a missing or invalid `BusName` or `Name`, or an invalid `Prefix`, is skipped.

The provider serves `org.uq.Launcher1.Provider` at `/org/uq/Launcher1`:

- `Search(s query) → a(sssssd)`: rows of (id, title, subtitle, icon, actions_json, score).
  `actions_json` is a JSON list of strings, and `score` is between 0 and 1.
- `Activate(s id, s action)`: called when the user picks a row, with that row's first action
  (empty if it has none). It has 1 s to answer.

Which queries a provider gets:

- Queries starting with `>` or `=` never go to providers.
- A query that starts with any registered provider's `Prefix` goes only to the providers with
  that prefix, with the prefix stripped (and nothing is sent if only the prefix was typed).
- A provider without a `Prefix` gets every other query of 2 or more characters.
- It's only called while its name has an owner (never auto-started), and it has 300 ms to
  answer.

What the bridge does with the rows:

- Rows with an empty id, or an id over 512 bytes, are dropped. At most 20 of the rest are kept.
- Titles are truncated to 200 bytes and subtitles to 300 bytes.
- Each action is truncated to 256 bytes. Whole actions are kept up to 2 KiB in all, and
  `actions_json` that isn't a JSON list of strings means no actions.
- An icon must be an icon-theme name (`[A-Za-z0-9._-]`, at most 128 bytes) or an absolute local
  path (at most 512 bytes, characters `[A-Za-z0-9._-/+ @,~]`, no `//`, empty or `..` segments,
  nothing under `/dev`, `/proc` or `/sys`). Anything else becomes `system-search`. Icons are
  never treated as URLs.
- Scores are clamped to 0-1 and scaled by 0.9, so a provider never outranks an exact or prefix
  app-name match.

Rows are shown as plain text, labelled with the provider's `Name`. uQ never runs anything a
provider sends; picking a row only calls back its `Activate`.

### Notifications

The Notifications island in the bar shows the latest app's icon (or a bell), how many
notifications there are, and a mark while Do Not Disturb is on.

- Left click, or `SUPER+N`, opens or closes the notification centre. `SUPER+N` doesn't work over
  the lock screen.
- Right click turns Do Not Disturb on or off. Quick settings has the same toggle.
- The latest summary shows only while you hover the island, or always with the
  `notif.show_summary` setting. It never shows while the screen is shared, and screen readers
  only hear the count and the Do Not Disturb state.

`uqctl notif backend <name>` picks who shows them:

- `auto` (the default) uses noti-fi when its socket answers, otherwise uQ's built-in server.
- `notifi`, `builtin`, `swaync`, `mako` or `dunst` uses that one. If noti-fi isn't running, or
  the chosen daemon isn't installed, uQ uses the built-in server and logs a warning. For swaync
  and mako uQ only checks that they're installed, not that they're running.
- mako has no centre, so `SUPER+N` and the click do nothing with it. dunst's centre brings back
  the last notification from its history.

The built-in server only takes the notification name when nothing else owns it. If another
daemon got there first, the island says "Notifications handled by <name>" and uQ leaves it
alone. uQ never starts, stops or masks another daemon. To stop one auto-starting, mask it or
remove its D-Bus activation file:

```bash
systemctl --user mask swaync.service
# or remove the file in /usr/share/dbus-1/services/ whose Name= is org.freedesktop.Notifications
```

The built-in server's Do Not Disturb isn't kept across shell restarts.

With noti-fi, its popups and list stay on its own monitor choice. To have them follow the focused
monitor, as uQ's OSD does, add `"screen": "focused"` to noti-fi's `rules.json`. uQ doesn't write
that file.

Your own Hyprland settings go in `~/.config/hypr/uq/user.lua`, which uQ loads last and never
rewrites.

## Develop

```bash
cargo test --workspace                     # unit, golden, D-Bus and Lua-mock tests
cargo test -p uqd --test verify_config -- --ignored   # needs Hyprland installed
scripts/lint-shell.sh                      # no bare Text, Label or Qt.createQmlObject in shell/
for t in tests/shell/*.sh; do bash "$t"; done
QT_QPA_PLATFORM=offscreen qmltestrunner -import shell -input tests/qml
scripts/smoke-shell.sh                     # needs Hyprland, quickshell, jq; exit 77 = skipped
```
