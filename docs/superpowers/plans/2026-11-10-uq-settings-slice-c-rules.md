# uQ Settings enhancements slice C: window and workspace rules — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A "Window rules" page in `uq-settings` makes Hyprland window, panel and workspace rules
without writing Lua. You pick a window by clicking it, see which open windows a rule matches while
you type, ask "Why does this window…?" to list every uQ rule that matches a window, and start from
one-click templates. Rules are a normal uQ domain: validated, rendered into `generated.lua`,
verified by Hyprland before they apply, journalled for undo and restore points, and explained
(a rule that `user.lua` overrides says so).

**Architecture:**
- **uq-core** gains `rules`:
  - `rules/hypr.rs`, the **one table** of Hyprland Lua names (match keys, effect keys, value
    kinds, static or dynamic), checked against the pinned Hyprland source;
  - `rules/mod.rs`, the domain model (`Rules { window, layer, workspace }`), its validation,
    id assignment and regex checks;
  - `rules/matcher.rs`, the Qt-free matcher that the live preview, "Why does this window…?" and
    the pick lookup share, `rules/templates.rs`, and `rules/explain.rs` (the `Explain("rules")`
    rows, shared by uqd and the app's fake uqd).

  `lua/rules.rs` renders the `-- Rules` block. `history` learns to summarise and label rules.
- **uqd:**
  - `rules` becomes a section domain, so `GetSection`, `SetSection`, `Explain` and `Reapply`
    cover it, and every write goes through `commit` → `persist(next, origin)`.
  - `pick.rs` holds the pick broker. New D-Bus calls: `PickWindow() → s`, `PickDone(s token,
    s outcome)`, `ListWindows() → s`, and the signal `PickRequested(s token)`.
  - The compositor gains `clients` (`j/clients`) and `cursor_pos` (`j/cursorpos`).
- **uqctl bridge / shell:** the bridge forwards `PickRequested` and the `PickDone` call. The shell
  gets a full-screen pick overlay (`shell/pick/PickOverlay.qml`, logic in `shell/logic/pick.js`).
- **uq-settings:** a Qt-free page logic module `pages/rules.rs`, tested against the fake uqd, a
  thin `rules_model.rs`, the `RulesPage.qml` page and its components, and a registry entry under
  the uQ category.

**Tech Stack:** Rust edition 2024, zbus 5, `regex` 1 and `regex-syntax` (new), cxx-qt 0.10, Qt 6,
Kirigami 6, Quickshell, Hyprland 0.56.2–0.57.x (Lua config).

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. This plan covers
§5 (the authority), the rules parts of §2 (the `user_lua` and `error` sources for rules), §3
(history entries and labels for the `rules` domain), the §4 `rules` reservation (kept, see the
ruling), §9, §10 (`PickWindow → s` and the pick-mode security line), and §12 slice C. It builds on
slice A (`2026-11-10-uq-settings-slice-a-trust.md`, merged) and follows slice B's layout
(`2026-11-10-uq-settings-slice-b-profiles.md`, merged).

**Paths.** The settings app lives at `apps/uq-settings` (not `crates/`). Its QML tests are in
`tests/settings-qml`, the shell's in `tests/qml`.

## Pre-flight rulings

A conflict scan against the code at `a28e7ab`
(`.superpowers/sdd/2026-11-10-uq-settings-slice-c-rules/preflight.md`) found 37 issues. All
rulings are accepted and folded into the task text below; this list is the index. The task count
is unchanged (1–11).

- **F1** Rules load per entry: a custom `Deserialize` keeps the entries that parse; entry structs are `deny_unknown_fields` (Task 2).
- **F2** A supplied id must be valid and unique *within the payload*; current ids only steer `fresh_id`. Resending `GetSection` unchanged is accepted (Task 2).
- **F3** `check_pattern` also refuses the flags `x`/`R`, special word boundaries (`\<`, `\>`, `\b{…}`), repeat bounds over 1000 and every `\p`/`\P` class (Task 2).
- **F4** Pattern refusals use fixed plain reasons, never the regex crate's text (Task 2).
- **F5** A failed `PickWindow` still records a pick status and bumps `pick.seq`; the page also re-shows on `busy` going false (Tasks 9, 10).
- **F6** The snapshot carries a `fields` catalogue built from `rules::hypr`; QML never copies the tables (Tasks 1, 9, 10).
- **F7** `rules::explain` is a pure uq-core function used by both uqd and the fake uqd (Tasks 5, 6, 9).
- **F8** The user.lua finders cut each line at `--` like `find_in_user_lua`; the rule's Lua name `uq:<id>` is the override marker (see the spec deviation below) (Task 5).
- **F9** `Explain("rules")` adds a section row `"rules"` with source `error` after a rejected write (Tasks 5, 6, 10).
- **F10** `WindowInfo` has `workspace_id` and `workspace_name`; bracket selectors give `Verdict::Unknown` (Task 4).
- **F11** The three history label rows land in Task 2; the registry domain test skips `rules` from Task 2 until Task 10 (Tasks 2, 5, 10).
- **F12** Task 3 re-blesses the golden `generated/*.lua` files and keeps `emit_workspace_rule` for uqd's `SetLayout` eval (Task 3).
- **F13** `Event::PickRequested(String)` carries the signal through `serve()` (Task 7).
- **F14** Pick tokens come from `monitors::new_token()` (Task 7).
- **F15** `windows_from_clients` uses `settings::clean_text` / a sibling `clean_text_spaced` (Task 4).
- **F16** `last_rejected` and `read_user_lua` become `pub(crate)` (Task 6).
- **F17** Preview is a synchronous pure invokable on `RulesModel`, not an `Op` (Tasks 9, 10).
- **F18** The snapshot's `pick` carries `class_pattern`; `RulesModel.exact(text)` is the one escaper QML uses (Tasks 9, 10).
- **F19** The Lua mock prints its calls; the hostile test counts rule calls and looks for the payload, not for any `hl.exec_cmd` (Task 3).
- **F20** Selector errors are re-keyed to the rule's field (Task 2).
- **F21** Rules live in `State.rules` beside `layouts`, not in `Settings`, so `SettingsChanged` doesn't carry them (Task 2).
- **F22** `MATCH_KEYS` is `[Field; 11]`; `silent` is listed explicitly in the known-key list (Tasks 1, 2).
- **F23** File lists completed (Tasks 5–9).
- **F24** Task 10 updates `profiles_page_registered_with_search_entries` and `pages_json_lists_pages_in_order`; icon `preferences-system-windows` (Task 10).
- **F25** The profile pin goes into `from_journal_maps_domains` in `profile/overlay.rs`; no comment-only test change (Task 5).
- **F26** QML plain-text tests find the label by `objectName` and assert its literal text (Task 10).
- **F27** Summary "fuzz" is a deterministic enumeration (Task 5).
- **F28** Dialog titles are fixed literals; no ToolTip/FormData bound to data; template menu items use `contentItem: UqLabel` (Task 10).
- **F29** RuleEditor builds JSON from catalogue keys; raw names live in a new `components/AdvancedDetails.qml` (Task 10).
- **F30** Spec deviations listed below and in the PR; rules-in-profiles gets a follow-up row (Task 11).
- **F31** `Verdict` and `Needs` serialise snake_case (Task 4).
- **F32** `window_at` skips disabled monitors (Task 4).
- **F33** Tag names starting with `uq_` are refused (Task 2).
- **F34** Summaries show `name:web` as "web" (Task 5).
- **F35** `RulesModel` uses slice A's refresh on window activation (Task 10).
- **F36** Undo restores a whole list; the README says so (Task 11).
- **F37** `regex` must be fetched once through the proxy before `--locked` builds (Task 1).

**Spec deviations (F30, recorded here and in the PR):**
1. **Override markers (F8).** Spec §2 says rule overrides are found through `-- uq:rule` markers
   "Phase 3 §2.4 already defines". §2.4 defines only `-- uq:bind`. For rules, the rendered Lua
   name `name = "uq:<id>"` is the marker; comment markers are not recognised, and comments are
   ignored when scanning (a `--` comment never counts as an override).
2. The shell returns no window data; uqd reads `clients` itself after a `click` (§5 says the shell
   returns the fields). This is stronger isolation.
3. The pick reply has the clicked window's 10 fields, not 5.
4. `ListWindows` exists (not in §10's D-Bus list).
5. `tag` matches a tag name, not a regex.
6. Profiles still refuse `rules`, so §4 "rules (enable or disable named rules only)" has no slice
   yet; Task 11 adds a follow-up row. Random per-machine ids make `[rules] "<id>"` profiles
   non-portable; the slice that lifts the reservation decides how to key them.

## Global Constraints

These are copied from the spec and the user's standing rules, except the lines marked
**ruling**, which this plan decides.

### Process

- **K4, no Qt here.** The build container has no Qt and no Quickshell. `uq-settings`, both
  `qmltestrunner` suites and `tests/shell/*.sh` that need Quickshell run in CI only. Every piece of
  logic that can be Qt-free is: the page logic in `apps/uq-settings/src/pages/rules.rs`, the
  matcher and templates in `uq-core`, and the pick state machine in `shell/logic/pick.js` (CI's
  `tst_pick.qml`). Locally, run `cargo test -p uq-core -p uqd -p uqctl`.
- **Build parallelism (earlyoom).** Every cargo command in this plan uses `-j 2`, and test runs
  add `-- --test-threads=2`. Set `CARGO_BUILD_JOBS=2` in the shell before the first build. Don't
  run two cargo commands at once.
- **K5, commits:** every commit message ends with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc, so the trailers sit on their own lines after a blank line.
- **Process (as slice 2):** private `dbus-daemon` buses only (`uq_core::test_bus`); never the real
  session bus or `hyprctl`; `cargo fmt`, `clippy -D warnings` and the tests must be clean; keep
  files at or under about 400 lines; run builds in the foreground and leave nothing running.

### The domain (§5)

- **Shape.** One new section domain, `rules`, stored as `[[rules.window]]`, `[[rules.layer]]` and
  `[[rules.workspace]]` in `state.toml`. Its section JSON is
  `{"window": [...], "layer": [...], "workspace": [...]}`, each in list order.
  - **Ruling (F21):** the field is `State.rules`, beside `State.layouts`, not a `Settings` field.
    `Settings` is broadcast whole in `SettingsChanged` after every section write and parsed by
    the bridge; rules don't belong there. `State::section`/`State::set_section` dispatch
    `"rules"` the way they dispatch `"layouts"`, and `State::parse_strict` calls
    `Rules::repaired` the way it calls `repair_rules`. The TOML location is the same.
  - **Ruling (F1), load is lenient per entry.** `Rules` has a hand-written `Deserialize`: each of
    `window`, `layer` and `workspace` is read as a JSON array, and each entry that parses is kept;
    one that doesn't is dropped with `tracing::warn!` (the pattern of `state.rs::lenient_rules`).
    The entry structs are `#[serde(deny_unknown_fields)]`, so an unknown key costs only its
    entry. A non-array list loads as empty. `repaired()` then applies limits, ids and patterns.
- **Window rule:** `{id, name, enabled, match, effects}`.
  - `id` is stable and made by uqd.
    - **Ruling:** `^r-[0-9a-f]{4,8}$`. uqd makes `r-` plus 4 random hex digits and retries on a
      collision. Ids are unique across window and layer rules.
    - **Ruling:** a rule sent without an id (or with `""`) gets one in `State::set_section`.
    - **Ruling (F2):** a supplied id must be valid and unique within the submitted section
      (across window and layer rules). Every write is whole-section, so existing ids are resent
      by every edit, undo and restore, and that is accepted. The current rules matter only to
      `fresh_id`, which avoids ids that exist now or in the payload.
  - `name` is plain text, 1–100 chars, with no control characters.
  - `enabled` is a bool.
- **Match fields (window, §5):** `class`, `title`, `initial_class`, `initial_title` (patterns);
  `tag`; `xwayland`, `floating`, `fullscreen`, `pinned` (bool); `workspace`. At least one field is
  required.
  - **Ruling, `tag` is a tag name, not a pattern.** Hyprland matches `tag` with `TagMatchEngine`
    (`CTagKeeper::isTagged`), not with its regex engine (`src/desktop/rule/Rule.cpp`). It's
    `^[A-Za-z0-9_.-]{1,32}$`.
  - **Ruling (F33):** a tag name (match or effect) must not start with `uq_`. uQ's own window
    tags (`uq_ws_*`, `crates/uqctl/src/bridge/dispatch.rs`) live there. Reason: "names starting
    with uq_ are kept for uQ's own use".
  - **Ruling, `workspace`** is a selector checked by `settings::rule_selector`, the same check the
    per-workspace layouts use.
  - **Ruling (F20):** `rule_selector`'s errors carry the key `layouts.workspaces`. `Rules`
    re-keys them to the rule's own field (`rules.workspace[2].workspace`,
    `rules.window[3].match.workspace`, `rules.window[3].effects.workspace`). `rule_selector`
    itself is unchanged.
  - **Ruling, one extra match field, `modal` (bool):** "float dialogs from an app" needs it, and
    Hyprland has it (`RULE_PROP_MODAL`, key `modal`). `clients -j` doesn't report it, so the
    preview shows such rows as "Matches if it's a dialog" (see the matcher).
- **Effects (window, §5).** This is a closed set.
  - Static: `float`, `tile`, `fullscreen`, `maximize`, `workspace` (with `silent`), `monitor`,
    `size`, `move`, `center`, `pin`, `no_initial_focus`.
  - Dynamic: `opacity`, `no_blur`, `no_shadow`, `no_anim`, `rounding`, `border_size`,
    `idle_inhibit`, `keep_aspect_ratio`, `tag`.
  - At least one effect is required.
  - **Ruling, values:**
    - bools are `true` or `false`;
    - `workspace` is a selector (`rule_selector`), with `silent: bool` beside it;
    - `monitor` must pass `settings::valid_output_name`;
    - `size` is `[w, h]`, integers 1–16384;
    - `move` is `[x, y]`, integers -16384 to 16384;
    - `opacity` is 0.1–1.0, rounded to 2 places;
    - `rounding` is 0–20 (0.56.2's limit; 0.57 allows 100);
    - `border_size` is 0–20;
    - `idle_inhibit` ∈ `none | always | focus | fullscreen`;
    - `tag` is a tag name, as above.
  - **Ruling, contradictions are refused:** `float` and `tile` both `true`; `move` with
    `center = true`; `silent` without `workspace`.
- **Layer rules (§5):**
  - `match` is `{namespace}`, a pattern, and it's required.
  - `effects` are `blur` (bool), `ignore_alpha` (0.0–1.0, 2 places), `no_anim` (bool) and
    `animation`.
  - **Ruling:** `animation` ∈ `fade | slide | popin`. Those are Hyprland's layer animation styles.
    The exact set is a live-check item (Task 1).
- **Workspace rules (§5, "extends the Phase 1 per-workspace layout rules").**
  `{workspace, monitor?, default?, persistent?, gaps_in?, gaps_out?, border_size?}`.
  - **Ruling, where layouts live:** a workspace's layout stays in `layouts.workspaces`. That's
    where `uqctl layout set`, the SUPER+ALT+D/M/S binds and the Look page write it, and it's
    unchanged. `rules.workspace` holds the other fields, keyed by the same normalised selector.
    The render merges both into one `hl.workspace_rule` call per selector. That's safe either
    way, because Hyprland's `CWorkspaceRuleManager::replaceOrAdd` also merges (`mergeLeft`).
  - **Ruling, limits:**
    - at most 64 workspace rules (the layouts limit);
    - selectors are unique;
    - at least one field besides `workspace`;
    - `gaps_in` and `gaps_out` are 0–100, `border_size` 0–20;
    - `monitor` passes `valid_output_name`.
- **Limits (§5):** 200 window rules, 50 layer rules, names ≤ 100 chars.
  - **Ruling:** patterns are 1–256 chars, and the compiled size is capped (below).
- **Rendering (§5):** "Rules render into a `rules` section of `generated.lua` in list order, so
  later rules win as in Hyprland. All strings go through `lua_string`."
  - **Ruling, block order:** the `-- Workspace rules` block replaces today's
    `-- Per-workspace layouts` block, at the same place. It has the merged calls: first the
    selectors in `rules.workspace` list order, then layout-only selectors in today's sorted order.
    The `-- Window rules` and `-- Panel rules` blocks follow, last in the file. The comment lines
    are fixed text; no user text reaches a Lua comment.
  - **Ruling, names:** every window and layer rule renders `name = "uq:<id>"`. Hyprland keeps
    named rules in a map (`m_luaWindowRules[name]`, `LuaBindingsConfigRules.cpp`). A later call
    with the same name in `user.lua` changes or disables uQ's rule, for example
    `hl.window_rule({ name = "uq:r-7f2c", enabled = false })`. The README documents this as the
    supported way to override a uQ rule from `user.lua`, and `Explain` looks for it.
  - **Ruling, turned-off rules:** a rule with `enabled = false` still renders, with
    `enabled = false`, so its name stays known to Hyprland and to `user.lua`.
- **Patterns (§5):** "Each regex is test-compiled, then the whole file goes through the usual
  verify step."
  - **Ruling, regex crate:** add `regex = "1"` and `regex-syntax = "0.8"` to the workspace and
    to `uq-core`. The repo has no direct regex dependency today; `regex-automata` and
    `regex-syntax` are already in `Cargo.lock` transitively.
    - Hyprland matches with RE2 (`src/desktop/rule/matchEngine/RegexMatchEngine.cpp`). Rust's
      `regex` is the same family: RE2 syntax, linear time, no backreferences or lookaround. That
      makes it the right engine for test-compiling, and safe for matching untrusted window
      titles in the preview.
    - An RE2 compile error is *not* a Hyprland config error (`CRegexMatchEngine` builds the
      `RE2` and the rule just never matches), so the verify step can't catch a bad pattern. The
      test-compile is the only check.
  - **Ruling, how a pattern is checked** (`rules::check_pattern`):
    - it must be 1–256 chars with no control characters;
    - it must not start with `negative:`, which Hyprland reads as "doesn't match"; v1 has no
      such toggle;
    - it must parse with `regex_syntax::ast::parse::Parser`, with no character-class set
      operation (`&&`, `--`, `~~`), since those are Rust-only syntax RE2 rejects;
    - **(F3)** walking the AST (`regex_syntax::ast::visit`), it must have no other syntax RE2
      rejects:
      - flags other than `i`, `m`, `s` and `U` (so no `x` or `R`);
      - no word-boundary assertion other than `\b` and `\B` (so no `\<`, `\>`, `\b{start}`,
        `\b{end}`, `\b{start-half}`, `\b{end-half}`);
      - no counted repetition with a bound over 1000 (RE2's `kMaxRepeat`);
      - no `\p`/`\P` Unicode class at all in v1 (RE2 knows only categories and scripts; the
        allowlist isn't worth it yet).
    - it must compile with `RegexBuilder::new(&format!("^(?:{p})$")).size_limit(1 << 18)`.
  - **Ruling (F4), refusal text.** A refused pattern gives one of these fixed plain reasons,
    never the regex crate's text (which is multi-line, starts "regex parse error:" and repeats
    the pattern):
    - "a bracket or group isn't closed" (unclosed or unopened `(`, `[`, `{`);
    - "a repeat (*, + or ?) has nothing before it";
    - "this uses something Hyprland's patterns don't support" (flags, boundaries, `\p`,
      class-set operations, repeats over 1000);
    - "this pattern is too complex" (`CompiledTooBig`);
    - "patterns are at most 256 characters", "patterns can't be empty", "patterns can't hold
      control characters", "patterns can't start with negative:";
    - any other parse error: "this pattern isn't valid".

    The message is `SettingError::Invalid { key, reason: "this pattern can't be used: <reason>" }`.
  - **Ruling, full match:** Hyprland uses `RE2::FullMatch`, so the matcher compiles
    `^(?:pattern)$`. A pattern without anchors still matches the whole string, as in Hyprland.
  - **Known difference:** Rust's `\d`, `\w` and `\s` are Unicode-aware and RE2's are ASCII. The
    preview can show a match on non-ASCII text that Hyprland doesn't make. The page's help says
    "Preview is a close guide".
- **Single write path (slice A).** Rule writes go through `SetSection("rules", json, origin)` →
  `commit_section` → `commit` → `persist(next, origin)`. That gives history, undo, restore points
  and `Restore` for free.
  - `renders_section("rules")` is true, and `change_for("rules")` is `Change::Reload`.
  - **Ruling:** there are no rule-specific write methods on D-Bus. The page edits the section
    JSON under its write lock, as `look.rs` does for `layouts`.

### Hyprland names (Task 1 holds them all)

- **Source.** The names come from the pinned series' own source:
  - Hyprland `v0.56.2` (tag `efb50993780079460b0cbed1363e2166a2de1d9f`);
  - Hyprland `v0.57.0` (tag `0aa6745f705f2c4b0a5004eb019ef76e5bbdb426`).

  CI's `uq-hyprland-pin` check (`.github/workflows/ci.yml`) enforces 0.56.2–0.57.x. This plan's
  author read these files at both tags:
  - `src/config/lua/bindings/LuaBindingsConfigRules.cpp`:
    - `hl.window_rule`, `hl.layer_rule` and `hl.workspace_rule` are registered with
      `setMgrFn(…, "window_rule" | "layer_rule" | "workspace_rule", …)`.
    - Fields `name`, `enabled` and `match` (a table). Every other key is an effect, and an
      unknown key is `addError("…unknown field…")`. That error fails `--verify-config`.
    - `LAYER_RULE_EFFECT_DESCS`: `no_anim`, `blur`, `blur_popups`, `ignore_alpha` (float 0–1),
      `dim_around`, `xray`, `animation` (string), `order`, `above_lock` and `no_screen_share`.
    - `WORKSPACE_RULE_FIELDS`: `monitor`, `default`, `persistent`, `gaps_in`, `gaps_out`,
      `float_gaps`, `border_size`, `no_border`, `no_rounding`, `decorate`, `no_shadow`,
      `on_created_empty`, `default_name`, `layout` and `animation`.
  - `src/config/lua/bindings/LuaBindingsInternal.hpp`, `WINDOW_RULE_EFFECT_DESCS`:
    - bool effects: `float`, `tile`, `fullscreen`, `maximize`, `center`, `no_initial_focus`,
      `pin`, `no_anim`, `no_blur`, `no_shadow` and `keep_aspect_ratio`;
    - string effects: `monitor`, `workspace`, `idle_inhibit`, `opacity` and `tag`;
    - `move` and `size` are expression vec2s that take a 2-element table, `{ 800, 600 }`;
    - `rounding` is `CLuaConfigInt(0, 0, 20)` on 0.56.2 and `(0, 0, 100)` on 0.57.0;
    - `border_size` is an int.
  - `src/desktop/rule/Rule.cpp` gives the match keys: `class`, `title`, `initial_class`,
    `initial_title`, `float`, `tag`, `xwayland`, `fullscreen`, `pin`, `modal`, `workspace` and
    `namespace`. Note that the spec's `floating` and `pinned` are `float` and `pin` in Lua.
  - `src/desktop/rule/windowRule/WindowRule.cpp`:
    - `idle_inhibit` takes `none`, `always`, `focus` or `fullscreen`;
    - `opacity` is a space-separated string, `"0.9"` or `"0.9 0.8"`.
  - `src/desktop/rule/windowRule/WindowRuleApplicator.cpp`: `applyStaticRule` handles `float`,
    `tile`, `fullscreen`, `maximize`, `move`, `size`, `center`, `monitor`, `workspace`,
    `no_initial_focus` and `pin`. That's the spec's static list; the rest are dynamic.
  - `src/desktop/view/Window.cpp` treats `silent` inside the workspace string as silent, so
    `workspace = "3 silent"`.
  - `src/debug/HyprCtl.cpp`:
    - the `clients` JSON has `address`, `mapped`, `hidden`, `at`, `size`, `workspace{id,name}`,
      `floating`, `monitor`, `class`, `title`, `initialClass`, `initialTitle`, `xwayland`,
      `pinned`, `fullscreen` (an int mode), `tags` and `focusHistoryID`;
    - `cursorpos` is registered.
  - `src/config/shared/workspace/WorkspaceRuleManager.cpp`: `replaceOrAdd` merges rules for the
    same workspace string.
- **Not confirmed from source** (each is a live-check item and a CI `--verify-config` case):
  - the layer `animation` style names;
  - whether the `tag` effect wants `"+name"` (set) or `"name"` (toggle). **Ruling:** emit
    `"+name"`, as the dispatcher form does (`crates/uqctl/src/bridge/dispatch.rs`);
  - whether `gaps_in` and `gaps_out` in `hl.workspace_rule` take a plain integer (they're a
    `CLuaConfigCssGap`);
  - whether named rules are dropped and recreated on `hyprctl reload`.
- **Ruling, one place to fix.** Every Lua key and every value spelling lives in
  `crates/uq-core/src/rules/hypr.rs` (`MATCH_KEYS`, `WINDOW_EFFECTS`, `LAYER_EFFECTS`,
  `WORKSPACE_FIELDS`, `IDLE_INHIBIT`, `LAYER_ANIMATIONS`). The renderer and the validator read
  them from there, and the page reads them through the snapshot's `fields` catalogue (F6). If CI's `--verify-config` or the live check rejects a name, the fix
  is one table row. If a name can't be fixed, drop that effect from the table and the page, and
  say so in the PR. Don't work around it.

### Sources and history (§2, §3)

- **Explain (§2):** "A uQ bind or rule that `user.lua` unbinds or replaces shows as overridden".
  `Explain("rules")` returns one row per rule, keyed `window.<id>`, `layer.<id>` or
  `workspace.<selector>`, each `{source, live: null, stored: null, detail}`. Sources:
  - `error` (with `message`): the last `rules` write was rejected by verification (slice A's
    `last_rejected`).
  - `user_lua` (with `line` and `path`): for a window or layer rule, a line of `user.lua` holds
    the literal `uq:<id>` followed by `"`, `'` or `]`. For a workspace rule, a line holds
    `workspace_rule` and the quoted selector. Both checks use the first match, on the same 1 MiB
    lossy read as today.
  - `uq` otherwise.
  - **Ruling (F8), comments.** The finders cut each line at `--`, exactly as
    `provenance::find_in_user_lua` does (one shared helper), so a commented-out line or a
    trailing comment never counts. The rule's Lua name is the marker; see "Spec deviations".
  - **Ruling (F9), a section row.** While `last_rejected` is `rules`, the answer also has a row
    keyed `"rules"`: `{source: "error", live: null, stored: null, detail: {message}}`. Without it
    a rejected write that added the only rule would leave no row to say so (§9 "no silent
    failure"). The page shows that row at the top.
  - **Ruling (F7), one implementation.** The rows are built by a pure uq-core function,
    `rules::explain(rules, user_lua, rejected, user_lua_path) -> Value`. uqd reads `user.lua` and
    `last_rejected` and calls it; the app's fake uqd calls it too.
  - **Ruling:** there's no `runtime` source for rules. Hyprland 0.56 has no request that lists
    live rules, so uqd can't compare.
  - **Ruling:** a rule from the active profile can't exist, because profiles still refuse
    `rules` (below).
- **History (§3).** `rules` is a journal domain, through `SECTION_DOMAINS`.
  - **Ruling, summaries:** `history::diff::summarize` hands the `rules` domain to
    `rules::summary(before, after)`. It names up to three changes and then "and N more", for
    example:
    - Window rule “Pavucontrol floats” added (or removed, changed, turned on, turned off);
    - Panel rule “Blur notifications” changed;
    - Workspace 3 rule changed;
    - Window rules reordered.

    It never prints rule JSON. Names are already plain text with no control characters. A
    `name:web` selector reads "web" ("Workspace web rule changed") (F34).
  - **Ruling, labels:** `history::label` gains `("rules", "window")` → "Window rules",
    `("rules", "layer")` → "Panel rules" and `("rules", "workspace")` → "Workspace rules".
    These land in Task 2 (F11), because adding `rules` to `SECTION_DOMAINS` makes
    `every_key_a_summary_can_show_has_a_plain_label` need them at once.
    `domain_label("rules")` is "Window rules", for the undo conflict prompt (Task 6).
  - **Undo** restores the whole list that changed (each list is one leaf, as arrays are today).
    A forced undo of an older entry therefore also undoes later edits to other rules in the same
    list; the conflict prompt warns, and the README says so (F36).
- **Ruling, profiles keep refusing `rules` (§4).** Slice B reserved `rules` and `keybinds`
  (`DomainKind::Reserved`, "…which this version of uQ can't do yet"). This slice keeps the
  reservation, because it's the smaller option:
  - §12 row C doesn't ask for profile support.
  - Lifting it needs overlay memos that address list items by id. Slice B's
    `read_leaf`/`write_leaves`/K3 logic works on object paths only, so that's a new
    turn-off case: a rule deleted while the profile is on.
  - The profile format (`[rules] "<id>" = true|false`) is already fixed, so a later slice can
    lift it with no format change.

  Task 5 adds a test pinning this, and `profile::from_journal("rules", …)` stays `None`, so
  "create from history" counts rule changes as "can't go in a profile".

### Pick mode (§5, §10)

- **Flow (§5).** "The app hides itself and asks the shell, through uqd and the bridge, to enter
  pick mode. The shell shows a full-screen overlay; the user clicks a window or presses Esc. The
  shell returns that window's `class`, `title`, `initial_class`, `initial_title` and `xwayland`
  from `clients -j`."
- **Ruling, protocol:**
  1. The app calls `PickWindow() → s`. uqd makes a token with `monitors::new_token()` (16 bytes
     of `/dev/urandom`, 32 lowercase hex; F14), keeps it in a pick broker *outside* the desktop
     lock, and then sends `PickRequested(token)` through `Event::PickRequested` (F13).
  2. The bridge forwards it as the event `PickRequested {token}`. The shell shows the overlay on
     every screen and calls `PickDone [token, "shown"]` at once.
  3. A click calls `PickDone [token, "click"]`, and Esc (or the shell's own 20 s timer) calls
     `PickDone [token, "cancel"]`. The shell sends no coordinates and no window data.
  4. On `click`, uqd reads `j/cursorpos`, `j/monitors` and `j/clients`, finds the window under
     the cursor with `rules::window_at`, and answers `PickWindow` with only that window's
     metadata.
- **Ruling, the reply** (JSON):
  - `{"status": "picked", "window": {class, title, initial_class, initial_title, xwayland,
    floating, fullscreen, pinned, workspace_id, workspace_name, tags}}` (F10);
  - `{"status": "none"}` when the click hit no window;
  - `{"status": "cancelled"}`;
  - `{"status": "timeout"}`;
  - `{"status": "no_shell"}` when there was no `shown` within 2 s;
  - `{"status": "unavailable"}` when Hyprland isn't reachable.

  The extra fields belong to the same clicked window. "Why does this window…?" needs them to
  match `floating`, `fullscreen`, `pinned`, `workspace` and `tag`. The reply never has an
  address, pid or any other window's data (§10: "Pick mode returns only window metadata the user
  clicked").
- **Ruling, timeouts:** `PICK_ACK_TIMEOUT` is 2 s and `PICK_TIMEOUT` is 20 s. zbus proxies have
  no default method timeout; 20 s also stays under libdbus/gdbus's 25 s default, so other
  callers get a reply rather than a bus error. A second `PickWindow` while one is pending is `Conflict("Already choosing a window.")`.
- **Ruling, who can call `PickDone`:** any session-bus client can see the token and call
  `PickDone`. The worst it can do is end a pick early or make it "click" at the current cursor.
  The window data always goes to the `PickWindow` caller, never to the `PickDone` caller.
  `PickDone` with an unknown token does nothing and returns `Ok`.
- **The shell's overlay** follows the lock rule: while `Bridge.locked` it answers `cancel` and
  shows nothing.
- **`ListWindows() → s` (ruling, needed by "Matches now").** It returns a JSON array of the same
  per-window fields as the pick reply, for every mapped window, with no addresses or pids.
  - Any session client can already run `hyprctl clients`, so this exposes nothing new. It's
    still only read when the editor asks: when the page opens, on Refresh, after a pick and after
    a rules write. It never polls (§2: "It never polls").

### Editor behaviour (§5)

- **Pick a window** pre-fills `match.class` with `^` + `regex::escape(class)` + `$`, taken from
  the snapshot's `pick.class_pattern`. "Use exactly this text" calls `RulesModel.exact(text)`;
  QML never escapes by itself (F18).
- **Matches now** re-runs the matcher in Rust on the cached `ListWindows` whenever the draft
  changes (QML debounces typing by 150 ms). It is a synchronous invokable,
  `RulesModel.preview(draft_json)`, with no D-Bus call and no snapshot (F17). "A rule with no
  matches says so but can still be saved."
- **"Why does this window…?"** picks a window and lists every uQ window rule that matches it,
  in list order. Each row has the rule's name, whether it's on, its effects as text, and a flag
  "Overridden in user.lua, line N" from `Explain`. A plain line says "Rules in user.lua aren't
  listed here."
- **When rules apply.** After a save that adds or changes a rule with any static effect, the page
  shows "Close and reopen {names} to apply." It names the open windows the rule matches, up to 3
  and then "and N more". Dynamic effects apply at once on reload, and the page says nothing more.
  - **Ruling:** there's no button that closes the user's windows; closing could lose their work.
- **Templates (§5):**
  - "Float an app's dialogs": `{class, modal: true}` → `{float, center}`;
  - "Open an app on workspace N": `{class}` → `{workspace: N}`, with a "Quietly" check box that
    sets `silent`;
  - "No blur for an app": `{class}` → `{no_blur}`;
  - "Picture-in-picture floats and stays on top":
    `{title: "^(Picture-in-Picture|Picture in picture)$"}` → `{float, pin, keep_aspect_ratio}`;
  - "Keep the screen awake while an app is fullscreen": `{class}` → `{idle_inhibit: "fullscreen"}`.

  Templates that need an app ask for it with Pick a window, or a typed app ID.

### Text and safety (§9, §10)

- **`user.lua` is never written.** uqd only reads it, for `Explain`.
- **Plain words (§9).** Every user-facing string passes `scripts/lint-settings.sh`. Labels:
  - "App ID" for `class`, "Window title" for `title`, and "App ID when it opened" / "Title when it
    opened" for the initial fields;
  - "Older X11 app" for `xwayland`;
  - "Panel rules" for layer rules (bars, notifications, launchers);
  - "Pattern" for regex.
  - **Ruling:** add `regex`, `xwayland`, `namespace`, `hl.window_rule` and `layerrule` to
    `scripts/settings-jargon.txt`. Raw Hyprland names appear only in an "Advanced details"
    disclosure, which is a new `components/AdvancedDetails.qml`, the file the jargon lint
    already exempts (F29). Rule JSON is built in QML from the catalogue's `key` values (data),
    never from quoted key literals such as `"xwayland"`.
  - **Rust-side text isn't linted.** The tests `labels_are_plain` (Task 1),
    `effects_text_is_plain` (Task 4) and `refusals_are_plain` (Task 2) cover it.
- **Untrusted text.** Window titles and classes come from other apps. Rule names and patterns are
  user text that can arrive from `state.toml` or a restore point. All of it is shown through
  `UqLabel` (app) or `UqText` (shell), never through a stock control's `text:` or a
  `Kirigami.InlineMessage`.
  - **Ruling:** the page's messages ("Close and reopen …", the pick result, refusals that echo a
    pattern) use a plain-text `RuleMessage.qml`, built like slice B's `ProfileMessage.qml`.
  - **Ruling (F28), traps the lints don't see.** `Kirigami.Dialog.title`, `ToolTip.text` and
    `Kirigami.FormData.label` render through styled Labels. Dialog titles are fixed literals
    ("New rule", "Edit rule"), never a rule name. No ToolTip or FormData label is bound to data.
    Menu items whose text comes from data (template names) use `contentItem: UqLabel`.

## Review Focus

1. **Lua injection through a rule.** A name, pattern, monitor, workspace or tag can hold
   `"`, `\`, `]]`, newlines or control characters, or `") hl.exec_cmd("x`. Every string reaches
   `generated.lua` only through `lua_string`, and names never reach `generated.lua` at all (only
   `uq:<id>` does). Task 3 tests `hostile_strings_stay_inside_lua_strings`, and Task 2 tests
   `refuses_hostile_rules`.
2. **A pattern Hyprland can't use.** That covers `negative:x`, `[a&&b]`, an unclosed group, a
   257-char pattern, `(a*)*` with a huge repeat, `(?x)a`, `\<a`, `a{1001}` and `\p{Greek}`.
   Each is refused before any write, with a plain message that names the field. Task 2 tests `check_pattern_refuses_what_re2_would_not_take`.
3. **Hyprland rejects the rendered rules** (a renamed effect on 0.56.2, say). Then
   `generated.lua` and `state.toml` stay as they were, `Explain("rules")` reports `error` with
   Hyprland's text (on every rule row and on the `"rules"` section row), and the page shows it. Task 6 tests `rejected_rules_keep_previous_config`.
4. **Pick mode leaks or hangs.** These cases must hold:
   - no shell answers within 2 s;
   - the user presses Esc;
   - nothing happens for 20 s;
   - a second pick starts;
   - a forged `PickDone` arrives.

   The caller gets a status every time, within the stated time. The desktop lock is never held
   while waiting (a `SetSection` during a pick goes through). Only the clicked window's fields
   come back. Task 7 tests `pick_*`. The app never stays hidden: a failed `PickWindow` call still
   gives the page a pick status (F5, Task 9).
5. **Workspace rules and per-workspace layouts together.** `uqctl layout set 3 master` with a
   `rules.workspace` entry for `3` renders one merged call. A `rules` write never changes
   `layouts.workspaces`, and the reverse holds too. Task 3 tests
   `workspace_rule_merges_layout`, and Task 6 tests `layout_and_workspace_rule_writes_stay_apart`.

---

### Task 1: `uq_core::rules::hypr`: the Hyprland name table, and the regex dependency

**Files:**
- Create: `crates/uq-core/src/rules/mod.rs` (only `pub mod hypr;` for now) and
  `crates/uq-core/src/rules/hypr.rs`.
- Modify:
  - `Cargo.toml` (`[workspace.dependencies]` `regex = "1"` and `regex-syntax = "0.8"`; use the
    `regex-syntax` version already in `Cargo.lock`);
  - `crates/uq-core/Cargo.toml` (both, `.workspace = true`);
  - `crates/uq-core/src/lib.rs` (`pub mod rules;`);
  - `Cargo.lock`.

**Interfaces:**
- Produces (`uq_core::rules::hypr`). The file's header comment cites the two tags and the files
  listed in Global Constraints, with the commit hashes.
  - `pub enum Kind { Bool, Text, Pair, Float, Int, Choice(&'static [&'static str]) }`.
  - `pub enum When { Static, Dynamic }`.
  - `pub struct Field { pub uq: &'static str, pub lua: &'static str, pub kind: Kind, pub label: &'static str, pub min: Option<i64>, pub max: Option<i64> }`.
    `label` is the plain page label. `min`/`max` are the value range where one applies (pairs
    and numbers; floats use hundredths, so `opacity` is 10–100 and `ignore_alpha` 0–100). The
    validator (Task 2) and the page's `fields` catalogue (Task 9, F6) both read the ranges from
    here.
  - `pub struct Effect { pub field: Field, pub when: When }`.
  - Tables:
    - `pub const MATCH_KEYS: [Field; 11]` (F22). Each row maps uq → lua: `class` → `class`,
      `title`, `initial_class`, `initial_title`, `tag`, `xwayland`, `floating` → `float`,
      `fullscreen`, `pinned` → `pin`, `modal`, `workspace`.
    - `pub const LAYER_MATCH_KEYS: [Field; 1]` (`namespace`).
    - `pub const WINDOW_EFFECTS: [Effect; 20]`, the 11 static and 9 dynamic effects from Global
      Constraints. `move` is `Pair`, `size` is `Pair`, `opacity` is `Float`, `idle_inhibit` is
      `Choice(&IDLE_INHIBIT)`, and `workspace` is `Text`. `silent` is not a row: it is a
      modifier of `workspace`, rendered through `SILENT_SUFFIX`.
    - `pub const LAYER_EFFECTS: [Field; 4]`.
    - `pub const WORKSPACE_FIELDS: [Field; 6]`. `layout` isn't in this list; it comes from
      `layouts`.
    - `pub const IDLE_INHIBIT: [&str; 4] = ["none", "always", "focus", "fullscreen"]`.
    - `pub const LAYER_ANIMATIONS: [&str; 3] = ["fade", "slide", "popin"]`.
    - `pub const ROUNDING_MAX: u32 = 20`.
    - `pub const SILENT_SUFFIX: &str = " silent"`.
    - `pub const TAG_SET_PREFIX: &str = "+"`.
    - `pub const RULE_NAME_PREFIX: &str = "uq:"`.
  - Lookups: `pub fn match_key(uq: &str) -> Option<&'static Field>`,
    `pub fn window_effect(uq: &str) -> Option<&'static Effect>` and
    `pub fn is_static(uq: &str) -> bool`.

- [ ] **Step 1: Write the failing tests** in `hypr.rs`:
  - `tables_have_unique_names` (no `uq` or `lua` repeats within a table);
  - `spec_effect_lists_are_covered`. The spec's §5 static list equals the `Static` rows, the
    dynamic list equals the `Dynamic` rows, and the layer list equals `LAYER_EFFECTS`.
  - `spec_match_names_map_to_lua` (`floating` → `float`, `pinned` → `pin`, the others are the
    same; `modal` is present as the plan's one extra key);
  - `labels_are_plain`: no label holds `regex`, `xwayland`, `namespace`, `class` or `hl.`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core rules::hypr -- --test-threads=2`. Expected:
  FAIL (module missing).
- [ ] **Step 3:** Implement. Cite source file and tag in a comment above each table. The `regex`
  crate isn't in `Cargo.lock` or the local registry cache (F37): the first build fetches it
  through the proxy. Run `cargo update -j 2 -p regex` once (or `--precise` a 1.x that accepts
  `regex-syntax 0.8.11` and `regex-automata 0.4.18`) before any `--locked` command, and commit
  the lock change.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): one table of Hyprland rule names, checked against 0.56.2 and 0.57.0`

### Task 2: `uq_core::rules`: the domain model, validation, ids and patterns

**Files:**
- Modify:
  - `crates/uq-core/src/rules/mod.rs` (the model, `mod check;`);
  - `crates/uq-core/src/state.rs` (F21):
    - `#[serde(default)] pub rules: Rules` on `State`, beside `layouts` (its `Deserialize` is
      per entry, F1);
    - `State::section` and `State::set_section` dispatch `"rules"` as they dispatch `"layouts"`;
    - `State::parse_strict` calls `Rules::repaired`, after `repair_rules`;
    - `Default for State` gains `rules: Rules::default()`;
  - `crates/uq-core/src/settings/mod.rs`:
    - `SECTION_DOMAINS` gains `"rules"`, and becomes `[&str; 8]`;
    - `renders_section` gains `"rules"`;
  - `crates/uq-core/src/settings/tests.rs` (`SECTION_DOMAINS` list);
  - `crates/uq-core/src/history/labels.rs` (F11): the three `("rules", …)` label rows, so
    `every_key_a_summary_can_show_has_a_plain_label` passes from this task on;
  - `apps/uq-settings/src/registry.rs` (F11): `every_journal_domain_has_one_page` skips
    `"rules"` with the comment `// Task 10 adds the Window rules page and removes this skip.`
    (CI-only; keeps every commit green).
- Create: `crates/uq-core/src/rules/check.rs` (patterns and value checks) and
  `crates/uq-core/src/rules/tests.rs`.

**Interfaces:**
- Consumes: Task 1's tables (including `min`/`max`); `settings::{rule_selector,
  valid_output_name, round2, check_known, invalid}` (`check_known` and `invalid` are
  `pub(crate)`, fine inside uq-core).
- Produces (`uq_core::rules`):
  - Serde structs. Every `Option` is `skip_serializing_if = "Option::is_none"`. The entry
    structs (`WindowRule`, `WindowMatch`, `WindowEffects`, `LayerRule`, `LayerMatch`,
    `LayerEffects`, `WorkspaceRule`) are `#[serde(deny_unknown_fields)]`.
    - `pub struct Rules { pub window: Vec<WindowRule>, pub layer: Vec<LayerRule>, pub workspace: Vec<WorkspaceRule> }`
      (`Default`, `Serialize`, and a hand-written `Deserialize` that is lenient per entry: each
      list is read as a JSON array, each entry that parses is kept, and the others are dropped
      with `tracing::warn!`; a non-array list is empty; the pattern of
      `state.rs::lenient_rules`).
    - `pub struct WindowRule { pub id: String, pub name: String, pub enabled: bool, #[serde(rename = "match")] pub matches: WindowMatch, pub effects: WindowEffects }`.
    - `pub struct WindowMatch { class, title, initial_class, initial_title, tag, workspace: Option<String>; xwayland, floating, fullscreen, pinned, modal: Option<bool> }`.
    - `pub struct WindowEffects`, with:
      - `float, tile, fullscreen, maximize, center, pin, no_initial_focus, no_blur, no_shadow, no_anim, keep_aspect_ratio: Option<bool>`;
      - `workspace: Option<String>`, `silent: Option<bool>`, `monitor: Option<String>`;
      - `size: Option<[u32; 2]>`, `#[serde(rename = "move")] position: Option<[i32; 2]>`;
      - `opacity: Option<f64>`, `rounding: Option<u32>`, `border_size: Option<u32>`;
      - `idle_inhibit: Option<String>`, `tag: Option<String>`.
    - `pub struct LayerRule { id, name, enabled, #[serde(rename = "match")] matches: LayerMatch { namespace: String }, effects: LayerEffects { blur, no_anim: Option<bool>, ignore_alpha: Option<f64>, animation: Option<String> } }`.
    - `pub struct WorkspaceRule { pub workspace: String, pub monitor: Option<String>, pub default: Option<bool>, pub persistent: Option<bool>, pub gaps_in: Option<u32>, pub gaps_out: Option<u32>, pub border_size: Option<u32> }`.
  - Constants: `MAX_WINDOW_RULES = 200`, `MAX_LAYER_RULES = 50`, `MAX_WORKSPACE_RULES = 64`,
    `MAX_NAME_CHARS = 100` and `MAX_PATTERN_CHARS = 256`.
  - `impl Rules`:
    - `from_section(v: &Value, current: &Rules) -> Result<Rules, SettingError>`. It's strict:
      `check_known` on every object (the window-effects list is the table's `uq` names plus
      `silent`, F22), typed values, every limit, range and contradiction in Global
      Constraints, every pattern through `check_pattern`, and tag names (no `uq_` prefix, F33).
      Ids (F2): a supplied id must pass `valid_id` and be unique within the payload, across
      window and layer rules; a missing or `""` id is filled with `fresh_id`, which avoids ids in
      `current` and in the payload. It rounds `opacity` and `ignore_alpha`, and normalises
      selectors with `rule_selector`, re-keying its errors to the rule's field (F20).
    - `repaired(self) -> Rules` (load time). It drops invalid, duplicate-id or over-limit
      entries with a log line, so the section stays writable.
    - `ids(&self) -> Vec<&str>`.
  - `pub fn fresh_id(taken: &BTreeSet<String>) -> String` (`r-` + 4 hex from `RandomState`, as
    `history::restore::random_suffix`, retrying on a collision).
  - `pub fn valid_id(id: &str) -> bool`.
  - `pub fn check_pattern(field: &str, p: &str) -> Result<(), SettingError>`, with the checks and
    the fixed plain reasons of Global Constraints (F3, F4).
  - `pub fn exact_pattern(s: &str) -> String` = `format!("^{}$", regex::escape(s))`.
  - `SettingError::Invalid` keys name the rule by position and field, for example
    `rules.window[3].match.class`. Reasons are plain: "this pattern can't be used: …",
    "a rule needs at least one thing to match", "a rule needs at least one effect", "Float and
    Tile can't both be on", "at most 200 window rules". No reason holds the regex crate's text.

- [ ] **Step 1: Write the failing tests** in `rules/tests.rs`:
  - `spec_example_parses`: the §5 TOML through `State::parse_strict`, with the spec's regex. It
    gives one window, one layer and one workspace rule, and `section("rules")` round-trips through
    `set_section`.
  - `missing_ids_are_assigned_and_unique`: three new rules get three distinct valid ids, and
    supplied ids are kept.
  - `resending_the_section_is_accepted` (F2): `set_section("rules", section("rules"))` on a state
    with rules succeeds and changes nothing; so does writing back a section with one rule
    edited.
  - `refuses_bad_or_duplicate_ids` (`"x"`, `"r-ZZZZ"`, the same id on a window and a layer rule
    in one payload).
  - `check_pattern_refuses_what_re2_would_not_take` (Review Focus 2): `negative:foo`, `[a&&b]`,
    `[a--b]`, `(`, 257 chars, `"a\u{7}"`, a pattern over the size limit (`"(?:a{1000}){1000}"`),
    `(?x)a b`, `\<word`, `\b{start}a`, `a{1001}`, `a{2,1001}` and `\p{Greek}`. Each is an `Err`
    with the field in the key. `^(org\.pulseaudio\.pavucontrol)$`, `firefox`, `(?i)fire.*`,
    `a{1000}` and `.*\.pdf - Viewer` are `Ok`.
  - `refusals_are_plain` (F4): for every refused pattern above, the reason has no `\n` and no
    "regex", and doesn't repeat the pattern.
  - `needs_a_match_and_an_effect`;
  - `refuses_contradictions` (float + tile, move + center, silent without workspace);
  - `ranges` (opacity 0.05 and 1.2, rounding 21, border 21, size `[0, 10]`, move 20000,
    `ignore_alpha` 1.5, gaps 101 are refused; opacity 0.333 → 0.33);
  - `closed_values` (`idle_inhibit = "sometimes"` and `animation = "spin"` are refused; tags
    `"a b"` and `"uq_ws_3"` are refused, as match and as effect);
  - `limits` (201 window, 51 layer, 65 workspace rules, a 101-char name, a name with `\n`);
  - `refuses_hostile_rules`: unknown keys at every level (`match.bogus`, `effects.exec`,
    `rules.bogus`), and wrong types (`enabled = "yes"`, `size = [1]`).
  - `workspace_rule_selectors_normalised_and_unique` (`"web"` → `name:web`; `"3"` twice is
    refused; a 65-char selector's error key is `rules.workspace[0].workspace`, not
    `layouts.workspaces`).
  - `lenient_load_drops_only_the_bad_rule` (F1): a `state.toml` with one rule that has
    `effects.bogus`, one with `enabled = "yes"` and one good rule loads only the good one, and
    the theme and pins are untouched.
  - `exact_pattern_escapes` (`org.kde.dolphin` → `^org\.kde\.dolphin$`).
  - In `settings/tests.rs`: update the `SECTION_DOMAINS` assertion, and assert
    `renders_section("rules")`.
  - In `history/labels.rs`: `populated()` gains a state with one rule of each kind, so the
    existing `every_key_a_summary_can_show_has_a_plain_label` checks the three new rows.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core rules -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Use `regex_syntax::ast::parse::Parser::new().parse(p)` and a
  `regex_syntax::ast::Visitor` that refuses `ClassSet::BinaryOp`, flags other than `i m s U`,
  word-boundary assertions other than `\b`/`\B`, `RepetitionRange` bounds over 1000 and
  `ClassUnicode`. Then compile with `regex::RegexBuilder` and `size_limit(1 << 18)` and
  `dfa_size_limit(1 << 18)`. Map `ast::ErrorKind` and `regex::Error::CompiledTooBig` to the fixed
  reasons.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS. Fix any
  other existing test that enumerated domains (`profile`).
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): rules domain with checked patterns, limits and stable ids`

### Task 3: Render rules into `generated.lua`

**Files:**
- Create: `crates/uq-core/src/lua/rules.rs`.
- Modify:
  - `crates/uq-core/src/lua/mod.rs`:
    - `mod rules; pub use rules::{emit_window_rule, emit_layer_rule, emit_workspace_rules};`;
    - **keep** `pub fn emit_workspace_rule(ws, layout)`: uqd's `SetLayout` evals it live
      (`crates/uqd/src/service.rs:1031`), and Hyprland's `replaceOrAdd` merge keeps the other
      fields (F12). `emit_workspace_rules` shares its field formatting.
    - `emit_generated` takes a new `rules: &Rules` parameter (rules live in `State`, F21):
      `emit_generated(theme, layouts, rules, settings)`. It replaces the
      `-- Per-workspace layouts` loop with `emit_workspace_rules(layouts, &rules.workspace)` and
      appends the window and panel blocks at the end.
    - update its callers: `lua/input.rs` and `lua/look.rs` tests,
      `crates/uq-core/tests/generated_golden.rs`, `crates/uqd/src/service.rs`
      (`render_candidate`, passes `&next.rules`) and `crates/uqd/tests/verify_config.rs`.
  - `crates/uq-core/tests/golden/generated/default.lua` and `all-set.lua` (F12): re-bless with
    `UQ_BLESS=1` (the comment becomes `-- Workspace rules`). `generated_golden.rs` passes a
    `Rules` with one workspace rule for `all-set`, so the merged call is in the golden file.
  - `crates/uqd/tests/verify_config.rs`:
    - the mock's must-call list gains nothing (rules are optional). The mock already proxies
      every `hl.*` call. `MOCK` now prints each recorded call on its own line after `ok <n>`
      (F19);
    - a new fixture `rules()` has one window rule using **every** effect (split across rules
      where they contradict), one layer rule with every effect, and one workspace rule with
      every field;
    - `lua_mock_runs_every_variant` and `all_variants_pass_hyprland_verify` also run with it.

**Interfaces:**
- Consumes: Task 1's tables (every Lua key comes from them), Task 2's model, and `lua_string` and
  `workspace_selector`.
- Produces:
  - `pub fn emit_window_rule(r: &WindowRule) -> String`. It's one line:
    `hl.window_rule({ name = "uq:<id>", enabled = <bool>, match = { <k> = <v>, … }, <effect> = <v>, … })`.
    - Keys come out in table order. Strings use `lua_string`, bools are `true`/`false`, and
      pairs are `{ a, b }`.
    - `workspace` + `silent` → `"<sel> silent"`, `opacity` → `"0.90"`, and `tag` → `"+name"`.
    - The rule's `name` never reaches the output; only `uq:<id>` does.
  - `pub fn emit_layer_rule(r: &LayerRule) -> String`, the same way.
  - `pub fn emit_workspace_rules(layouts: &BTreeMap<String, Layout>, rules: &[WorkspaceRule]) -> String`.
    It emits one `hl.workspace_rule({ workspace = …, layout = …, monitor = …, … })` per
    selector, in the order from Global Constraints, under the `-- Workspace rules` comment. It's
    empty when there's nothing. A layout-only selector renders exactly as
    `emit_workspace_rule` does today.
  - The blocks are `\n-- Window rules\n…` and `\n-- Panel rules\n…`, each only when non-empty.

- [ ] **Step 1: Write the failing tests** in `lua/rules.rs`:
  - `golden_window_rule`: the spec's Pavucontrol rule renders exactly
    `hl.window_rule({ name = "uq:r-7f2c", enabled = true, match = { class = "^(org\\.pulseaudio\\.pavucontrol)$" }, float = true, size = { 800, 600 }, center = true })`.
  - `golden_layer_rule` (the spec's noti-fi rule, `ignore_alpha = 0.3`);
  - `match_names_use_lua_keys` (`floating` → `float = true`, `pinned` → `pin = false`);
  - `silent_workspace_and_tag_and_opacity_forms`;
  - `disabled_rule_renders_enabled_false`;
  - `rules_keep_list_order`: three rules render in list order, after the theme block and the
    `hl.env` lines.
  - `workspace_rule_merges_layout` (Review Focus 5): layouts `{"3": master, "1": dwindle}` and
    rules `[{workspace: "3", persistent: true}]` give exactly two calls, `3` first with
    `layout = "master"` and `persistent = true`, then `1`, which equals
    `emit_workspace_rule("1", Dwindle)`.
  - `hostile_strings_stay_inside_lua_strings` (Review Focus 1). A pattern, monitor and tag hold
    `"`, `\`, `]]`, `\n` and `") hl.exec_cmd("x` (building the rules directly, bypassing
    validation). Each rule is exactly one output line starting with `hl.`, and every value
    round-trips through a small Lua-string unescaper in the test. A hostile *name* doesn't
    appear anywhere in the output.
  - In `crates/uqd/tests/verify_config.rs`, `hostile_rules_run_as_one_call_each` (F19) runs the
    same rules through the Lua mock and checks the printed calls: the number of
    `hl.window_rule` and `hl.layer_rule` calls equals the number of rules, and no call carries
    the payload (no `hl.exec_cmd:x`). base.lua's own `hl.exec_cmd` hook calls are expected and
    not counted. Like `lua_mock_runs_every_variant`, it skips without `lua` on `PATH` (CI
    installs it).
  - `emits_rules_sorted` in `lua/mod.rs` keeps passing unchanged (it checks only the rule
    lines; the comment is pinned by the golden files).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core lua -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement, then re-bless: `UQ_BLESS=1 cargo test -j 2 -p uq-core --test generated_golden -- --test-threads=2`,
  and read the diff of the two golden files (only the comment line and the merged call change).
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`,
  `cargo test -j 2 -p uqd --test verify_config -- --test-threads=2` and
  `cargo build -j 2 -p uqd`. Expected: PASS. The `--ignored` Hyprland run is CI's gate for the
  names (Global Constraints). If it fails, fix the row in `rules/hypr.rs` or drop the effect,
  and say so in the PR.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): render window, panel and workspace rules in list order`

### Task 4: `uq_core::rules`: the matcher, `window_at` and templates

**Files:**
- Create: `crates/uq-core/src/rules/matcher.rs` and `crates/uq-core/src/rules/templates.rs`.
- Modify:
  - `crates/uq-core/src/rules/mod.rs` (modules and re-exports);
  - `crates/uq-core/src/settings/monitors.rs` and `settings/mod.rs` (F15): add
    `pub fn clean_text_spaced(s: &str, max: usize) -> String` beside `clean_text` (control
    characters become spaces), and re-export it.

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)] pub struct WindowInfo { pub class: String, pub title: String, pub initial_class: String, pub initial_title: String, pub xwayland: bool, pub floating: bool, pub fullscreen: bool, pub pinned: bool, pub workspace_id: i64, pub workspace_name: String, pub tags: Vec<String> }`
    (F10). It's the pick reply's and `ListWindows`' row.
  - `pub fn windows_from_clients(json: &str) -> Vec<WindowInfo>`. It maps `initialClass` →
    `initial_class` and `workspace{id,name}` → `workspace_id`/`workspace_name`, takes
    `fullscreen` int > 0 as `true`, keeps only `mapped` windows, and caps the list at 500.
    Strings go through `settings::clean_text_spaced(s, 512)` (F15).
  - `#[derive(Serialize, …)] #[serde(rename_all = "snake_case")] pub enum Verdict { Yes, No, IfDialog, Unknown }`
    (F10, F31): `yes`, `no`, `if_dialog`, `unknown`.
  - `pub struct Compiled` (a window rule's match, compiled once) with
    `Compiled::new(&WindowMatch) -> Result<Compiled, SettingError>` (patterns through
    `check_pattern` first) and `fn test(&self, w: &WindowInfo) -> Verdict`. It's a full match on
    patterns. `workspace` (F10): a numeric selector compares with `workspace_id`; `name:x`
    compares `x` with `workspace_name`; `special` and `special:x` compare with
    `workspace_name`; any selector holding `[` gives `Unknown` (shown as "Can't tell from
    here"). `modal` gives `IfDialog` when everything else matches. `Unknown` wins over
    `IfDialog` when both apply; `No` wins over both.
  - `pub fn matching(rules: &[WindowRule], w: &WindowInfo) -> Vec<(usize, Verdict)>`, in list
    order, with disabled rules included and `No` left out. The caller greys disabled ones.
  - `pub fn window_at(clients_json: &str, monitors_json: &str, cursor_json: &str) -> Option<WindowInfo>`:
    - monitors with `"disabled": true` are skipped (`j/monitors all` lists them, F32);
    - the visible workspaces are each remaining monitor's `activeWorkspace.id` and
      `specialWorkspace.id` (when non-zero);
    - candidates are mapped, not-hidden windows on a visible workspace (or pinned) whose
      `at`/`size` box holds the cursor;
    - the order is: special-workspace windows first, then fullscreen, then floating by ascending
      `focusHistoryID`, then tiled by ascending `focusHistoryID`.
  - `pub fn effects_text(e: &WindowEffects) -> Vec<String>` gives plain lines ("Floats",
    "Opens on workspace 3, without switching to it", "See-through: 90 %", "Keeps the screen
    awake while fullscreen"). `layer_effects_text`, `workspace_text` and `match_text` do the
    same for their parts.
  - `pub fn has_static(e: &WindowEffects) -> bool` uses `hypr::is_static`.
  - `templates.rs`:
    - `#[derive(Serialize)] #[serde(rename_all = "snake_case")] pub enum Needs { App, AppAndWorkspace, Nothing }`
      (`app`, `app_and_workspace`, `nothing`; F31);
    - `pub struct Template { pub id: &'static str, pub name: &'static str, pub needs: Needs }`;
    - `pub const TEMPLATES: [Template; 5]` (Global Constraints);
    - `pub fn build(id: &str, app: Option<&str>, workspace: Option<&str>, silent: bool) -> Result<WindowRule, SettingError>`.
      It gives a rule with `id = ""`, `match.class = exact_pattern(app)` where an app is needed,
      and a plain default name ("Pavucontrol dialogs float"), taken from the last dot-segment of
      the app ID, capitalised.

- [ ] **Step 1: Write the failing tests:**
  - In `matcher.rs`:
    - `full_match_semantics`: `firefox` doesn't match `firefox-esr`, `fire.*` does, and `^`/`$`
      are harmless.
    - `every_match_field` (one case each, including `floating`/`pinned`/`xwayland`);
    - `workspace_match_forms` (F10): `"3"` matches id 3; `"name:web"` matches a window on
      workspace id 5 named `web` and not one named `webby`; `"special:magic"` matches name
      `special:magic`; `"w[tv1]"` gives `Unknown`;
    - `modal_gives_if_dialog`;
    - `matching_keeps_order_and_disabled`;
    - `verdict_serialises_snake_case` (`IfDialog` → `"if_dialog"`, `Unknown` → `"unknown"`);
    - `windows_from_clients_maps_and_cleans` (a fixture captured from `HyprCtl.cpp`'s format:
      `initialClass`, `workspace: {id: 5, name: "web"}`, `fullscreen: 2`, an unmapped window
      dropped, a title with `\u{1b}[2J` cleaned to `" [2J"`);
    - `window_at_prefers_floating_over_tiled`, `window_at_ignores_other_workspaces`,
      `window_at_sees_special_workspace`, `window_at_none_on_empty_desktop`,
      `window_at_skips_disabled_monitors` (a disabled monitor whose stale `activeWorkspace`
      would otherwise make a hidden window a candidate);
    - `effects_text_is_plain` (no Lua key appears in any line).
  - In `templates.rs`: `each_template_builds_a_valid_rule` (every result passes
    `Rules::from_section`), `templates_need_their_inputs`, `pip_template_needs_no_app` and
    `app_templates_use_exact_class` (`org.pulseaudio.pavucontrol` → `^org\.pulseaudio\.pavucontrol$`).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core rules -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): match windows the way Hyprland does, find the clicked window, rule templates`

### Task 5: History summaries, the `user.lua` finders and `rules::explain`; profiles still refuse rules

**Files:**
- Create: `crates/uq-core/src/rules/summary.rs` and `crates/uq-core/src/rules/explain.rs`.
- Modify:
  - `crates/uq-core/src/rules/mod.rs` (`mod summary; mod explain;` and their re-exports; F23);
  - `crates/uq-core/src/history/diff.rs` (`summarize` delegates `rules`);
  - `crates/uq-core/src/history/labels.rs` (test `rules_summary_uses_names`; the label rows
    landed in Task 2);
  - `crates/uq-core/src/settings/provenance.rs`:
    - factor the existing `--` cut of `find_in_user_lua` into
      `fn strip_lua_comment(line: &str) -> &str` (F8), used by all three finders;
    - `pub fn find_rule_in_user_lua(text: &str, id: &str) -> Option<u32>` and
      `pub fn find_workspace_rule_in_user_lua(text: &str, selector: &str) -> Option<u32>`;
  - `crates/uq-core/src/profile/overlay.rs` (tests only; F25).

**Interfaces:**
- Produces:
  - `pub fn summary(before: &Value, after: &Value) -> String` (`rules::summary`). It pairs
    rules by id (workspace rules by selector) and classifies each change as added, removed,
    turned on, turned off or changed. A list whose ids are the same but in a different order is
    "reordered". It caps at 3, then ", and N more", with the wording from Global Constraints.
    Names are cut to 40 chars with "…". A `name:` selector shows without the prefix (F34).
  - Both `user.lua` finders give 1-based line numbers. Each line is first cut at `--` with
    `strip_lua_comment`, as `find_in_user_lua` does (F8). `find_rule_in_user_lua` looks for
    `uq:<id>` followed by `"`, `'` or `]`. `find_workspace_rule_in_user_lua` looks for a line
    holding `workspace_rule` and the selector in double or single quotes.
  - `pub fn explain(rules: &Rules, user_lua: Option<&str>, rejected: Option<&str>, user_lua_path: &str) -> Value`
    (`rules::explain`, F7). It gives the rows of Global Constraints, "Sources and history":
    `window.<id>`, `layer.<id>`, `workspace.<selector>`, each
    `{source, live: null, stored: null, detail}`, and, when `rejected` is `Some`, the section
    row `"rules"` (F9). With `rejected`, every row's source is `error` with `{message}`;
    otherwise `user_lua` with `{line, path}` when a finder hits, else `uq`.

- [ ] **Step 1: Write the failing tests:**
  - In `summary.rs`:
    - `added_removed_changed_toggled`;
    - `reorder_only`;
    - `caps_at_three`;
    - `named_workspace_reads_plain` (`name:web` → "Workspace web rule changed");
    - `never_prints_json` (F27): a deterministic enumeration, with no randomness: every template
      × {added, removed, turned on, turned off, changed, reordered}, with rule names that
      contain `{`, `[` and `"`. Each summary holds the name text but never `"match"`,
      `"effects"`, `"id"` or a JSON object or array of the rule.
  - In `labels.rs`, add `rules_summary_uses_names`.
  - In `provenance.rs`:
    - `finds_rule_name_in_user_lua`: the line with `uq:r-7f2c"` is found; a commented-out line
      and a trailing comment (`x = 1 -- uq:r-7f2c"`) are skipped; `'uq:r-7f2c'` is found; and
      `uq:r-7f2c` doesn't match a line with `uq:r-7f2c0"`, because the id must be followed by a
      quote or `]`;
    - `finds_workspace_rule_line`;
    - the existing `find_in_user_lua` tests still pass after the `strip_lua_comment` refactor.
  - In `explain.rs`:
    - `rows_per_rule_and_kind`;
    - `user_lua_override_has_line_and_path`;
    - `rejected_adds_a_section_row_even_with_no_rules` (F9): empty `Rules` and
      `rejected = Some("bad")` give exactly `{"rules": {source: "error", detail: {message: "bad"}, …}}`.
  - In `profile/overlay.rs`: add `"rules"` to the loop of the existing `from_journal_maps_domains`
    (`from_journal("rules", …)` is `None`). `refuses_reserved_domains_for_now` already pins the
    reservation and is left alone (F25).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): plain history lines for rule changes, rule overrides in user.lua; profiles keep refusing rules`

### Task 6: uqd: the `rules` section, `Explain("rules")`, `ListWindows` and the compositor reads

**Files:**
- Create: `crates/uqd/src/rules.rs` (`impl Desktop`, as `profiles.rs` does) and
  `crates/uqd/src/rules/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`mod rules;`);
  - `crates/uqd/src/compositor.rs`:
    - the trait gains `async fn clients(&self) -> Result<String, CompositorError>` (`j/clients`)
      and `async fn cursor_pos(&self) -> Result<String, CompositorError>` (`j/cursorpos`);
    - `FakeCompositor` gains `clients_json` and `cursor_json` (empty means `[]` and
      `{"x":0,"y":0}`), both recorded in `calls`;
  - `crates/uqd/src/service.rs` (F16):
    - `last_rejected` and `fn read_user_lua` become `pub(crate)`;
    - `explain` hands `"rules"` to `Desktop::explain_rules` right after the `SECTION_DOMAINS`
      check, before `provenance::options_for`;
    - `domain_label` gains `"rules" => "Window rules"`;
  - `crates/uqd/src/dbus.rs` (`list_windows() → s`);
  - `crates/uq-core/src/client.rs` (`DesktopProxy::list_windows`);
  - `crates/uqd/tests/dbus.rs` (F23).

**Interfaces:**
- Consumes: Tasks 2–5 (`rules::explain`, `windows_from_clients`), `commit_section`,
  `note_section_commit`, `last_rejected` and `read_user_lua`.
- Produces:
  - `SetSection("rules", json, origin)` works with no new code beyond Task 2's
    `State::set_section`: `commit_section` already renders for every `renders_section` domain.
    The tests pin it.
  - `pub(crate) fn explain_rules(&self) -> serde_json::Value`: reads `user.lua` once with
    `read_user_lua` and calls `rules::explain(&self.state.rules, text, rejected, path)`.
  - `pub async fn list_windows(&self) -> Result<Vec<WindowInfo>, DesktopError>`. Hyprland down is
    `Ok(vec![])`.
  - D-Bus `ListWindows() → s` gives a JSON array.

- [ ] **Step 1: Write the failing tests** in `rules/tests.rs`, with `FakeCompositor`:
  - `set_rules_section_renders_verifies_and_journals`: the verified `generated.lua` contains the
    rule line, a `reload` is called, the journal entry's domain is `rules` with origin
    `settings`, and its summary is `Window rule “Pavucontrol floats” added`.
  - `ids_are_assigned_by_uqd` (`GetSection` after a write without ids has valid ids);
  - `resending_get_section_is_accepted` (F2);
  - `rejected_rules_keep_previous_config` (Review Focus 3): seed one rule first, then set
    `verify_error` and write a second one. `state.toml`, `generated.lua` and `GetSection` are
    unchanged, and `explain("rules")` has `window.<seeded id>` and `"rules"` with
    `source == "error"` and the message.
  - `rejected_first_rule_still_explained` (F9): with no rules, a rejected write leaves
    `explain("rules")["rules"].source == "error"`.
  - `undo_restores_the_rule_list` (add, then `undo(entry)`, gives no rules) and
    `restore_point_brings_rules_back`;
  - `explain_flags_user_lua_override` (a `user.lua` with `hl.window_rule({ name = "uq:<id>", enabled = false })`
    → `user_lua`, line 1, path);
  - `explain_workspace_rule_in_user_lua`;
  - `layout_and_workspace_rule_writes_stay_apart` (Review Focus 5): `set_layout("3", master)`
    leaves `state.rules.workspace` alone, and a `rules` write leaves `layouts` alone. The rendered
    file has one merged `workspace = "3"` call.
  - `rules_not_in_settings_changed` (F21): the `SettingsChanged` event after a `rules` write
    holds no `rules` key.
  - `list_windows_maps_clients` and `list_windows_empty_when_hyprland_down`;
  - `invalid_pattern_is_refused_before_render` (no `verify` call is recorded).
  - In `compositor.rs` socket tests: `clients_and_cursorpos_requests` (the fake socket sees
    `j/clients` and `j/cursorpos`).
  - In `crates/uqd/tests/dbus.rs`: `rules_section_and_list_windows_over_dbus`, and extend
    `desktop_proxy_matches_the_interface` with `list_windows` and `explain("rules")`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd rules -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `service.rs` growth to the two visibility changes, the
  `explain` hook and `domain_label`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS, including every
  slice A and B test.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): window rules as a verified section, explained against user.lua`

### Task 7: uqd: pick mode (`PickWindow`, `PickDone`, `PickRequested`)

**Files:**
- Create: `crates/uqd/src/pick.rs` (with its tests).
- Modify:
  - `crates/uqd/src/lib.rs` (`pub mod pick;`);
  - `crates/uqd/src/service.rs` (F13): `Event::PickRequested(String)`;
  - `crates/uqd/src/monitors.rs` (F14): `new_token` becomes `pub(crate)`;
  - `crates/uqd/src/dbus.rs`: the broker lives on `DesktopIface` beside `SharedDesktop`, not in
    `Desktop`. Add `pick_window`, `pick_done`, the `pick_requested` signal, and the
    `Event::PickRequested(token) => DesktopIface::pick_requested(e, token)` arm in `serve()`'s
    match.
  - `crates/uq-core/src/client.rs` (`pick_window`, `pick_done` and the `pick_requested` signal,
    so `receive_pick_requested` exists);
  - `crates/uqd/tests/dbus.rs`.

**Interfaces:**
- Produces (`crate::pick`):
  - `pub const PICK_ACK_TIMEOUT: Duration = 2 s` and `pub const PICK_TIMEOUT: Duration = 20 s`.
  - `pub enum Outcome { Shown, Click, Cancel }`, parsed from `"shown" | "click" | "cancel"`.
    Anything else is `InvalidArgument`.
  - `pub struct PickBroker` (`Default`) with:
    - `begin(&self) -> Result<(String, Waiter), DesktopError>`, where the token comes from
      `crate::monitors::new_token()` (32 lowercase hex) and a second `begin` while pending is
      `Conflict("Already choosing a window.")`;
    - `done(&self, token: &str, o: Outcome)`, which ignores unknown tokens;
    - `with_timeouts(ack, total)` for tests.
  - `pub async fn pick(broker: &PickBroker, comp: Arc<dyn Compositor>, emit: impl Fn(String)) -> String`.
    It returns the reply JSON from Global Constraints.
    - `pick_window` takes the desktop lock only to clone `comp` and `desktop.events` (both
      `pub(crate)`), then releases it. `emit` is `move |t| { let _ = events.send(Event::PickRequested(t)); }`,
      a sync broadcast send that `serve()` turns into the signal (F13).
    - Order: if `comp.clients()` fails with `NotRunning`, answer `unavailable` without emitting.
      Otherwise `broker.begin()` first, then `emit(token)`, so a fast `PickDone("shown")` always
      finds the token.
    - On `click`, the window comes from `rules::window_at(clients, monitors, cursor)`, reading
      `comp.monitors()` (`j/monitors all`), `comp.clients()` and `comp.cursor_pos()`.
  - D-Bus: `PickWindow() → s`, `PickDone(s token, s outcome)` and the signal
    `PickRequested(s token)`.

- [ ] **Step 1: Write the failing tests:**
  - In `pick.rs` (with a fake emitter that records tokens and a scripted responder task):
    - `pick_click_returns_only_the_clicked_window`: the fixture has three clients, and the
      cursor is over the floating one. The reply has that window's fields only, and its JSON has
      no `address` or `pid` key.
    - `pick_click_on_empty_desktop_is_none`;
    - `pick_escape_is_cancelled`;
    - `pick_without_shell_is_no_shell_within_ack_timeout` (ack 50 ms, finishes under 500 ms);
    - `pick_times_out` (total 100 ms);
    - `pick_second_request_conflicts`;
    - `pick_forged_token_is_ignored`;
    - `pick_hyprland_down_is_unavailable` (nothing is emitted);
    - `pick_token_is_32_hex`.
  - In `crates/uqd/tests/dbus.rs`:
    - `pick_over_dbus`: a client task listens for `PickRequested` and calls `PickDone`
      `shown` then `click`, and the `PickWindow` caller gets `picked`.
    - `pick_does_not_hold_the_desktop_lock` (Review Focus 4): while a pick waits, `SetSection("look", …)`
      from another proxy returns.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd pick -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement with `tokio::sync::oneshot` and `tokio::time::timeout`. Don't add a
  `rand` dependency, and don't write a second token generator.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): pick a window by clicking it, returning only that window's details`

### Task 8: The bridge and the shell's pick overlay

**Files:**
- Create: `shell/pick/PickOverlay.qml`, `shell/logic/pick.js` and `tests/qml/tst_pick.qml`.
- Modify:
  - `crates/uqctl/src/bridge/uqd.rs` (the `PickDone(String, String)` request with arity and type
    checks; `forward` gains `"PickRequested" => {"token": …}`);
  - `crates/uqctl/src/bridge/mod.rs` (`"PickDone"` in `handle`'s pass-through list);
  - `crates/uqctl/tests/bridge_dbus.rs`;
  - `shell/core/Bridge.qml`
    (`signal pickRequested(string token)` and `case "PickRequested": if (data) pickRequested(String(data.token)); break;`);
  - `shell/shell.qml` (`import "pick"` and `PickOverlay {}`; F23);
  - `tests/qml/tst_bridge_protocol.qml` (a `PickRequested` event case).

**Interfaces:**
- Consumes: Task 7's signal and `PickDone`.
- Produces:
  - `pick.js` (pure, no Quickshell imports):
    - `validToken(t)` is `/^[0-9a-f]{32}$/` (what `new_token` makes);
    - `start(state, token, locked)` → `{state, reply}`. When locked, `reply` is
      `["cancel"]` and the overlay doesn't show. Otherwise it's `["shown"]`, the overlay is
      visible, and any earlier token is replaced.
    - `click(state)` / `escape(state)` / `timeout(state)` → `{state: idle, reply}`, sent once;
      repeats give `reply: null`.
    - `PICK_TIMEOUT_MS = 20000`.
  - `PickOverlay.qml`:
    - one `PanelWindow` per screen (`Variants` over `Quickshell.screens`) on the overlay layer,
      with an exclusive zone of -1 and exclusive keyboard focus while active;
    - a translucent scrim and a centred `UqText`: "Click a window to choose it. Press Esc to
      cancel.";
    - a crosshair cursor, a `MouseArea` for the click, `Keys.onEscapePressed`, and a 20 s
      `Timer`;
    - it calls `Bridge.call("PickDone", [token, outcome])` and ignores errors;
    - it hides on lock (sending `cancel`) and on `UqdDown`.

- [ ] **Step 1: Write the failing tests:**
  - In `bridge_dbus.rs`, `bridge_forwards_pick_requests_and_done`. A direct `PickWindow` call
    runs in a task. The bridge emits `PickRequested {token}`, the test sends
    `PickDone [token, "shown"]` and `PickDone [token, "cancel"]` through the bridge, and the
    caller gets `cancelled`.
  - In `bridge/uqd.rs` tests, `pick_done_checks_arity` (`[]`, `["t"]`, `["t", 1]` and
    `["t", "x", "y"]` are refused).
  - In `tst_pick.qml`:
    - `test_bad_token_ignored`;
    - `test_locked_cancels`;
    - `test_click_replies_once`;
    - `test_escape_and_timeout`;
    - `test_new_token_replaces_old`.
  - In `tst_bridge_protocol.qml`: `PickRequested` with a token emits `pickRequested`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqctl --test bridge_dbus -- --test-threads=2` and
  `cargo test -j 2 -p uqctl bridge -- --test-threads=2`. Expected: FAIL. The QML suite runs in
  CI (K4).
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqctl -- --test-threads=2` and `scripts/lint-shell.sh`.
  Expected: PASS. Where Qt and Quickshell are installed, also run
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml` and
  `tests/shell/bar_loads.sh`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(shell): full-screen window picker for the rules editor`

### Task 9: uq-settings: the rules page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/rules.rs`;
  - `apps/uq-settings/src/pages/rules/tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/windows.rs`, a `FakeWindows` with a window list, a
    queue of scripted pick replies, and a switch that makes `PickWindow` fail with `Conflict`.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod rules;`);
  - `apps/uq-settings/src/pages/fake_uqd.rs`:
    - `list_windows` and `pick_window`, delegating to `FakeWindows`;
    - `explain("rules")` calls `uq_core::rules::explain(&state.rules, None, rejected, "user.lua")`
      and then applies `Explain.overrides` keyed `rules.<row key>` (F7). `rejected` is the
      message of the last `rules` write the fake refused with `ConfigInvalid`;
    - the `rules` section goes through the existing generic section write, so ids are assigned
      by `State::set_section`;
    - `Rig.windows`.

**Interfaces:**
- Consumes:
  - Tasks 1, 2, 4 and 5: `hypr` tables, `Rules`, `Compiled`, `matching`, `effects_text`,
    `has_static`, `TEMPLATES`, `build`, `exact_pattern`, `check_pattern`, `rules::explain`;
  - `runner::{PageLogic, patch_section, parse_json}`;
  - `DesktopProxy::{get_section, set_section, explain}`, `list_windows` (Task 6) and
    `pick_window` (Task 7) (F23).
- Produces:
  - `#[derive(Debug, Clone)] pub enum Op`. `kind` is `"window" | "layer" | "workspace"`, and
    `key` is a rule id or a workspace selector.
    - `Refresh`, `RefreshWindows`;
    - `Save { kind: String, rule: Value }`: an add when `rule` has no `id` (or a workspace
      selector not yet present), else a replace;
    - `Delete { kind, key }`, `Move { kind, key, to: usize }`, `SetEnabled { kind, key, on: bool }`;
    - `FromTemplate { template: String, app: Option<String>, workspace: Option<String>, silent: bool }`;
    - `Pick { purpose: String }` (`"match"` or `"why"`).

    There is no `Preview` op (F17).
  - `pub struct RulesState` (a `PageLogic`), with `new(conn: zbus::Connection) -> Self`. Every
    write reads the section, edits it, and writes it with origin `settings` under the write lock.
    It keeps the cached window list behind a `std::sync::Mutex`.
  - `pub fn preview(&self, draft: &Value) -> Value` (F17): synchronous and pure; no D-Bus. It
    validates the draft with `Rules::from_section` (wrapped as a one-rule section) and runs
    `Compiled` against the cached windows. It returns `{valid, error, matches: [{title, class,
    verdict}]}`. A draft that fails gives `valid: false` and the plain reason.
  - `pub fn exact(text: &str) -> String` (F18) = `exact_pattern(text)`.
  - `pub fn fields() -> Value` (F6): the catalogue built from `rules::hypr`:
    `{match: [...], window_effects: [...], layer_effects: [...], workspace: [...]}`, each item
    `{key, label, kind, static, min, max, choices, lua}`. `kind` is `bool | text | pattern |
    pair | float | int | choice` (`pattern` for the four text match keys and `namespace`).
  - **The snapshot:**
    ```
    { window:    [{id, name, enabled, match_text: [..], effects_text: [..], static: bool,
                   source, line, matches_now: n}],
      layer:     [{id, name, enabled, namespace, effects_text, source, line}],
      workspace: [{workspace, layout, fields_text, source, line}],
      section:   {source, message} | null,
      windows:   [WindowInfo],
      why:       {seq, window: WindowInfo | null, rules: [{id, name, enabled, verdict, effects_text, source, line}]},
      pick:      {seq, purpose, status, window: WindowInfo | null, class_pattern: "" },
      reopen:    {seq, names: [..], more: n},
      templates: [{id, name, needs}],
      fields:    {match: [..], window_effects: [..], layer_effects: [..], workspace: [..]},
      limits:    {window: 200, layer: 50, workspace: 64, name: 100, pattern: 256},
      notice:    {seq, text} }
    ```
    - `source` and `line` come from `Explain("rules")`; `section` is its `"rules"` row (F9).
    - `layout` comes from `GetSection("layouts")`.
    - `pick.status` is the uqd status, and the page turns it into plain text.
      `pick.class_pattern` is `exact_pattern(window.class)` when a window was picked (F18).
    - **A failed `PickWindow` call (F5)** still records a pick: `status: "error"`
      (`"unavailable"` when uqd has no owner), `window: null`, and a bumped `seq`. The op then
      returns the error, so the banner shows uqd's message too.
    - `reopen` is set after a `Save` or `FromTemplate` whose rule `has_static` and matches open
      windows.
    - `windows` are refetched on `Refresh`, `RefreshWindows`, after a pick and after a write, and
      stored in the cache `preview` uses.
    - `matches_now` uses the cached list.

- [ ] **Step 1: Write the failing tests** in `pages/rules/tests.rs`, against the fake uqd:
  - `save_new_rule_gets_id_and_journals` (origin `settings`, the history summary names it);
  - `edit_move_toggle_delete_round_trip` (§9's round-trip rule: reopening the state gives the
    same lists);
  - `preview_lists_matching_windows_and_says_none` (three windows; `^firefox$` gives one, and
    `^nope$` gives `matches: []`);
  - `preview_with_bad_pattern_is_invalid_not_an_error` (`(?x)a` and `[a&&b]` give
    `valid: false` with a plain reason and no D-Bus call);
  - `pick_for_match_prefills_exact_class` (`pick.window.class` and
    `pick.class_pattern == "^org\\.pulseaudio\\.pavucontrol$"`);
  - `pick_cancel_and_no_shell_are_plain_statuses`;
  - `pick_failure_still_reports_a_status` (F5): with `FakeWindows` set to fail, `Op::Pick` gives
    an error **and** a snapshot whose `pick.seq` went up and `pick.status == "error"`;
  - `why_lists_matching_rules_in_order_with_override_flag` (two of three rules match, and one
    has `source: "user_lua"`, `line: 4`, set through `Explain.set("rules.window.<id>", …)`);
  - `rejected_write_shows_section_error` (F9): with `verify_error` set, a `Save` of the first
    rule gives `section.source == "error"`;
  - `static_effect_save_asks_to_reopen` (float on a matching open window gives `reopen.names`
    with its title) and `dynamic_only_save_does_not` (opacity);
  - `templates_build_and_save` (each of the five, with `app = "org.pulseaudio.pavucontrol"` and
    workspace `3` where needed);
  - `fields_come_from_the_table` (F6): `fields.match` has 11 items in `MATCH_KEYS` order, and
    every `window_effects` item's `lua` equals the table's;
  - `workspace_rule_shows_layout_from_layouts`;
  - `limits_refusal_shows_uqd_message` (the 201st window rule);
  - `uqd_down_gives_empty_state_without_an_error`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-settings rules -- --test-threads=2` where Qt is
  installed (else CI, K4). Expected: FAIL.
- [ ] **Step 3:** Implement. No `cxx_qt` or Qt types in `pages/rules.rs`.
- [ ] **Step 4:** Re-run. Expected: PASS. Also run
  `cargo clippy -j 2 -p uq-settings --all-targets -- -D warnings` where Qt is installed.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): window rules page logic with live preview and pick`

### Task 10: uq-settings: the Window rules page, its model and its registry entry

**Files:**
- Create:
  - `apps/uq-settings/src/rules_model.rs`;
  - `apps/uq-settings/qml/pages/RulesPage.qml`;
  - `apps/uq-settings/qml/components/RuleEditor.qml`;
  - `apps/uq-settings/qml/components/RuleMatchList.qml`;
  - `apps/uq-settings/qml/components/RuleMessage.qml`;
  - `apps/uq-settings/qml/components/AdvancedDetails.qml` (F29; the file the jargon lint
    already exempts);
  - `tests/settings-qml/tst_rule_editor.qml`, `tests/settings-qml/tst_rule_match_list.qml` and
    `tests/settings-qml/tst_rule_message.qml`, with a fixture of the `fields` catalogue.
- Modify:
  - `apps/uq-settings/build.rs` (the five QML files and `src/rules_model.rs`);
  - `apps/uq-settings/src/main.rs` (`mod rules_model;`);
  - `apps/uq-settings/src/registry.rs` (the page; remove Task 2's `rules` skip in
    `every_journal_domain_has_one_page`; update the two order tests, F24);
  - `scripts/settings-jargon.txt` (Global Constraints).

**Interfaces:**
- Consumes: Task 9's `RulesState`, `Op`, `preview`, `exact`, `fields` and the snapshot.
- Produces:
  - `RulesModel` (a `#[qml_element]`) has the properties `state_json`, `error` and `busy`. Its
    invokables are:
    - `refresh()`, `refresh_windows()`;
    - `save(kind, rule_json)`, `remove(kind, key)`, `move_to(kind, key, to)` and
      `set_enabled(kind, key, on)`;
    - `from_template(id, app, workspace, silent)`;
    - `preview(draft_json) → QString`: synchronous, returns `RulesState::preview` as JSON, no
      busy flag, never touches `error` (F17);
    - `exact(text) → QString` (F18);
    - `pick(purpose)`.

    Its `FOLLOW` is: sections `rules` and `layouts`, plus `history`. Like every page, it
    re-reads when the window becomes active (slice A's ruling standing in for
    `configreloaded`), which also covers the window coming back after a pick and live check 4
    (F35).
  - **Registry.** The page is `PageInfo { id: "rules", name: "Window rules", icon: "preferences-system-windows", category: Category::Uq, qml: "pages/RulesPage.qml", settings: &RULES_SETTINGS, domains: &["rules"] }`,
    placed after `profiles` and before `history` (F24). `RULES_SETTINGS` has:
    - `entry("window-rules", "Window rules", &["float", "workspace", "opacity", "app", "window", "rule", "pin"])`;
    - `entry("pick-window", "Pick a window", &["click", "choose", "which window"])`;
    - `entry("why-window", "Why does this window…?", &["explain", "matches", "rule"])`;
    - `entry("panel-rules", "Panel rules", &["blur", "notifications", "bar", "layer"])`;
    - `entry("workspace-rules", "Workspace rules", &["monitor", "persistent", "gaps"])`.
  - **`RulesPage.qml`** is built with Kirigami, everything is reachable by keyboard, and every
    piece of data text goes through `UqLabel`:
    - **Header.** **New rule** (a menu: blank, or a template; template items use
      `contentItem: UqLabel`, F28), **Pick a window** and **Why does this window…?**.
    - **Section error.** When `section` is set, a `RuleMessage` at the top shows its message
      (F9).
    - **Tabs.** "Windows", "Panels" and "Workspaces". Each rule row has an on/off switch (fixed
      literal or no `text:`), its name, the match text and effects text, a source line
      ("Overridden in user.lua, line N" with **Open user.lua**, reusing `SettingRow`'s handler),
      **Edit**, **Move up/down** and **Delete**.
    - **Picking (F5).** Before `pick()` the page calls `applicationWindow().hide()`. It shows
      and raises the window again when `pick.seq` changes, **and** when `busy` goes false after
      a pick it started, whatever the result.
  - **`RuleEditor.qml`** (a `Kirigami.Dialog` whose `title` is the fixed literal "New rule" or
    "Edit rule", never the rule name, F28):
    - a name field (100 chars max);
    - `property var fields` (the snapshot's catalogue, F6) and `property var exact` (a function;
      the page passes `rm.exact`, tests pass a stub, F18);
    - "Match" rows, one per `fields.match` item, each with a check box to include it. Pattern
      items are "Pattern" boxes (256 chars max) with a "Use exactly this text" helper that calls
      `exact(text)`.
    - "Effects": check boxes and spin boxes per `fields.window_effects` item, with ranges from
      `min`/`max` and a muted "applies when the window opens" hint where `static` is true;
    - an embedded `RuleMatchList` fed by `rm.preview()` (150 ms debounce), headed "Matches
      now";
    - an `AdvancedDetails` disclosure with the raw `lua` keys and the rule's `uq:<id>`.
    - No `ToolTip` or `Kirigami.FormData.label` is bound to data (F28).
    - The rule JSON is built from the catalogue's `key` values; no quoted key literals (F29).

    It emits `accepted(string kind, string ruleJson)`.
  - **`RuleMatchList.qml`** has `property var matches`. Each row shows title and class through
    `UqLabel`s with `objectName`s (`matchTitle`, `matchClass`), "Matches if it's a dialog" for
    `if_dialog` and "Can't tell from here" for `unknown`. An empty list says "No open window
    matches. You can still save the rule."
  - **`RuleMessage.qml`** has `property var lines` and shows plain `UqLabel` lines with a close
    button. It's used for `reopen` ("Close and reopen {names} to apply."), the section error,
    pick statuses ("No window there.", "Choosing a window was cancelled.", "The desktop shell
    didn't answer. Try again.", "Took too long; nothing was chosen.", "Couldn't start choosing a
    window."), the "Why" result and refusals.

- [ ] **Step 1: Write the failing tests:**
  - In `registry.rs`:
    - `rules_page_registered_with_search_entries`;
    - update `profiles_page_registered_with_search_entries` (Profiles sits between Shell and
      Window rules) and `pages_json_lists_pages_in_order` (`…, "profiles", "rules",
      "history"`) (F24);
    - remove Task 2's skip from `every_journal_domain_has_one_page`, which now needs `rules` on
      exactly one page.
    - The existing `every_setting_has_keywords`, `no_duplicate_setting_ids` and
      `ids_unique_and_categories_ordered` must still pass.
  - In `tst_rule_editor.qml` (with the `fields` fixture and a stub `exact`):
    - `test_checked_fields_build_json` (float + size and class → the JSON the page sends);
    - `test_exact_text_helper_calls_exact` (the box gets the stub's return value);
    - `test_float_and_tile_exclusive`;
    - `test_name_limit`;
    - `test_title_is_fixed` (F28): a rule named `<b>x</b>` leaves the dialog title "Edit rule".
  - In `tst_rule_match_list.qml`:
    - `test_rows_are_plain_text` (F26): find the `matchTitle` label by `objectName`; for a title
      `<b>x</b>` its `text` is that literal and its `textFormat` is `Text.PlainText`;
    - `test_empty_says_so`;
    - `test_if_dialog_row` and `test_unknown_row`.
  - In `tst_rule_message.qml`: `test_reopen_lines` and `test_rich_text_not_rendered` (the line
    label found by `objectName` holds the literal text).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-settings registry -- --test-threads=2` and the
  settings QML suite where Qt is installed (else CI, K4). Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `scripts/lint-settings.sh` and `tests/shell/lint_settings.sh`. Expected:
  PASS. CI runs
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`
  and `cargo test -p uq-settings`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): the Window rules page`

### Task 11: Docs, changelog, live checks and the whole gate

**Files:**
- Modify: `README.md`, `CHANGELOG.md` (`## [Unreleased]` → `### Added`) and
  `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` (§12: a follow-up row
  for rules in profiles, F30).
- Create: `docs/notes/settings-slice-c-live-check.md`.

- [ ] **Step 1:** Update `README.md`:
  - Add a "#### Window rules page" paragraph. It covers:
    - what a rule can match and do, with the static/dynamic note ("Close and reopen to apply");
    - Pick a window, Matches now and Why does this window…?;
    - templates and panel rules;
    - workspace rules and how they join the per-workspace layouts;
    - the limits;
    - that undo puts back a whole list (window, panel or workspace rules) as it was (F36);
    - that `user.lua` is never written.
  - Add an "Overriding a uQ rule from user.lua" snippet:
    `hl.window_rule({ name = "uq:r-7f2c", enabled = false })`. Say that the id is shown under
    "Advanced details" in the editor, that the page then shows the rule as overridden, and that
    a line that is a `--` comment doesn't count.
  - Add `PickWindow`, `PickDone`, `ListWindows` and `PickRequested` to the Settings D-Bus
    paragraph, and say that rules use `GetSection`/`SetSection("rules")`.
  - Say that profiles can't hold rules yet.
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Window rules.** It names the page, pick mode, live
  preview, Why does this window…?, templates, panel and workspace rules, history and undo for
  rules, and the new D-Bus calls. In the spec, add to §12 a row "C2. Rules in profiles | §4
  `rules` key (enable or disable named rules), portable rule keys | C, B".
- [ ] **Step 3:** Write `docs/notes/settings-slice-c-live-check.md` in the style of
  `settings-slice-b-live-check.md` (run on real Hyprland; tick each item). The checks:
  1. **§1 criterion 4.** Open Pavucontrol. On **Window rules**, press **Pick a window** and click
     Pavucontrol. The editor fills the app ID, and **Matches now** lists it. Check **Float** and
     save. The page says "Close and reopen Pavucontrol to apply." Close it and reopen it: it
     floats.
  2. **Dynamic effects apply at once.** Add "See-through 80 %" to a rule for an open terminal. It
     changes without reopening, and no reopen message shows.
  3. **Why does this window…?** With two matching rules and one that doesn't match, pick the
     window. It lists the two, in order.
  4. **user.lua override.** Add `hl.window_rule({ name = "uq:<id>", enabled = false })` to
     `user.lua` and reload. Switch back to the app. The rule row and the Why list say
     "Overridden in user.lua, line N", and **Open user.lua** opens it. Comment the line out
     with `--`, reload, switch back, and the flag is gone. `user.lua`'s modification time is
     unchanged by every step of this checklist.
  5. **Pick mode edges:**
     - Esc cancels;
     - waiting 20 s gives "Took too long";
     - `systemctl --user stop uq-shell` gives "The desktop shell didn't answer" within about
       2 s (start it again after);
     - while the screen is locked, nothing shows;
     - pressing **Pick a window** with uqd stopped brings the app straight back with a message.

     In every case the app comes back.
  6. **Templates.** Each of the five builds, saves and works: dialogs of an app float; an app
     opens on workspace 3 quietly; no blur; Firefox picture-in-picture floats and stays on top;
     a fullscreen video in mpv keeps the screen awake (`idle_inhibit`).
  7. **Panels and workspaces:**
     - a panel rule "Blur notifications" on `^noti-fi$` blurs the popups;
     - a workspace rule for 3 on a second monitor, persistent, with gaps 0, moves and stays;
     - `uqctl layout set 3 master` keeps the rule, and `hyprctl workspacerules` shows one merged
       rule;
     - a window rule matching workspace `name:web` shows the window on a named workspace `web`
       in **Matches now**.
  8. **Hyprland names (the catalogue check).** Import the CI fixture's "every effect" rules into
     a test user's state, reload, and run `hyprctl configerrors`. It must be empty on the
     installed 0.56.x or 0.57.x. Then check by eye:
     - each layer animation (`fade`, `slide`, `popin`);
     - that `tag = "+name"` sets a tag (`hyprctl clients` lists it);
     - that the workspace `gaps_in = 0` takes effect;
     - that rules survive `hyprctl reload`.

     Note any failure in `rules/hypr.rs` and in the PR.
  9. **History, undo and restore.** Undo the rule from **History** (the row reads "Window rule
     “…” added"). It's gone. Restore yesterday's point, and the rules list matches it.
  10. **Refusals in plain words.** The patterns `negative:foo`, `[a&&b]`, `(?x)a` and `a{1001}`,
      a 201st rule and Float + Tile are each refused with a plain message (no "regex", one
      line), and nothing changes.
  11. **Profiles.** Importing a profile with a `[rules]` table is still refused with "…which this
      version of uQ can't do yet."
- [ ] **Step 4:** Run the full gate, exactly as CI does:
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features --locked -- -D warnings`;
  - `cargo test -j 2 --workspace --locked -- --include-ignored --test-threads=2`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh`;
  - both `qmltestrunner` suites.

  In the build container (K4), run everything without Qt (`--exclude uq-settings`) and without
  `--include-ignored`. Leave `uq-settings`, the QML suites, the Quickshell-based shell tests and
  the `--ignored` Hyprland verify to CI. Expected: all PASS.
- [ ] **Step 5:** Write the PR body's "Spec deviations" section from the list near the top of
  this plan (F30). Commit (K5 trailers): `docs: settings slice C — window rules, live checks and changelog`
