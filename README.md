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
uqctl notif backend shell
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
- Neither works over the lock screen. Both open on the focused monitor.

Scripts can drive them through the shell's IPC (ignored while the screen is locked):

```bash
qs ipc -p /usr/share/uq/shell call uq open spotlight    # or: close, toggle; spotlight | appmenu
```

`open` only opens and `close` only closes that surface; any other name is ignored. Each call
prints `ok` or `ignored`.

#### Search providers

Any app can add rows to Spotlight by owning a D-Bus name on the session bus and dropping a
`.provider` file in `/usr/share/uq/launcher/` or `~/.local/share/uq/launcher/`
(`$XDG_DATA_HOME/uq/launcher/`). If two files name the same `BusName`, the system one wins.

```ini
[Launcher Provider]
BusName=org.example.Notes
Name=Notes
# Optional, 1-4 characters, no spaces: only queries starting with it reach this provider,
# with the prefix stripped.
Prefix=n:
```

The provider serves `org.uq.Launcher1.Provider` at `/org/uq/Launcher1`:

- `Search(s query) → a(sssssd)`: rows of (id, title, subtitle, icon, actions_json, score).
  `actions_json` is a JSON list of strings, and `score` is between 0 and 1.
- `Activate(s id, s action)`: called when the user picks a row, with that row's first action
  (empty if it has none). It has 1 s to answer.

The provider has 300 ms to answer. It's only called while its name has an owner (never
auto-started). Unless it has a `Prefix`, it gets queries of 2 or more characters. Queries
starting with `>` or `=` never go to providers.

Results are clamped:

- at most 20 rows;
- title 200 bytes, subtitle 300 bytes, id 512 bytes, actions 2 KiB;
- an icon that isn't an icon-theme name or an absolute path becomes `system-search`;
- scores are scaled by 0.9, so a provider never outranks an exact or prefix app-name match.

They are shown as plain text, labelled with the provider's `Name`. uQ never runs anything a
provider sends; picking a row only calls back its `Activate`.

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
