# Changelog

All notable changes to uQ are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and uQ uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Release candidates are tagged
`vX.Y.Z-rc.N`; their package version is `X.Y.ZrcN`.

## [Unreleased]

### Added

- **Screenshots, the Plasma way.** Print freezes the screen and opens it in satty with the crop
  tool: drag an area, mark it up (arrows, boxes, text, blur), then Enter copies and saves it to
  `~/Pictures/Screenshots`. Shift+Print takes the whole screen and Super+Print the focused window,
  both copied and saved at once. All three are on the Keyboard shortcuts page, and **Screenshot**
  is in Spotlight and the app menu. (Spectacle itself only runs on KDE's own compositor.)
- **One button per open window in the bar.** The Work island shows each window on its screen
  (icon and title); click one to focus it, or to bring a minimised one back. The focused one keeps
  the window buttons and menu. With more than five windows it folds to one icon with the count,
  and opens again when clicked.
- **The window beside this one, at the screen's edges.** Push the pointer against the left or
  right edge of a screen, even over a fullscreen app, and the edge lights up with a small tab
  showing the icon of the window on that side. Click, or turn the wheel, to go there: the
  scrolling layout scrolls to it, and past the last window the tab offers the nearest workspace
  on that screen with windows in it (its number is shown). Nothing shows when there is nothing
  on that side, on an edge shared with another screen, or while locked. On the Shell page of
  Settings, **Show the window beside this one at the screen's edges** turns it off, and **Show
  the window's title on the tab** adds the title (hidden while the screen is shared). **Show it
  as: a slice of the window** swaps the tab for a strip of the neighbouring window itself, its
  near edge peeking in with its rounded corners and border, as the scrolling layout shows the
  column beside the one in view. It stays at the edge while a fullscreen or maximized window
  hides its neighbours, so every window can still be reached with a click or the wheel, and
  its border takes the accent colour when you point at it. While the screen is shared the
  slice shows the app icon instead of the window.

### Changed

- **Settings, redesigned.** Every page (except Shell, which follows the bar's own redesign) is
  split into tabs of two-column cards that fill a 1920x1080 window. Rows lead with an icon, labels
  are short, and longer explanations moved into "?" tooltips. Pages show rather than describe:
  theme tiles in each theme's own colours, a live window preview for gaps and corners, layout and
  rotation pictures, a battery gauge, power-mode tiles, a login screen preview, keyboard layouts
  and shortcuts drawn as keycaps, sample dates and money for your formats, and app icons in
  default apps, startup apps and rules. Search still finds every setting and opens its tab.
- **Quick settings has tabs.** The Control Center (Super+A, or the Status island's button) is
  split into **Sound**, **Network** (Wi-Fi and Bluetooth), **Display** (brightness, theme,
  dark/light, layout), **System** (battery, power profile, Do Not Disturb, Stay awake, profiles)
  and **Apps** (shown only when an app adds a tile), instead of one long list. The tab you pick
  stays while the panel is open and next time you open it. Left and Right move between tabs,
  Home and End jump to the first and last, and Ctrl+Tab / Ctrl+Shift+Tab switch tabs from
  anywhere in the panel. **Settings…** stays at the bottom of every tab.
- **Click to focus.** A window takes the keyboard only when you click it; moving the pointer over
  it no longer does (scrolling still reaches the window under the pointer). Put
  `hl.config({ input = { follow_mouse = 1 } })` in `user.lua` to have focus follow the mouse.
- **The login password unlocks the keyring.** The login screen now uses its own PAM service,
  `/etc/pam.d/uq-greetd` (greetd's, plus GNOME Keyring), so saved passwords are unlocked at login
  instead of asking again. `gnome-keyring` is now a dependency. An `/etc/uq/greetd.toml` you
  changed yourself (auto-login) keeps its `.pacnew`; add `[general] service = "uq-greetd"` to it.
- **One Wi-Fi icon.** nm-applet's tray icon is left out, since uQ shows the network beside the
  tray; nm-applet keeps running for VPN and enterprise Wi-Fi password prompts.
- **noti-fi's icon replaces the bell** while noti-fi handles notifications, and leaves the tray:
  left click does what it does in noti-fi, right click opens its menu, middle click toggles Do Not
  Disturb. With uQ's own notifications the bell stays.

### Fixed

- **Settings fixes:** adding a pointing device under Input → Devices no longer resets its speed
  to 0; keyboard, mouse and touchpad rows show when `user.lua` overrides them; the Login screen
  page's theme box is never blank; deleting a restore point asks first; the Login screen page
  comes and goes with uQ's login screen without restarting Settings (and no longer reappears on
  machines without it); searching for "Metered" or "Addresses and DNS" lands on a row you can see.
- **Packaging metadata matches the PKGBUILD again.** `.SRCINFO` now lists `kcolorscheme` and
  `kconfig` (build and runtime), and the packaging test checks the whole `depends` array for
  `polkit` instead of one exact line.
- **An app's bar or quick-settings widget runs its new code after a package update.** The shell
  kept the widget's old QML until it restarted. The bridge now sends a digest of each widget's
  files, and when it changes the shell loads the widget afresh.
- **Settings looks like the theme.** Outside Plasma, Kirigami took its colours from KDE's
  defaults, so Settings was Breeze Light with dark controls on top, whatever the theme. uqd now
  renders the theme as a KDE colour scheme (`~/.local/state/uq/theme/kde/uq.colors`), and Settings
  uses it, with the theme's icons and interface font. It follows a theme or light/dark change
  while it is open (controls drawn by Kvantum change at the next start). Dropdown lists are at
  least as wide as their box, the selected row in lists and menus uses the theme's text-on-accent
  colour, and Settings falls back to the Fusion style instead of failing when
  qqc2-desktop-style is missing. Plugin pages get the new colours as well.
- **Updating uq no longer leaves Hyprland in emergency mode.** Hyprland reloads its config when
  a file it read changes, and a package update replaces `/usr/share/uq/hypr/base.lua` by
  removing and rewriting it: a reload in between failed its `require` and left only Hyprland's
  emergency binds ("a lua config resulted in no binds"). uqd now follows Hyprland's
  `configreloaded` event, and when a reload failed only because a config file was missing that
  is there again, it reloads once more (at most once every 10 s).
- **`notify takeover --now` works on a uQ session that has been up a while.** uQ keeps the
  notification name with its built-in server until it has connected to noti-fi's socket, and it
  looked for that socket on the bridge backoff, every 30 s after the first half minute; noti-fi's
  takeover waited 10 s and rolled back. While the socket file is missing, uQ now looks every 3 s
  (the backoff still paces failed connects). noti-fi's takeover also waits longer when uQ holds
  the name.
- **Welcome's "Choose a look" buttons** showed `[object V4ReferenceObject]` and applying one said
  the theme wasn't installed; they show each theme's name and apply it (#40).
- **Settings pages no longer draw over each other** when you click through the page list
  quickly: pages switch at once, without the cross-fade (#40).
- **The bar's icons show on a fresh install.** The session runs Qt through qt6ct, which has no
  icon theme until your own `qt6ct.conf` names one, so the volume, battery and network icons,
  Settings' app icon and every app without its own icon were blank or a magenta square. uqd now
  writes the theme's icon theme to `~/.local/state/uq/theme/shell.env`, the shell and lock load it,
  and the shell restarts by itself when the icon theme changes (switching between light and dark
  switches Papirus-Light and Papirus-Dark, so it restarts then too).
- **The login screen no longer starts a desktop session for the `greeter` account.** Hyprland
  started `hyprland-session.target` there, which brought up portals, PipeWire and xdg-user-dirs
  for an account whose home is `/`, and left a failed `xdg-desktop-portal-gtk` at every boot.
- **Settings' drop-down lists no longer log a TypeError each** (`positionToRectangle … is not a
  function`) under the KDE desktop style.
- **The Users page opens again.** It used `Kirigami.Avatar`, which Kirigami 6.31 no longer has, so
  the page didn't load, and every page opened after it was drawn on top of the page before. A
  user's picture is now uQ's own, with their initial when there is none.
- **A Settings page that can't load says so** in its place, with the error, instead of leaving
  the previous page drawn underneath every page after it. `uq-settings --screenshots` exits 1
  over one, and a new test opens every page in CI.
- **Opening Settings on a page no longer toggles its window maximized.** Each `uqctl settings
  <page>` (and each link from the shell into Settings) maximized the open window or restored it,
  and Settings remembered whichever state it ended in.
- **Qt apps get the theme's icons.** With `uqctl setup --apps`, qt6ct loaded uQ's colours but
  named no icon theme, so Qt apps (Dolphin, uQ Settings) found no themed icons. uqd now keeps
  `icon_theme=` in that `qt6ct.conf` in step with the theme, as it does gsettings' for GTK apps,
  and leaves a `qt6ct.conf` of your own alone. Apps pick it up when they next start.
- **Drop-down lists in Settings have a maximum width**, so a long choice no longer squeezes the
  row's label (on Input, "Switch layouts with" wrapped two letters a line).

## [0.3.0-rc.2] - 2026-11-12

### Added

- **Pages from scdr, crumBS, noti-fi, XZram and scale_tail in Settings.** **Scheduling** shows
  whether scdr is on, pauses and resumes it, and edits per-app rules and the scheduler.
  **Storage and cleanup** shows disk use, cleans caches and leftovers, puts a cleanup back and
  uninstalls apps. **Notifications** sets do not disturb, automatic do-not-disturb and how much
  history noti-fi keeps. **Memory and swap** changes the zram size and algorithm and the
  swappiness. **Tailscale** connects, picks an exit node, accepts routes and signs out. The pages
  ship with those apps (update the app to get its page) and appear under **Apps** when the app is
  installed. New optional dependencies `xzram` and `scale_tail`. A weekly check runs each app's
  page against uQ (`.github/workflows/companions.yml`).
- **Display arrangements, the main display, and touchpad gestures.** The Displays page can
  **Remember for these screens**: each arrangement is matched by the make, model and serial of
  every connected screen (not by port), and when screens are plugged in or out uqd switches to
  the one for the new set within about 2 seconds, with no prompt (only when the set of screens
  changes, so a layout you keep stays until then); **Switch arrangements when
  screens change** turns it off, and automatic switches aren't recorded in History. A **main
  display** gets notifications, the volume pop-up and Spotlight, starts the session, and is the X11
  primary when `xrandr` is installed (new optional dependency `xorg-xrandr`). **Span across
  displays** draws one wallpaper over all screens. The **workspace planner** puts workspaces on
  screens one after another, in turn or by hand, and moves them back when a screen returns.
  **Bar per screen** shows the bar on all screens, the main one or chosen ones, and never on none.
  The Input page gains **touchpad gestures** with conflict checks (the editor warns and uqd
  refuses a save that Hyprland would refuse) and the workspace swipe options. New `uqd` call
  `RememberMonitors`; `ListOutputs` gains `roles` and `arrangement`; `SetSection("monitors")` now
  takes every key except the layout. The three-finger swipe moved from `base.lua` into
  `generated.lua`, so a gesture of your own in `user.lua` that overlaps uQ's makes Hyprland
  refuse the config.

### Fixed

- **Recording a shortcut on the Keyboard shortcuts page works on Hyprland.** Capture never
  started, because Hyprland only registers a submap that holds at least one bind; uQ's capture
  submap now has one placeholder bind (`SUPER+SHIFT+CTRL+ALT+XF86Launch9`). Found by the new live
  checks in `tests/live`.

## [0.3.0-rc.1] - 2026-11-11

### Added

- **Settings pages: longer D-Bus deadlines for writes that may ask for a password.** A page's
  `[dbus]` table takes `timeout_ms = { Method = <ms> }`, up to 120 seconds, for listed methods
  only; every other request still ends within 2 seconds. Checked by the manifest loader and
  `uqctl integrations check`. See docs/settings-pages.md.
- **`uqctl integrations check --no-programs`.** Skips only the check that each command's and
  app action's program is installed, so an app's CI can check its `uq/` folder without
  installing the app. Paths are still checked.
- **Settings pages may ship sibling files.** A page folder may hold a `logic.js` imported from
  its QML (`import "logic.js" as Logic`) and further component files; tested and documented in
  docs/settings-pages.md.
- **Settings pages follow changes instead of polling.** A page's read `[[command]]` takes a
  `watch` table (files, absolute or under `~/`, and D-Bus signal matches on the session or system
  bus); `ctx.command.watch(id, callback)` runs it now and again after each debounced burst of
  changes, one run at a time, until `ctx.command.unwatch` or the page closes. See
  docs/settings-pages.md.
- **`ctx.openApp(desktopId)` for Settings pages.** A page can open an app (its own full app,
  say) by its `name.desktop` id, the way the launcher does (`uqctl launch`), detached from
  Settings. Paths and other ids are refused.
- **A stable component list and a test kit for Settings pages.** docs/settings-pages.md lists the
  `org.uq.settings` components pages may use under `uq_api = 1` (`UqLabel`, `UqComboBox`,
  `SettingRow`, `SwitchRow`, `ValueSlider`, `ErrorCard`) with their properties.
  `templates/settings-page-tests/` is a kit an app copies into its repository: a stub
  `org.uq.settings` module, a stand-in `ctx` and an example offscreen `qmltestrunner` test. The
  template page now keeps its logic in `logic.js`.
- **Keyboard shortcuts.** A Keyboard shortcuts page lists every uQ shortcut by what it does:
  change its keys (press them to capture, or pick them), turn it off, reset it, or put all back.
  Add your own shortcuts: open a uQ panel, a window or workspace action, a layout, start an app
  (through `uqctl launch`) or run a command in its own scope. Two shortcuts on the same keys are
  refused, naming both. New default **Open Spotlight for emoji** (Super + period). The lock recovery shortcut stays on. New **Switch to the next profile**
  shortcut, and profiles can turn shortcuts on and off. `uqctl keys list|reset` works from a
  terminal. New `uqd` calls `ListBinds`, `PauseShortcuts` and `ResumeShortcuts`; `keybinds` is a
  `GetSection`/`SetSection` domain. A failed key capture never leaves shortcuts paused, and uqd
  resets the capture state at start.
- **Rules from apps.** Installed apps can offer window rules in
  `/usr/share/uq/integrations/<name>/rules.toml` (app ID patterns, a window title pattern, a
  workspace, full screen, float). The Window rules page lists them under **Rules from apps**,
  each off until you turn it on; turning one on is checked, rendered and verified like your own
  rules, and shows in History, where it can be undone. A rule whose app is removed stops applying
  and comes back with the app. `uqctl integrations check` checks `integrations/<name>/rules.toml`
  and `uqctl integrations list` shows the files. New `uqd` call `ListIntegrationRules` and the
  `integration_rules` section. See docs/integrating.md.
- **Emoji key.** `Super + .` opens Spotlight with `:` already typed, the emoji search an emoji
  provider such as emobie answers. Pressed while Spotlight is open, it starts the search over at
  `:`. The bind is `uq:emoji` in `base.lua`.
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
- **Welcome.** A short tour in Settings the first time you log in: what uQ is and the three keys
  to know, then the theme (with automatic light and dark), the wallpaper, how windows are
  arranged, your screens (only with more than one), the apps made for uQ that are installed and
  where to get the others, and saving a fingerprint for the lock screen (only with a reader and
  no finger saved yet). Every step can be skipped and every choice applies at once. Welcome
  shows once: closing it at any step counts as done. See it again from Settings → About → Show
  Welcome, or with `uqctl welcome --reset`.
- **Installer hand-off.** An installer's first-login window (bindle's app picker) can hold
  Welcome back with `$XDG_RUNTIME_DIR/uq/welcome.defer` and open it afterwards with
  `uqctl welcome`, so the two never show at once. A file older than 30 minutes is ignored, so a
  crashed installer can't hide Welcome for good. See docs/welcome.md.

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
  `Activatable=true`. When Spotlight or the app menu opens, uQ asks the session bus to start each
  such provider that isn't running (at most once every 30 seconds each, never while you type), so
  a provider no longer has to run all the time. Results still come only from running providers,
  so the first keystrokes after a cold start may miss them.
- **`uq-tokens` for web apps.** A small JavaScript module and CSS file (`web/uq-tokens/`) that
  Tauri, Electron and browser apps copy in to follow the uQ theme: colours, rounding, gaps and
  fonts become `--uq-*` CSS properties and update live when the theme changes. Bad values are
  ignored, and without uQ the app's own theme applies. The README has the Tauri and Electron
  recipes and a version check for vendored copies.
- **Sound, Bluetooth and Network in Settings.** Sound and Bluetooth join the Devices pages and
  Network gets its own Connections group. A missing service shows "Not available" with the
  package to install.
  - **Sound:** pick the speakers or headphones to play through and the microphone to record
    from, with volume and mute for each; set each app's volume and which device it plays through
    or records from; choose a sound card's mode; and play a test sound (needs
    `sound-theme-freedesktop`). The page updates by itself when devices or apps come and go.
  - **Bluetooth:** turn Bluetooth on or off, let other devices find this computer, look for new
    devices, pair, connect, disconnect, trust and forget them, and see their battery. When a
    device shows a code, the page shows the same code and waits for you to press Confirm;
    nothing is accepted on its own. The page answers pairing requests only while it is open.
  - **Network:** turn Wi-Fi on or off, see nearby networks, join one (also hidden networks and
    work or school networks that need a user name or certificate), and forget saved ones;
    wired connections; WireGuard VPNs (import a `.conf` file and edit it) and OpenVPN
    connections if NetworkManager's OpenVPN plugin is installed; a hotspot; addresses and DNS
    for IPv4 and IPv6; and marking a connection as metered. Passwords go straight to
    NetworkManager and are saved for you only, unless you tick "Everyone who uses this computer
    can join it too". uQ never writes them down or logs them.
- **System pages in Settings.** Power, Users, Date and time, Region, Default apps and About join
  Startup under System, and a Login screen page appears when uQ is your login screen. They talk
  to the system's own services, so they work even when uQ's settings service isn't running.
  Changes that need an administrator ask for the password; cancelling leaves the setting as it
  was, and a missing service is shown as "Not available" with the package to install.
  - **Power:** power mode, battery charge, time left and health, and what closing the lid (also
    while plugged in) or pressing the power button does. Those three are saved by the settings
    helper and take effect after a restart; without the helper they are hidden with a note.
  - **Users:** change a person's name, picture, password or account type, add people and remove
    them (with or without their files). Removing needs the user name typed; you can't remove
    yourself or the last administrator, or make the last administrator a standard user.
    Password boxes are emptied after every try, and passwords are hashed before they leave the
    app.
  - **Login screen:** the login screen's theme (or "same as mine", copied when saved),
    wallpaper (yours, a picture up to 20 MiB, or none), your display layout, hiding the user
    list, reduce motion, and auto-login, with a warning that anyone at the computer gets your
    session and that it's only for fully encrypted disks. The Users page links here. Without
    the settings helper it is view-only.
  - **Date and time:** time zone, setting the time from the network or by hand, and the bar's
    24-hour clock.
  - **Region:** system language, formats (dates, numbers, money, measurements, paper) and the
    text console's keyboard layout.
  - **Default apps:** web browser, email, files, text editor, pictures, videos, music, PDF
    documents and the terminal. Only the choices you make change your `mimeapps.list`; every
    other line in it stays.
  - **About:** the computer's name (editable), system, kernel, processor, graphics, memory, and
    the uQ, Hyprland and Quickshell versions.
- **Widgets from other apps.** An app can add a small widget to the bar (an icon in the Status
  island; up to four, the rest under a "…" button) or a tile to a new **Apps** group at the bottom
  of quick settings, by installing a folder to `/usr/share/uq/shell/widgets/`. Widgets appear
  and go a second or two after their package is installed or removed, without logging out, and
  only while what they need (a program, a running service) is there. A widget that fails to load
  or breaks is hidden on its own and tried again a minute later; it never takes the bar down. A
  widget can only call the D-Bus methods and run the commands its `widget.toml` lists.
- **App actions.** Right-clicking an app in Spotlight or the app menu lists actions other apps
  added for it, such as "Uninstall…" or "Check for update". The action runs as its file says,
  in your terminal when it asks for one; the clicked app's id is the only thing filled in.
- **Writing integrations.** `docs/integrating.md` covers all four ways an app can plug into uQ
  (theme, settings page, widget, launcher provider and app actions), what uQ checks, and a
  checklist for each. New templates: `templates/widget/` and `templates/provider/` (a small Rust
  launcher provider that quits after a minute without use).

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
- **The lock screen no longer waits for ever on a password check.** If PAM has not answered
  after 60 seconds, the check is stopped and the lock screen says "That took too long. Try
  again." Esc stops a check that is still waiting.
- **A slow session bus or uqd no longer holds up the shell's start.** The bridge gives up on
  connecting, subscribing and asking for uqd after 2 seconds and tries again later, and sends
  `Hello` after at most 2 seconds of waiting for Welcome's status (the shell asks again).
- **`uqctl history | head` no longer panics.** uqctl stops quietly when the program reading its
  output closes the pipe.
- **Reduce motion does something.** It is now your own setting (Settings → Look and layouts →
  Reduce motion, `ui.reduce_motion`): the idle dim and wallpaper changes happen at once, and
  window and panel animations are off. The login screen's own Reduce motion stops its user list
  from coasting when flicked; it already had no other animations.
- **"Use my display layout at login" is used.** The login screen now arranges its screens from
  the copied layout (`/etc/uq/greeter-monitors.toml`, checked like your own; a bad file falls back
  to the automatic layout). Settings → Login screen has **Use automatic layout** to go back
  (new helper call `ClearGreeterMonitors`).
- **Changing your own password asks for your current one, not an administrator's.** Settings →
  Users now changes it through `passwd`, which checks the current password and the password
  rules and says plainly why a password was refused. Other people's passwords still go through
  the accounts service.
- **The Bluetooth page no longer crashes when an adapter goes away while it reads.**
- **The shell says when something you asked for didn't happen.** The app menu and Spotlight
  stay open and say why when an app can't be opened (they close once it has been), Spotlight
  says when search isn't working, and quick settings says why a theme, schedule or layout switch
  failed. Each failure is also logged.
- **The Power page updates as the battery and power mode change.** It follows UPower and
  power-profiles-daemon instead of reading everything again every 30 seconds, so the charge and
  a mode changed elsewhere show at once. Only when the system bus can't be followed does it
  fall back to reading again every minute.
- **Removing the auto-login user turns auto-login off.** The login screen no longer goes on
  trying to log in an account that's gone; if turning it off is refused, the Users page says so.
  `uq-settings-helper` also refuses auto-login for a locked or expired account.
- **Safer file writes.** Every all-or-nothing write in uQ now uses a temp file no other write
  shares (two writes at once can no longer trip over each other) and syncs the folder after the
  rename, so a crash right after a save keeps it. A file that is a symlink is still replaced by
  a regular file, as before; this is now documented.
- **An integration command that starts something in the background no longer times out.** The
  runner used to wait for its output to close, which a background process it started keeps
  open; it now ends the run when the command exits, keeping the output that arrives within
  200 ms.
- **docs/settings-pages.md says what the command path check covers.** The `/usr/bin` and
  `/usr/lib` check is on the path as written; a symlink there that points elsewhere is accepted.

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
