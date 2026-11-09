# uQ Phase 2 slice 4: notifications — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The Notifications island shows whichever notification daemon is in charge (noti-fi,
uQ's own built-in server, swaync, mako or dunst): its unread count, the latest app and, on hover,
the summary. Clicking it or pressing `SUPER+N` opens that daemon's centre. With no other daemon,
uQ's built-in server shows its own popups and centre.

**Architecture:**
- Pure logic, tested with qmltestrunner:
  - `shell/logic/notif.js`: choosing a backend, the island's view, and parsers for the
    swaync, mako and dunst outputs;
  - `shell/logic/sanitize.js`: the body sanitiser and the built-in server's rules (timeouts,
    caps, image and icon refs);
  - `shell/logic/notifi.js` (extended): noti-fi's `state` fields.
- `shell/notif/NotifBackend.qml` is a singleton with one interface over five adapters:
  - `Notifi.qml` (existing, extended);
  - `Builtin.qml`, the built-in server, inside a `LazyLoader` so the D-Bus name is only taken when wanted;
  - `adapters/Swaync.qml`, `adapters/Mako.qml` and `adapters/Dunst.qml`, all fixed-argv `Process`.
- Built-in popups and the built-in centre are PanelWindows in `shell/notif/builtin/`. The centre
  is a fourth surface in the `Surfaces` state machine.
- The one Rust change: the bridge's `NotifOwner` event also says which process owns the name and
  whether it's this shell.

**Tech Stack:** Quickshell 0.3.1 (`Quickshell.Services.Notifications`, `Quickshell.Io`,
`LazyLoader`), Rust edition 2024, zbus 5.

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase2-shell-design.md`. The relevant parts are
§1 (the Notifications island, threat model), §2.3 (`NotifOwner`), §2.4 (`notif.*` settings),
§2.5, §2.6, §2.10, §2.12 (`SUPER+N`), §4 and §5. noti-fi's protocol is
`~/Projects/noti-fi/docs/protocol.md`.

## Global Constraints

- **Trust model (§1):** notification content (app name, icon, summary, body, action labels,
  image) and everything read from noti-fi, swaync, mako and dunst is untrusted.
  - All text goes through `UqText` and `scripts/lint-shell.sh` must pass. The one exception is
    `shell/notif/builtin/NotifBody.qml`, which renders the sanitiser's output as `StyledText`
    and takes only the raw body (it sanitises internally).
  - Nothing is loaded from the network. Icons are icon-theme names or absolute local paths
    (`launcher.js iconRef`). Images are `image://` (Quickshell's image-data provider) or
    absolute local paths only.
- **Backends (§2.5):** the interface is `available`, `count`, `latest` (`{appIcon, appName,
  summary}` or null), `dnd`, `hasCentre`, `toggleCentre()`, `setDnd(bool)`.
  - `notif.backend` is `auto` | `notifi` | `builtin` | `swaync` | `mako` | `dunst`. `auto`
    picks noti-fi when its socket answers, otherwise built-in.
  - A chosen backend that's missing falls back to built-in with a warning (§4).
  - Detection: noti-fi by its hello; swaync `swaync-client --version`; mako `makoctl --help`;
    dunst `dunstctl is-paused`. Exit 0 means present.
  - Updates: noti-fi push (`subscribe` → `state`); swaync push (`swaync-client -swb`); mako
    polls `makoctl list` every 5 s; dunst polls `dunstctl count` every 5 s.
  - Controls: noti-fi `center`/`dnd`; swaync `-t`, `-dn`/`-df`; mako
    `makoctl mode -a|-r do-not-disturb` and no centre; dunst `history-pop` and
    `set-paused true|false`.
  - uQ never starts, stops or masks a third-party daemon.
- **Built-in server (§2.5):**
  - It registers only when the bridge's `NotifOwner` says the name has no owner (or the owner
    is this shell). Otherwise the island shows "Notifications handled by <name>".
  - `bodyMarkupSupported: true`, `bodyHyperlinksSupported: false`,
    `bodyImagesSupported: false`, `actionsSupported: true`, `imageSupported: true`,
    `inlineReplySupported: false`, `persistenceSupported: true`.
  - Bodies are sanitised to `b i u a` (plus `br`). An `a` shows its href on hover and is never
    followed.
- **noti-fi (§2.6):** fields used from `state`: `unread`, `dnd`, `center`, `sharing`, `latest`
  `{id, t, appName, appIcon, summary, read}` or null, `holding`. uQ caps its own text and
  renders it as plain text. `center {action: "toggle"}` opens its centre.
- **Privacy (§1, §2.10):** the summary shows on hover, or always when `notif.show_summary` is
  true, and never while the screen is shared (`Bridge.sharing`, or noti-fi's `sharing`). While
  sharing, built-in popups and the centre show only the app name and icon.
- **Keybind (§2.12):** `SUPER+N` → `uq:notif-centre`, not locked.
- **Process and test rules (Phase 2):**
  - Never run the bridge or the shell against the real session bus, the user's desktop or
    `hyprctl`.
  - Private dbus-daemon socket paths must be under 108 bytes.
  - Use offscreen `/usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml`, and check its
    exit code directly (never pipe it into `grep`).
  - Don't run `tests/shell/bar_loads.sh` or `scripts/smoke-shell.sh` locally; CI runs them.
  - `bar_loads.sh` runs offscreen with no PanelWindow backend, so it may only instantiate
    window-free components.
  - Singletons that import `Quickshell` or `Quickshell.Io` live outside `shell/core`.
  - Rust: `cargo fmt`, `clippy -D warnings` and `cargo test --workspace` must all be clean.

## Review Focus

1. **noti-fi starting after the shell at login.** If built-in grabbed the D-Bus name first,
   noti-fi could never own it. Under `auto` and `notifi`, the shell waits a 5 s grace period
   before falling back, and built-in unloads (freeing the name) the moment noti-fi answers.
   → `test_choose_backend_grace` (Task 2).
2. **Hostile markup in a body:** unclosed or nested tags, `<img src="http://…">`,
   `<a href="javascript:…">`, `<font>`, entities, a lone `<` or `&`, a 1 MB body. The output is
   always well-formed `b i u a br` with escaped text, and nothing else.
   → `test_sanitize_*` (Task 3).
3. **Bridge down or not yet reporting.** An unknown owner is not "no owner": built-in never
   registers while the owner is unknown. → `test_builtin_should_register` (Task 2).
4. **Screen sharing starting while a popup or the centre is up.** Summaries and bodies hide at
   once (bindings, not snapshots), and the island's summary hides.
   → `test_island_view_sharing` (Task 2), `test_popup_view_sharing` (Task 3).
5. **A notification flood** (an app sending hundreds a second). At most 3 popups on screen, at
   most 50 kept in the centre (oldest expired), and the island shows `99+`.
   → `test_store_caps` (Task 3), `test_count_label` (Task 2).

---

### Task 1: Bridge `NotifOwner` names the owner

**Files:**
- Modify: `crates/uqctl/src/bridge/notifowner.rs`
- Modify: `shell/core/Bridge.qml`
- Test: `crates/uqctl/src/bridge/notifowner.rs` (tests module), `tests/qml/tst_bridge_protocol.qml`

**Interfaces:**
- Produces: the event `NotifOwner {owner: string|null, pid: number|null, name: string|null,
  self: bool}`. `name` is `/proc/<pid>/comm`, trimmed, at most 32 chars, printable ASCII only
  (else null). `self` is `pid == std::os::unix::process::parent_id()` (the shell spawns
  `uqctl bridge` directly, `BridgeIo.qml:11`).
- Produces: `Bridge.notifOwner` property `{known: bool, owner: string|null, name: string,
  self: bool}`, `{known: false, owner: null, name: "", self: false}` initially and after
  `reset()`. The old `notifOwner(string)` signal goes (nothing uses it).

- [ ] **Step 1: Tests.** Rust: `describe_owner_self_and_name` unit test on a pure
  `fn describe(owner: Option<String>, pid: Option<u32>, comm: Option<&str>, parent: u32) -> Value`
  (self true only when pid == parent; comm with control chars or non-ASCII → null name; 40-char
  comm truncated to 32). Extend the existing private-bus test to assert the new keys are present
  and `pid` is this test process's pid when the test itself owns the name. QML:
  `test_notif_owner_event` (event sets `known: true` and the fields; a `null` owner keeps
  `known: true`; `reset()` returns to unknown; a non-object payload leaves it unknown).
- [ ] **Step 2: Run, see them fail.** `cargo test -p uqctl notifowner`, qmltestrunner.
- [ ] **Step 3: Implement.** Look the pid up with `DBusProxy::get_connection_unix_process_id` on
  the owner; any lookup error gives `pid: null, name: null, self: false`.
- [ ] **Step 4: Run Rust (fmt, clippy, test) and qmltestrunner; all pass.**
- [ ] **Step 5: Commit** `feat(bridge): NotifOwner says which process owns the name`.

### Task 2: Backend choice, island view and daemon parsers (`notif.js`)

**Files:**
- Create: `shell/logic/notif.js`
- Test: `tests/qml/tst_notif.qml`

**Interfaces** (all pure; `.pragma library`):
- `NOTIFI_GRACE_MS = 5000`, `MAX_APP = 64`, `MAX_SUMMARY = 200`.
- `cleanText(s, max) → string`: non-strings give `""`; control characters (U+0000–U+001F,
  U+007F, and the bidi overrides U+202A–U+202E, U+2066–U+2069) become spaces; trimmed; cut to
  `max` chars with `…`.
- `chooseBackend(setting, env) → {backend, warning}` where `env = {notifi: bool, graceOver:
  bool, detected: {swaync, mako, dunst: true|false|null}}` and `backend` is one of `"notifi"`,
  `"builtin"`, `"swaync"`, `"mako"`, `"dunst"`, `"pending"`.
  - `auto` (and any unknown value): noti-fi if up; else `pending` until `graceOver`; else
    built-in, no warning.
  - `notifi`: same, but the fallback carries `warning: "noti-fi is not running"`.
  - `swaync|mako|dunst`: `true` → it; `null` → `pending`; `false` → built-in with
    `warning: "<name> is not installed"`.
  - `builtin` → built-in.
- `builtinShouldRegister(backend, owner) → bool`: `backend === "builtin" && owner.known &&
  (owner.owner === null || owner.self)`.
- `countLabel(n) → string`: `""` for 0 or invalid, `"99+"` above 99.
- `islandView(b, opts) → {count, appIcon, summary, yieldedTo, dnd}` where `b` is the backend's
  `{available, count, latest, dnd, sharing}`, `opts = {sharing, showSummary, hovered,
  yieldedTo}`. `summary` is `""` unless `(hovered || showSummary) && !sharing && !b.sharing`.
  `yieldedTo` is `"Notifications handled by " + name` (`name` falls back to `"another app"`).
- `parseSwaync(line) → {count, dnd, centre}|null`: accepts both `-s`
  (`{"count":N,"dnd":bool,"visible":bool}`) and `-swb` (`{"text":"N","alt":"dnd-notification",…}`;
  `dnd` when `alt` starts with `dnd`).
- `parseMakoList(out) → {count, latest}`: JSON (`{"data":[[{…"summary":{"data":…},"app-name":…}]]}`,
  mako < 1.9) or text (`Notification N: <summary>` lines followed by `  App name: <app>`,
  mako ≥ 1.9). `latest` is the first entry.
- `parseMakoModes(out) → bool`: a line equal to `do-not-disturb`.
- `parseDunstCount(out) → number`: `Waiting` + `Currently displayed`; anything unparsable is 0.
- `parseDunstPaused(out) → bool`: `out.trim() === "true"`.

- [ ] **Step 1: Write `tests/qml/tst_notif.qml`:** `test_clean_text` (controls, bidi, cap),
  `test_choose_backend_auto`, `test_choose_backend_grace` (auto and notifi stay `pending` while
  `!graceOver`; `notifi` up at any time wins; after grace, built-in), `test_choose_backend_missing`
  (each external `false` → built-in with its warning; `null` → `pending`),
  `test_builtin_should_register` (unknown owner → false; null owner → true; other owner → false;
  self → true; not built-in → false), `test_count_label` (0, 1, 99, 100, NaN, -3),
  `test_island_view_sharing` (hovered and `showSummary` both hide the summary while either
  sharing flag is set), `test_island_view_summary` (hover only by default; always with
  `showSummary`), `test_parse_swaync` (both shapes, garbage → null), `test_parse_mako_text`,
  `test_parse_mako_json`, `test_parse_mako_modes`, `test_parse_dunst`.
- [ ] **Step 2: Run, see them fail.**
- [ ] **Step 3: Implement `notif.js`.** Every string that reaches a view goes through
  `cleanText` (app name `MAX_APP`, summary `MAX_SUMMARY`).
- [ ] **Step 4: Run qmltestrunner; all pass.**
- [ ] **Step 5: Commit** `feat(shell): notification backend choice and daemon parsers`.

### Task 3: Body sanitiser and built-in rules (`sanitize.js`, `NotifBody.qml`)

**Files:**
- Create: `shell/logic/sanitize.js`, `shell/notif/builtin/NotifBody.qml`
- Modify: `scripts/lint-shell.sh`, `tests/shell/lint_test.sh`
- Test: `tests/qml/tst_sanitize.qml`

**Interfaces:**
- `MAX_BODY_IN = 8192` (chars read; the rest is cut before parsing), `MAX_BODY_OUT = 2000`
  (visible chars), `MAX_KEEP = 50`, `MAX_POPUPS = 3`, `MAX_ACTIONS = 3`, `MAX_ACTION_TEXT = 40`,
  `DEFAULT_TIMEOUT_MS = 5000`, `MAX_TIMEOUT_MS = 60000`.
- `sanitize(body) → string`: output contains only `<b> <i> <u> <br>` and `<a href="…">`, with
  every opened tag closed in order; all other text has `& < > "` escaped.
  - Tags are case-insensitive. Unknown tags are dropped and their text kept; the text inside
    `script` and `style` is dropped too.
  - Entities `&amp; &lt; &gt; &quot; &apos; &#N; &#xN;` decode, then re-escape; any other `&`
    is escaped.
  - Stray closing tags are dropped; `\n` becomes `<br>`.
  - `href` is kept as text (escaped, at most 512 chars); an `a` without one becomes plain text.
- `plainBody(body) → string`: the same input with all markup removed (for screen readers and
  `Accessible.name`).
- `timeoutMs(expireTimeout, urgency) → number`: critical → 0 (stays); `expireTimeout` 0 → 0;
  `< 0` or invalid → `DEFAULT_TIMEOUT_MS`; else clamped to 1000…`MAX_TIMEOUT_MS`.
- `popupAllowed(dnd, urgency) → bool`: false under DnD unless critical.
- `imageRef(image) → string`: `image://…` kept; `/abs/path` → `file://` URL (as `iconRef`);
  `file:///abs/path` kept; anything else (`http:`, `https:`, `data:`, relative) → `""`.
- `popupView(n, sharing) → {appName, appIcon, summary, body, actions}`: while sharing,
  `summary`, `body` and `actions` are empty.
- `trimKept(count) → number`: how many oldest to expire to stay at `MAX_KEEP`.
- `NotifBody.qml` (the only `StyledText` in the shell): property `string raw`;
  `text: S.sanitize(raw)`; `textFormat: Text.StyledText`; `onLinkHovered` sets a tooltip-like
  `hoveredLink` shown via a `UqText` under it; `onLinkActivated` does nothing.

- [ ] **Step 1: Write `tests/qml/tst_sanitize.qml`:** `test_sanitize_allowed` (b, i, u, br,
  a), `test_sanitize_strips` (`img`, `font`, `span style`, `script` and its text, `iframe`),
  `test_sanitize_unclosed` (`<b><i>x` → `<b><i>x</i></b>`), `test_sanitize_misnested`
  (`<b><i>x</b>y</i>` is well-formed), `test_sanitize_entities` (`&lt;b&gt;` stays text, `&#60;`,
  bad `&foo;`, lone `&`, lone `<`), `test_sanitize_href` (`javascript:` and `http:` hrefs are
  only text, quotes escaped, never `onclick`), `test_sanitize_huge` (1 MB input returns in
  bounded size), `test_plain_body`, `test_timeout_ms`, `test_popup_allowed`, `test_image_ref`,
  `test_popup_view_sharing`, `test_store_caps` (`trimKept` and the `MAX_POPUPS` constant).
- [ ] **Step 2: Run, see them fail.**
- [ ] **Step 3: Implement `sanitize.js`** with a single-pass tokenizer and a stack of open
  allowed tags. No regex with nested quantifiers (no catastrophic backtracking on a 8 KiB input).
- [ ] **Step 4: Allow `NotifBody.qml` in `lint-shell.sh`** next to `UqText.qml`. Extend
  `lint_test.sh`: a `Text {` in `shell/notif/builtin/NotifBody.qml` passes; the same in
  `shell/notif/builtin/Other.qml` fails.
- [ ] **Step 5: Run qmltestrunner, `bash tests/shell/lint_test.sh`, `scripts/lint-shell.sh`; all pass.**
- [ ] **Step 6: Commit** `feat(shell): notification body sanitiser`.

### Task 4: noti-fi adapter: status, centre and latest

**Files:**
- Modify: `shell/logic/notifi.js`, `shell/notif/Notifi.qml`
- Test: `tests/qml/tst_notifi.qml`

**Interfaces:**
- `notifi.js`: `statusFields(obj) → {unread, dnd, centre, sharing, latest, holding}|null`.
  `unread` is a non-negative integer (else 0); `latest` is `{appName, appIcon, summary}` via
  `notif.js cleanText` caps, or null; `holding` is one of the six spec values, else `""`.
  The `state` effect becomes `{kind: "state", status}`; `replyStatus(method, value)` replaces
  `replyDnd` and reads `subscribe` (`value.status`), `dnd` and `center` (`value`).
- `Notifi.qml` adds `count`, `latest`, `centreOpen`, `sharing`, `holding` and
  `toggleCentre() → handle` (`center {action: "toggle"}`). All reset when the connection drops.

- [ ] **Step 1: Tests:** `test_status_fields` (full object, missing fields, wrong types,
  negative and fractional unread, a 10 KB summary is capped, an unknown `holding` → `""`),
  `test_state_effect_carries_status`, `test_reply_status` (subscribe, dnd, center, other
  methods → null). Update existing tests that used `replyDnd` or `effect.dnd`.
- [ ] **Step 2: Run, see them fail.**
- [ ] **Step 3: Implement.** `notifi.js` may `.import "notif.js" as Notif`.
- [ ] **Step 4: Run qmltestrunner; all pass.** qmllint `Notifi.qml` adds no new warnings.
- [ ] **Step 5: Commit** `feat(shell): noti-fi adapter reports unread, latest and its centre`.

### Task 5: Built-in server (`Builtin.qml`)

**Files:**
- Create: `shell/notif/Builtin.qml` (singleton; register in `shell/notif/qmldir`)
- Test: `tests/shell/notif_builtin.sh` (CI only, like `bar_loads.sh`; offscreen, a private
  dbus-daemon, `notify-send` from libnotify)

**Interfaces:**
- Properties: `wanted: bool` (set by NotifBackend), `registered: bool` (the LazyLoader's item
  exists), `dnd: bool` (in memory, not persisted), `count: int` (arrivals since the centre was
  last opened), `latest`, `notifications` (newest first; at most `MAX_KEEP`), `popups` (newest
  first; at most `MAX_POPUPS`).
- Functions: `markRead()`, `dismiss(n)`, `clearAll()`, `invoke(n, identifier)`, `expirePopup(n)`.
- The `NotificationServer` lives in a `LazyLoader { active: root.wanted }` with the flags from
  Global Constraints and `keepOnReload: false`. On `notification(n)`: `n.tracked = true`;
  add to the store; add to `popups` only if `popupAllowed(dnd, n.urgency)`; expire the oldest
  past `MAX_KEEP` (`n.expire()`). A `transient` notification leaves the store when its popup
  goes.
- Popup timers run in the singleton (one `Timer` per popup via a small component, or one
  shared 250 ms ticker with deadlines, like `Notifi.qml`'s `requestTimer`); `timeoutMs` 0 never
  expires.

- [ ] **Step 1: Write `tests/shell/notif_builtin.sh`:** start a private dbus-daemon (path under
  108 bytes), run `qs` offscreen with a shell that sets `Builtin.wanted = true`, send
  `notify-send -a Test "Hi" "<b>x</b><img src='http://e/x.png'>"`, and have the shell log
  `NOTIF <count> <sanitised body>`; assert `NOTIF 1 <b>x</b>`. Send 60 more and assert the
  store holds 50. Skip with exit 77 if `notify-send` or `dbus-daemon` is missing.
- [ ] **Step 2: Implement `Builtin.qml`.**
- [ ] **Step 3: qmllint** `Builtin.qml` with no new warnings beyond the baseline; qmltestrunner
  still passes. Add `notif_builtin.sh` to the CI shell-test loop if the loop doesn't glob
  `tests/shell/*.sh`.
- [ ] **Step 4: Commit** `feat(shell): built-in notification server`.

### Task 6: `NotifBackend` and the swaync, mako and dunst adapters

**Files:**
- Create: `shell/notif/NotifBackend.qml` (singleton), `shell/notif/adapters/Swaync.qml`,
  `shell/notif/adapters/Mako.qml`, `shell/notif/adapters/Dunst.qml`
- Modify: `shell/notif/qmldir`
- Test: `tests/shell/notif_adapters.sh` (CI only): fake `swaync-client`, `makoctl` and
  `dunstctl` scripts on `PATH` that print fixed output and log their argv

**Interfaces:**
- Each external adapter: `active: bool` (set by NotifBackend; detection and polling run only
  while true), `detected: true|false|null`, `available`, `count`, `latest`, `dnd`, `hasCentre`,
  `toggleCentre()`, `setDnd(bool)`. Polls stop and the swaync subscriber process is killed
  when `active` goes false. A crashed `swaync-client -swb` restarts with `backoff.js`.
- `NotifBackend`: `backend` (the `chooseBackend` result), `warning`, `available`, `count`,
  `latest`, `dnd`, `hasCentre`, `sharing` (`Bridge.sharing || (backend === "notifi" &&
  Notifi.sharing)`), `yieldedTo` (the owner's `name` when built-in is chosen but another
  process owns the name, else `""`), `toggleCentre()`, `setDnd(bool)`.
  - Reads `Settings.values.notif.backend` (default `auto`) and `show_summary`.
  - A 5 s grace timer from startup sets `graceOver`.
  - Sets `Builtin.wanted = builtinShouldRegister(backend, Bridge.notifOwner)`. Once built-in is
    registered, `Bridge.notifOwner.self` keeps it so.
  - Logs `warning` once per change with `console.warn`.
  - For built-in, `toggleCentre()` is `Surfaces.toggle("notifcentre", null, null)` (Task 7).
  - With `backend === "pending"`, `available` is false and the controls are no-ops.

- [ ] **Step 1: Write `notif_adapters.sh`:** for each fake daemon, a shell that sets
  `notif.backend` through a stubbed `Settings.values` and logs the backend's state after 1 s:
  swaync count from `-swb`; mako count from text `makoctl list`; dunst count. Then call
  `setDnd(true)` and `toggleCentre()` and assert the fake logged the right argv (`-dn`, `-t`;
  `mode -a do-not-disturb`; `set-paused true`, `history-pop`). With the fake removed from
  `PATH`, the backend becomes `builtin` with the warning.
- [ ] **Step 2: Implement** the three adapters and `NotifBackend`.
- [ ] **Step 3: qmllint** with no new warnings; qmltestrunner passes.
- [ ] **Step 4: Commit** `feat(shell): NotifBackend over noti-fi, built-in, swaync, mako and dunst`.

### Task 7: Built-in popups and centre

**Files:**
- Create: `shell/notif/builtin/Popups.qml`, `shell/notif/builtin/Centre.qml`,
  `shell/notif/builtin/NotifCard.qml`
- Modify: `shell/logic/launcher.js` (`SURFACES` gains `"notifcentre"`, event
  `toggleNotifCentre`), `shell/core/Surfaces.qml` (`toggle` accepts `"notifcentre"`; `lock`
  wipes it; `ipc` does **not** accept it), `shell/shell.qml`
- Test: `tests/qml/tst_launcher.qml`, `tests/qml/tst_surfaces.qml`

**Interfaces:**
- `NotifCard`: one notification. App icon (`iconRef`), app name, summary (`UqText`), body
  (`NotifBody`), up to `MAX_ACTIONS` action buttons (`cleanText(text, MAX_ACTION_TEXT)`;
  `identifier === "default"` is not a button, it's what clicking the card does), an image
  (`imageRef`, `sourceSize` × DPR, at most 64 px), and a close button (`dismiss`). Everything
  comes from `popupView(n, NotifBackend.sharing)`, so sharing hides it live.
- `Popups`: a PanelWindow, layer Top, namespace `uq-notif`, top-right of
  `Hyprland.focusedMonitor` under the bar, `Builtin.popups` in a column, no keyboard focus;
  hidden while `Bridge.locked`; the window's `mask` covers only the cards.
- `Centre`: a PanelWindow like QuickSettings (layer Top, namespace `uq-notif-centre`,
  `HyprlandFocusGrab`, Esc closes, 400 px wide, max 70% tall), visible when
  `Surfaces.open === "notifcentre"`. Header with "Clear all" and a DnD toggle, then
  `Builtin.notifications`. Opening it calls `Builtin.markRead()`. Empty state: "No
  notifications".
- Every control has an `Accessible.name`; cards use `plainBody` for theirs.

- [ ] **Step 1: Tests:** `test_transition_notifcentre` in `tst_launcher.qml` (toggles, closes
  others, `lock` wipes it); `test_ipc_rejects_notifcentre` in `tst_surfaces.qml`.
- [ ] **Step 2: Run, see them fail; implement the state-machine change; run, pass.**
- [ ] **Step 3: Implement the three QML files** and add `Popups {}` and `Centre {}` to
  `shell.qml` (not to `bar_loads.sh`: they're PanelWindows).
- [ ] **Step 4: qmllint** with no new warnings; `scripts/lint-shell.sh` passes.
- [ ] **Step 5: Commit** `feat(shell): built-in notification popups and centre`.

### Task 8: Island, `SUPER+N`, quick-settings DnD, README

**Files:**
- Modify: `shell/bar/NotifIsland.qml`, `shell/launcher/LauncherRoutes.qml` (or a new
  `shell/notif/NotifRoutes.qml` added to `shell.qml` and `bar_loads.sh`),
  `shell/quick/PowerGroup.qml`, `config/hypr/base.lua`, `crates/uqd/tests/verify_config.rs`,
  `README.md`, `tests/shell/bar_loads.sh`
- Create: `docs/notes/notifi-hyprland-check.md`

**Interfaces:**
- `NotifIsland`: everything from `islandView(...)`.
  - Shows the latest app's icon (else a bell) and `countLabel`, a DnD mark, and the summary on
    hover or always with `notif.show_summary`.
  - While yielded, it shows "Notifications handled by <name>", elided.
  - Left click → `NotifBackend.toggleCentre()` when `hasCentre`. Right click → `setDnd(!dnd)`.
  - `Accessible.name` says the count and the DnD state, never the summary.
- `GlobalShortcut { name: "notif-centre" }` → `NotifBackend.toggleCentre()`, ignored while
  `Bridge.locked`.
- `base.lua`: `hl.bind(mainMod .. " + N", hl.dsp.global("uq:notif-centre"))`, not locked.
  `verify_config.rs` asserts it exists and isn't `locked`.
- `PowerGroup`'s DnD toggle uses `NotifBackend` (`visible: NotifBackend.available`).

- [ ] **Step 1: Test:** extend `verify_config.rs`; run `cargo test -p uqd --test verify_config`
  (the non-ignored part); see it fail; add the bind; pass.
- [ ] **Step 2: Implement the island, the route and the DnD change.** Update `bar_loads.sh` if
  a new route component is added (its `LOADED` line gains a boolean).
- [ ] **Step 3: README:** a Notifications section covering the backends and `auto`; that
  built-in yields to any other owner and that uQ never stops another daemon (how to mask one:
  `systemctl --user mask swaync.service` or remove its D-Bus activation file); `SUPER+N`; the
  island's privacy rules; noti-fi's `"screen": "focused"` rules.json hint; built-in DnD isn't
  kept across shell restarts. Fix the existing `uqctl notif backend shell` example to
  `builtin`.
- [ ] **Step 4: Write `docs/notes/notifi-hyprland-check.md`** as the manual checklist for the
  first real-Hyprland run of noti-fi (§2.6 open item): takeover with `notify takeover --plain`;
  hello and subscribe from the shell; popups on the focused monitor; OSD on the focused monitor;
  unplugging a monitor moves windows off it; the centre toggles from `SUPER+N`; theme tokens
  follow `uqctl theme set`; sharing hides summaries; nothing shows above the lock. Each line
  has a result column to fill in and "report to noti-fi" for failures.
- [ ] **Step 5: Run** qmltestrunner, `scripts/lint-shell.sh`, `bash tests/shell/lint_test.sh`,
  cargo fmt/clippy/test; all pass.
- [ ] **Step 6: Commit** `feat(shell): Notifications island, SUPER+N and the README`.
