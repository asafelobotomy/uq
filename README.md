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
uqctl launches clear
```

The shell runs as `uq-shell.service` (Quickshell, started with the session). `SUPER+CTRL+L`
restarts the lock surface (`uq-lock.service`, else `hyprlock`) if it ever dies.

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
