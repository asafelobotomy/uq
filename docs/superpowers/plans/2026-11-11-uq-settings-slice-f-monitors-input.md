# uQ Settings enhancements slice F: monitor profiles, multi-monitor extras and touchpad gestures — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Displays and Input pages in `uq-settings` gain what multi-screen laptop users and
touchpad users ask for most.
- **Display arrangements that switch on their own.** After a display change is kept, the
  Displays page offers "Remember for these screens". Each saved arrangement is matched by the
  make, model and serial of every connected screen, never by port. When screens are plugged in or
  out, uqd waits 1 s, finds the arrangement for exactly that set, and applies it, with no prompt
  and no 15 s revert. "Switch arrangements when screens change" turns this off.
- **Main display.** One screen is the main one: notifications, the volume and brightness
  pop-ups and Spotlight open there, the session starts there, and X11 apps see it as the
  primary output (`xrandr --primary`, when `xrandr` is installed).
- **One wallpaper across all screens.** A new wallpaper fit, "Span across displays", draws one
  image over every screen at their real positions; each screen draws its own part.
- **Workspaces per screen.** A planner puts workspaces 1–N on screens one after another, in
  turn, or by hand, and moves them back when a screen returns.
- **Bar per screen.** The bar shows on every screen, the main one only, or the ones you pick.
- **Touchpad gestures** on the Input page: a list of gestures (fingers, direction, action,
  optional modifier key), the workspace swipe options, and the two defaults uQ ships (three
  fingers sideways switches workspace; four fingers up opens Spotlight). Two gestures that
  Hyprland would let shadow each other are a conflict: the editor warns, and uqd refuses the
  save.

All new values are ordinary uQ settings: validated, saved through slice A's single save point
(`persist(next, origin)`), journalled with plain labels, covered by restore points, and
explained where Hyprland can report them. None of it writes `user.lua`.

**Architecture:**
- **uq-core:**
  - `conflict.rs` (new, small, shared): `Mods` (a modifier set with one canonical spelling) and
    `clashes`, the trigger-conflict finder. **The keybinds slice, planned in another thread,
    reuses this module unchanged**; nothing in it knows about gestures.
  - `settings/monitors/` grows: `identity.rs` (`MonitorId`, matching a saved screen to a live
    output), `extras.rs` (the new keys `primary`, `auto_profiles`, `bar`, `workspaces`,
    `profiles`), `plan.rs` (the workspace planner, arrangement matching, the main display and
    bar roles; all pure) and `summary.rs` (history lines for the new keys).
  - `gestures/` (new): `hypr.rs` is the **one table** of Hyprland gesture names, checked
    against the pinned source; `mod.rs` is the model, defaults and validation; `InputSettings`
    gains `gestures` and `swipe`.
  - `lua/monitors.rs` gains `MonitorRender` (the planner's lines and `cursor.default_monitor`);
    `lua/gestures.rs` (new) renders `hl.gesture` lines and the `gestures` option table.
  - `history`: labels and summaries for every new key, and a new `Origin::Hotplug` that, like
    `Schedule`, is saved but never journalled.
- **uqd:**
  - `hypr_events.rs` (new) reads Hyprland's event socket (`.socket2.sock`) and debounces
    `monitoradded` / `monitorremoved` by 1 s.
  - `hotplug.rs` (new) is the matcher: apply the saved arrangement, re-plan workspaces, move
    planned workspaces back, notify once on a bad arrangement, and set the X11 primary.
  - `outputs.rs` (new) gathers the live facts a render needs (one `monitors all` read) and the
    roles the shell needs.
  - `notify.rs` (new) and `xrandr.rs` (new) are traits with a real implementation made only in
    `main.rs` and recording fakes in tests (slice D's F1 pattern).
  - `SetSection("monitors")` is accepted when the layout (`outputs`) is unchanged.
    `ApplyMonitors` keeps the stored non-layout keys. New `RememberMonitors(s name, s origin)
    → s id`. `ListOutputs` gains `roles` and `arrangement`.
  - The compositor gains `workspaces()` (`j/workspaces`) and `dispatch(lua)`.
- **uqctl bridge / shell:** the bridge forwards `MonitorsChanged` and lets the shell call
  `ListOutputs`. The shell gets `core/Outputs.qml` (roles, pure QML) and `logic/outputs.js`;
  `shell.qml`'s bar `Variants` follows the bar choice; notifications, the OSD and Spotlight use
  the main display when one is set; `Wallpaper.qml` / `Crossfade.qml` / `logic/wallpaper.js`
  draw the span crop.
- **uq-settings:** Qt-free page logic in `pages/displays/extras.rs` and
  `pages/input/gestures.rs`, tested against the fake uqd; the existing `displays_model.rs` and
  `input_model.rs` gain properties and invokables; new QML components `DisplayRoles.qml`,
  `DisplayArrangements.qml`, `WorkspacePlanner.qml`, `GestureList.qml`, `GestureEditor.qml`;
  new registry setting entries on the existing Displays and Input pages (no new pages).

**Tech Stack:** Rust edition 2024, zbus 5, tokio, cxx-qt 0.10, Qt 6, Kirigami 6, Quickshell,
Hyprland 0.56.2–0.57.x (Lua config), `xrandr` (optional).

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. The authority
is §4's "Monitor profiles" part, §7.1 (multi-monitor extras) and §7.2 (touchpad gestures). This
plan also covers:
- §2: Explain rows for the swipe options and a `user_lua` row for gestures set in `user.lua`;
- §3: labels, summaries and restore-point coverage for every new key; hotplug switches follow
  §3's rule for automatic changes (spec deviation 1);
- §9: the quality rules (plain names, two clicks, no silent failure, round trips);
- §10: the `uq_core::settings` row (`monitors.primary`, `monitors.auto_profiles`), the
  `uq_core::profile` row's "monitor-set matching" (here in `settings::monitors`, ruling R2),
  uqd's "hotplug matcher" and "`xrandr` primary", the shell's "span wallpaper, bar per monitor",
  and the testing lines "monitor-set matching across port swaps", "the fake-Hyprland socket
  gains … monitor events" and "the gesture conflict warning";
- §12 row F.

It builds on Phase 3 slice 3 (`2026-11-10-uq-phase3-slice3-displays-input.md`, merged; read its
"Forward compatibility with uq#8" section and its Outcome) and on slice B
(`2026-11-10-uq-settings-slice-b-profiles.md`, merged), and follows the layout of slices C and D
(`…-slice-c-rules.md`, `…-slice-d-startup-apps.md`). Slice D's R13 ("the main monitor is the
output at 0,0") says "Slice F's `monitors.primary` replaces this rule when it lands"; Task 4 does
that.

**Paths.** The settings app lives at `apps/uq-settings` (not `crates/`). Its QML tests are in
`tests/settings-qml`, and the shell's are in `tests/qml`.

**Ownership.** Another thread owns `crates/uq-core/src/integration/`,
`apps/uq-settings/src/plugins.rs`, `shell/widgets/`, `shell/quick/AppsGroup.qml`, `templates/`,
`web/uq-tokens/` and the later greeter work: this plan edits none of them. These shared files get
**additive edits only** (new lines, new match arms, new list entries; nothing renamed, moved or
reordered): `apps/uq-settings/src/registry.rs`, `apps/uq-settings/qml/Sidebar.qml` and
`Main.qml` (this slice needs no edit to either), uqd's `dbus.rs` and `dbus/signals.rs`,
`crates/uq-core/src/history/labels.rs` (`history::label`) and `history/mod.rs`, `CHANGELOG.md`,
`packaging/uq/PKGBUILD`.

## Pre-flight rulings

No conflict scan has been run against this plan yet. Before Task 1, run the same scan slices C
and D ran (`.superpowers/sdd/2026-11-11-uq-settings-slice-f-monitors-input/preflight.md`)
against `main`, fold each finding into the task text as an `F<n>` ruling listed here, and
re-check these points the author could not settle from the code alone:

- **P1, the keybinds slice.** If the keybinds plan has landed first and already added a conflict
  module, Task 1 adopts its module and its names instead of creating `conflict.rs`. Either way
  there is exactly one conflict helper in `uq_core`, and both slices call it.
- **P2, `hyprctl dispatch` at 0.57.** At v0.56.2, `src/debug/HyprCtl.cpp:1132` turns
  `dispatch <x>` into `return hl.dispatch(<x>)` under a Lua config. The file moved at v0.57.0
  and the author didn't find it. Find it and confirm the same before Task 6; if it changed, Task
  6's `dispatch` evals `hl.dispatch(<x>)` instead, and the fake records the same string.
- **P3, `--verify-config` and `hl.gesture`.** Confirm that `--verify-config` runs `hl.gesture`
  (it registers gestures through `g_pTrackpadGestures`, which may not exist in verify mode). If
  verify skips it, the conflict refusal in `gestures::validate` is the only guard, and the live
  check (item 11) is the gate for overshadow errors.
- **P4, quickshell `Variants` with a filtered model.** Confirm against the pinned Quickshell
  that a `Variants` whose `model` is a JS array of `ShellScreen`s (not `Quickshell.screens`
  itself) creates and destroys delegates by object identity, so bars on unaffected screens
  aren't rebuilt (Task 9). If it rebuilds all, accept it (bars are cheap) and say so in the PR.

## Plan rulings

- **R1, monitor profiles live in `state.toml`.** They are `monitors.profiles`, a list in the
  `monitors` section, not files beside the named profiles. So restore points and undo cover
  them, the Displays page reads them with `ListOutputs`/`GetSection`, and named profiles
  (slice B) stay as they are: a named profile still refuses `[monitors]` and `[input]`
  (`refuses_unknown_domain` keeps passing). The id is `m-` plus 4 hex digits
  (`rules::fresh_prefixed("m-", …)`), the name is 1–60 characters of plain text. At most 32
  arrangements, each of at most 16 screens.
- **R2, matching is in `settings::monitors`, not `profile`.** Spec §10 puts "monitor-set
  matching" in `uq_core::profile`. Monitor profiles share nothing with named profiles' overlay
  code, so the pure matcher sits next to the `Monitors` type in `settings/monitors/plan.rs`.
- **R3, a screen's identity is `MonitorId { make, model, serial }`** from `monitors all -j`
  (`LiveOutput`'s cleaned fields), each at most 64 characters, no control characters. Matching
  is by **multiset**: two identical screens with the same (or empty) serial are two entries.
  Each saved screen also keeps `hint`, the connector it was on when saved. Resolving a saved
  arrangement to today's connectors gives each saved screen a distinct live output with the
  same identity, preferring the one on its `hint`, then by connector name. Outputs named
  `FALLBACK` or `HEADLESS-*` are never part of a set.
- **R4, an exact set match only.** An arrangement applies when the multiset of connected
  screens (enabled or not) equals its multiset. "Remember" replaces the arrangement for the
  same set, keeping its id and name, so there's at most one per set.
- **R5, a hotplug switch writes the layout.** Applying an arrangement sets `monitors.outputs`
  for the connected connectors (entries for connectors that aren't connected are kept; Hyprland
  ignores them) and, when the arrangement has its own planner settings, `monitors.workspaces`.
  It goes through `commit_section("monitors", next, Origin::Hotplug)`: rendered, verified by
  Hyprland, saved, reloaded. The Displays page therefore always shows the stored layout, and
  undo, restore and "no settings that reset themselves" keep working unchanged.
- **R6, `Origin::Hotplug` is never journalled** and saves no restore point, exactly like
  `Origin::Schedule`. `persist` checks `matches!(origin, Origin::Schedule | Origin::Hotplug)`.
  `Origin::parse("hotplug")` is `Unknown`, so no D-Bus caller can claim it. A user's own
  "Remember", "Forget", main display, bar and planner changes are journalled as usual. Spec
  deviation 1; open question 1 in the notes.
- **R7, `SetSection("monitors")` is accepted when `outputs` is unchanged.** Every key but the
  layout (`primary`, `auto_profiles`, `bar`, `workspaces`, `profiles`) is written like any
  section. A write whose `outputs` differs from the stored one is still refused with "displays
  change through ApplyMonitors" (slice 3's single layout write path). `ApplyMonitors` parses its
  whole JSON strictly, then uses only `outputs` from it and keeps every other stored key, so
  the page's existing Apply (which sends the whole draft section) keeps working. Spec
  deviation 2.
- **R8, `RememberMonitors(s name, s origin) → s id`** builds the arrangement in uqd from the
  stored layout and the live identities (the page never sends identities for this), then
  writes it with `commit_section`. It is refused while a display change is waiting to be kept
  ("keep or revert the display change first"), when a connected screen has no stored entry and
  no live mode ("arrange these screens first"), and past 32 arrangements. A name is trimmed and
  cleaned (`clean_text_spaced`, 60 chars). Spec deviation 3.
- **R9, while a display change is waiting**, a hotplug event is held: uqd remembers that the
  set changed and runs the matcher once the change is kept, reverted or expires
  (`confirm_monitors`, `revert`, `expire_monitors` call `self.after_window()`).
- **R10, a bad arrangement is skipped.** If the resolved layout fails `stays_visible`,
  `Monitors::validate` or Hyprland's verify, nothing changes, and uqd sends **one** desktop
  notification per arrangement id until the connected set changes again: summary "Display
  arrangement not used", body "“Desk” couldn't be used with these screens: <Hyprland's message,
  cleaned, ≤ 200 chars>". `ListOutputs`' `arrangement.failed` is `true` so the page can say so.
  The notification goes to `org.freedesktop.Notifications` with app name "uQ" through
  `notify::Notifier`.
- **R11, the main display (`monitors.primary`).** `primary: Option<MonitorId>`.
  - **Resolved** (`plan::main_output`): the live, enabled, non-mirroring output with that
    identity; otherwise slice D's R13 rule (the enabled output at 0,0, else the first enabled
    by name).
  - **uqd** uses the resolved main output for slice D's main scale (`GDK_SCALE`,
    `STEAM_FORCE_DESKTOPUI_SCALING`) and, only when `primary` is set and found, renders
    `cursor = { default_monitor = "<connector>" }` (`cursor:default_monitor` exists at v0.56.2
    `ConfigValues.cpp:601` and v0.57.0 `:712`), so the session starts there and workspace 1
    opens there at login.
  - **The shell** uses `roles.primary`, which is `""` unless `primary` is set **and** found: with
    no main display chosen, notifications, the OSD and Spotlight keep following the focused
    screen as they do today. Spec deviation 4.
  - **The app menu** opened from a bar button stays on that bar's screen (it hangs from the
    button); quick settings, the power menu and the polkit dialog are unchanged. Spotlight
    opened by its key goes to the main display when there is one. Spec deviation 5.
  - **X11:** after any commit or hotplug that changes the resolved, explicitly chosen main
    connector, and once at startup, uqd runs `xrandr --output <connector> --primary` (absolute
    path found once in `main.rs`; argv, never a shell; 2 s timeout; `DISPLAY` from uqd's
    environment, which `base.lua` exports). Skipped when `xrandr` isn't installed, when
    `DISPLAY` is unset, in a `uq-dev session`, and when `primary` is unset. A failure is logged
    once per connector.
- **R12, the bar choice (`monitors.bar`).** `{show: "all" | "main" | "chosen", on: [MonitorId]}`,
  default `all`. `roles.bar` is `null` for all screens, else the list of connectors. **The shell
  never shows zero bars:** if `main` or `chosen` resolves to no connected screen, `roles.bar`
  is `null`. `main` uses the resolved main output (R11's fallback included).
- **R13, the workspace planner (`monitors.workspaces`)** is
  `{strategy: "off" | "sequential" | "interleave" | "manual", count: 1–20 (default 10),
  manual: [{workspace: 1–20, monitor: MonitorId}], move_back: bool (default true)}`. Default
  `off`, so nothing changes for anyone until they choose.
  - **Screens** for planning: the enabled, non-mirroring connected outputs, with the stored
    layout laid over them (as `main_scale_for` does), ordered left to right by `x`, then `y`,
    then name.
  - **sequential:** workspaces 1..=count split into contiguous blocks, the first
    `count % k` screens getting one more (10 on 2 → 1–5, 6–10; 10 on 3 → 1–4, 5–7, 8–10).
  - **interleave:** workspace `n` goes to screen `(n - 1) % k`.
  - **manual:** each listed workspace goes to its screen if connected; others aren't planned.
  - With fewer than two screens, nothing is planned.
  - The first workspace of each screen's share gets `default = true` (sequential and
    interleave only).
- **R14, the plan renders at render time, merged into the workspace rules.** It is not
  written into `rules.workspace` (spec §7.1 says "It renders `rules.workspace[].monitor`"; the
  rendered effect is the same, but storing it in `rules` would make the plan's output editable
  on the Rules page and the plan impossible to re-run on hotplug). `emit_generated` takes the
  plan as `StartupRender.monitors.planned`, and for each planned workspace either fills
  `monitor` and `default` into the user's own `rules.workspace` entry for that selector (fields
  the user set win, field by field) or adds a rule with only those two fields. One
  `hl.workspace_rule` call per selector stays true. Spec deviation 6.
- **R15, render inputs come from one live read.** `render_candidate` calls
  `Desktop::live_outputs_for(next)` once (the `monitors all` read with `next`'s stored layout
  laid over it, which today's `main_scale_for` does) and derives the main scale, the plan and
  the default monitor from it. With Hyprland down there is no plan and no default monitor, as
  slice D's R13 leaves out scale lines with no scale known. `StartupRender` gains one field,
  `monitors: MonitorRender` (`Default` is empty), so every existing
  `StartupRender::default()` call site still compiles.
- **R16, moving workspaces back.** After a hotplug that **added** a screen (and after the
  arrangement, if any, is applied), when `move_back` is on and the strategy isn't `off`, uqd
  reads `j/workspaces` and, for each planned workspace that exists and sits on another
  connector, sends `dispatch hl.dsp.workspace.move({ workspace = "<n>", monitor = "<connector>" })`
  (P2). Connectors pass `valid_output_name` and the line is built with `lua_string`. Errors are
  logged, never surfaced. Nothing is moved on a removal (Hyprland moves the workspaces of a
  gone screen itself).
- **R17, hotplug timing.** Each `monitoradded` or `monitorremoved` line restarts a 1 s timer;
  when it fires, uqd takes the desktop lock once and runs `Desktop::outputs_changed(added)`,
  where `added` is whether any event in the burst was an add. The `…v2` events are ignored
  (each v1 event comes with one). At uqd start, after the startup render, the matcher runs once
  (screens may have changed while uqd was stopped).
- **R18, what a hotplug does, in order** (`hotplug::outputs_changed`):
  1. If a display change is waiting, note it and return (R9).
  2. Read `monitors all`. If the connected set is the one last handled and no write happened
     since, return.
  3. If `auto_profiles` is on and an arrangement matches (R4), resolve it (R3). If the
     resolved layout equals the stored one, skip to 5.
  4. Write it (R5). On a refusal, notify once (R10) and keep going with the stored layout.
  5. If nothing was written and the planner is on, re-render with the same state
     (`commit(self.state.clone(), Change::Reload, Origin::Hotplug)`; nothing is journalled), so
     the plan follows the new set.
  6. Move workspaces back (R16) if a screen was added.
  7. Set the X11 primary (R11).
  8. Emit `MonitorsChanged`.
- **R19, gestures are part of the `input` section** (slice 3 reserved the `gestures` key):
  `input.gestures: Option<Vec<Gesture>>` and `input.swipe: SwipeOptions`.
  - `gestures` absent or `null` means **uQ's defaults** (`gestures::defaults()`: 3 fingers
    `horizontal` → switch workspace; 4 fingers `up` → Spotlight). A list, even an empty one, is
    exactly that list. Each entry has `enabled`, so "both can be turned off" means the page
    writes the defaults out with `enabled: false`.
  - A `Gesture` is `{fingers: 3–5, direction, action, modifier?, workspace?, enabled}`, at most
    24 entries. `workspace` is the special workspace's name for `special` only (default
    `magic`, the name `base.lua`'s SUPER+S uses; `[A-Za-z0-9_-]{1,32}`).
  - `SwipeOptions` is `{distance: 0–2000, invert: bool, create_new: bool,
    cancel_ratio: 0–1 (2 places)}`, each `Option` (`None` = Hyprland's default).
  - Named profiles don't hold gestures (spec §4's list doesn't include `input`).
- **R20, gestures render after the input devices** as a `-- Touchpad gestures` block, one
  `hl.gesture({ … })` per enabled gesture, in list order, and the swipe options as a
  `gestures = { … }` table inside the main `hl.config` (only the set keys). `base.lua`'s own
  `hl.gesture` block is **removed**, because Hyprland refuses a second gesture that a first one
  shadows (R21), so uQ's default would fail verify. uqd's startup render writes the defaults
  before the shell starts, so a fresh install still has the 3-finger swipe.
- **R21, conflicts follow Hyprland's own rule, both ways.** Hyprland's `addGesture` (v0.56.2
  and v0.57.0 `src/managers/input/trackpad/TrackpadGestures.cpp`) refuses a new gesture when
  an earlier one has the same fingers and modifiers and its direction equals the new one's or
  the new one's axis (`horizontal` shadows `left`/`right`, `vertical` shadows `up`/`down`).
  uQ's rule is the symmetric form: two **enabled** gestures with the same fingers and modifier
  conflict when their directions are equal, or one is the axis of the other. `pinch_in` and
  `pinch_out` don't conflict with each other. The spec's rule ("same fingers, direction and
  modifier") is the subset; spec deviation 7. Disabled gestures never conflict.
- **R22, `user.lua` gestures.** `user.lua` loads after `generated.lua`, so a gesture there that
  one of uQ's shadows is refused by Hyprland, and verify fails for every later uQ write. uQ
  can't fix that (it never writes `user.lua`). `Explain("input")` gives a `gestures` row with
  source `user_lua` and the first `hl.gesture(` line outside a comment
  (`strip_lua_comment`), and the Input page shows it with **Open user.lua**; the verify error,
  when it happens, is shown as Hyprland's message (§2 `error`).
- **R23, the conflict helper is generic.** `uq_core::conflict` holds `Mods` (SUPER, SHIFT,
  CTRL, ALT, as a set; rendered in that order, space-separated, which Hyprland's
  `modMaskFromString` reads) and `clashes<T: Trigger>`, where `Trigger` gives `overlaps(&self,
  &other)` and `describe(&self) -> String`. Gestures implement `Trigger` in `gestures/mod.rs`;
  keybinds will implement it in their own module. `conflict.rs` has no gesture or keybind
  knowledge and no Hyprland names. The page warns from the same `clashes` the validator runs.
- **R24, span is for "All displays" only.** `Fit::Span` (`"span"`) is accepted only on the
  `wallpaper.outputs."*"` entry ("Span works only for All displays"). Screens with their own
  entry don't take part. The span box is the bounding box of the screens that draw the `*`
  entry, in the logical coordinates Quickshell's `ShellScreen` reports (`x`, `y`, `width`,
  `height`); each screen's background draws the image `PreserveAspectCrop`-fitted to the box,
  offset by its own position. A slideshow with `span` picks the slide from the clock
  (`floor(now / interval) % n`, skipping failed slides forward by name order), so every screen
  shows the same slide. An older uqd loads `span` as `fill` (the loader is lenient), so a
  downgrade just fills each screen.
- **R25, no new crates.** Process spawning uses `tokio::process`; the notification is a zbus
  call; the event socket is `tokio::net::UnixStream`. `Cargo.lock` doesn't change.

**Spec deviations (recorded here and in the PR):**
1. **Hotplug switches aren't journalled (R6).** §3 records confirmed monitor changes; an
   automatic switch is treated like the schedule's automatic flips, which §3 never records.
   The user's own Remember, Forget and role changes are journalled.
2. **`SetSection("monitors")` is allowed when the layout is unchanged (R7).**
3. **One new D-Bus method, `RememberMonitors` (R8)**, and two new `ListOutputs` fields,
   `roles` and `arrangement`. The shell calls `ListOutputs` through the bridge.
4. **The shell uses the main display only when one is chosen (R11).** With none chosen,
   pop-ups keep following the focused screen.
5. **The app menu stays on the screen whose bar opened it (R11).**
6. **The planner is stored in `monitors.workspaces` and merged at render (R14)**, not written
   into `rules.workspace`.
7. **Gesture conflicts include axis overlap (R21)**, which Hyprland itself refuses.
8. **`pinch_in` / `pinch_out` render as Hyprland's `pinchin` / `pinchout` (Task 5's table).**
9. **No restore point before a hotplug switch (R6).** §3's "before a profile switch" applies to
   named profiles.
10. **"Span" is only for All displays (R24).**

## Global Constraints

These are copied from the spec and the user's standing rules, except the lines marked
**ruling**, which this plan decides.

### Process

- **K4, no Qt here.** The build container has no Qt, no Quickshell, no Lua and no Hyprland.
  These run in CI only: `cargo test -p uq-settings`, both `qmltestrunner` suites, the
  `tests/shell/*.sh` scripts that need Quickshell, and the `--ignored` Hyprland verify tests.
  - Every piece of logic that can be Qt-free is: the page logic in
    `apps/uq-settings/src/pages/displays/extras.rs` and `pages/input/gestures.rs`, and all of
    the matching, planning, conflict and rendering code in `uq-core`.
  - The shell's JS logic is pure (`shell/logic/outputs.js`, `shell/logic/wallpaper.js`) and
    `shell/core/Outputs.qml` imports no Quickshell module, so `tests/qml` covers them under
    `qmltestrunner` in CI.
  - Locally, run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -p uqd -p uqctl -- --test-threads=2`.
  - **Page logic locally (as in slices C and D).** The scratch crate at
    `/tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/t7check`
    symlinks `apps/uq-settings/src/pages`, `desktop_entries.rs`, `registry.rs`, `search.rs`,
    `plugins.rs`, `system_lists.rs`, `xkb_lists.rs` and the app's `tests/` folder, and builds
    them without Qt. Run
    `CARGO_INCREMENTAL=0 cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::displays -- --test-threads=2`
    (and `pages::input`, `registry`). This slice adds no top-level app module that page logic
    needs, so t7check's `lib.rs` doesn't change. If the folder is gone (a new container),
    rebuild it from that description. CI's `cargo test -p uq-settings` stays the authority.
- **Build parallelism (earlyoom).** Every cargo command in this plan uses `-j 2` and
  `CARGO_INCREMENTAL=0`, and test runs add `-- --test-threads=2`. Export `CARGO_BUILD_JOBS=2`
  and `CARGO_INCREMENTAL=0` in the shell before the first build. Never run two cargo commands
  at once.
- **K5, commits.** Every commit message ends with:
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc, so the trailers sit on their own lines after a blank
  line.
- **Process (as slices C and D):**
  - private `dbus-daemon` buses only (`uq_core::test_bus`), never the real session bus,
    `hyprctl`, `xrandr` or a real notification server; tests use the fakes below;
  - no test reads or writes the real home: every uqd rig builds its paths with
    `Paths::under(temp)`, and the event-socket tests put `hypr/<sig>/.socket2.sock` under a temp
    runtime dir;
  - `serve()` never spawns a process, never starts the event listener and never sends a
    notification; only `main.rs` makes `SystemXrandr`, `SessionNotifier` and the listener;
  - `cargo fmt`, `clippy -D warnings` and the tests must be clean **at every commit**;
  - keep new files at or under about 400 lines (`crates/uqd/tests/verify_config.rs` is already
    over: Task 4 moves its helpers to `crates/uqd/tests/common/verify.rs` before adding cases);
  - run builds in the foreground and leave nothing running.
- **Dead code.** uq-core's additions are `pub` in a library. The uq-settings page logic of
  Tasks 11 and 13 lives in a binary crate and is unused until Tasks 12 and 14, so
  `pages/displays/extras.rs` and `pages/input/gestures.rs` start with
  `#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]`
  (Task 14 for gestures), which those tasks remove.
- **CI flake.** `tests/shell/pam_stacks.sh` is known to flake in CI. If it's the only red job,
  re-run it. Every other failure is real.

### Hyprland names (Tasks 4 and 5 hold them)

- **Source.** The pinned series' own source, as slices C and D: Hyprland `v0.56.2` and
  `v0.57.0` (CI's `uq-hyprland-pin` check enforces 0.56.2–0.57.x). The author read, at both
  tags:
  - `src/config/lua/bindings/LuaBindingsConfigRules.cpp`, `hlGesture` (registered as
    `Internal::setFn(L, "gesture", hlGesture)`):
    - `fingers` is an int **2–9**; `direction` a string; `action` a string, a Lua function, or
      a table of `start`/`update`/`finish` functions; optional `mods` (string), `scale`
      (float), `disable_inhibit` (bool), `workspace_name`, `mode`, `zoom_level` (strings).
    - String actions: `workspace`, `resize`, `move`, `special` (with `workspace_name`),
      `close`, `float` (with `mode`), `fullscreen` (with `mode`), `cursor_zoom`, `scroll_move`,
      `unset`. uQ offers `workspace`, `move`, `resize`, `special`, `close`, `fullscreen`,
      `float`.
    - A single-function action runs once, when the gesture ends
      (`LuaFunctionGesture.cpp`, `end()`).
  - `src/managers/input/trackpad/TrackpadGestures.cpp`:
    - `dirForString` lower-cases and accepts `swipe`, `left`/`l`, `right`/`r`, `up`/`u`/`top`/`t`,
      `down`/`d`/`bottom`/`b`, `horizontal`/`horiz`, `vertical`/`vert`, `pinch`,
      **`pinchin`**/`zoomin`, **`pinchout`**/`zoomout`. There is no `pinch_in`.
    - `addGesture`'s overshadow rule (R21).
  - `src/config/lua/bindings/LuaBindingsDispatchers.cpp`: `hl.dsp.global(name)` returns a
    function (`dsp_global`) and `hl.dsp.workspace.move({ workspace, monitor })` exists at both
    tags.
  - `src/config/values/ConfigValues.cpp`: `gestures:workspace_swipe_distance` (Int 0–2000,
    default 300), `gestures:workspace_swipe_invert` (Bool, true),
    `gestures:workspace_swipe_cancel_ratio` (Float 0–1, 0.5),
    `gestures:workspace_swipe_create_new` (Bool, true) at both tags (v0.57.0 lines 487–493);
    `cursor:default_monitor` (String).
  - `src/output/Monitor.cpp`: `monitoradded>>NAME`, `monitoraddedv2>>ID,NAME,DESC`,
    `monitorremoved>>NAME`, `monitorremovedv2>>…` on the event socket (v0.56.2 lines 387–398,
    v0.57.0 lines 379–390).
- **Ruling, `_` never `-`.** Hyprland's Lua config spells every option name with `_`
  (`luaConfigValueName`); slice 3 shipped `["tap-to-click"]` and only CI's `--verify-config`
  caught it (`tap_to_click`). Every option name in this slice is written with `_`
  (`workspace_swipe_distance`, `default_monitor`), the table in `gestures/hypr.rs` has a test
  that no Lua name contains `-`, and Task 4's and Task 5's verify cases set every new option so
  CI checks each name.
- **Ruling, one place to fix.** Every gesture Lua name (directions, actions, field names,
  option names) lives in `crates/uq-core/src/gestures/hypr.rs`; `cursor.default_monitor` and
  the `hl.dsp.workspace.move` text live in `crates/uq-core/src/lua/monitors.rs`. If a live
  check fails, the fix is one row.
- **Rendered shapes:**
  ```lua
  hl.config({
      …
      gestures = { workspace_swipe_distance = 400, workspace_swipe_invert = false },
      cursor = { default_monitor = "DP-1" },
  })
  -- Touchpad gestures
  hl.gesture({ fingers = 3, direction = "horizontal", action = "workspace" })
  hl.gesture({ fingers = 4, direction = "up", action = hl.dsp.global("uq:spotlight") })
  hl.gesture({ fingers = 3, direction = "down", action = "special", workspace_name = "magic" })
  hl.gesture({ fingers = 4, direction = "pinchin", action = "float", mods = "SUPER" })
  ```
  Every string goes through `lua_string`. `float` and `fullscreen` render without `mode`
  (Hyprland's default, toggle).
- **Not confirmed from source** (each is a live-check item; CI's `--verify-config` covers the
  syntax, P3):
  - that a C function from `hl.dsp.global(…)` as `action` runs the shell's global shortcut on
    the gesture's end, once;
  - that a config reload clears earlier gestures, so re-rendering never meets "will be
    overshadowed" against uQ's own previous lines;
  - that `cursor:default_monitor` takes a connector name at startup only (it is documented
    "on startup"), and does nothing harmful on reload;
  - that XWayland names its RandR outputs after the Wayland connectors (`DP-1`), so
    `xrandr --output DP-1 --primary` works;
  - that `dispatch hl.dsp.workspace.move(…)` moves a workspace that already has windows (P2).

### The `monitors` section's new keys (§4, §7.1)

- **JSON shape** (`Monitors::FIELDS` grows from `["outputs"]` to six):
  ```json
  {
    "outputs": {"DP-1": {…}},
    "primary": {"make": "Dell Inc.", "model": "DELL U2720Q", "serial": "ABC123"},
    "auto_profiles": true,
    "bar": {"show": "all", "on": []},
    "workspaces": {"strategy": "sequential", "count": 10, "manual": [], "move_back": true},
    "profiles": [
      {"id": "m-7f2c", "name": "Desk",
       "outputs": [{"monitor": {"make": "…", "model": "…", "serial": "…"}, "hint": "DP-1",
                    "config": {"mode": "2560x1440@144", "position": {"x": 0, "y": 0}}}],
       "workspaces": null}
    ]
  }
  ```
- **Loading is lenient per key and per entry** (slice 3's rule): a bad `primary` is `None`, a
  bad `bar` is the default, a bad arrangement or manual slot is dropped with `tracing::warn!`,
  and none of it costs `outputs`. `Monitors::sanitised` repairs the new keys too (duplicate ids
  get new ids, as `rules::Rules::repaired` does; more than 32 arrangements are cut).
- **Writes are strict** (`Monitors::from_section`): unknown keys, wrong types, an id that isn't
  `m-` and 4–8 hex digits, two arrangements with the same id or the same screen multiset,
  more than 32 arrangements or 16 screens in one, a name that is empty, over 60 chars or has a
  control character, a `MonitorId` field over 64 chars or with a control character, an
  arrangement `config` that `OutputConfig::validate` refuses, a `count` outside 1–20, a manual
  workspace outside 1–count or listed twice, and more than 16 `bar.on` entries are refused,
  each with a plain one-line reason.
- **Defaults** (`Monitors::default()`): `primary: None`, `auto_profiles: true`,
  `bar.show: all`, `workspaces.strategy: off`, `count: 10`, `move_back: true`, no arrangements.
  `skip_serializing_if` keeps a default `state.toml` byte-identical to today's, so the default
  `generated.lua` golden and `state_round_trip` don't change.

### Gestures (§7.2)

- **Model:** R19. **Directions** (uQ name → Hyprland name): `horizontal` → `horizontal`,
  `vertical` → `vertical`, `left` → `left`, `right` → `right`, `up` → `up`, `down` → `down`,
  `pinch_in` → `pinchin`, `pinch_out` → `pinchout`.
- **Actions** (uQ name → rendered `action`, the directions it allows, its plain label):
  | uQ | Rendered | Directions | Label |
  |---|---|---|---|
  | `workspace` | `"workspace"` | `horizontal`, `vertical` | "Switch workspace" |
  | `move` | `"move"` | the six swipe directions | "Move the window" |
  | `resize` | `"resize"` | the six swipe directions | "Resize the window" |
  | `special` | `"special"`, `workspace_name` | all eight | "Show or hide the scratch workspace" |
  | `close` | `"close"` | all eight | "Close the window" |
  | `fullscreen` | `"fullscreen"` | all eight | "Make the window full screen" |
  | `float` | `"float"` | all eight | "Float the window" |
  | `uq:spotlight` | `hl.dsp.global("uq:spotlight")` | all eight | "Open Spotlight" |
  | `uq:quick-settings` | `hl.dsp.global("uq:quick-settings")` | all eight | "Open quick settings" |
- **Modifier:** `None` or one `Mods` set (R23) of SUPER/SHIFT/CTRL/ALT; the page offers none,
  SUPER, ALT, CTRL and SHIFT.
- **Refusals** (plain, keyed `input.gestures[i].<field>`): fingers outside 3–5; an unknown
  direction or action; a direction the action doesn't allow ("Switching workspace works only
  with a sideways or up-and-down swipe"); `workspace` set on any action but `special`, or a bad
  name; more than 24 gestures; a conflict (R21): "3 fingers, swipe left is already used by
  “Switch workspace” (3 fingers, sideways)". `swipe.distance` outside 0–2000 and
  `swipe.cancel_ratio` outside 0–1 are refused; the ratio is rounded to 2 places.
- **Plain labels** (`Gesture::describe`, used by conflicts, summaries and the page):
  "3 fingers, swipe sideways", "4 fingers, swipe up", "3 fingers, pinch in",
  "SUPER + 4 fingers, swipe left".

### Sources and history (§2, §3)

- **Explain (§2).**
  - `monitors`: no option rows, as today (Hyprland has no option for a layout to compare).
  - `input`: four new `provenance::OPTIONS` rows, `input.swipe.distance` →
    `gestures:workspace_swipe_distance` (setting id `gesture-swipe-distance`),
    `input.swipe.invert` → `gestures:workspace_swipe_invert` (`gesture-swipe-invert`),
    `input.swipe.create_new` → `gestures:workspace_swipe_create_new` (`gesture-swipe-create`),
    `input.swipe.cancel_ratio` → `gestures:workspace_swipe_cancel_ratio`
    (`gesture-swipe-cancel`), all `NullSource::Default`; `provenance::expected` learns them.
    The generic loop then gives `runtime` and `user_lua` (via `find_in_user_lua` on
    `gestures:workspace_swipe_distance`, which already matches `gestures.workspace_swipe_distance`
    and the table form) for free. Task 5 adds a skip for these four ids in the registry test
    `provenance_rows_name_registry_settings` (slice D's F7 pattern); Task 14 removes it.
  - `input`'s `gestures` row (R22): uqd adds it after the generic loop; `source` is
    `user_lua` with `{line, path}` when `gestures::find_in_user_lua(text)` finds an
    `hl.gesture(` outside a comment, else `uq` for a stored list and `default` for `null`.
    There's no `runtime` source for gestures (Hyprland has no request that lists them).
- **History (§3).** `monitors` and `input` are already journal domains. Every new leaf key
  gets a label in `uq_core::history::label` **in the task that adds the key**, because
  `every_key_a_summary_can_show_has_a_plain_label` needs it at once, and the test's
  `populated()` sets each new field:
  - `("monitors", "primary.make" | "primary.model" | "primary.serial")` → "Main display";
  - `("monitors", "auto_profiles")` → "Switch arrangements when screens change";
  - `("monitors", "bar.show")` → "Show the bar on"; `("monitors", "bar.on")` → "Screens with
    a bar";
  - `("monitors", "workspaces.strategy")` → "Workspaces across screens";
    `"workspaces.count"` → "Planned workspaces"; `"workspaces.manual"` → "Workspaces on each
    screen"; `"workspaces.move_back"` → "Move workspaces back when a screen returns";
  - `("monitors", "profiles")` → "Saved display arrangements";
  - `("input", "gestures")` → "Touchpad gestures"; `("input", "swipe.distance")` → "Swipe
    distance"; `"swipe.invert"` → "Reverse swipe direction"; `"swipe.create_new"` → "Swipe past
    the last workspace to make a new one"; `"swipe.cancel_ratio"` → "How far to swipe before it
    switches".
- **Summaries.** `history::summarize` (additive arms in `history/diff.rs`) hands `monitors` to
  `settings::monitors::summary(before, after, labels)` and `input` to
  `settings::input_summary(before, after, labels)`. Both write their own lines for the list and
  identity keys, then fall back to the generic leaf lines (`summarize_leaves`, the existing
  body moved into a `pub(crate)` function, unchanged) for the rest. They never print JSON:
  - "Main display set to Dell U2720Q" / "Main display cleared";
  - "Display arrangement “Desk” saved" / "… updated" / "… renamed to “Home”" / "… removed";
  - "Bar shown on every screen" / "… on the main display only" / "… on 2 chosen screens";
  - "Workspaces across screens: one after another" / "… in turn" / "… by hand" / "… off";
  - "Touchpad gesture added: 3 fingers, swipe down", "… removed: …", "… turned off: …",
    "Touchpad gestures back to uQ's defaults"; up to three, then "and N more".
- **Undo** puts back a whole list (`profiles`, `manual`, `gestures`), as for rules. Undo of a
  `monitors` entry keeps using `write_monitors` (refused while a change is waiting).
- **uqd's `domain_label`** already has nothing for `monitors` and `input` ("This setting"):
  add `"monitors" => "Displays"` and `"input" => "Input"` (additive arms) for the undo-conflict
  sentence.

### Text and safety (§9, §10)

- **`user.lua` is never written.** uqd reads it for `Explain` only.
- **Plain words (§9).** Every user-facing string passes `scripts/lint-settings.sh`.
  - **Ruling:** add `xrandr`, `socket2`, `hl.gesture`, `pinchin`, `pinchout`, `connector`,
    `workspace_swipe` and `default_monitor` to `scripts/settings-jargon.txt`.
  - Labels: "Main display" (not "primary"), "Show the bar on", "Remember for these screens",
    "Saved arrangements", "Switch arrangements when screens change", "Workspaces across
    screens" with "One after another" / "In turn" / "Choose for each" / "Off", "Move workspaces
    back when a screen returns", "Touchpad gestures", "Span across displays".
  - Connector names (`DP-1`) and make/model/serial appear only as secondary text under the
    existing "Details" disclosure (slice 3's rule); a screen is named by its description.
- **Untrusted text:** live output descriptions, make, model and serial; arrangement names;
  Hyprland's verify message; the `user.lua` line. All of it is shown through `UqLabel` (never a
  stock control's `text:`, a `Kirigami.InlineMessage`, a `ToolTip`, a `Kirigami.FormData.label`
  or a `Dialog.title`; slice C's F28). Dialog titles are fixed literals: "Remember these
  screens", "Rename arrangement", "Add a gesture", "Change gesture". The notification body is
  cleaned (`clean_text_spaced`, 200 chars).
- **Lua injection.** Connector names in `cursor.default_monitor`, workspace rules and the
  `dispatch` line pass `valid_output_name` and `lua_string`; gesture names come only from the
  table; the special workspace name passes its charset rule and `lua_string`; arrangement names
  and identities are never rendered.
- **The shell never matches hardware.** It reads connector names from `roles` only; uqd does
  every identity match.

## Review Focus

1. **Port swaps and twins.** The desk's two identical screens (same make and model, empty
   serial) swap ports, or the dock enumerates them in another order. The arrangement must
   still match and each saved layout must land on a distinct live screen, preferring the old
   port. → Task 3's `match_survives_port_swap`, `twins_resolve_to_distinct_outputs`,
   `twins_prefer_their_old_port`, `fallback_and_headless_never_count`.
2. **A dock that flaps.** Five add/remove events in 600 ms give one match, after the last event
   plus 1 s; a burst during a pending display change is held and handled once, after keep or
   revert. → Task 6's `burst_is_debounced`, Task 8's `hotplug_waits_for_pending_change`.
3. **A bad arrangement never leaves a black screen and never spams.** A saved arrangement that
   turns every connected screen off (hand-edited `state.toml`) or that Hyprland refuses is
   skipped; the stored layout stays; exactly one notification per arrangement until the set
   changes. → Task 8's `bad_arrangement_skipped_and_notified_once`,
   `arrangement_turning_all_off_is_skipped`.
4. **The layout keeps one write path.** `SetSection("monitors")` with a changed `outputs` is
   still refused; `ApplyMonitors` with a changed `primary` in its JSON keeps the stored one;
   a hotplug switch never runs while a change is waiting. → Task 7's
   `set_section_monitors_layout_still_refused`, `apply_keeps_stored_extras`.
5. **Hotplug switches don't flood History** and don't create restore points, but the user's
   own Remember does journal. → Task 8's `hotplug_switch_not_journalled`, Task 7's
   `remember_is_journalled`.
6. **Gesture conflicts are refused both ways and match Hyprland.** `horizontal` then `left`,
   `left` then `horizontal`, two `up`s with different modifiers (fine), `pinch_in` with
   `pinch_out` (fine), a disabled duplicate (fine). → Task 1's `clashes_are_symmetric`,
   Task 5's `conflicts_follow_hyprland`.
7. **Lua injection and names.** A connector or special workspace name with `"`, `\`, `)` or a
   newline never escapes its string; no rendered option name contains `-`. → Task 4's
   `plan_lines_escape_names`, Task 5's `no_lua_name_has_a_hyphen`,
   `special_workspace_name_refused_or_escaped`.
8. **The shell never loses its bar.** A bar choice of "main" with no screen found, or "chosen"
   screens all unplugged, shows the bar on every screen. → Task 9's `bar_never_empty`.
9. **No process or notification from tests, and `xrandr` gets argv only.** → Task 8's
   `xrandr_argv_only_and_skips`, and every rig uses the fakes.

---

### Task 1: `uq_core::conflict`: one trigger-conflict helper for gestures and keybinds

**Files:**
- Create: `crates/uq-core/src/conflict.rs` and `crates/uq-core/src/conflict/tests.rs`.
- Modify: `crates/uq-core/src/lib.rs` (`pub mod conflict;`).

**Interfaces (produced, `uq_core::conflict`):**
- `#[derive(Clone, Copy, PartialEq, Eq, Hash, Default, Debug)] pub struct Mods(u8)` with
  `pub const SUPER`, `SHIFT`, `CTRL`, `ALT`, `pub const NONE`;
  `pub fn parse(s: &str) -> Result<Mods, String>` (names separated by spaces or `+`, case
  insensitive, `CONTROL`/`WIN`/`LOGO` accepted as aliases, an unknown name or a repeat is an
  error); `pub fn lua(self) -> String` (`"SUPER SHIFT CTRL ALT"` order, space-separated, `""`
  for none); `pub fn label(self) -> String` (`"SUPER + SHIFT"`); `Serialize`/`Deserialize` as
  the `lua()` string (an empty string or absent is `NONE`).
- `pub trait Trigger { fn overlaps(&self, other: &Self) -> bool; fn describe(&self) -> String; }`.
- `#[derive(Debug, Clone, PartialEq)] pub struct Clash { pub first: usize, pub second: usize, pub text: String }`;
  `text` is `"<second.describe()> is already used by <first.describe()>"` (callers prepend
  their own action names when they have them).
- `pub fn clashes<T: Trigger>(items: &[(usize, &T)]) -> Vec<Clash>`: every pair `(a, b)` with
  `a` before `b` in the slice where `a.overlaps(b) || b.overlaps(a)`, in order. Callers pass
  only enabled items, with their list indexes.
- `pub fn first_clash<T: Trigger>(items: &[(usize, &T)]) -> Option<Clash>`.
- Module docs say: "Shared by touchpad gestures (`gestures`) and keybinds. Knows nothing about
  either; each implements `Trigger`."

- [ ] **Step 1: Write the failing tests** in `conflict/tests.rs`:
  - `mods_parse_and_render` (`"super + shift"` → `SUPER|SHIFT`, `lua()` is `"SUPER SHIFT"`,
    `label()` is `"SUPER + SHIFT"`; `"CONTROL"` is `CTRL`; `""` is `NONE`; `"HYPER"` and
    `"SUPER SUPER"` are errors);
  - `mods_serde_round_trip` (JSON `"ALT"` ↔ `ALT`, absent → `NONE`);
  - `clashes_are_symmetric` (a toy `Trigger` whose `overlaps` is one-directional, like
    Hyprland's axis rule: the pair is found whichever order the items come in);
  - `clashes_report_indexes_in_order` and `no_items_no_clash`.
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core conflict -- --test-threads=2`. Expected: FAIL (module missing).
- [ ] **Step 3:** Implement (about 120 lines).
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -- --test-threads=2` and
  `CARGO_INCREMENTAL=0 cargo clippy -j 2 -p uq-core --all-targets -- -D warnings`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): a shared trigger-conflict helper for gestures and keybinds`

### Task 2: `monitors` gains identities, the main display, the bar choice, the planner and saved arrangements

**Files:**
- Create:
  - `crates/uq-core/src/settings/monitors/identity.rs`;
  - `crates/uq-core/src/settings/monitors/extras.rs`;
  - `crates/uq-core/src/settings/monitors/summary.rs`;
  - `crates/uq-core/src/settings/monitors/extras_tests.rs`.
- Modify:
  - `crates/uq-core/src/settings/monitors.rs` (`Monitors` gains the five fields; `FIELDS` has
    six; `from_section`, `validate`, `sanitised` cover them; new `layout_eq`);
  - `crates/uq-core/src/settings/mod.rs` (re-exports);
  - `crates/uq-core/src/history/labels.rs` (the monitors labels in Global Constraints;
    `populated()` sets every new field);
  - `crates/uq-core/src/history/diff.rs` (`summarize` sends `monitors` to
    `settings::monitors::summary`; the generic body becomes `pub(crate) fn summarize_leaves`,
    unchanged);
  - `crates/uq-core/src/history/mod.rs` (`Origin::Hotplug`, `as_str` "hotplug"; `parse`
    doesn't produce it);
  - `crates/uq-core/src/settings/monitors/tests.rs` (existing strictness tests keep passing;
    `bad_output_entry_costs_only_itself` gains a bad `primary` and a bad arrangement).

**Interfaces (produced, `uq_core::settings`):**
- `pub struct MonitorId { pub make: String, pub model: String, pub serial: String }`
  (`Ord`, `Serialize`, `Deserialize`, `deny_unknown_fields` on write, lenient on load);
  `MonitorId::of(&LiveOutput) -> MonitorId`; `fn label(&self) -> String` ("Dell U2720Q", make
  and model, falling back to "A display"); `fn check(&self, key: &str) -> Result<(), SettingError>`;
  `pub fn counts_in_sets(name: &str) -> bool` (false for `FALLBACK` and `HEADLESS-*`).
- `pub enum BarShow { All, Main, Chosen }` (`"all" | "main" | "chosen"`);
  `pub struct BarChoice { pub show: BarShow, pub on: Vec<MonitorId> }`.
- `pub enum Strategy { Off, Sequential, Interleave, Manual }`;
  `pub struct ManualSlot { pub workspace: u8, pub monitor: MonitorId }`;
  `pub struct WorkspacePlan { pub strategy: Strategy, pub count: u8, pub manual: Vec<ManualSlot>, pub move_back: bool }`
  (`Default`: off, 10, empty, true).
- `pub struct SavedOutput { pub monitor: MonitorId, pub hint: String, pub config: OutputConfig }`;
  `pub struct MonitorProfile { pub id: String, pub name: String, pub outputs: Vec<SavedOutput>, pub workspaces: Option<WorkspacePlan> }`;
  `MonitorProfile::set(&self) -> Vec<MonitorId>` (sorted multiset);
  `pub fn valid_profile_id(id: &str) -> bool` (`^m-[0-9a-f]{4,8}$`).
- `Monitors { outputs, primary: Option<MonitorId>, auto_profiles: bool, bar: BarChoice, workspaces: WorkspacePlan, profiles: Vec<MonitorProfile> }`;
  `Monitors::layout_eq(&self, other: &Monitors) -> bool` (compares `outputs` only);
  `Monitors::with_layout(&self, outputs) -> Monitors`.
- `pub fn summary(before: &Value, after: &Value, labels: Labels) -> String` in
  `settings/monitors/summary.rs` (lines in Global Constraints).

- [ ] **Step 1: Write the failing tests** in `extras_tests.rs`:
  - `defaults_serialise_to_nothing` (a default `Monitors` serialises to `{"outputs":{}}`, so
    `state.toml` and the golden don't change);
  - `section_round_trips` (every new key set, `from_section(section())` is equal; §9 round
    trip);
  - `strict_refusals` (each refusal listed in Global Constraints, one assertion each, with a
    plain reason and the right key);
  - `lenient_load_costs_only_the_bad_key` (a `state.toml` with `primary = 3`, one arrangement
    with id `bogus`, one good arrangement and good `outputs`: outputs and the good arrangement
    load, `primary` is `None`);
  - `repaired_gives_duplicate_ids_new_ones` and `repaired_cuts_past_32`;
  - `identity_check` (65-char model refused; a control character refused; all-empty allowed);
  - `layout_eq_ignores_extras`;
  - `summaries_are_plain` (each line in Global Constraints; no `{`, `[`, `m-`, `make`);
  - in `history/labels.rs`, `every_key_a_summary_can_show_has_a_plain_label` now sees the new
    keys (it fails until the labels exist);
  - in `history/mod.rs`, `origin_parse_round_trip` gains `assert_eq!(Origin::parse("hotplug"), Origin::Unknown)`
    and `Origin::Hotplug.as_str() == "hotplug"`.
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core monitors history -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `monitors.rs` under about 400 lines by putting the new types
  and their checks in `extras.rs`.
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -p uqd -- --test-threads=2`
  and clippy. Expected: PASS (uqd compiles; its `persist` doesn't know `Hotplug` yet, which is
  fine because nothing sends it until Task 8).
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): main display, bar choice, workspace planner and saved display arrangements in the monitors section`

### Task 3: The pure matcher, resolver, planner and roles

**Files:**
- Create: `crates/uq-core/src/settings/monitors/plan.rs` and `plan_tests.rs`.
- Modify: `crates/uq-core/src/settings/monitors.rs` (`mod plan; pub use plan::*;`),
  `crates/uq-core/src/compat/launch.rs` (`main_scale` keeps its signature and becomes
  `plan::fallback_main(live).map(|o| o.scale)`, so slice D's tests keep passing).

**Interfaces (produced):**
- `pub fn overlay(live: &[LiveOutput], stored: &Monitors) -> Vec<LiveOutput>` (today's
  `main_scale_for` overlay, moved here: stored position, scale and on/off over the live ones).
- `pub fn connected_set(live: &[LiveOutput]) -> Vec<MonitorId>` (sorted multiset; skips
  `counts_in_sets == false`).
- `pub fn find_arrangement<'a>(m: &'a Monitors, live: &[LiveOutput]) -> Option<&'a MonitorProfile>` (R4).
- `pub fn resolve(saved: &[SavedOutput], live: &[LiveOutput]) -> Option<Vec<(String, OutputConfig)>>`
  (R3: each saved screen to a distinct connector; `None` when any can't be placed).
- `pub fn arranged(m: &Monitors, p: &MonitorProfile, live: &[LiveOutput]) -> Option<Monitors>`
  (R5: the resolved entries replace those connectors' entries; other entries kept; the
  arrangement's `workspaces`, when `Some`, replaces `m.workspaces`; descriptions refreshed from
  live).
- `pub fn remember(m: &Monitors, live: &[LiveOutput], name: &str, id: Option<String>, taken: &BTreeSet<String>) -> Result<MonitorProfile, SettingError>`
  (R8: one `SavedOutput` per connected screen, its `config` the stored entry or, with none, the
  live mode, position and scale; an existing arrangement for the same set keeps its id and, when
  `name` is empty, its name).
- `pub fn screens(m: &Monitors, live: &[LiveOutput]) -> Vec<LiveOutput>` (R13 order and filter,
  after `overlay`).
- `#[derive(Debug, Clone, PartialEq, Serialize)] pub struct Planned { pub workspace: u8, pub output: String, pub default: bool }`;
  `pub fn plan(p: &WorkspacePlan, screens: &[LiveOutput]) -> Vec<Planned>` (R13).
- `pub fn fallback_main(live: &[LiveOutput]) -> Option<&LiveOutput>` (slice D's R13, moved);
  `pub fn main_output<'a>(m: &Monitors, live: &'a [LiveOutput]) -> Option<&'a LiveOutput>` (R11).
- `#[derive(Debug, Clone, PartialEq, Serialize)] pub struct Roles { pub primary: String, pub main: String, pub bar: Option<Vec<String>> }`;
  `pub fn roles(m: &Monitors, live: &[LiveOutput]) -> Roles` (R11, R12; `primary` is `""`
  unless chosen and found).

- [ ] **Step 1: Write the failing tests** in `plan_tests.rs` (fixtures build `LiveOutput`s; no
  JSON from Hyprland is needed):
  - `match_survives_port_swap` (saved on DP-1/DP-2, live on DP-3/HDMI-A-1 with the same
    identities: found, resolved to the new ports);
  - `twins_resolve_to_distinct_outputs` and `twins_prefer_their_old_port` (Review Focus 1);
  - `subset_or_superset_is_no_match` (laptop + desk saved; laptop alone connected: none);
  - `disabled_screens_still_count` (a connected but disabled screen is in the set);
  - `fallback_and_headless_never_count`;
  - `arranged_keeps_other_connectors_and_takes_plan`;
  - `remember_uses_stored_then_live` and `remember_same_set_keeps_id_and_name`;
  - `sequential_split` (10 on 1, 2, 3 screens; 3 on 2; count 1 on 2), `interleave_split`,
    `manual_skips_absent_screens`, `defaults_are_first_of_each_share`,
    `one_screen_plans_nothing`, `screens_order_left_to_right_then_top`;
  - `main_output_prefers_primary_then_fallback` (primary disabled → fallback; primary
    mirroring → fallback);
  - `roles_bar_never_empty` (main with no screen; chosen all absent → `None`) and
    `roles_primary_empty_unless_chosen`;
  - `main_scale_unchanged_without_primary` (slice D's cases through `compat::launch::main_scale`).
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core plan -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement (pure; no I/O).
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -- --test-threads=2` and clippy. Expected: PASS, slice D's `compat` tests included.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): match display arrangements by screen identity, plan workspaces and resolve the main display`

### Task 4: Render the plan and the default monitor; the main scale follows the main display

**Files:**
- Modify:
  - `crates/uq-core/src/lua/monitors.rs` (`MonitorRender`, `emit_cursor`, `merge_planned`,
    `workspace_move_line`);
  - `crates/uq-core/src/lua/startup.rs` (`StartupRender.monitors: MonitorRender`, `Default`
    empty);
  - `crates/uq-core/src/lua/mod.rs` (`emit_generated` emits the `cursor` table inside
    `hl.config` when `default_monitor` is set, and passes `merge_planned(&rules.workspace,
    &startup.monitors.planned)` to the existing `emit_workspace_rules`; re-exports);
  - `crates/uqd/src/startup.rs` (`main_scale_for` becomes `live_outputs_for(next) ->
    Option<Vec<LiveOutput>>`; the scale comes from `plan::main_output`);
  - `crates/uqd/src/service.rs` (`render_candidate` reads `live_outputs_for` once and fills
    `main_scale` and `StartupRender.monitors` from it, R15);
  - `crates/uqd/tests/verify_config.rs` → move `repo`, `tree`, `which` into
    `crates/uqd/tests/common/verify.rs` (`mod common;`), unchanged; add the variant
    `planned_and_cursor` to the variant list (two screens, sequential plan, a user rule for
    workspace 3 with `gaps_in`, and `default_monitor = "DP-1"`), so `lua_mock_runs_every_variant`
    and CI's `all_variants_pass_hyprland_verify` check the shapes.
- Test: `crates/uq-core/src/lua/monitors.rs` (inline tests), `crates/uq-core/src/lua/rules_tests.rs`
  (one merge case), `crates/uqd/src/startup/tests.rs`.

**Interfaces (produced, `uq_core::lua`):**
- `#[derive(Debug, Clone, Default, PartialEq)] pub struct MonitorRender { pub planned: Vec<Planned>, pub default_monitor: Option<String> }`.
- `pub fn monitor_render(m: &Monitors, live: &[LiveOutput]) -> MonitorRender` (planned from
  `plan(…, &screens(m, live))`; `default_monitor` from `roles(m, live).primary` when non-empty).
- `pub fn merge_planned(rules: &[WorkspaceRule], planned: &[Planned]) -> Vec<WorkspaceRule>`
  (R14: user fields win field by field; added rules come after the user's, in workspace
  order).
- `pub fn workspace_move_line(ws: u8, output: &str) -> String`
  (`hl.dsp.workspace.move({ workspace = "3", monitor = "DP-1" })`, both through `lua_string`).
- uqd: `pub(crate) async fn live_outputs_for(&self, next: &State) -> Option<Vec<LiveOutput>>`
  (one `monitors all` read with `overlay`); `main_scale_for` is removed and its callers use
  `plan::main_output(&next.settings.monitors, &live).map(|o| o.scale)`.

- [ ] **Step 1: Write the failing tests:**
  - `default_render_unchanged` (`StartupRender::default()` gives today's golden exactly);
  - `cursor_table_when_primary_found` (`cursor = { default_monitor = "DP-1" },` inside the
    first `hl.config`, only then);
  - `planned_rules_merge` (user rule for `3` with `monitor = "HDMI-A-1"` keeps it; user rule for
    `4` with `gaps_in` gains `monitor` and keeps `gaps_in`; planned `1` and `6` get
    `default = true`; one call per selector);
  - `plan_lines_escape_names` (Review Focus 7: a connector `DP-1"x` can't come from
    `valid_output_name`, so the test feeds `merge_planned` a hand-built `Planned` with
    `"a\"b"` and checks `lua_string` escaping; `workspace_move_line` likewise);
  - in `crates/uqd/src/startup/tests.rs`: `main_scale_follows_primary` (primary on the 1.5
    screen at 2560,0 gives 1.5; unset gives the 0,0 screen's 1.25, slice D unchanged) and
    `render_reads_monitors_once` (the fake compositor records one `monitors` call per commit).
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core lua -- --test-threads=2` and `… -p uqd startup -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -p uqd -- --test-threads=2`
  and clippy. Expected: PASS (the `--ignored` verify runs in CI).
- [ ] **Step 5:** Commit (K5 trailers): `feat: render the workspace plan and the main display's cursor; the main scale follows the main display`

### Task 5: `uq_core::gestures`: the name table, the model, conflicts and rendering

**Files:**
- Create:
  - `crates/uq-core/src/gestures/mod.rs` (model, defaults, validation, `Trigger` impl,
    `summary`, `find_in_user_lua`);
  - `crates/uq-core/src/gestures/hypr.rs` (the one name table);
  - `crates/uq-core/src/gestures/tests.rs`;
  - `crates/uq-core/src/lua/gestures.rs` (`emit_gesture`, `emit_gestures`, `emit_swipe`).
- Modify:
  - `crates/uq-core/src/lib.rs` (`pub mod gestures;`);
  - `crates/uq-core/src/settings/input.rs` (`InputSettings.gestures: Option<Vec<Gesture>>`,
    `InputSettings.swipe: SwipeOptions`; `FIELDS` gains `"gestures"`, `"swipe"`; strict parse,
    lenient load per entry, `sanitised` drops bad entries and the later of two clashing ones
    with a log line; `pub fn input_summary`);
  - `crates/uq-core/src/lua/mod.rs` (`emit_swipe` inside `hl.config`, after `input`; the
    `-- Touchpad gestures` block after `-- Input devices`);
  - `crates/uq-core/tests/golden/generated/default.lua` (the two default gesture lines; the
    golden is regenerated and the diff is only those lines);
  - `config/hypr/base.lua` (the `hl.gesture` block is removed, with a comment: "Touchpad
    gestures come from generated.lua (Input page), so a second copy here can't shadow them.");
  - `crates/uq-core/src/settings/provenance.rs` (the four swipe rows and `expected`);
  - `apps/uq-settings/src/registry.rs` (`provenance_rows_name_registry_settings` skips the four
    swipe ids until Task 14; additive);
  - `crates/uq-core/src/history/labels.rs` (the input labels; `populated()` sets gestures and
    swipe);
  - `crates/uq-core/src/history/diff.rs` (`summarize` sends `input` to `input_summary`);
  - `crates/uqd/tests/verify_config.rs` (variant `gestures_all`: one gesture per action, every
    direction, a modifier, a special name, every swipe option; and `base_lua_has_no_gesture`).

**Interfaces (produced, `uq_core::gestures`):**
- `pub enum Direction { Horizontal, Vertical, Left, Right, Up, Down, PinchIn, PinchOut }`
  (serde snake_case: `"pinch_in"`); `pub enum Action { Workspace, Move, Resize, Special, Close, Fullscreen, Float, Spotlight, QuickSettings }`
  (serde `"workspace"` … `"uq:spotlight"`, `"uq:quick-settings"`).
- `pub struct Gesture { pub fingers: u8, pub direction: Direction, pub action: Action, #[serde(default)] pub modifier: Mods, pub workspace: Option<String>, pub enabled: bool }`;
  `impl Trigger for Gesture` (R21; `describe` gives the plain label);
  `fn action_label(&self) -> &'static str`.
- `pub struct SwipeOptions { pub distance: Option<u32>, pub invert: Option<bool>, pub create_new: Option<bool>, pub cancel_ratio: Option<f64> }`.
- `pub const MAX_GESTURES: usize = 24;` `pub fn defaults() -> Vec<Gesture>`;
  `pub fn effective(g: &Option<Vec<Gesture>>) -> Cow<[Gesture]>`.
- `pub fn validate(list: &[Gesture]) -> Result<(), SettingError>` (every refusal in Global
  Constraints; conflicts through `conflict::first_clash`, message with both action labels);
  `pub fn clashes_of(list: &[Gesture]) -> Vec<Clash>` (for the page).
- `hypr.rs`: `pub struct DirName { pub uq: Direction, pub lua: &'static str, pub label: &'static str }`,
  `pub const DIRECTIONS: [DirName; 8]`; `pub struct ActionName { pub uq: Action, pub lua: ActionLua, pub allowed: &'static [Direction], pub label: &'static str }`
  where `pub enum ActionLua { Word(&'static str), Global(&'static str) }`;
  `pub const ACTIONS: [ActionName; 9]`; `pub const SWIPE_OPTIONS: [(&str /*uq*/, &str /*lua*/); 4]`;
  `pub const FIELDS: GestureFields { fingers: "fingers", direction: "direction", action: "action", mods: "mods", workspace_name: "workspace_name" }`.
- `pub fn find_in_user_lua(text: &str) -> Option<usize>` (R22).
- `pub fn summary(before: &Value, after: &Value) -> Vec<String>` (gesture lines, used by
  `input_summary`).
- `uq_core::lua`: `pub fn emit_gesture(g: &Gesture) -> String`,
  `pub fn emit_gestures(i: &InputSettings) -> String` (enabled only, list order),
  `pub(super) fn emit_swipe(out: &mut String, s: &SwipeOptions)`.

- [ ] **Step 1: Write the failing tests** in `gestures/tests.rs` and `lua/gestures.rs`:
  - `no_lua_name_has_a_hyphen` (every `lua` in `DIRECTIONS`, `ACTIONS`, `SWIPE_OPTIONS` and
    `FIELDS`; Review Focus 7);
  - `pinch_renders_as_hyprland_spells_it` (`pinch_in` → `"pinchin"`);
  - `conflicts_follow_hyprland` (Review Focus 6: `horizontal`/`left` both orders refused;
    `up` with `SUPER` and `up` without both allowed; `pinch_in` + `pinch_out` allowed; a
    disabled duplicate allowed; the message names both gestures and both actions);
  - `direction_must_suit_action` (`workspace` with `up` refused, plainly);
  - `fingers_three_to_five`, `too_many_gestures`, `special_workspace_name_refused_or_escaped`
    (`a b`, `x"y` refused; `magic` renders `workspace_name = "magic"`);
  - `defaults_are_the_two_spec_gestures` and `null_means_defaults_empty_means_none`;
  - `swipe_ranges` (2001 refused; `cancel_ratio` 0.555 → 0.56);
  - `input_section_round_trips` (§9: keyboard, mouse, touchpad, devices, gestures and swipe set;
    the existing `phase2_mouse_values_survive` still passes);
  - `lenient_load_drops_only_bad_gestures`;
  - `render_shapes` (each rendered shape in Global Constraints; a disabled gesture renders
    nothing; `uq:spotlight` renders `hl.dsp.global("uq:spotlight")` unquoted as a call);
  - `summary_lines_are_plain`;
  - `find_in_user_lua_skips_comments`;
  - the default golden test fails until the golden gains the two lines.
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core gestures lua -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Regenerate the golden with the test's own output, read the diff,
  and keep it only if it is exactly the gesture block.
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -p uqd -- --test-threads=2`
  (including `lua_mock_runs_every_variant`, whose mock already has `hl.gesture` and
  `hl.dsp.global` for `base.lua`) and clippy; registry through t7check. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): touchpad gestures in the input section, rendered from one checked name table`

### Task 6: uqd: Hyprland's event socket, and the `workspaces` and `dispatch` requests

**Files:**
- Create: `crates/uqd/src/hypr_events.rs` and `crates/uqd/src/hypr_events/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`pub mod hypr_events;`);
  - `crates/uqd/src/compositor.rs` (`workspaces()` → `j/workspaces`; `dispatch(lua)` →
    `expect_ok("dispatch <lua>")`; `FakeCompositor` records both and can be given a workspaces
    reply; trait default methods are **not** used, so every implementor is explicit);
  - `crates/uqd/src/hypr.rs` (`HyprSocket::event_path()`, the `.socket2.sock` beside
    `.socket.sock`).

**Interfaces (produced):**
- `pub enum OutputEvent { Added(String), Removed(String) }`;
  `pub fn parse_event(line: &str) -> Option<OutputEvent>` (only `monitoradded>>` and
  `monitorremoved>>`; names failing `valid_output_name` are dropped; lines over 4 KiB are
  ignored).
- `pub const DEBOUNCE: Duration = Duration::from_secs(1);`
- `pub struct Burst { pub added: bool }`; `pub async fn listen(path: PathBuf, tx: mpsc::Sender<Burst>)`:
  connects, reads lines, restarts the debounce timer on each event, sends one `Burst` per quiet
  second; on EOF or a missing socket, waits with backoff (1 s doubling to 30 s) and reconnects.
- `pub async fn run(shared: SharedDesktop, path: PathBuf)`: spawns `listen` and, for each
  `Burst`, locks the desktop once and calls `outputs_changed(burst.added)` (Task 8; in this task
  a stub that only counts calls, behind `#[cfg(test)]` hooks, so the commit stands alone).

- [ ] **Step 1: Write the failing tests** in `hypr_events/tests.rs` (a `UnixListener` at
  `<tmp>/hypr/<sig>/.socket2.sock`; `tokio::time::pause` with uqd's existing `test-util`
  dev-dependency):
  - `parses_only_monitor_events` (`monitoraddedv2>>…`, `workspace>>2`, `monitoradded>>DP-1"x`
    → none; `monitoradded>>DP-1` → `Added`);
  - `burst_is_debounced` (Review Focus 2: five events 100 ms apart give one `Burst{added: true}`
    1 s after the last);
  - `reconnects_after_hyprland_restarts` (the listener drops, a new one appears: events still
    arrive);
  - `missing_socket_is_quiet` (no panic, no busy loop: at most one attempt per backoff step);
  - in `compositor.rs`: `workspaces_and_dispatch_requests` (the request strings on the real
    socket; `dispatch` with a reply other than `ok` is `Rejected`).
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqd hypr_events compositor -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqd -- --test-threads=2` and clippy. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): read Hyprland's display events, debounced, and add workspace reads and dispatch`

### Task 7: uqd: the `monitors` writes, `RememberMonitors`, `ListOutputs` roles and the gestures Explain row

**Files:**
- Create: `crates/uqd/src/outputs.rs` and `crates/uqd/src/outputs/tests.rs`.
- Modify:
  - `crates/uqd/src/sections.rs` (`set_section("monitors")`: parse strictly; refuse when
    `!next.settings.monitors.layout_eq(&self.state.settings.monitors)`; otherwise
    `commit_section("monitors", …)`; refuse while a change is waiting only when the write
    changes `workspaces`, since that re-renders);
  - `crates/uqd/src/monitors.rs` (`apply_monitors` keeps the stored non-layout keys, R7;
    `list_outputs` adds `roles` and `arrangement`; `confirm_monitors`, `revert` and
    `expire_monitors` call `self.after_window()` (a no-op until Task 8));
  - `crates/uqd/src/service.rs` (`explain("input")` adds the `gestures` row, R22;
    `domain_label` gains `monitors` and `input`, additive);
  - `crates/uqd/src/dbus.rs` (additive: `RememberMonitors(s name, s origin) → s id`);
  - `crates/uq-core/src/client.rs` (`DesktopProxy::remember_monitors(name, origin)`);
  - `crates/uqd/src/monitors/tests.rs`, `crates/uqd/src/sections.rs` tests,
    `crates/uqd/tests/dbus_monitors.rs`.

**Interfaces (produced):**
- `Desktop::remember_monitors(&mut self, name: &str, origin: Origin) -> Result<String, DesktopError>` (R8).
- `ListOutputs` JSON: today's `{live, stored, pending}` plus
  `roles: {primary, main, bar}` (`plan::roles`) and
  `arrangement: {id, name, applied, failed} | null` (the arrangement for the connected set,
  whether the stored layout equals its resolved layout, and whether R10 skipped it this set).
- `pub(crate) fn outputs_json(state: &State, live: &[LiveOutput], failed: Option<&str>) -> Value`
  in `outputs.rs` (pure, so the fake uqd can call the same function; it lives in uqd but takes
  only `uq_core` types; Task 11's fake reimplements the three lines that call
  `plan::roles`/`find_arrangement`, as slice C's F7 pattern shares core functions, not uqd's).

- [ ] **Step 1: Write the failing tests:**
  - `set_section_monitors_layout_still_refused` (Review Focus 4; same message as today);
  - `set_section_monitors_extras_accepted` (primary, bar, planner, an arrangement rename: one
    journal entry each with origin `settings` and the plain summary);
  - `planner_change_rerenders` (the candidate has the workspace lines; refused while a change
    is waiting);
  - `apply_keeps_stored_extras` (Review Focus 4: an `ApplyMonitors` JSON with another `primary`
    leaves the stored one; confirm keeps it too);
  - `remember_is_journalled` (Review Focus 5: one entry "Display arrangement “Desk” saved",
    origin `settings`; a second Remember for the same set says "updated" and keeps the id);
  - `remember_refused_while_waiting` and `remember_refuses_past_32`;
  - `list_outputs_has_roles_and_arrangement`;
  - `explain_input_gestures_row` (`hl.gesture({` on line 7 of `user.lua` → `user_lua`, line 7;
    in a `--` comment → `default`; a stored list → `uq`) and `explain_input_swipe_rows`
    (`getoption gestures:workspace_swipe_distance` through the fake: `runtime` on a mismatch);
  - in `tests/dbus_monitors.rs`: `remember_over_dbus` (the real `serve()` path, private bus).
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqd monitors sections outputs -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core -p uqd -- --test-threads=2` and clippy. Expected: PASS, every slice 3 monitor test unchanged.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): remember display arrangements, write display roles and the planner, and report roles to the shell`

### Task 8: uqd: the hotplug matcher, moving workspaces back, the notification and the X11 primary

**Files:**
- Create:
  - `crates/uqd/src/hotplug.rs` and `crates/uqd/src/hotplug/tests.rs`;
  - `crates/uqd/src/notify.rs` (`Notifier` trait, `SessionNotifier`, `NoNotifier`,
    `#[cfg(test)] RecordingNotifier`);
  - `crates/uqd/src/xrandr.rs` (`Xrandr` trait, `SystemXrandr`, `NoXrandr`,
    `#[cfg(test)] RecordingXrandr`).
- Modify:
  - `crates/uqd/src/lib.rs` (three modules);
  - `crates/uqd/src/service.rs` (`Desktop` gains `notifier: Arc<dyn Notifier>`,
    `xrandr: Arc<dyn Xrandr>`, `hotplug: HotplugState { last_set, held, notified, last_primary }`;
    `with_notifier`, `with_xrandr` builders; `persist` skips the journal for `Origin::Hotplug`,
    R6; `startup()` runs the matcher once after its render, R17);
  - `crates/uqd/src/monitors.rs` (`after_window` runs the matcher when one was held, R9);
  - `crates/uqd/src/hypr_events.rs` (`run` calls the real `outputs_changed`; the Task 6 stub
    goes);
  - `crates/uqd/src/main.rs` (makes `SessionNotifier` on the session connection,
    `SystemXrandr::find()` unless `UQ_DEV_SESSION`, and spawns `hypr_events::run` with
    `HyprSocket::from_env().event_path()`);
  - `packaging/uq/PKGBUILD` (additive optdepend: `'xorg-xrandr: tells X11 apps which display is the main one'`).

**Interfaces (produced):**
- `#[async_trait] pub trait Notifier: Send + Sync { async fn notify(&self, summary: &str, body: &str); }`;
  `SessionNotifier::new(conn: zbus::Connection)` calls
  `org.freedesktop.Notifications.Notify("uQ", 0, "preferences-desktop-display", summary, body, [], {}, 8000)`
  with a 3 s timeout and logs failures.
- `#[async_trait] pub trait Xrandr: Send + Sync { async fn set_primary(&self, output: &str) -> Result<(), String>; }`;
  `SystemXrandr::find() -> Option<SystemXrandr>` (absolute path from `PATH` once; `None` when
  missing or `DISPLAY` unset); runs `[path, "--output", output, "--primary"]` with a 2 s
  timeout.
- `impl Desktop { pub async fn outputs_changed(&mut self, added: bool); pub(crate) async fn after_window(&mut self); }`
  in `hotplug.rs` (R18).

- [ ] **Step 1: Write the failing tests** in `hotplug/tests.rs` (rig: `FakeCompositor` with a
  scripted `monitors` reply, `RecordingNotifier`, `RecordingXrandr`, `Paths::under(tmp)`):
  - `desk_arrangement_applies_on_plug` (spec §1 criterion 3: a saved "Desk" for laptop + two
    screens; the reply changes to those three; `outputs_changed(true)` → stored outputs are the
    resolved Desk layout, a verified render and a `reload`, no revert window, `MonitorsChanged`
    sent);
  - `laptop_arrangement_on_unplug`;
  - `no_match_changes_nothing` (no write, no notification; `arrangement` is `null`);
  - `auto_off_changes_nothing_but_replans`;
  - `already_applied_is_not_rewritten`;
  - `hotplug_switch_not_journalled` (Review Focus 5: `ListHistory` unchanged, no restore point
    saved, `state.toml` changed);
  - `bad_arrangement_skipped_and_notified_once` and `arrangement_turning_all_off_is_skipped`
    (Review Focus 3: the fake verify rejects; stored layout unchanged; one notification across
    three `outputs_changed` calls with the same set; a second one after the set changes and
    comes back);
  - `hotplug_waits_for_pending_change` (Review Focus 2: an apply is waiting; the matcher runs
    only after `revert_monitors`, once);
  - `planned_workspaces_move_back` (workspaces reply has `3` on `eDP-1`, planned on `DP-1`:
    one `dispatch hl.dsp.workspace.move({ workspace = "3", monitor = "DP-1" })`; nothing when
    `move_back` is off, on a removal, or when it already sits there);
  - `xrandr_argv_only_and_skips` (Review Focus 9: called with `DP-1` once when the main display
    changes; not again for the same connector; never with `primary` unset; `NoXrandr` in a dev
    session);
  - `startup_runs_the_matcher_once`.
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqd hotplug -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. `hotplug.rs` holds only R18's sequence; the pieces are Task 3's
  pure functions.
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqd -- --test-threads=2` and clippy. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): switch display arrangements on hotplug, move workspaces back and set the X11 main display`

### Task 9: The bridge and the shell: roles, bar per screen and the main display

**Files:**
- Create:
  - `shell/logic/outputs.js` (pure);
  - `shell/core/Outputs.qml` (singleton, pure QML, no Quickshell imports, like
    `core/Surfaces.qml`);
  - `tests/qml/tst_outputs.qml`.
- Modify:
  - `crates/uqctl/src/bridge/uqd.rs` (forward `MonitorsChanged` as `{}`; `Request::ListOutputs`
    joins the shell's pass-through list, returning parsed JSON);
  - `shell/core/qmldir` (`singleton Outputs 1.0 Outputs.qml`);
  - `shell/shell.qml` (the bar `Variants` model is `Outputs.barScreens(Quickshell.screens)`);
  - `shell/notif/builtin/Popups.qml` and `shell/osd/Osd.qml` (`screen:` prefers
    `Outputs.primaryScreen(Quickshell.screens)`, else today's focused screen; `openedOn` still
    wins in the OSD);
  - `shell/launcher/Spotlight.qml` (when `Surfaces.screen` is null, the main display before the
    focused screen; the app menu is unchanged, R11);
  - `tests/shell/bar_loads.sh` (also instantiates `Outputs` and checks a bar per screen with
    `roles` null).

**Interfaces (produced):**
- `outputs.js`: `function primaryScreen(screens, roles)` (the screen whose `name` is
  `roles.primary`, else `null`); `function barScreens(screens, roles)` (all when `roles` or
  `roles.bar` is null, or when the filter is empty; Review Focus 8).
- `Outputs.qml`: `property var roles: null`; `function refresh()`
  (`Bridge.call("ListOutputs", [])` → `roles`); refreshes on `Hello` with `uqd_up`, `UqdUp`,
  `MonitorsChanged` and `SectionChanged` for `monitors`; `function primaryScreen(screens)`,
  `function barScreens(screens)`.

- [ ] **Step 1: Write the failing tests:**
  - `tests/qml/tst_outputs.qml`: `test_primary_found`, `test_primary_empty_is_null`,
    `test_bar_all_when_null`, `test_bar_chosen`, `bar_never_empty` (roles naming an absent
    screen → all);
  - in `crates/uqctl/src/bridge/uqd.rs` tests: `list_outputs_is_a_shell_call` and
    `monitors_changed_is_forwarded`.
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqctl bridge -- --test-threads=2`.
  Expected: FAIL. (The QML tests run in CI only, K4.)
- [ ] **Step 3:** Implement. `scripts/lint-shell.sh` must pass (run it locally; it needs no
  Quickshell).
- [ ] **Step 4:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uqctl -- --test-threads=2`, clippy
  and `scripts/lint-shell.sh`. Expected: PASS. CI runs `tst_outputs.qml` and `bar_loads.sh`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(shell): bar per screen, and pop-ups on the main display`

### Task 10: One wallpaper across all screens

**Files:**
- Modify:
  - `crates/uq-core/src/settings/wallpaper.rs` (`Fit::Span`, `NAMES` gains `"span"`, the
    loader maps `"span"`; `validate` refuses `span` on any entry but `"*"`, R24);
  - `crates/uq-core/src/history/labels.rs` (no new key; `populated()` sets the `*` entry's fit
    to `span` so the summary test sees the value);
  - `shell/logic/wallpaper.js` (`spanGeometry(screens, name, spanNames)` →
    `{x, y, width, height}` of the box relative to this screen, or `null`;
    `spanNames(wallpaper, screens)` → screens drawing the `*` entry with `fit` `span`;
    `clockIndex(nowMs, intervalS, n, errored)`; `fillMode("span")` → crop);
  - `shell/wallpaper/Wallpaper.qml` (passes the geometry to `Crossfade`; with span and a folder,
    the slide index comes from `clockIndex`, ticking each interval on the minute boundary);
  - `shell/wallpaper/Crossfade.qml` (`property var span: null`: when set, the image is sized to
    the box and placed at its offset inside a clipping item);
  - `apps/uq-settings/src/pages/wallpaper.rs` (pre-check: span only for All displays, plain
    message);
  - `apps/uq-settings/qml/components/WallpaperEntryEditor.qml` ("Span across displays" in the
    fit choices only when editing All displays);
  - `tests/qml/tst_wallpaper.qml`, `tests/qml/tst_wallpaper_crossfade.qml`,
    `tests/settings-qml/tst_wallpaper_editor.qml`, `crates/uq-core/src/settings/tests.rs`.

- [ ] **Step 1: Write the failing tests:**
  - uq-core: `span_only_on_all_displays`, `span_round_trips`, `older_values_still_load`
    (`"span"` loads as `Span`; an unknown fit still loads as `Fill`);
  - page logic: `span_offered_only_for_all_displays` (t7check, `pages::wallpaper`);
  - `tst_wallpaper.qml`: `test_span_geometry_side_by_side` (1920×1080 at 0,0 and 2560×1440 at
    1920,0: the left screen's box is `{x: 0, y: 0, width: 4480, height: 1440}`, the right
    screen's `{x: -1920, …}`), `test_span_geometry_stacked_and_negative`,
    `test_span_skips_screens_with_their_own_entry`, `test_clock_index_same_for_all_screens`,
    `test_clock_index_skips_failed`;
  - `tst_wallpaper_crossfade.qml`: `test_span_image_offset`;
  - `tst_wallpaper_editor.qml`: `test_span_choice_only_for_all_displays`.
- [ ] **Step 2:** Run `CARGO_INCREMENTAL=0 cargo test -j 2 -p uq-core wallpaper -- --test-threads=2`
  and t7check `pages::wallpaper`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the uq-core tests, t7check `pages::wallpaper`, clippy and
  `scripts/lint-shell.sh`. Expected: PASS. CI runs the QML suites.
- [ ] **Step 5:** Commit (K5 trailers): `feat: span one wallpaper across every display`

### Task 11: uq-settings: the Displays page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/displays/extras.rs`;
  - `apps/uq-settings/src/pages/displays/extras_tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/monitors.rs`.
- Modify:
  - `apps/uq-settings/src/pages/displays/mod.rs` (`pub mod extras;`; `Op` gains the variants
    below, handled by `extras`; `snapshot` adds `extras::snapshot_parts`);
  - `apps/uq-settings/src/pages/fake_uqd.rs` (`set_section("monitors")` follows R7,
    `RememberMonitors`, `ListOutputs` adds `roles` and `arrangement` from the core functions;
    scripted `live` with make/model/serial).

**Interfaces (produced, `pages::displays`):**
- New `Op` variants: `SetMain { output: Option<String> }`,
  `SetBar { show: String, outputs: Vec<String> }`, `SetAutoSwitch(bool)`,
  `SetPlan { strategy: String, count: u8, move_back: bool }`,
  `SetManual { workspace: u8, output: Option<String> }`, `Remember { name: String }`,
  `Rename { id: String, name: String }`, `Forget { id: String }`.
  Output names come from the page; `extras` turns each into its `MonitorId` from the live list
  (a name that isn't live is refused with "That display isn't connected").
- Every write but `Remember` is a `SetSection("monitors", <stored section with the one change>,
  "settings")`; pre-checked with `Monitors::from_section`; refusals shown with
  `runner::plain_refusal`.
- Snapshot additions: `main: {output, label} | null`, `bar: {show, outputs}`,
  `auto_switch`, `plan: {strategy, count, move_back, manual: [{workspace, output, label}],
  preview: [{label, workspaces: [..]}]}` (from `plan::plan` over the live screens, so the page
  shows the result before saving), `arrangements: [{id, name, screens: [label], current}]`,
  `offer: "remember" | "update" | null` (remember when no arrangement matches and nothing is
  waiting; update when one matches and its layout differs from the stored one),
  `arrangement_failed: bool`, and `user_lua_gestures` is not here (Task 13).

- [ ] **Step 1: Write the failing tests** in `extras_tests.rs` (fake uqd, private bus):
  - `set_main_by_output_name` and `clear_main`;
  - `bar_choices` (all; main; chosen with two screens; a disconnected name refused);
  - `auto_switch_round_trips`;
  - `plan_preview_before_save` (sequential on two fake screens shows 1–5 and 6–10);
  - `manual_slot_set_and_cleared`;
  - `remember_offered_then_saved` (after Keep with an unmatched set, `offer` is `remember`;
    `Remember{"Desk"}` makes it `null` and lists "Desk" as current);
  - `update_offered_after_change` (a matched set, then a kept change: `offer` is `update`);
  - `rename_and_forget`;
  - `every_key_round_trips` (§9: set each new key, re-open the state, read it back);
  - `layout_write_through_set_section_refused_plainly` (the fake mirrors uqd's message).
- [ ] **Step 2:** Run t7check `pages::displays`. Expected: FAIL.
- [ ] **Step 3:** Implement. `extras.rs` starts with the dead-code `cfg_attr` (Global
  Constraints) until Task 12.
- [ ] **Step 4:** Run t7check `pages::displays` and `pages::history` (History shows the new
  summaries), and clippy through t7check. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): Displays page logic for the main display, bar, arrangements and workspace planner`

### Task 12: uq-settings: the Displays page sections, the model and the registry entries

**Files:**
- Create:
  - `apps/uq-settings/qml/components/DisplayRoles.qml` (Main display combo; "Show the bar on"
    with All / Main display only / Choose, and a checkbox per screen for Choose);
  - `apps/uq-settings/qml/components/DisplayArrangements.qml` (the "Remember for these screens"
    / "Update “…”" banner, the saved list with Rename and Forget, the auto-switch switch, and
    "“Desk” couldn't be used with these screens" when `arrangement_failed`);
  - `apps/uq-settings/qml/components/WorkspacePlanner.qml` (strategy, count, Move back, the
    preview as one row per screen, and per-workspace screen combos for Choose for each);
  - `tests/settings-qml/tst_display_arrangements.qml`, `tests/settings-qml/tst_workspace_planner.qml`,
    `tests/settings-qml/tst_display_roles.qml`.
- Modify:
  - `apps/uq-settings/src/displays_model.rs` (invokables `set_main`, `set_bar`,
    `set_auto_switch`, `set_plan`, `set_manual`, `remember`, `rename_arrangement`,
    `forget_arrangement`; `FOLLOW` follows `SectionChanged("monitors")`; the dead-code
    attribute in `extras.rs` goes);
  - `apps/uq-settings/qml/pages/DisplaysPage.qml` (the three components below the arrangement;
    the Remember banner also appears after Keep);
  - `apps/uq-settings/build.rs` (the three QML files, additive);
  - `apps/uq-settings/src/registry.rs` (additive `DISPLAYS_SETTINGS` entries, each with
    keywords: `display-main` "Main display" [main, primary, notifications]; `display-bar`
    "Show the bar on" [bar, panel, screens]; `display-arrangements-auto` "Switch arrangements
    when screens change" [profile, dock, hotplug, automatic]; `display-remember` "Remember for
    these screens" [save, profile, layout]; `display-workspaces` "Workspaces across screens"
    [workspace, monitor, planner]; `display-workspaces-back` "Move workspaces back when a
    screen returns" [workspace, return, dock]; and the wallpaper page's fit row gains the
    keyword "span").

- [ ] **Step 1: Write the failing tests:**
  - `tst_display_arrangements.qml`: `test_remember_banner_shown_for_offer`,
    `test_name_shown_as_plain_text` (a `<b>x</b>` name stays literal), `test_forget_asks_first`,
    `test_failed_line_shown`, `test_keyboard_only` (Tab reaches every control; Enter on
    Remember opens the dialog);
  - `tst_workspace_planner.qml`: `test_preview_rows`, `test_manual_combos_only_for_choose`;
  - `tst_display_roles.qml`: `test_choose_shows_checkboxes`, `test_disconnected_main_says_so`;
  - registry (t7check): `display_settings_have_keywords` (the existing keywords test covers the
    new entries) and `search_finds_main_display`.
- [ ] **Step 2:** Run t7check `registry`. Expected: FAIL (the entries are missing). The QML
  tests run in CI.
- [ ] **Step 3:** Implement. Every string passes `scripts/lint-settings.sh` (run it locally).
- [ ] **Step 4:** Run t7check `registry` and `pages::displays`, `scripts/lint-settings.sh`.
  Expected: PASS. CI runs `cargo test -p uq-settings` and the settings QML suite.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): main display, bar per screen, saved arrangements and the workspace planner on the Displays page`

### Task 13: uq-settings: the gestures page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/input/gestures.rs`;
  - `apps/uq-settings/src/pages/input/gestures_tests.rs`.
- Modify:
  - `apps/uq-settings/src/pages/input.rs` (`pub mod gestures;`; `Op` gains the variants below;
    the snapshot adds `gestures::snapshot_parts`);
  - `apps/uq-settings/src/pages/fake_uqd.rs` (the `input` section stores `gestures` and
    `swipe` through `InputSettings::from_section`; `Explain("input")` can be scripted with a
    `gestures` `user_lua` row).

**Interfaces (produced, `pages::input`):**
- New `Op` variants: `SaveGesture { index: i32, gesture: Value }` (`-1` adds),
  `RemoveGesture { index: usize }`, `ToggleGesture { index: usize, enabled: bool }`,
  `ResetGestures` (writes `gestures: null`), `SetSwipe { field: String, value: Value }`.
  Every write sends the whole `input` section with `gestures` materialised from
  `gestures::effective` (so turning a default off writes the list), pre-checked with
  `InputSettings::from_section`.
- `pub fn draft_clash(list: &[Gesture], index: i32, draft: &Gesture) -> Option<String>`: the
  editor's live warning, from `conflict::clashes` (R23), before saving.
- Snapshot additions: `gestures: [{label, action_label, enabled, gesture}]`,
  `defaults: bool` (true when stored `null`), `clashes: {"<index>": "<text>"}` (stored lists
  can't clash, but a list loaded from an older or hand-edited file is checked anyway),
  `choices: {fingers: [3,4,5], directions: [{id, label}], actions: [{id, label, directions}],
  modifiers: [{id, label}]}` from `gestures::hypr`, `swipe`, and `user_lua: {line, path} | null`
  from `Explain("input")`'s `gestures` row.

- [ ] **Step 1: Write the failing tests** in `gestures_tests.rs`:
  - `defaults_listed_when_null` and `turning_a_default_off_writes_the_list`;
  - `add_edit_remove_toggle`;
  - `draft_clash_warns_before_save` (a draft `3 fingers left` against the default `3 fingers
    sideways` gives the plain text; saving it anyway is refused by the fake with the same
    reason, shown once, and the list is unchanged);
  - `directions_offered_per_action` (`workspace` offers only sideways and up-and-down);
  - `reset_restores_defaults`;
  - `swipe_options_round_trip` (§9) and `swipe_out_of_range_refused_plainly`;
  - `user_lua_gestures_reported`.
- [ ] **Step 2:** Run t7check `pages::input`. Expected: FAIL.
- [ ] **Step 3:** Implement, with the dead-code `cfg_attr` until Task 14.
- [ ] **Step 4:** Run t7check `pages::input` and clippy through t7check. Expected: PASS, every
  slice 3 input test unchanged.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): Input page logic for touchpad gestures and swipe options`

### Task 14: uq-settings: the gestures section, its editor, the model and the registry entries

**Files:**
- Create:
  - `apps/uq-settings/qml/components/GestureList.qml` (one row per gesture: its label, its
    action, a switch, Change and Remove; the conflict line under a row; "Back to uQ's
    gestures"; the `user.lua` line with **Open user.lua**, using the existing `SourceLine.qml`);
  - `apps/uq-settings/qml/components/GestureEditor.qml` (a `QQC2.Dialog` like `RevertDialog`:
    fingers, direction, action, modifier, the special workspace name when the action needs it;
    the live conflict warning through `RuleMessage.qml`; Save disabled while it clashes);
  - `tests/settings-qml/tst_gesture_list.qml` and `tests/settings-qml/tst_gesture_editor.qml`.
- Modify:
  - `apps/uq-settings/src/input_model.rs` (invokables `save_gesture(index, json)`,
    `remove_gesture`, `toggle_gesture`, `reset_gestures`, `set_swipe(field, json)`,
    `draft_clash(index, json) -> QString`; the dead-code attribute in `gestures.rs` goes);
  - `apps/uq-settings/qml/pages/InputPage.qml` (a "Touchpad gestures" group after the touchpad
    options, with the swipe options as `OverrideSpin`/`OverrideSwitch`/`ValueSlider` rows
    bound to their setting ids so the Explain source lines show);
  - `apps/uq-settings/build.rs` (two QML files, additive);
  - `apps/uq-settings/src/registry.rs` (additive `INPUT_SETTINGS` entries: `gestures`
    "Touchpad gestures" [gesture, swipe, pinch, fingers]; `gesture-swipe-distance` "Swipe
    distance"; `gesture-swipe-invert` "Reverse swipe direction"; `gesture-swipe-create` "Swipe
    past the last workspace to make a new one"; `gesture-swipe-cancel` "How far to swipe before
    it switches"; each with keywords), and the Task 5 skip in
    `provenance_rows_name_registry_settings` is removed.

- [ ] **Step 1: Write the failing tests:**
  - `tst_gesture_editor.qml` (spec §10 "the gesture conflict warning"):
    `test_conflict_warning_shown_and_save_disabled`, `test_warning_clears_when_fixed`,
    `test_directions_follow_action`, `test_special_name_only_for_special`,
    `test_escape_cancels_enter_saves`;
  - `tst_gesture_list.qml`: `test_defaults_badge`, `test_toggle_and_remove`,
    `test_user_lua_line_plain_text`, `test_keyboard_only`;
  - registry (t7check): the keywords test sees the new entries;
    `provenance_rows_name_registry_settings` passes without the skip.
- [ ] **Step 2:** Run t7check `registry`. Expected: FAIL until the entries exist.
- [ ] **Step 3:** Implement. Run `scripts/lint-settings.sh`.
- [ ] **Step 4:** Run t7check `registry` and `pages::input`, and `scripts/lint-settings.sh`.
  Expected: PASS. CI runs `cargo test -p uq-settings` and the settings QML suite.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): touchpad gestures on the Input page, with conflict warnings`

### Task 15: Docs, changelog, live checks and the whole gate

**Files:**
- Modify:
  - `README.md` (Displays and Input paragraphs; the D-Bus paragraph);
  - `CHANGELOG.md` (`## [Unreleased]` → `### Added`, additive);
  - `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` (§10 D-Bus summary
    gains `RememberMonitors` and says `ListOutputs` carries `roles`; §3 says hotplug switches
    aren't recorded; §7.1 says the plan is stored in `monitors.workspaces` and merged at render;
    §7.2 names Hyprland's axis rule and `pinchin`/`pinchout`);
  - `docs/developing.md` (a `uq-dev session` doesn't run `xrandr`).
- Create: `docs/notes/settings-slice-f-live-check.md`.

- [ ] **Step 1:** Update `README.md`:
  - "#### Displays": saved arrangements (matched by screen, not port; one per set; switched
    within about 2 s with no prompt; not recorded in History, while Remember, Forget and the
    other choices are); the main display (what moves there, and only when chosen; the X11
    primary when `xrandr` is installed); the bar choice and that a bar always shows somewhere;
    the workspace planner's three ways and Move back; that undo puts back a whole list.
  - "#### Input": touchpad gestures, the two defaults and how to turn them off, the
    conflict rule (including sideways covering left and right), the swipe options, and that a
    gesture in `user.lua` that overlaps uQ's makes Hyprland refuse the config.
  - "#### Wallpaper": Span across displays (All displays only; slideshows stay in step).
  - The D-Bus paragraph: `RememberMonitors`, `ListOutputs`' `roles` and `arrangement`, and
    `SetSection("monitors")` for everything but the layout.
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Display arrangements, the main display, and
  touchpad gestures.** Name the auto-switch, the main display, span wallpaper, the planner, bar
  per screen, gestures with conflict checks, the new D-Bus method, the `ListOutputs` fields,
  the `base.lua` gesture moving into `generated.lua`, and the `xorg-xrandr` optdepend.
- [ ] **Step 3:** Write `docs/notes/settings-slice-f-live-check.md` in the style of slice D's
  (run on real Hyprland with uQ installed; a laptop plus one or two external screens, ideally
  two of the same model; a touchpad; `xrandr` installed; note `user.lua`'s modification time
  first and check it is the same at the end). Items:
  1. **§1 criterion 3.** Undocked, arrange the laptop screen, Keep, and Remember it as
     "Laptop". Docked, arrange the three screens, Keep, Remember as "Desk". Unplug: within 2 s
     the laptop layout is back with no dialog. Plug in: "Desk" is back within 2 s. History
     shows the two Remembers and no hotplug entries.
  2. **Port swap.** Swap the two external cables (or dock ports). "Desk" still applies and
     each screen keeps its own position and scale. With two identical screens, record which
     way round they came back.
  3. **Flapping.** Unplug and replug quickly three times: one switch at the end, no dialog,
     `journalctl --user -u uqd` shows one apply.
  4. **Bad arrangement.** Hand-edit a saved arrangement's `mode` to one the screen can't do
     (or one Hyprland refuses) and replug: the layout stays, one notification appears, the
     Displays page says it couldn't be used; replug again without changing the set: no second
     notification.
  5. **Auto switch off.** Turn off "Switch arrangements when screens change" and replug:
     nothing changes, and the page offers to arrange and remember.
  6. **Main display.** Choose the external screen. Notifications, the volume OSD and
     Spotlight (SUPER+Space) appear there while you work on the laptop screen; the app menu
     from the laptop's bar stays on the laptop. `xrandr --listmonitors` (from an X11 terminal)
     shows it as primary (`*`). Log out and in: the cursor and workspace 1 start there.
     Unplug it: pop-ups follow the focused screen again. Record whether `default_monitor`
     changed anything on reload.
  7. **Bar per screen.** Main display only: one bar. Choose: only the ticked screens. Choose a
     screen and unplug it: bars on every screen.
  8. **Span.** Choose "Span across displays" for All displays with a wide image: the image
     runs on across the bezel at the screens' real positions; a screen with its own wallpaper
     is left out. With a folder, both screens change slide together.
  9. **Workspaces per screen.** Sequential with 10: `hyprctl workspaces` puts 1–5 on the left
     screen and 6–10 on the right. Open windows on 7, unplug the right screen, plug it back:
     workspace 7 moves back. Turn Move back off: it stays. Interleave and Choose for each
     behave as the preview said. A workspace rule of your own on the Rules page keeps its own
     screen.
  10. **Gesture defaults.** On a fresh login, three fingers sideways switches workspace and
      four fingers up opens Spotlight once per swipe. `grep -n gesture
      /usr/share/uq/hypr/base.lua` finds nothing; `generated.lua` has both lines.
  11. **Gesture editor.** Add three fingers down → Show or hide the scratch workspace: it
      works. Try three fingers left: the editor warns that sideways already uses it and Save
      stays off. Add SUPER + four fingers pinch in → Float the window: it works. Turn the
      default sideways swipe off: it stops. `hyprctl configerrors` is empty after each change
      (P3, and the "reload clears gestures" item).
  12. **Swipe options.** Distance 600, Reverse on, Make a new one off, ratio 0.3: each takes
      effect at once; `hyprctl getoption gestures:workspace_swipe_distance` is 600. Add
      `gestures = { workspace_swipe_distance = 200 }` to `user.lua` and reload: the row says
      "Overridden in user.lua, line N".
  13. **`user.lua` gesture.** Add `hl.gesture({ fingers = 3, direction = "left", action =
      "close" })` to `user.lua` and reload: the Input page's gestures row says user.lua sets
      gestures at that line, and the next uQ change shows Hyprland's message plainly. Remove it.
  14. **History and restore.** Undo the main display change, a bar change, a gesture change
      and a Remember from **History**: each row reads plainly and the page follows. Restore
      yesterday's point: the saved arrangements, the planner and the gestures match it.
  15. **Dev session.** In `uq-dev session`, plugging a screen into the host doesn't run
      `xrandr` (check `journalctl --user -u uqd`).
  16. `user.lua`'s modification time is unchanged.
- [ ] **Step 4:** Run the full gate, exactly as CI does, one command at a time:
  - `cargo fmt --all --check`;
  - `CARGO_INCREMENTAL=0 cargo clippy -j 2 --workspace --all-targets --all-features --locked -- -D warnings`;
  - `CARGO_INCREMENTAL=0 cargo test -j 2 --workspace --locked -- --include-ignored --test-threads=2`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh` (with `pam_stacks.sh`'s known flake handled as in Global Constraints);
  - both `qmltestrunner` suites.

  In the build container (K4), run everything without Qt (`--exclude uq-settings`) and without
  `--include-ignored`; run the page logic and registry tests through t7check. Leave
  `uq-settings`, both QML suites, the Quickshell-based shell tests (`bar_loads.sh`) and the
  `--ignored` Hyprland verify (Tasks 4 and 5's new variants) to CI, and say so in the report.
  Expected: all PASS.
- [ ] **Step 5:** Write the PR body's "Spec deviations" section from the list near the top of
  this plan, and its "Not confirmed from source" list from Global Constraints. Commit (K5
  trailers): `docs: settings slice F — display arrangements, multi-monitor extras and touchpad gestures`

## Notes: open questions for the user

1. **Hotplug switches in History (R6).** This plan doesn't journal automatic switches (like the
   theme schedule), so docking every day doesn't push older changes out of the 500-entry
   journal. The alternative is one entry per switch, "Desk arrangement used", origin
   `hotplug`. Changing it later is one line in `persist`.
2. **Where pop-ups go (R11).** With a main display chosen, Spotlight, notifications and the OSD
   open there, as the spec says, but only once one is chosen. The app menu stays with the bar
   that opened it. Should Spotlight instead keep following the focused screen?
3. **Remember after every confirm (R8).** The page offers Remember or Update after a kept
   change; it never saves on its own. Slice 3's plan suggested turning the stored layout into a
   first arrangement without asking; this plan asks instead.
4. **The planner and your own workspace rules (R14).** A rule you set on the Rules page wins,
   field by field, over the plan for the same workspace. Should the planner instead win, or
   should the Rules page show the planned screen read-only?
