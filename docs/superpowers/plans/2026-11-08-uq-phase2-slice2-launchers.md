# uQ Phase 2 Slice 2 (Launchers) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship two launchers:
- a Spotlight-style search on `SUPER+Space` covering apps, a calculator, run-command and external
  launcher providers;
- a KDE-style app menu opened from the System island button and by tapping SUPER, with a
  categories sidebar, a favourites grid and a power row.

Both rank apps by match plus launch history, and neither lets a provider take an app's top hit.

**Architecture:**
- **Bridge (Rust):** owns the providers. Built-ins `calc` and `run`, external
  `org.uq.Launcher1.Provider` services found through `.provider` files, gating, clamping and
  generations. Adds the `Search`, `Activate` and `Power` calls, and wires `Launch {run_id}`.
- **Shell (QML):** keeps app search local (`DesktopEntries` plus `logic/rank.js`), merges in
  provider results, and draws the two surfaces.
- **Session (`base.lua`):** the keybinds use `hl.dsp.global("uq:<name>")`, which the shell receives
  through `GlobalShortcut`.

**Tech Stack:**
- Rust: the existing workspace, plus `zbus` 5 (with `NO_AUTO_START` flags) and `tokio`.
- Quickshell 0.3.1:
  - `DesktopEntries.applications`, and `DesktopEntry` {id, name, genericName, comment, icon,
    categories, keywords, noDisplay, runInTerminal, actions}
  - `GlobalShortcut` {appid, name, pressed, released}
  - `HyprlandFocusGrab`, `Quickshell.clipboardText`, `IpcHandler`
- Hyprland 0.56.2–0.57.x Lua (`hl.dsp.global`, bind option `release`).

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase2-shell-design.md`. This plan covers slice 2
of §6. It implements §2.3 (built-in providers, external providers, app search and ranking), §2.10
(focus, keyboard, text safety) and §2.12 (the launcher keybinds and shell IPC).

## Global Constraints

- **Trust model (spec §1):**
  - Same-user unsandboxed processes are trusted.
  - Provider results, desktop-entry names and comments are untrusted text.
  - All shell text goes through `UqText` (plain text). The Phase 2 lint (`scripts/lint-shell.sh`)
    must still pass.
- **Built-in providers:**
  - `calc` runs only on queries starting with `=` or containing an operator; a bare number is not
    a calculation.
    - Grammar: `+ - * / ^ ( )`, unary minus, `x% of y`, and postfix `%` (÷100).
    - Functions: `sqrt sin cos tan ln log abs round`. Constants: `pi`, `e`.
    - At most 256 characters and nesting depth 32, parsed iteratively.
    - Results rounded to 12 significant digits; errors produce no result.
  - `run` runs only on queries starting with `>`. Its single item id is `run-<generation>`, mapped
    to that generation's command; any other id is rejected. `Launch {run_id}` runs the command with
    `sh -c` inside a `systemd-run --user --scope`.
- **External providers:**
  - Interface `org.uq.Launcher1.Provider` at `/org/uq/Launcher1`:
    `Search(s query) → a(sssssd)` returning (id, title, subtitle, icon, actions_json, score), and
    `Activate(s id, s action)`.
  - Discovered through `.provider` files in `/usr/share/uq/launcher/` and
    `~/.local/share/uq/launcher/`, with keys `BusName`, `Name` and optional `Prefix`.
  - Gating:
    - The shell debounces 70 ms.
    - Queries starting with `>` or `=` never reach providers.
    - A provider with a `Prefix` gets only queries starting with it, with the prefix stripped.
    - Other providers get queries of 2 or more characters.
    - Only names that currently have an owner are queried, with `NO_AUTO_START`.
    - Each provider has a 300 ms deadline.
  - Clamping:
    - At most 20 items per provider.
    - Truncate title to 200 bytes, subtitle to 300 bytes and actions to 2 KiB.
    - `icon` must be an icon-theme name (`[A-Za-z0-9._-]+`) or an absolute local path; anything
      else becomes the default icon `system-search`.
  - Scoring: scores are clamped to [0, 1] and multiplied by 0.9. An external provider can never
    outrank an exact or prefix app-name match. Every provider row shows the provider's `Name`.
  - `Activate` goes back to the provider that produced the item. The shell never runs anything
    from provider data.
- **App ranking:** match score + 0.15 × log(1 + launch_count). Exact and prefix name matches
  outrank everything.
- **Keybinds (spec §2.12):**
  - `SUPER+Space` → `uq:spotlight`.
  - Tapping SUPER (`release`) → `uq:appmenu`.
  - Neither is `locked`.
  - Shell IPC exposes only `open`, `close` and `toggle` for surfaces. These are ignored while
    locked. Nothing can unlock.
- **Focus and placement:** surfaces open on `Hyprland.focusedMonitor` and grab focus with
  `HyprlandFocusGrab`. Only one launcher surface is open at a time. Both close and wipe their
  query when `Bridge.locked` becomes true.
- **Launching:** only through the bridge (`Launch {desktop_id}` or `{run_id}`). Power actions go
  only through the bridge's closed `Power` set.
- **Process and test rules (Phase 2):**
  - Never run the bridge or the shell against the real session bus or on the user's KDE desktop.
  - Use private dbus-daemons with socket paths under 108 bytes.
  - Use offscreen qmltestrunner (`/usr/lib/qt6/bin/qmltestrunner`).

## Review Focus

1. **A provider returning "Firefox" with a score of 1.0 must not become the top hit for "fire".**
   The app does. Test: Task 5 `test_provider_cannot_take_app_top_hit`.
2. **Results from an older keystroke must never replace newer ones.** This covers a slow provider
   answering after the user typed more. Test: Task 3 `stale_generation_results_dropped`, plus
   Task 5 `test_merge_ignores_stale_generation`.
3. **A provider name, title or icon containing markup or a URL must render as literal text, and
   no remote image may load.** Test: Task 3 `icon_clamped_to_theme_name_or_local_path`; Task 6
   reuses `UqText`, enforced by the lint.
4. **Typing `> rm -rf ~` and pressing Enter runs only that command, inside a scope.** No other id
   can trigger `sh -c`, including an id replayed from an earlier generation. Test: Task 2
   `run_id_from_old_generation_rejected`.
5. **Locking while Spotlight is open must close it and clear the typed text.** That text could be
   a command or a secret. Test: Task 6 `test_close_and_wipe_on_lock` (logic) and the Task 8
   lock-flag check.

---

## File Structure

```
crates/uqctl/src/bridge/calc.rs        calculator (tokenizer + iterative precedence climbing)
crates/uqctl/src/bridge/run.rs         run provider: generation-scoped id → command map
crates/uqctl/src/bridge/providers.rs   .provider discovery, gating, fan-out, clamping, Search/Activate
crates/uqctl/src/bridge/power.rs       closed Power action set
crates/uqctl/src/bridge/mod.rs         route Search / Activate / Power; Launch {run_id}
shell/logic/rank.js                    fuzzy match, app scoring, merge with provider results, top hit
shell/logic/launcher.js                query routing (prefixes), generation counter, surface state machine
shell/logic/appmenu.js                 category mapping, favourites, power-row confirm state
shell/launcher/Spotlight.qml           overlay search surface
shell/launcher/AppMenu.qml             KDE-style menu surface
shell/launcher/Launchers.qml           owns both surfaces, GlobalShortcuts, IpcHandler
shell/bar/SystemIsland.qml             app-menu button wiring (modify)
shell/shell.qml                        instantiate Launchers (modify)
config/hypr/base.lua                   launcher binds (modify)
tests/qml/tst_rank.qml, tst_launcher.qml, tst_appmenu.qml
tests/shell/bar_loads.sh               also load Launchers (modify)
crates/uqctl/tests/providers_dbus.rs   fake provider on a private bus
```

---

### Task 1: Calculator provider

**Files:** Create `crates/uqctl/src/bridge/calc.rs`; test in the same module.

**Interfaces:**
- Produces:
  - `fn applies(query: &str) -> bool`: true if the query starts with `=`, or (after trimming)
    contains at least one operator `+ - * / ^ %` or a function name and isn't just a number.
  - `fn evaluate(query: &str) -> Option<String>`: strips a leading `=`, then evaluates. It returns
    the formatted result rounded to 12 significant digits, with trailing zeros removed and no
    exponent for magnitudes between 1e-6 and 1e15. It returns `None` on any error, on input over
    256 characters, or on nesting depth over 32.
  - `fn item(query: &str) -> Option<ProviderItem>`: id `calc`, title = the result, subtitle = the
    expression, icon `accessories-calculator`, score 1.0, actions `["copy"]`.
  - `struct ProviderItem { id: String, title: String, subtitle: String, icon: String, score: f64, actions: Vec<String> }`,
    defined here and shared with Tasks 2 and 3.
- Algorithm: tokenizer, then precedence climbing with an explicit operator stack (no recursion).
  `^` is right-associative. Unary minus binds tighter than `*` but looser than `^`, so `-2^2` is
  `-4`. `x% of y` means `x/100*y`. A postfix `%` means ÷100. Function calls need parentheses.
  Multiplication is never implicit.
- [ ] **Step 1: Failing tests:**
  - `precedence`: `= 2+3*4` → `"14"`; `2^3^2` → `"512"`; `-2^2` → `"-4"`.
  - `percent`: `18% of 2450` → `"441"`; `50%` → `"0.5"`.
  - `functions`: `sqrt(16)` → `"4"`; `round(2.5)` → `"3"`; `ln(e)` → `"1"`.
  - `bare_number_not_calc`: `applies("42")` is false; `applies("=42")` is true.
  - `limits`: a 257-character expression → `None`; 33 nested parentheses → `None`; 32 → `Some`.
  - `errors`: `1/0` → `None`; `2+` → `None`; `foo(1)` → `None`.
  - `no_code_execution`: `= system("x")` → `None`.
  - `rounding`: `0.1+0.2` → `"0.3"`; `1/3` → `"0.333333333333"`.
- [ ] **Step 2–4:** FAIL → implement → PASS (`cargo test -p uqctl calc`).
- [ ] **Step 5:** Commit `feat(bridge): calculator provider`.

### Task 2: Run provider and `Launch {run_id}`

**Files:**
- Create `crates/uqctl/src/bridge/run.rs`.
- Modify `crates/uqctl/src/bridge/launch.rs` (replace the slice-2 stub) and
  `crates/uqctl/src/bridge/mod.rs`.

**Interfaces:**
- Consumes:
  - `ProviderItem` (Task 1).
  - `launch.rs`'s `scope_command` and `exec_lua`. A run command is wrapped as
    `argv = ["sh", "-c", cmd]` and passed through the same quoting.
- Produces:
  - `struct RunState { generation: u64, command: Option<String> }`, held in the bridge behind a
    `Mutex`.
  - `fn run_item(query: &str, generation: u64, state: &mut RunState) -> Option<ProviderItem>`:
    - only for queries starting with `>`, with the command trimmed and not empty;
    - the command is at most 4096 bytes and has no NUL;
    - stores the command for this generation;
    - id `run-<generation>`, title `Run "<cmd truncated to 80 chars>"`, subtitle `in a terminal-less scope`,
      icon `utilities-terminal`, score 1.0, actions `["run"]`.
  - `fn take_command(state: &RunState, run_id: &str) -> Result<String, String>`: returns the
    command only when `run_id == format!("run-{}", state.generation)`; otherwise
    `Err("unknown run id")`.
  - `Launch {run_id}` executes the result through `launch::run_argv(["sh","-c",cmd], "run")`. The
    scope unit is named `app-uq-run-<rand>`.
- [ ] **Step 1: Failing tests:**
  - `run_item_only_with_prefix`.
  - `empty_command_no_item`.
  - `run_id_from_old_generation_rejected`: generation 3 stores `ls`; generation 4 then stores
    `pwd`; `take_command("run-3")` → `Err`.
  - `nul_rejected`.
  - `launch_run_id_wraps_sh_c_in_scope`: compares the exact `scope_command` output for
    `["sh","-c","echo hi"]`.
  - In `launch.rs`, update the old `argv_and_run_id_and_junk` test so a valid run id is now
    accepted.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): run provider and Launch run_id`.

### Task 3: External providers, gating, `Search` and `Activate`

**Files:**
- Create `crates/uqctl/src/bridge/providers.rs` and `crates/uqctl/tests/providers_dbus.rs`.
- Modify `bridge/mod.rs`.

**Interfaces:**
- Consumes: Tasks 1 and 2; the bridge's session-bus connection and Outbound sender; the shutdown
  token.
- Produces:
  - `struct ProviderDef { bus_name: String, name: String, prefix: Option<String>, source_dir: PathBuf }`.
  - `fn load_defs(dirs: &[PathBuf]) -> Vec<ProviderDef>`:
    - reads `*.provider` files in the `[Launcher Provider]` group, one per file;
    - `BusName` must be a valid well-known D-Bus name;
    - `Name` is at most 64 characters;
    - `Prefix` is 1–4 non-space characters;
    - invalid files are skipped with a single warning on stderr;
    - definitions are re-read whenever a directory's mtime changes;
    - the default directories are `/usr/share/uq/launcher` and `$XDG_DATA_HOME/uq/launcher`.
  - `fn route(query: &str, def: &ProviderDef) -> Option<String>`: the query to send to this
    provider, or `None`. Applies the gating rules in Global Constraints.
  - `fn clamp(item: RawItem, def: &ProviderDef) -> ProviderItem`: applies the clamps in Global
    Constraints, and turns the score into `clamp(score, 0, 1) * 0.9`. `RawItem` is the
    `(s s s s s d)` tuple; the actions JSON parses to a list of strings, or `[]` if invalid.
  - Bridge call `Search(query: string, generation: u64)`:
    1. Records `latest_generation`.
    2. Emits the built-ins synchronously as `SearchResults`, with provider `calc` or `run`,
       `provider_name` "Calculator" or "Run" and `source_dir` "builtin".
    3. Then, for each routed external provider whose name has an owner (checked with `NameHasOwner`
       on each search, not cached), calls `Search` with `NO_AUTO_START` and a 300 ms timeout, in
       parallel.
    4. For each answer whose generation is still the latest, emits
       `SearchResults {generation, provider: bus_name, provider_name, source_dir, items}`. Answers
       for a stale generation are dropped.
    5. Replies `null` immediately.
  - Bridge call `Activate(provider: string, id: string, action: string)`:
    - `calc` → replies `{"copy": "<result>"}`; the shell copies it to the clipboard.
    - `run` → `take_command` then Launch.
    - External → the provider's `Activate(id, action)` with `NO_AUTO_START` and a 1 s timeout,
      only if `provider` is a known `BusName` and `id` is at most 512 bytes.
  - Errors are logged once per provider per minute.
- [ ] **Step 1: Failing tests.**

  Unit tests:
  - `route_prefix_and_gating`: with prefix `:`, `":smi"` → `"smi"` and `"smi"` → `None`; with no
    prefix, `"a"` → `None`, `"ab"` → `"ab"`, `">ls"` → `None` and `"=1+1"` → `None`.
  - `clamp_limits`: 25 items → 20; a 300-byte title is truncated to 200 bytes on a char boundary.
  - `score_clamped_and_scaled`: 5.0 → 0.9; −1 → 0.
  - `icon_clamped_to_theme_name_or_local_path`:
    - `"firefox"` and `"/usr/share/x.png"` are kept;
    - `"http://x/y.png"`, `"file:///x"`, `"../x"` and `"<img>"` become the default.
  - `load_defs_skips_invalid`.

  Integration tests (`providers_dbus.rs`), on a private bus running a fake provider served by
  zbus in the test:
  - `search_fans_out_and_tags_provider`.
  - `slow_provider_dropped_after_300ms`: the fake sleeps 500 ms; there is no event from it within
    1 s.
  - `stale_generation_results_dropped`: Search g1 against a slow fake, then Search g2 against a
    fast one; only g2 events arrive.
  - `unowned_provider_not_activated`: a `.provider` file names an unowned bus; the fake bus has an
    activatable service file for it; nothing gets started (check with `NameHasOwner` afterwards).
  - `activate_routes_to_provider`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): launcher providers with gating, clamping and generations`.

### Task 4: Power actions

**Files:** Create `crates/uqctl/src/bridge/power.rs`; modify `bridge/mod.rs`.

**Interfaces:**
- Produces: bridge call `Power(action)`. The action must be one of `lock`, `logout`, `suspend`,
  `reboot`, `poweroff`. They map to these fixed argv lists, with no shell:

  | Action | argv |
  |---|---|
  | `lock` | `["loginctl","lock-session"]` (slice 5's shell answers logind's Lock signal) |
  | `suspend` | `["systemctl","suspend"]` |
  | `reboot` | `["systemctl","reboot"]` |
  | `poweroff` | `["systemctl","poweroff"]` |
  | `logout` | `hyprshutdown` if it is on `PATH`, otherwise `eval return hl.dispatch(hl.dsp.exit())` through `HyprSocket` |

  An unknown action returns `Err("unknown power action")`.
  `fn argv_for(action: &str) -> Result<PowerCmd, String>`, where
  `PowerCmd = Argv(Vec<String>) | HyprExit`.
- [ ] **Step 1: Failing tests:**
  - `argv_table`: the exact argv for each action.
  - `unknown_rejected`: `"halt"` and `"lock; rm"` are rejected.
  - `logout_prefers_hyprshutdown`: the PATH lookup is injected.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5:** Commit `feat(bridge): closed power actions`.

### Task 5: Ranking and merge logic (`rank.js`)

**Files:** Create `shell/logic/rank.js` and `tests/qml/tst_rank.qml`.

**Interfaces:**
- Produces:
  - `matchScore(query, text) → number in [0,1]`:
    - case-insensitive;
    - 1.0 for an exact match;
    - 0.9 for a prefix match;
    - 0.75 when a word in `text` starts with the query;
    - otherwise a subsequence (fuzzy) score in (0, 0.6], favouring consecutive and early matches;
    - 0 for no match.
  - `appScore(query, entry, launchCount) → {score, kind}`:
    - takes the best of name, then genericName, keywords and comment (the latter weighted ×0.8);
    - adds `0.15 * Math.log(1 + launchCount)`;
    - `kind` is `"exact"` or `"prefix"` when the name matches that way, otherwise `"fuzzy"`.
  - `searchApps(query, entries, counts, limit=8) → [{id, name, icon, comment, score, kind}]`:
    - skips `noDisplay` entries;
    - the empty query returns `[]`.
  - `merge(apps, providerGroups, generation) → {topHit, groups:[{provider, providerName, items}]}`:
    - `providerGroups` is a map keyed by provider of the latest `SearchResults` per provider,
      including the generation;
    - groups whose generation isn't the current one are dropped;
    - top hit: the best app if its `kind` is exact or prefix; otherwise the best item across apps
      and providers by score;
    - an external provider item can be the top hit only if no app has an exact or prefix match;
    - the group order is Apps, then built-ins (Calculator, Run), then external providers in
      `.provider` name order.
- [ ] **Step 1: Failing tests:**
  - `test_exact_prefix_order`.
  - `test_launch_count_boosts_but_not_over_prefix`: a fuzzy match with 1000 launches still ranks
    below a prefix match with 0.
  - `test_provider_cannot_take_app_top_hit`: the app "Firefox" against a provider item "Firefox"
    with score 0.9, for the query `"fire"` → the top hit is the app.
  - `test_provider_top_hit_when_no_app_match`.
  - `test_merge_ignores_stale_generation`.
  - `test_nodisplay_skipped`.
  - `test_empty_query`.
- [ ] **Step 2–4:** FAIL → implement → PASS (qmltestrunner).
- [ ] **Step 5:** Commit `feat(shell): ranking and merge logic`.

### Task 6: Spotlight

**Files:**
- Create `shell/logic/launcher.js`, `shell/launcher/Spotlight.qml` and `tests/qml/tst_launcher.qml`.

**Interfaces:**
- Consumes: `rank.js`; `Bridge.call("Search"|"Activate"|"Launch")`. Don't call
  `RecordLaunch`: `Launch {desktop_id}` already records the launch in the bridge, so calling it
  here would count every launch twice. Bridge's
  `SearchResults` events (add a `searchResults(data)` signal to `Bridge.qml` if one doesn't
  exist); `Settings.launchCounts`; `DesktopEntries.applications`; `UqInput`; `UqText`.
- Produces:
  - `launcher.js`:
    - `classify(query) → "calc"|"run"|"emoji"|"search"`, from the prefixes `=`, `>` and `:`.
    - `nextGeneration(g) → g+1`.
    - `surface state machine: transition(state, event) → state`:
      - states: `closed`, `spotlight`, `appmenu`;
      - events: `toggleSpotlight`, `toggleAppMenu`, `close`, `lock`;
      - `lock` from any state → `closed`;
      - opening one surface closes the other.
    - `move(selection, groups, key) → selection`, for Up and Down (wrapping within the whole list)
      and Tab or Shift+Tab (first item of the next or previous group).
  - `Spotlight.qml`:
    - A `PanelWindow` on the focused monitor: layer Overlay, namespace `uq-spotlight`, centred,
      width `min(640, 50% of screen)`, top margin 18% of the screen height.
    - Uses `HyprlandFocusGrab`.
    - The input field is `UqInput` with `maximumLength: 256`.
    - A 70 ms debounce, then `Search(query, gen)`. Apps are scored locally on every debounced
      change.
    - The view shows the top hit row, then groups with their provider-name headers.
    - Enter activates the selection:
      - an app → `Launch {desktop_id}` and close;
      - `calc` → copy to `Quickshell.clipboardText` and show "Copied" for 600 ms;
      - `run` → `Launch {run_id}` and close;
      - an external item → `Activate` and close.
    - Esc closes. Closing always clears the query, results and selection.
    - The `:` prefix with no emoji provider shows the plain-text hint "No emoji provider
      installed".
    - The footer shows `= maths   > command   : emoji   Tab next group`.
- [ ] **Step 1: Failing tests (`tst_launcher`):**
  - `test_classify` (including `"  =1"`, which is trimmed).
  - `test_state_machine_single_surface`.
  - `test_close_and_wipe_on_lock`: in the state model, `lock` → `closed`, and the wipe flag is set.
  - `test_move_wraps_and_tab_groups`.
- [ ] **Step 2–4:** FAIL → implement → PASS; the lint passes.
- [ ] **Step 5:** Commit `feat(shell): Spotlight`.

### Task 7: App menu

**Files:**
- Create `shell/logic/appmenu.js`, `shell/launcher/AppMenu.qml` and `tests/qml/tst_appmenu.qml`.
- Modify `shell/bar/SystemIsland.qml` (the app-menu button opens the menu on that island's screen).

**Interfaces:**
- Consumes: `rank.js` `searchApps`; `Settings.values.pinned`; `DesktopEntries.applications`;
  `Bridge.call("Launch"|"Power")` (no `RecordLaunch`, see Task 6); the Task 6 state machine.
- Produces:
  - `appmenu.js`:
    - `category(entry) → one of "Development","Graphics","Internet","Multimedia","Games","Office","Settings","System","Utilities"`.
      This is the first match of the freedesktop main categories, in this mapping:
      - Development ← Development
      - Graphics ← Graphics
      - Internet ← Network
      - Multimedia ← AudioVideo, Audio, Video
      - Games ← Game
      - Office ← Office, Education
      - Settings ← Settings
      - System ← System
      - Utilities ← Utility or anything else
    - `categories(entries) → ordered [{key, count}]`: Favourites first, then All apps, then the
      categories above that have at least one app.
    - `favourites(pinned, entries) → entries`, in pin order, skipping missing ids.
    - `powerConfirm(state, action) → state`: `reboot`/`poweroff` need a second click within 5 s,
      with the label "Click again to restart" or "Click again to shut down". `lock`, `logout` and
      `suspend` act on the first click.
  - `AppMenu.qml`:
    - A `PanelWindow` (layer Top, namespace `uq-appmenu`) anchored under the System island on the
      island's screen, 440×420.
    - Uses `HyprlandFocusGrab`.
    - The header has the avatar and an apps-only search box:
      - avatar source: `~/.face`, then `/var/lib/AccountsService/icons/$USER`, then an initial
        from `$USER`;
      - only local file paths are used.
    - The left column lists categories. The right side is a grid of apps, or search results while
      typing.
    - The footer shows `$USER@$hostname` (from `Quickshell.env`) and the five power buttons
      through `Power`.
    - Launching closes the menu.
- [ ] **Step 1: Failing tests (`tst_appmenu`):**
  - `test_category_mapping`, including `AudioVideo` → Multimedia, an unknown category →
    Utilities, and no categories → Utilities.
  - `test_categories_order_and_empty_hidden`.
  - `test_favourites_order_and_missing`.
  - `test_power_confirm_double_click_and_timeout`.
- [ ] **Step 2–4:** FAIL → implement → PASS; the lint passes.
- [ ] **Step 5:** Commit `feat(shell): app menu`.

### Task 8: Keybinds, shortcuts, IPC and wiring

**Files:**
- Create `shell/launcher/Launchers.qml`.
- Modify `shell/shell.qml`, `config/hypr/base.lua`, `crates/uqd/tests/verify_config.rs` (Lua mock
  assertions), `tests/shell/bar_loads.sh` and `README.md`.

**Interfaces:**
- Consumes: Tasks 6 and 7; the `Bridge.locked` property (exists, false until slice 5).
- Produces:
  - `Launchers.qml`:
    - Owns one `Spotlight` and one `AppMenu`, and the surface state from `launcher.js`.
    - `GlobalShortcut { appid: "uq"; name: "spotlight" }` and `{ name: "appmenu" }`, each
      `onPressed` → toggle.
    - `IpcHandler { target: "uq"; function open(s: string) / close(s) / toggle(s) }`:
      - `s` must be `spotlight` or `appmenu`; anything else is ignored;
      - all calls are ignored while `Bridge.locked`.
    - `onLockedChanged`: if locked, transition `lock` (close and wipe).
  - `base.lua`:
    - `hl.bind(mainMod .. " + Space", hl.dsp.global("uq:spotlight"))`.
    - `hl.bind(mainMod .. " + SUPER_L", hl.dsp.global("uq:appmenu"), { release = true })`.
  - `bar_loads.sh` instantiates `Launchers` headless, and the LOADED line gains a field.
  - The README documents the keys and the `.provider` format.
- [ ] **Step 0:** Confirm two things against the Hyprland v0.56.2 and 0.57 sources (paths in the
  Phase 2 ledgers):
  - the key name for the left Super key in Lua binds (`SUPER_L` vs `Super_L`);
  - that a `release` bind on a modifier key fires on a tap.

  Record what you find, and use the form both versions accept.
- [ ] **Step 1: Failing tests.**
  - Extend `lua_mock_runs_every_variant`: binds contain `uq:spotlight` and `uq:appmenu`, and the
    appmenu bind has `release = true`.
  - Neither bind is `locked`. Assert in `base.lua` that the line containing `uq:spotlight` has no
    `locked`.
- [ ] **Step 2–4:** FAIL → implement → PASS. Run cargo test, qmltestrunner, the `tests/shell/*.sh`
  scripts that are safe locally (`bridge_io.sh`, `theme_dirswap.sh`, `lint_test.sh`,
  `units_test.sh`; CI runs `bar_loads.sh`) and the lint.
- [ ] **Step 5:** Commit `feat: launcher keybinds, shortcuts and IPC`. The controller pushes and
  checks CI.
