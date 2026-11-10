# uQ Phase 3 slice 2: Look and layouts page, Shell page — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `uq-settings` gains a Look and layouts page, which sets gaps, borders, rounding, blur,
shadow, animations, the default layout, per-workspace layouts and dwindle/master/scrolling
options. Each change is verified by Hyprland before it applies. It also gains a Shell page, which
covers pins, launcher providers, islands, the clock, the terminal, the notification backend, idle
timings and privacy. All of them apply live.

**Architecture:**
- **First, a refactor.** Slice 1's two page models are near copies. Their shared runner, QObject
  glue and fake uqd move into one place, so the two new pages are small.
- **uq-core** gains three section domains:
  - `look` and `layouts` render into `generated.lua`;
  - `shell` takes the light path (save and emit).
  - Phase 2 keys (pins, idle, notif, privacy) stay on `SetSetting`, and their storage is unchanged.
- **The bridge** reads the `shell` settings it already mirrors. It uses them for the terminal on
  `Terminal=true` launches and to switch providers on or off.
- **The shell** reads islands, clock and provider order from `Settings.values.shell`.
- **A lint rule** makes text shown inside stock controls go through `UqLabel`.

**Tech Stack:** Rust edition 2024, zbus 5, cxx-qt 0.10, Qt 6, Kirigami 6, Quickshell 0.3,
Hyprland 0.56.2–0.57.x (Lua config).

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase3-settings-design.md`. The relevant parts are
§2.1, §2.2 (Look and layouts, Shell), §2.3 (the `look`, `layouts` and `shell` rows, D-Bus
additions), §3 (untrusted text), §4, §5 and §8 slice 2. Precedent:
`docs/superpowers/plans/2026-11-10-uq-phase3-slice1-settings-foundation.md`.

## Global Constraints

These are copied from the spec, except the lines marked **ruling**, which this plan decides.

- **Hyprland settings (§1):** "Hyprland settings (monitors, input, keybinds, look) are owned by
  uqd, stored in `state.toml`, rendered into `generated.lua` and verified before they apply.
  `user.lua` still overrides them."
- **`look` (§2.3):** "`gaps_in`, `gaps_out`, `border_width`, `rounding`, blur on/off/size/passes,
  shadow on/off, animations on/off/speed". It renders into "`general`, `decoration` and
  `animations` in `generated.lua`. These override the theme's `ui` tokens; `null` means use the
  theme."
- **`layouts` (§2.3):** "default layout, per-workspace rules (existing), dwindle/master/scrolling
  options", rendered into `generated.lua`.
- **`shell` (§2.3):** "island on/off per island, launcher providers on/off and order, `clock_24h`,
  `terminal` (a desktop id, used for `Terminal=true` launches instead of the hard-coded kitty)",
  applied through "bridge `SettingsChanged`".
- **Section API (§2.3):** "`GetSection(s domain) → s` and `SetSection(s domain, s json)`. Writes
  are validated per domain. An invalid write returns `InvalidSetting` with the reason." The signal
  is `SectionChanged(s domain, s json)`.
- **Plain text (§2.1, §3):** "All text goes through `UqLabel`". "SSIDs, device names, user real
  names, plugin text, app names and `mimeapps` entries all go through `UqLabel` (plain text)."
- **Errors (§4):**
  - "uqd not running: uQ pages show 'uQ's settings service isn't running' and become read-only".
  - "`generated.lua` verify fails: uqd rejects the change; the page shows Hyprland's message in
    plain text and the control returns to its last good value".
- **Testing (§5):** "Validation for every new domain"; "Golden tests of the rendered
  `generated.lua` sections; CI also verifies them with Hyprland `--verify-config`"; "Rust unit
  tests for every page model"; "The text lint over `apps/uq-settings/qml`".
- **Ruling, load and API strictness (slice 1 precedent):**
  - Loading is lenient: `#[serde(default)]`, and unknown keys in `state.toml` are ignored.
  - `SetSection` is strict, through explicit unknown-key checks (`check_known`). Do not use
    `deny_unknown_fields`.
  - TOML has no null: a `None` field is left out of the file.
- **Ruling, `look` scope:**
  - `look` changes only what Hyprland is given. `tokens.json` and the shell keep the theme's `ui`
    tokens.
  - Blur, shadow and animations have no theme token, so for them `null` means "base.lua's value"
    and nothing is emitted.
- **Ruling, Hyprland option names:** these were checked against Hyprland v0.57.0
  `src/config/values/ConfigValues.cpp`:
  - `general:gaps_in`, `gaps_out`, `border_size` and `layout`;
  - `decoration:rounding`, `blur:{enabled,size,passes}` and `shadow:enabled`;
  - `animations:enabled`;
  - `dwindle:{preserve_split,smart_split,force_split}` (`force_split` is an int, 0–2);
  - `master:{mfact,new_status,orientation}`;
  - `scrolling:{column_width,fullscreen_on_one_column}`.

  0.56.2 was not checked. The CI `--verify-config` run (`all_variants_pass_hyprland_verify`, run
  with `--ignored`) is the gate. If it rejects a key, drop that key from the domain and the page,
  and say so in the PR. Don't work around it.
- **Ruling, Phase 2 keys stay on `SetSetting`:** `pinned`, `idle.*`, `notif.*` and `privacy.*`.
  The Shell page calls `SetSetting` for them and `SetSection("shell")` for the rest.
- **Process (as slice 1):**
  - Use private `dbus-daemon` buses only, with socket paths under 108 bytes.
  - Never use the real session bus or `hyprctl`.
  - Don't run `tests/shell/bar_loads.sh` or `scripts/smoke-shell.sh` locally.
  - `cargo fmt`, `clippy -D warnings` and `cargo test --workspace` must be clean.
  - Keep files at or under about 400 lines.
  - Run builds and tests in the foreground and leave nothing running. Don't wait on CI; report
    what you pushed.

## Review Focus

1. **A 0.2.x `state.toml`** has `[layouts]` workspace rules but no `[look]`, `[layout_options]` or
   `[shell]`. It must load with the old rules intact, and `GetSection("layouts")` must list them.
   `Settings` is flattened into `State`, which already owns `layouts`, so storage must not
   collide. → `old_state_keeps_workspace_rules` (Task 4).
2. **Hyprland rejects a look or layout value**, even one that passed uqd's own validation (for
   example a renamed option on 0.56.2). Then `generated.lua` and `state.toml` stay as they were,
   the page shows Hyprland's text, and the control snaps back. →
   `rejected_look_keeps_previous_config` (Task 6) and
   `runner::tests::refusal_returns_uqd_state_and_message` (Task 2).
3. **`shell.terminal` names an app that was uninstalled**, or whose `Exec` won't parse. A
   `Terminal=true` launch still works, falling back to kitty. It is never refused and never runs
   a broken argv. → `terminal_falls_back_to_kitty` (Task 8).
4. **Idle timings out of order.** Setting `idle.dim` above `idle.lock` from the page is refused
   by uqd's §2.4 order rule. The page shows the message, and both controls show the saved values.
   → `pages::shell::tests::idle_order_refusal_snaps_back` (Task 11).
5. **Untrusted names in stock controls.** Provider names come from `.provider` files and app
   names from desktop entries. A ComboBox or ItemDelegate showing them through its own `text:`
   would render rich text. The lint must catch a non-literal `text:` on a stock control. →
   `tests/shell/lint_settings.sh` cases `control_bound_text` and `combo_without_contentitem`
   (Task 3).

---

### Task 1: Shared private bus for tests, and the settings test split

**Files:**
- Create: `crates/uq-core/src/test_bus.rs`, behind a new feature `test-bus` (`tempfile` becomes an
  optional dependency enabled by it).
- Modify these to use it:
  - `crates/uq-core/Cargo.toml`, `crates/uq-core/src/lib.rs`;
  - `crates/uqctl/Cargo.toml` and `apps/uq-settings/Cargo.toml` (dev-dependency
    `uq-core = { workspace = true, features = ["test-bus"] }`), and `crates/uqd/Cargo.toml`
    (the same, as a path dev-dependency);
  - `crates/uqctl/tests/common/mod.rs` (keeps `Bridge`, uses the shared `Bus`),
    `crates/uqctl/src/settings_app.rs`, `crates/uqctl/src/bridge/notifowner.rs`,
    `crates/uqctl/src/bridge/logind.rs` and `crates/uqd/tests/dbus.rs`.
- Delete: `apps/uq-settings/src/test_bus.rs`. Users switch to `uq_core::test_bus`.
- Move: the `#[cfg(test)] mod tests` body of `crates/uq-core/src/settings/mod.rs` to
  `crates/uq-core/src/settings/tests.rs` (`#[cfg(test)] mod tests;`), with its code unchanged.

**Interfaces:**
- Produces:
  - `pub struct Bus { pub address: String, .. }` with `pub fn dir(&self) -> &Path`, which kills
    the daemon on drop.
  - `pub fn private_bus() -> Bus`: socket under `/tmp/uqb*`, `assert!(address.len() < 108)`.
  - `pub fn private_bus_with_services(services: &[(&str /*file*/, &str /*body*/)]) -> Bus`, for
    uqctl's `org.test.Ghost` activatable service.
  - `pub async fn connect(bus: &Bus) -> zbus::Connection`.
- One config (the permissive `<policy>` already used by `uqctl/tests/common`) serves every
  caller. Callers that passed `--session` with `--address` now use the config file.

- [ ] **Step 1:** Write `test_bus::tests::bus_starts_and_two_clients_see_each_other`. One
  connection requests `org.test.A`; a second checks `NameHasOwner`.
- [ ] **Step 2:** Run `cargo test -p uq-core --features test-bus test_bus`. Expected: FAIL (no
  module).
- [ ] **Step 3:** Implement it, then replace the six copies. `grep -rn 'Command::new("dbus-daemon")' crates apps`
  must list only `crates/uq-core/src/test_bus.rs`.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS, with the same test count as
  before plus one.
- [ ] **Step 5:** Commit: `test: one private-bus helper for every crate; split settings tests`

### Task 2: Shared page-model runner and fake uqd in uq-settings

**Files:**
- Create:
  - `apps/uq-settings/src/pages/runner.rs`: plain Rust logic;
  - `apps/uq-settings/src/model_core.rs`: generic QObject glue, not a bridge file;
  - `apps/uq-settings/src/pages/fake_uqd.rs` (`#[cfg(test)]`).
- Modify: `apps/uq-settings/src/{appearance_model,wallpaper_model}.rs`,
  `apps/uq-settings/src/pages/{appearance,wallpaper,mod}.rs` and `apps/uq-settings/src/main.rs`.

**Interfaces:**
- Produces in `pages::runner`:
  - `pub struct Outcome { pub state: Value, pub error: Option<String> }`, moved from both pages.
  - `pub fn message(e: &zbus::Error) -> String` and
    `pub fn parse_json(text: &str) -> zbus::Result<Value>` ("uqd sent bad JSON: …").
  - `pub trait PageLogic: Send + Sync + 'static`:
    - `type Op: Send + 'static`;
    - `fn refresh() -> Self::Op` and `fn is_refresh(op: &Self::Op) -> bool`;
    - `fn write_lock(&self) -> &tokio::sync::Mutex<()>`;
    - `fn apply(&self, op: Self::Op) -> impl Future<Output = zbus::Result<()>> + Send`;
    - `fn snapshot(&self) -> impl Future<Output = zbus::Result<Value>> + Send`.
  - `pub async fn run<L: PageLogic>(l: &L, op: L::Op) -> Outcome`: today's `run` body, which is
    the same in both pages (lock, apply, then snapshot, with `no_owner` errors hidden).
  - `pub async fn patch_section(proxy: &DesktopProxy<'_>, domain: &str, edit: impl FnOnce(&mut serde_json::Map<String, Value>) -> zbus::Result<()>) -> zbus::Result<()>`:
    a read-modify-write of one section, called under the write lock.
- Produces in `model_core`:
  - `pub struct Follow { pub sections: &'static [&'static str], pub theme: bool, pub settings: bool }`
    and `pub fn wants(f: &Follow, domain: &str) -> bool`.
  - `pub struct ModelCore<L: PageLogic> { in_flight: usize, logic: Option<(Handle, Arc<L>)>, watcher: Option<JoinHandle<()>> }`,
    built by `ModelCore::new(make: impl FnOnce(zbus::Connection) -> L)` from `session::bus()`.
    Its `Drop` aborts the watcher.
  - `pub trait PageModel: cxx_qt::Threading + Sized`:
    - `type Logic: PageLogic`;
    - `const FOLLOW: Follow`;
    - `fn core(&self) -> &ModelCore<Self::Logic>` and
      `fn core_mut(self: Pin<&mut Self>) -> &mut ModelCore<Self::Logic>`;
    - `fn show_busy(self: Pin<&mut Self>, busy: bool)`, `fn show_error(self: Pin<&mut Self>, e: QString)`
      and `fn show_state(self: Pin<&mut Self>, json: QString)`. These are named so they don't
      clash with the generated `set_*`.
  - `pub fn initialize<M: PageModel>(m: Pin<&mut M>)` starts `follow_uqd` and runs a refresh.
  - `pub fn start<M: PageModel>(m: Pin<&mut M>, op: <M::Logic as PageLogic>::Op)` and its private
    `finish`. These keep today's behaviour: `in_flight` counts `busy`, and a non-refresh op clears
    `error`.
  - Each `*_model.rs` keeps only its bridge, its `impl PageModel` (about 15 lines) and its
    invokables, which call `model_core::start`.
- Produces in `fake_uqd` (tests):
  - `pub struct FakeUqd` serving `org.uq.Desktop1`. It holds a real `uq_core::state::State` behind
    `Arc<Mutex<_>>`.
  - It implements `get_theme`, `set_theme`, `list_themes` (`["gruvbox","dracula"]`),
    `get_schedule`, `set_schedule`, `get_settings`, `set_setting` (through `Settings::apply`),
    `get_section`, `set_section`, `clear_launch_counts` and `get_launch_counts`.
    - `get_section` sleeps 30 ms, as both fakes do today.
    - `set_section` goes through `Settings::set_section`; Task 4 switches it to
      `State::set_section`.
    - Errors are `org.uq.Desktop1.Error.InvalidSetting` with uq-core's message.
    - `verify_error: Arc<Mutex<Option<String>>>` makes the next rendering `set_section` fail with
      `ConfigInvalid`.
  - `pub async fn rig(state: State) -> Rig { pub bus, pub uqd: zbus::Connection, pub client: zbus::Connection, pub state: Arc<Mutex<State>>, pub verify_error }`.

- [ ] **Step 1:** Port the existing page tests to `fake_uqd::rig`. Assertions on fake-specific
  messages change to uq-core's real ones. For example, `error_restores_state` now expects an error
  containing `font_size`. Add `runner::tests::{refusal_returns_uqd_state_and_message, uqd_down_is_empty_without_error, writes_are_serialised}`,
  using a tiny `PageLogic` test impl over `patch_section`. Add
  `model_core::tests::wants_filters_by_domain`.
- [ ] **Step 2:** Run `cargo test -p uq-settings`. Expected: FAIL (the modules don't exist).
- [ ] **Step 3:** Implement. `AppearanceState` and `WallpaperState` implement `PageLogic`, and
  their `run` methods are deleted. The `FOLLOW` values are:
  - Appearance: `{sections: &["appearance"], theme: true, settings: true}`;
  - Wallpaper: `{sections: &["wallpaper"], theme: false, settings: false}`.
- [ ] **Step 4:** Run `cargo test -p uq-settings`, `cargo clippy -p uq-settings --all-targets -- -D warnings`
  and the offscreen `--screenshots` run into `/tmp/uqs-shots`. Expected: PASS, and
  `appearance.png` and `wallpaper.png` look as before.
- [ ] **Step 5:** Commit: `refactor(settings): one runner, QObject core and fake uqd for page models`

### Task 3: Control-text lint

**Ruling:** a stock control's own `text:` may only be a string literal. Text from data goes in a
`contentItem: UqLabel { … }`. A `ComboBox` must declare both `contentItem:` (a `UqLabel` showing
`displayText`) and `delegate:`, because the style otherwise paints model strings itself. This is
enforced by a lint, not just a convention, so slice 5's SSIDs are covered.

**Files:**
- Create: `scripts/lint-control-text.awk`.
- Modify: `scripts/lint-settings.sh` (it runs the awk script after `qml_lint_hits`, with the same
  output format), `tests/shell/lint_settings.sh`, and every existing hit. Expected hits are the
  `QQC2.ComboBox` in `qml/components/OverrideCombo.qml` and in
  `qml/components/WallpaperEntryEditor.qml` (no `contentItem`). Fix whatever else it finds.

**Interfaces:**
- The awk script tracks brace depth from a line that opens one of
  `(QQC2.|Controls.|Kirigami.)?(Button|ToolButton|CheckBox|RadioButton|Switch|ItemDelegate|CheckDelegate|SwitchDelegate|RadioDelegate|MenuItem|TabButton|ComboBox|Kirigami.Chip)`.
  At depth 1 of that block:
  - a `text:` whose value isn't one `"..."` literal is a hit;
  - for `ComboBox`, a block that closes without both `contentItem:` and `delegate:` is a hit at
    its opening line.
- Output is `file:line:text`. `lint-settings.sh` prints
  `lint-settings: stock controls must show data through UqLabel contentItems` before the hits.

- [ ] **Step 1:** Add the cases to `tests/shell/lint_settings.sh`:
  - `bad control_bound_text Case.qml 'Item {\n  QQC2.CheckBox {\n    text: modelData.name\n  }\n}\n' 3`;
  - `bad combo_without_contentitem Case.qml 'Item {\n  QQC2.ComboBox {\n    model: m\n    delegate: D {}\n  }\n}\n' 2`;
  - `good control_literal 'Item {\n  QQC2.CheckBox { text: "Show seconds" }\n}\n'`;
  - `good combo_full` (a ComboBox with both `contentItem: UqLabel { text: c.displayText }` and a
    `delegate:`);
  - `good nested_uqlabel_text`: `text: modelData` inside the control's `contentItem: UqLabel {}`
    is at depth 2, so it is allowed.
- [ ] **Step 2:** Run `bash tests/shell/lint_settings.sh`. Expected: FAIL on the two new bad cases.
- [ ] **Step 3:** Implement the script and fix the existing hits.
- [ ] **Step 4:** Run `bash tests/shell/lint_settings.sh && scripts/lint-settings.sh`, plus the
  settings qmltestrunner suite. Expected: PASS.
- [ ] **Step 5:** Commit: `lint(settings): stock controls show data only through UqLabel`

### Task 4: `look` and `layouts` domains in uq-core

**Files:**
- Create: `crates/uq-core/src/settings/look.rs` and `crates/uq-core/src/settings/layouts.rs`.
- Modify:
  - `crates/uq-core/src/settings/mod.rs`: fields, `SECTION_DOMAINS`, `renders_section` and
    `validated`;
  - `crates/uq-core/src/settings/tests.rs`;
  - `crates/uq-core/src/state.rs`: `State::section` / `set_section`, and tests;
  - `crates/uq-core/src/lua.rs`: `workspace_selector` moves here from `crates/uqd/src/service.rs`,
    unchanged, and uqd re-imports it.

**Interfaces:**
- Produces:
  - `pub struct Look`: each field is an `Option` with `#[serde(default, skip_serializing_if = "Option::is_none")]`:
    - `gaps_in: u8` (0–50), `gaps_out: u8` (0–100), `border_width: u8` (0–20),
      `rounding: u8` (0–40);
    - `blur: bool`, `blur_size: u8` (1–20), `blur_passes: u8` (1–4);
    - `shadow: bool`;
    - `animations: bool`, `animation_speed: f64` (0.25–4.0, finite, rounded with `round2`;
      2.0 = twice as fast).
  - `Look` also has `pub const FIELDS: [&str; 10]` and `fn validate(&self) -> Result<(), SettingError>`.
  - Layout option enums use `serde(rename_all = "snake_case")`:
    - `pub enum ForceSplit { FollowMouse, Left, Right }`, emitted as 0, 1 and 2;
    - `pub enum NewStatus { Master, Slave, Inherit }`;
    - `pub enum Orientation { Left, Right, Top, Bottom, Center }`.
  - Option structs, all fields `Option`, each with `FIELDS`:
    - `pub struct DwindleOptions { preserve_split: bool, smart_split: bool, force_split: ForceSplit }`;
    - `pub struct MasterOptions { mfact: f64 /*0.1–0.9, round2*/, new_status: NewStatus, orientation: Orientation }`;
    - `pub struct ScrollingOptions { column_width: f64 /*0.1–1.0, round2*/, fullscreen_on_one_column: bool }`.
  - `pub struct LayoutOptions { default: Option<Layout>, dwindle: DwindleOptions, master: MasterOptions, scrolling: ScrollingOptions }`.
  - `Settings` gains `pub look: Look` (TOML `[look]`) and `pub layout_options: LayoutOptions`
    (TOML `[layout_options]`). This avoids the existing top-level `[layouts]` table that holds
    workspace rules.
  - `SECTION_DOMAINS` becomes `["appearance", "wallpaper", "look", "layouts", "shell"]` (`shell` is
    Task 7). `renders_section` is true for `appearance`, `look` and `layouts`.
  - `impl State { pub fn section(&self, domain: &str) -> Result<Value, SettingError>; pub fn set_section(&mut self, domain: &str, v: Value) -> Result<(), SettingError>; }`:
    - `layouts` is `{default, workspaces: {<selector>: layout}, dwindle, master, scrolling}`.
      It is built from `layout_options` plus `State::layouts`.
    - Other domains go to `Settings`.
    - In `layouts`, every key of `workspaces` is passed through `workspace_selector`. After
      normalising, a key must be 1–64 chars with no control characters, and there may be at most
      64 keys. A missing `workspaces` means none.
    - Unknown keys at any level are rejected with `check_known`.
    - On error, `self` is unchanged.
  - `Settings::validated` validates `look` and `layout_options` too.

- [ ] **Step 1:** Write the failing tests:
  - `look::tests::{defaults_all_unset, ranges_and_rounding}`. `animation_speed` 1.234 is stored
    as 1.23; 0.2, NaN and 5 are rejected; `blur_passes` 0 is rejected.
  - `layouts::tests::{enums_parse_snake_case, ranges}`.
  - `settings::tests::{look_section_round_trip, unknown_look_field_rejected}`.
  - `state::tests`:
    - `layouts_section_normalises_and_replaces_rules`: `{"workspaces":{"web":"master"}}` stores
      `name:web` and drops the old rule `2`;
    - `layouts_section_rejects_bad_input`: 65 rules, `"monocle"`, a `"\n"` key, and
      `{"master":{"mfact":0.95}}`;
    - `old_state_keeps_workspace_rules`: parse `theme = "graphite"\n[layouts]\n"2" = "scrolling"\n`,
      then `section("layouts")["workspaces"]["2"] == "scrolling"` and `look == Look::default()`.
- [ ] **Step 2:** Run `cargo test -p uq-core`. Expected: FAIL.
- [ ] **Step 3:** Implement. Point `fake_uqd` (Task 2) at `State::set_section`.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): look and layouts settings sections`

### Task 5: Render `look` and `layouts` into `generated.lua`

**Files:**
- Move: `crates/uq-core/src/lua.rs` → `crates/uq-core/src/lua/mod.rs`, with the code unchanged.
- Create:
  - `crates/uq-core/src/lua/look.rs`: the look, layout-option and animation emitters;
  - `crates/uq-core/tests/generated_golden.rs`;
  - `crates/uq-core/tests/golden/generated/default.lua` and
    `crates/uq-core/tests/golden/generated/all-set.lua`.
- Modify:
  - `config/hypr/base.lua`: the `hl.curve` and `hl.animation` lines move to `generated.lua`, and
    a comment says so;
  - `crates/uqd/src/service.rs` (the caller);
  - `crates/uqd/tests/verify_config.rs`: the generated input sets every look and layout field.

**Interfaces:**
- Consumes: `Look` and `LayoutOptions` (Task 4).
- Produces:
  - The signature becomes `pub fn emit_generated(theme: &Theme, layouts: &BTreeMap<String, Layout>, settings: &Settings) -> String`,
    reading `settings.mouse`, `settings.look` and `settings.layout_options`.
  - In `general`:
    - `gaps_in = look.gaps_in.unwrap_or(ui.gap_in)`, and the same for `gaps_out`,
      `border_size` (from `border_width`) and `rounding` (from `radius`);
    - `layout = "<default>"` only when `default` is `Some`.
  - In `decoration`:
    - `blur = { enabled, size, passes }`, holding only the keys that are `Some`; the table is
      left out when none is set;
    - `shadow = { enabled = … }` when `Some`.
  - `animations = { enabled = … }` when `Some`.
  - `dwindle`, `master` and `scrolling` tables hold only the `Some` keys; `force_split` is emitted
    as an int and enums as `lua_string`.
  - After `hl.config`, the emitter writes `pub const CURVES: [(&str, [f64; 4]); 4]` and
    `pub const ANIMATION_LEAVES: [(&str /*leaf*/, f64 /*speed*/, &str /*bezier*/, Option<&str> /*style*/); 7]`,
    copied from today's `base.lua`. Each emitted speed is `round2(speed / animation_speed.unwrap_or(1.0))`
    and is never below 0.1.
  - Floats are emitted with `{:?}`, as `emit_mouse` does.

- [ ] **Step 1:** Write the failing tests:
  - `lua::look::tests::animation_leaves_match_old_base_lua`. Its const `OLD` holds the 4
    `hl.curve` and 7 `hl.animation` lines cut from `base.lua`, byte for byte. A default render
    must contain each, in order.
  - `lua::look::tests::{unset_look_uses_theme_tokens, blur_table_only_with_set_keys, speed_scales_leaves}`.
    With speed 2.0, `windowsIn` becomes `speed = 2.05`.
  - `lua::look::tests::layout_options_emit_only_set_keys`, which asserts
    `force_split = 2` and `new_status = "master"`.
  - `generated_golden::golden_default_and_all_set`, using graphite-dark with `UQ_BLESS=1` support
    as in `render_golden.rs`.
  - `verify_config::base_lua_has_no_animation_lines`: `base.lua` contains no `hl.animation(` or
    `hl.curve(`.
- [ ] **Step 2:** Run `cargo test -p uq-core && cargo test -p uqd --test verify_config`.
  Expected: FAIL.
- [ ] **Step 3:** Implement, then bless the goldens and read both files by eye.
  `lua_mock_runs_every_variant` must still pass. Its mock already accepts `hl.curve` and
  `hl.animation`; extend it if not.
- [ ] **Step 4:** Run `cargo test --workspace`, and `cargo test -p uqd --test verify_config -- --include-ignored`
  where Hyprland is installed. Locally there is no Hyprland, so the ignored test runs in CI; note
  that in the report. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): render look, layout options and animation speed into generated.lua`

### Task 6: uqd: `look` and `layouts` through `SetSection`

**Files:**
- Modify: `crates/uqd/src/sections.rs` (and its tests), `crates/uqd/src/service.rs`,
  `crates/uq-core/src/client.rs`, and `crates/uqd/tests/dbus.rs`.

**Interfaces:**
- Consumes: `State::section` / `set_section` (Task 4) and `emit_generated` (Task 5).
- Produces:
  - `Desktop::get_section` and `set_section` call `State::section` and `State::set_section`.
  - Renders choose the change by domain: `appearance` uses `Change::Theme` as before; `look` and
    `layouts` use a new `Change::Reload`, which reloads Hyprland and sends no events, because
    `set_section` already emits `SectionChanged` and `SettingsChanged`.
  - `Change::Layout` (the `SetLayout` path) also emits `SectionChanged { domain: "layouts", json }`,
    so the page follows keybind changes.
  - `DesktopProxy` adds `set_setting(key, json)`, `get_launch_counts() -> HashMap<String, u32>`
    and `clear_launch_counts()`. The names are checked by the existing `names_match_the_proxies`.

- [ ] **Step 1:** Write the failing tests in `sections.rs`:
  - `set_look_renders_and_reloads`: `generated.lua` contains `gaps_in = 9`, and the fake recorded
    `reload` and no `setcursor`;
  - `rejected_look_keeps_previous_config`: with `verify_error = Some("bad option")`, the result is
    `ConfigInvalid("bad option")`, `generated.lua` is byte-identical, `state.toml` has no
    `[look]`, and no `SectionChanged` is sent;
  - `set_layouts_replaces_rules`;
  - `set_layout_emits_layouts_section`.

  In `tests/dbus.rs`, add `look_section_round_trip_over_dbus`.
- [ ] **Step 2:** Run `cargo test -p uqd`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): look and layouts sections, verified before they apply`

### Task 7: `shell` domain

**Files:**
- Create: `crates/uq-core/src/settings/shell.rs`.
- Modify: `crates/uq-core/src/settings/{mod,tests}.rs` and `crates/uqd/src/sections.rs` (test
  only, since the light path already exists).

**Interfaces:**
- Produces:
  - `pub struct Islands { system: bool, work: bool, time: bool, notif: bool, status: bool }`,
    each defaulting to `true`. These are the five islands in `shell/bar/Bar.qml`.
  - `pub struct ProviderPref { id: String, enabled: bool }`, where `id` is `"calc"`, `"run"` or a
    D-Bus well-known name: `zbus::names::WellKnownName::try_from` succeeds and it doesn't start
    with `:`.
  - `pub struct ShellSettings { islands: Islands, providers: Vec<ProviderPref>, clock_24h: bool, terminal: Option<String> }`:
    - `providers` has at most 64 entries, with unique ids, in the user's order;
    - `clock_24h` defaults to `true`, which is today's `HH:mm`;
    - `terminal` passes `valid_pinned_id` and must end in `.desktop`. `null` means kitty.
  - `pub const BUILTIN_PROVIDERS: [&str; 2] = ["calc", "run"];`
  - `pub fn provider_order(prefs: &[ProviderPref], ids: &[String]) -> Vec<String>`. It puts the
    ids that `prefs` lists first, in `prefs` order, then the rest: builtins in
    `BUILTIN_PROVIDERS` order, then others sorted. Disabled ids are dropped.
  - `pub fn provider_enabled(prefs: &[ProviderPref], id: &str) -> bool` is true unless the id is
    listed and disabled.
  - `Settings` gains `pub shell: ShellSettings` (TOML `[shell]`). It is a section domain that
    doesn't render.
  - The apps group stays first and can't be switched off. **Ruling:** the spec lists providers,
    and apps is not one.

- [ ] **Step 1:** Write the failing tests:
  - `shell::tests::{defaults, rejects_bad_provider_ids_and_dupes, terminal_must_be_desktop_id, provider_order_and_enabled}`.
    The bad ids are `":1.5"`, `"a"`, `"org..x"` and a duplicate `"calc"`. For ordering,
    `prefs=[run, org.x.Y(off)]` and `ids=[calc, run, org.x.Y, org.a.B]` give
    `[run, calc, org.a.B]`.
  - `sections::tests::set_shell_saves_and_emits_without_render`.
- [ ] **Step 2:** Run `cargo test -p uq-core shell && cargo test -p uqd sections`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): shell settings section (islands, providers, clock, terminal)`

### Task 8: Bridge: the chosen terminal, and provider on/off

**Files:**
- Create: `crates/uq-core/src/launcher_providers.rs`. `ProviderDef`, `parse_def`, `load_defs` and
  `default_dirs` move here unchanged from `crates/uqctl/src/bridge/providers.rs`, so the Settings
  app can list installed providers. Their tests move too.
- Modify: `crates/uqctl/src/bridge/launch.rs` and `crates/uqctl/src/bridge/providers.rs`.

**Interfaces:**
- Consumes: `ShellSettings` (Task 7), and `Link::settings_rx()` (`crates/uqctl/src/bridge/uqd.rs:106`).
- Produces:
  - `plan(args, dirs, rand, runs, terminal: Option<&str>)`. `launch()` passes
    `link.settings_rx().borrow().as_ref().and_then(|s| s.shell.terminal.clone())`.
  - `fn terminal_prefix(dirs: &Dirs, terminal: Option<&str>) -> Vec<String>`:
    - It resolves the desktop id through `dirs.find` and `parse_entry`, which reads
      `X-TerminalArgExec` as well.
    - It returns `parse_exec(Exec)` followed by the `X-TerminalArgExec` value, or `-e` if there
      is none.
    - It returns `["kitty", "-e"]` when `terminal` is `None`, the entry is missing or `Hidden`,
      its `TryExec` is absent, its `Exec` fails to parse, or the terminal entry itself has
      `Terminal=true`.
  - `Providers::search` skips the `calc` and `run` builtins, and every external def, for which
    `provider_enabled` is false. It reads the same `settings_rx` snapshot; with no settings yet,
    everything is on.

- [ ] **Step 1:** Write the failing tests in `launch.rs`:
  - `terminal_uses_chosen_entry`: `foot.desktop` with `Exec=foot` gives a command ending
    `-- 'foot' '-e' 'htop'`;
  - `terminal_arg_from_x_terminal_arg_exec`: `Exec=gnome-terminal` and `X-TerminalArgExec=--`
    give `'gnome-terminal' '--' 'htop'`;
  - `terminal_falls_back_to_kitty`, for a missing id, `Exec=bad %d`, and `Hidden=true`.

  In `providers.rs`, add `disabled_builtin_and_external_are_not_asked`, using the existing
  provider test harness with `Settings.shell.providers = [{calc, false}, {org.test.P, false}]`:
  no `calc` results event, and the fake provider receives no `Search`.
- [ ] **Step 2:** Run `cargo test -p uqctl`. Expected: FAIL.
- [ ] **Step 3:** Implement. Rename `terminal_apps_get_kitty_and_bad_entries_refuse` to say it
  checks the fallback.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(bridge): Terminal=true launches use the chosen terminal; providers can be switched off`

### Task 9: Shell: islands, clock and provider order

**Files:**
- Modify: `shell/bar/Bar.qml`, `shell/bar/TimeIsland.qml`, `shell/logic/rank.js` and
  `shell/launcher/Spotlight.qml`.
- Create: `shell/logic/shellprefs.js` and `tests/qml/tst_shellprefs.qml`.
- Modify: `tests/qml/tst_rank.qml`.

**Interfaces:**
- Consumes: `Settings.values.shell`, which arrives through `SettingsChanged` or a refresh on
  `SectionChanged`; both are already wired in `shell/core/Settings.qml`.
- Produces in `shellprefs.js` (pure):
  - `islandOn(shell, id)` is true unless `shell.islands[id] === false`;
  - `clockFormat(shell)` is `"HH:mm"` unless `clock_24h === false`, then `"h:mm AP"`;
  - `providerOrder(shell)` is the list of enabled ids in `shell.providers` order.
- `rank.merge(apps, providerGroups, generation, order)`: the optional `order` replaces
  `BUILTIN_ORDER`. Ranking is the index in `order`, then `BUILTIN_ORDER`, then name, as now.
- `Bar.qml` binds each island's `visible` to `ShellPrefs.islandOn(Settings.values.shell, "<id>")`.
  A hidden island takes no space in the `RowLayout`.

- [ ] **Step 1:** Write the failing tests:
  - `tst_shellprefs.qml`: `test_island_defaults_on` (undefined, `{}` and a missing key all mean
    on), `test_clock_format`, `test_provider_order`;
  - `tst_rank.qml`: `test_merge_respects_order`, where `["run","calc"]` puts run before calc.
- [ ] **Step 2:** Run `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the QML suite and `scripts/lint-shell.sh`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(shell): islands, clock format and provider order follow settings`

### Task 10: Look and layouts page

**Files:**
- Create:
  - `apps/uq-settings/src/look_model.rs` (bridge; it sits in `src/` per QTBUG-93443);
  - `apps/uq-settings/src/pages/look.rs`;
  - `apps/uq-settings/qml/pages/LookPage.qml`;
  - `apps/uq-settings/qml/components/OverrideSwitch.qml`: a `SettingRow` holding a "Use default"
    `CheckBox` and a `Switch`, in the style of `OverrideSpin`;
  - `apps/uq-settings/qml/components/OverrideSlider.qml`: a float slider, debounced 400 ms like
    `OverrideSpin`, with "Use default";
  - `tests/settings-qml/tst_override_switch.qml`.
- Modify:
  - `apps/uq-settings/build.rs`: add the bridge file and the 3 QML files;
  - `apps/uq-settings/src/registry.rs`: the page and its search entries;
  - `apps/uq-settings/src/main.rs`.

**Interfaces:**
- Consumes: `runner`, `model_core` and `fake_uqd` (Task 2), and the `look` and `layouts` JSON
  (Task 4).
- Produces:
  - `pub enum Op { Refresh, SetLook { field: String, value: Value }, SetLayoutOption { group: String /*"" | dwindle | master | scrolling*/, field: String, value: Value }, SetWorkspace { selector: String, layout: Option<String> /*None removes*/ } }`.
  - `LookState: PageLogic`. Its snapshot is `{look, layouts, tokens: {gap_in, gap_out, border_width, radius}}`,
    where the tokens come from `App.themeJson` in QML, not Rust.
  - It uses `patch_section`. `group == ""` with `field == "default"` sets `layouts.default`.
  - Unknown fields are refused before any call, with `unknown field "<f>"`, as in Appearance.
  - `LookModel` has `FOLLOW = {sections: &["look", "layouts"], theme: true, settings: false}`.
    Its invokables are `refresh`, `setLook(field, json)`, `setLayoutOption(group, field, json)`
    and `setWorkspace(selector, layout_or_empty)`.
  - Registry: `PageInfo { id: "look", name: "Look and layouts", icon: "preferences-desktop-effects", category: Personal, qml: "pages/LookPage.qml" }`.
    Its search ids are `gaps-inner`, `gaps-outer`, `border-width`, `rounding`, `blur`,
    `blur-size`, `blur-passes`, `shadow`, `animations`, `animation-speed`, `default-layout`,
    `workspace-layouts`, `dwindle-options`, `master-options` and `scrolling-options`. Every one is
    a `SettingRow` id in `LookPage.qml`.
  - The page's sections are:
    - **Windows:** the gaps, border and rounding are `OverrideSpin`s, with their fallback from the
      theme tokens.
    - **Effects:** blur, size and passes, and shadow, as `OverrideSwitch` and `OverrideSpin`.
    - **Motion:** animations as `OverrideSwitch`, and speed as `OverrideSlider` from 0.25 to 4,
      labelled "Slower" and "Faster".
    - **Layouts:** the default layout as an `OverrideCombo`; the workspace rules as a list with a
      layout combo and Remove on each rule, plus an Add row (a selector field and a combo); and
      the three option groups, each shown only when relevant.
  - Combo item labels are literal; the shown text uses `contentItem` per Task 3.

- [ ] **Step 1:** Write the failing tests:
  - `pages::look::tests::{set_look_keeps_other_fields, set_option_in_group, workspace_add_and_remove, rejected_verify_restores_state, unknown_field_refused}`.
    `rejected_verify_restores_state` uses `fake_uqd`'s `verify_error`: the outcome's error equals
    the Hyprland message, and the state equals the state before the change.
  - `registry::tests::ids_unique_and_categories_ordered`, which already exists, now covers `look`.
  - `tst_override_switch.qml`: ticking "Use default" emits `chosen(null)`, and toggling the switch
    emits `chosen(true)`.
- [ ] **Step 2:** Run `cargo test -p uq-settings` and the settings QML suite. Expected: FAIL.
- [ ] **Step 3:** Implement. With uqd down, the page shows the §4 message and disables its
  controls, as Appearance does.
- [ ] **Step 4:** Run the tests, `scripts/lint-settings.sh`, and `--screenshots`. Check
  `look.png` by eye.
- [ ] **Step 5:** Commit: `feat(settings): Look and layouts page`

### Task 11: Shell page

**Files:**
- Create:
  - `apps/uq-settings/src/shell_model.rs` (bridge);
  - `apps/uq-settings/src/pages/shell.rs`;
  - `apps/uq-settings/src/desktop_entries.rs`;
  - `apps/uq-settings/qml/pages/ShellPage.qml`;
  - `apps/uq-settings/qml/components/OrderedToggleList.qml`: rows with a Switch, up and down
    buttons, and the name in a `UqLabel`;
  - `tests/settings-qml/tst_ordered_toggle_list.qml`.
- Modify: `build.rs`, `registry.rs` and `main.rs`.

**Interfaces:**
- Consumes: `SetSetting` keys (`pinned`, `idle.*`, `notif.backend`, `notif.show_summary`,
  `privacy.public_ip_lookup`, `privacy.record_launches`), `ClearLaunchCounts`, the `shell`
  section, and `uq_core::launcher_providers::{load_defs, default_dirs}` (Task 8).
- Produces:
  - `desktop_entries::scan(dirs: &[PathBuf]) -> Vec<AppEntry { id, name, icon, terminal_emulator: bool }>`.
    - It reads `[Desktop Entry]` `Name`, `Icon`, `Categories` (`TerminalEmulator`), `Hidden` and
      `NoDisplay`.
    - Earlier dirs win per id, and the result is sorted by name.
    - The dirs are `$XDG_DATA_HOME/applications` and then `/usr/share/applications`.
    - `Name` is at most 200 chars with control characters stripped. It is shown through `UqLabel`
      only.
  - `pub enum Op { Refresh, SetSetting { key: String, value: Value }, SetShell { field: String, value: Value }, ClearLaunchCounts }`.
    `SetShell` patches `islands.<id>` when `field` is `islands.<id>`.
  - `ShellState` snapshot:
    `{settings: <GetSettings>, shell: <section>, providers: [{id, name, builtin}], apps: [AppEntry], terminals: [AppEntry]}`.
    - `providers` lists the builtins (`calc` "Calculator", `run` "Run"), then installed defs, then
      ids found only in `shell.providers`, marked `missing: true`.
    - `terminals` is the apps with `terminal_emulator`.
  - `ShellModel` has `FOLLOW = {sections: &["shell"], theme: false, settings: true}`.
  - Registry: `PageInfo { id: "shell", name: "Shell", icon: "preferences-desktop", category: Uq, qml: "pages/ShellPage.qml" }`.
    Its search ids are `pins`, `providers`, `islands`, `clock-24h`, `terminal`, `notif-backend`,
    `notif-summary`, `idle-dim`, `idle-lock`, `idle-screen-off`, `idle-suspend`,
    `idle-locked-screen-off`, `idle-media`, `privacy-ip`, `privacy-launches` and
    `clear-launch-counts`.
  - The page's sections are:
    - **Pinned apps:** an `OrderedToggleList` without the switch (reorder and remove), showing
      names from `apps`, or the raw id when not installed.
    - **Launcher:** providers as an `OrderedToggleList`.
    - **Bar:** five island switches and a 24-hour clock switch.
    - **Terminal:** a combo of `terminals`, plus "Default (kitty)".
    - **Notifications:** the backend combo (`auto`, `notifi`, `builtin`, `swaync`, `mako`,
      `dunst`) and the summary switch.
    - **Idle:** an off-able spin for each stage, in minutes, with the order rule explained in a
      `UqLabel`. The locked screen-off is in seconds, 10–600. Media blocks lock is a switch.
    - **Privacy:** two switches and "Clear launch history", which asks to confirm in a
      `Kirigami.PromptDialog` first.

- [ ] **Step 1:** Write the failing tests:
  - `desktop_entries::tests::{scan_reads_names_and_terminal_category, user_dir_wins_and_hidden_skipped, name_control_chars_stripped}`;
  - `pages::shell::tests`:
    - `set_shell_keeps_other_fields`;
    - `islands_patch_one`;
    - `providers_list_marks_missing`;
    - `pins_go_through_set_setting`, which also checks that a bad id is refused with uq-core's
      message;
    - `idle_order_refusal_snaps_back`: with lock at 300, setting `idle.dim` to 400 returns an
      error and `state.settings.idle.dim == 240`;
    - `clear_launch_counts_calls_uqd`.
  - `tst_ordered_toggle_list.qml`: moving row 1 up emits `reordered(["b","a","c"])`, toggling
    emits `toggled("b", false)`, and the first row's up button is disabled.
- [ ] **Step 2:** Run `cargo test -p uq-settings` and the settings QML suite. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the tests, the lint, and `--screenshots`. Check `shell.png` by eye.
- [ ] **Step 5:** Commit: `feat(settings): Shell page`

### Task 12: Docs, changelog and the whole gate

**Files:**
- Create: `docs/notes/settings-slice2-live-check.md`.
- Modify: `CHANGELOG.md` (Unreleased) and `README.md` (the Settings section lists the new pages,
  and says that animations now come from `generated.lua`, so `user.lua` overrides of
  `hl.animation` still win).

The manual checklist covers:
- each look control changes live windows;
- blur and shadow off and on;
- animation speed 0.5 and 2;
- a workspace rule added from the page applies on that workspace, and removing it reverts after
  the reload;
- `SUPER+ALT+M` updates the page's rule list;
- a value that Hyprland on the installed version rejects shows its message and snaps back;
- hiding the Status island frees its space;
- the 12-hour clock;
- turning calc off stops calculator results;
- a provider's order changes the Spotlight section order;
- choosing foot as the terminal opens `htop.desktop` in foot, and uninstalling foot falls back to
  kitty;
- an idle order refusal shows its message;
- Clear launch history empties the ranking.

- [ ] **Step 1:** Write the files.
- [ ] **Step 2:** Run the whole gate:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `cargo test --workspace`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - both qmltestrunner suites;
  - `tests/shell/*.sh` except `bar_loads.sh`.

  Expected: all PASS.
- [ ] **Step 3:** Commit: `docs: Settings slice 2 notes, changelog and live checklist`. Push the
  branch and report. CI runs `--verify-config` and the smoke test. Don't wait on it.
