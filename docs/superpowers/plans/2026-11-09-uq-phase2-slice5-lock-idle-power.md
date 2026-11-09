# uQ Phase 2 slice 5: lock, sleep, idle, power menu and polkit — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:**
- `SUPER+L`, the power menu, `loginctl lock-session`, idle and sleep all lock the session with
  `uq-lock`: a separate Quickshell instance that unlocks with a password or a fingerprint.
- The machine never sleeps before the lock is secure.
- Idle dims, locks, turns screens off and, on battery, suspends.
- `SUPER+Escape` opens a power menu, and admin prompts from apps get a polkit dialog.

**Architecture:**
- **Bridge (Rust):**
  - talks to logind on the system bus: a sleep-delay inhibitor, `PrepareForSleep`, the
    session's `Lock` signal and `SetLockedHint`;
  - binds `hyprland-lock-notify-v1` on the Wayland display for the true lock state;
  - gains `Lock` (start `uq-lock.service`) and `SleepLockReady` calls.
- **`lock/`:** a second Quickshell config, run by `uq-lock.service`. It owns `WlSessionLock`
  and the PAM contexts, so passwords never enter the main shell.
- **Main shell:**
  - `shell/session/Session.qml`: the lock and sleep flow, and crash recovery;
  - `shell/idle/`: the idle stages and the dim overlay;
  - `shell/power/`: the power menu (a fifth `Surfaces` surface) and the polkit agent.
- **Pure logic, tested with qmltestrunner:** `shell/logic/lock.js`, `idle.js` and `power.js`.

**Tech Stack:**
- Quickshell 0.3.1: `Quickshell.Wayland` (`WlSessionLock`, `WlSessionLockSurface`,
  `IdleMonitor`), `Quickshell.Services.Pam` (`PamContext`), `Quickshell.Services.Polkit`
  (`PolkitAgent`, `AuthFlow`), `Quickshell.Services.UPower`, `Quickshell.Services.Mpris`.
- Rust edition 2024, zbus 5, wayland-client 0.31 and wayland-scanner 0.31 (both already in the
  local cargo cache).

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase2-shell-design.md`. The relevant parts are
§1 (lock and idle decisions, success criteria), §2.1 (units, recovery bind), §2.2, §2.3
(`SleepLockReady`, `Lock`, `PrepareForSleep`, `LockState`), §2.7, §2.10, §2.12 (`SUPER+L`,
`SUPER+Escape`, IPC `lock`), §2.13, §3, §4 and §5.

## Global Constraints

- **Lock safety (§1, §2.7):**
  - Killing uq-shell or the lock process never exposes the session, and the lock comes back
    on its own.
  - No IPC function can unlock, and `uq-lock` exposes none.
  - Passwords and PAM messages are never logged.
  - Hyprland keeps the session locked while no lock client exists (`allow_session_lock_restore`
    is already set in base.lua).
- **PAM (§2.7):**
  - `/etc/pam.d/uq-lock`: `auth include system-local-login` and
    `account include system-local-login`.
  - `/etc/pam.d/uq-lock-fingerprint`: `auth required pam_fprintd.so max-tries=3 timeout=30`,
    with no password module. Both are in the PKGBUILD's `backup=()`.
  - A missing file fails closed.
  - The password context answers only echo-off prompts, with the typed password.
  - The fingerprint context starts only when `fprintd-list $USER` lists a finger, and never
    answers a prompt: any `responseRequired` aborts it. It restarts at most once every 10 s, and
    also on resume. Whichever context succeeds first unlocks.
  - `PamContext.message` is shown as plain text, including faillock lockouts.
  - The field sets `Qt.ImhSensitiveData | Qt.ImhNoPredictiveText | Qt.ImhHiddenText` and is
    cleared after each attempt.
- **Lock state (§2.7):**
  - The bridge binds `hyprland-lock-notify-v1`; `LockState {locked}` is the truth.
  - If locked and `uq-lock.service` isn't active, the shell starts it.
  - The bridge calls logind `SetLockedHint`.
  - The session's `Lock` signal triggers a lock.
  - On lock, the shell closes and wipes Spotlight, the app menu, quick settings, the
    notification centre, the power menu and any popups. Open requests are ignored while locked.
- **Sleep (§2.7):**
  - The bridge always holds `Inhibit("sleep", "uQ", "Lock the screen before sleep", "delay")`.
  - On `PrepareForSleep(true)` the shell locks. Once the lock is secure (the lock process
    reports `WlSessionLock.secure`), the shell calls `SleepLockReady` and the bridge releases
    the inhibitor.
  - A 2 s fallback releases it anyway.
  - A fresh inhibitor is taken on `PrepareForSleep(false)`.
- **Idle (§2.7):** one `IdleMonitor` per stage, `respectInhibitors: true`.
  - Defaults: dim 240 s, lock 300 s, screen-off 360 s, suspend on battery 1200 s, and screens off
    30 s after locking (`idle.locked_screen_off`).
  - Dim is a 60% black overlay layer per screen, namespace `uq-dim`; input removes it.
  - Screen-off uses `Dispatch dpms {on: false}`.
  - Suspend only when UPower reports discharging and the lock is secure.
  - Mpris playing blocks dim and screen-off only, unless `idle.media_blocks_lock`. Stay awake
    blocks every stage.
  - A stage set to `null` is disabled.
- **Power menu (§2.13):**
  - Lock, log out, suspend, restart and shut down. Restart and shut down have a confirm step
    with a 30 s countdown.
  - Log out uses the bridge's existing `Power logout` (`hyprshutdown` or `hl.dsp.exit()`).
  - Opened by `SUPER+Escape` and from the app menu's bottom row.
- **Polkit (§2.13):**
  - `Quickshell.Services.Polkit`. The action id and message are plain text.
  - The dialog opens on the focused monitor above normal windows.
  - The password field uses the lock screen's hints.
- **Keybinds (§2.12):**
  - `SUPER+L` → `uq:lock`, `SUPER+Escape` → `uq:power-menu`, both not locked.
  - The existing `SUPER+CTRL+L` recovery bind stays.
  - IPC: `qs ipc … call uq lock` locks (it's ignored when already locked).
- **Units (§2.1):** `uq-lock.service` has `ExecStart=/usr/bin/qs -p /usr/share/uq/lock`,
  `Restart=always`, `RestartSec=1`, `StartLimitIntervalSec=0`, `KillMode=process`,
  `PartOf=graphical-session.target`, the same Hyprland `ExecCondition` as uq-shell, and no
  `[Install]` section. `lock/` sets `Quickshell.watchFiles = false`.
- **Trust and text:** all text goes through `UqText` or `UqInput`. `scripts/lint-shell.sh` must
  pass and now also covers `lock/`.
- **Process and test rules (Phase 2):**
  - Never run the bridge, the shell or the lock against the real session or system bus, the
    user's desktop or `hyprctl`. Never lock the user's session.
  - Private dbus-daemon socket paths must be under 108 bytes.
  - Use offscreen `/usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml`, and check its
    exit code directly.
  - Don't run `tests/shell/bar_loads.sh` or `scripts/smoke-shell.sh` locally.
  - `bar_loads.sh` may only instantiate window-free components.
  - Singletons using `Quickshell*` imports live outside `shell/core`.
  - Rust: `cargo fmt`, `clippy -D warnings` and `cargo test --workspace` must all be clean.

## Review Focus

1. **The shell dies while locked, or the lock dies.** On restart, the shell learns `locked` from
   lock-notify and starts `uq-lock.service` if it isn't active. A clean `uq-lock` exit only
   happens after a successful unlock, so a restart loop never re-locks an unlocked session.
   → `test_recovery_decision` (Task 5), unit-file checks (Task 4).
2. **Sleep with no lock.** If the lock never becomes secure, sleep goes ahead after 2 s (logind
   would force it at 5 s anyway), and on resume the lock is there. A `SleepLockReady` without a
   sleep pending is a no-op, and a second `PrepareForSleep(true)` doesn't stack inhibitors.
   → `sleep_fallback_releases`, `ready_without_sleep_is_noop` (Task 1).
3. **Fingerprint stack with no reader, or no fprintd.** It must fail and never succeed;
   `fprintd-list` errors mean "no finger"; the password still works.
   → `test_fprint_listed` (Task 3), `pam_stacks.sh` (Task 4).
4. **Idle edge cases.** Media playing with `media_blocks_lock` false still locks. Suspend never
   fires on AC, or while the lock isn't secure. Stay awake blocks everything. Disabled stages
   (`null`) never fire. → `test_idle_*` (Task 6).
5. **An accidental power-menu click.** A countdown confirm can be cancelled by Esc, a click
   outside or lock, and a cancelled countdown never fires later.
   → `test_countdown_*` (Task 7).

---

### Task 1: Bridge: logind sleep inhibitor, session lock signal and the `Lock` call

**Files:**
- Create: `crates/uqctl/src/bridge/logind.rs`
- Modify: `crates/uqctl/src/bridge/mod.rs`
- Test: `crates/uqctl/src/bridge/logind.rs` (tests module, a private dbus-daemon serving a fake
  `org.freedesktop.login1`)

**Interfaces:**
- **Events:**
  - `PrepareForSleep {sleeping: bool}`
  - `LockRequested {}`, sent when the session's `Lock` signal arrives
- **Calls:**
  - `SleepLockReady` → `null`. It releases the held delay inhibitor if a sleep is pending, and
    is a no-op otherwise.
  - `Lock` → `null`. It runs `systemctl --user start uq-lock.service` (fixed argv, as in
    `power.rs`). An error is returned as `err`.
- **Inhibitor:** taken at start with `Inhibit("sleep", "uQ", "Lock the screen before sleep",
  "delay")`. The returned fd is held. On `PrepareForSleep(true)` the bridge emits the event
  and starts a 2 s fallback (`SLEEP_FALLBACK_MS = 2000`) that drops the fd. On
  `PrepareForSleep(false)` it takes a fresh one, if none is held, and emits the event.
- **Session:** `GetSessionByPID(own pid)`, falling back to `XDG_SESSION_ID` →
  `GetSession(id)`. The bridge subscribes to the session's `Lock`, and exposes
  `pub async fn set_locked_hint(bool)` for Task 2.
- **System bus:** `zbus::Connection::system()`. The address comes from
  `DBUS_SYSTEM_BUS_ADDRESS` in tests. If the bus or logind is missing, log once to stderr; the
  calls still answer (`SleepLockReady` → `null`; `Lock` still runs systemctl).

- [ ] **Step 1: Tests**, with a fake logind (a zbus object server on a private bus):
  - `inhibit_taken_at_start`
  - `sleep_emits_event_and_ready_releases`: the fake sees the fd close
  - `sleep_fallback_releases`: no ready call, the fd is closed by about 2 s
  - `resume_retakes`
  - `ready_without_sleep_is_noop`
  - `double_prepare_does_not_stack`
  - `lock_signal_emits_lock_requested`
  - `locked_hint_forwarded`
  - `no_logind_degrades`: the calls still answer
  - The `Lock` argv is a unit test on the mapping function.
- [ ] **Step 2: Run, see them fail.** `cargo test -p uqctl logind`
- [ ] **Step 3: Implement.** Spawn the watcher next to the NotifOwner watch in `mod.rs`.
- [ ] **Step 4: fmt, clippy and `cargo test --workspace`; all pass.**
- [ ] **Step 5: Commit** `feat(bridge): logind sleep inhibitor, session lock and Lock call`.

### Task 2: Bridge: `hyprland-lock-notify-v1` → `LockState`

**Files:**
- Create: `crates/uqctl/src/bridge/locknotify.rs`,
  `crates/uqctl/protocols/hyprland-lock-notify-v1.xml` (vendored from hyprwm/hyprland-protocols,
  BSD-3-Clause; keep the copyright header)
- Modify: `crates/uqctl/Cargo.toml` (`wayland-client = "0.31"`, `wayland-scanner = "0.31"`),
  `crates/uqctl/src/bridge/mod.rs`, `README.md` (credits line)
- Test: `crates/uqctl/src/bridge/locknotify.rs`

**Interfaces:**
- **Event:** `LockState {locked: bool}`. Sent once lock-notify reports a state (Hyprland sends
  `locked` immediately if the session is already locked), then on every change.
- **Hello:** `Hello.locked` is the state known at Hello time, normally `false` before the first
  event. The shell treats `LockState` as authoritative.
- Every change calls Task 1's `set_locked_hint`.
- **Runtime:** a blocking Wayland event loop on its own thread (`std::thread`) feeding a tokio
  channel. With no `WAYLAND_DISPLAY`, or no `hyprland_lock_notifier_v1` global, it logs once
  and sends nothing.
- `fn lock_event(ev) -> Option<bool>` maps protocol events to the state (pure, unit-tested).

- [ ] **Step 1: Tests:**
  - `lock_event_maps`
  - `no_display_is_quiet`: with `WAYLAND_DISPLAY` unset, the watcher returns and sends no event
  - `hello_reports_known_state`, which may use an injected initial state
- [ ] **Step 2: Run, see them fail.**
- [ ] **Step 3: Implement** with `wayland_scanner::generate_client_code!` on the vendored XML.
- [ ] **Step 4: fmt, clippy and test; all pass.**
- [ ] **Step 5: Commit** `feat(bridge): LockState from hyprland-lock-notify`.

### Task 3: Lock logic (`lock.js`)

**Files:**
- Create: `shell/logic/lock.js`
- Test: `tests/qml/tst_lock.qml`

**Interfaces** (pure; `.pragma library`):
- `FPRINT_RETRY_MS = 10000`.
- `fprintListed(out, exitCode) → bool`: true only for exit 0, with at least one line matching
  `^\s*-\s*#\d+:` (fprintd-list's finger lines). "No fingers enrolled", "No devices available",
  errors and non-zero exits are all false.
- `avatarSource(accountsIcon, faceExists, home) → {path}` or `{initial}`. Order: the
  AccountsService icon path if non-empty, then `home + "/.face"` if it exists, then the user's
  initial.
- `initialOf(name) → string`: the first code point, uppercased, or `"?"`.
- `fprintCanRestart(lastStartMs, nowMs) → bool`: at least `FPRINT_RETRY_MS` since the last
  start.
- `passwordReply(responseRequired, responseVisible, typed) → string|null`: the typed text only
  for an echo-off prompt (`responseRequired && !responseVisible`), else null.
- `fprintAbortOnPrompt(responseRequired) → bool`: always `responseRequired`.
- `messageView(message, isError) → {text, error}`: `cleanText` caps at 200; empty when there's
  no message.

- [ ] **Step 1: Tests:** `test_fprint_listed` (real-format outputs: enrolled, none, no device,
  exit 1), `test_avatar_order`, `test_initial_of` (emoji, empty, lowercase),
  `test_fprint_restart_limit`, `test_password_reply` (echo-on prompt → null),
  `test_fprint_abort`, `test_message_view`.
- [ ] **Step 2: Run, see them fail. Step 3: Implement. Step 4: Run, pass.**
- [ ] **Step 5: Commit** `feat(shell): lock-screen logic`.

### Task 4: `uq-lock`: the lock instance, its unit and PAM stacks

**Files:**
- Create: `lock/shell.qml`, `lock/LockSurface.qml`, `lock/Auth.qml`,
  `packaging/uq-lock.service`, `packaging/pam/uq-lock`, `packaging/pam/uq-lock-fingerprint`,
  `tests/shell/pam_stacks.sh`
- Modify: `packaging/uq/PKGBUILD`, `tests/shell/units_test.sh` (also check uq-lock),
  `scripts/lint-shell.sh` and `tests/shell/lint_test.sh` (also scan `lock/`)

**Interfaces:**
- **`lock/shell.qml`:**
  - `ShellRoot` with `Quickshell.watchFiles = false`, a `ThemeIo {}`, and
    `WlSessionLock { id: lock; locked: true; surface: LockSurface {} }`.
  - When `lock.secure` becomes true, it runs
    `Quickshell.execDetached(["qs", "ipc", "-p", <sibling shell dir>, "call", "uq", "lockSecure"])`,
    where the sibling dir is `Quickshell.shellDir` + `"/../shell"`, normalised.
  - On a successful unlock: `lock.locked = false`, then `Qt.quit()` (exit 0).
- **Imports:** `../shell/core` (Theme, ThemeIo, UqText, UqInput) and `../shell/logic/lock.js`.
  It never imports Bridge, Settings or anything with a Process.
- **`LockSurface`:** a clock, the date, the avatar (`lock.js avatarSource`; the AccountsService
  icon is `/var/lib/AccountsService/icons/$USER`), the user name, a password `UqInput` with the
  Global Constraints hints, the PAM message, and a fingerprint hint when the fingerprint context
  is running. Every control has an `Accessible.name`.
- **`Auth.qml`:** two `PamContext`s:
  - `config: "uq-lock"` for the password, started on Enter;
  - `config: "uq-lock-fingerprint"` for the fingerprint, started when `fprintd-list $USER` (run
    once at start and on retry) says `fprintListed`, restarted per `fprintCanRestart`.
  - Either one's success → `unlocked()`. The field clears after each attempt.
  - Nothing is logged except the result enum names.
- **`uq-lock.service`:** the Global Constraints unit, plus `RestartPreventExitStatus=0`. A clean
  exit happens only after unlock, so it isn't restarted. A crash or `kill -9` is restarted.
- **PKGBUILD:**
  - Installs `lock/`, the unit and the PAM files (to `/etc/pam.d/`, mode 644), and adds
    `backup=(etc/pam.d/uq-lock etc/pam.d/uq-lock-fingerprint)`.
  - optdepends gains `fprintd` and `hyprlock` (if not already there).
  - Ruling: no `pamtester` (it's AUR-only). `pam_stacks.sh` uses `PamContext` itself.
- **`pam_stacks.sh`** (CI and local, offscreen `qs`, prints "skipped" and exits 0 when `qs` is
  missing): copies both stacks into a temp `configDirectory`, starts a `PamContext` on each and
  logs the results.
  - The fingerprint stack must not succeed. On CI there's no fprintd module, so it errors.
  - The password stack, answered with a wrong password, must fail.
  - Neither may ever report success.

- [ ] **Step 1: Write `pam_stacks.sh` and the unit checks; run them and see them fail.**
- [ ] **Step 2: Implement the lock files, unit, PAM files and PKGBUILD changes.**
- [ ] **Step 3:** run `pam_stacks.sh`, `units_test.sh`, `lint-shell.sh` and `lint_test.sh`, and
  qmllint `lock/*.qml` with `-I shell`; all pass. Don't start `lock/shell.qml` on this desktop:
  it would lock the user's session.
- [ ] **Step 4: Commit** `feat(lock): uq-lock instance, unit and PAM stacks`.

### Task 5: Session flow in the shell: lock, recovery, sleep and keybinds

**Files:**
- Create: `shell/session/Session.qml` (singleton), `shell/session/qmldir`
- Modify: `shell/core/Bridge.qml` (`LockState` sets `locked`; signals `prepareForSleep(bool)`
  and `lockRequested()`), `shell/logic/lock.js` (`recoveryDecision`), `shell/launcher/LauncherRoutes.qml`
  (GlobalShortcut `lock`; IPC `lock()` and `lockSecure()`), `shell/core/Surfaces.qml` (lock
  also wipes `notifcentre`; it already does), `shell/shell.qml`, `config/hypr/base.lua`,
  `crates/uqd/tests/verify_config.rs`
- Test: `tests/qml/tst_lock.qml`, `tests/qml/tst_bridge_protocol.qml`

**Interfaces:**
- **`lock.js recoveryDecision(locked, unitActive, starting) → "start"|"none"`:** `"start"` when
  `locked && !unitActive && !starting`.
- **`Session`:**
  - Properties: `secure` (true after a `lockSecure()` while locked; false on unlock),
    `sleepPending`.
  - `lock()`: calls `Bridge.call("Lock")` unless already locked.
  - `lockSecure()`: sets `secure` while `Bridge.locked`. If `sleepPending`, it calls
    `SleepLockReady` and clears `sleepPending`.
  - `onPrepareForSleep(true)`: sets `sleepPending` and calls `lock()`. If already
    `secure`, it calls `SleepLockReady` at once.
  - `onLockRequested`: calls `lock()`.
  - **Recovery:** on `LockState` locked, and every 3 s while locked, it runs
    `systemctl --user is-active uq-lock.service` (fixed argv) and starts the unit (through
    `Bridge.call("Lock")`) per `recoveryDecision`.
- **Keybinds and IPC:**
  - GlobalShortcut `lock` → `Session.lock()`.
  - IPC `lock(): string` → `"ok"` or `"ignored"` (when already locked).
  - IPC `lockSecure(): string` → `"ok"` (it only ever marks secure while locked; it can't
    unlock anything).
- **`base.lua`:** `hl.bind(mainMod .. " + L", hl.dsp.global("uq:lock"))`, not locked. Remove
  any older `SUPER+L` bind. `verify_config.rs` asserts it.

- [ ] **Step 1: Tests:**
  - `test_recovery_decision`;
  - `test_lockstate_event` (Bridge `LockState` sets `locked`; `Hello.locked` is an initial
    value only);
  - `test_sleep_signals` (`PrepareForSleep` and `LockRequested` emit their signals);
  - the `verify_config` bind test.
- [ ] **Step 2: Run, see them fail. Step 3: Implement. Step 4: Run all; pass.**
- [ ] **Step 5: Commit** `feat(shell): session lock flow, recovery and SUPER+L`.

### Task 6: Idle stages and the dim overlay

**Files:**
- Create: `shell/logic/idle.js`, `shell/idle/Idle.qml`, `shell/idle/Dim.qml`
- Modify: `shell/shell.qml`
- Test: `tests/qml/tst_idle.qml`

**Interfaces:**
- **`idle.js stages(settings) → {dim, lock, screenOff, suspend, lockedOff}`:** seconds or null,
  from `Settings.values.idle` (keys `dim`, `lock`, `screen_off`, `suspend_battery`,
  `locked_screen_off`, `media_blocks_lock`), with the defaults when missing.
- **`idle.js enabled(stage, ctx) → bool`,** where
  `ctx = {locked, secure, media, mediaBlocksLock, onBattery}`:
  - `dim`, `screenOff`: `!locked && !media`;
  - `lock`: `!locked && (!media || !mediaBlocksLock)`;
  - `suspend`: `onBattery && locked && secure`;
  - `lockedOff`: `locked`.
  - A null stage is never enabled. Stay awake is handled by `respectInhibitors` (the bar's
    `IdleInhibitor`).
- **`idle.js action(stage) → "dim"|"lock"|"dpms-off"|"suspend"`.**
- **`Idle.qml`:**
  - One `IdleMonitor` per stage (`timeout` in seconds, `respectInhibitors: true`,
    `enabled: I.enabled(...)`).
  - On becoming idle: dim → `Dim` visible; lock → `Session.lock()`; screen-off and locked-off →
    `Bridge.call("Dispatch", ["dpms", {on: false}])`; suspend → `Bridge.call("Power", ["suspend"])`.
  - Media is `Mpris.players.values.some(p => p.isPlaying)`; battery is `UPower.onBattery`.
- **`Dim.qml`:** `Variants` over `Quickshell.screens`. Each is a PanelWindow on layer Overlay,
  namespace `uq-dim`, fullscreen, `color: Qt.rgba(0,0,0,0.6)`, with no keyboard focus and
  exclusion Ignore. It's visible while the dim monitor `isIdle`, and goes away when idle ends.
  With `ui.reduce_motion`, there's no fade.

- [ ] **Step 1: Tests:** `test_idle_stages_defaults` (missing and null keys),
  `test_idle_media_rule` (both `media_blocks_lock` values), `test_idle_suspend_gate` (AC,
  battery without secure, battery with secure), `test_idle_locked_off`, `test_idle_action`.
- [ ] **Step 2: Run, see them fail. Step 3: Implement. Step 4: Run, pass;** qmllint with no new
  warnings.
- [ ] **Step 5: Commit** `feat(shell): idle stages and dim overlay`.

### Task 7: Power menu

**Files:**
- Create: `shell/logic/power.js`, `shell/power/PowerMenu.qml`
- Modify: `shell/logic/launcher.js` (`SURFACES` gains `"powermenu"`, event `togglePowerMenu`),
  `shell/core/Surfaces.qml` (toggle, lock wipe, `ipc` accepts `powermenu`),
  `shell/launcher/LauncherRoutes.qml` (GlobalShortcut `power-menu`), `shell/launcher/AppMenu.qml`
  and `shell/logic/appmenu.js` (restart and shut down open the power menu at that action's
  confirm step instead of the 5 s double-click), `shell/launcher/Launchers.qml` or
  `shell/shell.qml`, `config/hypr/base.lua`, `crates/uqd/tests/verify_config.rs`, `README.md`
- Test: `tests/qml/tst_power.qml`, `tests/qml/tst_launcher.qml`, `tests/qml/tst_surfaces.qml`,
  `tests/qml/tst_appmenu.qml`

**Interfaces:**
- `power.js`: `COUNTDOWN_S = 30`, `ACTIONS = ["lock", "logout", "suspend", "reboot", "poweroff"]`
  (the bridge's `Power` names), `needsConfirm(action)` (reboot and poweroff),
  `startConfirm(action, nowMs) → {action, deadline}`,
  `remaining(confirm, nowMs) → int` seconds (ceil, ≥ 0), `due(confirm, nowMs) → bool`,
  `cancel() → null`.
- `Surfaces.toggle("powermenu", screen, anchor, initialConfirm?)`: `initialConfirm` is an action
  name. `PowerMenu` reads it from `Surfaces.powerConfirm` when it opens.
- **`PowerMenu`:**
  - A centred panel on the focused monitor (layer Top, namespace `uq-power`,
    `HyprlandFocusGrab`, Esc closes).
  - Five buttons. `lock` → `Session.lock()`; the others → `Bridge.call("Power", [action])`.
  - Reboot and poweroff show "Restarting in N s" / "Shutting down in N s" with "Restart now" /
    "Shut down now" and "Cancel". The action runs when `due`.
  - Closing the surface, Esc, a click outside or lock cancels the countdown. A 1 s Timer runs
    only while a confirm is pending.
  - Every button has an `Accessible.name`.
- `base.lua`: `hl.bind(mainMod .. " + Escape", hl.dsp.global("uq:power-menu"))`, not locked.

- [ ] **Step 1: Tests:** `test_countdown_remaining` (30 → 0, never negative),
  `test_countdown_due`, `test_needs_confirm`, `test_countdown_cancel` (a cancelled confirm is
  never due), `test_transition_powermenu`, `test_ipc_powermenu`, the app-menu tests updated for
  the new restart and shut-down behaviour, and the verify_config bind test.
- [ ] **Step 2: Run, see them fail. Step 3: Implement. Step 4: Run all; pass.**
- [ ] **Step 5: Commit** `feat(shell): power menu with a 30 s confirm`.

### Task 8: Polkit agent, README and the live checklist

**Files:**
- Create: `shell/power/PolkitAgent.qml`, `docs/notes/lock-idle-live-check.md`
- Modify: `shell/shell.qml`, `README.md`

**Interfaces:**
- **`PolkitAgent.qml`:**
  - A `PolkitAgent {}` and a PanelWindow (layer Overlay, namespace `uq-polkit`, keyboard focus
    Exclusive, on `Hyprland.focusedMonitor`), visible while `agent.isActive && agent.flow`.
  - Shows `flow.message`, `flow.actionId`, `flow.supplementaryMessage` (error styled when
    `supplementaryIsError`), the identity when there's more than one (a selector over
    `flow.identities`), `flow.inputPrompt` and a `UqInput` with `echoMode` following
    `flow.responseVisible` and the lock-field hints.
  - Enter → `flow.submit(text)`, then clear. Cancel or Esc → `flow.cancelAuthenticationRequest()`.
  - All of it is plain text through `cleanText`, with the message at most 500 chars and the
    action id at most 200.
  - Nothing is logged.
- **README:** a Lock, idle and power section covering:
  - `SUPER+L`, `SUPER+Escape` and `SUPER+CTRL+L`;
  - fingerprint setup (`fprintd-enroll`);
  - the PAM files and that they're `backup`;
  - the idle defaults and `uqctl idle set`;
  - lock before sleep;
  - the power menu countdown;
  - the polkit agent, and that another running polkit agent takes precedence (the first to
    register wins);
  - the lock-notify protocol credit.
- **`docs/notes/lock-idle-live-check.md`:** a manual checklist for the first real Hyprland run:
  - lock and unlock with password and fingerprint;
  - `kill -9` the lock and the shell while locked;
  - suspend from the power menu and from logind while unlocked (lock shows on resume);
  - each idle stage with short `uqctl idle set` values;
  - media playing;
  - battery suspend;
  - a polkit prompt (`pkexec true`);
  - a faillock lockout message.

- [ ] **Step 1: Implement `PolkitAgent.qml`** and add it to `shell.qml`. It's not in
  bar_loads (it's a window).
- [ ] **Step 2: qmllint** with no new warnings; lint-shell passes; qmltestrunner passes.
- [ ] **Step 3: Write the README section and the checklist.**
- [ ] **Step 4: Commit** `feat(shell): polkit agent; docs for lock, idle and power`.
