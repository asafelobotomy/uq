# Stress test: races, leaks and wasted work (2026-11-15, v0.3.0-rc.4)

A full pass over uqd, uq-core, uqctl's bridge, uq-settings-helper, the Settings app and the QML
shell. It combined a static review with live load on the nested desktop (`scripts/uq-dev
session`). The real session wasn't touched. Fixes are on the branch `stress-fixes`, one commit on
top of `tab-attention-dot`.

## How it was run

- **Tests:** `cargo test --workspace` gave 1962 passed, 0 failed (11 ignored). The QML tests
  (`qmltestrunner`) gave 566 passed, and `scripts/lint-shell.sh` was clean. After the fixes the
  counts are 1965 and 567, all passing.
- **Live load:** in the nested session's Hyprland. All builds were debug builds, so absolute CPU
  numbers are higher than a packaged install would show. Ticks are 10 ms of CPU, read from
  `/proc/<pid>/stat`. "Memory" means VmRSS + VmSwap, because the machine swaps to zram and RSS
  alone jumps around. One cycle was:
  1. open 40 kitty windows (one kitty process), 50 ms apart;
  2. focus every window, 10 rounds (410 focus changes);
  3. switch through workspaces 1–5, 40 rounds (200 switches);
  4. 5 windows changing their title every 10 ms for 20 s (about 500 title events a second);
  5. 100 `notify-send`s;
  6. close the 40 windows one by one, 50 ms apart.
- **uqd over D-Bus:** 2000 reads (8 methods, 64 in parallel), then 200 parallel `RecordLaunch`
  calls to separate ids and 200 to one id.
- **Bridge:** a separate `uqctl bridge` driven over stdio. 5000 mixed requests with the output
  read, then 40000 requests with the output never read.
- **Monitor hot-plug** couldn't be driven: the nested Hyprland has one virtual output, and the
  headless smoke test can't start on this machine. That path was reviewed statically only.

## What held up

These are the measurements that found nothing wrong. They're listed so they don't need re-testing.

**No leaks over four load cycles.** The shell's fd count went back to 64 after every cycle and
its thread count to 24. Its memory settled at 167, 170, 172 and 173 MB, a smaller rise each
time. That looks like caches warming (icons, JS heap), not a leak. uqd stayed at 15 fds and the
bridge at 17 fds the whole time.

**uqd under parallel D-Bus calls.**
- 2000 parallel reads: no errors, fd count unchanged.
- 200 parallel `RecordLaunch` calls to separate ids all landed, and 200 to one id counted 200.
  Every method takes the one `Desktop` mutex, so nothing is lost.
- `ListBinds` is the heaviest read at about 8 ms CPU per call, with a 42 KB reply. Only Settings
  calls it.

**Bridge throughput and backpressure.**
- 5000 requests took 1.8 s with memory flat (30 MB → 33 MB) and fds unchanged.
- With stdout never read, the bridge stopped reading stdin. It stayed at 4 MB for 30 s and
  nothing queued without bound: the output channel (256) and `MAX_IN_FLIGHT` (64) hold as
  designed.

**Idle cost.** Over 30 s with no input, the shell used 0.2% CPU and uqd and the bridge nothing.

**Statically checked and fine.**
- No std mutex is held across an `.await`.
- Hyprland request sockets are opened per request with a timeout and a reply cap.
- Child processes are reaped. The only exception is finding 9 below, now fixed.
- The broadcast and watch channels handle lag. Maps are bounded (launch counts, journal, restore
  points, notification store).
- `HyprRefresh` coalesces refreshes and isn't pushed back by bursts.
- Popups and other large surfaces are loaded only while open.
- The bridge's data islands sleep while they aren't placed.

## Findings, most severe first

"Fixed" means fixed on `stress-fixes` with a test or a live measurement. "Open" findings are
recommendations.

### 1. Open windows: every `latest` change counted as notification activity — medium, fixed

- **Where:** `shell/bar/WorkIsland.qml` had its own `onLatestChanged` handler, which appended to
  `notes`.
- **What went wrong:** a backend replaces `NotifBackend.latest` whenever its state changes:
  - noti-fi on every state message;
  - mako on its 5 s poll;
  - the built-in server on every dismissal or replace.

  Each of those counted as a new notification. The "Last activity" order then showed the newest
  app as always active, and `ordered`, `fit` and `tabInfo` recomputed on every bar each time.
- **Found by:** reading the code. `Work.noteArrived` already tells a new notification from a
  re-sent one; this handler didn't use it.
- **Fix:** record the note in `checkNote()` after the `fresh` check, through a new pure helper
  `Work.withNote` (capped at 50). Tested in `tst_work.qml::test_with_note`.

### 2. Bridge: unfiltered `NameOwnerChanged` streams on the uqd connection — medium, fixed (two of three)

- **Where:** `crates/uqctl/src/bridge/uqd.rs` (`subscribe_unbounded`) and
  `crates/uqctl/src/bridge/notifowner.rs` (`watch`).
- **What went wrong:**
  - Both subscribed to every name owner change on the bus, so every client that connects or
    leaves (each `notify-send`, portals, browser helpers) woke the bridge.
  - zbus stops reading a connection while any one stream's queue (64) is full. If 64 changes
    arrive while the watch loop waits on `GetSettings` (up to 20 s), every reply on the shared
    connection stalls with it, including that `GetSettings`.
- **Measured:** bridge CPU during the 100-`notify-send` phase was 7–11 ticks before the fix and
  1–8 after (noisy).
- **Fix:** `receive_name_owner_changed_with_args(&[(0, NAME)])`, the pattern the Settings app
  already uses.
- **Still open:** `bridge/widgets.rs:514` (`owner_loop`) does the same on the widgets' own
  connections. Its set of watched names changes at run time, so it needs one filtered stream per
  name, or draining on a separate task. Left as a recommendation.

### 3. Bridge: `ProcessUsage` baselines wiped by the other screen's call — medium (wrong numbers), fixed

- **Where:** `crates/uqctl/src/bridge/procusage.rs`, `usage()`.
- **What went wrong:** each call replaced the whole baseline map with its own pids. The Open
  windows island exists once per screen, and each copy polls its own pids every 5 s. With two or
  more screens in "By system usage" order, each call wiped the other's baselines, so CPU read 0.0
  for most windows.
- **Fix:** update entries in place, drop entries older than 30 s, and cap the map at 512.
- **Tests:** `callers_with_other_pids_keep_each_others_baselines` fails on the old code;
  `stale_baselines_are_dropped_and_capped` covers the pruning.

### 4. uqd: a failing file write can re-trigger itself about once a second — medium, fixed

- **Where:** the main loop in `crates/uqd/src/main.rs`, with `files.rs::write_marked` and
  `uq-core/src/atomic.rs`.
- **What went wrong:**
  - `refresh_files` writes into folders it also watches: `~/.config/autostart`,
    `environment.d`, `*-flags.conf` and `~/.local/share/applications`.
  - When a write fails after creating its temporary file (disk full, a filesystem without hard
    links), that file's create and delete events end the 1 s debounce and wake the loop again.
    It retried roughly every second, each time taking the desktop lock, fsyncing and logging.
  - That skipped the intended 60 s `FILES_RETRY`.
- **Found by:** static review only. ENOSPC couldn't be reproduced without root.
- **Fix:** while a write is failing, a watch wake waits for the retry. This is a pure
  `files_due()` with a unit test (`a_failing_write_waits_for_its_retry`).
- **Trade-off:** while writes fail, a user's own edit in those folders is picked up within 60 s
  rather than 1 s.

### 5. Built-in notifications: a progress notification rebuilt every card on each update — low–medium, fixed

- **Where:** `refreshReplaced()` and `showPopup()` in `shell/notif/Builtin.qml`.
- **What went wrong:**
  - Every replace (a download or volume progress at 5–10 Hz) made new `notifications` and
    `popups` arrays even when the notification was already first.
  - Both are plain-array Repeater models, so every popup card was destroyed and recreated, and
    every centre card too while the centre was open. That includes their image loads.
- **Measured:** a notification replaced 100 times at 20 Hz, beside 2 others, on the built-in
  backend. Shell CPU was 24 and 25 ticks before the fix and 17 and 20 after, about 25% less.
  More cards mean a bigger saving.
- **Fix:** when the notification is already first, only `latest` is refreshed (store) or only
  the deadline (popups). The cards bind to the notification's own properties.

### 6. Open windows: tab titles froze while any title kept changing — low, fixed

- **Where:** `shell/bar/WorkIsland.qml`, the `windowtitlev2` handler.
- **What went wrong:** the handler called `titleTimer.restart()`, a debounce. A title changing
  faster than every 100 ms (a terminal's progress, a page loading) kept pushing the rebuild back,
  so tabs showed stale titles until it stopped.
- **Fix:** throttle instead of debounce (start only if not running), as `HyprRefresh` does.
- **Measured cost:** this is a correctness fix, and it costs CPU during title spam. In the 15 s,
  500-titles-a-second test, shell CPU went from 261–262 ticks to 278–383, because the tabs now
  actually update ten times a second.

### 7. Open windows: the opening-order map only grew — low, fixed

- **Where:** `openedSeq` in `rebuild()`, `shell/bar/WorkIsland.qml`.
- **What went wrong:** closed windows were never removed, and every rebuild copied the whole map.
  It grew by one entry per window ever opened, on every bar.
- **Fix:** build the map from the open windows only, keeping their numbers.

### 8. Shell: switched-off edge indicators kept doing all their work — low, fixed

- **Where:** `shell/edge/EdgeIndicators.qml`.
- **What went wrong:** with `shell.edge_indicators = false`, the indicators were hidden, but they
  still:
  - rebuilt and stringified the window list on every title change;
  - asked for Hyprland refreshes;
  - kept one watcher object per window.
- **Fix:** `rebuild()` and `refresh()` return early while off, the Instantiator's model is empty
  while off, and turning the indicators on rebuilds.

### 9. Settings: Welcome's launches left zombie processes — low, fixed

- **Where:** `launch_detached` in `apps/uq-settings/src/pages/welcome.rs`.
- **What went wrong:** it dropped the `std::process::Child`, so each launch left a zombie until
  Settings quit.
- **Fix:** reap it on a thread, as `pages/shell.rs` does.

### 10. Shell: a focus change asked Hyprland for monitors and workspaces too — low, fixed

- **Where:** `onActiveToplevelChanged` in `shell/windows/ScreenPages.qml`.
- **What went wrong:** a focus change requested monitors, workspaces and windows, although focus
  changes neither. `ColumnPolicy` already asks for windows only.
- **Fix:** `refresh(true)` on focus asks for windows only. Moves between workspaces and monitors
  still come with their own events, which refresh everything.
- **Measured:** 205 focus changes cost 92–125 ticks before and 90–107 after. That is within
  noise on one screen; the saving is two IPC round trips per focus change.

### 11. Shell: missing fallback icon logged a warning per tab — low, fixed

- **Where:** `shell/bar/WorkIsland.qml`, `shell/edge/EdgeWindow.qml` and
  `shell/bar/SystemIsland.qml`. They called `Quickshell.iconPath("application-x-executable")`
  without `check`.
- **What went wrong:** in an icon theme without that icon, every tab created for an app with no
  icon logged `Could not load icon`. That was 37–44 warnings per workspace-switching phase in
  the load test.
- **Fix:** pass `true` (an empty source instead). After the fix the phase logged 0 warnings.

### 12. Shell: each focus change costs about 5 ms of CPU in the shell — medium cost, open

- **Measured:** about 100 ticks (1 s) of shell CPU per 205 focus changes with 41 windows, so
  about 5 ms each. Each workspace switch costs about 6 ms. Debug build.
- **Cause:** each refresh walks every toplevel and builds its own list, several of them
  `JSON.stringify`ing the result for comparison:
  - `WorkIsland.rebuild` and `SystemIsland.rebuildDock`, once per bar;
  - `EdgeIndicators.rebuild`;
  - `ResizeGrips.rebuild`;
  - `ScreenPages.read`;
  - `ColumnPolicy.read`;
  - `FitWindows.check`.

  With 3 screens and 30 windows, that is about 10 passes and 8 full stringifies per focus change.
- **Proposal:**
  1. A shared window-snapshot singleton, built once per `HyprRefresh.settled` and per title
     change, keyed once, and read by all of these. Medium risk: it touches most of the window UI.
  2. A small step that can come first: change the Instantiators at `SystemIsland.qml:194`,
     `EdgeIndicators.qml:130` and `ResizeGrips.qml:84` from `Hyprland.toplevels.values` (a plain
     list, so every open or close recreates all N watchers on every bar) to
     `Hyprland.toplevels`.

### 13. uqd: blocking `gsettings` and `systemctl` under the desktop lock — medium, open

- **Where:** `crates/uqd/src/system.rs:74-112`.
- **What goes wrong:**
  - `GSettings::apply` runs 6 `gsettings set` one after another with `std::process::Command`,
    with no timeout, on a tokio worker, inside the desktop mutex.
  - This runs on every theme change, including the scheduled dark/light flip, and on
    `Explain("appearance")`.
  - A stuck dconf-service would block every D-Bus method for up to the D-Bus timeout per call.
- **Proposal:** `tokio::process::Command` with `kill_on_drop` and a timeout, as `xrandr.rs` and
  `compositor.rs::verify` do. Moderate change, low risk.

### 14. uqd: each history write re-reads the day's journal and re-prunes — low–medium, open

- **Where:** `uq-core/src/history/journal.rs:101-170`.
- **What goes wrong:** `append` reads the whole day file to check its last byte, and `prune`
  re-parses every kept file (≤500 entries) on each write. A Settings slider sending `SetSetting`
  repeatedly pays this every time. It is bounded.
- **Proposal:** check the last byte with `seek(End(-1))`, prune once per day or every N writes,
  and cache `last_auto(Daily)`.

### 15. uqd: every Hyprland event wakes it — low, measured, open

- **Measured:** 500 title events a second cost uqd about 0.5 s of CPU per 15 s (3.5% of a core,
  debug build). It only wants `monitoradded`, `monitorremoved` and `configreloaded`, but
  Hyprland sends every event to every socket2 client.
- **Not the cause:** a line-at-a-time rewrite of the parser measured exactly the same (51–55
  ticks before and after), so the parser isn't the cost and the rewrite was dropped. The cost is
  the per-event read wakeup.
- **Status:** nothing cheap to do here. Check the cost on a release build before acting.

### 16. Bridge and Settings: smaller items — low, open

- **Blocking fsync on the bridge's only thread.** `bridge/feeds.rs:129-146` (`write_private`)
  does `sync_all` and a rename after every stocks or news refresh on the current-thread runtime,
  which stalls all bridge I/O for the fsync. Drop `sync_all` (the cache can be thrown away) or
  use `spawn_blocking`.
- **Provider searches aren't fully bounded.** `bridge/providers.rs:461` spawns one task per
  provider per keystroke outside `MAX_IN_FLIGHT`, and `owned()` runs outside the 300 ms
  timeout. Abort the previous generation, or check `latest()` before calling.
- **A dead widget subscription isn't restarted.** In `bridge/widgets.rs:464/866`, a
  subscription whose stream ended keeps its handle in `subs`, so it isn't restarted until the
  owner changes. Drop entries whose handle `is_finished()`.
- **An extra round trip per widget call.** `bridge/widgets.rs:727` calls `activatable()`
  (`ListActivatableNames`) before every `WidgetCall` and `WidgetGet`. Cache it per bus for
  about 30 s.
- **No size cap on weather, geocode and ipinfo bodies.** `bridge/net.rs:137` reads them whole.
  Use `feeds::get_capped`.
- **"Lock before sleep" can stay off.** In `bridge/logind.rs:214-267`, one failed system-bus
  connect or the end of the `PrepareForSleep` stream turns it off until the bridge restarts. Add
  a reconnect loop with backoff.
- **Settings re-reads a whole page per uqd signal.** `apps/uq-settings/src/model_core.rs:166`
  (`follow_uqd`) does a full refresh for each signal of a burst. Use `changed.notify_one()` and
  let the existing 250 ms settle merge them.

### 17. Shell: smaller items — low, open

- **noti-fi socket probed forever when absent.** `notif/Notifi.qml` probes noti-fi's socket
  every 3 s even with the backend set to builtin or mako. The interval is deliberate (a takeover
  must be found within seconds), so probe only while `notif.backend` is `auto` or `notifi`.
- **The "+N" list loses its keyboard selection on every change.** The list popup
  (`WorkIsland.qml:958`) recreates its rows on any change, so the selection jumps back to row 0.
  Key the rows by address, and focus row 0 only on open.
- **`Bridge.call` never times out.** A dropped reply leaves `pending` and the caller's `.then`
  waiting until the bridge restarts. Add deadlines, as `Notifi` has.
- **Tray icons recreated on status changes.** `TrayIsland.qml:22` rebuilds every icon delegate
  when one item's status changes.
- **The VPN poll never stops.** `VpnStatus.qml:61` keeps its 10 s poll running after the Status
  island is removed from every bar.
- **The ticker redraws under a fullscreen window.** The Marquee animation runs at refresh rate
  even when a fullscreen window covers the bar.

### 18. uqd: minor items — low, open

- **`ApplyMonitors` revert sleepers.** `dbus.rs:332` leaves one 15 s task per call. It's
  harmless; the handle could be kept and aborted.
- **`CaptureBroker` timer race.** `capture.rs:76-97` has a narrow race between an expiring timer
  and a new `pause()`. It fails safe: shortcuts come back on.
- **After reconnecting, socket2 doesn't resync.** Monitor events missed during the gap aren't
  re-checked. Send one `outputs` burst after each reconnect.
- **`pick.rs:119` fetches `j/clients` twice.**
- **`power.rs` opens a system-bus connection per call.** It's user-triggered only.

## Not covered

- Monitor hot-plug under load (see "How it was run").
- greeter and lock were reviewed lightly. They run once per login or lock and keep no
  long-lived state.
- Release-build numbers.
