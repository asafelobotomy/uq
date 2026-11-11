# uQ Phase 3 slice 4: keyboard shortcuts in uqd, with an editor — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** uqd owns every uQ keyboard shortcut. They render from one table into `generated.lua`
and are checked by Hyprland before they apply. A **Keyboard shortcuts** page in `uq-settings`
covers every uQ default and the user's own shortcuts:
- rebind a default, turn it off, or reset it (or reset all of them);
- add your own shortcut that opens a uQ surface, acts on a window or workspace, starts an app
  (`uqctl launch <desktop-id>`) or runs a command;
- see a conflict while you type: the editor warns and names both shortcuts, and uqd refuses
  the save;
- see which shortcuts `user.lua` overrides (spec §2), and the `user.lua` binds marked
  `-- uq:bind`, read-only.

`uq.profile.next` ("Switch to the next profile") is a new default with no keys. A profile can
turn shortcuts on and off by id (spec §4), and never adds a command.

`keybinds` is a normal uQ domain. It is validated, saved through the single save point
`persist(next, origin)`, journalled for undo and restore points, explained, and it renders
through the usual verify step.

**Architecture:**
- **uq-core** gains `keybinds/`:
  - `keybinds/hypr.rs` is the **one table** of Hyprland Lua names: modifiers, `hl.bind` option
    names, dispatcher forms and shell shortcut names. Each is checked against the pinned
    Hyprland source.
  - `keybinds/keys.rs` is the closed key table: spellings, plain labels and kinds.
  - `keybinds/combo.rs` parses, normalises and displays combos.
  - `keybinds/actions.rs` holds the closed action set and its Lua form.
  - `keybinds/defaults.rs` is uQ's default table, migrated from `base.lua`.
  - `keybinds/mod.rs`, `check.rs` and `conflict.rs` are the domain: model, validation, ids and
    conflicts. The conflict finder is generic, so slice F's gestures (§7.2) can reuse it.
  - `keybinds/summary.rs`, `keybinds/explain.rs` and `keybinds/list.rs` are pure and shared by
    uqd and the app's fake uqd. They hold the history lines, the `Explain` rows, and the
    `ListBinds` view with the `user.lua` finders.

  `lua/keybinds.rs` renders the `-- Keyboard shortcuts` block. The `uq_core::profile` module
  gains a `Keybinds` domain kind.
- **uqd:**
  - `keybinds` becomes a section domain (`GetSection`/`SetSection`/`Explain`/`Reapply`).
  - New D-Bus calls: `ListBinds() → s`, `PauseShortcuts() → s` and `ResumeShortcuts(s)`, for
    key capture.
  - The compositor gains `binds()` (`j/binds`).
- **uqctl:** `uqctl keys list` and `uqctl keys reset <id>|--all`, a way back from a terminal.
- **uq-settings:**
  - Qt-free page logic in `pages/keybinds.rs` (and `pages/keybinds/`), tested against the fake
    uqd, including the Qt key code → key name mapping;
  - a thin `keybinds_model.rs`;
  - `KeybindsPage.qml` and its components;
  - one registry entry.

**Tech Stack:** Rust edition 2024, zbus 5, tokio, cxx-qt 0.10, Qt 6, Kirigami 6,
Hyprland 0.56.2–0.57.x (Lua config).

**Spec:** the authority is `docs/superpowers/specs/2026-11-08-uq-phase3-settings-design.md`:
- §2.3: the `keybinds` domain, `ListBinds`, `BindsChanged`;
- §2.4: the keybinds section itself;
- §3: the `Commands` line;
- §4: keybind conflicts;
- §5: conflict detection, and the test that the default table matches the old binds;
- §8: slice 4.

This plan also covers these parts of `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`:
- §2: the sources for shortcuts, and "a uQ bind that `user.lua` unbinds or replaces shows as
  overridden";
- §3: history entries, labels and restore points for `keybinds`;
- §4: `keybinds` in profiles ("enable or disable named binds only"), the optional
  `uq.profile.next` keybind, and "never imports `keybinds` that add commands";
- §7.2: the conflict rule shared with gestures, as an interface only;
- §9: the quality rules;
- §10: security.

It follows the layout of settings slices C and D (`2026-11-10-uq-settings-slice-c-rules.md` and
`…-slice-d-startup-apps.md`, both merged). Task 13 follows slice C's Task 11, whose code is in
`crates/uq-core/src/profile/rules.rs`.

**Paths.** The settings app lives at `apps/uq-settings` (not `crates/`). Its QML tests are in
`tests/settings-qml`.

## Pre-flight rulings

These are facts this plan's author checked against the code at `bb26a80` (`main`) and against
Hyprland's source at both pinned tags. The task text below uses them. Before executing, run the
usual conflict scan, store it as
`.superpowers/sdd/2026-11-11-uq-phase3-slice4-keybinds/preflight.md`, and fold its rulings in
here as `F1…`, as slices C and D did.

- **P1. No keybind code exists yet.** `config/hypr/base.lua` holds every bind (54 `hl.bind`
  calls once the two `for` loops are counted). Other places that mention keybinds:
  - `crates/uq-core/src/profile/mod.rs:74` has `("keybinds", DomainKind::Reserved)`.
  - Three tests expect `[keybinds]` to be refused:
    - `profile/mod.rs:762` (`NeedsNewer("keybinds")`);
    - `profile/rules/tests.rs:85`;
    - `apps/uq-settings/src/pages/profiles/tests.rs:502` (`[keybinds] terminal = true`).
  - `README.md:193` tells users to add `uqctl profile next` to `user.lua` by hand.
- **P2. `emit_generated(theme, layouts, rules, settings, startup)`** is called from:
  - the `lua/*.rs` tests;
  - `crates/uq-core/tests/generated_golden.rs`;
  - `crates/uqd/src/service.rs` (`render_candidate`);
  - `crates/uqd/tests/verify_config.rs`.

  The golden files are `crates/uq-core/tests/golden/generated/{default,all-set,startup-all}.lua`.
- **P3. Domain registration:**
  - `SECTION_DOMAINS` is `[&str; 11]` (`settings/mod.rs:65`);
  - `renders_section` lists `appearance look layouts input rules env compat`;
  - `State` keeps `rules`, `env`, `compat` and `autostart` beside `layouts`, not in `Settings`
    (slice C's F21);
  - `domain_label` and `change_for` live in `crates/uqd/src/service.rs`;
  - `history::diff::summarize` delegates `rules`, `env`, … to per-domain summaries;
  - array values are history leaves.
- **P4. `verify_config.rs`'s Lua mock**:
  - records every `hl.bind` as `{key, dsp, opts}`, but its `desc` keeps only the *string*
    arguments of a dispatcher, so `hl.dsp.focus({ workspace = 3 })` reads `hl.dsp.focus()`;
  - doesn't model `hl.unbind`;
  - its must-call list includes `hl.bind`;
  - its spotlight, appmenu, quick-settings, notif-centre, lock, power-menu and OSD checks look
    the binds up by dispatcher;
  - `base_lua_launcher_binds_are_not_locked` and `base_lua_osd_binds_are_locked` read
    `base.lua` text.
- **P5. `hl.bind` at both tags.** These were read in `src/config/lua/bindings/LuaBindingsToplevel.cpp`:
  - v0.56.2 `efb50993780079460b0cbed1363e2166a2de1d9f`: `hlBind` at line 132, registered at line 537;
  - v0.57.0 `0aa6745f705f2c4b0a5004eb019ef76e5bbdb426`: registered at line 472.

  What they show:
  - **Signature:** `hl.bind(keys, dispatcher[, opts])`.
  - **Boolean options** (`getBool`, read with `lua_getfield`): `repeating`, `locked`,
    `release`, `non_consuming`, `auto_consuming`, `transparent`, `ignore_mods`,
    `dont_inhibit`, `long_press`, `submap_universal`, `click`, `drag` and
    `allow_input_capture`.
  - **Other options:** `description` (or `desc`) is a string; `device = { inclusive, list }`.
  - **`mouse`:** 0.57.0 reads it (`BIND_FLAG_MOUSE`). **0.56.2 never reads it**, so it is
    ignored there.
  - **An unknown or misspelled option key is silently ignored.** `["non-consuming"]` would be
    a no-op that `--verify-config` passes. This is the `tap-to-click` trap (commit `4d376ce`)
    in a form CI *can't* catch, so Task 6's mock checks every option key against the table.
  - **Refusals:** `(long_press or release) and repeating` is refused, and `mouse` with
    `repeating`/`release`/`locked` is refused.
  - Copies of the files read are cached in
    `/tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/hyprbind/`
    (`Toplevel-0.56.2.cpp`, `Toplevel-0.57.0.cpp`, `Dispatchers-*.cpp`, `HyprCtl-v0.56.2.cpp`).
    Fetch them again from `raw.githubusercontent.com/hyprwm/Hyprland/<tag>/…` if they are gone.
- **P6. Key strings (`parseKeyString`, 0.56.2; moved out of this file at 0.57.0):**
  - Parts are split on `+` and trimmed; modifiers must come first.
  - Modifiers: `SHIFT`, `CAPS`, `CTRL`/`CONTROL`, `ALT`/`MOD1`, `MOD2`, `MOD3`,
    `SUPER`/`WIN`/`LOGO`/`MOD4`/`META` and `MOD5`.
  - Special keys: `mouse_down`, `mouse_up`, `mouse_left`, `mouse_right`, `mouse:<n>` and
    `switch:<name>`. One special key can't be combined with another key.
  - `code:<n>` is a keycode.
  - Anything else goes through `xkb_keysym_from_name(…, XKB_KEYSYM_CASE_INSENSITIVE)`. An
    unknown keysym is a config error (so `--verify-config` *does* catch a bad key name), and
    `Enter` is refused with "did you mean Return".
  - `catchall` is only allowed inside a submap.
- **P7. `hl.unbind(keys)` and `hl.unbind("all")`** exist at both tags (`hlUnbind`:
  `clearKeybinds()` / `Keybinds::mgr()->clearBinds()`). **`hl.define_submap(name[, reset], fn)`**
  and the dispatcher **`hl.dsp.submap(name)`** exist at both tags
  (`LuaBindingsDispatchers.cpp`, `setFn(L, "submap", hlSubmap)` at 0.56.2 line 1402).
- **P8. `hyprctl binds -j`** (0.56.2 `src/debug/HyprCtl.cpp` `bindsRequest`, line 1026) prints
  `modmask`, `submap`, `key`, `description`, `has_description`, `dispatcher` and `arg` per
  bind. `HyprCtl.cpp` moved before 0.57.0. Task 8 Step 3 re-reads the 0.57.0 file and records
  its path. If `description` is missing there, `Explain` reports from state (source `uq`) and
  the PR says so.
- **P9. Shell shortcut names.** The shell registers twelve names (`appid: "uq"`):
  - in `shell/launcher/LauncherRoutes.qml`: `spotlight`, `appmenu`, `quick-settings`,
    `notif-centre`, `lock` and `power-menu`;
  - in `shell/osd/OsdRoutes.qml`: `vol-up`, `vol-down`, `mute`, `mic-mute`, `bright-up` and
    `bright-down`.
- **P10. Commands that exist.** `uqctl launch <desktop-id>` (slice D, R21,
  `crates/uqctl/src/launch_cmd.rs`) and `uqctl profile next` (slice B,
  `crates/uqctl/src/profile_cmd.rs`) both exist. The compositor trait has `eval` but no
  `binds`.
- **P11. Registry.** `apps/uq-settings/src/registry.rs` has `CORE_PAGES: [PageInfo; 11]`. Its
  tests `every_journal_domain_has_one_page` and `pages_json_lists_pages_in_order` count and
  index pages (`v[10]["category"] == "system"`).
- **P12. Build container.** It has no Qt, Quickshell or Lua. It has
  `/usr/include/X11/keysymdef.h` but no xkbcommon headers. The scratch crate
  `/tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/t7check` already
  symlinks `apps/uq-settings/src/pages`, `registry.rs`, `search.rs`, `desktop_entries.rs`,
  `plugins.rs`, `system_lists.rs` and `xkb_lists.rs`.

## Plan rulings

- **R1, `base.lua` keeps the defaults as a fallback (spec deviation 1).** §2.4 says "`base.lua`
  no longer defines binds". If it didn't, a missing or stale `generated.lua` would leave the
  session with no shortcuts at all, not even a terminal or the lock recovery bind. That happens
  on a fresh install before uqd's first render, and after a package upgrade: Hyprland
  auto-reloads the new `base.lua` while an old `generated.lua` is still in place.
  - So `base.lua` keeps its bind lines, under a header comment that says they are a fallback.
  - `generated.lua`'s `-- Keyboard shortcuts` block opens with `hl.unbind("all")` (P7) and then
    renders the full table.
  - `user.lua` loads last, so its binds and unbinds still win.
  - The spec's migration test becomes permanent (Task 6,
    `base_lua_binds_equal_the_default_table`). The Lua mock runs `base.lua` alone and the
    rendered default block alone, and the two `(key, dispatcher, options)` lists must be equal.
    So the copies can never drift.
- **R2, one id per shortcut.**
  - Defaults have fixed ids in the `uq.` namespace (Global Constraints, "The default table"),
    for example `uq.spotlight` and `uq.ws.focus.3`, as in §2.4.
  - The user's own shortcuts get `k-` plus 4–8 hex digits, made by uq-core with
    `rules::fresh_prefixed` (slice D's generator).
  - Default ids are the same on every computer, so a profile's `[keybinds]` keys for defaults
    always mean the same shortcut (unlike rule ids).
- **R3, a closed key table, four modifiers, no submaps (spec deviation 7).**
  - **Modifiers:** a combo is `SUPER`, `CTRL`, `ALT` and `SHIFT` (any subset) plus one key from
    `keybinds/keys.rs`.
  - **Aliases** are accepted on input and stored canonically: `WIN`/`LOGO`/`MOD4`/`META` →
    `SUPER`, `CONTROL` → `CTRL`, `MOD1` → `ALT`. Letters are case-insensitive.
  - **Refused:** `CAPS`, `MOD2`, `MOD3`, `MOD5`, `code:<n>`, `switch:<…>` and `catchall`.
  - **Submaps:** v1 renders every bind in the global submap, so the conflict key is
    `(modifiers, key)` within that one submap. Spec §2.4's "and submap" holds trivially.
  - **Canonical text** is `MOD + MOD + Key`, with modifiers in the order SUPER, SHIFT, CTRL,
    ALT. That reproduces every `base.lua` string exactly, for example `SUPER + SHIFT + M`,
    `SUPER + ALT + D` and `SUPER + CTRL + L`.
- **R4, the default table keeps `base.lua`'s exact Lua.** That includes `exec_cmd("kitty")`
  for the terminal, the `hyprshutdown` exit string, and `{ mouse = true }` on the two mouse
  binds (0.56.2 ignores it, 0.57.0 reads it; P5). This slice changes no behaviour of a default.
  Making `uq.terminal` follow `shell.terminal` is notes question 1.
- **R5, the override marker is the rendered `description` (spec deviation 4).**
  - Every rendered bind carries `description = "uq:<id>"`. Hyprland keeps it and prints it in
    `hyprctl binds -j` (P8).
  - `Explain("keybinds")` reads `j/binds`. A uQ bind whose combo is live with *our*
    description is `uq`/`default`. Otherwise `user_lua` applies when a finder hits (with the
    line), and `runtime` when it doesn't.
  - The spec's `-- uq:bind` comment marker keeps its §2.4 meaning: it marks a bind *in
    `user.lua`* to be listed read-only.
  - The parity test (R1) ignores `description`, since `base.lua` doesn't set it.
- **R6, conflicts.** The finder is `keybinds::conflict::find`.
  - Two **enabled** binds with a combo conflict when their normalised `(mods, key)` are equal.
    A default with no combo (`uq.profile.next`) or a disabled bind never conflicts.
  - **uqd refuses** a conflicting `SetSection("keybinds")` with `InvalidSetting`, never
    `Conflict` (which History reads as "undo anyway?"). The message names both shortcuts:
    `“Screenshot” and “Open Spotlight” both use Super + K.` The key is the later bind's
    `keybinds.custom[i].combo` or `keybinds.overrides[i].combo`.
  - **The editor warns as you type**, with the same function, synchronously, with no D-Bus
    call.
  - **Binds marked `-- uq:bind` in `user.lua`** with the same combo give a warning only:
    "user.lua line 12 also uses Super + T. Both run." uqd doesn't refuse; `user.lua` is the
    user's file (spec deviation 8).
  - **Undo and restore** go through the same check. An undo that would conflict is refused
    plainly. A restore point is consistent by construction.
  - **The interface for slice F (§7.2):**
    `pub fn find<K: Ord + Clone>(items: &[(K, &str /*id*/, &str /*label*/)]) -> Vec<Conflict>`
    and `pub struct Conflict { pub first: String, pub second: String, pub labels: (String, String) }`.
    Gestures will call it with their own `(fingers, direction, modifier)` key. No gesture code
    is in this slice.
- **R7, `locked` and `repeating` are limited by action (spec deviation 5).**
  - `locked` (works over the lock screen) is allowed only for the OSD shell names (`vol-*`,
    `mute`, `mic-mute`, `bright-*`), the media actions and `uq.lock-recovery`. It is refused for
    `launch`, `command`, the launcher shell names and every window or workspace action, with
    "Shortcuts that open things can't work over the lock screen." This keeps spec Phase 2
    §2.12 ("launcher binds never work over the lock screen") true for custom binds too.
  - `repeating` is refused for `launch` and `command` ("This would start it over and over
    while the key is held") and for any combo whose default has `release` or a mouse button.
  - Defaults keep their own `locked`/`repeating`/`release`/`mouse` (R4). An override changes
    only `combo` and `enabled`.
- **R8, `uq.lock-recovery` can't be turned off (spec deviation 6).** It can be rebound. The
  refusal reads "This shortcut gets you back in if the lock screen stops working, so it stays
  on." Profiles and imports are refused the same way (Task 13).
- **R9, bare keys.** A combo with no SUPER, CTRL or ALT is allowed only when the key's group is
  `media` (the XF86 keys), `Print`, or F13–F24. Otherwise: "Add Super, Ctrl or Alt, so typing
  doesn't trigger it." `SHIFT` alone doesn't count.
- **R10, how `launch` and `command` render.** Hyprland's `exec_cmd` runs its string with
  `/bin/sh -c`.
  - A `launch` renders as `hl.dsp.exec_cmd(<lua_string("uqctl launch " + shell_quote(id))>)`.
    The id has already passed `valid_desktop_id`; quoting it anyway costs nothing.
  - A `command` renders as
    `hl.dsp.exec_cmd(<lua_string("systemd-run --user --scope --quiet --collect --slice=app.slice -- sh -c " + shell_quote(cmd))>)`.
    That is §2.4's "a string run with `sh -c` in a `systemd-run --user --scope`", with slice D's
    R22 slice. `--collect` releases the scope once it exits.
  - `shell_quote` is POSIX single quoting: `'` + the text with each `'` written as `'\''` + `'`.
  - The command text reaches `generated.lua` only as one Lua string literal (`lua_string`). A
    test feeds the rendered `sh -c` argument back through `sh` and checks it round-trips.
  - Binds therefore work with uqd stopped. `uqctl launch` already works without uqd (slice D,
    F2).
- **R11, key capture pauses uQ's shortcuts for at most 15 s (spec deviation 3).** Hyprland
  sees the keys first, so a combo that is already bound (Super + Space) would open Spotlight
  instead of reaching the editor.
  - `generated.lua` defines an empty submap: `hl.define_submap("uq_capture", function() end)`.
    In it no bind matches, so keys go to the focused app.
  - `PauseShortcuts() → s token` evals `hl.dispatch(hl.dsp.submap("uq_capture"))`.
  - `ResumeShortcuts(token)` evals `hl.dispatch(hl.dsp.submap("reset"))`. So does uqd itself
    after 15 s, or when a second pause replaces the first.
  - While paused, every shortcut, the lock recovery bind included, is off for at most 15 s.
    The page says "Your shortcuts are paused while you press keys."
  - Typing a combo by hand (modifier switches plus a key list) always works too, so capture is
    a convenience. Whether an empty submap passes keys through is a live-check item (notes
    question 2).
- **R12, a section domain, plus `ListBinds`, and no `BindsChanged` (spec deviation 2).**
  - Writes go through `SetSection("keybinds", json, origin)`. `SectionChanged("keybinds")` is
    the change signal. Slices C and D set the precedent of no domain-specific write methods
    or signals.
  - `ListBinds() → s` is the merged view the page shows. It covers the defaults with
    overrides applied, the user's own binds, and the `user.lua` binds marked `-- uq:bind`
    (§2.3). Reading `user.lua` belongs to uqd.
  - `keybinds` is a `State` field beside `rules` (F21 pattern), not a `Settings` field, so
    `SettingsChanged` doesn't carry it.
- **R13, the page is "Keyboard shortcuts"** (id `keybinds`, icon
  `preferences-desktop-keyboard-shortcuts`, category uQ, after History). That is spec §2.2's
  "Keybinds" page, named in plain words (§9; spec deviation 9).
- **R14, `uqctl keys list|reset`.** A broken shortcut setup must be fixable without the app
  (§3: "these matter when the desktop is broken"):
  - `uqctl keys reset <id>` drops one override;
  - `uqctl keys reset --all` drops every override;
  - the user's own shortcuts are kept, and the writes use origin `uqctl`.

  uqctl never adds or edits a command bind.
- **R15, profiles hold shortcut switches (spec §4; spec deviation 10).**
  - `[keybinds]` holds `"<id>" = true|false` for a default or the user's own shortcut, as
    slice C's `[rules]` does.
  - A profile can't set a combo, add a shortcut or carry a command. Import refuses anything
    else in `[keybinds]` (§4 "never imports keybinds that add commands").
  - An unknown `k-` id is skipped with a notice and kept in the file.
  - A profile that would create a conflict when applied is refused as a whole, with the
    conflict message.
  - Back to normal re-enables a bind only if that creates no conflict. Otherwise the bind is
    listed in Kept as "left off: its keys are now used by “X”".

**Spec deviations (recorded here and in the PR):**
1. `base.lua` keeps the defaults as a fallback; `generated.lua` replaces them with
   `hl.unbind("all")` (R1).
2. No `BindsChanged` signal; `SectionChanged("keybinds")` is the change signal (R12).
3. Two extra D-Bus calls, `PauseShortcuts` and `ResumeShortcuts` (R11).
4. uQ's rendered binds are recognised by `description = "uq:<id>"` (R5).
5. `locked` and `repeating` are limited by action (R7).
6. `uq.lock-recovery` can't be turned off (R8).
7. A closed key table, four modifiers, and no `code:` or submaps in v1 (R3); bare keys only for
   special keys (R9).
8. Conflicts with `user.lua`'s marked binds warn and don't refuse (R6).
9. The page is called "Keyboard shortcuts" (R13).
10. Profiles key shortcuts by id; a `k-` id from another computer is skipped (R15).

## Global Constraints

These are copied from the specs and the user's standing rules, except the lines marked
**ruling**, which this plan decides.

### Process

- **K4, no Qt here.** The build container has no Qt, no Quickshell and no Lua. These run in CI
  only:
  - `uq-settings` (`cargo test -p uq-settings`);
  - both `qmltestrunner` suites;
  - the `tests/shell/*.sh` scripts that need Quickshell;
  - the Lua-mock tests in `verify_config.rs` (they skip without `lua`);
  - the `--ignored` Hyprland verify tests.

  Every piece of logic that can be Qt-free is:
  - the page logic in `apps/uq-settings/src/pages/keybinds.rs` and `pages/keybinds/*.rs`,
    including the Qt key code table;
  - the combo parser, the key table, the conflict finder, the renderer and the finders in
    `uq-core`.

  Locally, run `cargo test -j 2 -p uq-core -p uqd -p uqctl -- --test-threads=2`.
  - **Page logic locally.** The scratch crate at
    `/tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/t7check`
    symlinks `apps/uq-settings/src/pages`, `registry.rs` and `search.rs`, among others (P12).
    It builds them without Qt. Run
    `cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::keybinds -- --test-threads=2`
    (and `pages::profiles`, `registry`, `search`). This slice needs no new top-level app
    module: everything the page logic needs is under `pages/`. CI's `cargo test -p uq-settings`
    stays the authority.
- **Build settings (earlyoom).** Before the first build, run
  `export CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`. Every cargo command in this plan uses
  `-j 2`, and test runs add `-- --test-threads=2`. Don't run two cargo commands at once. Run
  builds in the foreground and leave nothing running.
- **K5, commits.** Every commit message ends with these two lines, after a blank line:
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc. Don't push until the user asks.
- **Process (as slices C and D):**
  - use private `dbus-daemon` buses only (`uq_core::test_bus`), never the real session bus,
    `hyprctl` or `systemd-run`;
  - every uqd rig builds its paths with `Paths::under(temp)`;
  - `cargo fmt`, `clippy -D warnings` and the tests must be clean **at every commit**;
  - keep new files at or under about 400 lines.
- **Ownership (ruling).** Another thread owns these paths, so this plan edits none of them:
  - `crates/uq-core/src/integration/`;
  - `apps/uq-settings/src/plugins.rs`;
  - `shell/widgets/`;
  - `templates/`;
  - `web/uq-tokens/`;
  - later, `uq-settings-helper` and the greeter.

  These shared files get **additive edits only**. Add rows, entries, arms and tests; don't
  restructure, rename or reorder what is there. The only edits to existing lines are the
  counts and indices an added row forces, and each task names them.
  - `apps/uq-settings/src/registry.rs`;
  - `apps/uq-settings/qml/Sidebar.qml` and `Main.qml` (no change is expected: the sidebar reads
    `pages_json`);
  - `crates/uqd/src/dbus.rs` and `crates/uq-core/src/client.rs`;
  - `crates/uq-core/src/history/labels.rs`;
  - `CHANGELOG.md`;
  - `packaging/uq/PKGBUILD` (no change is expected: no new installed file).

  If a merge conflict with that thread appears in one of these files, keep both sides.
- **Dead code.** Until Task 12 wires `pages/keybinds.rs`, it starts with
  `#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]`. Task 12
  removes it.
- **CI flake.** `tests/shell/pam_stacks.sh` is known to flake. If it's the only red job,
  re-run it.
- **No new crates.** `Cargo.lock` doesn't change.

### Hyprland names (Task 1 holds them)

- **Source.** The names come from the pinned series' own source, read at both tags (P5–P8):
  - `src/config/lua/bindings/LuaBindingsToplevel.cpp` (`hlBind`, `parseKeyString`, `hlUnbind`,
    `hlDefineSubmap`);
  - `src/config/lua/bindings/LuaBindingsDispatchers.cpp` (`dsp` names: `exec_cmd`, `global`,
    `focus`, `submap`, `window.{close,float,move,pin,drag,resize}`,
    `workspace.toggle_special`; `float` and `pin` take `action = "toggle"`, `focus` takes
    `direction` or `workspace`);
  - `src/debug/HyprCtl.cpp` (`binds -j`).

  `keybinds/hypr.rs`'s header comment cites the files, tags and commit hashes.
- **Every name uQ renders lives in `keybinds/hypr.rs`:**
  - `BIND_OPTIONS`: the option names uQ may render: `locked`, `repeating`, `release`, `mouse`,
    `description`. Each row cites the source line, and all are spelled with `_`, never `-`.
  - `ALL_BIND_OPTIONS`: the full P5 list, used only by the mock whitelist test.
  - `MODIFIERS`: the canonical names and their aliases.
  - `DESCRIPTION_PREFIX = "uq:"`.
  - `UNBIND_ALL = "hl.unbind(\"all\")"`.
  - `CAPTURE_SUBMAP = "uq_capture"`.
  - `SHELL_NAMES: [&str; 12]` (P9).
  - The dispatcher forms used by the action catalogue.
- **Ruling, the `-`/`_` trap.** Hyprland's Lua config spells every option with `_`. A
  misspelled `hl.config` key fails `--verify-config` in CI, but a misspelled `hl.bind` option
  is **silently ignored** (P5). So:
  - Task 1's `option_names_use_underscores` checks every table name;
  - Task 6's Lua mock fails on any `hl.bind` option key not in `ALL_BIND_OPTIONS`;
  - Task 6's `generated_lua_table_keys_have_no_dashes` scans the golden and rendered files
    for any Lua table key with a `-`, across every domain, not only shortcuts.
- **Not confirmed from source.** Each of these is a live-check item:
  - an empty submap lets keys through to the focused app (R11);
  - `description` is printed by `binds -j` at 0.57.0 (P8);
  - `SUPER + SUPER_L` with `release` still opens the app menu when rendered from
    `generated.lua` after `hl.unbind("all")`.

### The domain

- **Shape.** The section JSON is
  ```
  { "overrides": [ { "id": "uq.spotlight", "combo": "SUPER + K" },
                   { "id": "uq.close", "enabled": false } ],
    "custom": [ { "id": "k-3f9c", "name": "Screenshot", "combo": "SUPER + SHIFT + P",
                  "enabled": true, "locked": false, "repeating": false,
                  "action": { "kind": "command", "command": "grim -g \"$(slurp)\" ~/shot.png" } } ] }
  ```
  - It's stored as `[[keybinds.overrides]]` and `[[keybinds.custom]]` in `state.toml`.
  - Both are lists, so each is one history leaf (as rules are). Undo puts back a whole list.
  - **Load is lenient per entry** (slice C's F1). A hand-written `Deserialize` keeps each entry
    that parses. Entry structs are `deny_unknown_fields`. A bad entry is dropped with
    `tracing::warn!`, and `repaired()` then drops unknown default ids, invalid combos, dupes
    and conflicts (keeping the earlier bind).
- **Overrides.**
  - `id` must be a default id. An unknown id is refused on write with "isn't one of uQ's
    shortcuts", and dropped on load: it might come from a newer uQ.
  - `combo` is optional (absent means the default's combo). It must be the same *kind* as the
    default's: a key, a wheel step or a mouse button. `uq.appmenu` may also take a lone
    modifier key, because it is a release bind.
  - `enabled` is optional; absent means true. `false` is refused for `uq.lock-recovery` (R8).
  - An override that equals the default (same combo, enabled) is dropped when written, so
    "reset" and "set back by hand" store the same thing.
  - Each id appears at most once. At most `DEFAULTS.len()` entries.
- **Custom.**
  - `id` follows slice C's F2 rule: a supplied id must be `^k-[0-9a-f]{4,8}$` and unique in
    the payload; a missing or `""` id gets a fresh one.
  - `name` is plain text, 1–100 chars, with no control characters.
  - `combo` is required.
  - `enabled`, `locked` and `repeating` are bools, limited by R7.
  - `action` is from the closed set below.
  - At most 100 custom shortcuts.
- **The default table** (`keybinds/defaults.rs`). These are in `base.lua` order, which is also
  the render order. Each row is `{id, label, group, combo, action, locked, repeating, release,
  mouse}`:

  | id | combo | Lua dispatcher (exact) | opts | label |
  |---|---|---|---|---|
  | `uq.terminal` | `SUPER + Return` | `hl.dsp.exec_cmd("kitty")` | | Open a terminal |
  | `uq.close` | `SUPER + Q` | `hl.dsp.window.close()` | | Close the window |
  | `uq.float` | `SUPER + V` | `hl.dsp.window.float({ action = "toggle" })` | | Float or tile the window |
  | `uq.exit` | `SUPER + SHIFT + M` | `hl.dsp.exec_cmd("command -v hyprshutdown >/dev/null 2>&1 && hyprshutdown \|\| hyprctl dispatch 'hl.dsp.exit()'")` | | Log out |
  | `uq.spotlight` | `SUPER + Space` | `hl.dsp.global("uq:spotlight")` | | Open Spotlight |
  | `uq.appmenu` | `SUPER + SUPER_L` | `hl.dsp.global("uq:appmenu")` | release | Open the app menu |
  | `uq.quick-settings` | `SUPER + A` | `hl.dsp.global("uq:quick-settings")` | | Open quick settings |
  | `uq.notif-centre` | `SUPER + N` | `hl.dsp.global("uq:notif-centre")` | | Open notifications |
  | `uq.lock` | `SUPER + L` | `hl.dsp.global("uq:lock")` | | Lock the screen |
  | `uq.power-menu` | `SUPER + Escape` | `hl.dsp.global("uq:power-menu")` | | Open the power menu |
  | `uq.layout.dwindle` / `.master` / `.scrolling` | `SUPER + ALT + D` / `M` / `S` | `hl.dsp.exec_cmd("uqctl layout set current dwindle")` (etc.) | | Use the dwindle (master, scrolling) layout here |
  | `uq.focus.left` / `right` / `up` / `down` | `SUPER + left` (etc.) | `hl.dsp.focus({ direction = "left" })` | | Focus the window to the left (etc.) |
  | `uq.ws.focus.N`, N = 1…10, interleaved with `uq.ws.move.N` | `SUPER + <N % 10>` | `hl.dsp.focus({ workspace = N })` | | Go to workspace N |
  | `uq.ws.move.N` | `SUPER + SHIFT + <N % 10>` | `hl.dsp.window.move({ workspace = N })` | | Move the window to workspace N |
  | `uq.special.toggle` | `SUPER + S` | `hl.dsp.workspace.toggle_special("magic")` | | Show or hide the scratchpad |
  | `uq.special.move` | `SUPER + SHIFT + S` | `hl.dsp.window.move({ workspace = "special:magic" })` | | Move the window to the scratchpad |
  | `uq.ws.next` | `SUPER + mouse_down` | `hl.dsp.focus({ workspace = "e+1" })` | | Next workspace |
  | `uq.ws.prev` | `SUPER + mouse_up` | `hl.dsp.focus({ workspace = "e-1" })` | | Previous workspace |
  | `uq.window.drag` | `SUPER + mouse:272` | `hl.dsp.window.drag()` | mouse | Move the window with the mouse |
  | `uq.window.resize` | `SUPER + mouse:273` | `hl.dsp.window.resize()` | mouse | Resize the window with the mouse |
  | `uq.lock-recovery` | `SUPER + CTRL + L` | `hl.dsp.exec_cmd("systemctl --user restart uq-lock.service \|\| hyprlock")` | locked | Restart the lock screen if it stops working |
  | `uq.vol-up` / `uq.vol-down` | `XF86AudioRaiseVolume` / `XF86AudioLowerVolume` | `hl.dsp.global("uq:vol-up")` (etc.) | locked, repeating | Volume up / down |
  | `uq.mute` / `uq.mic-mute` | `XF86AudioMute` / `XF86AudioMicMute` | `hl.dsp.global("uq:mute")` (etc.) | locked | Mute / Mute the microphone |
  | `uq.bright-up` / `uq.bright-down` | `XF86MonBrightnessUp` / `XF86MonBrightnessDown` | `hl.dsp.global("uq:bright-up")` (etc.) | locked, repeating | Brightness up / down |
  | `uq.media.next` / `uq.media.pause` / `uq.media.play` / `uq.media.prev` | `XF86AudioNext` / `XF86AudioPause` / `XF86AudioPlay` / `XF86AudioPrev` | `hl.dsp.exec_cmd("playerctl next")` / `"playerctl play-pause"` ×2 / `"playerctl previous"` | locked | Next track / Pause / Play / Previous track |
  | **`uq.profile.next`** | **none** (unbound) | `hl.dsp.exec_cmd("uqctl profile next")` | | Switch to the next profile |

  That gives 55 rows, of which 54 are bound by default. Groups, in page order: Apps and
  desktop, Windows, Workspaces, Sound and brightness, Media, Profiles.
- **Actions (closed set, `keybinds/actions.rs`).** These are for the user's own shortcuts. JSON
  is `{"kind": …, "name": …, …}`:
  - `shell`: `name` ∈ `SHELL_NAMES`, rendered `hl.dsp.global("uq:<name>")`.
  - `window`: `close`, `float` (toggle), `pin` (toggle), and `focus` with `direction` ∈
    `left right up down`. Window actions on mouse buttons (`drag`, `resize`) are for the two
    defaults only.
  - `workspace`:
    - `focus` with `workspace` 1–10;
    - `move` with `workspace` 1–10;
    - `toggle_special`;
    - `move_special`;
    - `next`;
    - `prev`.
  - `uq`: `terminal`, `exit`, `layout` with `layout` ∈ `dwindle master scrolling`,
    `media` with `media` ∈ `next play-pause previous`, `profile-next`, and `settings`
    (`uqctl settings`). Each renders the same string as its default.
  - `launch`: `app` passes `valid_desktop_id` and is normalised to end in `.desktop`
    (R10).
  - `command`: `command` is 1–1024 chars with no control characters (no newline, CR, tab or
    NUL), rendered per R10. **Ruling:** any session client can send one through `SetSection`,
    the same reach as editing `user.lua` (Review Focus 1). Profiles, imports and uqctl can't
    (R14, R15).
- **Rendering (`lua/keybinds.rs`).** `emit_generated` gains a `binds: &Keybinds` parameter,
  after `rules`. The block comes last in the file:
  ```
  -- Keyboard shortcuts (uQ Settings). These replace base.lua's fallback copies.
  hl.unbind("all")
  hl.define_submap("uq_capture", function() end)
  hl.bind("SUPER + Return", hl.dsp.exec_cmd("kitty"), { description = "uq:uq.terminal" })
  …
  hl.bind("SUPER + mouse:272", hl.dsp.window.drag(), { mouse = true, description = "uq:uq.window.drag" })
  ```
  - One line per enabled bind with a combo: defaults first in table order (with overrides
    applied), then the user's own in list order.
  - Options are emitted in `BIND_OPTIONS` order, and only when true (plus `description`).
  - Every string goes through `lua_string`. A custom bind's `name` never reaches the file;
    only `uq:<id>` does.
- **Display text (`combo::text`).** Plain words: `Super + Shift + Q`, `Super + Space`,
  `Super + Left mouse button`, `Super + Scroll down`, `Volume up key`. Each key row in
  `keys.rs` has its label. No `XF86`, `mouse:` or `hl.` text appears in any label (Task 1
  tests it; the §9 lint checks the QML).

### Sources and history

- **Explain rows** (`keybinds::explain`, pure, Task 7):
  - The key is `binds.<id>` for every default and custom bind:
    `{source, live: null, stored: <combo or null>, detail}`.
  - **Sources:**
    - `default`: an untouched default. The page shows nothing.
    - `uq`: overridden or custom, and live.
    - `user_lua`: the live bind for that combo isn't ours (or is gone) and a finder hits:
      `{line, path}`.
    - `runtime`: ours is gone and no finder hits.
    - `error`: the last `keybinds` write Hyprland rejected (`{message}`), on every row and on
      a section row `"keybinds"` (slice C's F9).
    - `profile`: set by Task 13's `mark_binds`.
  - `live` is `null`, because the live check is presence by description (R5).
  - With Hyprland down, every row reports from state.
- **The `user.lua` finders** (Task 7):
  - Each line is cut at `--` with `provenance::strip_lua_comment` (slice C's F8), *except*
    when looking for the `-- uq:bind` marker itself.
  - `find_bind_in_user_lua(text, combo) -> Option<u32>`: the first line with an `hl.bind(` or
    `hl.unbind(` call whose first string literal parses (R3 parser) to the same normalised
    combo, or an `hl.unbind("all")` line, which overrides every row.
  - `marked_binds(text) -> Vec<MarkedBind>`: lines holding `hl.bind(` and ending in a
    `-- uq:bind` comment, optionally followed by a label (`-- uq:bind Screenshot`). Each gives
    `{line, combo: Option<String>, text}`. `text` is the label, or else the line cut to 120
    chars, cleaned with `settings::clean_text_spaced`. A concatenated key string (`mainMod ..`)
    doesn't parse, so it has `combo: null` and never conflicts.
  - It's a hint, not a parser (§2). `user.lua` is read only, up to 1 MiB, through
    `read_user_lua`.
- **History.**
  - `history::label` gains:
    - `("keybinds", "overrides")` → "uQ's shortcuts";
    - `("keybinds", "custom")` → "Your own shortcuts".
  - `keybinds::summary(before, after)` pairs binds by id. It names each change by label (a
    custom bind by its name, cut to 40 chars):
    - "“Open Spotlight” now uses Super + K";
    - "“Close the window” turned off";
    - "“Open Spotlight” back to its default";
    - "Shortcut “Screenshot” added", "Shortcut “Screenshot” removed",
      "Shortcut “Screenshot” changed".

    When every override is dropped at once it reads "uQ's shortcuts put back to how they
    came". Summaries cap at 3, then ", and N more", and never print JSON or a command's text
    (Review Focus 1).
  - `domain_label("keybinds")` is "Keyboard shortcuts".

---

## Review Focus

1. **Injection through a shortcut.** These strings must stay inert:
   - a command holding `'`, `"`, `\`, `]]`, `$(…)`, `;` or `") hl.exec_cmd("x`;
   - a name holding the same;
   - a desktop id like `x.desktop; rm -rf ~`.

   Each must stay one Lua string literal and one `sh -c` argument. A name never reaches
   `generated.lua`. A command never appears in a history summary, a log line or a D-Bus error.
   Tests:
   - Task 5: `hostile_strings_stay_inside_lua_and_sh` (round-trips through a Lua-string
     unescaper and through `sh -c 'printf %s "$0"'`);
   - Task 4: `refuses_hostile_binds`;
   - Task 6: `hostile_binds_run_as_one_call_each`.
2. **A misspelled name passes CI.** Hyprland ignores unknown `hl.bind` options (P5). Task 1's
   `option_names_use_underscores`, Task 6's mock whitelist and Task 6's
   `generated_lua_table_keys_have_no_dashes` must each fail on a `["non-consuming"]` slip.
   Check that by mutating one row locally and watching the tests go red before you trust them.
3. **Losing every shortcut.** These cases must keep a working set of shortcuts:
   - `generated.lua` missing;
   - `generated.lua` from before this slice;
   - uqd stopped;
   - a rejected write.

   With a missing or stale file, `base.lua`'s fallback copies are live (R1). A rejected write
   leaves `generated.lua` and `state.toml` as they were. Tests:
   - Task 6: `base_lua_binds_equal_the_default_table`;
   - Task 8: `rejected_binds_keep_previous_config`.
4. **Conflicts.** These pairs conflict:
   - `WIN + q` and `SUPER + Q`;
   - `META + space` and `SUPER + Space`;
   - two custom binds on one combo;
   - a custom bind on a default's combo;
   - an override moving a default onto another's combo.

   These don't:
   - a disabled bind and an enabled one;
   - `uq.profile.next` with no combo.

   Each conflict is refused by uqd with both names, and warned by the editor before Save. Undo,
   a profile apply and Back to normal never produce one. Tests:
   - Task 4: `conflicts_*`;
   - Task 8: `conflicting_write_is_refused_with_both_names`;
   - Task 11: `check_warns_as_you_type`;
   - Task 13: `profile_conflicts_*`.
5. **The lock screen.** No `launch`, `command`, launcher-shell, window or workspace bind can
   be `locked` (R7). `uq.lock-recovery` can't be turned off by the page, uqctl, a profile or
   an import (R8). The launcher binds in the *rendered* file are never locked (Task 6 extends
   the mock checks to the generated binds).
6. **Paused shortcuts come back.** The capture pause must end on all of these:
   - `ResumeShortcuts`;
   - 15 s passing;
   - a second `PauseShortcuts`;
   - the app crashing (the timer still fires);
   - a forged token (refused).

   The desktop lock is never held while waiting. Task 9 tests `capture_*`.
7. **Profiles never add commands.** These are all refused on import with a plain message:
   - `[keybinds]` with a table value;
   - a string value;
   - a combo;
   - an `action`;
   - a bad id;
   - `"uq.lock-recovery" = false`.

   A `k-` id unknown here is skipped with a notice. Task 13 tests it.

---

### Task 1: `uq_core::keybinds::hypr` and `keys`: the name tables

**Files:**
- Create:
  - `crates/uq-core/src/keybinds/mod.rs` (only `pub mod hypr; pub mod keys;` for now);
  - `crates/uq-core/src/keybinds/hypr.rs`;
  - `crates/uq-core/src/keybinds/keys.rs`.
- Modify: `crates/uq-core/src/lib.rs` (`pub mod keybinds;`).

**Interfaces:**
- Produces (`uq_core::keybinds::hypr`). The file's header comment cites the tags, hashes,
  files and line numbers of P5–P8.
  - `pub struct OptionName { pub lua: &'static str, pub source: &'static str }`.
  - `pub const BIND_OPTIONS: [OptionName; 5]`: `locked`, `repeating`, `release`, `mouse`,
    `description`, in render order.
  - `pub const ALL_BIND_OPTIONS: [&str; 17]`: P5's full list plus `mouse`, `description`,
    `desc` and `device`.
  - `pub const MODIFIERS: [(&str, u8, &[&str]); 4]`. Each row is the canonical name, its bit
    in Hyprland's own `eKeyboardModifiers` mask (so `binds -j`'s `modmask` maps back directly)
    and its aliases: `SUPER` 64 `[WIN, LOGO, MOD4, META]`, `SHIFT` 1, `CTRL` 4 `[CONTROL]`,
    `ALT` 8 `[MOD1]`. Render order is SUPER, SHIFT, CTRL, ALT (R3). Task 1 cites the enum's
    source file and checks the bits there.
  - `pub const REFUSED_MODIFIERS: [&str; 4] = ["CAPS", "MOD2", "MOD3", "MOD5"]`.
  - `pub const SHELL_NAMES: [&str; 12]` (P9).
  - `pub const DESCRIPTION_PREFIX: &str = "uq:"`, `pub const CAPTURE_SUBMAP: &str = "uq_capture"`.
- Produces (`uq_core::keybinds::keys`):
  - `pub enum KeyKind { Key, Wheel, Button, ModKey }`.
  - `pub enum Group { Letter, Digit, Function, Navigation, Editing, Punctuation, Media, Mouse }`.
  - `pub struct KeyRow { pub spelling: &'static str, pub label: &'static str, pub kind: KeyKind, pub group: Group }`.
  - `pub static KEYS: &[KeyRow]`:
    - `A`–`Z` (uppercase spelling, as `base.lua`);
    - `0`–`9`;
    - `F1`–`F24`;
    - `Return`, `Space`, `Tab`, `Escape`, `BackSpace`, `Delete`, `Insert`, `Home`, `End`,
      `Prior`, `Next`, `left`, `right`, `up`, `down`, `Print`;
    - `minus`, `equal`, `bracketleft`, `bracketright`, `semicolon`, `apostrophe`, `grave`,
      `comma`, `period`, `slash`, `backslash`;
    - the XF86 keys: `AudioRaiseVolume`, `AudioLowerVolume`, `AudioMute`, `AudioMicMute`,
      `MonBrightnessUp`, `MonBrightnessDown`, `AudioNext`, `AudioPrev`, `AudioPlay`,
      `AudioPause`, `AudioStop`, `Calculator`, `Mail`, `Search`, `Explorer`, `HomePage`,
      `Tools`;
    - the wheel: `mouse_down`, `mouse_up`, `mouse_left`, `mouse_right`;
    - the buttons: `mouse:272`–`mouse:276`;
    - the lone modifier keys `SUPER_L` and `SUPER_R`.

    Labels are plain: "Enter" for `Return`, "Page up" for `Prior`, "Volume up key", "Scroll
    down", "Left mouse button", "Super key".
  - `pub fn find(name: &str) -> Option<(usize, &'static KeyRow)>`: case-insensitive, as xkb is.
    `Page_Up` → `Prior` and `Page_Down` → `Next` are accepted aliases. `Enter` is **not** a
    key (`None`); Task 2 refuses it with "Use Return", as Hyprland does (P6).

- [ ] **Step 1: Write the failing tests:**
  - In `hypr.rs`:
    - `option_names_use_underscores`: every `BIND_OPTIONS`/`ALL_BIND_OPTIONS` name is
      `^[a-z_]+$`;
    - `bind_options_are_known` (each `BIND_OPTIONS` name is in `ALL_BIND_OPTIONS`);
    - `modifier_aliases_are_unique`;
    - `shell_names_match_the_shell`: read `shell/launcher/LauncherRoutes.qml` and
      `shell/osd/OsdRoutes.qml` from the repo (`env!("CARGO_MANIFEST_DIR")/../..`), collect
      every `name: "<x>"` inside a `GlobalShortcut` with `appid: "uq"`, and check the set
      equals `SHELL_NAMES`. It reads, never writes, shell files (Ownership).
  - In `keys.rs`:
    - `spellings_unique_case_insensitively`;
    - `labels_are_plain`: no label contains `XF86`, `mouse`, `:`, `_` or `hl.`;
    - `find_is_case_insensitive` (`q` and `Q` give the same row; `RETURN` gives `Return`);
    - `enter_is_refused_page_up_is_alias`;
    - `keysyms_exist_in_x11_headers`: for every `Key`/`ModKey` row that isn't XF86, the
      spelling appears as `XK_<spelling>` in `/usr/include/X11/keysymdef.h` (case-insensitive
      for letters). XF86 rows are checked as `XF86XK_<rest>` in `XF86keysym.h`. It **skips** with
      a message when a header is missing. CI's verify fixture (Task 6) is the authority.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core keybinds -- --test-threads=2`. Expected: FAIL
  (module missing).
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(core): one table of Hyprland bind names and uQ's key list, checked against 0.56.2 and 0.57.0`

### Task 2: `keybinds::combo`: parse, normalise and display combos

**Files:**
- Create: `crates/uq-core/src/keybinds/combo.rs`.
- Modify: `crates/uq-core/src/keybinds/mod.rs` (`pub mod combo;` and re-exports).

**Interfaces:**
- Produces:
  - `#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)] pub struct Combo { pub mods: u8, pub key: u16 }`.
    `key` is the index into `KEYS`, so `Combo` equality *is* the conflict key (R6).
  - `pub fn parse(text: &str) -> Result<Combo, ComboError>`:
    - split on `+` and trim each part;
    - modifiers come first, aliases are accepted, and a repeated modifier is accepted once;
    - exactly one key follows;
    - refused modifiers and `code:`/`switch:`/`catchall` are refused (R3).
  - `pub fn canonical(c: Combo) -> String`: `SUPER + SHIFT + Q` (R3 order, the key's
    spelling).
  - `pub fn text(c: Combo) -> String`: the display text (Global Constraints).
  - `pub fn needs_modifier(c: Combo) -> bool` (R9).
  - `pub fn kind(c: Combo) -> KeyKind`.
  - `#[derive(thiserror::Error)] pub enum ComboError`, with plain messages:
    - "Pick a key as well as Super, Ctrl, Alt or Shift.";
    - "uQ doesn't know the key “…”." (the name is cleaned and cut to 32 chars);
    - "Use Return for the Enter key.";
    - "Put Super, Ctrl, Alt and Shift before the key.";
    - "Caps Lock and other lock keys can't be part of a shortcut.";
    - "A shortcut has one key.".

- [ ] **Step 1: Write the failing tests:**
  - `round_trips_every_base_lua_string`: every combo string in the default table section of
    Global Constraints (all 54) parses, and `canonical` gives it back byte for byte.
  - `aliases_normalise` (`WIN + q`, `META+Q`, `super + q` and `LOGO + Q` give the same
    `Combo` as `SUPER + Q`, whose canonical form is `SUPER + Q`);
  - `modifier_order_is_canonical` (`ALT + SUPER + d` → `SUPER + ALT + D`);
  - `refusals_are_plain`: `SUPER`, `SUPER + Q + W`, `Q + SUPER`, `CAPS + Q`, `code:24`,
    `switch:Lid Switch`, `catchall`, `SUPER + Enter`, `SUPER + Foo` and a 300-char string each
    give an error with no `\n`, and the 300-char one doesn't echo the input;
  - `text_is_plain` (`SUPER + mouse:272` → "Super + Left mouse button";
    `XF86AudioRaiseVolume` → "Volume up key");
  - `bare_key_rule` (R9: `Q` and `SHIFT + Q` need a modifier; `XF86AudioMute`, `Print` and
    `F13` don't; `F5` does).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core keybinds::combo -- --test-threads=2`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(core): parse and normalise shortcut combos`

### Task 3: `keybinds::actions` and `keybinds::defaults`: the closed action set and uQ's table

**Files:**
- Create:
  - `crates/uq-core/src/keybinds/actions.rs`;
  - `crates/uq-core/src/keybinds/defaults.rs`.
- Modify: `crates/uq-core/src/keybinds/mod.rs`.

**Interfaces:**
- Produces:
  - `#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)] #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)] pub enum Action`:
    - `Shell { name }`;
    - `Window { name, direction: Option<String> }`;
    - `Workspace { name, workspace: Option<u8> }`;
    - `Uq { name, layout: Option<String>, media: Option<String> }`;
    - `Launch { app }`;
    - `Command { command }`.
  - `impl Action`:
    - `fn check(&self) -> Result<Action, SettingError>`: closed names and values, launch id
      normalised (Global Constraints, "Actions");
    - `fn lua(&self) -> String`: the exact dispatcher expression, built from `hypr.rs` names
      and `lua_string`;
    - `fn text(&self) -> String`: plain words ("Opens Spotlight", "Starts Firefox" (from the
      id), "Runs a command"). A command's text is never included.
    - `fn may_lock(&self) -> bool` and `fn may_repeat(&self) -> bool` (R7).
  - `pub fn shell_quote(s: &str) -> String` (R10).
  - `pub struct Default { pub id: &'static str, pub label: &'static str, pub group: Group, pub combo: Option<&'static str>, pub action: DefaultAction, pub locked: bool, pub repeating: bool, pub release: bool, pub mouse: bool }`.
    `DefaultAction` is an `Action`-like enum of `'static` forms, with `fn lua(&self) -> String`
    reproducing the table's exact dispatcher text.
  - `pub fn defaults() -> &'static [Default]`, 55 rows in `base.lua` order.
  - `pub fn default_of(id: &str) -> Option<&'static Default>`.
  - `pub const LOCK_RECOVERY: &str = "uq.lock-recovery"`,
    `pub const PROFILE_NEXT: &str = "uq.profile.next"`.

- [ ] **Step 1: Write the failing tests:**
  - In `defaults.rs`:
    - `table_has_55_rows_54_bound` and `ids_unique_and_well_formed` (`^uq\.[a-z0-9.-]{1,48}$`);
    - `profile_next_is_unbound_and_runs_uqctl` (combo `None`; Lua
      `hl.dsp.exec_cmd("uqctl profile next")`);
    - `default_combos_parse_and_never_conflict` (Task 2 parser; all 54 distinct `Combo`s);
    - `lua_forms_match_base_lua_text`: for each bound default, `base.lua` (read from the repo)
      contains its dispatcher text. The loop rows are checked against their loop templates
      (`hl.dsp.focus({ workspace = i })`). This is a cheap Rust-side guard; Task 6 does the
      real parity.
    - `locked_and_repeating_as_base_lua` (the six OSD rows, the four media rows and the
      recovery bind are locked; four repeat; the launcher rows aren't locked).
  - In `actions.rs`:
    - `every_action_renders_a_known_dispatcher` (each form starts with `hl.dsp.` and uses only
      names from `hypr.rs`);
    - `shell_quote_round_trips` (via `sh -c 'printf %s "$1"' _ <quoted>`, run with
      `std::process::Command::new("sh")`; skipped if `sh` is missing). The cases are `a'b`,
      `''`, `$(x)`, `"`, `\`, a newline-free 1024-char string and the empty string.
    - `launch_renders_uqctl_launch` (`org.mozilla.firefox` →
      `hl.dsp.exec_cmd("uqctl launch 'org.mozilla.firefox.desktop'")`);
    - `command_renders_a_scope`;
    - `closed_values` (unknown shell name, workspace 11, direction `diag`, layout `monocle`,
      unknown kind and an extra field are each refused);
    - `lock_and_repeat_limits` (R7 table).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core keybinds -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(core): uQ's default shortcut table and the closed action set`

### Task 4: the `keybinds` domain: model, validation, ids and conflicts

**Files:**
- Create:
  - `crates/uq-core/src/keybinds/check.rs`;
  - `crates/uq-core/src/keybinds/conflict.rs`;
  - `crates/uq-core/src/keybinds/tests.rs`.
- Modify:
  - `crates/uq-core/src/keybinds/mod.rs` (the model);
  - `crates/uq-core/src/state.rs`:
    - `#[serde(default)] pub keybinds: Keybinds` beside `rules`;
    - `section`/`set_section` dispatch `"keybinds"`;
    - `parse_strict` calls `Keybinds::repaired`;
    - `Default` gains the field. Every full `State {}` literal in the tree gains it (slice D's
      F41).
  - `crates/uq-core/src/settings/mod.rs`:
    - `SECTION_DOMAINS` gains `"keybinds"`, becoming `[&str; 12]`;
    - `renders_section` gains `"keybinds"`;
  - `crates/uq-core/src/settings/tests.rs` (the `SECTION_DOMAINS` assertion);
  - `crates/uq-core/src/history/labels.rs` (additive): the two `("keybinds", …)` rows, and
    `populated()` gains one override and one custom bind, so
    `every_key_a_summary_can_show_has_a_plain_label` covers them from this task on;
  - `crates/uq-core/src/profile/mod.rs`: no behaviour change yet. Its domain-enumerating tests
    may need the new domain listed; `keybinds` stays `Reserved` until Task 13.
  - `apps/uq-settings/src/registry.rs` (additive, CI-only): `every_journal_domain_has_one_page`
    skips `"keybinds"` with `// Task 12 adds the Keyboard shortcuts page and removes this skip.`

**Interfaces:**
- Produces (`uq_core::keybinds`):
  - Serde structs: `Keybinds { overrides: Vec<Override>, custom: Vec<CustomBind> }`,
    `Override { id, combo: Option<String>, enabled: Option<bool> }` and
    `CustomBind { id, name, combo, enabled, locked, repeating, action: Action }`.
    - `Option`s are `skip_serializing_if = "Option::is_none"`.
    - Entry structs are `deny_unknown_fields`.
    - `Keybinds` has a lenient hand-written `Deserialize` (slice C's F1).
  - `impl Keybinds`:
    - `from_section(v: &Value, current: &Keybinds) -> Result<Keybinds, SettingError>`. It's
      strict and checks every rule in Global Constraints ("The domain", R6–R9). It stores
      canonical combos, assigns `k-` ids, drops overrides equal to the default, and runs
      `conflict::check` last.
    - `repaired(self) -> Keybinds` (load).
    - `effective(&self) -> Vec<Effective>`: every bind as rendered, in order. `Effective` is
      `{id, label, combo: Option<Combo>, enabled, lua_action, locked, repeating, release, mouse, is_default}`.
    - `ids(&self)`.
  - `pub fn valid_custom_id(id: &str) -> bool`. `rules::fresh_prefixed(taken, "k-")` is reused
    from slice D.
  - `conflict.rs`:
    - `pub struct Conflict { pub first: String, pub second: String, pub labels: (String, String), pub combo_text: String }`;
    - `pub fn find<K: Ord + Clone>(items: &[(K, &str, &str)]) -> Vec<Conflict>` (generic, R6;
      for slice F too);
    - `pub fn check(kb: &Keybinds) -> Result<(), SettingError>` (refuses with R6's message and
      key);
    - `pub fn message(c: &Conflict) -> String`.

- [ ] **Step 1: Write the failing tests** in `keybinds/tests.rs`:
  - `spec_shape_parses_and_round_trips`: the Global Constraints JSON through `set_section` and
    `section`, and `state.toml` through `State::parse_strict`.
  - `ids_assigned_and_kept`; `resending_the_section_is_accepted` (slice C's F2).
  - `override_equal_to_default_is_dropped` (setting `uq.spotlight` to `SUPER + Space` stores
    nothing).
  - `override_kind_must_match` (`uq.window.drag` → `SUPER + Q` is refused; `uq.appmenu` →
    `SUPER + D` and `SUPER_R` with SUPER are accepted).
  - `unknown_default_id_refused_on_write_dropped_on_load`.
  - `lock_recovery_stays_on` (R8).
  - `conflicts_refused_with_both_names`. These are refused, each with both labels in the
    message and the later entry's combo in the key:
    - a custom bind on `SUPER + Space` (vs `Open Spotlight`);
    - `WIN + space`;
    - two custom binds on `CTRL + ALT + T`;
    - moving `uq.close` onto `SUPER + V`.
  - `conflicts_ignore_disabled_and_unbound`: these are accepted:
    - disabling `uq.spotlight`, then adding a custom bind on `SUPER + Space`;
    - giving `uq.profile.next` `SUPER + ALT + P` (free);
    - giving it `SUPER + ALT + D` is refused (the dwindle layout uses it).
  - `find_is_generic`: called with `(u8, &str)` keys (a gesture-like tuple), it finds the
    duplicate (the slice F interface).
  - `lock_and_repeat_limits` (R7 through `from_section`), `bare_keys` (R9) and `limits` (101
    custom binds, a 101-char name, a command with `\n`, a 1025-char command).
  - `refuses_hostile_binds`: unknown keys at every level (`custom[0].exec`,
    `action.shell_cmd`, `overrides[0].action`) and wrong types.
  - `lenient_load_drops_only_the_bad_entry` (one custom bind with an unknown key, one
    conflicting, one good; theme and pins untouched).
  - In `settings/tests.rs`: the `SECTION_DOMAINS` list, and `renders_section("keybinds")`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core keybinds -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Fix every existing test that enumerates domains (history `diff`,
  `profile`).
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and
  `cargo build -j 2 -p uqd -p uqctl`. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(core): keybinds domain with overrides, your own shortcuts and conflict checks`

### Task 5: render shortcuts into `generated.lua`

**Files:**
- Create: `crates/uq-core/src/lua/keybinds.rs`.
- Modify:
  - `crates/uq-core/src/lua/mod.rs`: `mod keybinds; pub use keybinds::emit_binds;`.
    `emit_generated(theme, layouts, rules, binds, settings, startup)` appends `emit_binds(binds)`
    last.
  - The callers (P2): the `lua/*.rs` tests, `crates/uq-core/tests/generated_golden.rs`,
    `crates/uqd/src/service.rs` `render_candidate` (passes `&next.keybinds`), and
    `crates/uqd/tests/verify_config.rs` (passes `&Keybinds::default()` for now).
  - `crates/uq-core/tests/golden/generated/{default,all-set,startup-all}.lua`: re-bless.
    `all-set` passes a `Keybinds` with two overrides (one rebind, one off) and one custom bind
    of each action kind.

**Interfaces:**
- Produces:
  - `pub fn emit_binds(kb: &Keybinds) -> String`: the block in Global Constraints
    ("Rendering"). It's always emitted, even with no user changes, because the defaults are
    always rendered.
  - `pub fn emit_bind(e: &Effective) -> String`: one line.

- [ ] **Step 1: Write the failing tests** in `lua/keybinds.rs`:
  - `default_block_golden`: `emit_binds(&Keybinds::default())` starts with the comment line,
    `hl.unbind("all")` and the `define_submap` line. It has exactly 54 `hl.bind(` lines,
    `uq.profile.next` is absent, and the first bind line is exactly
    `hl.bind("SUPER + Return", hl.dsp.exec_cmd("kitty"), { description = "uq:uq.terminal" })`.
  - `options_render_in_table_order` (`uq.vol-up` →
    `{ locked = true, repeating = true, description = "uq:uq.vol-up" }`;
    `uq.appmenu` → `{ release = true, description = … }`; `uq.window.drag` →
    `{ mouse = true, description = … }`).
  - `override_rebinds_and_disables`; `profile_next_renders_once_bound`.
  - `custom_binds_follow_defaults_in_list_order`.
  - `hostile_strings_stay_inside_lua_and_sh` (Review Focus 1). A command holding `'`, `"`,
    `\`, `]]`, `$(id)`, `;` and `") hl.exec_cmd("x`, a launch id built past validation, and a
    hostile name each give exactly one output line. The `exec_cmd` argument round-trips
    through a Lua-string unescaper in the test. The `sh -c` argument, recovered from that
    string, round-trips through `sh -c 'printf %s "$0"'` (skipped without `sh`). The name
    appears nowhere.
  - In `lua/mod.rs`: the block comes after the rules blocks.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core lua -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement, then re-bless:
  `UQ_BLESS=1 cargo test -j 2 -p uq-core --test generated_golden -- --test-threads=2`. Read
  the diff: only the new block is added.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`,
  `cargo test -j 2 -p uqd -- --test-threads=2` and `cargo build -j 2 -p uqctl`. Expected:
  PASS.
- [ ] **Step 5:** Commit (K5): `feat(core): render uQ's shortcuts into generated.lua, replacing base.lua's fallback`

### Task 6: the verify tests: parity with `base.lua`, option names, and Hyprland's verdict

**Files:**
- Modify:
  - `crates/uqd/tests/verify_config.rs`:
    - **The mock (`MOCK`):**
      - `desc` serialises table arguments too, as sorted `k=v` pairs (`hl.dsp.focus{workspace=3}`),
        so binds that differ only in a table compare unequal (P4);
      - `hl.unbind("all")` empties `binds`;
      - every `hl.bind` opts key must be in a `KNOWN_OPTS` list, injected from
        `hypr::ALL_BIND_OPTIONS` when the test writes the mock, and must contain no `-`.
        Otherwise it errors with `unknown hl.bind option <k>`.
      - It prints `bind <key> | <dsp> | <sorted opts without description>` for each bind left
        at the end, after the existing `ok <n>` and call lines.
    - **A fixture `keybinds()`:** every default overridden or kept, `uq.profile.next` bound, and
      one custom bind per action kind and per value of each closed enum. It also has one bind
      per `KEYS` row, each on its own free combo, spread over custom binds within the 100
      limit; this is split across two fixtures if needed. Every `emit_generated` call in the
      file passes it (or `Keybinds::default()`), so `lua_mock_runs_every_variant` and
      `all_variants_pass_hyprland_verify` (CI `--ignored`) run every key name, option and
      dispatcher form through Lua and through Hyprland.
    - The existing launcher, lock and OSD assertions now see the *rendered* binds (after
      `hl.unbind("all")`), so they check `generated.lua`, not only `base.lua`.
  - `config/hypr/base.lua`: the KEYBINDINGS header becomes:
    ```
    -- Fallback shortcuts. uqd renders the real set into generated.lua, which starts with
    -- hl.unbind("all") and replaces all of these. They are here so a missing or old
    -- generated.lua still leaves a working desktop. Keep them equal to
    -- crates/uq-core/src/keybinds/defaults.rs (test base_lua_binds_equal_the_default_table).
    ```
    No bind line changes (R1, R4).
  - `crates/uq-core/tests/generated_golden.rs`: `generated_lua_table_keys_have_no_dashes`.

- [ ] **Step 1: Write the failing tests** in `verify_config.rs`:
  - `base_lua_binds_equal_the_default_table` (the spec's migration test, permanent by R1). Run
    the mock on `base.lua` alone, and on a file holding only
    `emit_binds(&Keybinds::default())`. The two `bind …` lists are equal, in order. Skip
    without `lua`; CI runs it.
  - `mock_rejects_a_dashed_option`: a hand-written `hl.bind("SUPER + Q", hl.dsp.window.close(), { ["non-consuming"] = true })`
    makes the mock fail with `unknown hl.bind option` (Review Focus 2). Skip without `lua`.
  - `hostile_binds_run_as_one_call_each`: Task 5's hostile binds give one `hl.bind` call each
    after the unbind, and no `hl.exec_cmd:x` call. Skip without `lua`.
  - `rendered_launcher_binds_are_not_locked`: the mock's existing checks run over
    `generated.lua`'s binds.
  - In `generated_golden.rs`, `generated_lua_table_keys_have_no_dashes`. For each golden file
    and `emit_generated` of the `all-set` inputs, every Lua table key it can find has no `-`:
    `\b([A-Za-z0-9_\-]+)\s*=` outside string literals, and `["…"]` keys. It's general, so a
    future `tap-to-click` slip in *any* domain fails locally too (Review Focus 2).
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd --test verify_config -- --test-threads=2` and
  `cargo test -j 2 -p uq-core --test generated_golden -- --test-threads=2`. Expected: the
  golden test FAILs until written; the Lua tests skip locally (no `lua`, K4).
- [ ] **Step 3:** Implement. Check the guard by hand: temporarily change one `BIND_OPTIONS`
  row to `repeating-x` and confirm that `option_names_use_underscores` and the no-dashes test
  go red; then revert.
- [ ] **Step 4:** Re-run both. Expected: PASS (the Lua ones skip). CI runs the mock tests with
  `lua` and `all_variants_pass_hyprland_verify --ignored` with Hyprland. **If CI's verify
  fails**, fix the row in `keybinds/hypr.rs` or `keys.rs` (or drop the key from `KEYS`), and
  say so in the PR.
- [ ] **Step 5:** Commit (K5): `test: base.lua's fallback shortcuts equal uQ's table; bind option names checked; every key verified by Hyprland`

### Task 7: history lines, the `user.lua` finders, `keybinds::explain` and `keybinds::list`

**Files:**
- Create:
  - `crates/uq-core/src/keybinds/summary.rs`;
  - `crates/uq-core/src/keybinds/explain.rs`;
  - `crates/uq-core/src/keybinds/list.rs`.
- Modify:
  - `crates/uq-core/src/keybinds/mod.rs`;
  - `crates/uq-core/src/history/diff.rs` (`summarize` delegates `keybinds`);
  - `crates/uq-core/src/history/labels.rs` (test only: `keybinds_summary_uses_labels`);
  - `crates/uq-core/src/settings/provenance.rs`: `find_bind_in_user_lua` and `marked_binds`,
    using the existing `strip_lua_comment`.

**Interfaces:**
- Produces:
  - `pub fn summary(before: &Value, after: &Value) -> String` (Global Constraints, "Sources and
    history").
  - `pub fn find_bind_in_user_lua(text: &str, combo: Combo) -> Option<u32>` and
    `pub struct MarkedBind { pub line: u32, pub combo: Option<String>, pub text: String }`,
    with `pub fn marked_binds(text: &str) -> Vec<MarkedBind>` (at most 100).
  - `pub fn explain(kb: &Keybinds, live: Option<&[LiveBind]>, user_lua: Option<&str>, rejected: Option<&str>, user_lua_path: &str) -> Value`.
    `LiveBind { combo: Option<Combo>, description: String }` is parsed from `j/binds` by
    `pub fn live_binds(json: &str) -> Vec<LiveBind>`, which caps at 2000 entries. `modmask` is
    used as-is (the `MODIFIERS` bits are Hyprland's; any other bit gives `combo: None`), and
    `key` goes through `keys::find`. `live: None` (Hyprland
    down) reports from state.
  - `pub fn list(kb: &Keybinds, user_lua: Option<&str>) -> Value`. It's `ListBinds`' JSON:
    ```
    { binds: [{id, label, group, combo, combo_text, default_combo, default_combo_text,
               enabled, is_default, can_disable, action_text, locked, repeating, name}],
      user_lua: [{line, combo, combo_text, text}],
      groups: [{id, label}],
      keys: [{spelling, label, kind, group}],
      actions: {shell: [...], window: [...], workspace: [...], uq: [...]},
      limits: {custom: 100, name: 100, command: 1024} }
    ```
    The catalogues (`keys`, `actions`) come from the tables, so QML never copies them
    (slice C's F6).

- [ ] **Step 1: Write the failing tests:**
  - In `summary.rs`:
    - `rebind_off_reset_and_custom_lines`;
    - `reset_all_reads_plainly`;
    - `caps_at_three`;
    - `never_prints_commands_or_json`: a deterministic enumeration over every action kind × {added,
      removed, changed, turned on/off}, with names holding `{`, `[` and `"`. No summary
      contains the command text, `"action"` or a JSON object.
  - In `provenance.rs`:
    - `finds_bind_and_unbind_lines`: `hl.bind("SUPER + Q", …)` and `hl.unbind("WIN + q")` both
      hit `SUPER + Q`; `-- hl.unbind("SUPER + Q")` doesn't; `hl.unbind("all")` hits every
      combo; `mainMod .. " + Q"` doesn't.
    - `marked_binds_read_label_or_line`.
  - In `explain.rs`:
    - `default_row_is_default`;
    - `override_live_is_uq`;
    - `missing_live_with_user_lua_unbind_is_user_lua` (line and path);
    - `missing_live_without_hit_is_runtime`;
    - `rejected_marks_every_row_and_the_section` (slice C's F9);
    - `hyprland_down_reports_state`;
    - `live_binds_parses_hyprctl_json`: a fixture in `bindsRequest`'s format (P8), with
      `"description": "uq:uq.spotlight"`, `modmask: 64` and `key: "Space"`.
  - In `list.rs`:
    - `lists_defaults_overrides_and_custom_in_order`;
    - `marked_user_lua_binds_are_read_only_rows`;
    - `catalogues_come_from_the_tables`;
    - `profile_next_listed_unbound`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(core): plain history lines for shortcut changes; shortcuts explained against user.lua`

### Task 8: uqd: the `keybinds` section, `Explain`, `ListBinds` and the compositor's `binds`

**Files:**
- Create:
  - `crates/uqd/src/keybinds.rs` (`impl Desktop`, as `rules.rs` does);
  - `crates/uqd/src/keybinds/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`mod keybinds;`);
  - `crates/uqd/src/compositor.rs`:
    - the trait gains `async fn binds(&self) -> Result<String, CompositorError>` (`j/binds`);
    - `FakeCompositor` gains `binds_json` (empty means `[]`), recorded in `calls`;
    - the socket test sees `j/binds`;
  - `crates/uqd/src/service.rs`:
    - `explain` hands `"keybinds"` to `Desktop::explain_keybinds` right after the `"rules"`
      hook;
    - `domain_label` gains `"keybinds" => "Keyboard shortcuts"`.
  - `crates/uqd/src/dbus.rs` (additive): `list_binds() → s`.
  - `crates/uq-core/src/client.rs` (additive): `DesktopProxy::list_binds`.
  - `crates/uqd/tests/dbus.rs`.

**Interfaces:**
- Consumes: Tasks 4–7; `commit_section`, `note_section_commit`, `last_rejected` and
  `read_user_lua`.
- Produces:
  - `SetSection("keybinds", json, origin)` needs no new write code. `State::set_section`
    validates (conflicts included), `commit_section` renders, verifies, reloads and then calls
    `persist(next, origin)`. The tests pin it.
  - `pub(crate) async fn explain_keybinds(&self) -> Value` (reads `binds()` once, and
    `user.lua` once).
  - `pub fn list_binds(&self) -> Result<String, DesktopError>`.
  - D-Bus `ListBinds() → s`.

- [ ] **Step 1: Write the failing tests** in `keybinds/tests.rs`, with `FakeCompositor`:
  - `set_section_renders_verifies_and_journals`: the verified `generated.lua` has the custom
    bind's line, `reload` is called, and the journal entry has domain `keybinds`, origin
    `settings` and summary `Shortcut “Screenshot” added`.
  - `conflicting_write_is_refused_with_both_names` (Review Focus 4): `InvalidSetting` names
    both. No `verify` call is recorded, and `state.toml` is unchanged.
  - `rejected_binds_keep_previous_config` (Review Focus 3): with `verify_error` set,
    `generated.lua`, `state.toml` and `GetSection` are unchanged, and
    `explain("keybinds")["keybinds"].source == "error"`.
  - `undo_and_restore_bring_binds_back`;
    `undo_that_would_conflict_is_refused` (rebind A onto a free combo X, add custom B on A's
    old combo; undoing the rebind would put A back on B's combo and is refused plainly).
  - `explain_live_and_user_lua`: `binds_json` lacking `uq:uq.close` plus a `user.lua` with
    `hl.unbind("SUPER + Q")` on line 3 give `binds.uq.close` = `user_lua`, line 3.
  - `list_binds_includes_marked_user_lua_rows`.
  - `keybinds_not_in_settings_changed` (R12).
  - In `crates/uqd/tests/dbus.rs`: `keybinds_section_and_list_binds_over_dbus`, and extend
    `desktop_proxy_matches_the_interface` with `list_binds` and `explain("keybinds")`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd keybinds -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Re-read `binds -j` at 0.57.0 (P8). Its file moved out of
  `src/debug/HyprCtl.cpp`, so find it at the tag and record the path in `keybinds/hypr.rs`'s
  header. If `description` is gone there, `explain` reports from state when no live bind has
  a `uq:` description, and the PR says so.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS, including
  every earlier slice's tests.
- [ ] **Step 5:** Commit (K5): `feat(uqd): shortcuts as a verified section, explained against Hyprland and user.lua`

### Task 9: uqd: pausing shortcuts for key capture (`PauseShortcuts`, `ResumeShortcuts`)

**Files:**
- Create: `crates/uqd/src/capture.rs` (with its tests).
- Modify:
  - `crates/uqd/src/lib.rs` (`pub mod capture;`);
  - `crates/uqd/src/dbus.rs` (additive). The broker lives on `DesktopIface` beside
    `SharedDesktop`, as slice C's pick broker does, so waiting never holds the desktop lock.
    It adds `pause_shortcuts() → s` and `resume_shortcuts(s)`.
  - `crates/uq-core/src/client.rs` (additive): both proxy methods.
  - `crates/uqd/tests/dbus.rs`.

**Interfaces:**
- Produces:
  - `pub const CAPTURE_TIMEOUT: Duration = Duration::from_secs(15)`.
  - `pub struct CaptureBroker`, with:
    - `with_timeout(d)`;
    - `async fn pause(&self, comp: &dyn Compositor) -> Result<String, DesktopError>`. It evals
      `hl.dispatch(hl.dsp.submap("uq_capture"))`, returns a token from
      `monitors::new_token()`, and arms the timer. A second pause replaces the token and
      re-arms.
    - `async fn resume(&self, comp, token) -> Result<(), DesktopError>`. It evals
      `hl.dispatch(hl.dsp.submap("reset"))`. A wrong token is `InvalidArgument`, and nothing
      is dispatched.
  - When the timer fires it resets the submap itself. With Hyprland down, `pause` returns
    `DesktopError::Io("Hyprland isn't answering")`, and the page falls back to typing keys.

- [ ] **Step 1: Write the failing tests** (`tokio::time::pause`; uqd's dev-dependencies
  already have `test-util`, slice D's F10):
  - `capture_pause_then_resume`: two evals, in order.
  - `capture_times_out_after_15s`.
  - `capture_second_pause_replaces_first` (the old token is refused).
  - `capture_forged_token_refused`.
  - `capture_never_holds_the_desktop_lock` (a `SetSection` during a pause goes through).
  - `capture_hyprland_down_is_plain_error`.
  - In `tests/dbus.rs`: `pause_and_resume_over_dbus`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd capture -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(uqd): pause shortcuts for up to 15 s while a new one is pressed`

### Task 10: `uqctl keys list` and `uqctl keys reset`

**Files:**
- Create: `crates/uqctl/src/keys_cmd.rs` (parsing, the write it makes, and printing; pure and
  tested, as `profile_cmd.rs` is).
- Modify: `crates/uqctl/src/main.rs`: `Keys { #[command(subcommand)] cmd: KeysCmd }`, and its
  dispatch.

**Interfaces:**
- Produces:
  - `KeysCmd::{List, Reset { id: Option<String>, #[arg(long)] all: bool }}`.
  - `pub fn reset_section(section: &Value, target: ResetTarget) -> Result<Value, String>`
    (pure).
  - **`list`** prints one line per bind: `<id>\t<combo or -> \t<label>`, then the marked
    `user.lua` binds as `user.lua:<line>\t<combo>\t<text>`. All text is cleaned of control
    characters.
  - **`reset <id>`** reads `GetSection("keybinds")`, drops that override and writes
    `SetSection` with origin `uqctl`. With no override it says "<id> already uses uQ's
    default." and exits 0. An unknown id exits 1 with a plain message.
  - **`reset --all`** drops every override; your own shortcuts are kept.
  - It never reads or writes `custom` beyond passing it through unchanged.

- [ ] **Step 1: Write the failing tests** in `keys_cmd.rs`:
  - `parses` (`keys list`, `keys reset uq.close`, `keys reset --all`; `keys reset` with
    neither is a usage error);
  - `reset_one_drops_only_that_override`;
  - `reset_all_keeps_custom`;
  - `list_prints_plain_rows` (a name with `\u{1b}` is cleaned);
  - a proxy test on a private bus with uqd's `serve` (as `profile_cmd.rs`'s):
    `reset_journals_with_origin_uqctl`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqctl keys -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqctl -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5): `feat(uqctl): list shortcuts and put them back from a terminal`

### Task 11: uq-settings: the Keyboard shortcuts page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/keybinds.rs`;
  - `apps/uq-settings/src/pages/keybinds/capture.rs` (the Qt key table);
  - `apps/uq-settings/src/pages/keybinds/tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/binds.rs`: `list_binds` through `keybinds::list`;
    `explain("keybinds")` through `keybinds::explain(…, None, …)` plus `Explain.overrides`;
    and `pause_shortcuts`/`resume_shortcuts`, recording calls with a `fail_pause` switch.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod keybinds;`);
  - `apps/uq-settings/src/pages/fake_uqd.rs` (the methods above and `Rig.binds`; the section
    goes through the generic section write).

**Interfaces:**
- Consumes: Tasks 2–9 (`combo`, `Keybinds`, `conflict`, `list`, `explain`), and
  `runner::{PageLogic, parse_json}`.
- Produces:
  - `pub enum Op`:
    - `Refresh`;
    - `SetCombo { id, combo }`;
    - `SetEnabled { id, on }`;
    - `Reset { id }`;
    - `ResetAll`;
    - `SaveCustom { bind: Value }` (an add when it has no id);
    - `DeleteCustom { id }`;
    - `PauseCapture`;
    - `ResumeCapture`.

    Every write reads the section, edits it, and writes it with origin `settings` under the
    write lock.
  - `pub struct KeybindsState` (a `PageLogic`), built with `new(conn)`.
  - `pub fn check(&self, draft: &Value) -> Value`: synchronous and pure, with no D-Bus (as
    slice C's F17 does). It validates a draft (`{id?, combo, action?, locked?, repeating?}`)
    against the cached section with `Keybinds::from_section`, and gives
    `{valid, error, conflicts: [{id, label}], user_lua: [{line, text}], combo_text}`. This is
    "the editor warns as you type" (R6).
  - `pub fn combo_from_qt(key: i32, modifiers: u32) -> Option<String>` (`capture.rs`):
    - The Qt `Key_*` codes map to `KEYS` spellings: letters, digits, F-keys, `Key_Return` and
      `Key_Enter` → `Return`, `Key_PageUp` → `Prior`, `Key_VolumeUp` → `XF86AudioRaiseVolume`,
      and so on.
    - `Qt::MetaModifier`, `ShiftModifier`, `ControlModifier` and `AltModifier` map to the four
      modifiers.
    - A press of a modifier key alone gives `None` (keep waiting).
    - **Ruling:** the codes are copied as constants from Qt 6's `qnamespace.h` with a citation
      comment. There is no Qt dependency, so it runs in t7check.
  - **The snapshot:**
    ```
    { groups: [{id, label, binds: [{id, label, combo, combo_text, default_combo_text, enabled,
                is_default, changed, can_disable, action_text, source, line, profile}]}],
      custom: [{…same, name, locked, repeating}],
      user_lua: [{line, combo_text, text}],
      section: {source, message} | null,
      capture: {seq, state: "idle" | "paused" | "unavailable", token},
      catalogue: {keys, actions}, limits, notice: {seq, text} }
    ```
    `source`, `line` and `profile` come from `Explain("keybinds")`.

- [ ] **Step 1: Write the failing tests** in `pages/keybinds/tests.rs`, against the fake uqd:
  - `rebind_disable_reset_round_trip` (§9's round-trip: reopening gives the same snapshot);
  - `reset_all_keeps_custom`;
  - `save_custom_gets_id_and_journals`;
  - `check_warns_as_you_type` (Review Focus 4):
    - `SUPER + Space` for a new custom bind gives `valid: false` and a conflict naming "Open
      Spotlight";
    - `WIN + space` gives the same;
    - a free combo gives `valid: true`;
    - none of these makes a D-Bus call;
  - `check_flags_marked_user_lua_without_refusing`;
  - `uqd_refusal_shows_plain_reason`: the fake's `refuse_next` (slice D's F32) produces the
    banner text without the `invalid value for <key>: ` prefix (slice D's F21);
  - `capture_pause_resume_and_unavailable`;
  - `combo_from_qt_maps_keys_and_modifiers`: Meta+Shift+Q gives `SUPER + SHIFT + Q`;
    Key_Space with Meta gives `SUPER + Space`; Key_Enter gives `Return`; Key_VolumeUp gives
    `XF86AudioRaiseVolume`; a lone Key_Meta gives `None`;
  - `profile_next_offered_unbound`;
  - `uqd_down_gives_empty_state_without_an_error`.
- [ ] **Step 2:** Run
  `cargo test -j 2 --manifest-path /tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/t7check/Cargo.toml pages::keybinds -- --test-threads=2`.
  Expected: FAIL.
- [ ] **Step 3:** Implement. No `cxx_qt` or Qt types in `pages/keybinds*`.
- [ ] **Step 4:** Re-run, and also run `pages::` as a whole in t7check. Expected: PASS. CI runs
  `cargo test -p uq-settings`.
- [ ] **Step 5:** Commit (K5): `feat(settings): keyboard shortcuts page logic with conflict warnings and key capture`

### Task 12: uq-settings: the Keyboard shortcuts page, its model and its registry entry

**Files:**
- Create:
  - `apps/uq-settings/src/keybinds_model.rs`. It's thin, like `rules_model.rs`:
    - `FOLLOW.sections = &["keybinds"]`, `FOLLOW.history = true`, `FOLLOW.profiles = true`;
    - invokables `refresh`, `setCombo`, `setEnabled`, `reset`, `resetAll`, `saveCustom`,
      `deleteCustom`, `check(json) → string` (sync), `comboFromKey(key, modifiers) → string`,
      `pauseCapture` and `resumeCapture`;
    - it calls `resumeCapture` on page close and on window deactivate.
  - `apps/uq-settings/qml/pages/KeybindsPage.qml`:
    - groups of `ShortcutRow`s;
    - **Your own shortcuts**, with **Add shortcut**;
    - **Put all back** (asks first: "Put all of uQ's shortcuts back to how they came? Your own
      shortcuts stay.");
    - the read-only "From user.lua" list;
    - every row's `SourceLine`/profile line.
  - `apps/uq-settings/qml/components/ShortcutRow.qml`: label, combo text, Change, a switch
    (disabled with a reason for `uq.lock-recovery`) and Reset (shown when changed).
  - `apps/uq-settings/qml/components/ShortcutEditor.qml`, a dialog:
    - a capture field ("Press the keys…"; shortcuts paused, R11);
    - manual pickers: four modifier switches and a key `UqComboBox` from `catalogue.keys`;
    - for your own shortcuts: a name field and an action picker by kind (from
      `catalogue.actions`), an app picker for `launch` (the existing desktop-entry list), and
      a single-line command field for `command`;
    - the switches "Also over the lock screen" and "Repeat while held", shown only when
      allowed;
    - a live warning `UqLabel` from `check()`. **Save is disabled while `valid` is false.**
  - `apps/uq-settings/qml/components/KeyCapture.qml`.
  - Tests:
    - `tests/settings-qml/tst_shortcut_row.qml`;
    - `tests/settings-qml/tst_shortcut_editor.qml` (the conflict warning text is shown, by
      `objectName`, Save is disabled, and the warning clears when the combo changes;
      slice C's F26);
    - `tests/settings-qml/tst_key_capture.qml`;
    - a fixture `tests/settings-qml/fixtures/keybinds_catalogue.json`.
- Modify:
  - `apps/uq-settings/build.rs` (the four QML files and `src/keybinds_model.rs`).
  - `apps/uq-settings/src/main.rs` (`mod keybinds_model;`).
  - `apps/uq-settings/src/pages/keybinds.rs`: remove the dead-code attribute.
  - `apps/uq-settings/src/registry.rs` (**additive**):
    - `KEYBINDS_SETTINGS`: `shortcut-list` "Keyboard shortcuts" `[keys, keybinds, hotkeys,
      bind]`; `add-shortcut` "Add a shortcut" `[custom, command, launch]`; `reset-shortcuts`
      "Put shortcuts back" `[reset, default]`; `next-profile-shortcut` "Switch to the next
      profile" `[profile, switch]`; `shortcut-capture` "Press keys to set a shortcut"
      `[record, capture]`.
    - A `PageInfo` row `keybinds` (R13) with `domains: &["keybinds"]`, placed after `history`,
      the last uQ-category row.
    - `CORE_PAGES` becomes `[PageInfo; 12]`.
    - Remove Task 4's skip.
    - Update only the index assertions this shifts in `pages_json_lists_pages_in_order`
      (System moves from `v[10]` to `v[11]`).
    - New test `keybinds_page_registered_with_search_entries`.
  - `Sidebar.qml` and `Main.qml`: no change expected (pages come from `pages_json`). If one is
    needed, it must be additive.

- [ ] **Step 1: Write the failing tests:**
  - the three QML tests above (CI);
  - `keybinds_page_registered_with_search_entries`, plus the updated registry tests (t7check:
    `cargo test -j 2 --manifest-path <t7check>/Cargo.toml registry -- --test-threads=2`).
- [ ] **Step 2:** Run the registry tests in t7check. Expected: FAIL.
- [ ] **Step 3:** Implement. All text goes through `UqLabel`. No internal names appear in
  labels (§9): combos are shown through `combo_text`, and the action kind names are
  "Open a uQ panel", "Window", "Workspace", "uQ", "Start an app" and "Run a command". Raw ids
  (`uq.spotlight`) are shown only inside `AdvancedDetails`.
- [ ] **Step 4:** Run t7check (`registry`, `search`, `pages::`) and
  `cargo test -j 2 -p uq-core -p uqd -p uqctl -- --test-threads=2`. Expected: PASS. Where Qt
  is installed (else CI, K4): `cargo test -j 2 -p uq-settings -- --test-threads=2`, the
  settings QML suite and `scripts/lint-settings.sh`.
- [ ] **Step 5:** Commit (K5): `feat(settings): Keyboard shortcuts page with rebinding, reset, your own shortcuts and key capture`

### Task 13: profiles turn shortcuts on and off

This task follows slice C's Task 11 (rules), with R15's limits.

**Files:**
- Create:
  - `crates/uq-core/src/profile/keybinds.rs` (helpers and tests);
  - `crates/uq-core/src/profile/keybinds/tests.rs` if it passes 400 lines.
- Modify:
  - `crates/uq-core/src/profile/mod.rs`:
    - `DomainKind::Keybinds`; `PROFILE_DOMAINS` maps `("keybinds", DomainKind::Keybinds)`.
      `Reserved` and `NeedsNewer` stay for future domains but are no longer used by any row.
    - `validate` checks the `[keybinds]` tree with `keybinds::check_tree`.
    - `read_leaf`/`write_leaves` dispatch to `enabled_of`/`set_enabled`. `write_leaves` runs
      `Keybinds::from_section`, so a conflict is a `SettingError` with R6's message.
    - `key_label("keybinds", [_])` is "A shortcut".
    - The test at `:762` changes from `NeedsNewer` to: `[keybinds]\n"uq.spotlight" = false`
      parses.
  - `crates/uq-core/src/profile/rules/tests.rs:85`: the `[keybinds]` case now parses.
  - `crates/uq-core/src/profile/overlay.rs`:
    - `apply` skips unknown `k-` ids (no write, no memo);
    - `turn_off` re-enables a bind only when that makes no conflict (otherwise it goes in Kept
      as "left off: its keys are now used by “X”") and skips binds that are gone;
    - `changed_since` sends `keybinds` entries through `bind_toggles`;
    - `from_journal` maps `"keybinds"` to `None`.
  - `crates/uqd/src/profiles.rs` (`missing_binds_notice` added to `notices`) and
    `crates/uqd/src/keybinds.rs` (`explain_keybinds` calls `profile::mark_binds`).
  - `apps/uq-settings/src/pages/profiles.rs` and `profiles_model.rs`:
    - `current` reads `GetSection("keybinds")` and offers every bind's switch with
      `bind_keys`;
    - the page list includes Keyboard shortcuts;
    - `FOLLOW.sections` gains `"keybinds"`.
  - `apps/uq-settings/src/pages/profiles/tests.rs:502`: `[keybinds] terminal = true` is still
    refused, now with "isn't one of uQ's shortcuts".
  - `apps/uq-settings/src/pages/fake_uqd/profiles.rs` and `fake_uqd.rs` (the notice, and
    `mark_binds`).
  - `apps/uq-settings/src/pages/keybinds.rs` rows gain `profile`; `KeybindsPage.qml` shows
    "From the X profile" under a switch, as the rules page does.

**Interfaces:**
- Produces (`uq_core::profile`, re-exported from `profile/keybinds.rs`):
  - `enabled_of(state, id) -> Option<bool>` (a default's override, else true; a custom bind's
    `enabled`).
  - `set_enabled(state, id, on) -> bool`. For a default, it writes or removes an override's
    `enabled`.
  - `check_tree(tree) -> Result<(), ProfileError>`:
    - every key is a default id or a `valid_custom_id`, and every value a bool (R15);
    - `"uq.lock-recovery" = false` is refused (R8);
    - the reason for anything else is "a profile can only turn a shortcut on or off";
    - keys in errors are cleaned with `plain`.
  - `bind_keys(kb) -> Vec<Key>` and `bind_label(kb, id) -> String`. The label is the default's
    label, "Shortcut “<name>”", or "A shortcut that isn't on this computer".
  - `missing_binds(state, profile) -> usize` and `missing_binds_notice(n) -> String`.
  - `bind_toggles(before, after) -> (Vec<String>, Vec<String>)`.
  - `mark_binds(rows, kb, active)`.

- [ ] **Step 1: Write the failing tests:**
  - In `profile/keybinds.rs`:
    - `keybinds_tree_validation` (Review Focus 7): `"uq.spotlight" = false` and
      `"k-3f9c" = true` parse and round-trip. Each of these is refused with a plain one-line
      message:
      - `"uq.spotlight" = "SUPER + K"`;
      - `[keybinds."k-3f9c"]\ncommand = "x"`;
      - `"uq.nope" = true`;
      - `"k-ZZ" = true`;
      - `"uq.lock-recovery" = false`;
      - a key holding `\u{1b}`.
    - `read_and_write_bind_leaves`;
    - `bind_keys_and_labels`;
    - `bind_toggles_classifies`;
    - `mark_binds_only_overrides_uq_and_default`.
  - In `profile/overlay.rs`:
    - `apply_skips_unknown_custom_ids`;
    - `profile_conflicts_refuse_the_apply` (a profile turning `uq.spotlight` back on while a
      custom bind holds `SUPER + Space` is refused with both names, and nothing changes);
    - `profile_conflicts_turn_off_keeps_bind_off` (K3: the profile turned `uq.close` off, and
      the user then put a custom bind on `SUPER + Q`. Back to normal leaves `uq.close` off and
      lists it in Kept with the custom bind's name);
    - `from_journal_maps_domains` gains `"keybinds"` → `None`.
  - In `crates/uqd/src/profiles/tests.rs`: `profile_turns_a_shortcut_off_and_back` (journal
    origin `profile:<id>`, summary "“Close the window” turned off", and the rendered file lacks
    the `uq:uq.close` line, then has it again).
  - In `apps/uq-settings/src/pages/profiles/tests.rs`: `shortcuts_offered_in_add_settings`,
    `create_from_keybinds_page` and `import_and_export_keybinds_round_trip` (t7check).
  - In `pages/keybinds/tests.rs`: `row_says_from_the_profile`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core profile -- --test-threads=2` and
  `cargo test -j 2 -p uqd profile -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -p uqd -p uqctl -- --test-threads=2` and
  t7check `pages::profiles` and `pages::keybinds`. Expected: PASS, every slice B, C and D
  profile test included.
- [ ] **Step 5:** Commit (K5): `feat: profiles turn keyboard shortcuts on and off`

### Task 14: docs, changelog, live checks and the whole gate

**Files:**
- Modify:
  - `README.md`:
    - Replace the `user.lua` snippet for `uqctl profile next` (`:193`) with "set **Switch to
      the next profile** on the Keyboard shortcuts page".
    - In the Profiles paragraph, `[keybinds]` is no longer refused: it holds shortcut on/off
      switches only.
    - A "#### Keyboard shortcuts page" paragraph covering:
      - the list of defaults, with the keys and labels taken from the table;
      - rebind, turn off, reset and "Put all back";
      - your own shortcuts and their actions;
      - commands running in their own scope;
      - the `locked` and `repeating` limits (R7);
      - the lock recovery bind staying on (R8);
      - conflicts refused;
      - `-- uq:bind` for marking `user.lua` binds;
      - key capture pausing shortcuts for up to 15 s;
      - `uqctl keys list|reset`;
      - that `base.lua` keeps fallback copies (R1).
    - Add `ListBinds`, `PauseShortcuts` and `ResumeShortcuts` to the Settings D-Bus paragraph,
      and say `keybinds` uses `GetSection`/`SetSection`.
  - `CHANGELOG.md` (additive): `## [Unreleased]` → `### Added`, an entry **Keyboard
    shortcuts.** It names the page, rebinding and reset, your own shortcuts (launch via
    `uqctl launch`, commands in a scope), conflict checks, `uq.profile.next`, profiles holding
    shortcut switches, `uqctl keys`, and the new D-Bus calls.
  - `docs/superpowers/specs/2026-11-08-uq-phase3-settings-design.md`:
    - §2.3: `BindsChanged` becomes `SectionChanged("keybinds")`, plus
      `PauseShortcuts`/`ResumeShortcuts`;
    - §2.4: the fallback ruling R1 and the `description` marker R5.
  - `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` §4: what a profile
    holds of `keybinds` (R15).
  - `docs/superpowers/plans/2026-11-09-uq-road-to-1.0.md`: tick slice 4 in M1.
- Create: `docs/notes/settings-slice4-live-check.md`.

- [ ] **Step 1:** Update the README and the specs as listed.
- [ ] **Step 2:** Add the CHANGELOG entry.
- [ ] **Step 3:** Write `docs/notes/settings-slice4-live-check.md` in the style of slice C's.
  It runs on real Hyprland 0.56.2 and on 0.57.x, with `user.lua`'s modification time noted at
  the start and the same at the end. Tick each item:
  1. **Upgrade keeps every shortcut.** Install the package over the previous one while logged
     in. Every default still works before and after `systemctl --user restart uqd`, and
     `hyprctl binds -j | grep -c '"uq:'` is 54. Move `generated.lua` aside and run
     `hyprctl reload`: the fallback shortcuts still work. Put it back.
  2. **Rebind.** Change Open Spotlight to Super + K. Super + K opens it at once, and
     Super + Space does nothing. **Reset** brings Super + Space back. History shows both lines
     plainly.
  3. **Conflict.** Add a shortcut on Super + Q. The editor names "Close the window" before
     Save, and Save stays disabled. `busctl --user call org.uq.Desktop1 … SetSection` with the
     same JSON is refused with both names.
  4. **Your own shortcuts.** Add Super + Shift + P → Start an app (Firefox): it starts in its
     own scope, with its Apps and scaling fix applied. Add Ctrl + Alt + T → Run a command
     (`foot`): `systemctl --user status 'run-*.scope'` shows it under `app.slice`. A command
     holding `'` and `$(…)` runs literally.
  5. **Key capture.** Press **Change** on any shortcut and press Super + Space. The editor
     shows "Super + Space" and Spotlight doesn't open (R11). Wait 15 s without pressing
     anything: shortcuts work again. Record whether the empty submap passed the keys on
     0.56.2 and on 0.57.x.
  6. **Lock screen.** Lock with Super + L. The volume keys work over the lock (OSD shows), and
     Super + Space doesn't. The recovery shortcut's switch is disabled with its reason. Kill
     the lock client: Super + Ctrl + L brings it back.
  7. **Switch to the next profile.** Give it Super + Alt + P. With two profiles, each press
     switches profile.
  8. **`user.lua`.** Add `hl.unbind("SUPER + Q")` to `user.lua` and reload. The Close row
     says "Overridden in user.lua, line N". Add
     `hl.bind("SUPER + T", hl.dsp.exec_cmd("foot")) -- uq:bind Terminal (foot)`: it's listed
     read-only, and a uQ shortcut on Super + T warns but saves.
  9. **Profiles.** Make a profile that turns off Close the window. Apply it: Super + Q does
     nothing, and the row says "From the … profile". Back to normal brings it back. Import a
     hand-edited profile with `"uq.spotlight" = "SUPER + K"` or `[keybinds."k-1234"] command`:
     it's refused plainly.
  10. **Terminal.** `uqctl keys reset --all` puts every override back and keeps your own
      shortcuts. `uqctl keys list` shows every row.
  11. **App menu tap.** Tapping Super alone still opens the app menu, now rendered from
      `generated.lua`.
  12. **Undo and restore.** Undo a rebind and an added shortcut from History, and restore
      yesterday's point. Each works live with no relog.
- [ ] **Step 4:** Run the full gate, exactly as CI does:
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features --locked -- -D warnings`;
  - `cargo test -j 2 --workspace --locked -- --include-ignored --test-threads=2`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh`;
  - both `qmltestrunner` suites.

  In the build container (K4), with `CARGO_BUILD_JOBS=2 CARGO_INCREMENTAL=0`:
  - run the cargo steps with `--exclude uq-settings` and without `--include-ignored`;
  - run `pages::`, `registry` and `search` through t7check;
  - leave `uq-settings`, the QML suites, the Lua-mock tests, the Quickshell-based shell tests
    and the `--ignored` Hyprland verify to CI.

  Expected: all PASS. CI's `all_variants_pass_hyprland_verify` is the check that every key
  name and dispatcher form is one Hyprland accepts.
- [ ] **Step 5:** Write the PR body's "Spec deviations" section from the list near the top of
  this plan, and list the notes questions below. Commit (K5): `docs: Phase 3 slice 4 — keyboard shortcuts in uqd and the Keyboard shortcuts page`

## Notes questions (for the user, answered in the PR)

1. Should **Open a terminal** follow the Shell page's terminal (`shell.terminal`, via
   `uqctl launch`) instead of `kitty`? R4 keeps `kitty` so this slice changes no default.
2. Does an empty submap pass keys through to the app on 0.56.2 and 0.57.x (R11)? If it
   doesn't, capture falls back to the manual pickers, and the button is hidden.
3. Should uQ also offer `long_press` or `non_consuming` for your own shortcuts? v1 offers only
   `locked` and `repeating`.
