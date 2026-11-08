# uQ Phase 2 — shell design

Date: 2026-11-08 · Status: approved in brainstorming, awaiting spec review
Parent spec: `2026-11-08-uq-desktop-design.md` (§2.3 shell, §2.4 extension points, §7 phase 2)

## 1. Intent

Phase 2 gives uQ its own shell, built with Quickshell: a bar, an app menu, a Spotlight-style
search, quick settings, on-screen displays, a notification surface, a lock screen, idle handling
and a power menu. Like everything in uQ, it's themed live from the active theme.

**What the user decided**

- **Bar:** floating islands along the top of each monitor, styled with the theme's radius and gaps.
  Five islands, in order:
  1. **System:** app-menu button, workspaces (each with a layout icon), and a merged dock. Pinned
     apps come first; running apps get a dot; unpinned running apps follow.
  2. **Work:** the focused window's icon and title, a dropdown of window actions, and minimise,
     maximise and close buttons.
  3. **Time and weather:** clock and date (calendar popover on click), current weather.
  4. **Notifications:** always shown. It shows whichever notification daemon the user chose
     (noti-fi, uQ's built-in server, swaync, mako, dunst).
  5. **Status:** tray, media, network and VPN with location, audio, battery, quick-settings
     button. There's no bell here: the Notifications island covers it.
- **App-menu button:** a full app menu laid out like KDE's launcher. Avatar and search at the top,
  categories on the left, a favourites grid on the right, and lock, log out, suspend, restart and
  shut down at the bottom. Its search covers apps only.
- **Super+Space:** a Spotlight-style search box centred on screen. It shows a top hit, then results
  grouped by source (apps, launcher providers, calculator, run-command), with the prefixes `=`
  (maths), `>` (command) and `:` (emoji). External providers appear only here.
- **Quick settings:** audio and brightness; network and Bluetooth; theme, dark/light, schedule and
  the current workspace's layout; power profile, battery, Do Not Disturb and Stay awake (idle
  inhibit).
- **On-screen displays:** volume and brightness OSDs are drawn by noti-fi when it's installed.
  uq-shell draws its own small OSD otherwise.
- **noti-fi** stays its own project and gains Hyprland support.
- **Weather:** Open-Meteo, for a city the user sets once.
- **VPN location:** the Tailscale exit node or the NetworkManager VPN name, plus the public IP's
  country.
- **Lock screen:** PAM password and fingerprint, whichever succeeds first.
- **Idle:** dim at 4 min, lock at 5, screens off at 6, suspend at 20 on battery only.
- **Shell-to-uqd link:** a long-lived `uqctl bridge` process speaking JSON lines.

**Assumptions**

- The greeter stays in Phase 5. Kvantum is still deferred (parent spec Task 4 ruling).
- Pinned apps, weather location, idle timings and launch counts are stored by uqd, so the Phase 3
  settings app can edit them.
- The public-IP lookup runs only when the network changes.
- Hyprland has no minimise, so minimise moves the window to a hidden `special:minimized`
  workspace.

**Success criteria**

- After logging into Hyprland, the bar appears on every monitor with all five islands. A theme
  change restyles it within a second, with no restart.
- Super+Space finds an app and launches it; `= 2+2` shows 4; a provider registered on the bus
  shows results.
- Volume and brightness keys show an OSD. Quick-settings toggles change the real setting.
- Locking works with a password, and with a fingerprint where there's a reader. Killing
  uq-shell while locked never exposes the session.
- Unplugging the network or stopping uqd degrades individual widgets; it never takes the bar
  down.

**Non-goals (Phase 2)**

- A greeter or login screen (Phase 5).
- Settings-app pages; uq-settings is Phase 3. Phase 2 values are set through `uqctl`.
- Integrating repos beyond what the bar needs. noti-fi's Hyprland mode and OSD are in scope;
  the other repos' launcher providers and widgets come in Phase 4. The provider API itself
  ships now.
- Clipboard history, file search and web search in the launcher.

## 2. Architecture

### 2.1 Processes

| Process | Started by | Role |
|---|---|---|
| `uqd` | Hyprland `hyprland.start` hook (Phase 1) | Desktop state; gains the settings in §2.4 |
| `uq-shell` | `uq-shell.service` (systemd user unit), restarted on `hyprland.start` and stopped on `hyprland.shutdown`, like uqd | Quickshell config at `/usr/share/uq/shell/shell.qml` |
| `uqctl bridge` | uq-shell, as a child `Process` | JSON-lines link to uqd, launcher providers, weather and IP lookups |

`uq-shell.service`: `ExecStart=/usr/bin/qs -p /usr/share/uq/shell`, `Restart=on-failure`,
`PartOf=graphical-session.target`. `base.lua` adds `systemctl --user restart uq-shell.service` to
the start hook and `stop` to the shutdown hook. It also sets
`misc = { allow_session_lock_restore = true }`.

### 2.2 Data sources

Hyprland state comes from `Quickshell.Hyprland`: workspaces, toplevels, focus, dispatch and raw
events. The rest comes from Quickshell service modules: `Pipewire` (audio), `UPower` (battery,
power profiles), `Mpris` (media), `SystemTray`, `Bluetooth`, `Notifications` (built-in
backend), `Pam` (lock), plus `IdleMonitor` and `IdleInhibitor`, and `DesktopEntries` (apps).
Networking uses `Quickshell.Networking` if Quickshell 0.3.1 has it, otherwise `nmcli` via
`Process`; the first plan checks which. Brightness uses `brightnessctl` via `Process`.

### 2.3 Bridge protocol (`uqctl bridge`)

One JSON object per line, in each direction, over stdin/stdout.

- **Request** (shell → bridge): `{"id": <u64>, "call": "<Name>", "args": [...]}`
- **Reply:** `{"id": <u64>, "ok": <value>}` or `{"id": <u64>, "err": "<message>"}`
- **Event** (bridge → shell, unsolicited): `{"event": "<Name>", "data": {...}}`

| Call | Args | Result |
|---|---|---|
| `GetTheme`, `SetTheme`, `ListThemes`, `GetLayouts`, `SetLayout`, `SetSchedule`, `Reload` | as `org.uq.Desktop1` | as `org.uq.Desktop1` |
| `GetSettings` | — | `{pinned:[desktop-id], weather_location:{name,lat,lon}\|null, idle:{dim,lock,screen_off,suspend_battery}, notif_backend:"auto"\|"notifi"\|"builtin"\|"swaync"\|"mako"\|"dunst"}` |
| `SetSetting` | key, JSON value | `null` |
| `Search` | query, generation u64 | `null`; results arrive as `SearchResults` events |
| `Activate` | provider, item id, action | `null` |
| `RecordLaunch` | desktop-id | `null` (feeds ranking) |
| `GeocodeCity` | name | `[{name, country, lat, lon}]` (Open-Meteo geocoding) |
| `NetworkChanged` | — | `null`; triggers a fresh `PublicIp` lookup |

| Event | Data |
|---|---|
| `ThemeChanged`, `LayoutChanged`, `ConfigError` | as the uqd signals |
| `SettingsChanged` | the full settings object |
| `SearchResults` | `{generation, provider, items:[{id, title, subtitle, icon, score, actions}]}` — one per provider; results for a stale generation are dropped |
| `Weather` | `{temp_c, code, is_day, location}`; fetched every 30 min while a location is set, and immediately when it changes |
| `PublicIp` | `{country_code, country, city}` or `null`; looked up via `https://ipinfo.io/json` when the shell sends `NetworkChanged` |
| `UqdDown` / `UqdUp` | — |
| `PrepareForSleep` | `{sleeping: bool}` from logind on the system bus; the shell locks on `true` |

Built-in providers inside the bridge:

- **`calc`:** answers queries starting with `=`, or any query that parses as a pure expression.
  Uses a safe evaluator (`+ - * / ^ % ( )`, `of`, and the functions `sqrt sin cos tan ln log abs
  round`).
- **`run`:** answers queries starting with `>`; activating it runs the command through
  `hyprctl dispatch exec`.

External providers are every bus name that owns the object `/org/uq/Launcher1` with interface
`org.uq.Launcher1.Provider`: `Search(s query) → a(sssssd)` (id, title, subtitle, icon, actions
as JSON, score) and `Activate(s id, s action)`. They are discovered through `.provider` files in
`/usr/share/uq/launcher/` and `~/.local/share/uq/launcher/` (keys `BusName`, `Name`, `Prefix`).
Each search is sent to every provider in parallel; anything that hasn't answered within 300 ms
is dropped.

App search happens in the shell (`DesktopEntries` plus a fuzzy match). Its ranking combines the
match score with `log(1 + launch_count)` from uqd.

### 2.4 uqd additions

The state gains `pinned: Vec<String>`, `weather_location: Option<Location>`, `idle:
IdleTimings`, `notif_backend: NotifBackend` and `launch_counts: BTreeMap<String, u32>`. The
defaults are no pins, no weather location, idle 240/300/360/1200 seconds, and backend `auto`.
D-Bus adds `GetSettings() → s` (JSON), `SetSetting(s key, s json)`, `RecordLaunch(s id)` and
`GetLaunchCounts() → a{su}`, plus the signal `SettingsChanged(s json)`. Settings changes are
saved to `state.toml` but don't touch `generated.lua`, so they go through a lighter commit path
with no verify step.

### 2.5 Notification backends

There's a QML interface, `NotifBackend`, with properties `available`, `count`, `latest`
(`{appIcon, appName, summary}` or null), `dnd` and `hasCentre`, and functions `toggleCentre()`
and `setDnd(bool)`. The adapters:

| Backend | Detect | Data | Controls |
|---|---|---|---|
| noti-fi | `qs ipc call notifd status` succeeds | `status()` JSON: unread count, DnD, latest | `center("toggle")`, `dnd(mode)` |
| built-in | uq-shell's own `NotificationServer` | full | own popup and centre |
| swaync | `swaync-client --count` | count, DnD | `-t`, `-d` |
| mako | `makoctl list` | count, latest | `makoctl mode -t do-not-disturb`; no centre |
| dunst | `dunstctl count` | count, paused | `dunstctl history-pop`, `set-paused toggle` |

`auto` picks noti-fi if it's available, otherwise built-in. The built-in server registers only
when `org.freedesktop.Notifications` is unowned. If another daemon owns it, the island says
"Notifications handled by <owner>". The built-in server has popups and a simple centre;
noti-fi remains the full-featured choice.

### 2.6 noti-fi changes (in the noti-fi repo, tracked here)

1. **Hyprland mode:** run notifd as a plain user service with no Plasma takeover. The `x-kde-*`
   hints become optional, and popup placement uses layer-shell anchors per monitor.
2. **OSD:** `osd(kind, value, max)` added to the IPC API, where kind is `volume`, `mic` or
   `brightness`. It draws a level OSD styled from noti-fi's config, and noti-fi reads uQ's
   `tokens.json` when it exists.
3. **`status()`** includes `latest: {appIcon, appName, summary}` for the island.

### 2.7 Lock and idle

- **Lock:** a `WlSessionLock` surface on every screen, showing the clock, an avatar and a
  password field. Two PAM contexts run side by side: `uq-lock` (password) and
  `uq-lock-fingerprint` (fprintd, started only if `fprintd-list $USER` lists a finger). The
  first one to succeed unlocks.
- **Crash safety:** a lock flag at `$XDG_RUNTIME_DIR/uq-locked` is written on lock and removed
  on unlock. On startup, uq-shell re-locks if the flag exists. Hyprland's
  `allow_session_lock_restore` lets the new lock take over.
- **Idle:** an `IdleMonitor` (which respects inhibitors) per stage: dim (brightness to 30%, then
  restored), lock, screens off (`hl.dsp.dpms` off, on again at the next input), and suspend
  (`systemctl suspend`, only when UPower reports discharging). Nothing advances while media is
  playing (Mpris) or Stay awake is on (`IdleInhibitor`). The machine also locks before sleep,
  using a logind `PrepareForSleep` watch in the bridge (event `PrepareForSleep`).

### 2.8 Window actions (Work island)

The dropdown offers float toggle, pin to all workspaces, fullscreen, move to workspace 1–10,
and close.

- **Minimise:** move to `special:minimized`. The shell remembers each minimised window's
  original workspace; clicking its greyed dock entry moves it back.
- **Maximise:** `fullscreen` with mode 1 (maximise).
- **Close:** `killactive`.

### 2.9 Theming

The shell reads `~/.local/state/uq/theme/tokens.json` with `FileView { watchChanges: true }`
into a `Theme` singleton. Every component takes its colours, radius, gaps and fonts from it.
There are no colours in the view files.

## 3. Code layout

```
shell/shell.qml                 root: per-screen bar, overlays, lock, idle
shell/core/                     Theme.qml, Bridge.qml, Settings.qml (singletons)
shell/bar/                      Bar.qml, SystemIsland/, WorkIsland/, TimeIsland/, NotifIsland/, StatusIsland/
shell/launcher/                 AppMenu.qml, Spotlight.qml, AppModel.js
shell/quick/                    QuickSettings.qml and groups
shell/osd/                      Osd.qml (fallback when noti-fi absent)
shell/notif/                    NotifBackend.qml, adapters/, builtin/
shell/lock/                     Lock.qml, Idle.qml
shell/logic/                    pure JS/QML models, no Quickshell imports (tested by qmltestrunner)
tests/qml/                      tst_*.qml for shell/logic
crates/uqctl/src/bridge/        protocol.rs, calc.rs, providers.rs, weather.rs, ip.rs, mod.rs
crates/uq-core/src/settings.rs  Settings types (shared by uqd and bridge)
packaging/uq-shell.service
```

## 4. Error handling

| Failure | Behaviour |
|---|---|
| uq-shell crash | `uq-shell.service` restarts it; if the lock flag is set it re-locks before drawing anything else |
| bridge crash or exit | Shell restarts it with backoff 1, 2, 4 … 30 s; uqd-backed controls grey out showing their last values; Hyprland-sourced and Quickshell-service widgets keep working |
| uqd not on the bus | The bridge emits `UqdDown`, polls every 5 s, then emits `UqdUp` and re-sends `GetSettings` |
| Provider slow or erroring | Dropped after 300 ms or on error; logged once per provider per minute |
| Chosen notification backend missing | Fall back to built-in with a warning; built-in yields if the bus name is owned |
| Offline or HTTP failure | Weather hidden (clock stays); `PublicIp` null (VPN name stays); retried at the next interval or network change |
| PAM fingerprint unavailable | Password only; no error shown |
| `brightnessctl`, `nmcli` or `tailscale` missing | That widget hides; no error loop |
| Hyprland IPC event stream drops | `Quickshell.Hyprland` reconnects; the shell re-reads its workspaces and toplevels |

## 5. Testing

- **Bridge (Rust):** protocol encode/decode, the calculator (precedence, `of`, errors, no code
  execution), provider fan-out (a fast provider is kept, a 500 ms provider is dropped, stale
  generations are dropped), weather and ipinfo clients against a local fake HTTP server, and the
  `UqdDown`/`UqdUp` cycle. Integration runs against a private dbus-daemon with a fake provider
  service.
- **uqd:** settings round-trip, `SetSetting` validation, launch counting and `SettingsChanged`
  emission.
- **Shell logic:** `qmltestrunner` on `shell/logic`, covering dock grouping (pins, running,
  minimised), fuzzy match and ranking, the minimise bookkeeping, the idle stage machine
  (inhibitors, battery-only suspend), and the notification adapters' output parsing.
- **Smoke test:** headless Hyprland in CI with uq-shell running. It asserts the `uq-bar` layer
  surfaces exist (via `hyprctl layers -j`) and captures a bar screenshot for each theme. If
  headless Hyprland can't host it, the slice-1 plan records that, and the step becomes a manual
  check.
- **noti-fi:** tests live in its own repo.

## 6. Slices (one plan each)

1. **Foundation of the shell:** `uq-shell.service`, Theme/Bridge/Settings singletons, the bridge
   (theme, layouts, settings, weather, IP), uqd settings, and the System, Work, Time/weather and
   Status islands. The Notifications island is a placeholder.
2. **Launchers:** the app menu, Spotlight, app model and ranking, the calc and run providers,
   external providers and `.provider` discovery.
3. **Quick settings and OSD:** all four groups, the fallback OSD, and the noti-fi OSD change.
4. **Notifications:** `NotifBackend`, the five adapters, the built-in server, and noti-fi's
   Hyprland mode and `status().latest`.
5. **Lock, idle and power:** the lock screen with PAM and fingerprint, crash re-lock, the idle
   stages, lock before sleep, and the power menu (`SUPER+Escape`).
