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

You don't have to run `uqctl setup` yourself: the package adds a **uQ** session to your display
manager, and its first login runs setup for you. uQ also has its own login screen for greetd:
`run0 pacman -S greetd && run0 uqctl greeter enable` (see [docs/greeter.md](docs/greeter.md)).

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

Profiles (a named set of settings you turn on together; see the Profiles page below):

```bash
uqctl profile list                     # id, name, number of settings; * marks the one that's on
uqctl profile apply <id>               # turn a profile on
uqctl profile off                      # Back to normal
uqctl profile next                     # normal -> each profile in list order -> normal
uqctl profile export <id> [file]       # write the profile's TOML (to the screen without a file)
uqctl profile import <file> [--replace] # add a profile from a file; --replace swaps one with the same id
uqctl profile delete <id>              # a profile that is on can't be deleted
```

The shell runs as `uq-shell.service` (Quickshell, started with the session). `SUPER+CTRL+L`
restarts the lock surface (`uq-lock.service`, else `hyprlock`) if it ever dies.

### Settings

`uqctl settings` (or **Settings…** at the bottom of quick settings) opens `uq-settings`: Appearance
(fonts, cursor, icons, Kvantum for Qt apps) and Wallpaper (per monitor, slideshow, fallback
colour), Look and layouts (gaps, borders, rounding, blur, shadow, animations and their speed,
default layout, per-workspace rules, dwindle/master/scrolling options) and Shell (islands on or off,
24-hour clock, launcher providers on/off and order, terminal for `Terminal=true` apps, idle,
notification and privacy settings), Displays (arrangement, mode, scale, rotation, mirroring, VRR)
and Input (keyboard layouts and switch combo, repeat, mouse, touchpad, per-device settings).
Animations, monitor lines and keyboard layouts now come from `generated.lua`, so an
`hl.animation` or `hl.monitor` line in `user.lua` still wins. A display change applies at once
and asks to be kept; if you don't keep it within 15 seconds, the old layout comes back. `uqctl settings look` (or `wallpaper`, `shell`,
...) opens a page directly, and a second call raises the same window. Run `uqctl setup --apps`
once to hook Qt apps up to the uQ Kvantum theme. Other packages can add a page under Apps by
installing `/usr/share/uq/settings/pages/<id>/page.toml` with a QML file. Settings are also
available over D-Bus as `GetSection` / `SetSection` on `uqd`. Every write method there
(`SetSection`, `SetSetting`, `SetTheme`, `SetLayout`, `SetSchedule`) takes a last `origin` string
that the history records; `""` records as unknown.
Themes use `ListThemeInfo()` (JSON with names, which are yours, and any folders skipped),
`ExportTheme(id, with_look)` (the `.uqtheme` text), `ImportTheme(text, replace, with_look,
origin)` (returns the id; `Conflict` if it exists and `replace` is false), `DeleteTheme(id,
origin)` (JSON `{switched, profiles}`) and a `ThemesChanged` signal.
Profiles use `ListProfiles` (JSON), `GetProfile(id)` (the TOML text), `SetProfile(toml, replace)`
(returns the id; `Conflict` if it exists and `replace` is false), `DeleteProfile(id)`,
`ApplyProfile(id)` and `ProfileOff()` (both return a JSON report), and a `ProfileChanged(id)`
signal, sent when the profile that's on or the list of profiles changes (`""` for none).
Keyboard shortcuts use `GetSection("keybinds")` / `SetSection("keybinds", json, origin)` and the
`SectionChanged` signal. `ListBinds()` returns uQ's shortcuts with your changes, your own, the
marked `user.lua` binds and the key and action lists (JSON); `PauseShortcuts()` and
`ResumeShortcuts()` pause and resume them for key capture (at most 15 s).
Window rules use `GetSection("rules")` / `SetSection("rules", json, origin)`, like any section,
and so do startup apps (`"autostart"`), environment variables (`"env"`) and Apps and scaling
(`"compat"`); there is no separate write call for them. `ListAutostart()` returns each startup
app's status as JSON, `ShellReady()` is what the bridge calls once the shell has sent its first
request (so uqd knows when to start the login apps), and `AutostartChanged` is the signal sent
when a status changes.
Displays use `GetSection("monitors")` / `SetSection("monitors", json, origin)` for everything but
the layout (main display, bar, saved arrangements, workspace planner); the layout still changes
only through `ApplyMonitors`. `RememberMonitors(name, origin)` saves an arrangement for the
screens that are connected and returns its id. `ListOutputs()` also carries `roles` (the main
display and which screens get a bar) and `arrangement` (`id`, `name`, `applied`, `failed`).
Gestures and the swipe options are part of `GetSection("input")` / `SetSection("input", ...)`.
`PickWindow()` asks the shell to let you click a window and returns only that window's details
(JSON with a `status`), `PickDone(token, outcome)` is what the shell calls back with, `ListWindows()`
returns the open windows' details (no addresses or process ids), and `PickRequested(token)` is the
signal that tells the shell to start.

#### Displays

Besides the layout, the Displays page keeps **saved arrangements**. After you keep a change it
offers **Remember for these screens** (or **Update**, when the set already has one). An
arrangement is matched by the make, model and serial of every connected screen, never by port,
so swapping cables doesn't matter. There is one per set of screens. When screens are plugged in
or out, uQ switches to the arrangement for the new set within about 2 seconds, with no prompt
and no 15 second revert. **Switch arrangements when screens change** turns that off. A switch
isn't recorded in History; Remember, Forget and the other choices on the page are, and each can
be undone. An arrangement that can't be used leaves the layout alone and shows one
notification.

**Main display** is the screen where notifications, the volume and brightness pop-ups and
Spotlight open, where the session starts, and, when `xrandr` is installed (optional,
`xorg-xrandr`), the X11 primary. It does nothing until you choose one; until then those follow
the focused screen. Menus opened from a bar stay on that bar's screen. **Show the bar on** picks
all screens, the main one or the ones you tick; a bar always shows somewhere. **Workspaces
across screens** puts workspaces 1 to N on screens one after another, in turn, or by hand
(Choose for each), shows a preview, and **Move workspaces back when a screen returns** (on by
default) sends them home when the screen comes back. A rule of your own on the Rules page for
the same workspace wins, field by field. Undo in History puts back a whole list of arrangements
or plan at once.

#### Input

The Input page lists **touchpad gestures**: fingers (3 to 5), direction (sideways, up, down,
left, right, pinch in, pinch out), what it does, and an optional held key. uQ ships two:
three fingers sideways switches workspace, and four fingers up opens Spotlight. Turn either off
with its switch, or use **Back to uQ's gestures** to restore the list. Two gestures that use the
same fingers and key conflict when their directions are equal or one covers the other (sideways
covers left and right, up and down are covered by vertical), because Hyprland refuses the second
one; the editor says which two clash and won't save. The swipe options (distance, reverse, make a
new workspace past the last, cancel ratio) apply at once. A gesture in `user.lua` that overlaps
one of uQ's makes Hyprland refuse the whole config, so every later uQ change then fails with
Hyprland's own message; uQ never edits `user.lua`, and the page shows the line and offers to open
it.

#### Wallpaper

A wallpaper entry for **All displays** can use **Span across displays**: one image is drawn over
the screens at their real positions, and each screen shows its part. Screens with their own
entry are left out. A folder slideshow changes slide on every screen together.

#### Profiles page

A profile is a named set of settings you turn on together, such as Gaming or Presentation. It
holds only the settings you put in it, and only those change when it's on. It can hold settings
from Look and layouts, Appearance (including the theme, light or dark, and fonts), Shell and
Wallpaper, and one power mode (Power saver, Balanced or Performance). Only one profile is on at a
time. A profile can turn window rules and panel rules on and off (see the Window rules page
below) and keyboard shortcuts on and off (see the Keyboard shortcuts page below).

**New profile** starts from the settings you changed lately, or from the current settings of the
pages you tick; **Add settings** adds more later, each with the value it has now. The app can't
add a power mode; to set one, export the profile, add `[power]` and `profile = "performance"`
(or `"balanced"`, `"power-saver"`) to the file, and import it with `--replace`. You can remove it
in the app again. Editing a profile that's on takes effect the next time you turn it on.

Turning a profile on first saves a restore point named "Before the Gaming profile", then applies
the whole profile in one step. Rows on the other pages say "From the Gaming profile" while it
holds their value, and History lists the changes "from the Gaming profile". Power mode goes
through the system's power service; if there isn't one, or it refuses or doesn't answer within
3 seconds, everything else still applies and a notice says "Power mode wasn't changed".

**Back to normal** puts back each setting the profile changed, but only if it still has the value
the profile gave it. A setting you changed by hand since stays as you set it, and the message
names it, such as "Gaps between windows". Turning on a second profile while one is on does the
same, then applies the second in one step. While a profile that sets light or dark is on, the
light/dark schedule doesn't switch it; the schedule carries on after Back to normal. Restoring a
restore point ends the profile that's on and, like Back to normal, puts its power mode back. A
profile that's on stays on after uqd restarts.

A profile can also turn window and panel rules on or off, and nothing more: it can't add, change
or delete a rule. It names each rule by the id uQ gave it (`[rules]` then `"r-7f2c" = false`),
and those ids are different on every computer. A profile you bring from another computer usually
finds none of its rules, so it skips them and says "1 rule in this profile isn't on this computer,
so it was left out." (the lines stay in the file). **Back to normal** switches a rule back only if
it still has the value the profile gave it, so a switch you flipped by hand is kept and named.
Workspace rules have no on/off switch and can't be in a profile, and keyboard shortcuts still
can't be either.

A profile can also hold environment variables (set one, or remove it while the profile is on), the
five switches of Apps and scaling, a fix's **Run on** and **Size**, and the on/off switch and delay
of a startup app that already exists on the computer (see the Startup and Apps and scaling pages
below). It can't add a startup app or command, launch options, a fix's own variables or a launcher
file, and it never turns on "Also when started from other launchers". It never removes a fix that
also works in other launchers; those lines are left out with a notice. A startup app the profile
names but this computer doesn't have is left out the same way. Startup changes take effect at the
next login: nothing starts or stops when you turn a profile on, and apps that are already open
keep their environment.

Switch from the quick-settings **Profile** row (it shows only when you have profiles; **Normal**
is Back to normal), from the page, or with `uqctl profile`. To switch with a key, set **Switch to
the next profile** on the Keyboard shortcuts page.

A profile can also turn keyboard shortcuts on and off, and nothing more: it names each shortcut by
its id (`[keybinds]` then `"uq.close" = false`, or `"k-3f9c" = true` for one of your own). It
can't set keys, add a shortcut or carry a command, and import refuses anything else with a plain
message. A shortcut of yours that is not on this computer is left out with a notice. A profile that
would make two shortcuts share keys is refused as a whole; **Back to normal** leaves a shortcut off
if turning it on would now clash, and names the shortcut it clashes with.

**Export** and `uqctl profile export` write a TOML file with a header comment and `format = 1`
(`id`, `name`, optional `icon`, then the settings as tables). **Import** checks every setting
and refuses a file that is over 64 KiB, isn't text, has an unknown setting or a value out of
range, or has a `[keybinds]` entry that isn't a shortcut switch, with a plain
message and nothing saved. Turning on a profile whose theme has since been uninstalled is refused and changes nothing; if the theme you used before a profile has been uninstalled since, Back to normal uses uQ's default theme and says so. Importing saves no restore point, since no setting changes. Manual
checks are in [docs/notes/settings-slice-b-live-check.md](docs/notes/settings-slice-b-live-check.md).

#### Keyboard shortcuts page

Every shortcut uQ sets up is listed here by what it does, with its keys, grouped as Apps and
desktop, Windows, Workspaces, Sound and brightness, Media, and Profiles. The defaults:
Open a terminal (Super + Return), Close the window (Super + Q), Float or tile the window
(Super + V), Log out (Super + Shift + M), Open Spotlight (Super + Space), Open Spotlight for emoji (Super + period; it needs the shell's emoji shortcut, which comes with the emoji picker), Open the app menu
(tap Super), Open quick settings (Super + A), Open notifications (Super + N), Lock the screen
(Super + L), Open the power menu (Super + Escape), the three layouts (Super + Alt + D, M and S),
focus with Super + the arrows, workspaces 1 to 10 with Super + the number (Shift moves the
window), the scratchpad (Super + S, Shift moves), Super + the wheel for the next and previous
workspace, Super + left and right mouse buttons to move and resize, and the volume, microphone,
brightness and media keys. **Switch to the next profile** has no keys until you give it some.

**Change** a shortcut's keys, turn it **off**, **Reset** it to its default, or **Put all back**
for every change at once (your own shortcuts are kept). **Add a shortcut** makes one of your own,
with an action: a uQ panel, a window or workspace action, a layout, **Start an app** (the app
starts through `uqctl launch`, so its Apps and scaling fix applies) or **Run a command**. A command
runs in its own systemd scope, so it doesn't die with uqd and its quotes are kept as typed. Two
options, "Also over the lock screen" and "Repeat while held", are offered only for actions
where they make sense (volume and brightness, for example). The lock recovery shortcut
(Super + Ctrl + L) can be given other keys but can't be turned off.

Two shortcuts on the same keys are refused, naming both: the editor says so before you save, and
uqd refuses the same write over D-Bus. A shortcut in `user.lua` that you mark with a trailing
`-- uq:bind Name` comment is listed read-only, and a clash with it only warns. Unmarked binds in
`user.lua` are not shown, and a `hl.unbind` there shows as "Overridden in user.lua, line N".

**Press the keys…** in the shortcut editor listens for the new keys: while it waits, uQ's shortcuts are paused for at most
15 seconds, so Super + Space can be pressed without opening Spotlight. `uqctl keys list` prints
every shortcut and `uqctl keys reset <id>` or `--all` puts changes back. `base.lua` keeps a
fallback copy of the defaults, so the shortcuts still work if `generated.lua` is missing. Manual
checks are in [docs/notes/settings-slice4-live-check.md](docs/notes/settings-slice4-live-check.md).

#### Window rules page

A window rule says "when a window looks like this, do that". A rule can match a window by App ID,
window title, the App ID or title it had when it opened, its tag, whether it is an older X11 app,
floating, fullscreen or pinned, whether it is a dialog, and the workspace it is on. It can then
make it float or tile, go fullscreen or maximized, open on a workspace (quietly, if you like) or a
monitor, take a size or position, be centered or pinned, or not take focus when it opens; and set
opacity, blur, shadow, animation, rounding, border size, whether it keeps the screen awake, a
keep-aspect-ratio hint, and a tag. Effects that decide where or how a window opens only apply to
windows opened afterwards, so after saving the page says "Close and reopen Pavucontrol to apply."
and names the matching windows that are open. The rest apply at once.

**Pick a window** hides the app and lets you click any window; the editor then fills in its App ID
(Esc cancels, and the app comes back whatever happens). **Matches now** lists the open windows a
rule would catch, as you type; it is a close guide, not a promise, because Hyprland's matching can
differ a little for non-English text. **Why does this window…?** lets you pick a window and lists
the uQ rules that match it, in order. Five templates start a rule for you: float an app's
dialogs, open an app on a workspace, no blur for an app, picture-in-picture floats and stays on
top, and keep the screen awake while an app is fullscreen. **Panel rules** do the same for the
bar, notifications and launchers (blur, transparency cut-off, animation), and **Workspace rules**
set a monitor, gaps, border size, whether a workspace stays when empty, and whether it is the
default. They join each workspace's layout from the Look and layouts page, so each workspace still
becomes one rule for Hyprland, and changing a layout never touches these fields or the reverse.

Limits: 200 window rules, 50 panel rules, 64 workspace rules, names up to 100 characters and
patterns up to 256. Later rules win, as in Hyprland, so order matters. Patterns must match the
whole text; ones Hyprland can't use are refused with a plain message before anything is saved.
Every change goes through History, so **Undo** and **Restore** work. Undo puts back the whole list
the change was in (window rules, panel rules or workspace rules) as it was before, so undoing an
old change also undoes later changes to other rules in the same list, and the app asks first.
A profile can turn a window or panel rule on and off (see Profiles above).

uQ never writes `user.lua`. Each uQ rule is given the name `uq:<id>`, so a line in `user.lua`
can change or turn off one of them (the id is shown under "Advanced details" in the editor):

```lua
hl.window_rule({ name = "uq:r-7f2c", enabled = false })
```

The page then shows that rule as overridden in `user.lua`, with a button that opens the file. A
line that is only a `--` comment doesn't count. Rules written in `user.lua` itself aren't listed
on the page. Manual checks are in
[docs/notes/settings-slice-c-live-check.md](docs/notes/settings-slice-c-live-check.md).

#### Startup page

**Startup** (under System) lists the apps that start when you log in, and the environment
variables every app gets. It reads the XDG autostart folders: `~/.config/autostart` and each
`autostart` folder in `XDG_CONFIG_DIRS` (usually `/etc/xdg/autostart`); a file in your folder
replaces a system file with the same name. Each row says where the app came from and why it
won't start if it won't (hidden, not for this desktop, not installed, needs a terminal, or its
command can't be read).

uQ's login runner starts the apps once per session, after the shell is up (or 20 seconds after
uqd starts if it never hears from the shell). Each app starts in its own scope under `app.slice`
and its output goes to the journal under `uq-autostart-<id>`, so restarting uqd never stops an
app, and a uqd that restarts in the same session starts nothing new. A row's status is one of:
**Starts next time you log in** (turned on since this login), **Waiting** (with its delay),
**Running**, **Started, then finished**, **Failed** (with the exit code and the last lines of its
output), **Not started** (with the reason), **Off** or **Off in uQ**, or **Started earlier;
finished** for an app a previous uqd started that has since ended. A **Start delay** of up to 600
seconds makes an app wait after the shell is ready.

Turn an app off with its switch. For a system entry, uQ writes a copy in `~/.config/autostart`
with `Hidden=true` and `X-uQ-Autostart=true`; turning it on deletes that copy. uQ never edits a
file of your own there: turning one off is kept in uQ's settings only, and the row says "Off in
uQ". **Add an app or command** adds an installed app, or a command you type. A command is split
into its words, the way a desktop file's `Exec` is, and run directly, with no shell: no pipes,
no `$VAR`, no `&&`. At most 50 apps can be added.

**Environment variables** are set for every app uQ starts. A change reaches apps you start after
it; apps that are open keep theirs, and one that was removed stays set until you log out. uQ
writes them to `generated.lua` (Hyprland passes them on to the user's services) and to
`~/.config/environment.d/60-uq.conf` for the next login. A name uQ needs for the session
(`WAYLAND_DISPLAY`, `XDG_SESSION_*`, `XDG_CONFIG_HOME`, `HOME` and a few more) or sets itself
(the cursor and Qt theme names) is refused with a plain message; `PATH`, `LD_PRELOAD` and
a few other risky names are allowed with a warning. A value can't contain `$`, because Hyprland
and `environment.d` would read it differently. An `hl.env("NAME", ...)` line in `user.lua` wins, and
the row says "Overridden in user.lua, line N". The scale-derived lines of Apps and scaling are
not written to `environment.d`, since that file applies to every desktop of your account. Every
change goes through History. Undo puts back the whole list the change was in (all startup apps,
or all variables), as it was before, and the app asks first.

In `uq-dev session` the runner starts nothing and says "Not started in a test session". Manual
checks are in [docs/notes/settings-slice-d-live-check.md](docs/notes/settings-slice-d-live-check.md).

#### Apps and scaling page

Five switches, each applying to apps started after the change:

- **Sharp X11 apps** turns on Hyprland's `xwayland:force_zero_scaling` and sets `GDK_SCALE` (the
  main screen's scale, rounded to a whole number) for new apps. uQ doesn't set `QT_SCALE_FACTOR`
  for everyone, because it would also enlarge Qt apps that already run on Wayland.
- **Run Electron apps on Wayland**, **Run Chromium-based browsers on Wayland**, **Qt apps prefer
  Wayland** (Wayland when an app can, X11 when it can't) and **Steam UI scale**.

**Blurry right now** lists the open older X11 apps, the ones that get stretched on a scaled
screen, each with a **Fix this app** button. A fix sets an app's **Run on** (Automatic, Wayland
or X11), its **Size** (100 % to 300 % in steps of 25 %), extra variables and launch options. It
applies when the app is started from uQ's launcher, Spotlight, a pin, `uqctl launch <id>` or
the login runner (Startup), and not when something else starts it. **Also when started from
other launchers** writes a copy of the app's desktop file into `~/.local/share/applications` that carries the fix,
marked `X-uQ-Override=true`, so other launchers pick it up. It's refused if a file of yours is
already there, and removing the fix, or turning the switch off, deletes the copy. For Chromium,
Chrome, Brave, Vivaldi and Edge, uQ writes the browser's flags file in `~/.config` (such as
`chromium-flags.conf`) with a first line saying uQ manages it; a file without that line is left
alone and the page says so. A system change, such as an upgrade of an app's desktop file or a
newly installed browser, is picked up within a minute.

uQ never writes through a symlink, and never edits or deletes a file it didn't mark. The one
exception is `~/.config/environment.d/60-uq.conf`, which is uQ's by name: uQ replaces it
whatever it holds (a symlink there becomes a plain file). In `uq-dev session` the page refuses the
elsewhere switch ("Not available in a test session"). Manual checks are in
[docs/notes/settings-slice-d-live-check.md](docs/notes/settings-slice-d-live-check.md).

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

#### Your own themes

The **Theme** row on the Appearance page shows the themes that come with uQ and your own, tagged
**Yours**. Your themes live in `~/.local/share/uq/themes/user-<name>/` as `dark.toml` and
`light.toml`; the `user-` prefix means one can never replace a theme that comes with uQ. Theme
files are data only: uQ refuses a file over 64 KiB, names with special characters, values out of
range, text that is hard to read (4.5 to 1 contrast, in both versions) and a missing dark or
light version, and it never runs anything from one. **New theme from this one…** opens an editor
with seven colours per version (background, panels, text, accent, good, warning, bad); the rest
of the palette is worked out from them, and a version that is hard to read can't be saved.
**Export…** writes a `.uqtheme` file with the theme and its name, and with the box ticked your
window gaps, corners and effects. It never holds your wallpaper or your font and cursor picks.
**Import…** saves a restore point first, adds the theme and does not switch to it (**Use it now**
does). **Delete…** removes one of yours; if it is the active theme, uQ switches to its default
theme first, and the profiles that use it are named. A folder you copy into the themes folder
appears within a minute, or when you open the Appearance page. In a `uq-dev` session, import and
delete are not available. `uqctl theme list --long`, `uqctl theme import <file> [--replace]
[--with-look]`, `uqctl theme export <id> [file]` and `uqctl theme delete <id>` do the same from
a terminal. Manual checks are in
[docs/notes/settings-slice-g-live-check.md](docs/notes/settings-slice-g-live-check.md).

### Launchers

- `SUPER+Space` opens or closes Spotlight: apps, a calculator (`=2+2`), a command runner
  (`>cmd`) and any installed search providers.
- `SUPER+.` opens Spotlight at `:`, the emoji search. It needs an emoji provider (emobie);
  without one Spotlight says "No emoji provider installed".
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

To use the installed uQ and work on a checkout on the same machine (a nested dev desktop,
side-by-side app runs, and installing a dev build with a rollback), see
[docs/developing.md](docs/developing.md) and `uq-dev --help`.

## Credits

- `crates/uqctl/protocols/hyprland-lock-notify-v1.xml` is vendored unchanged from
  [hyprwm/hyprland-protocols](https://github.com/hyprwm/hyprland-protocols),
  Copyright © 2025 Maximilian Seidler, under the BSD 3-Clause License (its full text is in the file).
- Theme credits are in [`themes/CREDITS.md`](themes/CREDITS.md).
