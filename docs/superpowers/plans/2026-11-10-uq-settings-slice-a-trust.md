# uQ Settings enhancements slice A: value sources, undo and history, quality rules — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Each setting row in `uq-settings` shows where its live value comes from, and warns when
`user.lua` or a runtime change overrides it. Every change to uQ's settings is journalled, so it
can be undone from the app (Ctrl+Z), a History page or `uqctl`. Restore points bring back a whole
earlier state in one step. Every page follows the §9 quality rules.

**Architecture:**
- **uq-core** gains `history` (origins, the per-domain diff, the journal and restore points) and
  `settings::provenance` (the key → Hyprland option table, and the `user.lua` line finder). Both
  are plain Rust with unit tests.
- **uqd: one save point.** It gets a single save point, `Desktop::persist(next, origin)`. Today's
  three save sites (`commit`, the light path in `set_setting`, and the non-render path in
  `set_section`) all go through it, so history is recorded in one place.
- **uqd: new D-Bus methods.** It gains `Explain`, `Reapply`, history and restore methods, and an
  `origin` argument on every write method.
- **uq-settings** gets source labels on rows, an undo/redo stack, a "Changed this session" panel, a
  History page, a jargon lint and remembered window state.

**Tech Stack:** Rust edition 2024, zbus 5, cxx-qt 0.10, Qt 6, Kirigami 6, Hyprland 0.56.2–0.57.x
(Lua config).

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. This plan covers
§2, §3, §9 and §12 slice A. Precedents: `docs/superpowers/plans/2026-11-10-uq-phase3-slice2-look-shell.md`
(page runner, fake uqd, section API) and the Phase 3 spec §2.3.

## Global Constraints

These are copied from the spec, except the lines marked **ruling**, which this plan decides.

- **`user.lua` (§1 criterion 8, §2):** "None of the above writes to `user.lua`, ever." "`user.lua` is
  read only, never executed or written by uqd."
- **Sources (§2):** `theme`, `uq`, `profile`, `user_lua`, `runtime`, `error`. `profile` is never
  produced in this slice; slice B adds it.
- **Ruling, `default` source:** a `null` key whose fallback is base.lua or Hyprland's default,
  not a theme token (blur, shadow, animations, layout options), reports `default`. The app shows it
  as "Default", muted. The spec's `theme` covers only keys backed by theme tokens: gaps, border
  width, rounding, fonts, cursor and icons.
- **When sources are computed (§2):** "Only when a page opens, after `SectionChanged`, and after
  Hyprland's `configreloaded` event. It never polls."
  - **Ruling:** uqd doesn't listen to Hyprland events today, so the app re-asks when its window
    becomes active instead of after `configreloaded`. That covers the "removed the line from
    user.lua, then came back" case in §1 criterion 1.
- **Journal (§3):** entries `{id, time, origin, domain, summary, before, after}`; origins
  `settings`, `uqctl`, `shell`, `profile:<id>`, `restore`, `import` (plus `unknown`). Location:
  "`~/.local/state/uq/history/` as one JSON line per entry, rotated daily". Retention: "30 days or
  500 entries, whichever is smaller" (§11 decision 6). "Launch counts, the theme schedule's own
  automatic flips, and unconfirmed monitor applies are never recorded."
- **Restore points (§3):** "full copies of `state.toml` in `~/.local/state/uq/restore/<id>.toml`,
  plus a name and time". "Automatic points: once a day on the first change, before a profile
  switch, before an import, and when uqd starts on a new uQ version. Manual points … are never
  rotated away. Up to 20 automatic points are kept."
- **Ruling, D-Bus origin:** D-Bus has no optional arguments. So `origin` is a required last
  argument on `SetSection`, `SetSetting`, `SetTheme`, `SetLayout` and `SetSchedule`, and `""`
  means `unknown`. Every caller is in this repo. Nothing in `~/Projects` outside uq calls
  `org.uq.Desktop1` (checked 2026-11-10).
- **Coordination ruling (agreed with the slice 3 thread):** history is recorded only in
  `Desktop::persist`. Slice 3's `ConfirmMonitors(s token, s origin)` will call `commit` /
  `persist`. `ApplyMonitors` must never reach `persist`. Slice 3 adds its `monitors` and `input`
  keys to the provenance table (Task 5).
- **Plain text:** every string from uqd (summaries, Hyprland messages, `user.lua` paths) is shown
  through `UqLabel`.
- **Quality rules (§9):** plain names in labels, two clicks or one search, the window remembers
  its size, maximised state and last page, no silent failure, no settings that reset themselves,
  everything works from a keyboard.
  - **Ruling:** window state goes in `~/.local/state/uq/settings-app.conf` through QtCore
    `Settings` (INI). The spec's `settings-app.toml` would need a hand-written TOML writer for
    three values. Hyprland ignores the size for a tiled window. It applies when the window floats.

## Review Focus

1. **Crash between saving `state.toml` and appending the journal.** The journal misses one
   entry, and that's all. `state.toml` stays the source of truth, and uqd never refuses to start
   over a bad journal line. Task 1 tests that a truncated last line is skipped. Task 3 tests that
   a journal write failure still returns `Ok` and logs.
2. **Undo after the same key changed again.** The undo must ask first, not overwrite silently.
   Task 4 tests `Undo(id, false)` → `Conflict`, and `Undo(id, true)` → applied.
3. **Restore of a point whose theme was since uninstalled.** It's refused as a whole, with the
   unknown theme named, and nothing changes. Task 4 test.
4. **A `user.lua` that mentions an option in a comment or another table.** The line finder may
   return that line. It's a hint, and the label says "line N" without claiming more. The scan must
   never panic on non-UTF-8 or huge files: it reads at most 1 MiB and decodes lossily. Task 5
   tests a commented match, a 2 MiB file and invalid UTF-8.
5. **Undo/redo stack after a write from somewhere else.** If the user presses Ctrl+Z in the app
   after a `uqctl` change, the app undoes only its own last entry. The `uqctl` change is listed in
   "Changed this session" and isn't touched. Task 7 test.

---

### Task 1: `uq_core::history` — origins, diff and the journal

**Files:**
- Create: `crates/uq-core/src/history/mod.rs`, `crates/uq-core/src/history/diff.rs`,
  `crates/uq-core/src/history/journal.rs`.
- Modify: `crates/uq-core/src/lib.rs` (`pub mod history;`), `crates/uq-core/src/paths.rs` (add
  `history_dir` = `<state>/history`, `restore_dir` = `<state>/restore`, and their path tests).

**Interfaces:**
- Produces:
  - `pub enum Origin { Settings, Uqctl, Shell, Profile(String), Restore, Import, Schedule, Unknown }`
    with `Origin::parse(&str) -> Origin` (`"" | unknown text → Unknown`, `"profile:<id>"` →
    `Profile`) and `as_str() -> String`. `Schedule` is never journalled.
  - `pub struct DomainChange { pub domain: String, pub before: serde_json::Value, pub after: serde_json::Value, pub summary: String }`.
  - `pub fn diff(old: &State, new: &State) -> Vec<DomainChange>`. Domains are compared through
    `State::section(domain)` for every name in `settings::SECTION_DOMAINS` plus `"layouts"`,
    plus the pseudo-domains `"theme"` (`{theme, variant}`), `"schedule"` and `"settings"` (the
    `SETTING_KEYS` values). `launch_counts` is never compared.
  - `pub fn summarize(domain: &str, before: &Value, after: &Value) -> String`. One line, at most 3
    changed leaves named and then "and N more". It uses the setting's search label where the
    provenance table (Task 5) knows it, else the JSON key. Values are rendered with `null` as
    "default". Example: `"Gaps between windows: 5 → 8"`.
  - `pub struct Entry { pub id: String, pub time: i64 /* unix secs */, pub origin: String, pub domain: String, pub summary: String, pub before: Value, pub after: Value }`
    (serde). `id` = `"{time}-{seq:04}"`, unique within the process.
  - `pub struct Journal { dir: PathBuf }` with:
    - `open(dir)`;
    - `append(&self, entries: &[Entry]) -> io::Result<()>`, which appends JSON lines to
      `history-YYYY-MM-DD.jsonl` (local date) and fsyncs;
    - `list(&self, limit: usize) -> Vec<Entry>`, newest first, skipping unparsable lines;
    - `get(&self, id) -> Option<Entry>`;
    - `prune(&self, now: i64)`, which deletes whole files older than 30 days, then drops the
      oldest entries until at most 500 remain (rewriting the oldest kept file atomically).
    - Constants: `MAX_AGE_DAYS = 30`, `MAX_ENTRIES = 500`.

- [ ] **Step 1: Write the failing tests** in `history/diff.rs` and `history/journal.rs`:
  - `diff_reports_only_changed_domains`: change `look.gaps_in` 5→8. One change, domain `look`,
    summary `"Gaps between windows: 5 → 8"`.
  - `diff_ignores_launch_counts`.
  - `diff_theme_pseudo_domain`.
  - `summary_caps_at_three_leaves`: 5 changed keys → `"…, and 2 more"`.
  - `journal_round_trip_newest_first`.
  - `journal_skips_truncated_last_line`.
  - `prune_drops_files_older_than_30_days`.
  - `prune_keeps_at_most_500`.
  - `origin_parse_round_trip` (including `"profile:gaming"` and `""` → `Unknown`).
- [ ] **Step 2:** Run `cargo test -p uq-core history`. Expected: FAIL (module missing).
- [ ] **Step 3:** Implement. `summarize` takes the label lookup as `&dyn Fn(&str, &str) -> Option<&'static str>`
  so Task 5 can plug the provenance table in without a cycle. Until then, pass `|_, _| None`.
- [ ] **Step 4:** Run `cargo test -p uq-core`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): settings history journal and per-domain diff`

### Task 2: Restore points

**Files:**
- Create: `crates/uq-core/src/history/restore.rs`.

**Interfaces:**
- Produces:
  - `pub enum PointKind { Auto(AutoReason), Manual }`.
  - `pub enum AutoReason { Daily, BeforeRestore, BeforeProfile, BeforeImport, NewVersion }` (serde
    snake_case).
  - `pub struct RestorePoint { pub id: String, pub name: String, pub time: i64, pub kind: PointKind }`.
  - `pub struct RestoreStore { dir: PathBuf }` with:
    - `save(&self, state: &State, name: &str, kind: PointKind, now: i64) -> io::Result<RestorePoint>`,
      which writes `<id>.toml` (the state, via `State::save`) and `<id>.meta.json` atomically;
    - `list(&self) -> Vec<RestorePoint>`, newest first;
    - `load(&self, id) -> io::Result<State>`, through the lenient `State` load;
    - `delete(&self, id) -> io::Result<()>`;
    - `last_auto(&self, reason) -> Option<RestorePoint>`;
    - `prune(&self)`, which keeps the newest 20 `Auto` points and never removes `Manual` ones.
  - Point names are plain text, at most 100 chars, with control characters stripped. Ids match
    `[0-9]{10}-[0-9a-f]{6}` and `load`/`delete` refuse any other id, so a D-Bus caller can't reach
    a path.

- [ ] **Step 1: Write the failing tests:**
  - `save_load_round_trip`;
  - `prune_keeps_20_autos_and_all_manual` (25 autos + 2 manual → 22);
  - `bad_id_is_refused` (`"../x"`, `"x"`, `""`);
  - `name_is_capped_and_stripped`;
  - `last_auto_by_reason`.
- [ ] **Step 2:** Run `cargo test -p uq-core restore`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p uq-core`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(core): restore points`

### Task 3: uqd — one save point, with history

**Files:**
- Modify: `crates/uqd/src/service.rs`, `crates/uqd/src/sections.rs`, and `crates/uqd/src/main.rs`
  (construct the `Journal`/`RestoreStore` from `Paths`).

**Interfaces:**
- Consumes: Task 1's `Origin`, `diff`, `Entry` and `Journal`; Task 2's `RestoreStore` and `PointKind`.
- Produces:
  - `pub(crate) fn persist(&mut self, next: State, origin: Origin) -> Result<(), DesktopError>`. It
    saves `state.toml` (the existing `map_err(io_err("saving state"))`). Unless `origin` is
    `Schedule` or the diff is empty, it then:
    1. saves an `Auto(Daily)` restore point of the *old* state if none exists for today's local
       date;
    2. appends one `Entry` per `DomainChange`;
    3. prunes both stores;
    4. sends `Event::HistoryChanged`.

    A journal or restore failure is logged with `tracing::warn!` and never fails the write.
    `self.state = next` happens inside `persist`.
  - `commit(&mut self, next: State, change: Change, origin: Origin)`. Its `next.save(...)` and
    `self.state = next` become `self.persist(next, origin)?`.
  - The `set_setting` light path and the `sections.rs` non-render path call `persist`.
  - `set_theme`, `set_layout`, `set_schedule`, `set_setting` and `set_section` each take an
    `origin: Origin` as their last parameter. `tick_schedule_at` and `startup` pass
    `Origin::Schedule`. `reload` passes `Origin::Unknown`, and its diff is empty anyway.
  - `startup` also saves an `Auto(NewVersion)` point when the newest `NewVersion` point's name
    isn't `env!("CARGO_PKG_VERSION")`. The point's name is the version.
  - `Event::HistoryChanged` is a new variant.

- [ ] **Step 1: Write the failing tests** in `service.rs`, using the existing `rig()`:
  - `section_write_appends_one_history_entry` (`look.gaps_in`; entry origin `settings`, domain
    `look`);
  - `light_setting_write_is_journalled`;
  - `schedule_tick_is_not_journalled`;
  - `rejected_verify_writes_no_entry`;
  - `first_change_of_the_day_saves_a_daily_point` (a second change the same day saves none);
  - `journal_failure_does_not_fail_the_write` (make `history_dir` a file);
  - `startup_saves_new_version_point_once`;
  - `history_changed_event_sent`.
- [ ] **Step 2:** Run `cargo test -p uqd`. Expected: FAIL.
- [ ] **Step 3:** Implement. Update every internal caller and test to pass an origin. Tests that
  don't care use `Origin::Settings`.
- [ ] **Step 4:** Run `cargo test -p uqd`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): one save point that records settings history`

### Task 4: D-Bus — origin argument, history and restore methods, every caller

**Files:**
- Modify: `crates/uqd/src/dbus.rs`, `crates/uqd/src/service.rs` (undo and restore logic),
  `crates/uq-core/src/client.rs`, `crates/uqctl/src/main.rs`, `crates/uqctl/src/settings_cmd.rs`,
  `crates/uqctl/src/bridge/*` (every write call), `apps/uq-settings/src/pages/*.rs` and
  `apps/uq-settings/src/*_model.rs` (pass `"settings"`), `apps/uq-settings/src/pages/fake_uqd.rs`,
  `crates/uqd/tests/dbus.rs` and `README.md` (the `uqctl` command list).

**Interfaces:**
- Consumes: Task 3's `persist`/`commit` with an origin.
- Produces (D-Bus, `org.uq.Desktop1`):
  - Writes gain a last `s origin` argument: `SetSection(s,s,s)`, `SetSetting(s,s,s)`,
    `SetTheme(s,s,s)`, `SetLayout(s,s,s)` and `SetSchedule(s,s)`. The bridge passes `"shell"`,
    `uqctl` passes `"uqctl"`, and uq-settings passes `"settings"`.
  - `ListHistory(u limit) → s` (JSON array of `Entry`, `limit` capped at 500).
  - `Undo(s entry_id, b force) → s new_entry_id`:
    1. It writes the entry's `before` back into its domain through the normal path, with origin
       `restore`.
    2. If a newer entry exists for the same domain and `force` is false, it returns
       `org.uq.Desktop1.Error.Conflict`, with the message
       `"{label} was changed again after this. Undo anyway?"`.
    3. An unknown id returns `InvalidArgument`.
    4. The pseudo-domains `theme`, `schedule` and `settings` undo through `set_theme`,
       `set_schedule` and `set_setting` respectively.
  - `ListRestorePoints() → s`, `SaveRestorePoint(s name) → s id`, `DeleteRestorePoint(s id)`.
  - `Restore(s id)`:
    1. It loads the point and validates its theme exists, refusing otherwise with
       `UnknownTheme(..)` and changing nothing.
    2. It saves an `Auto(BeforeRestore)` point.
    3. It then calls `commit(point_state_with_current_launch_counts, Change::Theme, Origin::Restore)`.
       `Change::Theme` re-renders and reloads everything.
  - Signal `HistoryChanged()`, emitted from `Event::HistoryChanged`.
  - `UqError::Conflict(String)` and `DesktopError::Conflict(String)`.
  - `DesktopProxy` gains every method above (the `names_match_the_proxies` test covers it).
  - `uqctl`:
    - `uqctl history [--limit N]` prints `id  time  origin  summary`;
    - `uqctl undo [<id>] [--force]` (no id means the newest entry);
    - `uqctl restore list|save <name>|delete <id>|<id>`.

- [ ] **Step 1: Write the failing tests** in `crates/uqd/tests/dbus.rs`:
  - `set_section_with_origin_records_it`;
  - `undo_round_trip_over_dbus`;
  - `undo_conflict_then_force`;
  - `undo_unknown_id_is_invalid_argument`;
  - `restore_brings_back_old_state_and_is_undoable` (after `Restore`, `Undo` of the restore entry
    returns the pre-restore value);
  - `restore_with_missing_theme_changes_nothing`;
  - `restore_point_crud`.

  Also, in `crates/uqctl` tests, add `history_lines_format`.
- [ ] **Step 2:** Run `cargo test --workspace`. Expected: FAIL.
- [ ] **Step 3:** Implement. Update every caller (`grep -rn '\.set_section(\|\.set_setting(\|\.set_theme(\|\.set_layout(\|\.set_schedule(' crates apps`).
- [ ] **Step 4:** Run `cargo test --workspace` and `tests/shell/bridge_io.sh`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): history, undo and restore over D-Bus; writes carry their origin`

### Task 5: Provenance — `Explain` and `Reapply`

**Files:**
- Create: `crates/uq-core/src/settings/provenance.rs`.
- Modify: `crates/uq-core/src/settings/mod.rs`, `crates/uqd/src/compositor.rs` (trait method plus
  the fake), `crates/uqd/src/hypr.rs`, `crates/uqd/src/system.rs` (trait method plus the fake),
  `crates/uqd/src/service.rs` (track the last rejection, `explain`), `crates/uqd/src/dbus.rs`,
  `crates/uq-core/src/client.rs`, and `crates/uq-core/src/history/diff.rs` (use the label lookup).

**Interfaces:**
- Produces:
  - `pub struct OptionRow { pub key: &'static str /* "look.gaps_in" */, pub setting_id: &'static str /* "gaps-inner" */, pub label: &'static str, pub hypr: Option<&'static str> /* "general:gaps_in" */, pub gsettings: Option<(&'static str, &'static str)>, pub null_source: NullSource }`,
    with `pub enum NullSource { Theme, Default }`.
  - `pub static OPTIONS: &[OptionRow]`. It covers every `look` field, the `layouts` options
    (`default` → `general:layout`, `dwindle.*` → `dwindle:*`, `master.*` → `master:*`,
    `scrolling.*` → `scrolling:*`) and the `appearance` fonts, cursor and icon theme (gsettings
    `org.gnome.desktop.interface` `font-name`, `monospace-font-name`, `cursor-theme`,
    `cursor-size`, `icon-theme`). Hyprland option paths are checked against the pinned 0.56.2
    source at implementation time. A test asserts each `key` exists in `State::section`'s JSON and
    each `setting_id` exists in `apps/uq-settings`' registry (via a shared list in uq-core, or a
    test in uq-settings that iterates `OPTIONS`).
  - `pub fn label_for(domain: &str, json_key: &str) -> Option<&'static str>` for Task 1's
    `summarize`.
  - `pub fn find_in_user_lua(text: &str, hypr_path: &str) -> Option<u32>` returns a 1-based line.
    It matches, case-sensitively, any of:
    - `"<section>.<leaf>"`;
    - `"<section>:<leaf>"`;
    - a line with `<leaf> =` inside a `<section> = {` table, tracked by brace depth.

    It's a heuristic scan of plain text. The first hit wins.
  - `Compositor::get_option(&self, path: &str) -> Result<serde_json::Value, CompositorError>`
    sends `j/getoption <path>` over the request socket and returns the value field (`int`, `float`,
    `str` or `custom`). The fake gets a settable map.
  - `SystemSettings::read(&self, schema: &str, key: &str) -> Option<String>` runs
    `gsettings get`. The fake gets a map.
  - `Desktop::explain(&self, domain: &str) -> Result<Value, DesktopError>` returns
    `{ "<key>": {source, live, stored, detail} }`:
    - `source` is one of `theme`, `default`, `uq`, `user_lua`, `runtime` or `error`.
    - `detail` is `{line, path}` for `user_lua` and `{message}` for `error`.
    - It compares the rendered value (`stored` or the theme/default fallback) with `live`.
    - `user.lua` is read only when there is a mismatch, at most 1 MiB, decoded with
      `String::from_utf8_lossy`.
    - When the compositor is down, every key reports `uq`, `theme` or `default` from state alone.
  - `Desktop` remembers `last_rejected: Option<(String /* domain */, String /* message */)>`. It's
    set when `commit` gets `ConfigInvalid` for a section write and cleared when that domain next
    commits. That domain's keys report `error`.
  - D-Bus: `Explain(s domain) → s`, and `Reapply(s domain)`, which runs
    `commit(self.state.clone(), change_for(domain), Origin::Settings)`. The diff is empty, so it
    writes no journal entry. Reapply is refused with `InvalidArgument` for keys whose source is
    `user_lua`; the app hides the button for those anyway.

- [ ] **Step 1: Write the failing tests:**
  - In `provenance.rs`:
    - `finds_dotted_path`;
    - `finds_colon_path`;
    - `finds_leaf_inside_section_table`;
    - `ignores_leaf_in_other_table`;
    - `commented_match_still_counts` (documents the heuristic);
    - `huge_and_non_utf8_input_is_safe`;
    - `every_option_key_exists_in_state_json`.
  - In `service.rs`:
    - `explain_reports_user_lua_override_with_line` (fake `get_option("general:gaps_in")` = 2,
      stored 5, `user.lua` sets `gaps_in = 2` on line 3);
    - `explain_reports_runtime_when_user_lua_silent`;
    - `explain_null_key_reports_theme_or_default`;
    - `explain_after_rejected_write_reports_error_then_clears`;
    - `explain_without_compositor_uses_state`;
    - `reapply_writes_no_history`.
- [ ] **Step 2:** Run `cargo test --workspace`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(uqd): Explain where each setting's live value comes from`

### Task 6: Source labels on setting rows

**Files:**
- Modify: `apps/uq-settings/qml/components/SettingRow.qml`, `apps/uq-settings/src/pages/runner.rs`
  (`snapshot` helpers), `apps/uq-settings/src/pages/look.rs`, `apps/uq-settings/src/pages/appearance.rs`,
  `apps/uq-settings/src/pages/fake_uqd.rs` (`explain`, `reapply`), `apps/uq-settings/src/model_core.rs`
  (a `reapply(domain)` invokable on each model), `apps/uq-settings/qml/pages/LookPage.qml`,
  `apps/uq-settings/qml/pages/AppearancePage.qml` and `apps/uq-settings/qml/Main.qml`
  (re-refresh on window activation).
- Create: `tests/settings-qml/tst_setting_row_source.qml`.

**Interfaces:**
- Consumes: Task 5's `Explain`/`Reapply` JSON.
- Produces:
  - Each page's snapshot JSON gains `"sources": { "<setting_id>": {source, detail} }`, keyed by
    `OptionRow.setting_id`, for the domains the page shows. A failed `Explain` gives `{}`, so the
    row shows nothing.
  - `SettingRow` gains:
    - `property var source: null`, the `{source, detail}` object;
    - `signal openUserLua(int line)` and `signal reapply()`.
  - `SettingRow` renders, under its label:
    - `theme` → "From the theme" (muted);
    - `default` → "Default" (muted);
    - `uq` or `null` → nothing;
    - `user_lua` → a warning icon, "Overridden in user.lua, line N", and an **Open user.lua**
      button;
    - `runtime` → "Changed while running (not by uQ)" and a **Re-apply** button;
    - `error` → the message, in the negative colour.

    All of these go through `UqLabel`.
  - **Open user.lua** calls `Qt.openUrlExternally("file://" + path)`. The label keeps the line
    number.
  - Pages bind `source: model.state.sources ? model.state.sources["gaps-inner"] ?? null : null` on
    each row.
  - Every `Override*` component passes `source` straight through, since they're `SettingRow`s.

- [ ] **Step 1: Write the failing tests:**
  - In `tst_setting_row_source.qml`: each source shows the right text; `user_lua` shows the
    button and emits `openUserLua(3)`; `runtime` emits `reapply`; `null` shows nothing and the
    row height is unchanged from today.
  - In the Rust page tests (fake uqd): `look_snapshot_includes_sources` and
    `explain_failure_gives_empty_sources`.
- [ ] **Step 2:** Run `cargo test -p uq-settings` and
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run both, plus `scripts/lint-settings.sh`. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(settings): rows show where their value comes from`

### Task 7: Undo/redo, "Changed this session" and the History page

**Files:**
- Create: `apps/uq-settings/src/pages/history.rs`, `apps/uq-settings/src/history_model.rs`,
  `apps/uq-settings/qml/pages/HistoryPage.qml`, `apps/uq-settings/qml/components/SessionChanges.qml`,
  `tests/settings-qml/tst_session_changes.qml`.
- Modify: `apps/uq-settings/src/registry.rs` (page `history`, category `Uq`, icon
  `document-open-recent`, search entries `history` "Settings history" with `["undo", "revert",
  "changes", "log"]`, and `restore-points` "Restore points" with `["backup", "snapshot",
  "restore", "go back"]`), `apps/uq-settings/src/pages/mod.rs`, `apps/uq-settings/qml/Main.qml`
  (header button, `Shortcut`s), `apps/uq-settings/build.rs` / `Cargo.toml` if models are listed,
  and `apps/uq-settings/src/pages/fake_uqd.rs`.

**Interfaces:**
- Consumes: Task 4's `ListHistory`, `Undo`, the restore methods and `HistoryChanged`.
- Produces:
  - `HistoryState` (a `PageLogic`) with ops:
    - `Refresh`;
    - `Undo { id, force }`;
    - `SaveRestorePoint { name }`, `DeleteRestorePoint { id }` and `Restore { id }`.

    Its snapshot is `{entries: [...], points: [...], session_start: i64}`, where `session_start`
    is the app's start time.
  - The app-level undo stack lives in `HistoryModel`:
    - `own: Vec<String>` holds the ids of entries whose origin is `settings` and whose time is
      after `session_start`, newest last. It's rebuilt on every refresh from entries with origin
      `settings`.
    - `redo: Vec<String>` holds the restore-entry ids returned by `Undo`.
    - `undo_last()` calls `Undo(own.last, false)` and pushes the returned id onto `redo`.
    - `redo_last()` calls `Undo(redo.pop, false)`.
    - Any new `settings`-origin entry that isn't from undo or redo clears `redo`.
  - A `Conflict` error surfaces as a confirm dialog (`Kirigami.PromptDialog`) with uqd's message.
    "Undo anyway" re-calls with `force = true`.
  - `Shortcut { sequences: [StandardKey.Undo] }` → `undo_last()`, and
    `Shortcut { sequences: [StandardKey.Redo, "Ctrl+Shift+Z"] }` → `redo_last()`. They're disabled
    while a text field has focus, so text editing keeps its own undo.
  - `SessionChanges.qml` is a header button labelled "Changed this session (N)" that opens a
    `Kirigami.OverlaySheet`. It lists every entry after `session_start`, from any origin, each with
    its summary, origin in words ("from the terminal", "from the panel", "here") and an Undo
    button.
  - `HistoryPage.qml` shows:
    - a timeline grouped by local day, filterable by page (domain → page name through the
      registry);
    - the restore points, each with Restore (behind a confirm) and Delete (manual points only);
    - **Save a restore point** (a name field, 100 chars max).
  - Toasts (`Kirigami.InlineMessage`) confirm each action.

- [ ] **Step 1: Write the failing tests:**
  - In `pages/history.rs`, against the fake uqd:
    - `undo_last_undoes_only_own_entries` (Review Focus 5);
    - `redo_reapplies_after_undo`;
    - `new_change_clears_redo`;
    - `conflict_surfaces_as_error_then_force_applies`;
    - `restore_requires_existing_id`.
  - In `tst_session_changes.qml`: the count label, origin words, and an Undo click emitting the id.
  - In the registry: `history_page_registered_with_search_entries`.
- [ ] **Step 2:** Run `cargo test -p uq-settings` and the settings QML tests. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run both. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(settings): undo, redo, session changes and the History page`

### Task 8: Quality rules

**Files:**
- Create: `scripts/settings-jargon.txt`.
- Modify: `scripts/lint-settings.sh`, `tests/shell/lint_settings.sh` (fixtures in
  `tests/settings-qml/fixtures/`), `apps/uq-settings/qml/Main.qml` (window state),
  `apps/uq-settings/src/registry.rs` (tests), and the page model tests.

**Interfaces:**
- Produces:
  - **Jargon lint.** `lint-settings.sh` fails on any word from `settings-jargon.txt`
    (case-insensitive, whole word) inside a quoted string in `apps/uq-settings/qml`. The words are
    `uqd`, `generated.lua`, `qt6ct`, `dbus`, `d-bus`, `org.uq`, `hyprctl`, `state.toml` and
    `gsettings`. Allowed exceptions:
    - a line ending with the comment `// jargon-ok: <reason>`;
    - files under `components/AdvancedDetails.qml` (none exist yet);
    - `"Kvantum"` and `"Qt"` stay allowed. The Appearance row is user-facing and named so in the
      spec.

    It prints `file:line: word`.
  - **Window state.** `Main.qml` uses
    `QtCore.Settings { location: StandardPaths.writableLocation(StandardPaths.GenericStateLocation) + "/uq/settings-app.conf"; property alias width; property alias height; property alias maximized; property string lastPage }`.
    The window restores these on start. `ShowPage` with a page id still wins over `lastPage`.
    `GenericStateLocation` needs Qt 6.7 or newer; with older Qt, the file is
    `$XDG_STATE_HOME/uq/settings-app.conf` built from the env in Rust and passed in as a context
    property.
  - **Two clicks / one search.** In `registry.rs`, `every_setting_has_keywords` fails if a core
    page's `SearchEntry` has an empty `keywords`, and `no_duplicate_setting_ids` fails on a
    repeated id.
  - **No self-resetting settings.** In each page model test module (`look`, `appearance`,
    `wallpaper`, `shell`), `every_field_round_trips` writes a non-default value to each field via
    its `Op`. It then builds a fresh state object against the same fake uqd and asserts the value
    reads back.

- [ ] **Step 1: Write the failing tests:**
  - In `tests/shell/lint_settings.sh`, a fixture page containing `"Restart uqd"` fails the lint,
    and the same line with `// jargon-ok: test` passes.
  - `every_setting_has_keywords`, `no_duplicate_setting_ids` and the four `every_field_round_trips`.
- [ ] **Step 2:** Run `tests/shell/lint_settings.sh` and `cargo test -p uq-settings`. Expected:
  FAIL where gaps exist.
- [ ] **Step 3:** Implement. Fix any existing string the lint catches by rewording it in plain
  language.
- [ ] **Step 4:** Re-run, plus `scripts/lint-settings.sh` over the real tree. Expected: PASS.
- [ ] **Step 5:** Commit: `feat(settings): quality rules (plain words, keywords, round trips, window state)`

### Task 9: Docs, changelog and the whole gate

**Files:**
- Modify: `README.md` (uqctl history/undo/restore; the History page), `CHANGELOG.md` (Unreleased),
  `docs/notes/` (create `settings-slice-a-live-check.md`).

- [ ] **Step 1:** Write `docs/notes/settings-slice-a-live-check.md` with these checks:
  1. Put `general.gaps_in = 2` (or the user's config style) in `user.lua`. Open Look. The gaps row
     says "Overridden in user.lua, line N". Remove the line and run `hyprctl reload`. Switch focus
     back to the app, and the warning is gone.
  2. `hyprctl keyword general:gaps_out 30` → "Changed while running", then Re-apply restores the
     value.
  3. Change three settings, then press Ctrl+Z three times. Each one reverts live. Ctrl+Shift+Z
     redoes them.
  4. Run `uqctl` to change the theme. "Changed this session" lists it "from the terminal", and
     Ctrl+Z in the app doesn't undo it.
  5. Save a restore point, change several pages, then Restore. Everything comes back. Undo the
     restore and the changes return.
  6. Restart uqd after a version bump. A "new version" restore point appears.
- [ ] **Step 2:** Run the full gate, exactly as CI does: `cargo fmt --check`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`,
  `scripts/lint-shell.sh`, `scripts/lint-settings.sh`, `tests/shell/*.sh`, and both
  `qmltestrunner` suites. Expected: all PASS.
- [ ] **Step 3:** Commit: `docs: settings slice A — history, sources and live checks`
