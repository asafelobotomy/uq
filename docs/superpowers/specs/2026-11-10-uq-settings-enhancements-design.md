# uQ Settings enhancements design

Date: 2026-11-10 · Status: approved. The user chose all ten suggestions from a research pass and
confirmed every decision in §11 on 2026-11-10.
Parent spec: `2026-11-08-uq-phase3-settings-design.md` (the app, domains, plugin pages, slices).
Related: `2026-11-10-uq-phase4-integrations-design.md` (draft, uq#5), which already plans the
companion-app pages that §8 moves earlier.

## 1. Intent

Phase 3 builds a settings app that covers what other desktops already do well: displays with
auto-revert, input, keybinds, sound, network, Bluetooth, users and plugin pages. This spec adds
the things Phase 3 does not plan. They come from what users of other settings apps most often ask
for, or find broken.

**Research behind it (2026-11-10)**

| Source | What it showed |
|---|---|
| HyprMod (GTK4 Hyprland settings app), open issues | Top bugs: "changes don't apply to main config" (#81) and "window rules having no effect" (#79). Top requests: gestures (#61) and more settings (#15). Users like its Ctrl+Z undo, profiles and pending-changes page; backups with a history browser are next on its roadmap. |
| hyprmoncfg (monitor TUI) | Monitor profiles matched to the connected hardware, a hotplug daemon, a workspace planner (sequential, interleave, manual) and a safe apply with a confirm timer. Its stated motivation: "no visual feedback until after you've committed to a config". |
| TheActualWorld/hyprsettings (Quickshell) | Monitor canvas, keybind editor, autostart and env management. |
| COSMIC reviews and changelogs (2026) | Complaints: apps pick the wrong "main" monitor, no wallpaper spanning, autologin. Praise: themes that can be exported and shared. A settings bug where the slideshow interval reset itself. |
| KDE System Settings (Plasma 6 feedback) | Internal names like "KWin" in the UI, the window not remembering its maximised state, too many clicks, and settings that silently don't take effect (SDDM cursor). |
| Framework "State of HiDPI on Linux" | Blurry Electron, Chromium and Steam apps under fractional scaling, and hand-edited `.desktop` files and flags to fix them. |

**The ten features**, in priority order:

1. **Where's this value from?** Each setting shows where its live value comes from and warns when
   something else overrides it (§2).
2. **Undo and history.** Ctrl+Z, a list of this session's changes, and restore points (§3).
3. **Profiles.** Named sets of settings, plus monitor profiles that switch on their own (§4).
4. **Window and workspace rules editor**, with a window picker and a live match preview (§5).
5. **Companion app pages** for scdr, crumBS, noti-fi, XZram and scale_tail, moved earlier (§8).
6. **Apps and scaling page.** XWayland scaling, Wayland for Electron/Chromium, per-app fixes (§6).
7. **Multi-monitor extras.** Primary monitor, spanned wallpaper, workspaces per monitor (§7.1).
8. **Touchpad gestures** (§7.2).
9. **Autostart and environment variables** (§7.3).
10. **Theme export, import and custom themes** (§7.4).

Plus quality rules every page follows (§9).

**Success criteria**

1. If `user.lua` sets `general.gaps_in`, the Look page's gaps row says it's overridden and names
   the line in `user.lua`. Removing that line clears the warning after the next reload, with no
   restart.
2. Ctrl+Z in the app undoes the last change, live. Restoring yesterday's restore point brings
   back every uQ setting as it was, in one step, and that restore can itself be undone.
3. Plugging in the desk monitors applies the "Desk" monitor profile within 2 s, with no prompt.
   Unplugging returns to the laptop layout.
4. A rule made by clicking a window, with "Float" checked, floats that app the next time it
   opens. The editor showed the window as a match before it was saved.
5. An Electron app installed from the repos runs natively on Wayland and is sharp at 125 % scale
   after one switch on the Apps and scaling page.
6. Apps in `~/.config/autostart` start at login, and each can be turned off from the app.
7. A theme exported on one machine imports on another and appears in the theme list, with no
   relog.
8. None of the above writes to `user.lua`, ever.

**Non-goals**

- Editing `user.lua`. It stays the user's file. uQ only reads it, to explain overrides.
- Settings sync between machines. Export and import (§4, §7.4) cover moving things by hand.
- A sandbox for imported themes or profiles. They are data files, validated like any other
  write, and they carry no code.
- Rules, gestures or profiles for other compositors.

## 2. Where's this value from? (provenance)

The problem: uQ renders `generated.lua`, but `user.lua` loads after it and wins. A value can also
change at runtime (`hyprctl keyword`, a plugin). Today the app shows uQ's stored value and has no
way to tell the user it isn't the one in effect. This is exactly HyprMod's top bug report.

**Sources.** Each setting row shows one of these, as a small label under the control:

| Source | Meaning | Shown as |
|---|---|---|
| `theme` | Not set; the theme's token is in effect (`null` in the domain) | "From the theme" (muted) |
| `uq` | Set in uQ and live | nothing (the normal case) |
| `profile` | Set by the active profile (§4) | "From the Gaming profile" |
| `user_lua` | `user.lua` sets a different value | warning: "Overridden in user.lua, line 42" with **Open user.lua** |
| `runtime` | Live value differs and `user.lua` doesn't set it | warning: "Changed while running (not by uQ)" with **Re-apply** |
| `error` | The last render failed verification, so the old value is live | error: Hyprland's message in plain text |

**How uqd works it out.** A new method, `Explain(s domain) → s json`, returns
`{key: {source, live, stored, detail}}` for every key in the domain.

- **Hyprland options.** Each key that renders to a Hyprland option has a fixed option path in
  `uq_core::settings` (for example `look.gaps_in` → `general:gaps_in`). uqd reads the live value
  with `getoption` on the request socket it already uses (`hypr.rs`), and compares it with the
  value it rendered.
- **Finding the line.** On a mismatch, uqd scans `user.lua` as plain text for the option path in
  either form (`general.gaps_in`, `gaps_in =` inside a `general` table, `hl.config` calls). The
  first hit gives the line number. This is a hint, not a parser: if nothing matches, the source
  is `runtime`. `user.lua` is read only, never executed or written by uqd.
- **Non-Hyprland keys.** Appearance keys applied through gsettings, Kvantum or qt6ct compare with
  the live gsettings value or file. A mismatch there is `runtime` ("changed by another app").
- **When.** Only when a page opens, after `SectionChanged`, and after Hyprland's
  `configreloaded` event. It never polls.
- **Re-apply** runs the domain's normal render path again. It can't beat `user.lua`, so it isn't
  offered for `user_lua`.

**Keybinds and rules** reuse this. A uQ bind or rule that `user.lua` unbinds or replaces shows as
overridden, using the `-- uq:bind` / `-- uq:rule` markers Phase 3 §2.4 already defines.

**Open user.lua** opens the file in the user's text editor at the line, through the default-apps
`text/plain` handler. If the handler doesn't take a line number, the file opens at the top and
the label keeps the number.

## 3. Undo and history

uQ applies changes live, so it needs no pending-changes queue like HyprMod's. What users want
from that queue is to see what they changed and take it back. uQ does that with a history journal
in uqd.

**Journal.** Each successful write that changes `state.toml` appends an entry:
`{id, time, origin, domain, summary, before, after}`.

- `origin` is `settings`, `uqctl`, `shell`, `profile:<id>`, `restore` or `import`. Writes come
  from all of these, so uqd records it, not the app. Callers pass it as a new optional argument
  on the write methods; old callers record as `unknown`.
- `before` and `after` hold only the changed domain or key, not the whole file.
- `summary` is a plain-text line built by uqd ("Gaps inside: 5 → 8").
- Files live in `~/.local/state/uq/history/` as one JSON line per entry, rotated daily. uqd keeps
  30 days or 500 entries, whichever is smaller.
- Launch counts, the theme schedule's own automatic flips, and unconfirmed monitor applies are
  never recorded. A confirmed monitor change is. Neither is a hotplug arrangement switch (slice F);
  the user's own Remember, Forget and role changes are.

**Restore points** are full copies of `state.toml` in `~/.local/state/uq/restore/<id>.toml`, plus
a name and time.

- **Automatic points:** once a day on the first change, before a profile switch, before an
  import (§7.4), and when uqd starts on a new uQ version.
- **Manual points:** "Save a restore point" with a name. These are never rotated away.
- Up to 20 automatic points are kept.

**D-Bus additions:**

| Method | Does |
|---|---|
| `ListHistory(u limit) → s` | newest first |
| `Undo(s entry_id)` | writes that entry's `before` back, validated like any write, recorded with origin `restore` |
| `ListRestorePoints() → s`, `SaveRestorePoint(s name) → s id`, `DeleteRestorePoint(s id)` | |
| `Restore(s id)` | validates the whole file as one write, saves an automatic point first, then applies every domain through the usual render and verify path; on a verify failure nothing changes |
| Signal `HistoryChanged()` | |

An undo whose domain has changed again since that entry asks first: "Gaps were changed again
after this. Undo anyway?"

**In the app:**

- **Ctrl+Z / Ctrl+Shift+Z** undo and redo this window's own changes in this session. Redo is
  kept in the app, as a stack of entry ids.
- **"Changed this session"** is a button in the header that opens a panel listing this session's
  changes from any origin, each with Undo.
- **History** is a page under the uQ category. It shows a timeline grouped by day with a filter
  by page, and the restore points with Restore and Delete.
- `uqctl history [--limit N]`, `uqctl undo [<id>]` and `uqctl restore <id>` do the same from a
  terminal. These matter when the desktop is broken badly enough that the app can't open.

## 4. Profiles

**Model.** A profile is a named overlay: a subset of keys from uqd's domains, not a full copy.
Only the keys it holds change when it applies.

```toml
# ~/.local/state/uq/profiles/gaming.toml
id = "gaming"
name = "Gaming"
icon = "input-gaming"

[look]
animations = false
blur = false
gaps_in = 0
gaps_out = 0

[shell]
islands.clock = false

[power]
profile = "performance"   # power-profiles-daemon; the only non-uqd key in v1
```

- **What a profile can hold:** `look`, `layouts`, `appearance` (theme, variant, fonts), `shell`,
  `wallpaper`, `keybinds` (turn a named shortcut on or off by its id, nothing else: no keys, no new shortcut, no command; slice 4, R15), `rules` (enable or disable named
  rules only), `env` (set a variable, or remove it while the profile is on), `compat` (the five
  switches, and a fix's Run on and Size) and `autostart` (an existing entry's on/off and delay),
  plus `power.profile`. A profile never adds a startup entry or command, launch options or a
  launcher file; startup changes take effect at the next login.
- **Applying.** uqd saves an automatic restore point, remembers the values of the keys the
  profile touches, then applies the overlay as one validated write with origin
  `profile:<id>`. At most one profile is active.
- **Turning off** ("Back to normal") writes the remembered values back. A key the user changed by
  hand while the profile was on keeps the hand-made value, and the app says which keys did.
- **Editing.** The app's Profiles page creates a profile from "what I've changed since…" (a
  history range) or from the current values of chosen pages, then lets you add and remove keys.
- **Switching.** From the Profiles page, a quick-settings tile (a menu of profiles),
  `uqctl profile list|apply <id>|off|next`, and an optional keybind (`uq.profile.next`).
- **Export and import** use the same TOML with a header comment and `format = 1`. Import validates
  every key, refuses unknown domains, and never imports `keybinds` that add commands.

**Monitor profiles** are separate because they switch on their own.

- **Saving.** After a monitor change is confirmed (Phase 3 §2.3), the Displays page offers
  "Remember for this set of screens".
- **Matching.** A monitor profile is keyed by the *set* of connected outputs, each identified by
  make, model and serial from `monitors all -j`. It is not keyed by connector name, so using a
  different port still matches. It stores the `monitors` domain plus optional workspace-to-monitor
  assignments (§7.1).
- **Hotplug.** On Hyprland's `monitoradded` / `monitorremoved` events, debounced 1 s, uqd looks
  for an exact set match. If it finds one, it applies the profile through the monitor render path
  with no 15 s revert, since the layout was confirmed when it was saved. If there is no match,
  nothing changes, and the Displays page offers to arrange and remember.
- **Bad profiles.** A profile that fails to verify is skipped, and uqd sends one notification.
- Turning automatic switching off is a Displays page switch (`monitors.auto_profiles`).

## 5. Window and workspace rules editor

There is no good GUI for Hyprland rules anywhere, and broken rules are HyprMod's second most
common bug report.

**New domain `rules`:**

```toml
[[rules.window]]
id = "r-7f2c"                 # stable, made by uqd
name = "Pavucontrol floats"   # plain text, ≤ 100 chars
enabled = true
match = { class = "^(org\\.pulseaudio\\.pavucontrol)$" }
effects = { float = true, size = [800, 600], center = true }

[[rules.layer]]
id = "r-0a91"
name = "Blur notifications"
enabled = true
match = { namespace = "^noti-fi$" }
effects = { blur = true, ignore_alpha = 0.3 }

[[rules.workspace]]           # extends the Phase 1 per-workspace layout rules
workspace = "3"
monitor = "DP-1"              # optional
default = false
persistent = true
gaps_in = 0                   # optional overrides
```

- **Match fields (window):** `class`, `title`, `initial_class`, `initial_title`, `tag` (all
  regex), and `xwayland`, `floating`, `fullscreen`, `pinned` (bool), `workspace`. At least one
  field is required.
- **Effects (window):** a closed set. Static effects: `float`, `tile`, `fullscreen`, `maximize`,
  `workspace` (with `silent`), `monitor`, `size`, `move`, `center`, `pin`, `no_initial_focus`.
  Dynamic effects: `opacity`, `no_blur`, `no_shadow`, `no_anim`, `rounding`, `border_size`,
  `idle_inhibit`, `keep_aspect_ratio`, `tag`. The exact Lua calls and names come from the pinned
  Hyprland source at plan time, as Phase 3 does for key names.
- **Effects (layer):** `blur`, `ignore_alpha`, `no_anim`, `animation`.
- **Rendering and validation.** Rules render into a `rules` section of `generated.lua` in list
  order, so later rules win as in Hyprland. All strings go through `lua_string`. Each regex is
  test-compiled, then the whole file goes through the usual verify step. Up to 200 window rules
  and 50 layer rules.

**Editor (a "Window rules" page under uQ):**

- **Pick a window.** The app hides itself and asks the shell, through uqd and the bridge, to enter
  pick mode. The shell shows a full-screen overlay; the user clicks a window or presses Esc. The
  shell returns that window's `class`, `title`, `initial_class`, `initial_title` and `xwayland`
  from `clients -j`, and the editor pre-fills the match with the class as an exact regex.
- **Live preview.** "Matches now" lists open windows that match while you type, using `clients
  -j` and the same regex engine family. A rule with no matches says so but can still be saved.
- **"Why does this window…?"** Pick a window to list every uQ rule that matches it, in order,
  with overrides from `user.lua` flagged (§2).
- **When rules apply.** Dynamic effects apply to open windows at once. Static effects apply when
  a window opens, so the editor offers "Close and reopen to apply" with the window's name.
- **Common rules** are offered as one-click templates: float dialogs from an app, open on
  workspace N, no blur for an app, make picture-in-picture float and pin, inhibit idle while an
  app is fullscreen.

## 6. Apps and scaling page

The most common HiDPI complaint on Linux is blurry XWayland, Electron and Steam apps at
fractional scale, which users fix by hand with env vars and edited `.desktop` files. This page
puts those fixes in one place. It lives under Devices, after Displays.

**Global switches** (new domain `compat`):

| Switch | Applied as |
|---|---|
| Sharp X11 apps (XWayland apps draw at native size) | `xwayland.force_zero_scaling = true`, plus `GDK_SCALE` and `QT_SCALE_FACTOR` set from the main monitor's scale for X11 apps (§7.3 env path) |
| Run Electron apps on Wayland | `ELECTRON_OZONE_PLATFORM_HINT=auto` (Electron 28 and newer) |
| Run Chromium-based browsers on Wayland | adds `--ozone-platform-hint=auto` to `~/.config/chromium-flags.conf` and the matching files for the browsers found installed; only files uQ created are ever changed |
| Qt apps prefer Wayland | `QT_QPA_PLATFORM=wayland;xcb` |
| Steam UI scale | `STEAM_FORCE_DESKTOPUI_SCALING` from the main monitor's scale |

Each switch says what it does in one plain sentence, and that it applies to apps started after
the change.

**"Blurry right now".** The page lists open windows that run under XWayland (from `clients -j`),
each with its app name, and a **Fix this app** button that opens the per-app panel.

**Per-app overrides** (`compat.apps.<desktop_id>`): run on Wayland or X11, a scale factor, extra
env vars, and extra arguments.

- **Applying.** They apply at launch through uQ's own launch path: the shell, Spotlight, pins and
  `launch` keybinds all use it.
- **Other launchers.** For apps started any other way, the user can turn on "Also when started
  elsewhere". uqd then writes a desktop-file override,
  `~/.local/share/applications/<id>.desktop`, with `X-uQ-Override=true` and the original `Exec`
  wrapped in `env …`. uqd regenerates it when the system file changes, and deletes it when the
  override is removed. uqd never touches a desktop file in that folder that doesn't carry its own
  marker.
- **Validation.** Env var names must match `[A-Z_][A-Z0-9_]*`, and the §7.3 blocklist applies.
  Arguments are stored as a list and never pass through a shell.

## 7. Smaller features

### 7.1 Multi-monitor extras (on the Displays page, after Phase 3 slice 3)

- **Primary monitor.** Hyprland has no primary output, which is why apps "pick the wrong main
  monitor". uQ's primary is where the shell puts notifications, OSDs and the launcher, and where
  new workspaces open. For X11 apps, uqd also runs `xrandr --output <name> --primary` through
  XWayland whenever the primary or the layout changes, if `xrandr` is installed. It is stored as
  `monitors.primary`, matched by make, model and serial.
- **Spanned wallpaper.** A new wallpaper fit, `span`, draws one image across all outputs using
  their layout positions. Each monitor's background layer draws its own crop.
- **Workspaces per monitor.** A planner with three strategies, taken from hyprmoncfg: sequential
  (1–5 on the first screen, 6–10 on the next), interleave (odd on one, even on the other) and
  manual. The plan is stored in `monitors.workspaces` and merged into the workspace rules at render
  (slice F), so it never appears as editable rules. "Move workspaces back when a monitor returns"
  is on by default.
- **Bar per monitor.** The shell's bar can be on all monitors, the primary only, or a chosen set.

### 7.2 Touchpad gestures (on the Input page, after Phase 3 slice 3)

- **Model.** A list of gestures: `{fingers: 3..5, direction, action, modifier?}`, where direction
  is one of `horizontal`, `vertical`, `left`, `right`, `up`, `down`, `pinch_in`, `pinch_out`. The
  action comes from a closed set: switch workspace, move window, resize window, toggle special
  workspace, close, fullscreen, float, or a shell action (`uq:spotlight`, `uq:quick-settings`). It
  renders through Hyprland's gesture config, with names checked against the pinned source.
- **Workspace swipe options:** distance, invert, create a new workspace past the last, and
  cancel ratio.
- **Conflicts.** Two gestures with the same fingers and modifier conflict when their directions are
  equal or one is the other's axis (`horizontal` covers `left` and `right`, `vertical` covers `up`
  and `down`), which is Hyprland's own rule; `pinch_in` and `pinch_out` render as Hyprland's
  `pinchin` and `pinchout`. The same rules as keybinds apply: the editor warns, and uqd rejects the save.
- **Defaults.** uQ ships 3-finger horizontal for switching workspace and 4-finger up for
  Spotlight, both of which can be turned off.

### 7.3 Autostart and environment variables (a "Startup" page under System)

**Autostart.** uQ doesn't run XDG autostart entries today, so apps that add themselves to
`~/.config/autostart` never start.

- **Running entries.** uqd runs them once the shell is ready. It reads `/etc/xdg/autostart` and
  `~/.config/autostart`, and honours `Hidden`, `OnlyShowIn` / `NotShowIn` against
  `XDG_CURRENT_DESKTOP=Hyprland:uQ`, `TryExec`, and `X-GNOME-Autostart-Delay` where present.
- **How they run.** Each entry runs in `systemd-run --user --scope --slice=app.slice`, so it is
  logged and scdr treats it as an app. Phase 3's `command` actions use the same path.
- **The page** lists every entry with its source (system or yours), a switch, its status (running,
  exited, failed, with the last journal lines on failure), and an optional start delay.
- **Turning off a system entry** writes a user copy with `Hidden=true`, which is the XDG way.
  Users can add an app (by desktop id) or a command.
- uqd runs them itself rather than systemd's `xdg-autostart-generator` (§11, decision 3).

**Environment variables** (new domain `env`):

- **Where they apply.** Each variable renders both as `hl.env(...)` in `generated.lua` and in
  `~/.config/environment.d/60-uq.conf`, so apps launched by Hyprland and systemd user services
  both see it.
- **Validation.** Names must match `[A-Z_][A-Z0-9_]*`. Values are plain strings with no newlines,
  ≤ 4 KiB each, and 100 variables at most.
- **Blocked names**, because they break the session: `WAYLAND_DISPLAY`, `DISPLAY`,
  `XDG_RUNTIME_DIR`, `XDG_SESSION_*`, `HYPRLAND_*`, `DBUS_SESSION_BUS_ADDRESS`. `LD_PRELOAD`,
  `LD_LIBRARY_PATH` and `PATH` are allowed with a warning.
- **The page** says that changes take effect for apps started after the change, and fully after
  logging out. The §6 switches are listed here read-only, with a link back to that page.

### 7.4 Theme export, import and custom themes

Themes load only from `$UQ_DATA_DIR/themes` (`/usr/share/uq/themes`) today.

- **User themes.** uqd also loads `~/.local/share/uq/themes/<id>/{dark,light}.toml`. User theme
  ids are prefixed `user-`, so they can never shadow a system theme. They go through the same
  `ThemeRegistry` validation and the contrast checks in `contrast.rs`.
- **New theme from the current one** (on the Appearance page). The user tweaks a short list of
  base colours (background, surface, text, accent, plus good, warn and bad) for each variant. The
  editor shows the contrast check live and blocks saving a variant that fails it.
- **Export** writes one `.uqtheme` file: TOML with `format = 1`, the id, name, author, both
  variants, and optionally the current `look` keys. Wallpaper images are never included.
- **Import** accepts a `.uqtheme` from a file dialog or `uqctl theme import <file>`. It validates
  the file, saves a restore point, and installs it as a user theme, which shows up in the theme
  list at once through inotify. Files over 64 KiB are refused, as is anything outside the schema.
  Name text goes through `UqLabel`.
- **Delete** removes a user theme. If it's the active theme, uQ switches to the default first.
- **As built (slice G).** A theme's folder is named by its full id (`user-<slug>`). There is no
  inotify: uqd reloads right after its own import, save and delete, and finds a folder copied in
  by hand within 60 s or when `ListThemeInfo` is called (deviation). Editing one of the user's
  own themes is allowed and saves through `ImportTheme`. The window look of an import is
  journalled with the caller's origin, not `import`.

## 8. Companion app pages, moved earlier

Phase 4 (uq#5) plans pages for these apps, one slice per repo in its §10 order. The user wants
them sooner. This spec proposes:

- **Plugin page path first.** The plugin-page loader landed in Phase 3 slice 1. Phase 3 slice 6
  (SDK, docs, template) and Phase 4's `command` helper (§2.3, for CLI-first repos) move ahead of
  Phase 3 slices 3–5. That makes slice E below possible as soon as slice A lands.
- **Five pages first**, in this order, ahead of the rest of Phase 4:

| Page | Repo | Backend | Page contents (from Phase 4 §3) |
|---|---|---|---|
| Scheduling | scdr | `org.scdr.Daemon1`, `org.scdr.Session1` | daemon status, profiles, per-app rules, sched_ext choice, focus agent on/off |
| Storage and cleanup | crumBS | `crumbs` CLI through `command` (it has no D-Bus API) | disk use, caches, orphans, uninstall; never touches appOFholding's AppImages |
| Notifications | noti-fi | noti-fi's own interface | do-not-disturb schedule, per-app rules, history length |
| Memory and swap | XZram | `xzramd` D-Bus | zram size, algorithm, swappiness |
| Tailscale | scale_tail | `tailscale` CLI through `command` | up/down, exit node, accept routes, account |

- **Where files live.** Each page ships from its own repo's `uq/` folder, as Phase 4 §2.6
  describes. Nothing here changes Phase 4's manifest or security rules. Only the order changes.
- **Sources and history.** These pages' values live in the companion apps, not in uqd, so §2
  sources and §3 history don't cover them. The pages use the same row and label components, so
  they look and behave like uQ's own pages. `page.toml` gains no new keys.

**Implemented by slice E** (plan `2026-11-11-uq-settings-slice-e-companion-pages.md`). All five pages
ship from their apps' own `uq/` folders: Scheduling (scdr), Storage and cleanup (crumBS),
Notifications (noti-fi), Memory and swap (XZram) and Tailscale (scale_tail). Deviations from the
proposal above:

1. Writes. scdr writes over D-Bus (`SetSettings`, `SetRoleEnabled`, `Pause`, `Resume`), with the
   confirmation in the page; no `scdrctl settings`. XZram writes over D-Bus too (`ApplyNowZram`,
   `ApplyNowSysctl`). crumBS and Tailscale go through their CLIs, as proposed.
2. noti-fi gained two CLI calls for the page: `notify settings --json` and `notify set`.
3. The noti-fi page has no do-not-disturb schedule, because noti-fi has none. It offers the
   automatic do-not-disturb it has (full screen, screen sharing) and focus modes.
4. Per-app rules on the noti-fi page are read-only; editing them stays in noti-fi.
5. Pages poll (every 5 s while visible) instead of following signals, apart from the watches a
   manifest can declare.
6. scdr: no "try a scheduler" and no per-power-profile scheduler choice on the page.
7. Page ids are the app names (`scdr`, `crumbs`, `noti-fi`, `xzram`, `scale-tail`), not the page
   titles in the table.
8. XZram: no swap-file management on the page (it stays in XZram, with a link to open it).
9. Tailscale: no automatic connect on the page, the sign-in link is shown as selectable text with
   a Copy button, and a refused write tells the user to run `sudo tailscale set --operator=$USER`
   once.

## 9. Quality rules for every page

These come from KDE and COSMIC complaints, and a lint or test checks each one where it can.

- **Plain names.** No internal names in labels or descriptions: `uqd`, `generated.lua`,
  `Hyprland option paths`, `Kvantum` and `qt6ct` (except on the Appearance page's Qt row), or
  D-Bus names. Raw names may appear only in an "Advanced details" disclosure. A word list in
  `scripts/lint-settings.sh` checks the QML strings.
- **Two clicks.** Every setting is reachable within two clicks of opening the app, or one search.
  The search index test fails if a declared setting has no keywords.
- **The window remembers** its size, maximised state and last page, in
  `~/.local/state/uq/settings-app.toml`. `ShowPage` with a page still wins.
- **No silent failure.** A control whose write fails returns to its last good value *and* shows
  why (§2 `error` source). It never just snaps back.
- **No settings that reset themselves** (COSMIC's slideshow interval bug). Each page model test
  saves every key, re-opens the model, and checks it round-trips.
- **Everything works from a keyboard**, as Phase 3 already requires.

## 10. Architecture summary

| Piece | Change |
|---|---|
| `uq_core::settings` | new domains `rules`, `compat`, `env`, `profiles` (index), `monitors.primary` and `monitors.auto_profiles`; option-path table for §2 |
| `uq_core::history` (new) | journal, restore points, rotation |
| `uq_core::profile` (new) | overlay model, apply and turn-off logic, monitor-set matching |
| `uqd` | `Explain`, history and restore methods, profile methods, hotplug matcher, autostart runner, pick-window request, `xrandr` primary |
| `uqctl` | `history`, `undo`, `restore`, `profile …`, `theme import|export` |
| `uqctl bridge` / shell | pick-window overlay, span wallpaper, bar per monitor, profile tile, origin `shell` on writes |
| `uq-settings` | source labels on every row; Changed-this-session panel; pages: History, Profiles, Window rules, Apps and scaling, Startup; additions to Displays, Input and Appearance |

**D-Bus summary** (all on `org.uq.Desktop1`): `Explain`, `ListHistory`, `Undo`,
`ListRestorePoints`, `SaveRestorePoint`, `DeleteRestorePoint`, `Restore`, `ListProfiles`,
`GetProfile`, `SetProfile`, `DeleteProfile`, `ApplyProfile`, `ProfileOff`, `RememberMonitors`, `PickWindow → s`,
`ListAutostart`, `ShellReady`, `ListThemeInfo`, `ExportTheme`, `ImportTheme`, `DeleteTheme`; `ListOutputs` carries `roles` (main display and bar screens) and `arrangement`; the `autostart`, `env` and `compat` sections are written with
`SetSection("autostart" | "env" | "compat", json, origin)`. Signals: `HistoryChanged`, `ProfileChanged(s id)`,
`AutostartChanged`, `ThemesChanged`. `uqctl theme` gains `import`, `export`, `delete` and `list --long`.

**Security.**

- `user.lua` is never written.
- The desktop-file overrides (§6) and Chromium flag files are written only when uQ created them,
  identified by marker.
- Imports (themes, profiles) are data-only, size-capped, schema-checked and validated per key.
  They can't add `command` binds or autostart commands.
- Pick mode returns only window metadata the user clicked.
- Autostart runs only entries from the two XDG folders and the user's own additions, each in its
  own scope.

**Testing.**

- **Unit tests:** the option-path table and the `user.lua` line finder, with fixtures for each
  config style; journal rotation; undo when there's a newer change; restore atomicity on a
  verify failure; profile apply and off, including keys changed by hand meanwhile; monitor-set
  matching across port swaps; rule rendering golden tests; regex rejection; env blocklist;
  `.uqtheme` and profile import fuzzing for size and schema.
- **Integration tests:** the existing fake-Hyprland socket gains `getoption`, `clients -j` and
  monitor events.
- **Offscreen QML tests:** source labels, the Changed-this-session panel, the rules editor's live
  preview, and the gesture conflict warning.
- **Live checklist per slice:** the success criteria in §1.

## 11. Decisions (confirmed by the user, 2026-11-10)

1. **Slices go between Phase 3 slices,** as in §12, not after Phase 3 finishes. A and B are
   foundations that every later page uses, so they come straight after slice 2.
2. **Profiles are overlays, not full snapshots** (§4). Snapshots are what restore points are for.
3. **Autostart is run by uqd** (§7.3), not by systemd's `xdg-autostart-generator`. The generator
   needs `xdg-desktop-autostart.target`, which Hyprland doesn't reach. uqd's runner also gives
   per-entry status and delays. Rejected alternative: start that target from `base.lua` and live
   without status.
4. **Per-app overrides use uQ's launch path first** and write desktop-file overrides only when
   asked (§6). Writing them for every app would put files in the user's folder they didn't ask
   for.
5. **Companion pages move ahead of Phase 4's order** (§8): scdr, crumBS, noti-fi, XZram,
   scale_tail. This needs the Phase 4 `command` helper early.
6. **History keeps 30 days / 500 entries** and 20 automatic restore points.

## 12. Slices (one plan each)

| Slice | Contents | Needs |
|---|---|---|
| A. Trust | §2 sources, §3 undo/history/restore, §9 quality rules and lint, window state | Phase 3 slice 2 merged |
| B. Profiles | §4 profiles, quick-settings tile, `uqctl profile` | A |
| C. Rules | §5 rules domain, editor, pick mode, "why does this window…", §4 `rules` key (on/off by rule id) | A, B |
| D. Startup and apps | §7.3 autostart and env, §6 Apps and scaling | A |
| E. Companion pages | §8, Phase 3 slice 6 SDK and Phase 4 `command` helper pulled forward, then one sub-slice per repo | A; lands in each companion repo too |
| F. Monitors and input extras | §4 monitor profiles, §7.1, §7.2 | Phase 3 slice 3, B |
| G. Themes | §7.4 | A |

C, D, E and G can run in parallel after A. F waits for Phase 3's Displays and Input pages.
