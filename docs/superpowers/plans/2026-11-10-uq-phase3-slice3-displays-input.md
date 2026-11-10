# uQ Phase 3 slice 3: Displays page with apply-and-revert, and the Input page — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Runs in parallel with uq#8 slice A** (history, undo and restore points), which another thread
> is building. Tasks 1–12 are written against today's `Desktop::commit(next, change)`. Task 13
> merges `main` after slice A lands and moves this slice onto slice A's
> `commit(next, change, origin)` and `persist(next, origin)`. Whichever branch merges second
> merges `main` and resolves the shared lists: `settings/mod.rs` domain lists, uqd `dbus.rs` and
> `service.rs`, and the app's `Main.qml`. The hook API is the one recorded from the Settings
> design thread on 2026-11-10 (it is summarised in Task 13).

**Goal:** `uq-settings` gains a Displays page and an Input page.
- **Displays** covers arrangement, resolution and refresh rate, scale, rotation, on/off,
  mirroring and VRR. A change applies at once. uqd puts the old layout back after 15 s unless the
  user keeps it, even if the app has crashed or the screen is black.
- **Input** covers keyboard layouts and variants, the layout-switch combo, key repeat, the mouse,
  the touchpad, and per-device pointer overrides.

**Architecture:**
- **uq-core** gains two section domains:
  - `monitors` is keyed by output name. It renders `hl.monitor` lines into `generated.lua`.
  - `input` holds the keyboard, touchpad and device settings. It renders into the `input` block
    and into `hl.device` lines.
  - The Phase 2 `[mouse]` table becomes the `mouse` part of the `input` section. Its storage stays
    where it is.
- **uqd:**
  - `commit` is split into "render and verify" and "install".
  - Every section write goes through one `commit_section`.
  - A new `monitors.rs` holds the apply/confirm/revert state machine. Its rollback point is the
    stored config, which nothing changes while an apply is pending.
  - The 15 s timer runs in uqd's D-Bus layer.
  - A small pending marker file lets a restarted uqd revert after a crash.
- **The app:** two page models on the shared runner.
  - The Displays draft and the arrangement maths are plain Rust.
  - The revert dialog is QML and is driven by `MonitorsChanged`.

**Tech Stack:** Rust edition 2024, zbus 5, cxx-qt 0.10, Qt 6, Kirigami 6, Hyprland 0.56.2–0.57.x
(Lua config).

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase3-settings-design.md`. The relevant parts are
§1 (success criteria), §2.2 (Devices: Displays, Input), §2.3 (the `monitors` and `input` rows, the
D-Bus additions, "Monitor apply and revert"), §3 (untrusted text), §4, §5 and §8 slice 3.
Precedent: the slice 1 and slice 2 plans in `docs/superpowers/plans/2026-11-10-uq-phase3-slice{1,2}-*.md`.
Companion: the uq#8 enhancements spec (§3 history, §4 monitor profiles, §7.1, §7.2, §9), which
is not yet confirmed.

## Global Constraints

These are copied from the spec, except the lines marked **ruling**, which this plan decides.

- **Hyprland settings (§1):** "Hyprland settings (monitors, input, keybinds, look) are owned by
  uqd, stored in `state.toml`, rendered into `generated.lua` and verified before they apply.
  `user.lua` still overrides them."
- **Success criterion (§1):** "A display change that blacks out the screen comes back on its own
  within 15 s, with no input from the user." Also: "Each change takes effect live and survives a
  reboot."
- **`monitors` (§2.3):** "per output name: mode, position, scale, transform, enabled, mirror-of,
  VRR". These are applied through "`hl.monitor({…})` lines; applied live through `eval`".
- **`input` (§2.3):** "keyboard layouts and variants, the layout-switch combo, repeat rate and
  delay, natural scroll, tap-to-click, pointer speed and acceleration profile, all per device
  class". These are applied through the "`input` block and `hl.device`".
- **D-Bus (§2.3):**
  - "`ListOutputs() → s`: live outputs from Hyprland (`monitors all -j`) together with the stored
    config."
  - "`ApplyMonitors(s json) → s token` and `ConfirmMonitors(s token)`."
  - Signal `MonitorsChanged`.
- **Monitor apply and revert (§2.3):**
  1. "`ApplyMonitors` validates the request."
  2. "It renders a candidate and verifies it."
  3. "It saves the current monitor config as a rollback point."
  4. "It applies the new config live and returns a token."
  5. "`ConfirmMonitors(token)` writes the change to `state.toml` and drops the rollback point."
  6. "If no confirm arrives within 15 s, uqd re-applies the rollback point, emits
     `MonitorsChanged`, and discards the token."

  "The state file is written only on confirm, so a reboot during the revert window comes back
  on the old config. Only one apply can be pending at a time; a second `ApplyMonitors` call
  replaces the first, keeping the original rollback point."
- **Assumption (§1):** "The display revert lives in uqd, not the app, so it still happens if the
  app crashes."
- **Errors (§4):**
  - "Display change not confirmed: uqd reverts after 15 s, whether or not the app is open".
  - "`generated.lua` verify fails: uqd rejects the change; the page shows Hyprland's message in
    plain text and the control returns to its last good value".
  - "uqd not running: uQ pages show 'uQ's settings service isn't running' and become read-only".
- **Plain text (§2.1, §3):** "All text goes through `UqLabel`". "Device names" are untrusted. Output
  descriptions and input device names from Hyprland count as device names.
- **Testing (§5):**
  - "Validation for every new domain".
  - "Golden tests of the rendered `generated.lua` sections; CI also verifies them with Hyprland
    `--verify-config`".
  - "The monitor apply/revert state machine: confirm, timeout, crash during the revert window,
    reboot during the revert window, and a second apply".
  - "Rust unit tests for every page model".
  - "Offscreen QML tests for … the revert dialog".
  - The live checklist covers "monitor revert on real hardware".
- **Ruling, strictness (slice 1 and 2 precedent):**
  - Loading is lenient per field or entry (`lenient`, `#[serde(default)]`); unknown keys in
    `state.toml` are ignored.
  - `SetSection` is strict: `check_known` plus `check_kept`. Don't use `deny_unknown_fields`.
  - A bad stored value costs only its own field or output entry, never the user's theme or pins.
  - TOML has no null, so a `None` is left out of the file.
- **Ruling, Hyprland names.** These were checked against v0.57.0
  `src/config/values/ConfigValues.cpp` and `src/config/lua/bindings/LuaBindingsConfigRules.cpp`.
  - **`hl.monitor` fields:**
    - `output` (string, required), `mode`, `position` and `scale` are all **strings**
      (`"preferred"`, `"auto"`);
    - `disabled` (bool), `transform` (int 0–7), `mirror` (string) and `vrr` (int, -1 to 3).
  - **`input` options:**
    - `kb_layout`, `kb_variant` and `kb_options` (strings);
    - `repeat_rate` (int 0–200) and `repeat_delay` (int 0–2000 ms);
    - `sensitivity`, `accel_profile`, `natural_scroll`, `left_handed` and `scroll_factor`.
  - **`input.touchpad` options:**
    - `natural_scroll`, `disable_while_typing` and `scroll_factor`;
    - **`tap-to-click`**, with a hyphen, emitted as `["tap-to-click"]`.
  - **`hl.device` fields:** `name` (string, required; Hyprland turns spaces into `-`),
    `sensitivity`, `accel_profile`, `natural_scroll`, `left_handed` and `scroll_factor`.
  - **Not checked:** 0.56.2, whether `hl.config`'s Lua table accepts the hyphenated
    `["tap-to-click"]`, and whether `eval` of `hl.monitor` applies at once. CI's
    `all_variants_pass_hyprland_verify` (`--ignored`) is the gate for names. If it rejects a key,
    drop that key from the domain and the page and say so in the PR. Don't work around it. The
    live checklist is the gate for `eval`.
- **Ruling, `mouse.*` coexists with `input`; no migration.**
  - The `input` section's JSON is `{keyboard, mouse, touchpad, devices}`. Its `mouse` part *is*
    `Settings.mouse`, read from and written to the existing `[mouse]` TOML table.
  - `SetSetting("mouse.*")` and `uqctl mouse` keep working unchanged.
  - The new parts are stored in a new `[input]` table.
  - Nothing moves, so every stored `mouse.*` value is kept by construction. The test
    `phase2_mouse_values_survive` pins this.
  - `emit_mouse`'s lines stay byte-identical, so the default golden doesn't change.
- **Ruling, per device class.**
  - Keyboard settings and mouse settings are the global `input` options; Hyprland applies the
    mouse ones to every pointer.
  - The touchpad class uses `input.touchpad`, which has no speed option in Hyprland.
  - Pointer speed and acceleration for one device, such as a touchpad, are set as a per-device
    override keyed by Hyprland's device name and rendered as `hl.device`. A new
    `ListInputDevices() → s` lists the devices to choose from. The spec doesn't list this
    method.
- **Ruling, D-Bus beyond the spec.**
  - `RevertMonitors(s token)` serves the dialog's "Revert" button and Escape.
  - `ListInputDevices() → s`.
  - `MonitorsChanged` has no arguments, as in the spec. The page re-reads `ListOutputs`, whose
    `pending` field says whether an apply is waiting and how long is left.
  - `SetSection("monitors")` is refused with "displays change through ApplyMonitors", so there is
    exactly one write path for monitors.
- **Ruling, the rollback point is the stored config.**
  - During the window `state.toml` and `generated.lua` hold the old config.
  - A revert is a Hyprland `reload`, which re-reads them. base.lua's `output = ""` catch-all rule
    covers any output the candidate touched that has no stored entry.
  - If the reload fails, uqd evals the rollback lines instead.
  - The `Pending` record still keeps a copy of the stored `Monitors`, as the spec's step 3 says.
- **Ruling, other writes during the window.** A commit that reloads Hyprland while an apply is
  pending (a look change, or the theme schedule) re-evals the candidate afterwards. Otherwise
  that reload would silently revert the displays early.
- **Ruling, a second apply restarts the 15 s** from the second call. The first token stops
  working.
- **Ruling, safety guards in `ApplyMonitors`.** It refuses:
  - a config that leaves no connected output enabled and not mirroring: "at least one display
    must stay on";
  - any call while Hyprland is not running: "displays can't be changed while the desktop isn't
    running".
- **uq#8 rules adopted (they don't contradict the spec):**
  - **Single write path:** every new monitors and input save goes through uqd's commit path,
    through `Desktop::commit_section` and then `commit`. Slice A turns `commit` into
    `persist(next, origin)`.
    - A confirmed monitor change is exactly one call of it.
    - `ApplyMonitors` and any revert never save `state.toml`.
    - `ConfirmMonitors` gains an optional trailing `s origin` in Task 13.
  - **§9 plain names:** labels never say `uqd`, `generated.lua`, `Hyprland` or D-Bus names.
    Connector names (`DP-1`) and raw device names appear only as secondary text under
    "Details".
  - **§9 no silent failure:** a refused write snaps back and shows why.
  - **§9 no settings that reset themselves:** each page model has a test that sets every field,
    re-opens the model and checks the round trip.
  - **§9 search:** every declared search entry has keywords. The existing registry test is
    extended to check this.
- **Process:**
  - **Builds:** run with `CARGO_BUILD_JOBS=4` and one cargo invocation at a time. cxx-qt builds
    are heavy, and earlyoom kills rustc on this host when cargo commands overlap.
  - **Hyprland:** it isn't installed locally. `--verify-config` runs only in CI, so say so in the
    report. Never use `hyprctl` or the real session bus; use private `dbus-daemon` buses only,
    with socket paths under 108 bytes.
  - **Hardware:** the displays revert needs real hardware and is on the live checklist, not
    automated.
  - **Gate:** `cargo fmt`, `clippy -D warnings` and `cargo test --workspace` must be clean. Keep
    files at or under about 400 lines.
  - **Background work:** run builds and tests in the foreground and leave nothing running. Don't
    wait on CI; report what you pushed.

## Forward compatibility with uq#8

The data shapes here let uq#8's slice F add its features without migrating slice 3's data.

- **The section is an object.** The `monitors` section's JSON is `{"outputs": {<name>: {...}}}`,
  not a bare map. Slice F's `primary` and `auto_profiles` become sibling keys. Older uQ versions
  ignore them on load (lenient), and `check_known` gets the new names.
- **Every field is optional.** Each output entry's fields are all `Option`, where `None` means
  "Hyprland's default". A monitor profile can then be an overlay of the same `OutputConfig` type,
  where a `Some` wins.
- **Identity is recorded.**
  - Each entry carries `description: Option<String>` (Hyprland's "make model serial"). uqd fills
    it from the live output on confirm.
  - Slice F matches profiles by make/model/serial *sets*, so it can turn slice 3's stored layout
    into a first profile without asking the user again.
  - Slice 3 never renders `description`.
- **Profiles go elsewhere.** Monitor profiles live in their own table or file (uq#8 §4) and are
  applied *over* `monitors`. The stored `monitors` stays the "no profile matched" layout, so slice
  3's data keeps its meaning.
- **Hooks for history.**
  - `apply_monitors` takes the candidate through the same render and verify path that slice F's
    hotplug apply needs (`Desktop::render_candidate`), so F calls it without the 15 s window.
  - The confirm is one `commit_section("monitors", …)`, and so one `persist` after Task 13.
    That is the entry uq#8 §3 records.
  - A rolled-back candidate never reaches `commit_section`, which matches uq#8 §3's rule that
    "unconfirmed monitor applies are never recorded".
- **Gestures.** The `input` section reserves nothing for them. uq#8 §7.2 adds a `gestures` key
  beside `keyboard`, `mouse`, `touchpad` and `devices`.

## Review Focus

1. **The screen goes black and nobody touches anything.** The apply's live eval "succeeds", the
   app might be frozen or closed, and 15 s later the old layout is back. This must come from
   uqd's own timer, over the real `serve()` path, not from a unit call. →
   `unconfirmed_apply_reverts_on_its_own` (Task 6), with the window shortened through
   `with_revert_after`, plus `revert_window_is_fifteen_seconds`.
2. **Something else reloads Hyprland during the window**: the theme schedule flips at sunset, or
   the user changes gaps in another page. The candidate must stay live until confirm or timeout,
   and the confirm must still save it. → `reload_during_window_reapplies_candidate` (Task 5).
3. **A request that turns off the only screen**, or makes every output mirror another, must be
   refused before anything is applied. Hyprland can't show a dialog on zero outputs. →
   `apply_refuses_all_outputs_off` (Task 5).
4. **A hand-edited or newer `state.toml`** with one bad output entry (`scale = "huge"`, a mirror
   of itself) or an unknown keyboard layout syntax. The other outputs, the user's `[mouse]`
   values and everything else must load. → `bad_output_entry_costs_only_itself` (Task 1) and
   `phase2_mouse_values_survive` (Task 2).
5. **Hostile names from Hyprland.** An output description or input device name with quotes,
   newlines or Lua must not break `generated.lua` or render as rich text. Device names reach
   `hl.device` only through `lua_string`, and names longer than 128 characters or containing
   control characters are refused. → `device_names_are_escaped` (Task 3) and
   `live_output_strings_are_capped` (Task 1). The QML side is covered by the existing
   control-text lint.

---

### Task 1: `monitors` domain in uq-core

**Files:**
- Create: `crates/uq-core/src/settings/monitors.rs`.
- Modify: `crates/uq-core/src/settings/{mod,load,tests}.rs`.

**Interfaces:**
- Produces:
  - `pub struct OutputConfig`. Every field is `Option` with `#[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "lenient")]`:
    - `mode: String`: `preferred`, `highres`, `highrr`, or `WxH` / `WxH@R`, where W and H are
      2–5 digits and R is `\d{1,3}(\.\d{1,3})?`;
    - `position: Position { x: i32, y: i32 }`, each within -32768..=32767; `None` means `auto`;
    - `scale: f64`: 0.25–4.0 and finite, rounded to 6 places; `None` means `auto`;
    - `transform: u8` (0–7) and `enabled: bool`;
    - `mirror_of: String`: an output name, not itself, and the target doesn't mirror another;
    - `vrr: u8` (0–3);
    - `description: String`: at most 200 chars with control characters stripped; metadata only.
  - `OutputConfig` has `pub const FIELDS: [&str; 8]`.
  - `pub struct Monitors { pub outputs: BTreeMap<String, OutputConfig> }`, with
    `pub const FIELDS: [&str; 1] = ["outputs"]`:
    - an output name matches `^[A-Za-z0-9._-]{1,32}$`;
    - there are at most 16 outputs;
    - `outputs` loads entry by entry: a bad entry is dropped with a log line, as `lenient_rules`
      does.
  - `impl Monitors`:
    - `pub fn from_section(v: &Value) -> Result<Monitors, SettingError>` is strict;
    - `pub fn sanitised(self) -> (Monitors, Vec<SettingError>)` drops invalid entries one by one
      and clears a broken `mirror_of`.
  - `pub struct LiveOutput { name, description, make, model, serial, width: u32, height: u32, refresh_rate: f64, x: i32, y: i32, scale: f64, transform: u8, disabled: bool, mirror_of: Option<String>, vrr: bool, available_modes: Vec<String> }`:
    - it is parsed from Hyprland's camelCase `monitors all -j`;
    - `mirrorOf: "none"` becomes `None`;
    - strings are at most 200 chars with control characters stripped, and there are at most 64
      modes.
  - `pub fn parse_live(json: &str) -> Vec<LiveOutput>` returns an empty list on bad JSON.
  - `pub fn stays_visible(m: &Monitors, live: &[LiveOutput]) -> bool`: at least one live output
    is enabled and not mirroring. An output with no entry counts as enabled.
  - `Settings` gains `pub monitors: Monitors` (TOML `[monitors]`, `lenient`). `SECTION_DOMAINS`
    gains `"monitors"`. `renders_section("monitors")` is **false**: uqd treats it specially
    (Task 5). `validated()` uses `sanitised`.

- [ ] **Step 1:** Write the failing tests:
  - `monitors::tests::modes_and_ranges`:
    - accepts `"2560x1440@143.912"`, `"preferred"` and scale `1.333333`;
    - rejects `"2560x1440@"`, `"big"`, scale 0.2, scale 4.5, NaN, transform 8, vrr 4, the name
      `"DP 1"` and 17 outputs.
  - `monitors::tests::mirror_rules`: a mirror of itself and a chain A→B→C are refused, and a
    mirror of an output with no entry is allowed.
  - `monitors::tests::unknown_fields_refused`: `{"outputs":{"DP-1":{"rotation":1}}}` and
    `{"primary":"DP-1"}`.
  - `monitors::tests::bad_output_entry_costs_only_itself`: parse `State` from a TOML with
    `[monitors.outputs.DP-1] scale = "huge"`, `[monitors.outputs.DP-2] mode = "preferred"` and
    `[mouse] sensitivity = 0.4`. DP-2 and the mouse value are kept, and DP-1 is dropped.
  - `monitors::tests::live_output_strings_are_capped`: a description of 500 chars with `\n` and
    `"` is cut to 200 with no control characters, and `"mirrorOf":"none"` becomes `None`.
  - `monitors::tests::stays_visible_rules`: all disabled gives false, all mirroring gives false,
    and an empty `outputs` gives true.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-core monitors`. Expected: FAIL (no
  module).
- [ ] **Step 3:** Implement. Add the fixture
  `crates/uq-core/tests/fixtures/monitors-all.json`: two outputs, eDP-1 and DP-1, written by hand
  in Hyprland's shape.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-core`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): monitors settings section`

### Task 2: `input` domain in uq-core

**Files:**
- Create: `crates/uq-core/src/settings/input.rs`.
- Modify: `crates/uq-core/src/settings/{mod,load,tests}.rs`.

**Interfaces:**
- Produces:
  - `pub struct KbLayout { pub layout: String /*^[a-z]{2,8}$*/, pub variant: Option<String> /*^[A-Za-z0-9_-]{1,32}$*/ }`.
  - `pub struct KeyboardSettings { layouts: Vec<KbLayout> /*0–4; empty = base.lua's "us"*/, switch_combo: Option<String> /*^grp:[a-z0-9_]{1,40}$*/, repeat_rate: Option<u8> /*1–100 per s*/, repeat_delay: Option<u16> /*100–2000 ms*/ }`.
    Duplicate `(layout, variant)` pairs are refused.
  - `pub struct TouchpadSettings { natural_scroll: Option<bool>, tap_to_click: Option<bool>, disable_while_typing: Option<bool>, scroll_factor: Option<f64> /*0.1–10, round2*/ }`.
  - `pub struct DeviceOverride { sensitivity: Option<f64> /*-1–1, round2*/, accel_profile: Option<AccelProfile>, natural_scroll: Option<bool>, left_handed: Option<bool>, scroll_factor: Option<f64> }`.
  - `pub struct InputSettings { keyboard, touchpad, devices: BTreeMap<String, DeviceOverride> }`:
    - there are at most 32 devices;
    - a name is 1–128 chars, holds no control characters, and has spaces turned into `-` on
      write, as Hyprland does;
    - an entry with every field `None` is dropped.
  - Each struct has `FIELDS`. Every field is lenient on load.
  - `Settings` gains `pub input: InputSettings` (TOML `[input]`). `SECTION_DOMAINS` gains
    `"input"`, and `renders_section("input")` is true.
  - `Settings::section("input")` is `{keyboard, mouse, touchpad, devices}`, with `mouse` taken
    from `self.mouse`.
  - `Settings::set_section("input", v)` is strict.
    - `mouse` is checked against `["sensitivity","accel_profile","natural_scroll","left_handed","scroll_factor"]`,
      then each key is replayed through `apply("mouse.<k>")` on a clone. A missing `mouse` means
      `MouseSettings::default()`, as with replace semantics elsewhere.
    - The other parts go through `from_value`, `check_kept` and `validate`.
    - On error `self` is unchanged.

- [ ] **Step 1:** Write the failing tests:
  - `input::tests::keyboard_rules`:
    - accepts `[{us}, {de, nodeadkeys}]`;
    - rejects 5 layouts, `"US"`, `"u s"`, a duplicate, `switch_combo = "ctrl:nocaps"`,
      `repeat_rate` 0 and `repeat_delay` 50.
  - `input::tests::device_names`: `"Logitech G502"` is stored as `"Logitech-G502"`, while `"a\nb"`,
    a 129-char name and 33 devices are refused.
  - `input::tests::section_carries_mouse`: `set_section("input", {"mouse":{"sensitivity":0.3}})`
    sets `settings.mouse.sensitivity == 0.3`, and `section("input")["mouse"]["natural_scroll"] == false`.
  - `input::tests::phase2_mouse_values_survive`: parse `[mouse]\nsensitivity = -0.5\nnatural_scroll = true\n`,
    then `section("input")["mouse"]` equals those values, and `save` and reload keep `[mouse]`
    and write no `[input].mouse`.
  - `input::tests::unknown_fields_refused`, at the top level, in `keyboard` and in `mouse`.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-core input`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-core`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): input settings section (keyboard, touchpad, devices; mouse kept in [mouse])`

### Task 3: Render monitors and input into `generated.lua`

**Files:**
- Create:
  - `crates/uq-core/src/lua/monitors.rs`: `emit_monitor_line` and `emit_monitors`;
  - `crates/uq-core/src/lua/input.rs`: `emit_mouse` moves here, unchanged, plus `emit_input` and
    `emit_devices`.
- Modify:
  - `crates/uq-core/src/lua/mod.rs`;
  - `crates/uq-core/tests/generated_golden.rs` and `tests/golden/generated/all-set.lua`;
  - `crates/uqd/tests/verify_config.rs`: `settings()` sets every new field, and the mock
    requires `hl.monitor` and `hl.device`;
  - `config/hypr/base.lua`: comments only, saying that monitor lines and input overrides come
    from `generated.lua` and the `output = ""` rule is the fallback.

**Interfaces:**
- Consumes: `Monitors` and `OutputConfig` (Task 1); `InputSettings` and `MouseSettings` (Task 2).
- Produces:
  - `pub fn emit_monitor_line(name: &str, o: &OutputConfig) -> String` returns one line,
    `hl.monitor({ output = "<n>", … })`:
    - `enabled == Some(false)` gives `disabled = true` and nothing else;
    - otherwise it emits `mode`, `position` (`"XxY"`), `scale` (`{:?}` of the float, as a
      string) and `transform` when set, `mirror = "<n>"` when set, and `vrr` when set;
    - an unset `mode`, `position` or `scale` is emitted as `"preferred"`, `"auto"` or `"auto"`,
      because Hyprland would otherwise reuse a previous rule's value;
    - every string goes through `lua_string`.
  - `pub fn emit_monitors(m: &Monitors) -> String` returns the lines in name order. It is written
    after `hl.config(...)` under `-- Monitors`.
  - The `input = {…}` table in `hl.config` holds:
    - the mouse lines first, byte for byte as today;
    - then `kb_layout` and `kb_variant` (comma-joined, empty variant slots kept) when `layouts`
      isn't empty;
    - `kb_options` when `switch_combo` is set;
    - `repeat_rate` and `repeat_delay` when set;
    - then `touchpad = { … }` with only the set keys, where tap-to-click is
      `["tap-to-click"] = …`.
  - `pub fn emit_devices(i: &InputSettings) -> String` returns one
    `hl.device({ name = "<n>", … })` per device, with only the set keys. Floats use `{:?}`, and
    `accel_profile` `Default` is left out.
  - `emit_generated` keeps its signature.

- [ ] **Step 1:** Write the failing tests:
  - `lua::monitors::tests::line_shapes`:
    - `DP-1 {mode:"2560x1440@144", position:{0,0}, scale:1.25, transform:1, vrr:1}` gives exactly
      `hl.monitor({ output = "DP-1", mode = "2560x1440@144", position = "0x0", scale = "1.25", transform = 1, vrr = 1 })`;
    - a disabled output gives `hl.monitor({ output = "HDMI-A-1", disabled = true })`;
    - an empty entry gives `mode = "preferred", position = "auto", scale = "auto"`.
  - `lua::input::tests::default_input_unchanged`: a default render is byte-identical to the
    current `tests/golden/generated/default.lua`, without blessing.
  - `lua::input::tests::keyboard_and_touchpad`: `[us, de(nodeadkeys)]` gives
    `kb_layout = "us,de"` and `kb_variant = ",nodeadkeys"`. Also checks
    `["tap-to-click"] = false` and that no `touchpad` table is emitted when nothing is set.
  - `lua::input::tests::device_names_are_escaped`: the device `x"y\\z` gives
    `name = "x\"y\\\\z"`. `generated.lua` gains no extra line.
  - `generated_golden::golden_default_and_all_set`, where `all-set` gains two monitors (one
    mirrored), two layouts, a switch combo, touchpad keys and one device.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-core`, then
  `CARGO_BUILD_JOBS=4 cargo test -p uqd --test verify_config`. Expected: FAIL.
- [ ] **Step 3:** Implement. Bless only `all-set.lua` (`UQ_BLESS=1`) and read it by eye.
  `lua_mock_runs_every_variant` must pass.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test --workspace`. Expected: PASS.
  `all_variants_pass_hyprland_verify` runs in CI only; the PR names the hyphenated
  `tap-to-click` key as the one to watch.
- [ ] **Step 5:** Commit: `feat(core): render monitor lines, keyboard, touchpad and device overrides`

### Task 4: uqd: one section-commit path, `input` writes, and live device lists

**Files:**
- Modify:
  - `crates/uqd/src/compositor.rs` (trait, `Hyprland` and `fake`);
  - `crates/uqd/src/service.rs`: split `commit`;
  - `crates/uqd/src/sections.rs` and its tests;
  - `crates/uqd/src/dbus.rs`;
  - `crates/uq-core/src/client.rs`;
  - `crates/uqd/tests/dbus.rs`.

**Interfaces:**
- Produces:
  - `Compositor` gains `async fn monitors(&self) -> Result<String, CompositorError>`
    (`j/monitors all`) and `async fn devices(&self) -> Result<String, CompositorError>`
    (`j/devices`).
  - `FakeCompositor` gains:
    - `monitors_json: Mutex<String>` and `devices_json: Mutex<String>`;
    - `eval_error: Mutex<Option<String>>` and `reload_error: Mutex<Option<String>>`, each
      returning `Rejected` once.
  - `Desktop::render_candidate(&self, next: &State) -> Result<Candidate, DesktopError>`, where
    `Candidate { files, generated: String, path: PathBuf }`. It holds today's render, candidate
    write and verify, unchanged, and leaves the candidate file on disk.
  - `Desktop::install(&mut self, c: Candidate, next: State, change: Change) -> Result<(), DesktopError>`.
    It holds the rest of today's `commit`, and `commit` becomes those two calls.
  - `pub(crate) async fn commit_section(&mut self, domain: &str, next: State) -> Result<(), DesktopError>`:
    - it holds everything after validation in today's `set_section`: render or save, the layout
      events, `SectionChanged` and `SettingsChanged`;
    - it adds 1 to `section_commits: u32` on success, through the accessor
      `pub fn section_commits(&self) -> u32`;
    - a doc comment says it is the single section save path, which slice A's `persist` journals
      (Task 13).
  - `set_section`:
    - it parses, validates, then calls `commit_section`;
    - for `"monitors"` it returns `InvalidSetting("displays change through ApplyMonitors")`.
      Only `confirm_monitors` (Task 5) calls `commit_section("monitors", …)`, which commits with
      `Change::Quiet`: it writes the files, and the config is already live;
    - `input` renders with `Change::Reload`.
  - `Desktop::list_input_devices(&self) -> Result<String, DesktopError>`:
    `{keyboards:[{name}], pointers:[{name, default_speed}], touch:[{name}]}`, taken from
    `devices -j`'s `keyboards`, `mice` and `touch`. Names are capped as in Task 1. Hyprland down
    gives `{keyboards:[],pointers:[],touch:[]}`.
  - D-Bus `ListInputDevices() → s`, and `DesktopProxy::list_input_devices`.

- [ ] **Step 1:** Write the failing tests:
  - `sections::tests::set_input_renders_and_reloads`: `generated.lua` contains
    `kb_layout = "us,de"`, and the fake recorded `reload`.
  - `sections::tests::rejected_input_keeps_previous_config`: copy `rejected_look_keeps_previous_config`
    for `input`.
  - `sections::tests::monitors_section_write_refused`.
  - `sections::tests::every_section_write_counts_once`: `appearance`, `look` and `input` each add
    1; a refused write adds 0.
  - `sections::tests::input_devices_from_hyprland`: uses a fixture with one keyboard and two mice,
    one with a 300-char name.
  - In `tests/dbus.rs`: `input_section_round_trip_over_dbus`.
  - Every existing `service.rs` test passes unchanged; that is the refactor's check.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uqd`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): one section-commit path; input section and device list`

### Task 5: uqd: the monitor apply/confirm/revert state machine

**Files:**
- Create: `crates/uqd/src/monitors.rs`, an `impl Desktop` block with its tests, as `sections.rs`
  is.
- Modify:
  - `crates/uqd/src/service.rs`: the `pending` field, the re-eval after a reload in `install`, a
    marker check in `startup`, and the `Event::MonitorsChanged` variant;
  - `crates/uqd/src/lib.rs`.

**Interfaces:**
- Consumes: `render_candidate` and `commit_section` (Task 4); `Monitors`, `stays_visible`,
  `parse_live` (Task 1); `emit_monitor_line` (Task 3).
- Produces:
  - `pub const REVERT_AFTER: Duration = Duration::from_secs(15);`
  - `Desktop::with_revert_after(self, d: Duration) -> Self` is for tests.
  - The pending record:
    `struct Pending { token: String, candidate: Monitors, rollback: Monitors, touched: BTreeSet<String>, deadline: Instant }`.
    - The token is 32 hex chars from `/dev/urandom`; add no new dependency.
    - `touched` is every output named in any candidate since the first apply.
  - `pub async fn apply_monitors(&mut self, json: &str, now: Instant) -> Result<(String /*token*/, Instant /*deadline*/), DesktopError>` runs these steps in order:
    1. Parse with `Monitors::from_section`.
    2. Read `comp.monitors()`. `NotRunning` refuses with the ruling's message.
    3. Refuse if `!stays_visible`.
    4. Fill each entry's `description` from the live output of the same name.
    5. Call `render_candidate` on `state` with `monitors = candidate`, then delete the candidate
       file. Nothing is installed.
    6. Write the marker `state_file.with_file_name("monitors-pending.json")` = `{token, touched}`.
    7. `eval` all candidate lines joined by `" "`. If that is rejected, revert at once and return
       `ConfigInvalid(msg)`.
    8. Store `Pending` (keeping an existing `rollback`), emit `MonitorsChanged`, and return.
  - `pub async fn confirm_monitors(&mut self, token: &str) -> Result<(), DesktopError>`:
    - with a matching token: `commit_section("monitors", next)` with `Change::Quiet` (already
      live), then clear `Pending`, delete the marker and emit `MonitorsChanged`;
    - a wrong or expired token gives `InvalidSetting("no display change is waiting for confirmation")`;
    - if `commit_section` fails, `Pending` is kept, so the timer still reverts.
  - `pub async fn revert_monitors(&mut self, token: &str) -> Result<(), DesktopError>` checks the
    token, then calls `revert`.
  - `pub async fn expire_monitors(&mut self, token: &str, now: Instant) -> bool` reverts only if
    the token matches and `now >= deadline`, and returns whether it did.
  - `async fn revert(&mut self)`:
    - it runs `comp.reload()`; if that is `Rejected` or times out, it evals
      `emit_monitors(rollback)` plus a catch-all line (`emit_monitor_line(name, &OutputConfig::default())`)
      for each touched output with no rollback entry;
    - then it clears `Pending`, deletes the marker and emits `MonitorsChanged`;
    - it never calls `commit_section`.
  - In `install`, after any `reload()` while `pending` is `Some`, it evals the candidate lines
    again.
  - In `startup`, before the reload, a leftover marker means "crashed during a window": log it,
    delete the marker, then let the normal startup reload restore the stored config, and emit
    `MonitorsChanged`.
  - `pub async fn list_outputs(&self, now: Instant) -> String` returns
    `{live: [LiveOutput], stored: <monitors section>, pending: null | {token, seconds_left: u64}}`.
    With Hyprland down, `live` is `[]`.

- [ ] **Step 1:** Write the failing tests in `monitors.rs`, using the `sections.rs` rig with
  `monitors_json` set to the Task 1 fixture:
  - `confirm_saves_once`:
    - apply `{outputs:{DP-1:{scale:1.5}}}`; the fake recorded one `eval hl.monitor({ output = "DP-1", … scale = "1.5" …})`;
    - `state.toml` and `generated.lua` are byte-identical to before;
    - then `confirm` puts `scale = 1.5` in `state.toml` and `generated.lua` and sets
      `description` from the fixture;
    - `section_commits()` went 0 → 1, the marker is gone, and confirming again errors.
  - `timeout_reverts`:
    - `expire_monitors(token, now + 14 s)` returns false and does nothing;
    - at `now + 15 s` it returns true, the fake recorded `reload` after the eval, and the files
      are unchanged;
    - `section_commits() == 0`, `MonitorsChanged` was sent, and the token is dead.
  - `crash_during_window_recovers_on_restart`: apply, then drop the `Desktop` without confirming.
    A new `Desktop` on the same paths runs `startup()`. Then the marker is gone, the stored
    monitors are the old ones, the new fake recorded `reload`, and `MonitorsChanged` was emitted.
  - `reboot_during_window_keeps_old_config`: apply, then read `state.toml` and `generated.lua`
    from disk. Neither mentions the candidate. A fresh `State::load` has the old `monitors`. The
    marker lives in the state dir, so `startup` after a reboot also deletes it, and that test
    covers this.
  - `second_apply_replaces_and_keeps_rollback`:
    - stored is `DP-1 scale 1`; apply A (`DP-1 scale 2`), then apply B (`HDMI-A-1 disabled`);
    - token A is dead, and the deadline is B's time plus 15 s;
    - the rollback is still `DP-1 scale 1`, and `touched == {DP-1, HDMI-A-1}`;
    - after expiry with `reload_error` set, the fallback eval contains `DP-1`'s stored line and
      `HDMI-A-1`'s catch-all line.
  - `apply_refuses_all_outputs_off`: every live output is disabled or mirroring. The fake records
    no eval, and there is no marker.
  - `rejected_eval_reverts_at_once`: with `eval_error` set, the result is `ConfigInvalid`, there
    is no pending apply, and `reload` was recorded.
  - `rejected_verify_applies_nothing`: with `verify_error` set, there is no eval and no marker.
  - `reload_during_window_reapplies_candidate`: apply, then `set_section("look", …)`. The calls
    end `reload, eval <candidate>`. Then confirm saves both the look change and the monitors.
  - `apply_refused_without_hyprland`.
  - `revert_window_is_fifteen_seconds`: `assert_eq!(REVERT_AFTER, Duration::from_secs(15))`.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uqd monitors`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): monitor apply, confirm and automatic revert`

### Task 6: uqd D-Bus: ListOutputs, ApplyMonitors, ConfirmMonitors, RevertMonitors and the timer

**Files:**
- Modify:
  - `crates/uqd/src/dbus.rs`;
  - `crates/uq-core/src/client.rs`;
  - `crates/uqd/tests/dbus.rs`;
  - `crates/uqctl` (only where `names_match_the_proxies` or exhaustive matches need it).

**Interfaces:**
- Consumes: Task 5.
- Produces:
  - D-Bus methods:
    - `ListOutputs() → s`;
    - `ApplyMonitors(s json) → s token`;
    - `ConfirmMonitors(s token)`;
    - `RevertMonitors(s token)`;
    - signal `MonitorsChanged()`.
  - `apply_monitors` in `DesktopIface` spawns `revert_timer(shared.clone(), token, deadline)`:
    it runs `tokio::time::sleep_until(deadline)`, then
    `shared.lock().await.expire_monitors(&token, Instant::now())`. The task holds no lock while it
    sleeps. A replaced token makes it a no-op.
  - `serve` forwards `Event::MonitorsChanged` as the signal.
  - `serve_with(builder, desktop)` is unchanged; tests use `Desktop::with_revert_after`.
  - `DesktopProxy` gains `list_outputs`, `apply_monitors`, `confirm_monitors`, `revert_monitors`,
    `list_input_devices` and `#[zbus(signal)] fn monitors_changed`.

- [ ] **Step 1:** Write the failing tests in `tests/dbus.rs`:
  - `unconfirmed_apply_reverts_on_its_own`:
    - the `Desktop` uses `with_revert_after(300 ms)` and a `FakeCompositor` with the fixture;
    - over the bus, `ApplyMonitors`, then wait for `MonitorsChanged` twice (applied, then
      reverted) with a 3 s timeout;
    - the fake recorded `reload` after the eval, and `ConfirmMonitors(token)` now fails with
      `InvalidSetting`.
  - `confirmed_apply_survives_the_window`: confirm within 100 ms, sleep 500 ms, and there is no
    `reload` after the eval. `GetSection("monitors")` has the change.
  - `revert_now_over_dbus`.
  - `list_outputs_reports_pending`: `pending.seconds_left` is 0 or 1 within the 300 ms window,
    and `null` after confirm.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uqd --test dbus`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): displays over D-Bus, reverted by uqd's own timer`

### Task 7: Fake uqd support and the Displays arrangement logic

**Files:**
- Create: `apps/uq-settings/src/pages/displays/arrange.rs` and
  `apps/uq-settings/src/pages/displays/mod.rs` (`pub mod arrange;` only for now).
- Modify:
  - `apps/uq-settings/src/pages/fake_uqd.rs`;
  - `apps/uq-settings/src/pages/mod.rs`;
  - `apps/uq-settings/src/model_core.rs`: `Follow` gains `monitors: bool`, which re-reads on
    `MonitorsChanged`, and every existing `FOLLOW` sets it to `false`.

**Interfaces:**
- Produces in `fake_uqd`:
  - `live_outputs: Arc<Mutex<Vec<LiveOutput>>>` and `pending: Arc<Mutex<Option<(String, Monitors)>>>`;
  - `list_outputs` and `list_input_devices` (canned: one keyboard, `elan-touchpad`, `logitech-mouse`);
  - `apply_monitors`, which validates through `Monitors::from_section` and `stays_visible` and
    returns the token `"t1"`, `"t2"` and so on;
  - `confirm_monitors`, which stores into `state.settings.monitors`, and `revert_monitors`;
  - `set_section("monitors")` refused, as uqd does;
  - `pub async fn expire(rig: &Rig)`, which drops the pending apply and emits `MonitorsChanged`
    from the fake's connection.
  - `Rig` exposes `live_outputs` and `pending`.
- Produces in `arrange` (pure, and in logical pixels):
  - `pub struct Rect { pub name: String, pub x: i32, pub y: i32, pub w: i32, pub h: i32 }`.
  - `pub fn logical_size(mode_w: u32, mode_h: u32, scale: f64, transform: u8) -> (i32, i32)`:
    the size divided by scale and rounded, with w and h swapped for odd transforms.
  - `pub fn place(others: &[Rect], moving: &Rect, x: i32, y: i32) -> (i32, i32)`:
    - it snaps each edge to another output's edge within 32 px;
    - then, if the result overlaps, it moves to the nearest free side (right, then below);
    - the result always touches at least one other output when there are any.
  - `pub fn normalise(rects: &mut [Rect])` shifts everything so the minimum x and y are 0.
  - `pub fn scale_choices(mode_w: u32, mode_h: u32) -> Vec<f64>` keeps those of
    `[1.0, 1.25, 1.333333, 1.5, 1.6, 1.75, 2.0, 2.5, 3.0]` whose logical w and h are whole numbers
    within 0.01. 1.0 is always kept.
  - `pub fn parse_mode(s: &str) -> Option<(u32, u32, Option<f64>)>` reads Hyprland's
    `"2560x1440@143.91Hz"` form.

- [ ] **Step 1:** Write the failing tests:
  - `arrange::tests::{snaps_to_right_edge, overlap_pushed_out, rotated_size_swaps, normalise_to_origin, scale_choices_for_4k_and_1080p, parse_hyprland_modes}`.
    For 3840x2160 the choices include 1.5 and 2.0. For 1920x1080 they include 1.25 and not 1.6.
  - `fake_uqd` tests: `fake_apply_then_confirm` and `fake_refuses_all_off`.
  - `model_core::tests::wants_filters_by_domain` still passes.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-settings arrange`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-settings`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(settings): display arrangement maths and fake uqd displays`

### Task 8: Displays page model

**Files:**
- Create: `apps/uq-settings/src/displays_model.rs` (bridge; it sits in `src/` per QTBUG-93443).
- Modify:
  - `apps/uq-settings/src/pages/displays/mod.rs`, which gets `DisplaysState` and `Op`;
  - `apps/uq-settings/build.rs` (the bridge file);
  - `apps/uq-settings/src/main.rs`.

**Interfaces:**
- Consumes: `runner`, `model_core` (with `Follow.monitors`), `arrange` and the `fake_uqd`
  displays (Task 7).
- Produces:
  - `pub enum Op { Refresh, Edit { output: String, field: String, value: Value }, Move { output: String, x: i32, y: i32 }, Discard, Apply, Keep, Revert }`.
  - `DisplaysState: PageLogic` holds `draft: std::sync::Mutex<Option<Monitors>>`.
    - `Edit` changes only the draft. Its fields are those in `OutputConfig::FIELDS` minus
      `description`; others are refused with `unknown field "<f>"`.
    - `Move` calls `arrange::place` and `normalise`, with the other rects built from the live
      outputs and the draft.
    - `Discard` clears the draft.
    - `Apply` sends the draft (or stored, if there is no draft) through `ApplyMonitors` and keeps
      the draft, so the dialog can show it.
    - `Keep` calls `ConfirmMonitors(pending.token)` and clears the draft.
    - `Revert` calls `RevertMonitors(pending.token)` and keeps the draft, so the user can fix it.
    - No op touches uqd's stored config except through these calls.
  - The snapshot is `{live, stored, draft, dirty: bool, pending, view: [{name, label, rect, enabled, mirror_of, modes: [{w,h,rates:[..]}], scale_choices}]}`:
    - `label` is `description`, or the name when that is empty;
    - `modes` groups `available_modes` by size, largest first.
  - `DisplaysModel` has:
    - `FOLLOW = {sections: &["monitors"], theme: false, settings: false, monitors: true}`;
    - the invokables `refresh`, `edit(output, field, json)`, `moveOutput(output, x, y)`,
      `discard`, `apply`, `keep` and `revert`;
    - the property `pending_seconds: i32` (−1 when nothing is pending), set from each snapshot.
  - After a revert caused by a timeout (`MonitorsChanged` while the page has a draft and
    `pending` is `null`), `error` becomes "The previous display settings were restored." This is
    plain copy for the page.

- [ ] **Step 1:** Write the failing tests in `pages::displays::tests`:
  - `edit_changes_only_the_draft`: the fake's stored monitors are unchanged, and `dirty` is
    true.
  - `apply_then_keep_saves`: the fake's stored monitors equal the draft, and `pending` is `null`.
  - `apply_then_revert_keeps_draft`.
  - `timeout_reports_restore`: apply, `fake_uqd::expire`, then the next refresh's outcome error
    is the restore message, and the draft is kept.
  - `refused_apply_shows_reason`: an all-off draft gives an error containing "at least one
    display must stay on", and `pending` is `null`.
  - `round_trip_every_field`, the uq#8 §9 rule: set `mode`, `position`, `scale`, `transform`,
    `enabled`, `mirror_of` and `vrr`, apply and keep, make a fresh `DisplaysState`, refresh, and
    every field reads back.
  - `unknown_field_refused`.
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-settings displays`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-settings` and
  `CARGO_BUILD_JOBS=4 cargo clippy -p uq-settings --all-targets -- -D warnings`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(settings): Displays page model (draft, apply, keep, revert)`

### Task 9: Displays page QML and the revert dialog

**Files:**
- Create:
  - `apps/uq-settings/qml/pages/DisplaysPage.qml`;
  - `apps/uq-settings/qml/components/DisplayArrangement.qml`: scaled rectangles, each draggable;
    on release it calls `moveOutput`, and the label goes through `UqLabel`;
  - `apps/uq-settings/qml/components/RevertDialog.qml`;
  - `tests/settings-qml/tst_revert_dialog.qml` and `tests/settings-qml/tst_display_arrangement.qml`.
- Modify: `apps/uq-settings/build.rs`, and `apps/uq-settings/src/registry.rs` (page and search
  entries).

**Interfaces:**
- Consumes: `DisplaysModel` (Task 8).
- Produces:
  - `RevertDialog` is a `Kirigami.PromptDialog`:
    - properties: `seconds: int` and `open: bool`; signals: `keep()` and `revert()`;
    - its title is "Keep these display settings?";
    - its text is a `UqLabel`: "Reverting in N seconds." It counts down with a 1 s `Timer` that
      starts from `seconds` and is never below 0;
    - the buttons are "Keep changes" and "Revert". Neither is the default, so Enter does
      nothing. Escape and closing the dialog emit `revert()`;
    - at 0 it shows "Reverting…" and does not call `revert()`, since uqd reverts by itself.
  - `DisplaysPage`:
    - the arrangement sits on top;
    - for the selected output there is a `SettingRow` each for: on/off (`SwitchRow`); resolution
      and refresh rate (two `UqComboBox`es from `view.modes`); scale (`UqComboBox` of
      `scale_choices`); rotation (`UqComboBox`: Normal, 90°, 180°, 270°, Flipped, Flipped 90°,
      Flipped 180°, Flipped 270°, mapped to transforms 0–7); mirror (`UqComboBox`: "Don't mirror"
      plus the other outputs' labels); and variable refresh rate (`UqComboBox`: Off, On,
      Fullscreen only, Fullscreen games and video, mapped to 0–3);
    - a "Details" disclosure shows the connector name and the description;
    - Apply and Discard are enabled when `dirty`;
    - `RevertDialog` is open while `pendingSeconds >= 0`;
    - the `error` text shows in an inline `Kirigami.InlineMessage`;
    - with uqd down, the page shows the §4 message and its controls are disabled; with no live
      outputs, it shows "Can't read your displays right now."
  - Registry:
    - `PageInfo { id: "displays", name: "Displays", icon: "preferences-desktop-display", category: Devices, qml: "pages/DisplaysPage.qml" }`;
    - its search ids are `display-arrangement`, `display-resolution`, `display-refresh`,
      `display-scale`, `display-rotation`, `display-enabled`, `display-mirror` and `display-vrr`,
      each a `SettingRow` id, with keywords such as `monitor`, `screen`, `resolution` and
      `hz`.

- [ ] **Step 1:** Write the failing tests:
  - `tst_revert_dialog.qml`:
    - `test_counts_down`: with `seconds: 3`, after 1.1 s the text contains "2";
    - `test_enter_does_not_keep`: a `Return` key press emits no `keep`;
    - `test_escape_reverts`;
    - `test_zero_does_not_call_revert`.
  - `tst_display_arrangement.qml`: dragging rect `DP-1` emits `moved("DP-1", x, y)` in output
    coordinates, at a fixed preview scale.
  - `registry::tests::ids_unique_and_categories_ordered` covers `displays`, and a new
    `every_entry_has_keywords` (uq#8 §9) passes for all pages.
- [ ] **Step 2:** Run `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`
  and `CARGO_BUILD_JOBS=4 cargo test -p uq-settings registry`. Expected: FAIL.
- [ ] **Step 3:** Implement. All labels are plain words: no "Hyprland", "uqd" or connector names
  outside "Details".
- [ ] **Step 4:** Run the QML suite, `bash tests/shell/lint_settings.sh && scripts/lint-settings.sh`,
  and the offscreen `--screenshots` run into `/tmp/uqs-shots`. Check `displays.png` by eye; with
  no bus it shows the uqd-down state.
- [ ] **Step 5:** Commit: `feat(settings): Displays page with keep-or-revert dialog`

### Task 10: Input page model and the xkb lists

**Files:**
- Create:
  - `apps/uq-settings/src/xkb_lists.rs`;
  - `apps/uq-settings/src/input_model.rs` (bridge);
  - `apps/uq-settings/src/pages/input.rs`;
  - `apps/uq-settings/tests/fixtures/evdev.lst`: a trimmed real file with sections
    `! model`, `! layout`, `! variant` and `! option`.
- Modify: `apps/uq-settings/src/pages/mod.rs`, `build.rs` and `main.rs`.

**Interfaces:**
- Consumes: the `input` section (Task 2), `ListInputDevices` (Task 4) and the `fake_uqd`
  devices (Task 7).
- Produces:
  - `xkb_lists::parse(src: &str) -> XkbLists { layouts: Vec<(code, name)>, variants: BTreeMap<layout, Vec<(code, name)>>, switch_options: Vec<(code, name)> }`:
    - `switch_options` keeps only `grp:` options that end in `_toggle` or `_switch`;
    - names are at most 200 chars with control characters stripped, and are shown through
      `UqLabel` only.
  - `xkb_lists::load() -> XkbLists` reads `/usr/share/X11/xkb/rules/evdev.lst`. A missing file
    gives empty lists, and the page then offers a plain layout-code field.
  - `pub enum Op { Refresh, SetKeyboard { field: String, value: Value }, SetMouse { field, value }, SetTouchpad { field, value }, SetDevice { name: String, field: String, value: Value /*null removes the field*/ }, RemoveDevice { name: String } }`.
    Every op uses `patch_section(proxy, "input", …)` under the write lock, and unknown fields are
    refused before any call.
  - The `InputState` snapshot is `{input: <section>, devices: <ListInputDevices>, xkb: XkbLists}`.
    `xkb` is loaded once per model.
  - `InputModel` has:
    - `FOLLOW = {sections: &["input"], theme: false, settings: true, monitors: false}`. Settings
      is followed because `uqctl mouse` changes `[mouse]` through `SettingsChanged`.
    - the invokables `refresh`, `setKeyboard`, `setMouse`, `setTouchpad`, `setDevice` and
      `removeDevice`.

- [ ] **Step 1:** Write the failing tests:
  - `xkb_lists::tests::{parses_fixture, switch_options_only_grp_toggles, names_sanitised}`.
  - `pages::input::tests`:
    - `layouts_add_reorder_remove`;
    - `mouse_write_lands_in_mouse_table`: after `SetMouse{sensitivity, 0.4}`, the fake's
      `state.settings.mouse.sensitivity == 0.4`;
    - `device_override_add_and_clear`: setting the last field to null removes the device;
    - `rejected_verify_restores_state`;
    - `bad_layout_refused_with_reason`;
    - `round_trip_every_field` (uq#8 §9).
- [ ] **Step 2:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-settings input xkb`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_BUILD_JOBS=4 cargo test -p uq-settings` and clippy. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(settings): Input page model and keyboard layout lists`

### Task 11: Input page QML

**Files:**
- Create:
  - `apps/uq-settings/qml/pages/InputPage.qml`;
  - `apps/uq-settings/qml/components/KeyboardLayoutList.qml`: rows with up and down buttons,
    Remove, the layout and variant names in `UqLabel`, and an Add row with two `UqComboBox`es;
  - `tests/settings-qml/tst_keyboard_layout_list.qml`.
- Modify: `build.rs` and `registry.rs`.

**Interfaces:**
- Consumes: `InputModel` (Task 10).
- Produces:
  - The `InputPage` sections are:
    - **Keyboard:** `KeyboardLayoutList`; "Switch layouts with" (`OverrideCombo` of
      `switch_options`, with "Use default" meaning none); repeat delay and rate (`OverrideSpin`,
      100–2000 ms and 1–100 per second); and a test field, a plain `QQC2.TextField` with
      placeholder "Type here to test", which the lint allows because it shows typed text only.
    - **Mouse:** speed as a slider from −1 to 1, labelled "Slower" and "Faster"; acceleration
      (`UqComboBox`: Default, Adaptive, Flat); natural scrolling; left-handed; and scroll speed.
    - **Touchpad:** shown only when `devices.pointers` has a name containing `touchpad`, or when
      any touchpad key is set. It has natural scrolling, tap to click and disable while typing as
      `OverrideSwitch`, and scroll speed as `OverrideSlider`.
    - **Per device:** a list of the current overrides, plus "Add a device setting" (a
      `UqComboBox` of `devices.pointers` names). Each override has speed, acceleration, natural
      scrolling, left-handed and scroll speed. Device names are shown in `UqLabel` under
      "Details".
  - Registry:
    - `PageInfo { id: "input", name: "Input", icon: "preferences-desktop-peripherals", category: Devices, qml: "pages/InputPage.qml" }`;
    - its search ids are `keyboard-layouts`, `layout-switch`, `repeat-delay`, `repeat-rate`,
      `mouse-speed`, `mouse-accel`, `mouse-natural-scroll`, `mouse-left-handed`,
      `mouse-scroll-speed`, `touchpad-natural-scroll`, `tap-to-click`, `disable-while-typing`,
      `touchpad-scroll-speed` and `device-overrides`, each a `SettingRow` id.

- [ ] **Step 1:** Write the failing tests:
  - `tst_keyboard_layout_list.qml`: moving row 1 up emits `reordered([...])`, Remove emits
    `removed(i)`, the first row's up button is disabled, and Add with no layout chosen is
    disabled.
  - The registry tests now include `input`.
- [ ] **Step 2:** Run the settings QML suite and `CARGO_BUILD_JOBS=4 cargo test -p uq-settings registry`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests, both lint scripts and `--screenshots`. Check `input.png` by eye.
- [ ] **Step 5:** Commit: `feat(settings): Input page`

### Task 12: Docs, changelog, live checklist and the whole gate

**Files:**
- Create: `docs/notes/settings-slice3-live-check.md`.
- Modify:
  - `CHANGELOG.md` (Unreleased);
  - `README.md`: the Settings section lists Displays and Input. It says that monitor lines and
    keyboard layouts now come from `generated.lua`, that `user.lua`'s own `hl.monitor` lines
    still win, and that a display change you don't keep is undone after 15 seconds.

The manual checklist runs on real hardware with two outputs where possible:
- **Blackout:** choose a mode the panel can't show. Leave the keyboard and mouse alone. The old
  layout must be back within 15 s.
- **Only output off:** turning off the only output is refused with "at least one display must
  stay on".
- **Crash:** kill the app during the window; the layout reverts on time.
- **uqd restart:** `systemctl --user kill -s KILL uqd` during the window. After the restart the
  old layout is back.
- **Reboot:** reboot during the window; it comes back on the old layout.
- **Keep:** keep, then reboot; it comes back on the new layout.
- **Second apply:** apply twice within 15 s, then wait; it returns to the layout from before the
  first apply.
- **Mirror and on/off:** mirror, and turn one of two outputs off.
- **Scale:** 1.25 and 1.5 on a 1080p and a 4K panel.
- **VRR:** on and off.
- **Gaps during the window:** change gaps on the Look page while the dialog counts down. The
  displays stay on the candidate.
- **Keyboard:** two layouts with a switch combo; the combo switches layout. Repeat delay of
  250 ms is noticeable.
- **Mouse:** `uqctl mouse speed -0.5` shows on the Input page at once.
- **Touchpad:** tap-to-click off works. A per-device speed for the touchpad differs from the
  mouse.
- **`eval`:** Hyprland's `eval` of `hl.monitor` applied without a reload. Record the version.

- [ ] **Step 1:** Write the files.
- [ ] **Step 2:** Run the whole gate, one command at a time, all with `CARGO_BUILD_JOBS=4`:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `cargo test --workspace`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - both qmltestrunner suites;
  - `tests/shell/*.sh` except `bar_loads.sh`.

  Expected: all PASS.
- [ ] **Step 3:** Commit: `docs: Settings slice 3 notes, changelog and live checklist`. Push the
  branch and report. CI runs `--verify-config`, which covers the `tap-to-click` spelling and the
  `hl.monitor` and `hl.device` fields. Don't wait on it.

### Task 13: Merge `main` after uq#8 slice A, and move onto its history hook

Do this task only once slice A is on `main`. If slice 3 merges first, the slice A thread does the
same merge from its side, and this task is dropped.

**Slice A's hook API:**
- All `state.toml` saves go through `Desktop::persist(&mut self, next: State, origin: Origin) -> Result<(), DesktopError>`.
- `commit` becomes `commit(next, change, origin)`.
- `uq_core::history::Origin` is one of `Settings | Uqctl | Shell | Profile(String) | Restore | Import | Unknown | Schedule`.
  It is parsed from an optional trailing `s origin` argument; a missing or empty one means
  `Unknown`.
- The domain and summary come from `uq_core::history::diff(&State, &State)`, so callers pass no
  labels.
- `ApplyMonitors` must never call `persist`.

**Files:**
- Modify:
  - `crates/uqd/src/{sections,monitors,service,dbus}.rs`;
  - `crates/uq-core/src/client.rs`;
  - `apps/uq-settings/src/pages/{displays/mod,input}.rs` and `apps/uq-settings/src/pages/fake_uqd.rs`;
  - and whatever the merge touches.

**Interfaces:**
- Consumes: slice A's `persist`, `commit(next, change, origin)`, `Origin` and `diff`.
- Produces:
  - `commit_section(domain, next, origin: Origin)` passes `origin` to `commit`, or to `persist`
    on the non-render path. The `section_commits` counter is deleted; slice A's journal replaces
    it.
  - `confirm_monitors(token, origin: Origin)` gives D-Bus `ConfirmMonitors(s token, s origin)`,
    where `origin` is optional and trailing, as on slice A's other methods.
  - `ApplyMonitors`, `RevertMonitors`, `expire_monitors`, the re-eval after a reload, and the
    startup marker recovery take no origin and never call `persist`.
  - `DesktopProxy::confirm_monitors(token, origin)`. Both new page models pass `"settings"` on
    every `SetSection` and `ConfirmMonitors`.

- [ ] **Step 1:** Run `git merge origin/main`. Resolve the domain lists in `settings/mod.rs`
  (keep both sides' domains), the `DesktopIface` methods and events in `dbus.rs`, the `commit`
  call sites in `service.rs`, and the page list in `Main.qml` (keep both).
- [ ] **Step 2:** Rewrite the counter assertions as journal assertions, through slice A's
  `ListHistory`:
  - `confirm_saves_once`: after apply there are 0 new entries; after confirm there is exactly 1,
    with domain `monitors` and origin `settings`;
  - `timeout_reverts`, `second_apply_replaces_and_keeps_rollback`,
    `crash_during_window_recovers_on_restart` and `rejected_eval_reverts_at_once`: 0 new entries
    each;
  - `every_section_write_counts_once`: one entry per accepted `input` write, and none for a
    refused one.
  - Add `tests/dbus.rs::confirm_origin_is_recorded`: `ConfirmMonitors(token, "uqctl")` records
    origin `uqctl`, and a missing origin records `unknown`.
- [ ] **Step 3:** Run `CARGO_BUILD_JOBS=4 cargo test -p uqd`. Expected: FAIL until the origin is
  threaded through.
- [ ] **Step 4:** Implement, then run the whole Task 12 gate again, one command at a time.
  Expected: all PASS.
- [ ] **Step 5:** Commit: `merge: main after slice A; displays and input record history through persist`.
  Push and report. Don't wait on CI.
