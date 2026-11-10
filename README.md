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

uQ is not on the AUR yet: `uq` and `uq-hyprland-pin` go up with the first release, v0.2.0.
After that, `yay -S uq` (or `paru -S uq`) installs both, since `uq` pulls in `uq-hyprland-pin`.

Until then, install from a checkout (builds HEAD; uncommitted changes are not included):

```bash
scripts/build-local-pkg.sh --package uq-hyprland-pin -si
scripts/build-local-pkg.sh -si
uqctl setup
```

`build-local-pkg.sh` passes its other arguments to `makepkg` and leaves the packages in
`target/pkg/`. Releases and the AUR update are described in [docs/releasing.md](docs/releasing.md);
the changes in each release are in [CHANGELOG.md](CHANGELOG.md).

`uqctl setup` writes `~/.config/hypr/hyprland.lua`. With `--apps` it also writes the GTK, qt6ct,
kitty, foot and alacritty files that load uQ's theme; those apply in every desktop you use, not
only Hyprland, so they are opt-in. It never overwrites a file you already have; it prints the line
to add instead. Then log in to Hyprland. uqd starts with the Hyprland session and stops when it
ends.

Installers (bindle offers uQ as its desktop) can run `uqctl setup` unattended, as the user who
will log in (for example `runuser -l <user> -c 'uqctl setup --json'`). Running it again changes
nothing. The result is in the exit code, and `--json` prints it as one object on stdout:

| Exit | Meaning |
|---|---|
| 0 | uQ's `hyprland.lua` is in place (written now, or already there) |
| 1 | error: uQ's `base.lua` is missing (the package isn't installed), or a file couldn't be written |
| 2 | refused: run as root, which would set up root's account instead of the user's |
| 3 | the user's own `hyprland.lua` was kept; the session isn't uQ until they add the stub lines |

Theme files kept by `--apps` are reported but never change the exit code.

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

History and restore points (uqd records every change, who made it, and what it was before):

```bash
uqctl history [--limit 20]             # id, time, origin (settings, uqctl, shell, ...), summary
uqctl undo [<id>] [--force]            # no id: the newest change; --force if it changed again since
uqctl restore list                     # also: restore save <name>, restore delete <id>
uqctl restore <id>                     # bring back every setting as it was; to go back,
                                       # restore the newest auto:before_restore point it saved
```

The shell runs as `uq-shell.service` (Quickshell, started with the session). `SUPER+CTRL+L`
restarts the lock surface (`uq-lock.service`, else `hyprlock`) if it ever dies.

### Settings

`uqctl settings` (or **Settings…** at the bottom of quick settings) opens `uq-settings`: Appearance
(fonts, cursor, icons, Kvantum for Qt apps) and Wallpaper (per monitor, slideshow, fallback
colour), Look and layouts (gaps, borders, rounding, blur, shadow, animations and their speed,
default layout, per-workspace rules, dwindle/master/scrolling options) and Shell (islands on or off,
24-hour clock, launcher providers on/off and order, terminal for `Terminal=true` apps, idle,
notification and privacy settings). Animations now come from `generated.lua`, so an
`hl.animation` override in `user.lua` still wins. `uqctl settings look` (or `wallpaper`, `shell`,
...) opens a page directly, and a second call raises the same window. Run `uqctl setup --apps`
once to hook Qt apps up to the uQ Kvantum theme. Other packages can add a page under Apps by
installing `/usr/share/uq/settings/pages/<id>/page.toml` with a QML file. Settings are also
available over D-Bus as `GetSection` / `SetSection` on `uqd`. Every write method there
(`SetSection`, `SetSetting`, `SetTheme`, `SetLayout`, `SetSchedule`) takes a last `origin` string
that the history records; `""` records as unknown.

#### History page

The **History** page lists your restore points and every change from the last 30 days (up to 500),
newest first, each with where it came from (the Settings app, `uqctl`, the shell). **Undo** on a
row puts that setting back; if it was changed again since, the app asks first. **Save** keeps a
restore point under a name you pick, and **Restore** brings every setting back as it was, after
saving a restore point of your current settings, so a restore can be undone too. uQ also saves
one on its own before the first change of each day, before a restore, and when uQ is updated
(named after the new version); it keeps the last 20 of these, and the ones you save are never
removed. In the app, Ctrl+Z and Ctrl+Shift+Z undo and redo your own changes this session, and
"Changed this session" lists them. Rows on the other pages say when a value comes from
`user.lua` or was changed while running, with **Re-apply** to put uQ's value back. uQ never
writes to `user.lua`. Manual checks are in
[docs/notes/settings-slice-a-live-check.md](docs/notes/settings-slice-a-live-check.md).

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
qs ipc -p /usr/share/uq/shell call uq open spotlight    # or: close, toggle; spotlight | appmenu | quicksettings | powermenu
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

### Lock, idle and power

- `SUPER+L` locks the screen. Scripts can lock too (ignored when already locked):
  `qs ipc -p /usr/share/uq/shell call uq lock`.
- `SUPER+Escape` opens the power menu on the focused monitor: Lock, Log out, Suspend, Restart and
  Shut down. Restart and Shut down count down 30 s and then run; Esc, Cancel, a click outside the
  menu or locking the screen aborts the countdown. The app menu's restart and shut-down buttons
  open the power menu at that countdown.
- `SUPER+CTRL+L` is the recovery bind: it restarts `uq-lock.service`, or runs `hyprlock` if that
  fails.

The lock screen is a separate Quickshell instance, `uq-lock.service`, run through
`/usr/lib/uq/uq-lock-run` (which keeps Quickshell's PAM relay lines out of the journal). If it or
the shell is killed, the session stays locked and the lock comes back on its own.

It takes your password or, once you've enrolled a finger, your fingerprint:

```bash
fprintd-enroll
```

Whichever succeeds first unlocks. The PAM stacks are `/etc/pam.d/uq-lock` (password, via
`system-local-login`) and `/etc/pam.d/uq-lock-fingerprint` (fingerprint only). Both are marked
`backup` in the package, so your edits are kept on upgrade. The lock runs only PAM's auth phase,
not the account phase, so account checks such as password or account expiry don't stop an
unlock; faillock still applies, since it hooks the auth phase.

The screen locks before the machine sleeps: uQ holds a logind delay inhibitor and releases it
once the lock is secure (or after 2 s at the latest).

Idle stages and their defaults:

| Stage | Default | Setting |
|-------|---------|---------|
| Dim the screens | 4 min | `dim` |
| Lock | 5 min | `lock` |
| Screens off | 6 min | `screen-off` |
| Suspend, on battery only and only behind a secure lock | 20 min | `suspend` |
| Screens off after locking | 30 s | `locked-screen-off` |

```bash
uqctl idle set dim 120        # seconds, or off
uqctl idle set suspend off
```

Any input removes the dim. While media is playing, dim and screens off wait, but lock still
happens (unless the `idle.media_blocks_lock` setting is on). Stay awake in quick settings blocks
every stage.

uQ is also a polkit authentication agent: when an app asks for admin rights (`pkexec`, a
settings app), a dialog opens on the focused monitor. The first agent to register for the session
wins, so if another one is running (KDE's, GNOME's, lxpolkit) it handles the prompts instead. The
dialog hides while the screen is locked and comes back after you unlock.

The lock uses Hyprland's `hyprland-lock-notify-v1` protocol (see Credits).

Your own Hyprland settings go in `~/.config/hypr/uq/user.lua`, which uQ loads last and never
rewrites.

## Develop

```bash
cargo test --workspace                     # unit, golden, D-Bus and Lua-mock tests
cargo test -p uqd --test verify_config -- --ignored   # needs Hyprland installed
scripts/lint-shell.sh                      # no bare Text, Label or Qt.createQmlObject in shell/
for t in tests/shell/*.sh; do bash "$t"; done
QT_QPA_PLATFORM=offscreen qmltestrunner -import shell -input tests/qml
scripts/smoke-shell.sh                     # needs Hyprland, quickshell, jq, a DRM card and a seat; 77 = skipped
```

## Credits

- `crates/uqctl/protocols/hyprland-lock-notify-v1.xml` is vendored unchanged from
  [hyprwm/hyprland-protocols](https://github.com/hyprwm/hyprland-protocols),
  Copyright © 2025 Maximilian Seidler, under the BSD 3-Clause License (its full text is in the file).
- Theme credits are in [`themes/CREDITS.md`](themes/CREDITS.md).
