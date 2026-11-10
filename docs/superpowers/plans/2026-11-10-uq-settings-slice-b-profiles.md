# uQ Settings enhancements slice B: profiles, the quick-settings tile and `uqctl profile` — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named profiles ("Gaming", "Presentation") hold a few settings and switch them on in one
step and off again ("Back to normal"). You switch them from a new Profiles page in `uq-settings`,
from a tile in quick settings, or with `uqctl profile`. Keys you change by hand while a profile is
on keep their value and are named. Profiles export to a TOML file and import back.

**Architecture:**
- **uq-core** gains `profile`: the file format (`format = 1`) and its validation, the profile
  store, and the overlay logic. The overlay logic applies a profile to a `State`, remembers what
  it replaced, and turns it off again (K3). It also builds a profile from a history range. It's
  plain Rust with unit tests.
- **uqd** gains `profiles.rs`:
  - list, get, set and delete;
  - `apply_profile` and `profile_off`, each one validated write through `commit` with origin
    `profile:<id>`, after an `Auto(BeforeProfile)` restore point;
  - an active-profile file that survives a restart;
  - power modes through power-profiles-daemon in `power.rs`. They're skipped with a notice when
    it's missing, and never fail the apply.

  `Explain` reports the `profile` source. D-Bus gains six methods and `ProfileChanged(s id)`.
- **uqctl** gains `uqctl profile list|apply|off|next|export|import|delete`. The bridge forwards
  three profile calls and the signal, and quick settings gains a profile row.
- **uq-settings** gets a Profiles page (Qt-free page logic tested against the fake uqd), the
  "From the … profile" row label and profile names in history origins.

**Tech Stack:** Rust edition 2024, zbus 5, toml, cxx-qt 0.10, Qt 6, Kirigami 6, Quickshell,
Hyprland 0.56.2–0.57.x (Lua config), power-profiles-daemon over the system bus.

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. This plan covers
§4 "Profiles" (named profiles only; monitor profiles are slice F), the profile parts of §2 (the
`profile` source), §3 (origin `profile:<id>`, the before-profile restore point) and §10, and §12
slice B. It builds on slice A (`docs/superpowers/plans/2026-11-10-uq-settings-slice-a-trust.md`,
merged).

## Global Constraints

These are copied from the spec and the user's rulings (K1–K6), except the lines marked **ruling**,
which this plan decides.

- **Model (§4):** "A profile is a named overlay: a subset of keys from uqd's domains, not a full
  copy. Only the keys it holds change when it applies." "At most one profile is active."
- **K1, what a profile holds:** in v1, `look`, `layouts`, `appearance` (with the theme, variant and
  fonts), `shell`, `wallpaper` and `power.profile`. Keybinds and rules don't exist yet (rules is
  slice C, keybinds Phase 3 slice 4).
  - **Ruling:** `keybinds` and `rules` are reserved domains in the format. Their shape is a table
    of `<id> = true|false`, which can't add a command. v1 refuses a file that has either, with a
    plain message ("…which this version of uQ can't do yet"). A later slice turns them on by
    changing `DomainKind::Reserved` to a real kind. The format doesn't change.
- **File format (§4):** `~/.local/state/uq/profiles/<id>.toml`. "Export and import use the same
  TOML with a header comment and `format = 1`. Import validates every key, refuses unknown
  domains, and never imports `keybinds` that add commands." The top-level keys are `format`,
  `id`, `name` and `icon`, plus the domain tables.
  - **Ruling:** the spec's example `islands.clock` is `islands.time` in code
    (`ShellSettings::islands`).
  - **Ruling, limits:**
    - id `^[a-z0-9][a-z0-9-]{0,31}$`;
    - name 1–100 chars with no control characters;
    - icon (optional) `^[A-Za-z0-9._-]{1,64}$`;
    - file ≤ 64 KiB, ≤ 500 keys, ≤ 50 profiles;
    - `power.profile` ∈ `power-saver | balanced | performance`;
    - the theme name must pass `settings::valid_theme_name`, and uqd also checks that the theme
      is installed.
  - **Ruling:** a profile holds set values only. TOML has no null, so "use the theme's value"
    can't be in a profile.
- **Applying (§4):** "uqd saves an automatic restore point, remembers the values of the keys the
  profile touches, then applies the overlay as one validated write with origin `profile:<id>`."
  - **Ruling:** switching from profile A to B is also one write. A is turned off in memory, B is
    applied over the result, and the whole write is committed once with origin `profile:<B>`. If
    the verify step fails, nothing changes and A stays on.
  - **Ruling:** "Back to normal" is journalled with origin `profile:<id>` too. History then says
    "from the Gaming profile" for both.
- **K3, hand-changed keys:** "Back to normal" writes the remembered value back only for keys whose
  current value still equals what the profile set. Keys changed by hand keep their value and are
  reported by plain label.
  - **Ruling:** a key whose current value already equals the remembered one (an undo did it) is
    neither written nor reported.
- **K2, power:** `power.profile` goes through power-profiles-daemon on the system bus. It tries
  `org.freedesktop.UPower.PowerProfiles` (`/org/freedesktop/UPower/PowerProfiles`, the current
  name since power-profiles-daemon 0.20) first, then `net.hadess.PowerProfiles`
  (`/net/hadess/PowerProfiles`), using the `ActiveProfile` property. If it's missing, refuses,
  or doesn't answer within 3 s, the key is skipped with a notice. That never fails the apply.
  Power is set after the settings write succeeds, and K3 applies to it on "Back to normal".
- **Active state survives a uqd restart:**
  - **Ruling:** it's kept in `~/.local/state/uq/profile-active.json` (`Paths::profile_active`),
    not in `state.toml`, so restore points and the journal never carry it. The file is written
    *before* the commit, and put back as it was if the commit fails.
- **Restore point (§3):** "Automatic points: … before a profile switch". This is
  `AutoReason::BeforeProfile`, named "Before the <name> profile", saved on every apply (not on
  "Back to normal").
  - **Ruling:** importing a profile changes no setting, so it saves no restore point.
- **Ruling, the rest of the lifecycle:**
  - `Restore` ends the active profile: its memory is dropped and `ProfileChanged("")` is sent.
  - Deleting the active profile is refused ("Turn this profile off before deleting it.").
  - Editing the active profile takes effect the next time it's turned on.
- **Origin (§3):** `profile:<id>` (`Origin::Profile(id)`, which already exists).
- **D-Bus (§10):** `ListProfiles`, `GetProfile`, `SetProfile`, `DeleteProfile`, `ApplyProfile`,
  `ProfileOff`, and the signal `ProfileChanged(s id)`.
  - **Ruling:** the signatures are:
    - `ListProfiles() → s` (JSON);
    - `GetProfile(s id) → s` (the TOML file text);
    - `SetProfile(s toml, b replace) → s id`. It gives `Conflict` when the id exists and
      `replace` is false.
    - `DeleteProfile(s id)`;
    - `ApplyProfile(s id) → s report`;
    - `ProfileOff() → s report`.

    `ProfileChanged(id)` is sent when the active profile or the list of profiles changes; `id` is
    the active profile, or `""`.
- **Switching (§4):** "From the Profiles page, a quick-settings tile (a menu of profiles),
  `uqctl profile list|apply <id>|off|next`, and an optional keybind (`uq.profile.next`)."
  - **Ruling, `next`:** profiles go in list order (by name, case-insensitive, then id). `next`
    goes from normal to the first profile, through each one, and after the last back to normal.
  - **Ruling, `uq.profile.next` is ruled out of this slice.** Binds are fixed lines in
    `config/hypr/base.lua` today. There is no bind table, no bind ids and no way to ship a bind
    that's off by default (Phase 3 §2.4, slice 4). Adding a combo now would take a key from every
    user. Instead:
    - this slice ships `uqctl profile next`;
    - the README shows the `user.lua` line
      `hl.bind("SUPER + ALT + P", hl.dsp.exec_cmd("uqctl profile next"))`;
    - the keybinds slice adds `uq.profile.next` as an unbound default that runs it.
- **Source label (§2):** `profile`, "Set by the active profile", shown as "From the Gaming profile".
  It's reported only when the key is `uq` (stored and live match) and the active profile set it
  to the value it still has.
- **K4:** there's no Qt in the build container. `uq-settings`, the `qmltestrunner` suites and
  `tests/shell/*.sh` run in CI. All page logic stays in Qt-free `pages/*.rs`, tested with the fake
  uqd. Locally, run `cargo test -p uq-core -p uqd -p uqctl` (plus `cargo build -p uq-settings`
  and its tests if Qt is there).
- **K5, commits:** every commit message ends with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc, so the trailers sit on their own lines after a blank line.
- **K6, plain words (§9):** every user-facing string must pass `scripts/lint-settings.sh` (QML)
  and the plain-words rule. That covers uqd's error and notice texts, `uqctl` output, the
  page's, and the tile's. Name things "power mode", "profile", "settings" and "Back to normal".
  Never say "power-profiles-daemon", "D-Bus", "uqd" or "overlay". User text (profile names, keys
  from files) is shown through `UqLabel` in the app and `UqText` in the shell.

## Review Focus

1. **uqd stops, or the verify step fails, halfway through an apply.** "Back to normal" must still
   work and must never write a wrong value. A failed apply leaves the settings, the active file
   and the active profile exactly as they were. Task 4 tests
   `failed_verify_changes_nothing_and_keeps_previous_profile` and
   `active_file_written_but_state_not_still_turns_off_cleanly`.
2. **Switching straight from one profile to another after changing a key by hand.** The first
   profile's untouched keys come back, the hand-changed key is kept and named, and the second
   profile remembers the hand value as its "before". Task 2 tests
   `switch_turns_first_off_then_applies_second`, and Task 4 tests
   `switching_profiles_returns_untouched_keys_then_applies_next`.
3. **The power service is missing, refuses (no performance mode on this laptop) or hangs.** The
   settings still apply, the notice says so in plain words, and the call returns within about 3 s.
   Task 4 tests `power_missing_is_a_notice_not_a_failure`, `power_refused_is_a_notice` and
   `power_timeout_is_a_notice`.
4. **A hostile or broken import.** This covers a 1 MiB file, invalid UTF-8, deep nesting, an id
   like `../x`, `[keybinds]` or `[monitors]`, a value out of range, an unknown key, and
   `format = 2`. Each is refused with a plain message and nothing is stored. Task 1 tests
   `refuses_hostile_files`, Task 5 `read_import_refuses_big_and_binary_files` and Task 7
   `import_refuses_hostile_file`.
5. **A profile whose theme was uninstalled since it was saved.** Applying it is refused as a
   whole, with the theme named, and nothing changes: no restore point, no active file, no power
   change. Task 4 tests `apply_with_uninstalled_theme_changes_nothing`.

---

### Task 1: `uq_core::profile`: the format, validation and the store

**Files:**
- Create: `crates/uq-core/src/profile/mod.rs`, `crates/uq-core/src/profile/store.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs` (`pub mod profile;`);
  - `crates/uq-core/src/paths.rs` (`profiles_dir` = `<state>/uq/profiles` and `profile_active` =
    `<state>/uq/profile-active.json`, and their lines in `under_root_puts_everything_inside`);
  - `crates/uq-core/src/history/diff.rs` (make `get_path` and `put_path` `pub(crate)`);
  - `crates/uq-core/src/history/labels.rs` (one test).

**Interfaces:**
- Consumes: `State::section` / `State::set_section`, `settings::valid_theme_name`,
  `theme::Variant`, `history::label`, and `atomic::atomic_write`.
- Produces (`uq_core::profile`):
  - Constants:
    - `FORMAT: i64 = 1`;
    - `MAX_FILE_BYTES: u64 = 65_536`, `MAX_KEYS: usize = 500`, `MAX_PROFILES: usize = 50` and
      `MAX_NAME_CHARS: usize = 100`;
    - `POWER_MODES: [&str; 3] = ["power-saver", "balanced", "performance"]`;
    - `HEADER: &str = "# A uQ settings profile. Import it on the Profiles page of uQ Settings, or with\n# `uqctl profile import <file>`.\n"`.
  - `pub enum DomainKind { Section, Appearance, Power, Reserved }`, and
    `pub const PROFILE_DOMAINS: [(&str, DomainKind); 8]`. In this order: `look`, `layouts`,
    `shell` and `wallpaper` are `Section`; then `appearance` is `Appearance`; `power` is
    `Power`; `keybinds` and `rules` are `Reserved`. `Appearance` is the `appearance` section
    plus the leaves `theme` and `variant`, which map to `State::theme` and `State::variant`.
  - `pub struct Profile { pub id: String, pub name: String, pub icon: Option<String>, pub values: BTreeMap<String, serde_json::Value> }`.
    `values` maps a domain to a JSON object tree that is never empty.
  - `pub struct Key { pub domain: String, pub path: Vec<String>, pub value: Value }`.
  - `impl Profile`:
    - `parse(text: &str) -> Result<Profile, ProfileError>`;
    - `to_toml(&self) -> String`, which writes `HEADER`, `format = 1`, `id`, `name`, `icon`, then
      the domains in `PROFILE_DOMAINS` order;
    - `validate(&self) -> Result<(), ProfileError>`;
    - `keys(&self) -> Vec<Key>`;
    - `with_key(&mut self, key: Key)`;
    - `without_key(&mut self, domain: &str, path: &[String]) -> bool`;
    - `power(&self) -> Option<&str>`.
  - Free functions:
    - `pub fn leaves(v: &Value) -> Vec<(Vec<String>, Value)>` descends objects; arrays and
      scalars are leaves, and an empty object is none.
    - `pub fn read_leaf(state: &State, domain: &str, path: &[String]) -> Option<Value>`;
    - `pub fn write_leaves(state: &mut State, domain: &str, leaves: &[(Vec<String>, Option<Value>)]) -> Result<(), SettingError>`
      writes every leaf of one domain through a single `State::set_section` (`None` removes the
      leaf). For `Appearance`, `theme` and `variant` set the `State` fields. A theme that fails
      `valid_theme_name`, or a variant other than `dark`/`light`, is
      `SettingError::Invalid`.
    - `pub fn valid_id(id: &str) -> bool`;
    - `pub fn id_for_name(name: &str, taken: &[String]) -> String`: lowercase ASCII
      alphanumerics, every other run becomes `-`, trimmed, cut to 32 chars; empty becomes
      `profile`; a taken id gets `-2`, `-3`, ….
    - `pub fn key_label(domain: &str, path: &[String]) -> String`:
      - `("appearance", ["theme"|"variant"])` → `history::label("theme", …)`;
      - `("power", ["profile"])` → `"Power mode"`;
      - otherwise `history::label(domain, path.join("."))`, falling back to the dotted path.
    - `pub fn power_label(mode: &str) -> &str`: "Power saver", "Balanced" or "Performance".
    - `pub fn value_text(domain: &str, path: &[String], v: &Value) -> String`. It gives strings
      as they are, booleans as "On"/"Off", numbers as written, arrays as "N items" and the power
      mode through `power_label`.
    - `pub fn power_notice(reason: &str) -> String` = `format!("Power mode wasn't changed: {reason}.")`.
    - `pub fn read_file(path: &Path) -> Result<String, ProfileError>`. It checks the length
      first and reads at most `MAX_FILE_BYTES + 1` bytes, so a bigger file is `TooBig`. Bytes
      that aren't UTF-8 are `NotAProfile`.
  - `pub enum ProfileError` (`thiserror`, `PartialEq`), each `Display` in plain words:
    - `TooBig` → "The file is too big to be a profile."
    - `Unreadable(String)` → "Can't read the file: {0}"
    - `NotAProfile` → "This file isn't a uQ profile."
    - `NewerFormat(i64)` → "This profile was made by a newer version of uQ."
    - `BadId` → "The profile's id can only use a–z, 0–9 and dashes."
    - `BadName` → "A profile needs a name of 1 to 100 characters."
    - `BadIcon` → "The profile's icon name isn't valid."
    - `UnknownDomain(String)` → "This profile has settings uQ doesn't know: {0}."
    - `NeedsNewer(String)` → for `keybinds`, "This profile turns keyboard shortcuts on or off,
      which this version of uQ can't do yet."; for `rules`, the same with "window rules".
    - `Invalid { key: String, reason: String }` → "{label}: {reason}", using `key_label`.
    - `Empty` → "A profile needs at least one setting."
    - `TooManyKeys` → "A profile can hold up to 500 settings."
  - **Validation.** `parse` and `validate` check, in order:
    1. the size;
    2. that the text is TOML;
    3. `format` (missing → `NotAProfile`, above 1 → `NewerFormat`);
    4. the id, name and icon;
    5. the top-level keys (not a known domain → `UnknownDomain`; `Reserved` → `NeedsNewer`);
    6. `Power`, which must be exactly `{profile = <POWER_MODES>}`;
    7. that there's at least one key and at most `MAX_KEYS`;
    8. then every `Section`/`Appearance` domain through `write_leaves` onto `State::default()`.
       A `SettingError` becomes `Invalid`. Any leaf that `read_leaf` can't read back afterwards
       is `Invalid { reason: "not a setting uQ knows" }`.
- `pub struct ProfileStore { dir: PathBuf }` (`store.rs`) has:
  - `open(dir)`;
  - `list(&self) -> Vec<Profile>`, sorted by name (case-insensitive), then id. It skips, with a
    `tracing::warn!`, files that don't parse or whose id isn't their file name.
  - `load(&self, id) -> io::Result<Profile>`. A bad id is `InvalidInput`, a missing file
    `NotFound`, and a file that doesn't parse `InvalidData`.
  - `save(&self, p: &Profile) -> io::Result<()>`, which writes `<id>.toml` atomically from
    `to_toml`;
  - `delete(&self, id) -> io::Result<()>`.

- [ ] **Step 1: Write the failing tests:**
  - In `profile/mod.rs`:
    - `parse_spec_example` is the spec's `gaming.toml`, with `format = 1` added and
      `islands.time`. It has 6 keys and `power()` is `Some("performance")`.
    - `to_toml_round_trips`: `parse(&p.to_toml()) == Ok(p)`. The text starts with `HEADER` and
      has `format = 1`, with a wallpaper key under output `"HDMI-A.1"`.
    - `refuses_missing_or_newer_format`;
    - `refuses_unknown_domain` (`[monitors]`, `[input]`, `[settings]`);
    - `refuses_reserved_domains_for_now` (`[keybinds] "uq.spotlight" = false` →
      `NeedsNewer("keybinds")`, and the same for `[rules]`);
    - `refuses_invalid_values`:
      - `look.gaps_in = 999` → `Invalid { key: "look.gaps_in", .. }`;
      - `look.bogus = 1` → `Invalid`;
      - `power.profile = "turbo"` → `Invalid { key: "power.profile", .. }`;
      - `appearance.variant = "dim"` → `Invalid`.
    - `refuses_hostile_files`:
      - 65 KiB of comments → `TooBig`;
      - `id = "../x"` → `BadId`;
      - a 101-character name or a name with `\u{7}` → `BadName`;
      - 5 000 nested tables → an `Err`, not a panic.
    - `empty_profile_is_refused`;
    - `id_for_name_slugs_and_dedups` (`"Gaming"` → `gaming`; `"Gaming"` with `["gaming"]`
      taken → `gaming-2`; `"!!!"` → `profile`);
    - `key_label_names_every_kind` (`Theme`, `Light or dark`, `Power mode`,
      `Gaps between windows`, `Bar island: Clock`);
    - `read_file_refuses_big_and_binary_files`.
  - In `profile/store.rs`: `store_round_trip_sorted_by_name`, `store_skips_bad_and_mismatched_files`
    and `store_refuses_bad_ids` (`""`, `"../x"`, `"A"`).
  - In `history/labels.rs` tests: `every_profile_key_has_a_plain_label`. For every
    `State` in `populated()`, each leaf of each `Section`/`Appearance` profile domain, plus
    `appearance.theme`, `appearance.variant` and `power.profile`, has a `key_label` that isn't
    its dotted path.
- [ ] **Step 2:** Run `cargo test -p uq-core profile` and `cargo test -p uq-core labels`.
  Expected: FAIL (module missing).
- [ ] **Step 3:** Implement. Convert with `toml::Value` ↔ `serde_json::Value`. TOML integers stay
  integers, so `gaps_in = 0` reads as `0`, not `0.0`.
- [ ] **Step 4:** Run `cargo test -p uq-core`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): profile file format, validation and store`

### Task 2: `uq_core::profile`: apply, "Back to normal", active state and history ranges

**Files:**
- Create: `crates/uq-core/src/profile/overlay.rs`, `crates/uq-core/src/profile/active.rs`.
- Modify: `crates/uq-core/src/profile/mod.rs` (`mod overlay; mod active;` and the re-exports).

**Interfaces:**
- Consumes: Task 1's `Profile`, `read_leaf`, `write_leaves`, `leaves` and `key_label`;
  `history::Entry`.
- Produces:
  - Serde structs:
    - `pub struct KeyMemo { pub domain: String, pub path: Vec<String>, pub before: Option<Value>, pub set: Value }`;
    - `pub struct PowerMemo { pub before: String, pub set: String }`;
    - `pub struct ActiveProfile { pub id: String, pub name: String, pub since: i64, pub keys: Vec<KeyMemo>, pub power: Option<PowerMemo> }`;
    - `pub struct Kept { pub domain: String, pub path: Vec<String>, pub label: String }`.
  - `pub fn apply(current: &State, profile: &Profile, now: i64) -> Result<(State, ActiveProfile), ProfileError>`.
    It writes every non-power key through `write_leaves`, one call per domain. `before` is
    `read_leaf(current, …)`, and `set` is `read_leaf(next, …)` after the write, so it holds the
    normalised value. `power` is left `None`; uqd fills it in.
  - `pub fn turn_off(current: &State, active: &ActiveProfile) -> Result<(State, Vec<Kept>), ProfileError>`.
    For each memo, with `cur = read_leaf(current)`:
    - `cur == Some(set)` → write `before` back (`None` removes the key);
    - `cur == before` → nothing;
    - otherwise → push a `Kept` with its `key_label`.

    It doesn't look at power.
  - `pub fn holds(active: &ActiveProfile, domain: &str, path: &[String], current: &Value) -> bool`
    is true when a memo has this `domain`/`path` and `set == *current`.
  - `pub fn from_journal(domain: &str, path: &[String]) -> Option<(String, Vec<String>)>`:
    - `theme` + `[theme|variant]` → `("appearance", path)`;
    - a `Section` or `Appearance` profile domain → itself;
    - anything else (`schedule`, `settings`, `monitors`, `input`) → `None`.
  - `pub fn changed_since(newest_first: &[Entry], since: i64) -> (Vec<(String, Vec<String>)>, usize)`.
    For entries with `time >= since`, it collects the paths whose leaf differs between `before`
    and `after` (via `leaves`), maps each through `from_journal`, and deduplicates in first-seen
    order. The count is the number of changed paths that can't go in a profile.
  - `pub fn next_after(ids_in_order: &[String], active: Option<&str>) -> Option<String>`:
    - with no active profile (or one that isn't in the list) → the first id;
    - the last id → `None`;
    - an empty list → `None`.
  - In `active.rs`:
    - `pub fn load_active(path: &Path) -> Option<ActiveProfile>` (missing → `None`; unreadable
      or damaged → `tracing::warn!` and `None`);
    - `pub fn save_active(path: &Path, a: &ActiveProfile) -> io::Result<()>` (atomic);
    - `pub fn clear_active(path: &Path) -> io::Result<()>` (a missing file is fine).

- [ ] **Step 1: Write the failing tests** in `overlay.rs` and `active.rs`:
  - `apply_sets_only_profile_keys_and_remembers_before`: `gaps_in` 5 with the profile setting
    0, and `blur` unset with the profile setting `false`. The memo has `before` `Some(5)` and
    `None`, and every other key of `look` and `shell` is unchanged.
  - `apply_records_normalised_value` (`animation_speed = 1.234` → `set` 1.23);
  - `apply_theme_and_variant_through_appearance`;
  - `turn_off_restores_untouched_keys`;
  - `turn_off_keeps_hand_changed_keys_and_reports_them` (`gaps_in` set to 3 by hand → kept,
    label "Gaps between windows", and `blur` is unset again);
  - `turn_off_skips_keys_already_back`;
  - `switch_turns_first_off_then_applies_second`: A sets gaps 0 and blur off, the user sets
    gaps 3, and B sets blur on. `turn_off` then `apply` gives gaps 3, blur on, `Kept` = [gaps],
    and B's memo has blur `before` `None`.
  - `holds_only_while_value_unchanged`;
  - `changed_since_maps_theme_and_skips_other_domains`: a `look` change before `since` is left
    out; after it, a `look.blur` change, a `theme` variant change and a `settings` idle change
    give `[("look", ["blur"]), ("appearance", ["variant"])]` and a skip count of 1.
  - `next_after_cycles_through_then_back_to_normal`;
  - `active_round_trip` and `damaged_active_file_reads_as_none`.
- [ ] **Step 2:** Run `cargo test -p uq-core profile`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p uq-core`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): apply and turn off profiles, remembering what they replaced`

### Task 3: uqd: power modes through power-profiles-daemon

**Files:**
- Create: `crates/uqd/src/power.rs`.
- Modify: `crates/uqd/src/lib.rs` (`pub mod power;`), `crates/uqd/src/service.rs` (fields and
  builders), and `crates/uqd/src/main.rs` (`.with_power(Arc::new(SystemPowerProfiles::default()))`).

**Interfaces:**
- Produces:
  - `#[async_trait] pub trait PowerProfiles: Send + Sync` with
    `async fn active(&self) -> Result<String, PowerError>` and
    `async fn set_active(&self, mode: &str) -> Result<(), PowerError>`.
  - `#[derive(Debug, thiserror::Error, PartialEq)] pub enum PowerError`, with the texts that
    `profile::power_notice` takes:
    - `Missing` → "power modes aren't available on this computer";
    - `Refused(String)` → "the computer didn't accept it". The inner text is only logged.
    - `Timeout` → "the power settings didn't answer".
  - `pub const SERVICES: [(&str, &str, &str); 2]` holds the `(name, path, interface)` of the two
    services, in K2's order.
  - `pub struct NoPowerProfiles;` always gives `Missing`. It's the default in `Desktop::new`, so
    tests never touch the real system bus.
  - `pub struct SystemPowerProfiles { address: Option<String> }`:
    - `Default` uses `zbus::Connection::system()`, and `at(address)` is for tests.
    - It builds a `zbus::proxy::Builder` with `cache_properties(CacheProperties::No)` for each
      entry of `SERVICES` in turn, and reads or writes `ActiveProfile`.
    - `ServiceUnknown` or `NameHasNoOwner` on both names is `Missing`. Any other error is
      `Refused`.
  - `#[cfg(any(test, feature = "fake"))] pub mod fake` has `pub struct FakePower` with:
    - `pub mode: Mutex<Option<String>>` (`None` means missing);
    - `pub refuse: AtomicBool`;
    - `pub delay: Mutex<Duration>`;
    - `pub sets: Mutex<Vec<String>>`.
  - `Desktop` gains:
    - the fields `pub(crate) power: Arc<dyn PowerProfiles>` and `pub(crate) power_timeout: Duration`
      (`POWER_TIMEOUT = 3 s`);
    - `pub fn with_power(self, p: Arc<dyn PowerProfiles>) -> Self` and
      `pub fn with_power_timeout(self, d: Duration) -> Self`;
    - `pub(crate) async fn power_active(&self) -> Result<String, PowerError>` and
      `pub(crate) async fn power_set(&self, mode: &str) -> Result<(), PowerError>`. Both wrap the
      call in `tokio::time::timeout(self.power_timeout)`, which gives `Timeout`.

- [ ] **Step 1: Write the failing tests** in `power.rs`. Each runs a tiny zbus interface with a
  read-write `ActiveProfile` property on `uq_core::test_bus::private_bus()`:
  - `system_client_uses_new_name` (reads "balanced", and sets "performance" on the fake);
  - `system_client_falls_back_to_hadess_name`;
  - `system_client_reports_missing_without_either_name`;
  - `power_call_times_out` (`FakePower` with a 1 s delay and `power_timeout` 50 ms →
    `Err(Timeout)` in under 500 ms).
- [ ] **Step 2:** Run `cargo test -p uqd power`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p uqd`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): read and set the power mode, never waiting more than 3 s`

### Task 4: uqd: the profile service

**Files:**
- Create: `crates/uqd/src/profiles.rs` (`impl Desktop`, as in `sections.rs`) and
  `crates/uqd/src/profiles/tests.rs`, which has its own `rig()` as `monitors/tests.rs` does.
- Modify:
  - `crates/uqd/src/lib.rs` (`mod profiles;`);
  - `crates/uqd/src/service.rs`:
    - the fields `profiles: ProfileStore` and `pub(crate) active: Option<ActiveProfile>`, loaded
      in `Desktop::new` from `paths.profile_active`;
    - `Event::ProfileChanged(String)`;
    - `restore` ends the active profile;
    - `explain` reports `profile`;
    - the per-section event loop moves out of `restore` into
      `pub(crate) fn announce_all(&self)`.
  - `crates/uqd/src/dbus.rs`: the `profile_changed(e, id: &str)` signal and its arm in `serve`'s
    event match. The methods come in Task 5.

**Interfaces:**
- Consumes: Tasks 1–3, plus `commit`, `persist`, `RestoreStore`, `AutoReason::BeforeProfile` and
  `Origin::Profile`.
- Produces (`crate::profiles`):
  - `#[derive(Serialize)] pub struct ProfileSummary { pub id: String, pub name: String, pub icon: Option<String>, pub keys: usize, pub power: Option<String>, pub active: bool }`.
  - `#[derive(Serialize, Default, Debug, PartialEq)] pub struct ProfileReport { pub active: String, pub kept: Vec<Kept>, pub notices: Vec<String> }`.
  - `impl Desktop`:
    - `pub fn list_profiles(&self) -> Vec<ProfileSummary>`, in store order.
    - `pub fn get_profile(&self, id: &str) -> Result<String, DesktopError>` returns
      `Profile::to_toml`. An unknown id is `InvalidArgument("no profile {id:?}")`.
    - `pub fn set_profile(&mut self, toml: &str, replace: bool) -> Result<String, DesktopError>`:
      1. It parses the text; a `ProfileError` becomes `InvalidSetting(text)`.
      2. A theme that isn't installed for the stored variant is refused with `UnknownTheme`.
      3. If the id exists and `replace` is false, it gives
         `Conflict("A profile called “{existing name}” already exists. Replace it?")`.
      4. A 51st profile is `InvalidArgument("You can keep up to 50 profiles.")`.
      5. Otherwise it saves the profile and sends `ProfileChanged(active id)`.
    - `pub fn delete_profile(&mut self, id: &str) -> Result<(), DesktopError>`. The active
      profile gives `InvalidArgument("Turn this profile off before deleting it.")`. Otherwise it
      deletes and sends `ProfileChanged`.
    - `pub async fn apply_profile(&mut self, id: &str) -> Result<ProfileReport, DesktopError>`:
      1. It loads the profile.
      2. `base` is `turn_off(state, old)` if a profile is on, else the state. The `Kept` list
         from that goes into the report.
      3. `(next, memo) = profile::apply(&base, &p, now)`. An error becomes `InvalidSetting`.
      4. It checks that `self.themes.get(&next.theme, next.variant)` exists. If not, it gives
         `UnknownTheme`, and nothing has changed yet.
      5. It saves an `Auto(BeforeProfile)` restore point of `self.state`, named
         `"Before the {name} profile"`. A failure is logged.
      6. `save_active(memo)`, where an `Io` error stops it with nothing changed.
      7. `commit(next, Change::Theme, Origin::Profile(id))`. On `Err`, it puts the old active file
         back (`save_active(old)` or `clear_active`) and returns the error.
      8. `announce_all()`.
      9. Power. The target is `p.power()`, or else the old profile's `power.before` when the
         current mode still equals its `set`. It reads `before` and sets the target. Any
         `PowerError` adds `power_notice(&e.to_string())`. On success `memo.power` is
         `Some(PowerMemo { before, set })`, or when switching from a profile with power, its
         original `before` is carried over.
      10. `self.active = Some(memo)`, then `save_active` again and send `ProfileChanged(id)`.
    - `pub async fn profile_off(&mut self) -> Result<ProfileReport, DesktopError>`:
      1. With no profile on, it returns `Ok(ProfileReport::default())`.
      2. `turn_off` gives the next state. It's committed with `Origin::Profile(id)` only when it
         differs from the current state, then `announce_all()`.
      3. Power, by K3: current == `set` → set `before`; current == `before` → nothing; otherwise
         a `Kept` labelled "Power mode". A `PowerError` adds a notice.
      4. `clear_active`, `self.active = None`, and `ProfileChanged("")`.
    - `pub fn active_profile(&self) -> Option<&ActiveProfile>`.
  - `restore` ends the active profile after its commit: `self.active = None`, `clear_active`,
    `ProfileChanged("")`.
  - `explain` gives `("profile", json!({"name": a.name}))` in place of `("uq", Null)` when
    `self.active` `holds(domain, json_key split on '.', stored)`.

- [ ] **Step 1: Write the failing tests** in `profiles/tests.rs`. Profiles are saved with
  `set_profile`, and `FakeCompositor` and `FakePower` are used:
  - `apply_sets_keys_with_profile_origin` (look gaps 0; each new journal entry has origin
    `profile:gaming`);
  - `apply_saves_before_profile_point` (the newest point is `Auto(BeforeProfile)`, named "Before
    the Gaming profile");
  - `off_restores_untouched_and_reports_hand_changed`;
  - `switching_profiles_returns_untouched_keys_then_applies_next` (Review Focus 2);
  - `active_profile_survives_restart`: a new `Desktop` on the same `Paths` has
    `active_profile()` set, and `profile_off` restores.
  - `active_file_written_but_state_not_still_turns_off_cleanly`: write a memo with
    `save_active` without the commit, then start a new `Desktop`. `profile_off` writes nothing
    and reports nothing (Review Focus 1).
  - `failed_verify_changes_nothing_and_keeps_previous_profile`: the compositor rejects. The
    state, the active file, `active_profile()` and the restore point count are unchanged
    (Review Focus 1).
  - `apply_with_uninstalled_theme_changes_nothing` (Review Focus 5);
  - `power_missing_is_a_notice_not_a_failure`, `power_refused_is_a_notice` and
    `power_timeout_is_a_notice`. In each, the notice text equals
    `power_notice(&PowerError::… .to_string())` and the settings did apply (Review Focus 3).
  - `off_puts_power_back_only_if_unchanged`;
  - `restore_ends_the_active_profile`;
  - `explain_reports_profile_source`: `gaps_in` reports `profile` with detail name "Gaming".
    After a hand change of `gaps_in` it reports `uq`.
  - `set_profile_conflict_then_replace`, `set_profile_with_unknown_theme_is_refused`,
    `delete_active_profile_is_refused` and `fifty_first_profile_is_refused`;
  - `profile_changed_event_sent` (on apply, off, set and delete).
- [ ] **Step 2:** Run `cargo test -p uqd profiles`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `service.rs` growth to the fields, the `Event` variant, the
  `restore`/`explain` hooks and `announce_all`. Everything else goes in `profiles.rs`.
- [ ] **Step 4:** Run `cargo test -p uqd`. Expected: PASS, including every slice A test.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): profiles that apply as one write and turn off without undoing your own changes`

### Task 5: D-Bus methods, the client proxy and `uqctl profile`

**Files:**
- Create: `crates/uqctl/src/profile_cmd.rs`.
- Modify:
  - `crates/uqd/src/dbus.rs` (six methods);
  - `crates/uq-core/src/client.rs` (`DesktopProxy` methods and `profile_changed` signal);
  - `crates/uqd/tests/dbus.rs`;
  - `crates/uqctl/src/main.rs` (`Cmd::Profile { cmd: ProfileCmd }`, `mod profile_cmd;`, the
    dispatch).

**Interfaces:**
- Consumes: Task 4's `Desktop` methods, `ProfileSummary` and `ProfileReport`; Task 1's
  `read_file` and `MAX_FILE_BYTES`; Task 2's `next_after`.
- Produces (D-Bus, `org.uq.Desktop1`, as in Global Constraints):
  - `list_profiles() → s` (JSON array of `ProfileSummary`);
  - `get_profile(s id) → s`;
  - `set_profile(s toml, b replace) → s`;
  - `delete_profile(s id)`;
  - `apply_profile(s id) → s` (JSON `ProfileReport`);
  - `profile_off() → s`.

  `DesktopProxy` mirrors them all, plus `#[zbus(signal)] fn profile_changed(&self, id: &str)`.
- `uqctl` (`profile_cmd.rs`):
  - `#[derive(Subcommand, Debug, PartialEq)] pub enum ProfileCmd` has these variants:
    - `List`, `Apply { id: String }`, `Off` and `Next`;
    - `Export { id: String, file: Option<PathBuf> }`, which writes to stdout without a file;
    - `Import { file: PathBuf, #[arg(long)] replace: bool }`;
    - `Delete { id: String }`.
  - `pub fn profile_lines(json: &str) -> Result<Vec<String>, String>` gives
    `"* gaming  Gaming  (6 settings)"` for the active profile and `"  work  Work  (1 setting)"`
    for the others. With none, it gives
    `["No profiles yet. Make one on the Profiles page of uQ Settings."]`.
  - `pub fn report_lines(json: &str) -> Result<Vec<String>, String>` gives:
    - first `"Profile on: {active}"`, or `"Back to normal"`;
    - then one `"Kept your own value: {label}"` per kept key;
    - then each notice as it is.
  - `pub fn next_target(list_json: &str) -> Result<Option<String>, String>` uses `next_after` on
    the ids, in list order, and the active one.
  - `pub fn read_import(path: &Path) -> Result<String, String>` uses `profile::read_file`, with
    the error's `Display`.
  - `pub async fn run(proxy: &zbus::Proxy<'_>, cmd: ProfileCmd) -> Result<(), String>`. A
    `Conflict` on import adds " (run again with --replace)". Export writes atomically.

- [ ] **Step 1: Write the failing tests:**
  - In `crates/uqd/tests/dbus.rs`:
    - `profile_crud_over_dbus` (set, list, get round-trips the TOML, conflict, replace, delete);
    - `apply_and_off_over_dbus_emit_profile_changed` (the report JSON has `active: "gaming"`,
      then `""`, and the signal carries the same ids);
    - extend `desktop_proxy_matches_the_interface` to call all six through `DesktopProxy`.
  - In `profile_cmd.rs`:
    - `cli_parses_profile_commands`;
    - `profile_lines_format` (marker, plural, and the empty list);
    - `report_lines_format`;
    - `next_target_cycles` (normal → first → second → normal);
    - `read_import_refuses_big_and_binary_files` (Review Focus 4).
- [ ] **Step 2:** Run `cargo test -p uqd --test dbus` and `cargo test -p uqctl profile`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test --workspace --exclude uq-settings` (or `cargo test --workspace`
  where Qt is installed). Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqctl): uqctl profile, and profile methods on D-Bus`

### Task 6: The bridge and the quick-settings profile row

**Files:**
- Create: `shell/quick/ProfileGroup.qml`.
- Modify:
  - `crates/uqctl/src/bridge/uqd.rs` (`Request` variants and their parsing and sending; the
    `ProfileChanged` signal in `forward`);
  - `crates/uqctl/src/bridge/mod.rs` (the call names in `handle`);
  - `crates/uqctl/tests/bridge_dbus.rs`;
  - `shell/core/Bridge.qml` (`signal profileChanged(string id)`, and
    `case "ProfileChanged": if (data) profileChanged(String(data.id)); break;`);
  - `shell/logic/quickui.js` and `tests/qml/tst_quickui.qml`;
  - `shell/quick/QuickSettings.qml` (`ProfileGroup { Layout.fillWidth: true; active: root.shown }`
    after `ThemeGroup`).

**Interfaces:**
- Consumes: Task 5's `ListProfiles`, `ApplyProfile`, `ProfileOff` and `ProfileChanged`.
- Produces:
  - Bridge calls:
    - `ListProfiles` (0 args) answers uqd's JSON parsed into an array.
    - `ApplyProfile` (1 string arg, the id) and `ProfileOff` (0 args) answer the parsed
      `ProfileReport` object.

    The `ProfileChanged` event's data is `{"id": "<id>"}`. These are reads or profile
    switches, so there's no `ORIGIN` argument; uqd journals them as `profile:<id>`.
  - `quickui.js`:
    - `profileList(v)` turns a `ListProfiles` reply into `[{id, name, active}]`. It keeps rows
      whose `id` and `name` are non-empty strings, at most 50, with `active === true` only for a
      literal `true`. Anything else → `[]`.
    - `activeProfile(list)` gives the active row's id, or `""`.
    - `profileReportText(r)` gives `"Kept your own value: A, B"` (from `r.kept[].label`) when
      anything was kept, then each of `r.notices`, joined with a space. A malformed report or `null` → `""`.
  - `ProfileGroup.qml` follows `ThemeGroup.qml`'s style and rules:
    - It has a heading "Profile", then a `Flow` of chips: "Normal" first, then one per profile
      (`UqText`, name cut to one line, 160 px max).
    - The active chip is accented. Each chip is an `Accessible.RadioButton` with
      Space/Return/click. The "Normal" chip is named "Back to normal" for accessibility.
    - Choosing calls `ApplyProfile [id]` or `ProfileOff []`, then shows `profileReportText` in a
      muted `UqText` until the panel closes.
    - It refetches when the panel opens, on `Bridge.profileChanged` and on `uqdUp`. It's
      `visible: profiles.length > 0`, greyed out while uqd is down, and ignores failures quietly.

- [ ] **Step 1: Write the failing tests:**
  - In `crates/uqctl/tests/bridge_dbus.rs`, `bridge_lists_applies_and_turns_off_profiles`. Save
    a profile through a direct `DesktopProxy::set_profile`, then check:
    - `ListProfiles` has `gaming`;
    - `ApplyProfile ["gaming"]` answers `active: "gaming"`;
    - a `ProfileChanged` event arrives with `{"id":"gaming"}`;
    - `ProfileOff` answers `active: ""`.
  - In `crates/uqctl/src/bridge/uqd.rs`, a new `#[cfg(test)] mod tests` with
    `profile_requests_check_arity` (`ApplyProfile []`, `ApplyProfile [1]` and
    `ProfileOff ["x"]` are refused).
  - In `tests/qml/tst_quickui.qml`: `test_profile_list` (malformed input, cap, non-string ids),
    `test_active_profile` and `test_profile_report_text`.
- [ ] **Step 2:** Run `cargo test -p uqctl --test bridge_dbus` and `cargo test -p uqctl bridge`.
  Expected: FAIL. The QML suite runs in CI (K4).
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -p uqctl` and `scripts/lint-shell.sh`. Expected: PASS. Where Qt
  and Quickshell are installed, also run
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml` and
  `tests/shell/bar_loads.sh`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(shell): switch profiles from quick settings`

### Task 7: uq-settings: the profiles page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/profiles.rs`;
  - `apps/uq-settings/src/pages/profiles/tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/profiles.rs`, a `FakeProfiles` store, as
    `fake_uqd/devices.rs` does for `Displays`.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod profiles;`);
  - `apps/uq-settings/src/pages/fake_uqd.rs`: the six methods and the `profile_changed` signal on
    `FakeUqd`, delegating to `FakeProfiles`; `Rig.profiles`; and the `FakeError` variant
    `UnknownTheme(String)` if needed.

**Interfaces:**
- Consumes:
  - Tasks 1–2's `Profile`, `apply`, `turn_off`, `changed_since`, `from_journal`, `leaves`,
    `key_label`, `value_text`, `read_file` and `power_notice`;
  - Task 5's `DesktopProxy` methods;
  - `registry::find(page).domains`.
- Produces:
  - **The fake.** `FakeProfiles { store: Arc<Mutex<BTreeMap<String, Profile>>>, active: Arc<Mutex<Option<ActiveProfile>>> }`.
    It follows uqd's rules with the same uq-core functions:
    - conflict, replace, and refusing to delete the active profile;
    - apply and off go through `FakeUqd::write` with origin `profile:<id>`;
    - power always adds `power_notice("power modes aren't available on this computer")`.
  - **The page logic.**
    - `pub const ICONS: [&str; 7] = ["preferences-desktop", "input-gaming", "video-display", "battery", "weather-clear-night", "applications-office", "media-playback-start"]`.
    - `#[derive(Debug, Clone)] pub enum Op` has these variants:
      - `Refresh`;
      - `Apply { id: String }`, `Off`;
      - `CreateFromPages { name: String, icon: String, pages: Vec<String> }`;
      - `CreateFromHistory { name: String, icon: String, since: i64 }`;
      - `AddKeys { id: String, keys: Vec<(String, Vec<String>)> }`;
      - `RemoveKey { id: String, domain: String, path: Vec<String> }`;
      - `Rename { id: String, name: String, icon: String }`;
      - `Delete { id: String }`;
      - `Export { id: String, path: PathBuf }`;
      - `Import { path: PathBuf, replace: bool }`.
    - `pub struct ProfilesState` (a `PageLogic`), with `new(conn: zbus::Connection) -> Self`.
  - **How each op works:**
    - **Creating a profile.** The id is `id_for_name(name, ids)`. `CreateFromPages` takes, for
      each page's journal domains, the keys `from_journal` maps. `theme` comes from
      `get_theme` and sections from `get_section`, with every leaf through `leaves`.
      `CreateFromHistory` uses `changed_since(list_history(500), since)` with the current
      values. A skip count above 0 adds the notice "{n} of those changes can't go in a profile".
      No keys gives the error `ProfileError::Empty`'s text.
    - **Edits.** Every edit is `get_profile` → `Profile::parse` → change → `set_profile(…, true)`.
      `AddKeys` takes the current values. Removing the last key gives `Empty`'s text.
    - **Export** writes `get_profile`'s text atomically.
    - **Import** reads with `read_file`. On a `Conflict` it keeps
      `{seq, path, message}` for the page to ask about, and `Import { replace: true }` answers
      it.
    - **Apply/Off** keep the returned report as `{seq, kept: [labels], notices}`.
  - **The snapshot:**
    ```
    { profiles: [{id, name, icon, active, power, keys: [{domain, path, label, text}]}],
      active: "<id>" | "",
      available: [{domain, path, label, text}],
      pages: [{id, name}],
      icons: ICONS,
      notice: {seq, text},
      report: {seq, kept, notices},
      conflict: {seq, path, message} | null }
    ```
    - `available` holds every leaf of the current values of the profile domains, plus
      `appearance.theme` and `appearance.variant`, labelled.
    - `pages` lists the core pages with at least one domain `from_journal` maps, in registry
      order: Appearance, Look and layouts, Wallpaper and Shell.
    - `power` is shown with `power_label`.

- [ ] **Step 1: Write the failing tests** in `pages/profiles/tests.rs`, against the fake uqd:
  - `create_from_pages_captures_current_values` (Look with gaps 8 and Appearance with the
    theme → a profile with `look.gaps_in = 8`, `appearance.theme` and `appearance.variant`);
  - `create_from_history_takes_keys_changed_since`: a gaps change before `since`, a blur change
    and an idle change after it. The profile has only `look.blur`, and the notice says "1 of
    those changes can't go in a profile".
  - `create_with_nothing_to_hold_is_an_error`;
  - `add_and_remove_keys`;
  - `apply_then_off_reports_kept_keys` (`report.kept == ["Gaps between windows"]`);
  - `apply_with_power_shows_notice`;
  - `export_then_import_round_trips` (in a tempdir; importing the same id gives a conflict, and
    `replace` then succeeds);
  - `import_refuses_hostile_file` (100 KiB → error text "The file is too big to be a profile.";
    Review Focus 4);
  - `delete_active_is_refused_with_message`;
  - `uqd_down_gives_empty_state_without_an_error`.
- [ ] **Step 2:** Run `cargo test -p uq-settings profiles` where Qt is installed (else CI, K4).
  Expected: FAIL.
- [ ] **Step 3:** Implement. No `cxx_qt` or Qt types in `pages/profiles.rs`.
- [ ] **Step 4:** Re-run. Expected: PASS. Also run `cargo clippy -p uq-settings --all-targets -- -D warnings`
  where Qt is installed.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): profiles page logic`

### Task 8: uq-settings: the Profiles page, its model and its registry entry

**Files:**
- Create:
  - `apps/uq-settings/src/profiles_model.rs`;
  - `apps/uq-settings/qml/pages/ProfilesPage.qml`;
  - `apps/uq-settings/qml/components/ProfileKeyList.qml`;
  - `tests/settings-qml/tst_profile_key_list.qml`.
- Modify:
  - `apps/uq-settings/build.rs` (both QML files and `src/profiles_model.rs`);
  - `apps/uq-settings/src/main.rs` (`mod profiles_model;`);
  - `apps/uq-settings/src/registry.rs`;
  - `apps/uq-settings/src/model_core.rs`: `Follow` gains `profiles: bool`, and `follow_uqd`
    adds `receive_profile_changed()` when it's set.
  - every `*_model.rs` `FOLLOW` (`profiles: false`; `true` in `profiles_model.rs`);
  - `scripts/settings-jargon.txt` (add `power-profiles-daemon`).

**Interfaces:**
- Consumes: Task 7's `ProfilesState`, `Op` and the snapshot.
- Produces:
  - `ProfilesModel` (a `#[qml_element]`, not a singleton) has the properties `state_json`,
    `error` and `busy`. Its invokables are:
    - `refresh()`, `apply(id)` and `off()`;
    - `create_from_pages(name, icon, pages_json)`, where `pages_json` is a JSON array of page ids;
    - `create_from_history(name, icon, since: i64)`;
    - `add_keys(id, keys_json)`, where `keys_json` is `[[domain, [path…]], …]`;
    - `remove_key(id, domain, path_json)`;
    - `rename(id, name, icon)` and `delete(id)`;
    - `export_to(id, url)` and `import_from(url, replace)`, which turn a `file://` URL into a
      path with `QUrl::toLocalFile`, as the wallpaper editor's dialogs do.

    Its `FOLLOW` is: sections `look`, `layouts`, `appearance`, `shell` and `wallpaper`; `theme`;
    `history`; and `profiles`.
  - **Registry.** The page is `PageInfo { id: "profiles", name: "Profiles", icon: "bookmarks", category: Category::Uq, qml: "pages/ProfilesPage.qml", settings: &PROFILES_SETTINGS, domains: &[] }`,
    placed between `shell` and `history`. `PROFILES_SETTINGS` has two entries:
    - `entry("profiles", "Profiles", &["modes", "gaming", "presentation", "switch", "preset"])`;
    - `entry("profile-files", "Export or import a profile", &["share", "file", "backup", "copy"])`.
  - **`ProfileKeyList.qml`** has the properties `keys` (the snapshot's `keys` rows) and
    `editable: bool`, and the signal `removeKey(string domain, string pathJson)`. Each row shows
    the label and text through `UqLabel`, and a "Remove" button when editable. An empty list
    says "No settings in this profile yet".
  - **`ProfilesPage.qml`**, built with Kirigami, all text through `UqLabel`, everything
    reachable by keyboard:
    - **Header.** A **New profile** button and **Import…**.
    - **One card per profile.** It has:
      - the icon and name, and an "On" tag when active;
      - **Turn on**, or **Back to normal** when active;
      - an expandable `ProfileKeyList`;
      - **Add settings…**, a sheet of `available` with check boxes;
      - **Rename…**, **Export…**, and **Delete**. Delete is disabled while active, with the hint
        "Turn it off first".
    - **New profile** is a `Kirigami.Dialog` with:
      - a name field (100 chars max) and an icon choice from `icons`;
      - a choice between "Settings I changed in the last hour / today / the last 7 days / the
        last 30 days" (computed `since`) and "The current settings of these pages" (check boxes
        from `pages`).
    - **After Turn on / Back to normal**, a `Kirigami.InlineMessage` lists "Kept your own value:
      …" and the notices.
    - **A `conflict`** opens a `Kirigami.PromptDialog`: "{message}" with **Replace** and
      **Cancel**.
    - **`notice` and `error`** show as on the History page.
- [ ] **Step 1: Write the failing tests:**
  - In `registry.rs`, `profiles_page_registered_with_search_entries`. The existing
    `every_setting_has_keywords`, `no_duplicate_setting_ids`, `ids_unique_and_categories_ordered`
    and `every_journal_domain_has_one_page` must still pass.
  - In `tst_profile_key_list.qml`, these test functions:
    - `test_rows_show_labels_and_values`;
    - `test_remove_emits_domain_and_path` (`"look"`, `'["gaps_in"]'`);
    - `test_not_editable_hides_remove`;
    - `test_empty_list_says_so`.
- [ ] **Step 2:** Run `cargo test -p uq-settings registry` and the settings QML suite where Qt is
  installed (else CI, K4). Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `scripts/lint-settings.sh` and `tests/shell/lint_settings.sh`. Expected:
  PASS. CI runs
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`
  and `cargo test -p uq-settings`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): the Profiles page`

### Task 9: "From the … profile" on rows, and profile names in history

**Files:**
- Modify:
  - `apps/uq-settings/qml/components/SettingRow.qml` and
    `tests/settings-qml/tst_setting_row_source.qml`;
  - `apps/uq-settings/src/pages/runner.rs` (`rank`);
  - `apps/uq-settings/src/pages/fake_uqd.rs` (`explain` answers `profile` when the fake's active
    profile `holds` the key);
  - `apps/uq-settings/src/pages/look.rs` (a test);
  - `apps/uq-settings/qml/components/OriginLabel.qml` and `tests/settings-qml/tst_session_changes.qml`;
  - `apps/uq-settings/src/pages/history.rs` (the snapshot gains `profiles`);
  - `apps/uq-settings/qml/pages/HistoryPage.qml` and
    `apps/uq-settings/qml/components/SessionChanges.qml` (pass the names);
  - `apps/uq-settings/src/history_model.rs` (`FOLLOW.profiles = true`).

**Interfaces:**
- Consumes: Task 4's `profile` source with `detail.name`, Task 5's `list_profiles` and Task 8's
  `Follow::profiles`.
- Produces:
  - `SettingRow.sourceText` gains `case "profile"`: `sourceDetail.name ? "From the " + sourceDetail.name + " profile" : "From a profile"`.
    It's muted like "From the theme", with no button.
  - `runner::rank` becomes `error` 6, `user_lua` 5, `runtime` 4, `profile` 3, `uq` 2,
    `theme`/`default` 1.
  - `HistoryState::snapshot` adds `"profiles": {"<id>": "<name>"}` from `list_profiles`. A
    failure gives `{}`.
  - `OriginLabel` gains `property var profileNames: ({})`. For `profile:<id>` it shows
    `"from the " + name + " profile"` when the name is known, else today's
    `"from the profile " + id`.

- [ ] **Step 1: Write the failing tests:**
  - In `tst_setting_row_source.qml`, `test_profile_source` (the text, muted, no button, and
    `"From a profile"` without a name).
  - In `runner.rs` tests, `profile_source_outranks_uq_but_not_runtime`.
  - In `look.rs` tests, `look_snapshot_shows_profile_source` (the fake's active profile sets
    `gaps_in`, and `sources["gaps-inner"].source == "profile"`).
  - In `history.rs` tests, `snapshot_names_profiles`.
  - In `tst_session_changes.qml`, `test_profile_origin_uses_name`.
- [ ] **Step 2:** Run `cargo test -p uq-settings` and the settings QML suite where Qt is installed
  (else CI, K4). Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run both, plus `scripts/lint-settings.sh`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): rows say when a profile set them; history names profiles`

### Task 10: Docs, changelog, live checks and the whole gate

**Files:**
- Modify: `README.md` and `CHANGELOG.md` (`## [Unreleased]` → `### Added`).
- Create: `docs/notes/settings-slice-b-live-check.md`.

- [ ] **Step 1:** Update `README.md`:
  - Add a "Profiles" block under "History and restore points":
    - `uqctl profile list`;
    - `uqctl profile apply <id>`;
    - `uqctl profile off`;
    - `uqctl profile next` (normal → each profile → normal);
    - `uqctl profile export <id> [file]`;
    - `uqctl profile import <file> [--replace]`;
    - `uqctl profile delete <id>`.
  - Add a "#### Profiles page" paragraph. It covers what a profile holds (K1), Back to normal
    and kept keys (K3), the power mode (K2), the quick-settings row, the restore point saved
    before each switch, and the user.lua keybind line from Global Constraints. It says that a
    shortcut setting comes with the keybinds page.
  - Add the new D-Bus methods to the Settings paragraph.
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Profiles.** It names the Profiles page, the
  quick-settings row, `uqctl profile`, power mode support with its notice, export and import as
  TOML `format = 1`, and the `ProfileChanged` signal.
- [ ] **Step 3:** Write `docs/notes/settings-slice-b-live-check.md` with these checks:
  1. **Create and turn on.** Make "Gaming" from the Look page's current values after setting
     gaps 0 and animations off. Add "Power mode: Performance" by importing a file with
     `[power] profile = "performance"` and `--replace`. Turn it on from quick settings:
     - gaps and animations change at once;
     - the quick-settings power row shows Performance;
     - History lists the changes "from the Gaming profile";
     - a "Before the Gaming profile" restore point exists;
     - the Look rows say "From the Gaming profile".
  2. **A hand change survives Back to normal.** While it's on, set gaps to 4 on the Look page,
     then press Back to normal. Gaps stay 4 and the message names "Gaps between windows";
     animations and the power mode come back.
  3. **Restarts keep it.** `systemctl --user restart uqd` while Gaming is on. The tile and the
     page still show it on, and Back to normal still works.
  4. **No power service.** With `sudo systemctl mask --now power-profiles-daemon`, turning
     Gaming on applies everything else and says "Power mode wasn't changed: …". Unmask it
     afterwards.
  5. **Export and import.** `uqctl profile export gaming /tmp/g.toml`, then
     `uqctl profile delete gaming`, then `uqctl profile import /tmp/g.toml`. A second import says
     it already exists, and `--replace` works. Importing a 1 MiB file and a file with
     `[keybinds]` is refused with plain words.
  6. **`next` and a keybind.** With two profiles, `uqctl profile next` goes first → second →
     normal. The `user.lua` bind from the README does the same from the keyboard.
  7. **Restore ends a profile.** Restore the "Before the Gaming profile" point while Gaming is
     on. The settings come back and no profile shows as on.
- [ ] **Step 4:** Run the full gate, exactly as CI does:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`;
  - `cargo test --workspace --locked -- --include-ignored`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh`;
  - both `qmltestrunner` suites.

  In the build container (K4), run everything without Qt, and leave `uq-settings`, the QML
  suites and the Quickshell-based shell tests to CI. Expected: all PASS.
- [ ] **Step 5:** Commit (K5 trailers): `docs: settings slice B — profiles, live checks and changelog`
