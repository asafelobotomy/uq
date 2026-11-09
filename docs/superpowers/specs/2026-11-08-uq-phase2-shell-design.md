# uQ Phase 2 — shell design

Date: 2026-11-08 · Status: revised after an independent audit (gaps, enhancements, security), awaiting spec review
Parent spec: `2026-11-08-uq-desktop-design.md` (§2.3 shell, §2.4 extension points, §7 phase 2)

## 1. Intent

Phase 2 gives uQ its own shell, built with Quickshell: a bar, an app menu, a Spotlight-style
search, quick settings, on-screen displays, a notification surface, a lock screen, idle handling,
a power menu and a polkit agent. Like everything in uQ, it's themed live from the active theme.

**What the user decided**

- **Bar:** floating islands along the top of each monitor, styled with the theme's radius and
  gaps. Five islands, in order:
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
  the current workspace's layout; power profile, battery, Do Not Disturb and Stay awake.
- **On-screen displays:** drawn by noti-fi when it's installed; uq-shell draws its own small OSD
  otherwise.
- **noti-fi** stays its own project and gains Hyprland support.
- **Weather:** Open-Meteo, for a city the user sets once.
- **VPN location:** the Tailscale exit node or the NetworkManager VPN name, plus the public IP's
  country.
- **Lock screen:** PAM password and fingerprint, whichever succeeds first.
- **Idle:** dim at 4 min, lock at 5, screens off at 6, suspend at 20 on battery only.
- **Shell-to-uqd link:** a long-lived `uqctl bridge` process speaking JSON lines.

**Audit-driven defaults** (these refine the decisions above; flagged for the user to confirm)

- **Lock still happens while media plays.** Media blocks only dim and screen-off. Apps showing
  video already hold idle inhibitors, so music or a muted browser tab can't leave an unattended
  machine unlocked. (Setting: `idle.media_blocks_lock`, default false.)
- **The Notifications island shows the app icon and unread count.** The summary appears on hover,
  or always if `notif.show_summary` is set. It's hidden while the screen is being shared, so
  message previews and 2FA codes stay out of screenshots and screen shares.
- **The public-IP country is looked up only while a VPN or exit node is active**, never while a
  VPN is still connecting. Only the country is kept. It can be switched off with
  `privacy.public_ip_lookup`, which defaults to on to match the user's choice.

**Assumptions**

- The greeter stays in Phase 5. Kvantum is still deferred.
- Pinned apps, weather location, idle timings, privacy switches and launch counts are stored by
  uqd and can be set through `uqctl` until the Phase 3 settings app exists.
- Hyprland has no minimise, so minimise moves the window to a hidden `special:minimized`
  workspace.
- **Threat model:** this is a single-user desktop. A process running as the user, unsandboxed, is
  trusted to the same degree as the user; uQ doesn't defend against it beyond not making
  escalation easy. Flatpak and other sandboxed apps are untrusted. All text from outside the
  shell is untrusted: window titles, SSIDs, Bluetooth names, media metadata, tray tooltips,
  notification content, and launcher provider results.

**Success criteria**

- After logging into Hyprland, the bar appears on every monitor with all five islands. A theme
  change restyles it within a second, with no restart.
- Super+Space finds an app and launches it; `= 2+2` shows 4; a provider registered on the bus
  shows results. **An app launched from the shell survives a shell restart.**
- Volume, mic and brightness keys show an OSD. Quick-settings toggles change the real setting.
- Locking works with a password, and with a fingerprint where there's a reader. **Killing
  uq-shell or the lock process never exposes the session, and the lock comes back on its own.**
  The machine never suspends before the lock surface is secure.
- Unplugging the network or stopping uqd degrades individual widgets; it never takes the bar
  down.
- No text from outside the shell is ever interpreted as HTML, and nothing loads remote resources.

**Non-goals (Phase 2)**

- A greeter (Phase 5) or settings-app pages (Phase 3).
- Integrating repos beyond the bar's needs: noti-fi's changes are in scope; the other repos'
  providers and widgets come in Phase 4. The provider API ships now.
- Clipboard history, file search and web search in the launcher.
- Managing third-party notification daemons. uQ doesn't start, stop or mask swaync, mako or
  dunst.

## 2. Architecture

### 2.1 Processes and units

| Unit / process | Started by | Role |
|---|---|---|
| `uqd.service` | `hyprland.start` hook (Phase 1) | Desktop state, plus the settings in §2.4 |
| `uq-shell.service` | `hyprland.start` (restart), `hyprland.shutdown` (stop) | Quickshell: bar, launchers, quick settings, OSD fallback, notifications, idle, polkit agent |
| `uq-lock.service` | started by the shell on lock, and by the bridge/recovery if locked with no lock client | A separate small Quickshell instance (`/usr/share/uq/lock`) that owns `WlSessionLock`. A crash in the bar can't take the lock down, and passwords never enter the main shell's JS heap |
| `uqctl bridge` | uq-shell, as a child `Process` | JSON-lines link to uqd, launcher providers, logind, lock-notify, weather and IP |

Both shell units have:

- `ExecStart=/usr/bin/qs -p /usr/share/uq/<shell|lock>`
- `Restart=always`, `RestartSec=1`, `StartLimitIntervalSec=0`
- `KillMode=process`
- `PartOf=graphical-session.target`

Each Quickshell config sets `Quickshell.watchFiles = false`, so a package upgrade never
hot-reloads half-written files mid-session.

**`base.lua` additions:**

- The start hook also restarts `uq-shell.service`. The shutdown hook stops `uq-shell.service`
  and `uq-lock.service`.
- The environment export gains `DISPLAY`, `XCURSOR_SIZE`, `HYPRCURSOR_SIZE`, `XCURSOR_THEME`
  and `XDG_SESSION_TYPE`.
- `misc` options:
  - `allow_session_lock_restore = true`
  - `session_lock_xray = false` (pinned)
  - `key_press_enables_dpms = true`
  - `mouse_move_enables_dpms = true`
- uQ never adds `above_lock` layer rules for the `uq-*` or `notifd-*` namespaces.
- A recovery bind, `SUPER+CTRL+L`, marked `locked = true`, runs
  `systemctl --user restart uq-lock.service`. If the lock service is broken, it falls back to
  `hyprlock`.

**Launching apps.** No app ever runs as a child of the shell; that would put it in
`uq-shell.service`'s cgroup, and it would die when the shell restarts. Every launch is a bridge
call, `Launch`, which runs
`hl.dsp.exec_cmd("systemd-run --user --scope --collect --unit=app-uq-<id>-<rand> -- <argv>")`,
so each app gets its own scope. `KillMode=process` is only a backstop.

### 2.2 Data sources

| Need | Source |
|---|---|
| Workspaces, windows, focus, monitors | `Quickshell.Hyprland` (window class from `lastIpcObject.class`) |
| Wi-Fi and wired | `Quickshell.Networking` (exists in 0.3.1; Wi-Fi and wired only) |
| VPN name | `nmcli -t -f NAME,TYPE con show --active`, polled every 10 s |
| Tailscale exit node | `tailscale status --json`, polled every 10 s, only when the `tailscale` binary exists |
| Audio | `Quickshell.Services.Pipewire` |
| Battery, power profiles | `Quickshell.Services.UPower` (`PowerProfiles`) |
| Media | `Quickshell.Services.Mpris` |
| Tray | `Quickshell.Services.SystemTray` |
| Bluetooth | `Quickshell.Bluetooth` |
| Apps | `DesktopEntries` (`byId`, `heuristicLookup`, `command`) |
| Idle | `IdleMonitor` (with `respectInhibitors`), `IdleInhibitor` for Stay awake |
| Polkit | `Quickshell.Services.Polkit` |
| Brightness | `brightnessctl` (set and get), run only from shell actions |
| Lock state | `hyprland-lock-notify-v1`, bound by the bridge (§2.7) |
| Sleep | logind on the system bus, via the bridge (§2.7) |
| Screen sharing | Hyprland `screencast` events → `sharing` flag (§2.10) |

A network change, an exit-node change or a VPN appearing or disappearing triggers
`NetworkChanged`, debounced by 3 s.

### 2.3 Bridge protocol (`uqctl bridge`)

The bridge exchanges one JSON object per line over stdin/stdout.

**Framing and limits:**

- Every message carries `"v":1`.
- Maximum line length is 256 KiB in each direction; longer lines are dropped, never buffered.
- The bridge logs only to stderr, because stdout is the protocol.
- It exits on stdin EOF.

**Message shapes:**

- Request: `{"v":1,"id":<u64>,"call":"<Name>","args":[...]}`
- Reply: `{"v":1,"id":<u64>,"ok":<value>}` or `{"v":1,"id":<u64>,"err":"<message>"}`
- Event: `{"v":1,"event":"<Name>","data":{...}}`

**Calls:**

| Call | Args | Result |
|---|---|---|
| `GetTheme`, `SetTheme`, `ListThemes`, `GetLayouts`, `SetLayout`, `SetSchedule`, `Reload` | as `org.uq.Desktop1` | as `org.uq.Desktop1` |
| `GetSettings` | — | settings object (§2.4); excludes launch counts |
| `SetSetting` | key, JSON value | `null` or validation error |
| `GetLaunchCounts` | — | `{desktop_id: count}` |
| `RecordLaunch` | desktop-id | `null` |
| `Search` | query, generation u64 | `null`; results arrive as `SearchResults` events |
| `Activate` | provider, item id, action | `null` |
| `Launch` | `{desktop_id}` or `{argv:[...]}` or `{run_id}` | `null`; see §2.1 and §2.8 |
| `Dispatch` | action name, args (closed set, §2.8) | `null` |
| `GeocodeCity` | name (≤ 100 chars) | `[{name, country, lat, lon}]` (at most 5) |
| `NetworkChanged` | — | `null` |
| `SleepLockReady` | — | `null`; releases the sleep inhibitor (§2.7) |
| `Lock` | — | `null`; starts `uq-lock.service` |

**Events:**

| Event | Data |
|---|---|
| `Hello` | `{v, uqd_up, locked}`, sent first on start |
| `ThemeChanged`, `LayoutChanged`, `ConfigError` | as the uqd signals |
| `SettingsChanged` | settings object (no launch counts) |
| `LaunchCountsChanged` | `{desktop_id: count}` |
| `SearchResults` | `{generation, provider, provider_name, source_dir, items:[{id, title, subtitle, icon, score, actions}]}` |
| `Weather` | `{temp_c, code, is_day, location_name}`; sent at start, when the location changes, then every 30 min |
| `PublicIp` | `{country_code, country}` or `null` (§2.11) |
| `NotifOwner` | `{owner: "<unique name or well-known owner>"\|null}`, the owner of `org.freedesktop.Notifications` (watched with `NameOwnerChanged`) |
| `PrepareForSleep` | `{sleeping: bool}` |
| `LockState` | `{locked: bool}` from lock-notify |
| `UqdDown` / `UqdUp` | — |

**Built-in providers (inside the bridge):**

- **`calc`:** runs only for queries starting with `=`, or queries containing an operator. A bare
  number isn't a calculation.
  - Grammar: `+ - * / ^ ( )`, unary minus, `x% of y`, and a postfix `%` meaning ÷100. The
    functions are `sqrt sin cos tan ln log abs round`; the constants are `pi` and `e`.
  - Limits: queries up to 256 characters and nesting up to 32 levels deep. The parser is
    iterative (precedence climbing with an explicit stack), so deep nesting can't overflow it.
  - Results are rounded to 12 significant digits. Errors return no result.
- **`run`:** runs only for queries starting with `>`. Its single item's id is the bridge-local
  token `run-<generation>`, mapped to the typed command for that generation only. `Launch
  {run_id}` executes the mapped string with `sh -c` inside a scope (§2.1). That's intended: the
  user typed it. Any other id is rejected.

**External providers:**

- A provider is any bus name that owns `/org/uq/Launcher1` with the interface
  `org.uq.Launcher1.Provider`, offering `Search(s query) → a(sssssd)` and
  `Activate(s id, s action)`.
- Providers are discovered from `.provider` files in `/usr/share/uq/launcher/` and
  `~/.local/share/uq/launcher/`, with the keys `BusName`, `Name` and `Prefix` (optional).
  Same-user processes are trusted to write these (threat model), and each result carries its
  `source_dir` for debugging.
- **Gating:** searches are debounced 70 ms. Queries starting with `>` or `=` are never sent to
  external providers. A provider with a `Prefix` gets only queries that start with it (with the
  prefix stripped). Others get queries of 2 characters or more.
- Only names that currently have an owner are queried, with `NO_AUTO_START` set. Each call has a
  300 ms deadline.
- **Clamping:** at most 20 items per provider. Titles are truncated to 200 bytes, subtitles to
  300 and actions to 2 KiB. `icon` must be an icon-theme name (`[A-Za-z0-9._-]+`) or an absolute
  local path; anything else becomes the provider's default icon.
- **Scoring:** provider scores are clamped to [0, 1] and multiplied by 0.9. An external provider
  can never outrank an exact or prefix app-name match, so it can never take the top hit for an
  app's name. Every provider row shows the provider's `Name`.
- `Activate` goes back to the provider that produced the item. The shell never runs anything
  taken from provider data.

**App search and ranking** run in the shell (`DesktopEntries` plus a fuzzy match). The score is
the match score + 0.15 × log(1 + launch_count). Exact and prefix name matches outrank everything.

### 2.4 uqd additions

The state gains:

- `pinned: Vec<String>`
- `weather_location: Option<Location>`
- `idle: IdleTimings`
- `notif: NotifSettings`
- `privacy: PrivacySettings`
- `launch_counts: BTreeMap<String, u32>`

D-Bus gains `GetSettings() → s`, `SetSetting(s key, s json)`, `RecordLaunch(s id)`,
`GetLaunchCounts() → a{su}`, `ClearLaunchCounts()`, and the signals `SettingsChanged(s)` and
`LaunchCountsChanged(a{su})`. There are no caller checks; under the threat model in §1, strict
validation is the guard.

**Setting keys** (anything else is rejected):

| Key | Type | Validation | Default |
|---|---|---|---|
| `pinned` | array of desktop ids | ≤ 32 entries, each matching `^[A-Za-z0-9._-]{1,128}(\.desktop)?$`, de-duplicated | `[]` |
| `weather_location` | `{name, lat, lon}` or null | name ≤ 100 chars, lat in [-90, 90], lon in [-180, 180]; stored rounded to 2 dp | `null` |
| `idle.dim`, `idle.lock`, `idle.screen_off`, `idle.suspend_battery` | seconds, or null to disable | each 30–86400; enabled stages must be strictly increasing (dim < lock < screen_off < suspend) | 240, 300, 360, 1200 |
| `idle.locked_screen_off` | seconds | 10–600; screens go off this soon after locking | 30 |
| `idle.media_blocks_lock` | bool | — | false |
| `notif.backend` | `auto` \| `notifi` \| `builtin` \| `swaync` \| `mako` \| `dunst` | enum | `auto` |
| `notif.show_summary` | bool | — | false |
| `privacy.public_ip_lookup` | bool | — | true |
| `privacy.record_launches` | bool | — | true |

**Writes and persistence:**

- Settings writes use a light commit path: validate, save `state.toml`, emit. They don't touch
  `generated.lua` and have no verify step.
- `RecordLaunch` updates memory immediately and saves at most once every 30 s. It's a no-op when
  `privacy.record_launches` is false.
- Launch counts are capped at 10,000 per app and pruned of uninstalled ids on startup.

**Upgrade path:** a Phase 1 `state.toml` loads through `serde(default)`. A file that won't parse
is renamed to `state.toml.bad-<timestamp>` before defaults are written, so pins and counts are
never silently lost.

### 2.5 Notification backends

There's a QML interface, `NotifBackend`, with properties `available`, `count`, `latest`
(`{appIcon, appName, summary}` or null), `dnd`, `hasCentre`, and functions `toggleCentre()` and
`setDnd(bool)`. The adapters:

| Backend | Detect | Updates | Controls |
|---|---|---|---|
| noti-fi | `notifd.sock` greets with `name "notifd"`, `protocol 1` (§2.6) | push: `subscribe` → `state` events (`unread`, `dnd`, `center`, `sharing`, `latest`) | `center`, `dnd`, `osd` methods over the same socket (§2.6) |
| built-in | uq-shell's own `NotificationServer` | full | own popups and centre |
| swaync | `swaync-client --version` | push: `swaync-client -swb` | `-t`, `-d` |
| mako | `makoctl --help` | poll `makoctl list` every 5 s | `makoctl mode -t do-not-disturb`; no centre |
| dunst | `dunstctl is-paused` | poll `dunstctl count` every 5 s | `history-pop`, `set-paused toggle`; no centre |

**Choosing a backend:**

- `auto` picks noti-fi when its socket answers, otherwise built-in.
- Built-in registers only when the bridge's `NotifOwner` says the name has no owner. If something
  else owns it, the island shows "Notifications handled by <owner>".
- uQ doesn't start, stop or mask third-party daemons. Their D-Bus activation files can auto-start
  them; the README documents masking.

**The built-in server:**

- Sets `bodyMarkupSupported: true` but sanitises bodies to `b i u a` (with `a` showing its href
  on hover, never auto-followed).
- Sets `bodyImagesSupported: false` and `bodyHyperlinksSupported: false`.
- Accepts images only as `image-data` hints or local paths.

### 2.6 noti-fi integration (shipped in noti-fi commit `0454a4d`; see its `docs/protocol.md`)

noti-fi made the changes uQ asked for. The adapter talks to it over `$XDG_RUNTIME_DIR/notifd.sock`
using one long-lived connection.

- **Detect:** the first line is
  `{"event":"hello","name":"notifd","protocol":1,"daemon":"<ver>","pid":N,"seq":N}`. The adapter
  requires `name == "notifd"` and `protocol == 1`; anything else counts as "noti-fi unavailable".
- **Requests:** `{"id":N,"method":"<m>","params":{...}}`.
  - `osd {kind: "volume"|"mic"|"brightness", value, max (default 100), muted}` replies
    `{"ok":true}`, or the error code `bad_params`. It's cheap, so the adapter sends one line per
    key-repeat step.
  - `center {action: "open"|"close"|"toggle"}`.
  - `dnd {mode: "on"|"off"|"toggle"}`.
  - `subscribe`.
- **`state` events** carry the full status object. The fields uQ uses:
  - `unread`
  - `dnd`
  - `center` (whether the list is open)
  - `sharing`
  - `latest`: `{id, t, appName, appIcon, summary, read}` or null; `summary` is `""` while sharing
  - `holding`, which is one of `""`, `"dnd"`, `"inhibited"`, `"screen sharing"`, `"fullscreen"`
    or `"focus"`

  noti-fi also sends `unreadPerZone` and `inhibitors`, which uQ ignores.
- **Limits:** 1 MiB per line and 16 connections. uQ's adapter still applies its own text caps
  and renders everything as plain text.
- **Placement:** the OSD and the confirmation pill appear on Hyprland's focused monitor, and every
  window moves off a monitor that's unplugged. Putting `"screen": "focused"` in noti-fi's
  `rules.json` moves popups and the list there too; uQ documents it rather than writing the
  user's file.
- **Theming:** with `theme.tokens` set to `"auto"` (the default), noti-fi reads
  `$XDG_STATE_HOME/uq/theme/tokens.json` every 2 s. That works with the directory swap. If the
  file is broken, it keeps the last good tokens; the user's own `rules.json` theme takes
  precedence.
- **Hyprland mode:** `notify takeover --plain` (the default outside KDE) installs a plain user
  service. Other notification daemons have to be stopped first.
- **Security:** all app-supplied text is plain text, and bodies keep only `b`, `i`, `u` and `br`,
  with no remote loads. While the screen is shared, popups show only the app name and icon.
  notifd never sets `above_lock`.
- **Open item:** Hyprland mode hasn't been tested on a real Hyprland session yet. uQ's
  notifications slice is that first real test, and reports problems back to noti-fi.

### 2.7 Lock, sleep and idle

**Lock UI.** `uq-lock.service` runs a separate Quickshell instance that shows a `WlSessionLock`
surface on every screen, with the clock, the avatar (AccountsService icon, then `~/.face`, then
an initial) and a password field.

**PAM.** The files are installed to `/etc/pam.d/` and listed in the PKGBUILD's `backup=()`:

- `uq-lock`: `auth include system-local-login` and `account include system-local-login`.
- `uq-lock-fingerprint`: `auth required pam_fprintd.so max-tries=3 timeout=30`, with no password
  module.

If a file is missing, Arch's `/etc/pam.d/other` denies, so authentication fails closed.

**Contexts:**

- The password context responds only to echo-off prompts, with the typed password.
- The fingerprint context starts only when `fprintd-list $USER` lists a finger. It **never
  answers a prompt**: any `responseRequired` aborts it.
- Whichever succeeds first unlocks. After `max-tries` or a timeout, the fingerprint context
  restarts at most once every 10 s. It also restarts on resume (`PrepareForSleep(false)`).

**Messages and the password field:**

- `PamContext.message` is shown as plain text. A faillock lockout ("account locked, try again in
  N min") is shown, never hidden.
- The field sets `Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText`, and is
  cleared after each attempt.
- Passwords and PAM messages are never logged.

**Lock state and crash recovery:**

- The bridge binds `hyprland-lock-notify-v1`. Hyprland reports `locked` right away when the
  session is already locked, so the shell always knows the true lock state from `LockState`.
- If `LockState` is locked and no `uq-lock` client is running (the shell checks the unit),
  uq-shell starts `uq-lock.service`, and `allow_session_lock_restore` lets it take over.
- `uq-lock.service` has `Restart=always`. A lock that dies is restarted in about 1 s.
- If everything fails, the `SUPER+CTRL+L` recovery bind remains (§2.1). Hyprland keeps the
  session locked throughout (verified), so nothing is exposed.

**Locking rules:**

- On lock, the shell closes and wipes Spotlight, the app menu, quick settings and any popups.
- Open requests are ignored while locked.
- No IPC function can unlock; `uq-lock` exposes none.
- `SetLockedHint` is called on logind.
- The logind session's `Lock` signal (`loginctl lock-session`) triggers a lock.

**Lock before sleep:**

- The bridge holds a logind `Inhibit("sleep", "uQ", "Lock the screen before sleep", "delay")`
  at all times.
- On `PrepareForSleep(true)`, the shell locks. When `WlSessionLock.secure` is true, the lock
  process tells the shell, which calls `SleepLockReady`, and the bridge releases the inhibitor.
- A fallback releases it anyway just before logind would: logind's `InhibitDelayMaxUSec` less
  0.5 s (4.5 s with logind's default of 5 s; 4.5 s if it can't be read; never below 2 s). A fresh
  `qs` lock instance can take several seconds to load on a busy machine. If the fallback fires,
  Hyprland still renders the lock first on resume, because the lock request is already in place.
- A fresh inhibitor is taken on `PrepareForSleep(false)`.

**Idle stages** use one `IdleMonitor` each, with `respectInhibitors: true`:

1. **Dim:** a 60% black overlay layer surface per screen (namespace `uq-dim`). It works on every
   monitor, needs no brightness tool, and disappears by itself if the shell crashes. Input
   removes it.
2. **Lock:** starts `uq-lock.service`.
3. **Screens off:** `hl.dsp.dpms({ action = "off" })`. Screens come back on through Hyprland's
   `*_enables_dpms` options. Once locked, screens go off after `idle.locked_screen_off`.
4. **Suspend:** only when UPower reports discharging, **and** only once the lock is secure. If
   the lock failed, there's no suspend.

Media (Mpris playing) blocks only dim and screen-off, unless `idle.media_blocks_lock` is set.
Stay awake (`IdleInhibitor`) blocks every stage.

### 2.8 Window actions and dispatch

The shell never builds Lua or dispatch strings. It calls `Dispatch(action, args)` on the bridge,
which maps a closed set of actions to Lua. All string arguments go through the Phase 1
`lua_string` escaper, and windows are always addressed as `address:0x…`.

| Action | Lua |
|---|---|
| `focus_workspace {id}` | `hl.dsp.focus({ workspace = <int> })` |
| `focus_window {address}` | `hl.dsp.focus({ window = "address:0x…" })` |
| `close {address}` | `hl.dsp.window.close({ window = … })` |
| `toggle_float {address}` | `hl.dsp.window.float({ action = "toggle", window = … })` |
| `pin {address}` | `hl.dsp.window.pin({ window = … })` |
| `fullscreen {address, mode: "full"\|"maximized"}` | `hl.dsp.window.fullscreen({ mode = …, window = … })` |
| `move {address, workspace}` | `hl.dsp.window.move({ workspace = <int or escaped string>, follow = false, window = … })` |
| `minimise {address}` | tag the window `uq_ws_<n>`, then `move` it to `special:minimized` |
| `restore {address}` | read the tag, move it back, untag |
| `dpms {on}` | `hl.dsp.dpms({ action = "on"\|"off" })` |

Every exact Lua form is checked against the pinned Hyprland source in the slice-1 plan, and
covered by a `--verify-config`-style eval test.

**Minimise is stored in a window tag**, so a shell restart doesn't strand minimised windows.
`special:` workspaces never appear in the workspaces island.

**Dock mapping:**

- A window's class (`lastIpcObject.class`) is matched to a desktop entry through
  `DesktopEntries.heuristicLookup`, which covers `StartupWMClass`, Flatpak and Electron apps.
- Clicking an app with several windows cycles through them in focus order.
- The dock lists windows from all monitors, and marks the focused monitor's windows.

**Launching a desktop entry** (`Launch {desktop_id}`):

- The bridge reads the entry's `Exec`, strips field codes (`%f %F %u %U %i %c %k`), and splits it
  into argv following the desktop-entry spec.
- Each argument is shell-quoted, and the whole string is Lua-escaped for `hl.dsp.exec_cmd`
  inside a `systemd-run` scope (§2.1).
- Terminal apps get `kitty -e`.

### 2.9 Theming

The shell reloads `tokens.json` on the bridge's `ThemeChanged` event, which is the primary
trigger. As a backstop, it also watches the parent directory `~/.local/state/uq/`, because
Phase 1 swaps the whole `theme/` directory and a file watch on the old inode would go stale. If
a read fails, the shell keeps the last good tokens. A test runs the real `atomic_replace_dir` and
asserts the shell reloads.

### 2.10 Text safety, privacy and focus

- **Plain text only.** All text goes through `UqText`, which uses `textFormat: Text.PlainText`.
  A CI lint (`grep`) forbids a bare `Text {` in `shell/` outside `UqText.qml`. The only rich text
  is built-in notification bodies, after sanitising (§2.5).
- **No remote resources.** Icons come only from the icon theme or absolute local paths; nothing
  is fetched over the network.
- **Screen sharing.** Hyprland's `screencast` events set `sharing`. While it's true, the shell
  hides notification summaries, the VPN country and the weather city.
- **Focus and monitors:**
  - Spotlight, the app menu, quick settings and the fallback OSD open on
    `Hyprland.focusedMonitor`, and take keyboard focus through `HyprlandFocusGrab`.
  - Popups close if their monitor is unplugged.
  - Icons use `sourceSize` × the device pixel ratio.
- **Keyboard:**
  - Spotlight: arrows move, Tab moves between groups, Enter activates, Esc closes.
  - The app menu also supports type-to-search.
  - Every control has an `Accessible.name`.
- **Reduced motion.** `tokens.json` gains `ui.reduce_motion` (default false, a theme token);
  when it's set, animations are skipped.

### 2.11 Network lookups

**Common to every lookup:**

- HTTPS only, through `reqwest` with `rustls` and the platform verifier (system CA store).
- User-Agent `uq/<version>`.
- 5 s timeout.
- No response is logged or persisted.

**Weather (Open-Meteo):**

- Off until a city is set.
- Geocoding sends the typed city name.
- Forecasts send the latitude and longitude rounded to 2 dp.

**Public IP (ipinfo.io):**

- Only when `privacy.public_ip_lookup` is on **and** a VPN or Tailscale exit node is active.
- Runs 5 s after the network settles, at most once every 60 s.
- Only `country` and `country_code` are kept; the IP, city, org and location are discarded.
- No lookup happens with no VPN, which also avoids revealing the real IP while a VPN is
  connecting.

### 2.12 Keybinds and shell IPC

The binds live in `base.lua`. Shell actions use `hl.dsp.global("uq:<name>")` bound to Quickshell
`GlobalShortcut`s.

| Bind | Action | `locked` |
|---|---|---|
| `SUPER+Space` | `uq:spotlight` | no |
| `SUPER` tap (release) | `uq:appmenu` | no |
| `SUPER+N` | `uq:notif-centre` | no |
| `SUPER+A` | `uq:quick-settings` | no |
| `SUPER+Escape` | `uq:power-menu` | no |
| `SUPER+L` | `uq:lock` | no |
| `SUPER+CTRL+L` | restart `uq-lock.service`, falling back to `hyprlock` | yes |
| `XF86AudioRaiseVolume`, `XF86AudioLowerVolume`, `XF86AudioMute`, `XF86AudioMicMute` | `uq:vol-up`, `uq:vol-down`, `uq:mute`, `uq:mic-mute` (the shell changes Pipewire and shows the OSD) | yes |
| `XF86MonBrightnessUp`, `XF86MonBrightnessDown` | `uq:bright-up`, `uq:bright-down` (the shell runs `brightnessctl` and shows the OSD) | yes |

These replace Phase 1's direct `wpctl` and `brightnessctl` binds.

- The OSD ignores Pipewire changes in the first 2 s after startup, so it doesn't fire at login.
- The shell's `IpcHandler` (`qs ipc -p /usr/share/uq/shell call uq …`) exposes only `open`,
  `close` and `toggle` for its surfaces, plus `lock`. These are ignored while locked; there's no
  unlock.

### 2.13 Power menu and polkit

- **Power menu:** lock, log out, suspend, restart and shut down. Restart and shut down each have a
  confirm step with a 30 s countdown. Log out uses `hyprshutdown` when available, otherwise
  `hl.dsp.exit()`. It's opened by `SUPER+Escape` and from the app menu's bottom row.
- **Polkit agent:** `Quickshell.Services.Polkit`, so admin prompts from apps don't hang. It shows
  the action id and message as plain text, opens on the focused monitor above normal windows, and
  uses the same password-field hints as the lock screen.

## 3. Code layout

```
shell/shell.qml                 root: per-screen bar, overlays, idle, polkit
shell/core/                     Theme.qml, Bridge.qml, Settings.qml, UqText.qml (singletons/components)
shell/bar/                      Bar.qml, SystemIsland/, WorkIsland/, TimeIsland/, NotifIsland/, StatusIsland/
shell/launcher/                 AppMenu.qml, Spotlight.qml
shell/quick/                    QuickSettings.qml and groups
shell/osd/                      Osd.qml (fallback when noti-fi absent)
shell/notif/                    NotifBackend.qml, adapters/, builtin/
shell/idle/                     Idle.qml, Dim.qml
shell/power/                    PowerMenu.qml, PolkitAgent.qml
shell/logic/                    pure JS/QML models, no Quickshell imports (qmltestrunner)
lock/shell.qml                  separate uq-lock Quickshell config (Lock.qml, Pam.qml)
tests/qml/                      tst_*.qml for shell/logic
crates/uqctl/src/bridge/        protocol.rs, calc.rs, providers.rs, launch.rs, dispatch.rs, logind.rs, locknotify.rs, net.rs, mod.rs
crates/uq-core/src/settings.rs  Settings types and validation (shared by uqd, uqctl and the bridge)
packaging/uq-shell.service, packaging/uq-lock.service, packaging/pam/uq-lock, packaging/pam/uq-lock-fingerprint
```

**New `uqctl` commands:**

- `uqctl pin add|rm|list <desktop-id>`
- `uqctl weather set <city>|off`
- `uqctl idle set <stage> <secs|off>`
- `uqctl notif backend <name>`
- `uqctl privacy <public-ip|launches> on|off`
- `uqctl launches clear`
- `uqctl bridge`

**In-shell controls:** right-click a dock entry to pin or unpin it; the time popover has a
"Set city" entry.

**Packaging (`uq` PKGBUILD):**

- `depends`: add `quickshell>=0.3.1` and `quickshell<0.4` (Quickshell's Hyprland IPC depends on
  the Lua dispatch format, so it's pinned next to `uq-hyprland-pin`), `brightnessctl`, `upower`
  and `systemd`.
- `optdepends`:
  - `fprintd`: fingerprint unlock
  - `networkmanager`: VPN name
  - `tailscale`: exit node
  - `bluez`: Bluetooth
  - `power-profiles-daemon`: power profiles
  - `hyprlock`: recovery lock
  - `noti-fi`: full notifications and OSD
- `backup=(etc/pam.d/uq-lock etc/pam.d/uq-lock-fingerprint)`.
- `checkdepends`: `qt6-declarative` (for `qmltestrunner`) and `pamtester`.
- Install `uq-shell.service`, `uq-lock.service`, `shell/` and `lock/`.

## 4. Error handling

| Failure | Behaviour |
|---|---|
| uq-shell crash or kill | `Restart=always` brings it back in about 1 s; apps live on in their own scopes; if `LockState` is locked and no lock client is running, it starts `uq-lock.service` |
| uq-lock crash or kill | `Restart=always`; Hyprland keeps the session locked meanwhile; recovery bind as last resort |
| Bridge crash or exit | Shell restarts it with backoff 1, 2, 4 … 30 s. uqd-backed controls grey out showing their last values; Hyprland and Quickshell widgets keep working. The sleep inhibitor is re-taken on restart; while the bridge is down, sleep isn't delayed, but the next resume shows the lock first because idle and logind `Lock` still go to the shell |
| uqd not on the bus | `UqdDown`; poll every 5 s; then `UqdUp` and re-send `GetSettings` |
| Provider slow, erroring or oversized | Dropped after 300 ms, on error, or clamped (§2.3); logged once per provider per minute |
| Chosen notification backend missing | Fall back to built-in with a warning; built-in yields while another owner holds the name |
| Offline or HTTP failure | Weather hidden (clock stays); `PublicIp` null (VPN name stays); retried at the next interval or network change |
| PAM: no fingerprint reader, faillock lockout | Password only; the lockout message is shown in plain text |
| `brightnessctl`, `nmcli` or `tailscale` missing | That widget or action hides; no error loop |
| Hyprland IPC event stream drops | `Quickshell.Hyprland` reconnects; the shell re-reads its state |
| Monitor unplugged | Popups on it close; the bar for it is destroyed and re-created on replug |
| Corrupt `state.toml` | Renamed to `.bad-<ts>`, then defaults |

## 5. Testing

- **Bridge (Rust):**
  - Protocol: framing, the 256 KiB cap, a line that's too long gets dropped, and EOF makes it
    exit.
  - Calculator: precedence, `of` and `%`, a bare number gives no result, depth and length caps,
    and no code execution.
  - Providers: fan-out keeps the fast one, drops a 500 ms one, drops stale generations, applies
    clamping and score capping, and the top-hit guard holds against a spoofing provider.
  - `run`: only ids from the current generation are accepted.
  - Launch: Exec parsing and field-code stripping, shell quoting, Lua escaping, and the
    `systemd-run` command shape.
  - Dispatch: a table test of every action's Lua output.
  - Clients: Open-Meteo and ipinfo against a local fake HTTPS server. Coordinates are rounded,
    only the country is kept, and there's no lookup without a VPN.
  - Logind inhibitor flow: `PrepareForSleep`, `SleepLockReady`, release, and the 2 s fallback,
    with a fake system bus.
  - The `UqdDown`/`UqdUp` cycle.
  - Integration runs against a private dbus-daemon with a fake provider.
- **uqd:**
  - Settings validation: every key, bounds, the ordering rule, unknown keys rejected.
  - Launch counting: throttled saves, the cap, the opt-out, pruning.
  - A Phase 1 `state.toml` fixture loads; a corrupt one is renamed.
  - `SettingsChanged` excludes launch counts.
- **Shell logic:** `qmltestrunner` on `shell/logic`, covering:
  - dock grouping (pins, running, minimised, cycling)
  - fuzzy match and ranking
  - the minimise tag codec
  - the idle stage machine (inhibitors, the media rule, battery-only suspend, suspend gated on
    a secure lock)
  - parsing for the notification adapters and the noti-fi socket adapter
  - the HTML sanitiser
  - the `UqText` lint as a CI step
- **PAM:** `pamtester` against temporary copies of both stacks. A fingerprint stack with no
  reader must fail, never succeed.
- **Smoke tests (headless Hyprland in CI):**
  - Start uq-shell and assert the `uq-bar` layer surfaces exist (`hyprctl layers -j`).
  - Launch an app through `Launch`, restart the shell, and check the app is still alive.
  - Lock, `kill -9` the `uq-lock` process, and check that lock-notify still reports locked and
    the lock is re-acquired within 3 s.
  - Capture screenshots of the bar, Spotlight and quick settings for each theme.
  - If headless Hyprland can't host Quickshell, the slice-1 plan records it, and these become a
    manual checklist.
- **noti-fi:** tests live in its own repo.

## 6. Slices (one plan each)

1. **Foundation of the shell:**
   - both units, plus the `base.lua` changes (environment, misc options, recovery bind,
     keybinds)
   - Theme, Bridge and Settings, plus `UqText` and its lint
   - the bridge core (protocol, theme and layouts, settings, `Launch`, `Dispatch`, weather, IP,
     `NotifOwner`)
   - uqd settings and the new `uqctl` subcommands
   - the System, Work, Time/weather and Status islands
   - The Notifications island is a placeholder.
2. **Launchers:** app menu, Spotlight, app model and ranking, the calc and run providers,
   external providers with gating and clamping, `.provider` discovery.
3. **Quick settings and OSD:** all four groups, the OSD keybinds, the fallback OSD, and the
   noti-fi `osd` client. noti-fi already ships the `osd` method.
4. **Notifications:** `NotifBackend`, the five adapters (the noti-fi one per §2.6), the
   built-in server and its sanitiser, and a first real-Hyprland check of noti-fi's Hyprland
   mode.
5. **Lock, sleep, idle, power, polkit:**
   - `uq-lock` with PAM and fingerprint
   - lock-notify, crash recovery, the logind inhibitor flow
   - idle stages and dim overlay
   - power menu and polkit agent
