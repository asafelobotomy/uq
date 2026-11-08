# uQ Phase 2 Slice 1 (Shell Foundation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a running uq-shell on Hyprland. It has a themed bar with the System, Work,
Time/weather and Status islands, and a placeholder for Notifications. It's backed by uqd
settings, the `uqctl bridge` (protocol, Launch, Dispatch, weather, public IP, notification-owner
watch), and new `uqctl` settings commands. It's packaged as `uq-shell.service`.

**Architecture:**
- uq-core gains a validated `Settings` model that uqd persists and serves over D-Bus.
- `uqctl bridge` is a long-lived JSON-lines process. It forwards uqd calls and signals, launches
  apps into their own systemd scopes through Hyprland, maps a closed set of window actions to
  escaped Lua, and does the two network lookups.
- The Quickshell shell keeps decisions in pure JS modules under `shell/logic/`, tested with
  qmltestrunner. Views are thin QML that only use `UqText` for text.

**Tech Stack:**
- Rust (existing workspace): `zbus` 5, `tokio`, `serde_json`, `reqwest` (rustls, json), `clap`.
- Quickshell 0.3.1 (QML, Qt 6). Modules: `Quickshell`, `Quickshell.Io`, `Quickshell.Hyprland`,
  `Quickshell.Wayland`, `Quickshell.Widgets`, `Quickshell.Services.{Mpris,UPower,Pipewire,SystemTray}`,
  `Quickshell.Networking`.
- `qmltestrunner` (qt6-declarative), systemd user units, Hyprland 0.56.2–0.57.x Lua config.

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase2-shell-design.md` (slice 1 of §6). Parent:
`docs/superpowers/specs/2026-11-08-uq-desktop-design.md`.

## Global Constraints

- **Threat model (spec §1):** a single user. Same-user unsandboxed processes are trusted. Every
  string from outside the shell is untrusted: window titles, SSIDs, media metadata, tray tooltips,
  notification content and provider data.
- **Text:**
  - All shell text goes through `UqText` (`textFormat: Text.PlainText`).
  - A bare `Text {` in `shell/` outside `shell/core/UqText.qml` fails CI.
  - No remote resources: icons only from the icon theme or absolute local paths.
- **Dispatch and launch:**
  - The shell never builds Lua or dispatch strings, and never runs an app as its own child.
  - Every launch is `Launch` on the bridge, which runs
    `hl.dispatch(hl.dsp.exec_cmd("systemd-run --user --scope --collect --unit=app-uq-<id>-<rand> -- <argv>"))`.
  - Windows are addressed only as `address:0x…`.
  - All Lua strings go through `uq_core::lua::lua_string`.
- **Bridge framing:**
  - Every message carries `"v":1`; at most 256 KiB per line, and longer lines are dropped.
  - stdout is protocol only, and logs go to stderr. The bridge exits on stdin EOF.
- **Settings keys and validation:** exactly the spec §2.4 table. Unknown keys are rejected.
  `SettingsChanged` never includes launch counts.
- **Network:**
  - HTTPS only; `reqwest` with `rustls` and the platform verifier; User-Agent `uq/<version>`;
    5 s timeout; nothing logged or persisted.
  - Weather: latitude and longitude rounded to 2 dp.
  - Public IP: only while a VPN or exit node is active and `privacy.public_ip_lookup` is true;
    5 s after the network settles; at most once every 60 s; keep only `country` and
    `country_code`.
- **Units:** `Restart=always`, `RestartSec=1`, `StartLimitIntervalSec=0`, `KillMode=process`,
  `PartOf=graphical-session.target`. The shell sets `Quickshell.watchFiles = false`.
- **Pinned versions:** `quickshell>=0.3.1`, `quickshell<0.4`; Hyprland as pinned in Phase 1
  (0.56.2–0.57.x).
- **Island order:** System, Work, Time/weather, Notifications (placeholder in this slice),
  Status. They're floating, use the theme radius and gaps, and appear on every monitor.

## Review Focus

1. **An app launched from the shell must survive a shell restart.** Test: Task 13
   `smoke_launch_survives_restart`, plus Task 5 `launch_wraps_in_systemd_run_scope`.
2. **Hostile window titles and SSIDs must render as literal text.** For example
   `<img src=http://x>` or `<b>`. Test: Task 8 `tst_uqtext_plaintext` and the lint in Task 8.
3. **A theme change must restyle the bar even though Phase 1 swaps the whole theme directory.**
   Test: Task 8 `tst_theme_reload_after_dir_swap`.
4. **When uqd is stopped and restarted, the bar must keep working and recover.** Test: Task 4
   `uqd_down_then_up_events`, plus Task 8 `tst_bridge_backoff`.
5. **Plugging or unplugging a monitor must add or remove that monitor's bar without restarting
   the shell.** Test: manual check in Task 13 (`hyprctl output create headless` in the smoke run),
   asserting the `uq-bar` layer count changes.

---

## File Structure

```
crates/uq-core/src/settings.rs          Settings model + validation (new)
crates/uq-core/src/state.rs             gains settings + launch_counts; corrupt-file rename
crates/uqd/src/service.rs               settings commit path, launch counting
crates/uqd/src/dbus.rs                  GetSettings/SetSetting/RecordLaunch/GetLaunchCounts/ClearLaunchCounts + signals
crates/uqd/src/hypr.rs                  (moved) Hyprland socket client, re-exported from compositor.rs
crates/uqctl/src/main.rs                new subcommands; `bridge`
crates/uqctl/src/bridge/mod.rs          event loop: stdin reader, stdout writer, task wiring
crates/uqctl/src/bridge/protocol.rs     Message types, framing, caps
crates/uqctl/src/bridge/uqd.rs          D-Bus proxy to org.uq.Desktop1, signal forwarding, up/down watch
crates/uqctl/src/bridge/launch.rs       desktop-entry Exec parsing, quoting, systemd-run
crates/uqctl/src/bridge/dispatch.rs     closed action set → Lua
crates/uqctl/src/bridge/net.rs          Open-Meteo geocode/forecast, ipinfo
crates/uqctl/src/bridge/notifowner.rs   org.freedesktop.Notifications owner watch
shell/shell.qml                         root
shell/core/{Theme,Bridge,Settings}.qml  singletons (+ qmldir)
shell/core/UqText.qml                   plain-text Text
shell/bar/Bar.qml, Island.qml           per-screen PanelWindow + island chrome
shell/bar/{SystemIsland,WorkIsland,TimeIsland,NotifIsland,StatusIsland}.qml
shell/logic/{tokens,dock,minimise,rank,backoff,net}.js   pure JS (no Quickshell imports)
tests/qml/tst_*.qml                     qmltestrunner tests
scripts/lint-shell.sh                   UqText lint
packaging/uq-shell.service
config/hypr/base.lua                    env, misc, binds, hooks (modify)
packaging/uq/PKGBUILD, .github/workflows/ci.yml (modify)
```

---

### Task 1: Settings model and validation (uq-core)

**Files:**
- Create: `crates/uq-core/src/settings.rs`
- Modify: `crates/uq-core/src/lib.rs`
- Test: in-module tests

**Interfaces:**
- Produces:
  - `struct Settings { pinned: Vec<String>, weather_location: Option<Location>, idle: IdleTimings, notif: NotifSettings, privacy: PrivacySettings }`
    (serde, `Default` per spec §2.4, `#[serde(default)]`).
  - `struct Location { name: String, lat: f64, lon: f64 }`.
  - `struct IdleTimings { dim: Option<u32>, lock: Option<u32>, screen_off: Option<u32>, suspend_battery: Option<u32>, locked_screen_off: u32, media_blocks_lock: bool }`.
  - `enum NotifBackend { Auto, Notifi, Builtin, Swaync, Mako, Dunst }`, serialised lowercase, with
    `Notifi` as `"notifi"`.
  - `struct NotifSettings { backend: NotifBackend, show_summary: bool }`.
  - `struct PrivacySettings { public_ip_lookup: bool, record_launches: bool }`.
  - `fn Settings::apply(&mut self, key: &str, value: serde_json::Value) -> Result<(), SettingError>`.
    It validates and applies one key from the spec §2.4 table, and leaves `self` unchanged on
    error.
  - `enum SettingError { UnknownKey(String), Invalid { key: String, reason: String } }`, with
    `Display`.
  - `pub const SETTING_KEYS: [&str; 13]`.

- [ ] **Step 1: Write failing tests.**
  - `defaults_match_spec`: `pinned == []`; `weather_location == None`; idle
    `Some(240), Some(300), Some(360), Some(1200)`; `locked_screen_off == 30`;
    `media_blocks_lock == false`; `backend == Auto`; `show_summary == false`;
    `public_ip_lookup == true`; `record_launches == true`.
  - `rejects_unknown_key`: `apply("theme.x", json!(1))` returns `UnknownKey`.
  - `pinned_validation`: 33 entries fails; `"../evil"` fails; duplicates are collapsed;
    `"firefox.desktop"` is accepted.
  - `idle_order_enforced`: `idle.lock = 200`, which is below dim's 240, fails; `idle.dim = null`
    then `idle.lock = 200` succeeds; `idle.lock = 29` fails (range 30–86400).
  - `locked_screen_off_range`: 9 fails, 10 succeeds.
  - `location_validation`: lat 91 fails; a 101-character name fails;
    `{name:"London", lat:51.50735, lon:-0.12776}` is stored as `51.51, -0.13`.
  - `backend_enum`: `"notifi"` succeeds, `"growl"` fails.
  - `apply_is_atomic`: after a failed apply, the settings are unchanged.
- [ ] **Step 2:** `cargo test -p uq-core settings` → FAIL (module missing).
- [ ] **Step 3:** Implement. Keys follow spec §2.4 exactly. The pinned id regex is
  `^[A-Za-z0-9._-]{1,128}(\.desktop)?$`. Rounding is `(x * 100.0).round() / 100.0`.
- [ ] **Step 4:** `cargo test -p uq-core` → PASS; clippy clean.
- [ ] **Step 5:** Commit `feat(core): validated Settings model`.

### Task 2: uqd settings, launch counts and corrupt-state rename

**Files:**
- Modify: `crates/uq-core/src/state.rs`, `crates/uqd/src/service.rs`, `crates/uqd/src/dbus.rs`
- Test: in-module tests, plus `crates/uqd/tests/dbus.rs` and the fixture
  `crates/uq-core/tests/fixtures/phase1-state.toml`

**Interfaces:**
- Consumes: Task 1's `Settings`, `apply`, `SettingError`.
- Produces:
  - `State` gains `settings: Settings` (`#[serde(default)]`, flattened) and
    `launch_counts: BTreeMap<String, u32>` (`#[serde(default)]`).
  - `State::load` renames an unparsable file to `state.toml.bad-<unix_secs>` before returning
    the defaults.
  - `Desktop::set_setting(&mut self, key: &str, value: Value) -> Result<(), DesktopError>`. On
    success it saves and emits `Event::SettingsChanged(Settings)`.
  - `Desktop::record_launch(&mut self, id: &str)`:
    - a no-op when `record_launches` is false
    - caps the count at 10,000
    - saves at most once every 30 s, through a `last_launch_save: Instant` and a `dirty` flag
      flushed on the schedule tick
    - emits `Event::LaunchCountsChanged(BTreeMap)`
  - `Desktop::clear_launch_counts(&mut self)`.
  - `Desktop::prune_launch_counts(&mut self, installed: &dyn Fn(&str) -> bool)`, called at
    startup. The installed check looks up `<id>` in the `XDG_DATA_DIRS` `applications/` folders.
  - `DesktopError::InvalidSetting(String)`.
  - D-Bus methods: `GetSettings() → s` (JSON), `SetSetting(s key, s json)`,
    `RecordLaunch(s id)`, `GetLaunchCounts() → a{su}`, `ClearLaunchCounts()`.
  - D-Bus signals: `SettingsChanged(s json)`, `LaunchCountsChanged(a{su})`.
  - D-Bus error: `org.uq.Desktop1.Error.InvalidSetting`.

- [ ] **Step 1: Write failing tests.**
  - `phase1_state_upgrades`: the fixture (Phase 1 keys only) loads with
    `settings == Settings::default()` and its layouts kept.
  - `corrupt_state_is_renamed`: a file containing `{{nope` loads as defaults, and a
    `state.toml.bad-*` sibling exists with the original bytes.
  - `set_setting_persists_and_emits`: `idle.lock = 400` saves to `state.toml` and emits one
    `SettingsChanged` whose JSON has no `launch_counts`.
  - `set_setting_invalid_rejected`: `idle.lock = 1` returns `InvalidSetting`; the state is
    unchanged and no event is emitted.
  - `record_launch_throttled`: two calls 1 s apart give an in-memory count of 2 but only one save
    (check the file's mtime or a save counter on the test rig). It emits `LaunchCountsChanged`.
  - `record_launch_opt_out`: with `record_launches = false`, the count stays empty.
  - `launch_count_capped`: starting from 10,000, a further launch stays 10,000.
  - `prune_drops_uninstalled`.
  - In `dbus_round_trip`, extend the test: `SetSetting("notif.backend","\"mako\"")`, then
    `GetSettings` contains `"mako"`; `SetSetting("nope","1")` gives the error name
    `org.uq.Desktop1.Error.InvalidSetting`.
- [ ] **Step 2:** `cargo test --workspace` → the new tests FAIL.
- [ ] **Step 3:** Implement. Settings writes call `state.save` directly, with no render or
  verify step (spec §2.4).
- [ ] **Step 4:** `cargo test --workspace` → PASS.
- [ ] **Step 5:** Commit `feat(uqd): settings and launch counts`.

### Task 3: Bridge protocol and framing

**Files:**
- Create: `crates/uqctl/src/bridge/{mod.rs,protocol.rs}`
- Modify: `crates/uqctl/src/main.rs` (add `Cmd::Bridge`), `crates/uqctl/Cargo.toml` (add
  `tokio` features `io-std`, `io-util`)
- Test: in-module tests

**Interfaces:**
- Produces:
  - `enum Inbound { Request { id: u64, call: String, args: Vec<Value> } }`.
  - `enum Outbound { Ok { id: u64, ok: Value }, Err { id: u64, err: String }, Event { event: String, data: Value } }`.
    Each serialises with `"v":1`.
  - `const MAX_LINE: usize = 262_144`.
  - `fn decode_line(line: &[u8]) -> Result<Inbound, FrameError>`, where
    `FrameError::{TooLong, BadJson, BadVersion}`.
  - `fn encode(msg: &Outbound) -> Vec<u8>` (newline-terminated).
  - `async fn read_lines<R: AsyncRead>(r: R, tx: mpsc::Sender<Result<Inbound, FrameError>>)`.
    It reads with a 256 KiB cap and discards the rest of an over-long line up to the next newline,
    without buffering it.
  - `async fn run(stdin, stdout) -> ExitCode`. It answers `Ping` with
    `{"ok":"pong"}`, any unknown call with `{"err":"unknown call <name>"}`, sends a `Hello` event
    first, and returns on EOF.

- [ ] **Step 1: Write failing tests.**
  - `encode_has_version_and_newline`.
  - `decode_rejects_v2`.
  - `overlong_line_dropped_next_ok`: 300 KiB of `a`, a newline, then a valid Ping, gives
    `TooLong` and then the Ping. Peak buffer stays ≤ 256 KiB; assert through a reader with a
    counting buffer.
  - `run_hello_ping_eof` (in-memory duplex): the first line out is the `Hello` event; Ping is
    answered; closing stdin makes `run` return.
  - `unknown_call_errs`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): JSON-lines protocol and framing`.

### Task 4: Bridge ↔ uqd forwarding

**Files:**
- Create: `crates/uqctl/src/bridge/uqd.rs`
- Modify: `bridge/mod.rs`
- Test: `crates/uqctl/tests/bridge_dbus.rs` (private dbus-daemon, serving a real `Desktop` with
  fakes; uqctl dev-depends on `uqd` with feature `fake`)

**Interfaces:**
- Consumes: the D-Bus API from Phase 1 and Task 2.
- Produces:
  - Bridge calls `GetTheme`, `SetTheme`, `ListThemes`, `GetLayouts`, `SetLayout`,
    `SetSchedule`, `Reload`, `GetSettings`, `SetSetting`, `GetLaunchCounts`, `RecordLaunch`.
    Each maps 1:1 to the D-Bus method; JSON-string results are parsed into objects; D-Bus errors
    become `err` text.
  - Events `ThemeChanged`, `LayoutChanged`, `ConfigError`, `SettingsChanged`,
    `LaunchCountsChanged`, `UqdDown`, `UqdUp`.
  - `Hello.data = {v:1, uqd_up: bool, locked: false}`. `locked` stays false until slice 5.
  - The up/down watch uses `NameOwnerChanged` for `org.uq.Desktop1`, plus a 5 s poll while it's
    down.
  - `fn bridge_with_bus(addr: Option<&str>)` builds against a given bus address, for tests.

- [ ] **Step 1: Write failing tests.**
  - `bridge_forwards_set_theme`: write
    `{"v":1,"id":1,"call":"SetTheme","args":["dracula","light"]}`; read `{"id":1,"ok":null}`,
    then a `ThemeChanged` event with `name == "dracula"`.
  - `bridge_settings_round_trip`: `SetSetting("idle.lock", 400)`, then `GetSettings` returns an
    object with `idle.lock == 400`.
  - `uqd_down_then_up_events`: stop the served connection and get `UqdDown`; serve again and get
    `UqdUp`. A call while down gets `{"err":"uqd is not running"}`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): forward uqd calls and signals`.

### Task 5: Launch (desktop entries and argv) into systemd scopes

**Files:**
- Create: `crates/uqctl/src/bridge/launch.rs`, `crates/uqd/src/hypr.rs`
- Modify: `crates/uqd/src/compositor.rs` (move `Hyprland`'s socket client into `hypr.rs`,
  re-export it; behaviour unchanged), `bridge/mod.rs`
- Test: in-module tests

**Interfaces:**
- Consumes:
  - `uq_core::lua::lua_string`.
  - `uqd::hypr::HyprSocket::{from_env, eval(&str)}`. This is the moved client with the same 2 s
    timeout; uqctl depends on the `uqd` lib.
- Produces:
  - `fn parse_exec(exec: &str) -> Result<Vec<String>, ExecError>`. It follows the desktop-entry
    spec: quoting with `"`, the escapes `\" \` \$ \\`, and `%%` → `%`. It drops the field codes
    `%f %F %u %U %i %c %k`, and rejects deprecated codes (`%d %D %n %N %v %m`).
  - `fn shell_quote(arg: &str) -> String` (POSIX single-quote).
  - `fn scope_command(app_id: &str, argv: &[String], rand: u32) -> String`. It returns
    `systemd-run --user --scope --collect --unit=app-uq-<sanitised id>-<rand hex8> -- <quoted argv…>`.
    The id is sanitised to `[A-Za-z0-9_-]`, with at most 64 characters.
  - `fn exec_lua(cmd: &str) -> String`. It returns
    `return hl.dispatch(hl.dsp.exec_cmd(<lua_string(cmd)>))`.
  - Bridge call `Launch(args[0])`:
    - `{desktop_id}` reads the entry from the `XDG_DATA_DIRS` `applications/` folders. It
      prefixes `kitty -e` when `Terminal=true`, and records the launch.
    - `{argv:[...]}` launches the given argv.
    - `{run_id}` is rejected with `"run ids arrive in slice 2"` for now.

- [ ] **Step 1: Write failing tests.**
  - `parse_exec_strips_field_codes`: `firefox %u` gives `["firefox"]`.
  - `parse_exec_quotes`: `sh -c "echo \"hi\" \$HOME"` gives `["sh","-c","echo \"hi\" $HOME"]`.
  - `parse_exec_rejects_deprecated`: `%d`.
  - `shell_quote_single_quotes`: `it's` gives `'it'\''s'`.
  - `launch_wraps_in_systemd_run_scope`:
    `scope_command("org.mozilla.firefox", ["firefox"], 0xabcd)` gives
    `systemd-run --user --scope --collect --unit=app-uq-org_mozilla_firefox-0000abcd -- 'firefox'`.
  - `exec_lua_escapes`: a command containing `"` and a newline gives a valid escaped Lua literal.
    Assert the exact output.
  - `hostile_desktop_entry_stays_quoted`: `Exec=evil "a; rm -rf ~"` gives an argument quoted as a
    single word.
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p uqctl -p uqd`).
- [ ] **Step 5:** Commit `feat(bridge): Launch apps into their own systemd scopes`.

### Task 6: Dispatch (closed window-action set → Lua)

**Files:**
- Create: `crates/uqctl/src/bridge/dispatch.rs`
- Modify: `bridge/mod.rs`
- Test: in-module tests, and `crates/uqd/tests/verify_config.rs` (an extra ignored test)

**Interfaces:**
- Produces:
  - `enum Action` with variants `FocusWorkspace{id: i64}`, `FocusWindow{address}`,
    `Close{address}`, `ToggleFloat{address}`, `Pin{address}`,
    `Fullscreen{address, mode: Full|Maximized}`, `Move{address, workspace: WsTarget}`,
    `Minimise{address, from_ws: i64}`, `Restore{address, to_ws: i64}`, `Dpms{on: bool}`.
  - `fn Action::from_call(name: &str, args: &Value) -> Result<Action, String>`.
  - `fn Action::to_lua(&self) -> Vec<String>`. It returns one or more `return hl.dispatch(…)`
    statements, sent with `eval`.
  - `fn valid_address(s: &str) -> bool` (`^0x[0-9a-f]{1,16}$`). Strings are always passed as
    `"address:0x…"`.
  - `WsTarget = Id(i64) | Special(String)`, where `Special` must match `^[a-z0-9_-]{1,32}$` and is
    emitted as `"special:<name>"`.
  - Minimise:
    `hl.dsp.window.tag({ tag = "uq_ws_<n>", window = … })`, then
    `hl.dsp.window.move({ workspace = "special:minimized", follow = false, window = … })`.
  - Restore: move back to `to_ws`, then `tag` with `"-uq_ws_<n>"`.
  - Bridge call `Dispatch(name, args)`.
- [ ] **Step 0:** Confirm every `hl.dsp.*` name and argument key against
  `src/config/lua/bindings/LuaBindingsDispatchers.cpp` in Hyprland v0.56.2 and 0.57. Record any
  difference as a ruling and use the form both accept.
- [ ] **Step 1: Write failing tests.**
  - `dispatch_table`: one assertion per action with its exact Lua.
  - `rejects_bad_address`: `"0xZZ"` and `"title:foo"`.
  - `rejects_unknown_action`.
  - `special_name_validated`: `"magic\"x"` is rejected.
  - Ignored test `dispatch_lua_parses_under_hyprland`: wrap every generated statement in
    `hl.on("hyprland.start", function() <stmt> end)` in a temp `user.lua`, and run
    `Hyprland --verify-config`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): closed window-action dispatch`.

### Task 7: Network lookups and the notification-owner watch

**Files:**
- Create: `crates/uqctl/src/bridge/{net.rs,notifowner.rs}`
- Modify: `bridge/mod.rs`, `crates/uqctl/Cargo.toml` (add
  `reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls-native-roots"] }`)
- Test: in-module tests against a local `tokio` HTTP fake. The base URL is injectable;
  plain-HTTP base URLs are accepted only under `#[cfg(test)]`.

**Interfaces:**
- Produces:
  - `struct NetClient { http, weather_base, geo_base, ipinfo_base }`, with `NetClient::new()`
    pointing at the real HTTPS endpoints:
    - `https://api.open-meteo.com/v1/forecast`
    - `https://geocoding-api.open-meteo.com/v1/search`
    - `https://ipinfo.io/json`
  - `async fn geocode(&self, name: &str) -> Result<Vec<Location>, NetError>`: name at most
    100 characters, `count=5`, coordinates rounded.
  - `async fn weather(&self, loc: &Location) -> Result<Weather, NetError>`: sends
    `latitude=…&longitude=…` with 2 dp and `current=temperature_2m,weather_code,is_day`.
  - `struct Weather { temp_c: f64, code: u16, is_day: bool, location_name: String }`.
  - `async fn public_country(&self) -> Result<Country, NetError>`, where
    `Country { country_code, country }`. Every other field is dropped.
  - `struct IpGate { vpn_active: bool, last: Option<Instant> }` with
    `fn should_lookup(&mut self, enabled: bool, now: Instant) -> bool`. It's true only when
    `enabled && vpn_active` and at least 60 s have passed since the last lookup.
  - Bridge calls:
    - `GeocodeCity(name)`.
    - `NetworkChanged({vpn_active: bool})`, sent by the shell. After a 5 s settle, if `IpGate`
      allows it, the bridge looks up and emits `PublicIp`. When `vpn_active` is false, it emits
      `PublicIp null` straight away.
  - Weather loop: emit `Weather` at start (if a location is set), on `SettingsChanged` when the
    location changed, and every 30 min. On failure it emits nothing and keeps the last value
    shown.
  - `notifowner`: watches `NameOwnerChanged` for `org.freedesktop.Notifications` on the session
    bus and emits `NotifOwner {owner}` at start and on every change.
- [ ] **Step 1: Write failing tests.**
  - `weather_rounds_coordinates`: the fake server records the query and sees `latitude=51.51`,
    not `51.50735`.
  - `public_country_drops_other_fields`: the fake returns
    `{ip, city, loc, org, country:"NL", …}`; the result has no ip or city, and the emitted event
    JSON has only `country_code` and `country`.
  - `ip_gate`: no VPN means false; with a VPN the first call is true, a second call within 60 s
    is false, and after 60 s it's true again.
  - `geocode_name_too_long_rejected`.
  - `http_base_rejected_outside_tests`: `NetClient::with_bases("http://…")` is
    `#[cfg(test)]`-only; `new()` uses HTTPS. Assert the scheme.
  - `notif_owner_event` (private bus): claim the name, then `NotifOwner{owner: Some}`; release
    it, then `NotifOwner{owner: null}`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): weather, gated public-IP country, notif owner`.

### Task 8: Shell skeleton, singletons, UqText and lint

**Files:**
- Create:
  - `shell/shell.qml`
  - `shell/core/{Theme.qml,Bridge.qml,Settings.qml,UqText.qml,qmldir}`
  - `shell/logic/{tokens.js,backoff.js}`
  - `tests/qml/{tst_tokens.qml,tst_bridge_backoff.qml,tst_uqtext_plaintext.qml,tst_theme_reload_after_dir_swap.qml}`
  - `scripts/lint-shell.sh`
  - `packaging/uq-shell.service`

**Interfaces:**
- Consumes:
  - the bridge protocol (Tasks 3–7)
  - `tokens.json` (Phase 1 renderer: the keys `id, name, variant, colors.*, ui.*, fonts.*, icons.*`)
- Produces:
  - `Theme` (singleton): the properties `bg surface surfaceAlt overlay text textMuted accent
    accentText border success warning error` (color), `radius gapIn gapOut borderWidth`
    (int), `fontUi fontMono` (string), `fontSize` (int), `variant` (string).
    - `function reload()`.
    - It reads with `FileView { path: <state>/uq/theme/tokens.json }`. It reloads on
      `Bridge.themeChanged` and on a 2 s debounced `FileView` watch of the parent `uq/` folder.
    - If a parse fails, it keeps the previous values (`tokens.parse` returns `null`).
  - `logic/tokens.js`: `parse(text) → object|null` and `toTheme(obj) → flat object`, with
    defaults matching Graphite dark.
  - `Bridge` (singleton):
    - `function call(name, args) → Promise-like {then(cb), catch(cb)}`.
    - Signals: `event(name, data)`, `themeChanged()`, `settingsChanged(settings)`,
      `layoutChanged(ws, layout)`, `weather(data)`, `publicIp(data)`, `notifOwner(owner)`.
    - Properties: `uqdUp` (bool), `connected` (bool), and `sharing` (bool). `sharing` is set from
      `Hyprland.rawEvent` events named `screencast` (data `"<state>,<owner>"`, where state
      `1` means sharing started) and is used to hide the weather city, the VPN country and,
      later, notification summaries (spec §2.10).
    - It runs `Process { command: ["uqctl","bridge"]; stdinEnabled: true; stdout: SplitParser {} }`.
      It restarts with `backoff.next(attempt)` delays of 1, 2, 4, 8, 16, 30, 30… s.
  - `logic/backoff.js`: `next(attempt) → ms`.
  - `Settings` (singleton): mirrors `GetSettings`, plus `launchCounts`. It refreshes on
    `UqdUp` and `SettingsChanged`.
  - `UqText`: `Text { textFormat: Text.PlainText; color: Theme.text; font.family: Theme.fontUi; font.pixelSize: Theme.fontSize; elide: Text.ElideRight }`.
  - `shell.qml`:
    - Sets `Quickshell.watchFiles = false`.
    - Instantiates `Variants { model: Quickshell.screens; delegate: Bar {} }`. Task 9 creates
      `Bar`; until then, use a stub `Bar.qml` with an empty `PanelWindow`, namespace `uq-bar`.
  - `uq-shell.service`: `ExecStart=/usr/bin/qs -p /usr/share/uq/shell`, plus the Global
    Constraints unit options.
  - `scripts/lint-shell.sh`: exits 1, listing `file:line`, when `grep -rnE '^\s*Text\s*\{'
    shell/` matches outside `shell/core/UqText.qml`.
- [ ] **Step 1: Write failing tests** (run with `qmltestrunner -import shell -input tests/qml`).
  - `tst_tokens`: a valid tokens JSON maps `colors.accent` to `accent`; garbage gives `null`; a
    missing key falls back to the default.
  - `tst_bridge_backoff`: the delays are 1000, 2000, 4000, 8000, 16000, 30000, 30000 ms.
  - `tst_uqtext_plaintext`: a `UqText` with `text: "<b>x</b><img src='http://127.0.0.1:1/'>"`
    has `textFormat === Text.PlainText`, and its `implicitWidth` is greater than that of a
    `UqText` with text `"x"` (the literal tags are rendered).
  - `tst_theme_reload_after_dir_swap`:
    - Point `Theme` at a temp state dir and write `tokens.json` (accent `#111111`).
    - Rename the directory to `.old`, then write a new `theme/tokens.json` (accent `#222222`).
    - Within 3 s, `Theme.accent` is `#222222`. This uses `Theme.statePathOverride` (test-only)
      and the parent-folder watch.
  - `scripts/lint-shell.sh` on a temp copy containing a bare `Text {` exits 1.
- [ ] **Step 2:** Run them → FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** All `tst_*` pass; `scripts/lint-shell.sh` exits 0 on the repo;
  `qs -p shell --no-daemonize` starts under a running Hyprland, or reports "no compositor"
  without QML errors when there isn't one.
- [ ] **Step 5:** Commit `feat(shell): skeleton, Theme/Bridge/Settings, UqText and lint`.

### Task 9: Bar and System island (workspaces and merged dock)

**Files:**
- Create:
  - `shell/bar/{Bar.qml,Island.qml,SystemIsland.qml}`
  - `shell/logic/dock.js`
  - `tests/qml/tst_dock.qml`

**Interfaces:**
- Consumes: `Theme`, `Bridge.call("Launch"|"Dispatch"|"SetSetting"|"GetLayouts")`, `Settings.pinned`.
- Produces:
  - `Bar.qml`:
    - A `PanelWindow` with `required property ShellScreen modelData`, `screen: modelData`,
      anchored top/left/right, `WlrLayershell.namespace: "uq-bar"`, transparent background,
      `exclusiveZone` equal to the island height plus `Theme.gapOut`.
    - A row of five `Island`s; in this slice, Notifications is `NotifIsland` showing a static
      bell.
  - `Island.qml`: a rounded `Rectangle`
    (`color: Theme.surface; radius: Theme.radius; border.color: Theme.border`) with a default
    content slot.
  - `logic/dock.js`:
    - `entries(pinned: [id], toplevels: [{address, cls, title, workspace, minimised, monitor}], lookup: fn(cls)→id|null) → [{id, pinned, windows:[address], running, minimisedOnly}]`.
      The order is pinned in pin order, then unpinned running apps in first-seen order.
    - `nextWindow(entry, focusOrder: [address]) → address`, cycling.
  - `SystemIsland.qml`:
    - An app-menu button that is inert until slice 2.
    - Workspaces from `Hyprland.workspaces`, filtered to `id > 0`. Each shows a layout icon from
      `GetLayouts` (keys `"<id>"` or `"name:<name>"`; default dwindle). Clicking dispatches
      `focus_workspace {id}`.
    - The dock from `dock.entries`. Its class comes from `toplevel.lastIpcObject.class`, and the
      lookup is `DesktopEntries.heuristicLookup(cls)?.id`.
    - Click: a running entry → `Dispatch focus_window`, cycling with `nextWindow`; a
      minimised-only entry → `Restore`; not running → `Launch {desktop_id}`.
    - Right-click opens a menu with "Pin"/"Unpin", which calls `SetSetting("pinned", …)`.
    - Icons are `IconImage { source: Quickshell.iconPath(entry.icon, true) }` with
      `implicitSize` × the screen's `devicePixelRatio`.
- [ ] **Step 1: Write failing tests.**
  - `tst_dock`:
    - pinned order is kept
    - a running unpinned app is appended
    - two windows of the same class are grouped
    - a minimised-only entry is flagged
    - an unknown class with no desktop id gets the id `"class:<cls>"`
    - `nextWindow` cycles through 3 windows
- [ ] **Step 2–4:** FAIL → implement → PASS; lint passes.
- [ ] **Step 5:** Commit `feat(shell): bar and System island`.

### Task 10: Work island (focused window and actions)

**Files:**
- Create: `shell/bar/WorkIsland.qml`, `shell/logic/minimise.js`, `tests/qml/tst_minimise.qml`

**Interfaces:**
- Consumes: `Hyprland.activeToplevel`, `Bridge.call("Dispatch")`.
- Produces:
  - `logic/minimise.js`:
    - `tagFor(ws: int) → "uq_ws_<ws>"`
    - `wsFromTags(tags: [string]) → int|null`, which reads `lastIpcObject.tags`
  - `WorkIsland.qml`:
    - The icon and `UqText` title, elided to 40% of the bar width.
    - A dropdown with float toggle, pin, fullscreen, "Move to workspace" (1–10) and close.
    - Buttons for minimise (`Minimise {address, from_ws}`), maximise (`Fullscreen maximized`)
      and close.
    - With no focused window, it shows the desktop name `"Desktop"` and no buttons.
- [ ] **Step 1: Write failing tests.**
  - `tst_minimise`: the tag round-trips; `["foo","uq_ws_3"]` gives 3; no tag gives null; a
    malformed `uq_ws_x` gives null.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(shell): Work island and window actions`.

### Task 11: Time and weather island

**Files:**
- Create: `shell/bar/TimeIsland.qml`, `shell/logic/weather.js`, `tests/qml/tst_weather.qml`

**Interfaces:**
- Consumes: `Bridge.weather` and `Bridge.call("GeocodeCity"|"SetSetting")`.
- Produces:
  - `logic/weather.js`: `icon(code, isDay) → freedesktop icon name`, mapping the WMO codes:
    - 0 → `weather-clear` / `weather-clear-night`
    - 1–2 → `weather-few-clouds` / `weather-few-clouds-night`
    - 3 → `weather-overcast`
    - 45, 48 → `weather-fog`
    - 51–57, 80–82 → `weather-showers-scattered`
    - 61–67 → `weather-showers`
    - 71–77, 85, 86 → `weather-snow`
    - 95–99 → `weather-storm`
    - anything else → `weather-severe-alert`

    It also has and
    `format(tempC) → "9°"`, rounded.
  - `TimeIsland.qml`:
    - The clock (`SystemClock`, minute precision), formatted `HH:mm` and `ddd d MMM`.
    - Weather is hidden until the first `Weather` event, and hidden while `Bridge.sharing` (Task 8) is true.
    - The popover is a month calendar plus a "Set city" field. Typing calls `GeocodeCity` and
      shows up to 5 results; choosing one calls
      `SetSetting("weather_location", {name, lat, lon})`. There's also a "Turn off weather"
      entry, which sets `null`.
- [ ] **Step 1: Write failing tests.**
  - `tst_weather`: code 0 by day is clear sky and by night is the clear-night icon; 95 is the
    storm icon; an unknown code gives the generic icon; `format(8.6)` is `"9°"`;
    `format(-0.4)` is `"0°"`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(shell): Time and weather island`.

### Task 12: Status island (tray, media, network/VPN, audio, battery)

**Files:**
- Create: `shell/bar/StatusIsland.qml`, `shell/logic/net.js`, `tests/qml/tst_net.qml`

**Interfaces:**
- Consumes: `SystemTray.items`, `Mpris.players`, `Networking.devices`,
  `Pipewire.defaultAudioSink`, `UPower.displayDevice`, `Bridge.publicIp` and
  `Bridge.call("NetworkChanged")`.
- Produces:
  - `logic/net.js`:
    - `parseNmcliActive(text) → [{name, type}]` (input `-t -f NAME,TYPE`, with `\:` escapes)
    - `vpnName(conns) → string|null` (types `vpn` and `wireguard`)
    - `parseTailscale(json) → {exitNode: string|null}`, read from `ExitNodeStatus` and the peer
      `HostName`
    - `vpnActive(vpnName, exitNode) → bool`
  - `StatusIsland.qml`:
    - Tray icons. Click → `activate()`; right-click → the menu through `QsMenuAnchor`.
    - Media shows the first playing `MprisPlayer`'s `trackTitle` (in `UqText`), with play/pause
      on click and next on scroll.
    - Network shows a Wi-Fi or wired icon and strength from `Networking`. A 10 s
      `Timer`-driven `Process` runs `nmcli -t -f NAME,TYPE con show --active`, and
      `tailscale status --json` when `tailscale` is on `PATH`.
    - When `vpnActive` changes, after a 3 s debounce, it calls
      `NetworkChanged {vpn_active}`. The VPN label is the VPN name or exit node; the country
      comes from `PublicIp` and is hidden while sharing.
    - Audio shows the volume icon; scrolling changes the sink volume by ±5% (clamped 0–1.5).
    - Battery shows the icon and percentage, hidden when there's no `isLaptopBattery` device.
    - A quick-settings button that is inert until slice 3.
    - A missing tool (`nmcli` or `tailscale`) hides that part; the failing process doesn't loop
      (a failure is cached for 5 min).
- [ ] **Step 1: Write failing tests.**
  - `tst_net`:
    - nmcli parsing with an escaped colon in a name
    - the VPN pick ignores `802-11-wireless`
    - a WireGuard connection counts as a VPN
    - Tailscale with no exit node gives null; with one, its hostname
    - `vpnActive` truth table
    - a hostile SSID string passes through unchanged (the rendering test is Task 8)
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(shell): Status island`.

### Task 13: Session wiring, uqctl settings commands, packaging, CI and smoke test

**Files:**
- Modify: `config/hypr/base.lua`, `crates/uqctl/src/main.rs`, `packaging/uq/PKGBUILD`,
  `.github/workflows/ci.yml`, `crates/uqd/tests/verify_config.rs` (Lua mock),
  `README.md`
- Create: `scripts/smoke-shell.sh`

**Interfaces:**
- Consumes: everything above.
- Produces:
  - `base.lua`:
    - The start hook exports `WAYLAND_DISPLAY XDG_CURRENT_DESKTOP HYPRLAND_INSTANCE_SIGNATURE
      QT_QPA_PLATFORMTHEME DISPLAY XCURSOR_SIZE HYPRCURSOR_SIZE XCURSOR_THEME
      XDG_SESSION_TYPE`, then restarts `uqd.service` and `uq-shell.service`.
    - The shutdown hook stops both.
    - `hl.config({ misc = { allow_session_lock_restore = true, session_lock_xray = false,
      key_press_enables_dpms = true, mouse_move_enables_dpms = true } })`.
    - The recovery bind `SUPER+CTRL+L` (`locked = true`) runs
      `systemctl --user restart uq-lock.service || hyprlock`. The unit arrives in slice 5;
      `hyprlock` works now.
    - The other spec §2.12 binds (Spotlight, app menu, quick settings, OSD keys, lock, power
      menu) land with the slice that builds each surface. Slice 1 leaves the Phase 1 media and
      brightness binds in place.
  - `uqctl` subcommands:
    - `pin add|rm|list <id>`
    - `weather set <city>|off`: `set` geocodes through `NetClient`, prints the top 5 matches,
      and stores the first (or `--pick N`).
    - `idle set <dim|lock|screen-off|suspend|locked-screen-off> <secs|off>`
    - `notif backend <name>`
    - `privacy <public-ip|launches> on|off`
    - `launches clear`
    - Each one calls `SetSetting` or `ClearLaunchCounts`. Validation errors print
      `uqctl: <message>` and exit 1.
  - PKGBUILD:
    - `depends+=(quickshell>=0.3.1 quickshell<0.4 brightnessctl upower systemd)`
    - `optdepends+=(networkmanager tailscale bluez power-profiles-daemon hyprlock noti-fi fprintd)`
    - `checkdepends+=(qt6-declarative)`
    - `package()` installs `shell/` to `/usr/share/uq/shell/` and `packaging/uq-shell.service`
      to `/usr/lib/systemd/user/`.
  - CI: install `quickshell qt6-declarative`; run `scripts/lint-shell.sh` and
    `qmltestrunner -import shell -input tests/qml` (with `QT_QPA_PLATFORM=offscreen`) as the
    tester user; then run `scripts/smoke-shell.sh`.
  - `scripts/smoke-shell.sh`:
    - Starts Hyprland headless (try `AQ_BACKENDS=headless Hyprland -c <tmp stub>`; record in
      the ledger whether it works).
    - Runs `qs -p shell`, waits up to 10 s for `hyprctl layers -j` to contain the `uq-bar`
      namespace, and captures `grim` screenshots per theme if `grim` is present.
    - **`smoke_launch_survives_restart`**: calls the bridge's `Launch {argv:["sleep","300"]}`,
      restarts the shell, and asserts that the `app-uq-*` scope is still active
      (`systemctl --user is-active`).
    - **Hot-plug**: `hyprctl output create headless`, then the `uq-bar` layer count goes up by
      one; `hyprctl output remove …`, then it goes back down.
    - If headless Hyprland can't host Quickshell, the script exits 77 (skip). CI treats 77 as
      skipped, and the ledger records it as a manual check.
- [ ] **Step 1: Write failing tests.**
  - Extend `lua_mock_runs_every_variant`'s mock assertions:
    - `systemctl --user restart uq-shell.service` appears inside the start hook
    - `stop uq-shell.service` appears inside the shutdown hook
    - `hl.config` is called with `misc` (assert the string `allow_session_lock_restore` is
      present in `base.lua`)
  - `uqctl` CLI parse tests for each new subcommand, and `idle set lock off` maps to
    `SetSetting("idle.lock", null)`.
- [ ] **Step 2–4:** FAIL → implement → PASS. `bash -n scripts/smoke-shell.sh`; run the smoke
  script locally when Hyprland is installed, otherwise rely on CI.
- [ ] **Step 5:** Commit `feat: session wiring, uqctl settings, packaging and shell CI`; push;
  confirm CI is green, including qmltestrunner and the smoke result (pass or recorded skip).
