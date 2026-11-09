# uQ Phase 2 slice 3: quick settings and OSD — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Volume, mic and brightness keys change the real setting and show an OSD (noti-fi's when it
answers, uQ's own otherwise), and `SUPER+A` or the Status island's button opens a quick-settings
panel with four groups.

**Architecture:**
- Pure logic lives in `shell/logic/quick.js` and `shell/logic/notifi.js`, tested with qmltestrunner.
- Three thin singletons wrap the system:
  - `shell/core/Brightness.qml`: `brightnessctl`, fixed argv;
  - `shell/notif/Notifi.qml`: one long-lived `notifd.sock` connection;
  - `shell/osd/OsdRouter.qml`: routes an OSD to noti-fi, or to the fallback window.
- Quick settings is a third launcher-style surface in the existing `Surfaces` state machine, with
  one QML file per group.
- The one Rust change is a `GetSchedule` read-back in uqd and its bridge passthrough.

**Tech Stack:**
- Quickshell 0.3.1, with these modules: `Quickshell.Services.Pipewire`,
  `Quickshell.Services.UPower` (`PowerProfiles`), `Quickshell.Networking`, `Quickshell.Bluetooth`,
  `Quickshell.Io` (`Socket`, `Process`), and `Quickshell.Wayland` (`IdleInhibitor`).
- Rust edition 2024, zbus 5.

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase2-shell-design.md`. The relevant parts are
§1 (quick settings, OSD, success criteria), §2.2, §2.5–2.6 (noti-fi), §2.10, §2.12 and §3.

## Global Constraints

- **Trust model (spec §1):** SSIDs, Bluetooth names, Pipewire node names and noti-fi replies are
  untrusted text.
  - All shell text goes through `UqText`, and `scripts/lint-shell.sh` must pass.
  - Nothing is loaded from the network.
  - Icons are icon-theme names or local paths only (`launcher.js iconRef`).
- **Keybinds (§2.12):**
  - `XF86AudioRaiseVolume`/`LowerVolume`/`Mute`/`MicMute` → `uq:vol-up`, `uq:vol-down`,
    `uq:mute`, `uq:mic-mute`.
  - `XF86MonBrightnessUp`/`Down` → `uq:bright-up`, `uq:bright-down`.
  - All six are `locked = true` and `repeating = true`. They replace the Phase 1 `wpctl` and
    `brightnessctl` exec binds.
  - `SUPER+A` → `uq:quick-settings`, not locked.
- **OSD (§2.6, §2.12):**
  - With noti-fi, the request is
    `osd {kind: "volume"|"mic"|"brightness", value, max (default 100), muted}`, one line per
    key-repeat step, and the reply is `{"ok":true}`.
  - The fallback OSD is used when noti-fi is unavailable or the request fails.
  - It appears on `Hyprland.focusedMonitor`.
  - Pipewire changes in the first 2 s after startup are ignored.
- **noti-fi socket (§2.6):**
  - The path is `$XDG_RUNTIME_DIR/notifd.sock`, over one long-lived connection.
  - The first line must be `{"event":"hello","name":"notifd","protocol":1,…}`. Anything else
    means noti-fi is unavailable.
  - Requests are `{"id":N,"method":"<m>","params":{…}}`.
  - uQ applies its own cap of 64 KiB per line and drops longer lines.
- **Brightness (§2.2):** only `brightnessctl`, run from shell actions with a fixed argv. If it is
  missing or there is no backlight device, the widget and keys hide or no-op, with no error loop
  (§4).
- **Focus and placement (§2.10):**
  - Quick settings opens on `Hyprland.focusedMonitor`, or on the Status island's screen when
    opened from its button.
  - It uses `HyprlandFocusGrab`, has at most one launcher-style surface open, and closes and wipes
    on lock.
  - Popups close if their monitor is unplugged.
  - Every control has an `Accessible.name`.
- **Shell IPC (§2.12):** only `open`, `close` and `toggle`, now also for `quicksettings`. These
  are ignored while `Bridge.locked`.
- **Process and test rules (Phase 2):**
  - Never run the bridge or the shell against the real session bus, the user's desktop or
    `hyprctl`.
  - Private dbus-daemon socket paths must be under 108 bytes.
  - Use offscreen `/usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml`.
  - Don't run `tests/shell/bar_loads.sh` or `scripts/smoke-shell.sh` locally; CI runs them.
  - `bar_loads.sh` runs offscreen with no PanelWindow backend, so it may only instantiate
    window-free components.
  - Rust: `cargo fmt`, `clippy -D warnings` and `cargo test --workspace` must all be clean.

## Review Focus

1. **Volume at the ends.** Holding vol-up at 100% stays at 100%; holding vol-down at 0% stays at 0
   and doesn't unmute. A mute key press while at 0 still toggles. → `test_volume_step_clamps`
   (Task 2).
2. **No default sink or source, or one that changes.** Unplugged headphones mean
   `Pipewire.defaultAudioSink` is null or changing. Keys no-op, quick settings shows "No output",
   and the OSD watcher re-binds to the new node without firing an OSD for the switch itself.
   → `test_osd_should_show` covers node switches (Task 2); Task 4 binds through
   `PwObjectTracker`.
3. **No brightness device.** On a desktop with external monitors, `brightnessctl -m` exits
   non-zero or lists no `backlight` class. The brightness slider hides, and the keys do nothing
   without spawning a process per key-repeat. → `test_brightness_parse` covers non-backlight and
   garbage output (Task 2); `Brightness.available` gates the action (Task 4).
4. **noti-fi restarts or misbehaves.**
   - The socket drops mid-session: the OSD falls back at once, and the client reconnects with
     `backoff.js`.
   - A non-hello first line, an oversized line, a non-JSON line or a reply with an unknown id is
     ignored.
   → `test_notifi_handle_line` (Task 3).
5. **Untrusted network and Bluetooth names.** Very long or control-character SSIDs and device
   names stay plain text and are elided. A secured network that isn't known can't be connected
   from the shell, because the shell has no password prompt. → `test_wifi_rows_and_connectable`
   (Task 2); the lint (Task 7).

---

### Task 1: uqd `GetSchedule` and bridge passthrough

**Files:**
- Modify:
  - `crates/uqd/src/dbus.rs`
  - `crates/uqd/src/service.rs` (getter)
  - `crates/uqctl/src/bridge/uqd.rs` (request enum and arity table)
  - `crates/uqctl/src/bridge/mod.rs` (passthrough list)
- Test: `crates/uqd/tests/dbus.rs`, `crates/uqctl/tests/bridge_dbus.rs`

**Interfaces:**
- Produces:
  - D-Bus `org.uq.Desktop1.GetSchedule() → s`: the stored `Schedule` as JSON, in the same format
    `SetSchedule` accepts (`{"mode":"off"}` | `{"mode":"fixed","light_at":"HH:MM:SS","dark_at":…}`
    | `{"mode":"sun","lat":…,"lon":…}`).
  - The bridge call `GetSchedule` takes no args and returns that JSON parsed into an object, not a
    string.

- [ ] **Step 1: Failing tests:**
  - In `dbus.rs`, `get_schedule_round_trips`:
    - after `SetSchedule('{"mode":"sun","lat":51.5,"lon":-0.13}')`, `GetSchedule()` parses back
      to `mode == "sun"`, `lat == 51.5`;
    - on a fresh state it returns `{"mode":"off"}`.
  - In `bridge_dbus.rs`, `bridge_get_schedule`: the call returns an object with `mode`.
- [ ] **Step 2:** Run `cargo test -p uqd --test dbus get_schedule` → FAIL (unknown method).
- [ ] **Step 3: Implement.** `Desktop::schedule(&self) -> &Schedule`, and the `get_schedule`
  interface method serialising it with `serde_json`. The bridge passthrough follows the pattern
  of the existing `GetLayouts` entry.
- [ ] **Step 4:** Run `cargo test --workspace` → PASS; fmt and clippy are clean.
- [ ] **Step 5:** Commit `feat(uqd): GetSchedule and its bridge passthrough`.

### Task 2: Quick-settings and OSD logic (`quick.js`)

**Files:** Create `shell/logic/quick.js` and `tests/qml/tst_quick.qml`.

**Interfaces:**
- Produces (pure JS, `.pragma library`, no Quickshell imports):
  - `volumeStep(volume, dir) → number`: `dir` is `+1` or `-1`, the step is 0.05, the result is
    clamped to [0, 1] and rounded to 2 dp. Non-finite input → 0.
  - `osdShouldShow(startedAtMs, nowMs, prevNodeId, nodeId) → bool`: false when
    `nowMs - startedAtMs < 2000`, and false when `prevNodeId !== nodeId` (a device switch, not a
    level change).
  - `brightnessParse(stdout) → {device, current, max, percent} | null`:
    - parses `brightnessctl -m` output (`name,class,current,percent%,max`, one line per device);
    - takes the first line whose class is `backlight`;
    - returns null for none, for garbage, or when `max <= 0`.
  - `brightnessArgs(dir) → string[]`: `["brightnessctl","-m","-e4","-n2","set","5%+"]`, or
    `"5%-"` for `dir < 0`. Reading uses `["brightnessctl","-m"]`; export it as
    `BRIGHTNESS_READ`.
  - `wifiRows(networks) → [{name, signal, connected, known, secure, connectable}]`:
    - input is plain objects (Task 7 maps the Quickshell objects);
    - drops empty names;
    - sorts connected first, then known, then signal descending, then name;
    - `connectable = !connected && (known || !secure)`;
    - at most 20 rows.
  - `scheduleLabel(schedule) → string`:
    - `"Off"`;
    - `"Sunset to sunrise"` for `sun`;
    - `"Dark HH:MM–HH:MM"` for `fixed` (dark_at to light_at, shown as HH:MM);
    - `"Unknown"` for anything else.
  - `nextSchedule(schedule, location) → schedule | null`: the Theme group's toggle.
    - `off` → `{"mode":"sun","lat":location.lat,"lon":location.lon}` when `location` (the
      `weather_location` setting) is non-null, otherwise null (the toggle is disabled);
    - `sun` or `fixed` → `{"mode":"off"}`.
  - `LAYOUTS = ["dwindle","master","scrolling"]`.
  - `batteryLabel(percent, charging, secondsLeft) → string`, e.g. `"82%"`, `"82% · 1 h 20 min"`,
    `"Charging 82%"`. `secondsLeft <= 0` omits the time.
  - `profileLabel(p) → "Power saver"|"Balanced"|"Performance"`. It takes 0, 1 or 2, the
    `PowerProfile` enum order; check the qmltypes.
- [ ] **Step 1: Failing tests (`tst_quick.qml`):**
  - `test_volume_step_clamps`:
    - `volumeStep(0.98, 1) == 1`, `volumeStep(1, 1) == 1`, `volumeStep(0.02, -1) == 0`;
    - `volumeStep(0, -1) == 0`, `volumeStep(0.5, 1) == 0.55`, `volumeStep(NaN, 1) == 0`.
  - `test_osd_should_show`:
    - `(0, 1999, 5, 5)` → false; `(0, 2000, 5, 5)` → true;
    - `(0, 9000, 5, 6)` → false; `(0, 9000, 5, 5)` → true.
  - `test_brightness_parse`:
    - `"intel_backlight,backlight,4800,50%,9600\n"` → `{device:"intel_backlight", current:4800, max:9600, percent:50}`;
    - only `"input0::capslock,leds,0,0%,1"` → null; `""` → null; `"x,backlight,1,1%,0"` → null.
  - `test_brightness_args`: the exact arrays above.
  - `test_wifi_rows_and_connectable`:
    - the order is connected, known, signal;
    - an unknown secured network has `connectable == false`;
    - an open unknown network has `connectable == true`;
    - an empty name is dropped;
    - 25 inputs give 20 rows.
  - `test_schedule_label_and_next`: every case above, including `nextSchedule({mode:"off"}, null) === null`.
  - `test_battery_and_profile_labels`.
- [ ] **Step 2:** Run `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml/tst_quick.qml`
  → FAIL (stub module, failing assertions).
- [ ] **Step 3: Implement** `quick.js`.
- [ ] **Step 4:** Run the same command → PASS. Then run the whole `-input tests/qml`.
- [ ] **Step 5:** Commit `feat(shell): quick-settings and OSD logic`.

### Task 3: noti-fi client (`notifi.js` and `Notifi.qml`)

**Files:**
- Create:
  - `shell/logic/notifi.js`
  - `shell/notif/Notifi.qml` (singleton)
  - `shell/notif/qmldir`
  - `tests/qml/tst_notifi.qml`

**Interfaces:**
- Produces:
  - `notifi.js`:
    - `MAX_LINE = 65536`.
    - `handleLine(state, line) → {state, effect}`. `state` is
      `{greeted: bool, pending: {id: method}}`, and `effect` is one of:
      - `{kind:"hello"}`: first line valid; sets `greeted`.
      - `{kind:"unavailable"}`: first line isn't a valid hello. The caller closes.
      - `{kind:"reply", id, ok: bool, value | error}`: only for a pending id, which is removed.
      - `{kind:"state", dnd: bool}`: a `state` event after the hello. Only `dnd` is taken here;
        slice 4 extends it.
      - `{kind:"ignore"}`: anything else, including lines over `MAX_LINE` bytes, non-JSON, an
        unknown id, or lines before the hello.
    - `request(id, method, params) → string`: one JSON line with no newline.
    - `osdParams(kind, value, max, muted) → object | null`:
      - `kind` must be `volume`, `mic` or `brightness`;
      - `value` is rounded to an integer and clamped to `[0, max]`;
      - `max` is an integer ≥ 1 and defaults to 100;
      - `muted` is a bool;
      - otherwise null.
  - `Notifi.qml` singleton:
    - properties `available` (bool, true after hello) and `dnd` (bool);
    - `function osd(kind, value, max, muted) → handle`. The handle has the same `then`/`catch`
      shape as `Bridge.call`, and rejects at once when the client is unavailable or the params are
      invalid.
    - `function setDnd(on)`: sends `dnd {mode: "on"|"off"}`.
    - Uses `Quickshell.Io` `Socket` on `Quickshell.env("XDG_RUNTIME_DIR") + "/notifd.sock"`, with
      a line parser (check the `Socket`/`SplitParser` qmltypes).
    - On hello it sends `subscribe`.
    - On disconnect or error: `available = false`, every pending call is rejected, and it
      reconnects after `backoff.js` delays.
    - It never writes before the hello.
- [ ] **Step 1: Failing tests (`tst_notifi.qml`):**
  - `test_notifi_handle_line`, each case asserting the `effect.kind`:
    - a valid hello → `hello`;
    - `{"event":"hello","name":"other","protocol":1}` → `unavailable`;
    - a protocol 2 hello → `unavailable`;
    - non-JSON first line → `unavailable`;
    - after a hello, a reply for a pending id → `reply` (with the id removed from `pending`);
    - a reply for an unknown id → `ignore`;
    - a 70,000-char line → `ignore`;
    - a `state` event with `dnd:true` → `state` with `dnd == true`;
    - any line before the hello other than a hello → `unavailable`.
  - `test_request_line`: `request(3,"osd",{kind:"volume"})` parses back to
    `{id:3, method:"osd", params:{kind:"volume"}}` and contains no `\n`.
  - `test_osd_params`:
    - `("volume", 55.4, undefined, false)` → `{kind:"volume", value:55, max:100, muted:false}`;
    - `("volume", 140, 100, false).value == 100`;
    - `("speaker", …)` → null; `("mic", 1, 0, true)` → null.
- [ ] **Step 2:** Run `qmltestrunner … tst_notifi.qml` → FAIL.
- [ ] **Step 3:** Implement `notifi.js`, then `Notifi.qml` as a thin wrapper.
- [ ] **Step 4:** Run the tests → PASS. Run qmllint on `Notifi.qml`; its only allowed warnings
  are ones already present elsewhere in the shell.
- [ ] **Step 5:** Commit `feat(shell): noti-fi socket client`.

### Task 4: Brightness, OSD router and fallback OSD

**Files:**
- Create:
  - `shell/core/Brightness.qml` (singleton, registered in `shell/core/qmldir`)
  - `shell/osd/OsdRouter.qml` (singleton)
  - `shell/osd/Osd.qml` (fallback window)
  - `shell/osd/OsdWatch.qml` (window-free Pipewire watcher)
  - `shell/osd/qmldir`
- Test: extend `tests/qml/tst_quick.qml` only if new pure logic appears.

**Interfaces:**
- Consumes:
  - `quick.js`: `brightnessParse`, `brightnessArgs`, `BRIGHTNESS_READ`, `osdShouldShow`.
  - `Notifi.osd`.
- Produces:
  - `Brightness`:
    - properties `available` (bool) and `percent` (int 0–100);
    - `function step(dir)`. It is a no-op when not `available` or while a step is still running,
      so key-repeat never queues processes.
    - It reads once at startup with `BRIGHTNESS_READ`. Each step runs `brightnessArgs(dir)` and
      parses that step's own output (`-m` prints the new value). It emits
      `changed(int percent)` after a successful step.
    - Never `sh -c`.
  - `OsdRouter.show(kind, value, max, muted)`:
    - `Notifi.available` → `Notifi.osd(...)`, and on rejection the fallback;
    - otherwise it goes straight to the fallback `Osd`, which owns the visible state;
    - `kind`, `value`, `max` and `muted` are exposed as properties for `Osd.qml`.
  - `Osd.qml`:
    - a `PanelWindow` on the focused monitor, layer Overlay, namespace `uq-osd`, no keyboard
      focus, `ExclusionMode.Ignore`, anchored bottom-centre, 240×48;
    - an icon (theme names `audio-volume-*`, `microphone-sensitivity-*`/`-muted`,
      `display-brightness`) and a bar;
    - it hides 1500 ms after the last `show`.
  - `OsdWatch.qml`:
    - `PwObjectTracker` on the default sink and source;
    - on a volume or mute change of the default sink it calls
      `OsdRouter.show("volume", round(vol*100), 100, muted)`, and does the same with `"mic"` for
      the source;
    - both are gated by `osdShouldShow(startedAt, now, prevId, id)`;
    - brightness OSDs come from `Brightness.changed`.
- [ ] **Step 1:** If new branching logic is needed, add pure helpers to `quick.js` with failing
  tests first. Otherwise there are no new tests here, and Task 5 drives these objects.
- [ ] **Step 2:** Implement the four files.
- [ ] **Step 3:** qmllint all four files. The only allowed warnings are the existing PanelWindow
  and `margins` ones on `Osd.qml`. Check every Quickshell name against the qmltypes.
- [ ] **Step 4:** Run the full qmltestrunner and the lint.
- [ ] **Step 5:** Commit `feat(shell): brightness, OSD router and fallback OSD`.

### Task 5: OSD keybinds and routes

**Files:**
- Create `shell/osd/OsdRoutes.qml` (window-free: six `GlobalShortcut`s).
- Modify:
  - `shell/shell.qml` (one `OsdRoutes`, one `OsdWatch`, one `Osd`)
  - `config/hypr/base.lua`
  - `crates/uqd/tests/verify_config.rs`
  - `tests/shell/bar_loads.sh`
  - `README.md`

**Interfaces:**
- Consumes: `Pipewire.defaultAudioSink`/`Source` (`audio.volume`, `audio.muted`); `quick.js`
  `volumeStep`; `Brightness.step`; Task 4 objects.
- Produces:
  - `OsdRoutes`, with `appid: "uq"`:
    - `vol-up` and `vol-down` set `sink.audio.volume = volumeStep(sink.audio.volume, ±1)`. Raising
      the volume while muted unmutes.
    - `mute` toggles `sink.audio.muted`; `mic-mute` toggles `source.audio.muted`.
    - `bright-up` and `bright-down` call `Brightness.step(±1)`.
    - Each is a no-op when its node is null.
    - All of them act on `onPressed`. Repeats come from Hyprland's `repeating`.
    - `routed` is a bool that is true when all six shortcuts exist with the right names.
  - `base.lua`: the six exec binds become
    `hl.bind("XF86…", hl.dsp.global("uq:<name>"), { locked = true, repeating = true })`. The
    `playerctl` binds stay as they are.
  - `bar_loads.sh` instantiates `OsdRoutes`, and the LOADED line gains a sixth field,
    `osd.routed`. Keep the grep in sync.
  - README: the keys and OSD behaviour, and the noti-fi OSD note.
- [ ] **Step 1: Failing tests:**
  - Extend `lua_mock_runs_every_variant`: the six globals are present, each with
    `locked == true` and `repeating == true`, and there are no `wpctl` or `brightnessctl` exec
    binds left.
  - Add `base_lua_osd_binds_are_locked`, which does the same check on the text, line by line.
- [ ] **Step 2:** Run `cargo test -p uqd --test verify_config` → FAIL.
- [ ] **Step 3:** Implement the binds, `OsdRoutes` and the `shell.qml` wiring.
- [ ] **Step 4:** Run the cargo workspace tests, the full qmltestrunner, the lint, the four safe
  shell scripts and qmllint → PASS.
- [ ] **Step 5:** Commit `feat: OSD keybinds through the shell`.

### Task 6: Quick settings in the surface state machine

**Files:**
- Modify:
  - `shell/logic/launcher.js` (`transition`)
  - `shell/core/Surfaces.qml` (`toggle`, `ipc`)
  - `shell/launcher/LauncherRoutes.qml` (a `quick-settings` shortcut and the IPC names)
  - `config/hypr/base.lua`
  - `crates/uqd/tests/verify_config.rs`
  - `tests/qml/tst_launcher.qml`
  - `tests/qml/tst_surfaces.qml`

**Interfaces:**
- Produces:
  - The surface states become `closed|spotlight|appmenu|quicksettings`, with the event
    `toggleQuickSettings`.
  - `Surfaces.toggle("quicksettings", screen, anchor)` and `Surfaces.ipc(verb, "quicksettings")`
    behave exactly as for the other two.
  - `Surfaces.wipe("quicksettings")` is emitted on close and on lock.
  - `GlobalShortcut { appid: "uq"; name: "quick-settings" }` → toggle, with screen null.
  - `base.lua`: `hl.bind(mainMod .. " + A", hl.dsp.global("uq:quick-settings"))`, not locked.
- [ ] **Step 1: Failing tests:**
  - `test_state_machine_single_surface` gains the quicksettings cases: opening it closes
    Spotlight, the wipe is reported, and `lock` closes it.
  - `tst_surfaces` IPC tests gain `quicksettings`.
  - verify_config: the bind exists and isn't `locked`.
- [ ] **Step 2:** Run → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run → PASS.
- [ ] **Step 5:** Commit `feat(shell): quick settings in the surface state machine`.

### Task 7: Quick-settings window, Audio and Power groups, Status button

**Files:**
- Create:
  - `shell/quick/QuickSettings.qml` (the window)
  - `shell/quick/AudioGroup.qml`
  - `shell/quick/PowerGroup.qml`
  - `shell/core/StayAwake.qml` (singleton: `property bool enabled`, session-only)
- Modify:
  - `shell/bar/StatusIsland.qml` (the inert button calls
    `Surfaces.toggle("quicksettings", root.screen, button)`)
  - `shell/bar/Bar.qml` (`IdleInhibitor { window: bar; enabled: StayAwake.enabled }`)
  - `shell/launcher/Launchers.qml` (one `QuickSettings`)

**Interfaces:**
- Consumes:
  - Spotlight's and AppMenu's patterns: `screenFor`, `openedOn`, `HyprlandFocusGrab` with
    `onCleared` → `Surfaces.close()`, wipe on `Surfaces.wipe("quicksettings")` and on
    `Bridge.locked`, and `onScreensChanged` closing the surface.
  - `quick.js`: `volumeStep`, `batteryLabel`, `profileLabel`.
  - `Brightness`; `Notifi.available`, `Notifi.dnd`, `Notifi.setDnd`.
- Produces:
  - `QuickSettings.qml`:
    - layer Top, namespace `uq-quick`, 380 wide, anchored under the Status island when opened
      from it, else top-right of the focused monitor;
    - a scrollable column of four group components. Task 8 adds `NetworkGroup` and `ThemeGroup`
      in the slots this task leaves.
  - `AudioGroup`:
    - an output volume slider and mute toggle;
    - an output device list from `Pipewire.nodes` (sinks only, set via
      `Pipewire.preferredDefaultAudioSink`), labels through `UqText`;
    - mic mute and an input volume slider;
    - a brightness slider, only when `Brightness.available`;
    - "No output" when there is no sink.
  - `PowerGroup`:
    - a power-profile selector, only when `PowerProfiles` reports a daemon (check the qmltypes);
    - battery, only when `UPower.displayDevice` is a battery and present;
    - Do Not Disturb, only when `Notifi.available` (slice 4 generalises this to `NotifBackend`);
    - Stay awake, bound to `StayAwake.enabled`.
- [ ] **Step 1:** If there is new pure logic, add tests first.
- [ ] **Step 2:** Implement.
- [ ] **Step 3:** qmllint every new and changed file; the only allowed warnings are pre-existing
  ones. Run the lint. Check that every control has `Accessible.name`.
- [ ] **Step 4:** Run the full qmltestrunner.
- [ ] **Step 5:** Commit `feat(shell): quick settings with audio and power groups`.

### Task 8: Network and Theme groups

**Files:** Create `shell/quick/NetworkGroup.qml` and `shell/quick/ThemeGroup.qml`, and add them
to `QuickSettings.qml`.

**Interfaces:**
- Consumes:
  - `Networking.wifiEnabled`, the devices' `networks`, `WifiNetwork.signalStrength`/`security`,
    and `Network.known`/`connected`/`connect()`/`disconnect()`;
  - `Bluetooth.defaultAdapter.enabled`, and `devices` (`paired`, `connected`, `connect()`,
    `disconnect()`, `batteryAvailable`, `battery`);
  - `quick.js`: `wifiRows`, `scheduleLabel`, `nextSchedule`, `LAYOUTS`;
  - Bridge calls `ListThemes`, `GetTheme`, `SetTheme`, `GetSchedule`, `SetSchedule`,
    `GetLayouts` and `SetLayout`, plus `Bridge.themeChanged` and `Bridge.layoutChanged`. Check
    the argument shapes against `crates/uqctl/src/bridge/uqd.rs`.
  - `Settings.values.weather_location`.
- Produces:
  - `NetworkGroup`:
    - a Wi-Fi toggle, hidden when `wifiHardwareEnabled` is false;
    - the network list from `wifiRows`. A tap connects when `connectable`, or disconnects when
      connected. A secured network that isn't known shows a plain-text "Set up in Settings" with
      no password prompt.
    - the wired link state;
    - a Bluetooth adapter toggle, and paired devices with connect/disconnect and battery;
    - the whole Bluetooth section hides without an adapter.
  - `ThemeGroup`:
    - a theme picker from `ListThemes`;
    - a dark/light toggle (`SetTheme(currentName, otherVariant)`);
    - the schedule row: `scheduleLabel`, with a toggle via `nextSchedule` that is disabled with
      "Set a weather city first" when it returns null;
    - the current workspace's layout, three buttons from `LAYOUTS` using
      `SetLayout(String(Hyprland.focusedWorkspace.id), l)`;
    - all of it refreshes on `themeChanged` and `layoutChanged`.
- [ ] **Step 1:** If there is new pure logic, add tests first.
- [ ] **Step 2:** Implement.
- [ ] **Step 3:** qmllint, the lint, and the full qmltestrunner.
- [ ] **Step 4:** Commit `feat(shell): network and theme quick-settings groups`. The controller
  pushes and checks CI.
