# Changelog

All notable changes to uQ are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and uQ uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Release candidates are tagged
`vX.Y.Z-rc.N`; their package version is `X.Y.ZrcN`.

## [Unreleased]

### Added

- **`uq-dev`.** Use uQ and develop it on the same machine: `uq-dev session` opens a nested
  uQ desktop running a checkout's uqd and shell on a private bus with sandboxed config,
  `uq-dev run` runs an app from its checkout with throwaway config, and `uq-dev promote` /
  `rollback` install a package built from a checkout's HEAD and undo it (docs/developing.md).
- **Settings app.** `uq-settings` (Qt, Kirigami; `uqctl settings [page] [setting]` or the new
  "Settings…" button in quick settings) with Appearance and Wallpaper pages, search across
  settings, and a single instance on `org.uq.Settings1`. Third-party pages are read from
  `/usr/share/uq/settings/pages/<id>/page.toml`; a page that fails to load shows an error card.
  `uqd` gains `GetSection` / `SetSection` for the `appearance` and `wallpaper` domains.
- **Appearance.** UI and monospace fonts, font size, cursor theme and size, and icon theme, each
  overriding the theme when set. They reach Hyprland, GTK and the shell.
- **Wallpaper.** A wallpaper per monitor (or one for all): an image with fill, fit, center or
  tile, a folder slideshow with an interval, and a fallback colour. The shell draws it on the
  background layer.
- **Kvantum.** Qt apps follow the uQ theme through a generated Kvantum theme
  (`uqctl setup --apps` links it in; needs the `kvantum` package). An unchanged qt6ct.conf
  from 0.2 is switched to `style=kvantum`; one you edited is left alone with a hint. Turn it
  off with `appearance.kvantum = false`.
- **Look and layouts page.** Gaps, border width, rounding, blur (on, size, passes), shadow, and
  animations (on/off, speed) override the theme's values; workspace rules, the default layout and
  dwindle/master/scrolling options live on the same page. `uqd` gains `look` and `layouts`
  sections (stored as `[look]` and `[layout_options]`), verified with Hyprland before they apply;
  a rejected value keeps the old config and the page shows Hyprland's message. `SetLayout` also
  emits `SectionChanged("layouts")`, so the page follows the `SUPER` keybind.
- **Shell page and `shell` section.** Islands on or off, 24-hour clock, launcher providers on/off
  and order, and the terminal for `Terminal=true` apps. The bridge opens such apps in the chosen
  terminal (`X-TerminalArgExec`, else `-e`; kitty if it is missing or unparseable) and refuses to
  activate or launch from a switched-off provider.

### Changed

- `base.lua`'s start and shutdown hooks do nothing when `UQ_DEV_SESSION` is set, so a nested
  dev session can no longer restart, repoint or stop the installed uqd and shell.
- **Animations moved to `generated.lua`.** The animation lines are no longer in `base.lua`.
  After an upgrade Hyprland shows its own default animations until `uqd` re-renders
  `generated.lua` (restart `uqd` or change any setting). `hl.animation` calls in `user.lua`
  still win.

## [0.2.1] - 2026-11-10

### Fixed

- **`uqctl setup` on a fresh install.** An unedited copy of Hyprland's stock `hyprland.lua`
  (written by Hyprland on first start, by hyprland-welcome, or by bindle's installer) is no
  longer treated as the user's own config. It is moved to `hyprland.lua.stock` and replaced
  with uQ's stub, so a fresh install starts uQ on the next login instead of exiting 3.

### Changed

- **scdr alignment.** `uqd`, `uq-shell` and `uq-lock` run in `session.slice` instead of the
  default `app.slice`, so priority daemons such as scdr treat them as the desktop rather than
  apps (scdr would otherwise demote the bar and lock screen under CPU load). When scdr's session
  agent is enabled, Hyprland's start hook (re)starts it with the session environment, so scdr's
  focus roles work on uQ; it stops with the session. scdr is a new optional dependency.

## [0.2.0] - 2026-11-09

Phase 2: the uQ shell, a Quickshell desktop shell that runs as `uq-shell.service`.

### Added

- **Unattended setup.** `uqctl setup` reports its outcome in the exit code (0 uQ config in place,
  1 error, 2 refused as root, 3 your own `hyprland.lua` kept) and with `--json`, so installers
  such as bindle can run it for the user.
- **Bar and islands.** A floating, themed bar on every monitor with five islands:
  - System: workspaces with their layout icon, and a dock of pinned and running apps
    (`uqctl pin add|rm|list`).
  - Work: the focused window's title, with minimise, maximise, close and a window menu.
  - Time and weather: clock, month calendar and the current weather for a city you pick
    (`uqctl weather set`).
  - Status: tray, media, network and VPN (with the exit country, if `uqctl privacy public-ip on`),
    volume and battery.
  - Notifications (see below).
- Window titles, network names and every other outside string are shown as plain text only.
  The bar follows theme changes live and survives uqd restarts and monitor hot-plug.
- **Launchers.**
  - `SUPER+Space` opens Spotlight: apps ranked by match and launch history, a calculator
    (`=2+2`), a command runner (`>cmd`) and search providers that other apps can add over D-Bus.
  - Tapping `SUPER` opens the app menu: categories, favourites and a power row.
  - Apps launched from the shell run in their own systemd scopes, so they survive a shell
    restart. `uqctl launches clear` forgets the launch history.
- **Quick settings and OSD.**
  - `SUPER+A` (or the Status island) opens quick settings: volume and output device, mic,
    brightness, power profile, battery, Do Not Disturb, Stay awake, Wi-Fi, wired, Bluetooth,
    theme, dark/light, the theme schedule and the current workspace's layout.
  - Volume, mic-mute and brightness keys work, including over the lock screen, and show an OSD:
    noti-fi's when it is running, otherwise uQ's own on the focused monitor.
- **Notifications.**
  - The Notifications island shows the latest app, the count and Do Not Disturb. Click it or
    press `SUPER+N` for the notification centre; right-click toggles Do Not Disturb.
  - `uqctl notif backend auto|notifi|builtin|swaync|mako|dunst` picks who shows them. `auto`
    uses noti-fi when it answers, otherwise uQ's built-in server with its own popups and centre.
  - uQ never starts, stops or masks another notification daemon; if one already owns the name,
    the island says so.
- **Lock, sleep, idle, power and polkit.**
  - `SUPER+L`, `loginctl lock-session`, idle and sleep lock the session with `uq-lock`, a
    separate Quickshell instance that unlocks with your password or fingerprint (PAM stacks
    `/etc/pam.d/uq-lock` and `uq-lock-fingerprint`, kept on upgrade). If the lock or the shell is
    killed, the session stays locked. `SUPER+CTRL+L` is the recovery bind.
  - The machine never sleeps before the lock is secure.
  - Idle dims, locks, turns the screens off and, on battery, suspends; each stage is set with
    `uqctl idle set`. Playing media holds off dimming and screens off.
  - `SUPER+Escape` opens the power menu (lock, log out, suspend, restart, shut down); restart and
    shut down wait 30 s and can be cancelled.
  - uQ is a polkit authentication agent: admin prompts from apps open a themed dialog.
- `uqctl mouse` sets pointer speed, acceleration, natural scroll, left-handed mode and scroll
  factor.

### Changed

- The packages build from the tagged GitHub Release tarball; `scripts/build-local-pkg.sh` builds
  one from a checkout.

## [0.1.0] - 2026-11-08

Phase 1: the foundation. Not published as a release.

### Added

- `uqd`, a session daemon that owns the theme, the dark/light schedule and per-workspace
  layouts, writes uQ's generated Hyprland config and rendered theme files, and serves
  `org.uq.Desktop1` on the session bus.
- `uqctl`, its command-line client: `theme list|set`, `layout set`, `schedule fixed|sun|off`
  and `setup` (writes `~/.config/hypr/hyprland.lua`; `--apps` also themes GTK, qt6ct, kitty,
  foot and alacritty, never overwriting your files).
- Seven themes, each in dark and light: Gruvbox, Dracula, Matcha, Forest, Graphite, Rosewood and
  Velvet. Every text and background pair meets WCAG AA contrast.
- A base Hyprland Lua config with per-workspace layouts (`SUPER+ALT+D` dwindle, `SUPER+ALT+M`
  master, `SUPER+ALT+S` scrolling) kept across reloads and reboots, and your own settings in
  `~/.config/hypr/uq/user.lua`.
- Arch packages `uq` and `uq-hyprland-pin`, which holds Hyprland to 0.56.2–0.57.x.
