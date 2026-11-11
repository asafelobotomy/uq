# Changelog

All notable changes to uQ are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and uQ uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Release candidates are tagged
`vX.Y.Z-rc.N`; their package version is `X.Y.ZrcN`.

## [Unreleased]

### Added

- **Keyboard shortcuts.** A Keyboard shortcuts page lists every uQ shortcut by what it does:
  change its keys (press them to capture, or pick them), turn it off, reset it, or put all back.
  Add your own shortcuts: open a uQ panel, a window or workspace action, a layout, start an app
  (through `uqctl launch`) or run a command in its own scope. Two shortcuts on the same keys are
  refused, naming both. The lock recovery shortcut stays on. New **Switch to the next profile**
  shortcut, and profiles can turn shortcuts on and off. `uqctl keys list|reset` works from a
  terminal. New `uqd` calls `ListBinds`, `PauseShortcuts` and `ResumeShortcuts`; `keybinds` is a
  `GetSection`/`SetSection` domain. A failed key capture never leaves shortcuts paused, and uqd
  resets the capture state at start.
- **Your own themes: make, import, export and delete.** The Appearance page has **New theme from
  this one…** (seven colours per version, live contrast check, a version that is hard to read
  can't be saved), **Import…**, **Export…** and **Delete…**, and shows the theme names. Themes
  are `.uqtheme` files (TOML, `format = 1`, size-capped, schema-checked, never run) saved in
  `~/.local/share/uq/themes/user-<slug>/`; imports save a restore point and can bring window
  gaps, corners and effects. `uqctl theme list --long|import|export|delete` do the same, and the
  shell's quick settings show names. New `uqd` calls `ListThemeInfo`, `ExportTheme`,
  `ImportTheme`, `DeleteTheme` and the `ThemesChanged` signal.
- **uQ session.** A "uQ" entry in your display manager's session list. Its first login sets up
  uQ for you (no `uqctl setup` needed); if setup reports a problem, the problem goes to
  `~/.local/state/uq/session.log` and the desktop starts anyway. `uqctl setup` gains `--quiet`.
- **Login screen.** uQ's own greeter for greetd, in uQ's look: users with their pictures, the
  password field, the session to start (uQ first), and Suspend, Restart and Shut down (click
  twice). It follows the theme, variant and wallpaper chosen for it and never reads anyone's home
  folder. Wrong passwords, locked accounts and other login messages show as plain text; an
  expired password asks for a new one twice and checks they match. If it fails 3 times in a
  minute, greetd's text login takes over so nobody is locked out. Turn it on with
  `run0 uqctl greeter enable` (needs `greetd`) and off with `run0 uqctl greeter disable`, which
  brings back the display manager it replaced. See docs/greeter.md.

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
- **Settings history, undo and restore points.** `uqd` records every settings change (who made
  it, and the values before and after) in `~/.local/state/uq/history/` for 30 days or 500
  entries, and keeps restore points of `state.toml` in `~/.local/state/uq/restore/` (automatic
  ones before the first change of each day, before a restore and when uQ is updated, 20 at most;
  your own are never removed). New `uqctl history [--limit N]`, `uqctl undo [<id>] [--force]` and
  `uqctl restore list | save <name> | delete <id> | <id>`. On D-Bus: `ListHistory`, `Undo`,
  `ListRestorePoints`, `SaveRestorePoint`, `DeleteRestorePoint`, `Restore` and a `HistoryChanged`
  signal. **Breaking for D-Bus callers:** `SetSection`, `SetSetting`, `SetTheme`, `SetLayout`
  and `SetSchedule` take a last `origin` string argument (`""` records as unknown).
- **Where a setting's value comes from.** `uqd` can explain a setting's live value (theme,
  default, uQ, `user.lua`, changed while running, or error). Setting rows in uq-settings say
  "Overridden in user.lua, line N" or "Changed while running (not by uQ)", with a Re-apply
  button. uQ reads `user.lua` and never writes it.
- **Undo and History in uq-settings.** Ctrl+Z and Ctrl+Shift+Z undo and redo your changes, a
  "Changed this session" list shows what changed (including changes "from the terminal"), and a
  History page lists restore points and past changes, with Undo, Save and Restore. The window
  remembers its size.
- **Displays page.** Arrange outputs by dragging, and set mode, refresh rate, scale, rotation,
  mirroring, VRR and on/off. A change applies live and a dialog asks to keep it; if you don't
  answer within 15 seconds (or press Escape, or the app or uqd stops), the old layout comes back.
  `uqd` gains `ListOutputs`, `ApplyMonitors`, `ConfirmMonitors`, `RevertMonitors` and a
  `MonitorsChanged` signal; only a confirmed change is saved and recorded in the history.
- **Input page.** Keyboard layouts (from xkeyboard-config's list) with variants, order and a
  switch combo, repeat delay and rate, mouse and touchpad speed, acceleration, natural scroll,
  tap-to-click and per-device overrides. `uqd` gains an `input` section and `ListInputDevices`.
- **Profiles.** A profile is a named set of settings (Look and layouts, Appearance with theme,
  light/dark and fonts, Shell, Wallpaper, and a power mode) that you turn on together and
  turn off with Back to normal, which keeps anything you changed by hand since. New Profiles
  page in uq-settings, a Profile row in quick settings, and `uqctl profile list | apply | off |
  next | export | import | delete`. Each switch saves a "Before the ... profile" restore point and
  shows in History as "from the ... profile". A power mode goes through the system's power
  service; if it can't be set, the rest still applies with a notice. While a profile that sets
  light or dark is on, the light/dark schedule doesn't switch it. Export and import use TOML
  (`format = 1`); keybinds can't be in a profile yet. `uqd` gains `ListProfiles`,
  `GetProfile`, `SetProfile`, `DeleteProfile`, `ApplyProfile`, `ProfileOff` and a
  `ProfileChanged` signal.
- **Window rules.** A Window rules page in uq-settings: rules that match a window by App ID, title,
  state or workspace and make it float, open on a workspace, blur less, and more, with a live
  "Matches now" preview and **Pick a window** (click a window to fill in its App ID). **Why does
  this window…?** lists the rules that apply to a window in order and flags any that `user.lua`
  overrides. Five templates, panel rules (bars, notifications, launchers) and workspace rules that
  join the per-workspace layouts are included. Rules have history, undo and restore points, and a
  profile can turn them on and off. `uqd` gains the `rules` section, `PickWindow`, `PickDone`,
  `ListWindows` and a `PickRequested` signal. uQ never writes `user.lua`; name a rule
  `uq:<id>` there to override it.
- **Startup apps, environment variables and app scaling.** A Startup page (under System) lists
  the apps in the autostart folders, with a status for each (Running, Waiting, Failed with its
  last output, and so on), a start delay, and add-an-app-or-command (run without a shell). uqd now
  starts them once per session, each in its own scope under `app.slice` with its output in the
  journal, and a restarted uqd starts nothing new. Turning off a system entry writes a hidden
  copy marked `X-uQ-Autostart=true`; your own files are never edited. Environment variables set
  here go to Hyprland and `~/.config/environment.d/60-uq.conf`. An Apps and scaling page has five
  switches (Sharp X11 apps, Electron on Wayland, Chromium-based browsers on Wayland, Qt apps prefer
  Wayland, Steam UI scale), a Blurry right now list and per-app fixes (Run on, Size, extra
  variables and options), which also apply to autostart. "Also when started from other
  launchers" writes a desktop-file override marked `X-uQ-Override=true`, and Chromium-based
  browsers get a flags file with a marker line. uQ never writes through a symlink and never edits
  or deletes a file it didn't mark; it replaces its own `60-uq.conf` whatever it holds. All three
  domains have history, undo and restore points, and a profile can hold environment variables,
  the switches, a fix's Run on and Size, and an existing startup app's on/off and delay (never a
  new command, launch options or a launcher file; startup changes apply at next login).
  `uqd` gains `ListAutostart`, `ShellReady` and an `AutostartChanged` signal, and the `env`,
  `compat` and `autostart` sections use `GetSection` / `SetSection`. `uqctl launch <desktop-id>`
  launches an app the way uQ's launcher does.
- **Settings helper.** A small system service, `uq-settings-helper`, lets Settings change a few
  system-wide things after you confirm with an administrator password: what the lid and power
  button do, and the login screen's theme, wallpaper, monitor layout and auto-login. Each change
  writes one fixed file and accepts only a fixed list of values; it never restarts logind (the
  lid and power-button change applies at its next reload). The wallpaper is copied as-is, up to
  20 MiB, and never opened as an image. Turning auto-login on or off asks for the password every
  time. It starts when needed, runs in a tight sandbox with no special privileges beyond root's
  own files, and quits after 30 seconds without use.
- **Settings pages from other apps can do more.** A page's `page.toml` can now list fixed
  commands it may run (`[[command]]`), conditions for it to show (`[requires]`: a program in
  `/usr/bin`, a running service) and the one D-Bus object it talks to (`[dbus]`, on the session
  or system bus). The page gets `ctx.command.run` and `ctx.dbus.call` / `ctx.dbus.get`; a
  command marked `confirm` asks "Are you sure?" with the page's own question first. Commands
  run without a shell, with a clean environment, time limits and output limits. A page's id
  must now match its folder name. Pages installed or removed while Settings is open appear or
  go within a second or two. Writing one: `docs/settings-pages.md` and
  `templates/settings-page/`.
- **`uqctl integrations`.** `uqctl integrations list [--json]` shows every widget, settings
  page, launcher provider and app action uQ found under `/usr/share/uq/`, whether it shows, and
  why not. `uqctl integrations check <dir>` checks an app's `uq/` folder before packaging (the
  layout, every file, the plain-text rule, that each command's program exists).
- **Launcher providers can be started on demand.** A `.provider` file may say
  `Activatable=true` (read now; starting such providers when the launcher opens comes next).
- **`uq-tokens` for web apps.** A small JavaScript module and CSS file (`web/uq-tokens/`) that
  Tauri, Electron and browser apps copy in to follow the uQ theme: colours, rounding, gaps and
  fonts become `--uq-*` CSS properties and update live when the theme changes. Bad values are
  ignored, and without uQ the app's own theme applies. The README has the Tauri and Electron
  recipes and a version check for vendored copies.

### Changed

- `base.lua`'s start and shutdown hooks do nothing when `UQ_DEV_SESSION` is set, so a nested
  dev session can no longer restart, repoint or stop the installed uqd and shell.
- **Animations moved to `generated.lua`.** The animation lines are no longer in `base.lua`.
  After an upgrade Hyprland shows its own default animations until `uqd` re-renders
  `generated.lua` (restart `uqd` or change any setting). `hl.animation` calls in `user.lua`
  still win.

### Fixed

- **Restore points saved in the same second list newest first.** They used to sort by a
  random suffix, so "the newest point" could be an older one.

## [0.2.1] - 2026-11-10

### Fixed

- **Displays: the 15 s to keep a change starts when it shows.** uqd counted the window from
  when the request arrived, before the config check (up to 10 s) and the apply, so on a slow or
  busy machine a new layout could show for a second or two before reverting.
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
