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
    the pick lookup share, and `rules/templates.rs`.

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
  `[[rules.workspace]]` in `state.toml` (a new `Settings.rules` field, so it sits at the top level
  beside `[layouts]`, like every other section). Its section JSON is
  `{"window": [...], "layer": [...], "workspace": [...]}`, each in list order.
- **Window rule:** `{id, name, enabled, match, effects}`.
  - `id` is stable and made by uqd.
    - **Ruling:** `^r-[0-9a-f]{4,8}$`. uqd makes `r-` plus 4 random hex digits and retries on a
      collision. Ids are unique across window and layer rules.
    - **Ruling:** a rule sent without an id (or with `""`) gets one in `State::set_section`. A
      supplied id must be valid and unused.
  - `name` is plain text, 1–100 chars, with no control characters.
  - `enabled` is a bool.
- **Match fields (window, §5):** `class`, `title`, `initial_class`, `initial_title` (patterns);
  `tag`; `xwayland`, `floating`, `fullscreen`, `pinned` (bool); `workspace`. At least one field is
  required.
  - **Ruling, `tag` is a tag name, not a pattern.** Hyprland matches `tag` with `TagMatchEngine`
    (`CTagKeeper::isTagged`), not with its regex engine (`src/desktop/rule/Rule.cpp`). It's
    `^[A-Za-z0-9_.-]{1,32}$`.
  - **Ruling, `workspace`** is a selector checked by `settings::rule_selector`, the same check the
    per-workspace layouts use.
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
    - it must compile with `RegexBuilder::new(&format!("^(?:{p})$")).size_limit(1 << 18)`.
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
  `WORKSPACE_FIELDS`, `IDLE_INHIBIT`, `LAYER_ANIMATIONS`). The renderer, the validator and the
  page read them from there. If CI's `--verify-config` or the live check rejects a name, the fix
  is one table row. If a name can't be fixed, drop that effect from the table and the page, and
  say so in the PR. Don't work around it.

### Sources and history (§2, §3)

- **Explain (§2):** "A uQ bind or rule that `user.lua` unbinds or replaces shows as overridden".
  `Explain("rules")` returns one row per rule, keyed `window.<id>`, `layer.<id>` or
  `workspace.<selector>`, each `{source, live: null, stored: null, detail}`. Sources:
  - `error` (with `message`): the last `rules` write was rejected by verification (slice A's
    `last_rejected`).
  - `user_lua` (with `line` and `path`): for a window or layer rule, `user.lua` contains the
    literal `uq:<id>`. For a workspace rule, a line holds `workspace_rule` and the quoted
    selector. Both checks use the first match, on the same 1 MiB lossy read as today.
  - `uq` otherwise.
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

    It never prints rule JSON. Names are already plain text with no control characters.
  - **Ruling, labels:** `history::label` gains `("rules", "window")` → "Window rules",
    `("rules", "layer")` → "Panel rules" and `("rules", "workspace")` → "Workspace rules".
    `domain_label("rules")` is "Window rules", for the undo conflict prompt.
  - **Undo** restores the whole list that changed (each list is one leaf, as arrays are today).
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
  1. The app calls `PickWindow() → s`. uqd makes a 128-bit random hex token, keeps it in a pick
     broker *outside* the desktop lock, and sends `PickRequested(token)`.
  2. The bridge forwards it as the event `PickRequested {token}`. The shell shows the overlay on
     every screen and calls `PickDone [token, "shown"]` at once.
  3. A click calls `PickDone [token, "click"]`, and Esc (or the shell's own 20 s timer) calls
     `PickDone [token, "cancel"]`. The shell sends no coordinates and no window data.
  4. On `click`, uqd reads `j/cursorpos`, `j/monitors` and `j/clients`, finds the window under
     the cursor with `rules::window_at`, and answers `PickWindow` with only that window's
     metadata.
- **Ruling, the reply** (JSON):
  - `{"status": "picked", "window": {class, title, initial_class, initial_title, xwayland,
    floating, fullscreen, pinned, workspace, tags}}`;
  - `{"status": "none"}` when the click hit no window;
  - `{"status": "cancelled"}`;
  - `{"status": "timeout"}`;
  - `{"status": "no_shell"}` when there was no `shown` within 2 s;
  - `{"status": "unavailable"}` when Hyprland isn't reachable.

  The extra fields belong to the same clicked window. "Why does this window…?" needs them to
  match `floating`, `fullscreen`, `pinned`, `workspace` and `tag`. The reply never has an
  address, pid or any other window's data (§10: "Pick mode returns only window metadata the user
  clicked").
- **Ruling, timeouts:** `PICK_ACK_TIMEOUT` is 2 s and `PICK_TIMEOUT` is 20 s. 20 s stays under the
  usual 25 s D-Bus method-call timeout, so the caller always gets a reply rather than a bus
  error. A second `PickWindow` while one is pending is `Conflict("Already choosing a window.")`.
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

- **Pick a window** pre-fills `match.class` with `^` + `regex::escape(class)` + `$`.
- **Matches now** re-runs the matcher in Rust on the cached `ListWindows` whenever the draft
  changes (QML debounces typing by 150 ms). "A rule with no matches says so but can still be
  saved."
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
    disclosure.
- **Untrusted text.** Window titles and classes come from other apps. Rule names and patterns are
  user text that can arrive from `state.toml` or a restore point. All of it is shown through
  `UqLabel` (app) or `UqText` (shell), never through a stock control's `text:` or a
  `Kirigami.InlineMessage`.
  - **Ruling:** the page's messages ("Close and reopen …", the pick result, refusals that echo a
    pattern) use a plain-text `RuleMessage.qml`, built like slice B's `ProfileMessage.qml`.

## Review Focus

1. **Lua injection through a rule.** A name, pattern, monitor, workspace or tag can hold
   `"`, `\`, `]]`, newlines or control characters, or `") hl.exec_cmd("x`. Every string reaches
   `generated.lua` only through `lua_string`, and names never reach a Lua comment. Task 3 tests
   `hostile_strings_stay_inside_lua_strings`, and Task 2 tests `refuses_hostile_rules`.
2. **A pattern Hyprland can't use.** That covers `negative:x`, `[a&&b]`, an unclosed group, a
   257-char pattern and `(a*)*` with a huge repeat. Each is refused before any write, with a
   plain message that names the field. Task 2 tests `check_pattern_refuses_what_re2_would_not_take`.
3. **Hyprland rejects the rendered rules** (a renamed effect on 0.56.2, say). Then
   `generated.lua` and `state.toml` stay as they were, `Explain("rules")` reports `error` with
   Hyprland's text, and the page shows it. Task 6 tests `rejected_rules_keep_previous_config`.
4. **Pick mode leaks or hangs.** These cases must hold:
   - no shell answers within 2 s;
   - the user presses Esc;
   - nothing happens for 20 s;
   - a second pick starts;
   - a forged `PickDone` arrives.

   The caller gets a status every time, within the stated time. The desktop lock is never held
   while waiting (a `SetSection` during a pick goes through). Only the clicked window's fields
   come back. Task 7 tests `pick_*`.
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
  - `pub struct Field { pub uq: &'static str, pub lua: &'static str, pub kind: Kind, pub label: &'static str }`.
    `label` is the plain page label.
  - `pub struct Effect { pub field: Field, pub when: When }`.
  - Tables:
    - `pub const MATCH_KEYS: [Field; 10]`. Each row maps uq → lua: `class` → `class`, `title`,
      `initial_class`, `initial_title`, `tag`, `xwayland`, `floating` → `float`, `fullscreen`,
      `pinned` → `pin`, `modal`, `workspace`.
    - `pub const LAYER_MATCH_KEYS: [Field; 1]` (`namespace`).
    - `pub const WINDOW_EFFECTS: [Effect; 20]`, the 11 static and 9 dynamic effects from Global
      Constraints. `move` is `Pair`, `size` is `Pair`, `opacity` is `Float`, `idle_inhibit` is
      `Choice(&IDLE_INHIBIT)`, and `workspace` is `Text`.
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
  - `spec_match_names_map_to_lua` (`floating` → `float`, `pinned` → `pin`, the others are the same);
  - `labels_are_plain`: no label holds `regex`, `xwayland`, `namespace`, `class` or `hl.`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core rules::hypr -- --test-threads=2`. Expected:
  FAIL (module missing).
- [ ] **Step 3:** Implement. Cite source file and tag in a comment above each table.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): one table of Hyprland rule names, checked against 0.56.2 and 0.57.0`

### Task 2: `uq_core::rules`: the domain model, validation, ids and patterns

**Files:**
- Modify:
  - `crates/uq-core/src/rules/mod.rs` (the model);
  - `crates/uq-core/src/settings/mod.rs`:
    - `pub rules: Rules` on `Settings`, with `#[serde(deserialize_with = "lenient")]`;
    - `SECTION_DOMAINS` gains `"rules"`, and becomes `[&str; 8]`;
    - `renders_section` gains `"rules"`;
    - `Settings::section`/`set_section` gain `"rules"`;
  - `crates/uq-core/src/settings/tests.rs` (`SECTION_DOMAINS` list);
  - `crates/uq-core/src/settings/load.rs` (the `validated` repair step calls `Rules::repaired`).
- Create: `crates/uq-core/src/rules/check.rs` (patterns and value checks) and
  `crates/uq-core/src/rules/tests.rs`.

**Interfaces:**
- Consumes: Task 1's tables; `settings::{rule_selector, valid_output_name, round2, check_known, invalid}`;
  `std::hash::RandomState` for ids (as `history::restore::random_suffix`).
- Produces (`uq_core::rules`):
  - Serde structs. Every `Option` is `skip_serializing_if = "Option::is_none"`, and load is
    lenient per entry: a bad stored rule is dropped with `tracing::warn!`, and the rest load.
    - `pub struct Rules { pub window: Vec<WindowRule>, pub layer: Vec<LayerRule>, pub workspace: Vec<WorkspaceRule> }`
      (`Default`).
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
    - `from_section(v: &Value, taken_from: &Rules) -> Result<Rules, SettingError>`. It's strict:
      `check_known` on every object, typed values, every limit and contradiction in Global
      Constraints, and every pattern through `check_pattern`. Missing ids are filled with
      `fresh_id`. It rounds `opacity` and `ignore_alpha`, and normalises selectors with
      `rule_selector`.
    - `repaired(self) -> Rules` (load time). It drops invalid, duplicate-id or over-limit
      entries with a log line, so the section stays writable.
    - `ids(&self) -> Vec<&str>`.
  - `pub fn fresh_id(taken: &BTreeSet<String>) -> String`.
  - `pub fn valid_id(id: &str) -> bool`.
  - `pub fn check_pattern(field: &str, p: &str) -> Result<(), SettingError>`.
  - `pub fn exact_pattern(s: &str) -> String` = `format!("^{}$", regex::escape(s))`.
  - `SettingError::Invalid` keys name the rule by position and field, for example
    `rules.window[3].match.class`. Reasons are plain: "this pattern can't be used: …",
    "a rule needs at least one thing to match", "a rule needs at least one effect", "Float and
    Tile can't both be on", "at most 200 window rules".

- [ ] **Step 1: Write the failing tests** in `rules/tests.rs`:
  - `spec_example_parses`: the §5 TOML through `State::parse_strict`, with the spec's regex. It
    gives one window, one layer and one workspace rule, and `section("rules")` round-trips through
    `set_section`.
  - `missing_ids_are_assigned_and_unique`: three new rules get three distinct valid ids, and
    supplied ids are kept.
  - `refuses_bad_or_duplicate_ids` (`"x"`, `"r-ZZZZ"`, the same id on a window and a layer rule).
  - `check_pattern_refuses_what_re2_would_not_take` (Review Focus 2): `negative:foo`, `[a&&b]`,
    `[a--b]`, `(`, 257 chars, `"a\u{7}"`, and a pattern over the size limit (`"(?:a{1000}){1000}"`).
    Each is an `Err` with the field in the key. `^(org\.pulseaudio\.pavucontrol)$`, `firefox`
    and `.*\.pdf - Viewer` are `Ok`.
  - `needs_a_match_and_an_effect`;
  - `refuses_contradictions` (float + tile, move + center, silent without workspace);
  - `ranges` (opacity 0.05 and 1.2, rounding 21, border 21, size `[0, 10]`, move 20000,
    `ignore_alpha` 1.5, gaps 101 are refused; opacity 0.333 → 0.33);
  - `closed_values` (`idle_inhibit = "sometimes"` and `animation = "spin"` are refused; tag
    `"a b"` is refused);
  - `limits` (201 window, 51 layer, 65 workspace rules, a 101-char name, a name with `\n`);
  - `refuses_hostile_rules`: unknown keys at every level (`match.bogus`, `effects.exec`,
    `rules.bogus`), and wrong types (`enabled = "yes"`, `size = [1]`).
  - `workspace_rule_selectors_normalised_and_unique` (`"web"` → `name:web`; `"3"` twice is
    refused);
  - `lenient_load_drops_only_the_bad_rule`: a `state.toml` with one rule that has
    `effects.bogus` and one good rule loads the good one, and the theme and pins are untouched.
  - `exact_pattern_escapes` (`org.kde.dolphin` → `^org\.kde\.dolphin$`).
  - In `settings/tests.rs`: update the `SECTION_DOMAINS` assertion, and assert
    `renders_section("rules")`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core rules -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Use `regex_syntax::ast::parse::Parser::new().parse(p)` and walk
  `Ast::ClassBracketed` for `ClassSet::BinaryOp`, then `regex::RegexBuilder` with
  `size_limit(1 << 18)` and `dfa_size_limit(1 << 18)`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS. Fix any
  existing test that enumerated domains (`history::labels`, `profile`).
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): rules domain with checked patterns, limits and stable ids`

### Task 3: Render rules into `generated.lua`

**Files:**
- Create: `crates/uq-core/src/lua/rules.rs`.
- Modify:
  - `crates/uq-core/src/lua/mod.rs`: `mod rules; pub use rules::{emit_window_rule, emit_layer_rule, emit_workspace_rules};`.
    `emit_generated` replaces the `-- Per-workspace layouts` loop with
    `emit_workspace_rules(layouts, &settings.rules.workspace)` and appends the window and panel
    blocks at the end.
  - `crates/uqd/tests/verify_config.rs`:
    - the mock's must-call list gains nothing (rules are optional), but the mock gets
      `hl.window_rule` and `hl.layer_rule` through its proxy;
    - a new fixture state `rules_settings()` has one window rule using **every** effect (split
      across rules where they contradict), one layer rule with every effect, and one workspace
      rule with every field;
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
  - `pub fn emit_layer_rule(r: &LayerRule) -> String`, the same way.
  - `pub fn emit_workspace_rules(layouts: &BTreeMap<String, Layout>, rules: &[WorkspaceRule]) -> String`.
    It emits one `hl.workspace_rule({ workspace = …, layout = …, monitor = …, … })` per
    selector, in the order from Global Constraints, under the `-- Workspace rules` comment. It's
    empty when there's nothing.
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
    `layout = "master"` and `persistent = true`, then `1`.
  - `hostile_strings_stay_inside_lua_strings` (Review Focus 1). A name, pattern, monitor and tag
    hold `"`, `\`, `]]`, `\n` and `") hl.exec_cmd("x`. Each rule is exactly one output line
    starting with `hl.`, and every value round-trips through a small Lua-string unescaper in the
    test. In `crates/uqd/tests/verify_config.rs`, `hostile_rules_run_as_one_call_each` runs the
    same rules through the Lua mock and checks that it records exactly one `hl.window_rule` or
    `hl.layer_rule` call per rule and no `hl.exec_cmd`.
  - Update `emits_rules_sorted` in `lua/mod.rs` for the new comment.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core lua -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and
  `cargo test -j 2 -p uqd --test verify_config -- --test-threads=2`. Expected: PASS. The
  `--ignored` Hyprland run is CI's gate for the names (Global Constraints). If it fails, fix the
  row in `rules/hypr.rs` or drop the effect, and say so in the PR.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): render window, panel and workspace rules in list order`

### Task 4: `uq_core::rules`: the matcher, `window_at` and templates

**Files:**
- Create: `crates/uq-core/src/rules/matcher.rs` and `crates/uq-core/src/rules/templates.rs`.
- Modify: `crates/uq-core/src/rules/mod.rs` (modules and re-exports).

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)] pub struct WindowInfo { pub class: String, pub title: String, pub initial_class: String, pub initial_title: String, pub xwayland: bool, pub floating: bool, pub fullscreen: bool, pub pinned: bool, pub workspace: String, pub tags: Vec<String> }`.
    It's the pick reply's and `ListWindows`' row.
  - `pub fn windows_from_clients(json: &str) -> Vec<WindowInfo>`. It maps `initialClass` →
    `initial_class`, takes `fullscreen` int > 0 as `true`, keeps only `mapped` windows, and caps
    the list at 500. Strings are cut to 512 chars, with control characters replaced by spaces.
  - `pub enum Verdict { Yes, No, IfDialog }`.
  - `pub struct Compiled` (a window rule's match, compiled once) with
    `Compiled::new(&WindowMatch) -> Result<Compiled, SettingError>` and
    `fn test(&self, w: &WindowInfo) -> Verdict`. It's a full match on patterns. `workspace`
    compares with `rule_selector` normal forms (an id or `name:`); `modal` gives `IfDialog` when
    everything else matches.
  - `pub fn matching(rules: &[WindowRule], w: &WindowInfo) -> Vec<(usize, Verdict)>`, in list
    order, with disabled rules included. The caller greys them.
  - `pub fn window_at(clients_json: &str, monitors_json: &str, cursor_json: &str) -> Option<WindowInfo>`:
    - the visible workspaces are each monitor's `activeWorkspace.id` and `specialWorkspace.id`
      (when non-zero);
    - candidates are mapped, not-hidden windows on a visible workspace (or pinned) whose
      `at`/`size` box holds the cursor;
    - the order is: special-workspace windows first, then fullscreen, then floating by ascending
      `focusHistoryID`, then tiled by ascending `focusHistoryID`.
  - `pub fn effects_text(e: &WindowEffects) -> Vec<String>` gives plain lines ("Floats",
    "Opens on workspace 3, without switching to it", "See-through: 90 %", "Keeps the screen
    awake while fullscreen"). `layer_effects_text` and `match_text` do the same for their parts.
  - `pub fn has_static(e: &WindowEffects) -> bool` uses `hypr::is_static`.
  - `templates.rs`:
    - `pub enum Needs { App, AppAndWorkspace, Nothing }`;
    - `pub struct Template { pub id: &'static str, pub name: &'static str, pub needs: Needs }`;
    - `pub const TEMPLATES: [Template; 5]` (Global Constraints);
    - `pub fn build(id: &str, app: Option<&str>, workspace: Option<&str>, silent: bool) -> Result<WindowRule, SettingError>`.
      It gives a rule with `id = ""` and a plain default name ("Pavucontrol dialogs float"),
      taken from the last dot-segment of the app ID, capitalised.

- [ ] **Step 1: Write the failing tests:**
  - In `matcher.rs`:
    - `full_match_semantics`: `firefox` doesn't match `firefox-esr`, `fire.*` does, and `^`/`$`
      are harmless.
    - `every_match_field` (one case each, including `floating`/`pinned`/`xwayland` and
      `workspace` `"web"` against `name:web`);
    - `modal_gives_if_dialog`;
    - `matching_keeps_order_and_disabled`;
    - `windows_from_clients_maps_and_cleans` (a fixture captured from `HyprCtl.cpp`'s format:
      `initialClass`, `fullscreen: 2`, an unmapped window dropped, a title with `\u{1b}[2J`
      cleaned);
    - `window_at_prefers_floating_over_tiled`, `window_at_ignores_other_workspaces`,
      `window_at_sees_special_workspace`, `window_at_none_on_empty_desktop`;
    - `effects_text_is_plain` (no Lua key appears in any line).
  - In `templates.rs`: `each_template_builds_a_valid_rule` (every result passes
    `Rules::from_section`), `templates_need_their_inputs`, and `pip_template_needs_no_app`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core rules -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): match windows the way Hyprland does, find the clicked window, rule templates`

### Task 5: History summaries and labels for rules; profiles still refuse rules

**Files:**
- Create: `crates/uq-core/src/rules/summary.rs`.
- Modify:
  - `crates/uq-core/src/history/diff.rs` (`summarize` delegates `rules`);
  - `crates/uq-core/src/history/labels.rs` (three labels; `populated()` gains a state with rules
    of every kind);
  - `crates/uq-core/src/settings/provenance.rs`:
    `pub fn find_rule_in_user_lua(text: &str, needle: &str) -> Option<u32>` and
    `pub fn find_workspace_rule_in_user_lua(text: &str, selector: &str) -> Option<u32>`;
  - `crates/uq-core/src/profile/mod.rs` (tests only).

**Interfaces:**
- Produces:
  - `pub fn summary(before: &Value, after: &Value) -> String` (`rules::summary`). It pairs
    rules by id (workspace rules by selector) and classifies each change as added, removed,
    turned on, turned off or changed. A list whose ids are the same but in a different order is
    "reordered". It caps at 3, then ", and N more", with the wording from Global Constraints.
    Names are cut to 40 chars with "…".
  - `history::label` rows for `rules`.
  - Both `user.lua` finders give 1-based line numbers. They skip lines whose first non-space
    characters are `--`.

- [ ] **Step 1: Write the failing tests:**
  - In `summary.rs`:
    - `added_removed_changed_toggled`;
    - `reorder_only`;
    - `caps_at_three`;
    - `never_prints_json`: no `{`, `[` or `"match"` in any summary from a fuzz of 50 random
      before/after pairs built from templates.
  - In `labels.rs`, extend `every_key_a_summary_can_show_has_a_plain_label` to `rules`, and add
    `rules_summary_uses_names`.
  - In `provenance.rs`:
    - `finds_rule_name_in_user_lua`: the line with `uq:r-7f2c` is found, a commented-out line is
      skipped, and `uq:r-7f2` doesn't match `uq:r-7f2c0`, because the needle must be followed by a
      quote;
    - `finds_workspace_rule_line`.
  - In `profile/mod.rs`:
    - extend `refuses_reserved_domains_for_now` with a comment that slice C keeps the
      reservation;
    - add `from_journal_skips_rules` (`profile::from_journal("rules", &["window".into()])` is
      `None`).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): plain history lines for rule changes; profiles keep refusing rules`

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
      `{"x":0,"y":0}`);
  - `crates/uqd/src/service.rs`:
    - `explain` hands `"rules"` to `Desktop::explain_rules`;
    - `domain_label` gains `"rules" => "Window rules"`;
  - `crates/uqd/src/dbus.rs` (`list_windows() → s`);
  - `crates/uq-core/src/client.rs` (`DesktopProxy::list_windows`).

**Interfaces:**
- Consumes: Tasks 2–5, `commit_section`, `note_section_commit`, `last_rejected` and
  `read_user_lua`.
- Produces:
  - `SetSection("rules", json, origin)` works with no new code beyond Task 2's `set_section`. The
    tests pin it.
  - `pub(crate) fn explain_rules(&self) -> serde_json::Value` (see Global Constraints).
  - `pub async fn list_windows(&self) -> Result<Vec<WindowInfo>, DesktopError>`. Hyprland down is
    `Ok(vec![])`.
  - D-Bus `ListWindows() → s` gives a JSON array.

- [ ] **Step 1: Write the failing tests** in `rules/tests.rs`, with `FakeCompositor`:
  - `set_rules_section_renders_verifies_and_journals`: the verified `generated.lua` contains the
    rule line, a `reload` is called, the journal entry's domain is `rules` with origin
    `settings`, and its summary is `Window rule “Pavucontrol floats” added`.
  - `ids_are_assigned_by_uqd` (`GetSection` after a write without ids has valid ids);
  - `rejected_rules_keep_previous_config` (Review Focus 3): `verify_error` is set, so
    `state.toml`, `generated.lua` and `GetSection` are unchanged, and
    `explain("rules")["window.<id>"].source == "error"` with the message.
  - `undo_restores_the_rule_list` (add, then `undo(entry)`, gives no rules) and
    `restore_point_brings_rules_back`;
  - `explain_flags_user_lua_override` (a `user.lua` with `hl.window_rule({ name = "uq:<id>", enabled = false })`
    → `user_lua`, line 1, path);
  - `explain_workspace_rule_in_user_lua`;
  - `layout_and_workspace_rule_writes_stay_apart` (Review Focus 5): `set_layout("3", master)`
    leaves `rules.workspace` alone, and a `rules` write leaves `layouts` alone. The rendered file
    has one merged `workspace = "3"` call.
  - `list_windows_maps_clients` and `list_windows_empty_when_hyprland_down`;
  - `invalid_pattern_is_refused_before_render` (no `verify` call is recorded).
  - In `compositor.rs` socket tests: `clients_and_cursorpos_requests` (the fake socket sees
    `j/clients` and `j/cursorpos`).
  - In `crates/uqd/tests/dbus.rs`: `rules_section_and_list_windows_over_dbus`, and extend
    `desktop_proxy_matches_the_interface`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd rules -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `service.rs` growth to the `explain` hook and `domain_label`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS, including every
  slice A and B test.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): window rules as a verified section, explained against user.lua`

### Task 7: uqd: pick mode (`PickWindow`, `PickDone`, `PickRequested`)

**Files:**
- Create: `crates/uqd/src/pick.rs` (with its tests).
- Modify:
  - `crates/uqd/src/lib.rs` (`pub mod pick;`);
  - `crates/uqd/src/dbus.rs`: the broker lives on the interface struct beside `SharedDesktop`,
    not in `Desktop`. Add `pick_window`, `pick_done` and the `pick_requested` signal.
  - `crates/uq-core/src/client.rs` (`pick_window`, `pick_done` and `receive_pick_requested`);
  - `crates/uqd/tests/dbus.rs`.

**Interfaces:**
- Produces (`crate::pick`):
  - `pub const PICK_ACK_TIMEOUT: Duration = 2 s` and `pub const PICK_TIMEOUT: Duration = 20 s`.
  - `pub enum Outcome { Shown, Click, Cancel }`, parsed from `"shown" | "click" | "cancel"`.
    Anything else is `InvalidArgument`.
  - `pub struct PickBroker` (`Default`) with:
    - `begin(&self) -> Result<(String, Waiter), DesktopError>`, where a second `begin` while
      pending is `Conflict("Already choosing a window.")`;
    - `done(&self, token: &str, o: Outcome)`, which ignores unknown tokens;
    - `with_timeouts(ack, total)` for tests.
  - `pub async fn pick(broker: &PickBroker, comp: Arc<dyn Compositor>, emit: impl Fn(&str)) -> String`.
    It returns the reply JSON from Global Constraints. It takes the compositor `Arc` cloned
    under the desktop lock, then releases the lock before waiting. The window comes from
    `rules::window_at(clients, monitors, cursor)`.
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
    - `pick_hyprland_down_is_unavailable`.
  - In `crates/uqd/tests/dbus.rs`:
    - `pick_over_dbus`: a client task listens for `PickRequested` and calls `PickDone`
      `shown` then `click`, and the `PickWindow` caller gets `picked`.
    - `pick_does_not_hold_the_desktop_lock` (Review Focus 4): while a pick waits, `SetSection("look", …)`
      from another proxy returns.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd pick -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement with `tokio::sync::oneshot` and `tokio::time::timeout`. The token is
  16 bytes from `RandomState`-seeded hashes, hex-encoded; don't add a `rand` dependency.
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
  - `shell/shell.qml` (`PickOverlay {}`).

**Interfaces:**
- Consumes: Task 7's signal and `PickDone`.
- Produces:
  - `pick.js` (pure, no Quickshell imports):
    - `validToken(t)` is `/^[0-9a-f]{32}$/`;
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
  - `apps/uq-settings/src/pages/fake_uqd/windows.rs`, a `FakeWindows` with a window list and a
    queue of scripted pick replies.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod rules;`);
  - `apps/uq-settings/src/pages/fake_uqd.rs`:
    - `list_windows`, `pick_window` and `explain("rules")`, delegating to `FakeWindows` and
      `uq_core::rules`;
    - the `rules` section goes through the existing generic section write, so ids are assigned
      by `State::set_section`;
    - `Rig.windows`.

**Interfaces:**
- Consumes:
  - Tasks 2, 4 and 5's `Rules`, `Compiled`, `matching`, `effects_text`, `has_static`,
    `TEMPLATES`, `build` and `exact_pattern`;
  - `runner::{PageLogic, patch_section, parse_json}`;
  - `DesktopProxy::{get_section, set_section, explain, list_windows, pick_window}`.
- Produces:
  - `#[derive(Debug, Clone)] pub enum Op`. `kind` is `"window" | "layer" | "workspace"`, and
    `key` is a rule id or a workspace selector.
    - `Refresh`, `RefreshWindows`;
    - `Save { kind: String, rule: Value }`: an add when `rule` has no `id` (or a workspace
      selector not yet present), else a replace;
    - `Delete { kind, key }`, `Move { kind, key, to: usize }`, `SetEnabled { kind, key, on: bool }`;
    - `FromTemplate { template: String, app: Option<String>, workspace: Option<String>, silent: bool }`;
    - `Preview { draft: Value }`, which only computes and writes nothing;
    - `Pick { purpose: String }` (`"match"` or `"why"`).
  - `pub struct RulesState` (a `PageLogic`), with `new(conn: zbus::Connection) -> Self`. Every
    write reads the section, edits it, and writes it with origin `settings` under the write lock.
  - **The snapshot:**
    ```
    { window:    [{id, name, enabled, match_text: [..], effects_text: [..], static: bool,
                   source, line, matches_now: n}],
      layer:     [{id, name, enabled, namespace, effects_text, source, line}],
      workspace: [{workspace, layout, fields_text, source, line}],
      windows:   [WindowInfo],
      preview:   {seq, valid: bool, error: "", matches: [{title, class, verdict}]},
      why:       {seq, window: WindowInfo | null, rules: [{id, name, enabled, verdict, effects_text, source, line}]},
      pick:      {seq, purpose, status, window: WindowInfo | null},
      reopen:    {seq, names: [..], more: n},
      templates: [{id, name, needs}],
      limits:    {window: 200, layer: 50, workspace: 64, name: 100, pattern: 256},
      notice:    {seq, text} }
    ```
    - `source` and `line` come from `Explain("rules")`.
    - `layout` comes from `GetSection("layouts")`.
    - `pick.status` is the uqd status, and the page turns it into plain text.
    - `reopen` is set after a `Save` or `FromTemplate` whose rule `has_static` and matches open
      windows.
    - `windows` are refetched on `Refresh`, `RefreshWindows`, after a pick and after a write.
      `Preview` uses the cached list.
    - A preview whose pattern fails `check_pattern` gives `valid: false` and the plain reason.

- [ ] **Step 1: Write the failing tests** in `pages/rules/tests.rs`, against the fake uqd:
  - `save_new_rule_gets_id_and_journals` (origin `settings`, the history summary names it);
  - `edit_move_toggle_delete_round_trip` (§9's round-trip rule: reopening the state gives the
    same lists);
  - `preview_lists_matching_windows_and_says_none` (three windows; `^firefox$` gives one, and
    `^nope$` gives `matches: []`);
  - `preview_with_bad_pattern_is_invalid_not_an_error`;
  - `pick_for_match_prefills_exact_class` (`pick.window.class` and the draft suggestion
    `^org\.pulseaudio\.pavucontrol$`);
  - `pick_cancel_and_no_shell_are_plain_statuses`;
  - `why_lists_matching_rules_in_order_with_override_flag` (two of three rules match, and one
    has `source: "user_lua"`, `line: 4`);
  - `static_effect_save_asks_to_reopen` (float on a matching open window gives `reopen.names`
    with its title) and `dynamic_only_save_does_not` (opacity);
  - `templates_build_and_save` (each of the five, with `app = "org.pulseaudio.pavucontrol"` and
    workspace `3` where needed);
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
  - `tests/settings-qml/tst_rule_editor.qml`, `tests/settings-qml/tst_rule_match_list.qml` and
    `tests/settings-qml/tst_rule_message.qml`.
- Modify:
  - `apps/uq-settings/build.rs` (the QML files and `src/rules_model.rs`);
  - `apps/uq-settings/src/main.rs` (`mod rules_model;`);
  - `apps/uq-settings/src/registry.rs`;
  - `scripts/settings-jargon.txt` (Global Constraints).

**Interfaces:**
- Consumes: Task 9's `RulesState`, `Op` and the snapshot.
- Produces:
  - `RulesModel` (a `#[qml_element]`) has the properties `state_json`, `error` and `busy`. Its
    invokables are:
    - `refresh()`, `refresh_windows()`;
    - `save(kind, rule_json)`, `remove(kind, key)`, `move_to(kind, key, to)` and
      `set_enabled(kind, key, on)`;
    - `from_template(id, app, workspace, silent)`;
    - `preview(draft_json)`;
    - `pick(purpose)`.

    Its `FOLLOW` is: sections `rules` and `layouts`, plus `history`.
  - **Registry.** The page is `PageInfo { id: "rules", name: "Window rules", icon: "window", category: Category::Uq, qml: "pages/RulesPage.qml", settings: &RULES_SETTINGS, domains: &["rules"] }`,
    placed after `profiles` and before `history`. `RULES_SETTINGS` has:
    - `entry("window-rules", "Window rules", &["float", "workspace", "opacity", "app", "window", "rule", "pin"])`;
    - `entry("pick-window", "Pick a window", &["click", "choose", "which window"])`;
    - `entry("why-window", "Why does this window…?", &["explain", "matches", "rule"])`;
    - `entry("panel-rules", "Panel rules", &["blur", "notifications", "bar", "layer"])`;
    - `entry("workspace-rules", "Workspace rules", &["monitor", "persistent", "gaps"])`.
  - **`RulesPage.qml`** is built with Kirigami, everything is reachable by keyboard, and every
    piece of data text goes through `UqLabel`:
    - **Header.** **New rule** (a menu: blank, or a template), **Pick a window** and **Why does
      this window…?**.
    - **Tabs.** "Windows", "Panels" and "Workspaces". Each rule row has an on/off switch, its
      name, the match text and effects text, a source line ("Overridden in user.lua, line N" with
      **Open user.lua**, reusing `SettingRow`'s handler), **Edit**, **Move up/down** and
      **Delete**.
    - **Picking.** Before `pick()` the page calls `applicationWindow().hide()`, and it shows and
      raises the window again when `pick.seq` changes.
  - **`RuleEditor.qml`** (a `Kirigami.Dialog`):
    - a name field (100 chars max);
    - "Match" rows, one per `MATCH_KEYS` field, each with a check box to include it. Text fields
      are "Pattern" boxes (256 chars max) with a "Use exactly this text" helper that escapes.
    - "Effects": check boxes and spin boxes per effect, with a muted "applies when the window
      opens" hint on static ones;
    - an embedded `RuleMatchList` fed by `preview()` (150 ms debounce), headed "Matches now";
    - an "Advanced details" disclosure with the raw Hyprland keys.

    It emits `accepted(string kind, string ruleJson)`.
  - **`RuleMatchList.qml`** has `property var matches`. Each row shows title and class through
    `UqLabel`, and "Matches if it's a dialog" for `if_dialog`. An empty list says "No open window
    matches. You can still save the rule."
  - **`RuleMessage.qml`** has `property var lines` and shows plain `UqLabel` lines with a close
    button. It's used for `reopen` ("Close and reopen {names} to apply."), pick statuses ("No
    window there.", "Choosing a window was cancelled.", "The desktop shell didn't answer. Try
    again.", "Took too long; nothing was chosen."), the "Why" result and refusals.

- [ ] **Step 1: Write the failing tests:**
  - In `registry.rs`, `rules_page_registered_with_search_entries`. The existing
    `every_setting_has_keywords`, `no_duplicate_setting_ids`, `ids_unique_and_categories_ordered`
    and `every_journal_domain_has_one_page` (which now needs `rules`) must still pass.
  - In `tst_rule_editor.qml`:
    - `test_checked_fields_build_json` (float + size and class → the JSON the page sends);
    - `test_exact_text_helper_escapes`;
    - `test_float_and_tile_exclusive`;
    - `test_name_limit`.
  - In `tst_rule_match_list.qml`:
    - `test_rows_are_plain_text`: a title `<b>x</b>` shows the literal text, and the `UqLabel`'s
      `textFormat` is `Text.PlainText`;
    - `test_empty_says_so`;
    - `test_if_dialog_row`.
  - In `tst_rule_message.qml`: `test_reopen_lines` and `test_rich_text_not_rendered`.
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
- Modify: `README.md` and `CHANGELOG.md` (`## [Unreleased]` → `### Added`).
- Create: `docs/notes/settings-slice-c-live-check.md`.

- [ ] **Step 1:** Update `README.md`:
  - Add a "#### Window rules page" paragraph. It covers:
    - what a rule can match and do, with the static/dynamic note ("Close and reopen to apply");
    - Pick a window, Matches now and Why does this window…?;
    - templates and panel rules;
    - workspace rules and how they join the per-workspace layouts;
    - the limits;
    - that `user.lua` is never written.
  - Add an "Overriding a uQ rule from user.lua" snippet:
    `hl.window_rule({ name = "uq:r-7f2c", enabled = false })`. Say that the id is shown under
    "Advanced details" in the editor, and that the page then shows the rule as overridden.
  - Add `PickWindow`, `PickDone`, `ListWindows` and `PickRequested` to the Settings D-Bus
    paragraph, and say that rules use `GetSection`/`SetSection("rules")`.
  - Say that profiles can't hold rules yet.
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Window rules.** It names the page, pick mode, live
  preview, Why does this window…?, templates, panel and workspace rules, history and undo for
  rules, and the new D-Bus calls.
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
     `user.lua` and reload. The rule row and the Why list say "Overridden in user.lua, line N",
     and **Open user.lua** opens it. Remove the line, reload, and the flag is gone. `user.lua`'s
     modification time is unchanged by every step of this checklist.
  5. **Pick mode edges:**
     - Esc cancels;
     - waiting 20 s gives "Took too long";
     - `systemctl --user stop uq-shell` gives "The desktop shell didn't answer" within about
       2 s (start it again after);
     - while the screen is locked, nothing shows.

     In every case the app comes back.
  6. **Templates.** Each of the five builds, saves and works: dialogs of an app float; an app
     opens on workspace 3 quietly; no blur; Firefox picture-in-picture floats and stays on top;
     a fullscreen video in mpv keeps the screen awake (`idle_inhibit`).
  7. **Panels and workspaces:**
     - a panel rule "Blur notifications" on `^noti-fi$` blurs the popups;
     - a workspace rule for 3 on a second monitor, persistent, with gaps 0, moves and stays;
     - `uqctl layout set 3 master` keeps the rule, and `hyprctl workspacerules` shows one merged
       rule.
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
  10. **Refusals in plain words.** The pattern `negative:foo`, the pattern `[a&&b]`, a 201st rule
      and Float + Tile are each refused with a plain message, and nothing changes.
  11. **Profiles.** Importing a profile with a `[rules]` table is still refused with "…which this
      version of uQ can't do yet."
- [ ] **Step 4:** Run the full gate, exactly as CI does:
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features --locked -- -D warnings`;
  - `cargo test -j 2 --workspace --locked -- --include-ignored --test-threads=2`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh`;
  - both `qmltestrunner` suites.

  In the build container (K4), run everything without Qt (`--exclude uq-settings`). Leave
  `uq-settings`, the QML suites, the Quickshell-based shell tests and the `--ignored` Hyprland
  verify to CI. Expected: all PASS.
- [ ] **Step 5:** Commit (K5 trailers): `docs: settings slice C — window rules, live checks and changelog`
