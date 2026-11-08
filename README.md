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

Hyprland is pinned to **0.56.x** (`uq-hyprland-pin`), the newest release in Arch `extra` on
2026-11-08. The Lua API uQ uses was checked against the 0.56.2 and 0.57.0 sources.

## Use

```bash
uqctl theme list
uqctl theme set rosewood light
uqctl layout set 2 scrolling
uqctl schedule fixed 07:00 19:00   # or: uqctl schedule sun 51.5 -0.13, or: uqctl schedule off
```

Your own Hyprland settings go in `~/.config/hypr/uq/user.lua`, which uQ loads last and never
rewrites.

## Develop

```bash
cargo test --workspace                     # unit, golden, D-Bus and Lua-mock tests
cargo test -p uqd --test verify_config -- --ignored   # needs Hyprland installed
```
