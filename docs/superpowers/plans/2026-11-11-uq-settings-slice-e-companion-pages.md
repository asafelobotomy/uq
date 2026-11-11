# uQ Settings enhancements slice E: companion app pages — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five companion apps get a page under **Apps** in uQ Settings. Each page ships from the
app's own repository, in its `uq/` folder, and is installed by the app's own package. uQ finds it
with live discovery: no restart, no log-out.

| Sub-slice (one PR each) | Repo | Page id | Page name | What the page does |
|---|---|---|---|---|
| E1 | scdr | `scdr` | Scheduling | Shows whether scdr is running or paused, CPU pressure and which apps are boosted now. Pause and resume. Turns each role on or off (focused app, audio, games, background). Picks the CPU scheduler. Lists, adds and removes your own app rules. Turns the session agent (follow the focused window) on or off. |
| E2 | crumbs | `crumbs` | Storage and cleanup | Shows reclaimable space by category, for your files and for the system. Cleans one category at a time. Lists leftovers of removed apps and moves the ones you pick to the trash. Shows recent cleanups and puts a batch back. Lists apps with their size and uninstalls one. Never touches appOFholding's AppImages. |
| E3 | noti-fi | `noti-fi` | Notifications | Do not disturb on or off. Focus mode. Mute and unmute each group. Automatic Do not disturb for full-screen apps and screen sharing. History length, whether history is kept, clearing it. A read-only list of your rules. |
| E4 | XZram | `xzram` | Memory and swap | zram size and compression algorithm, swappiness, active swap devices and files, and memory status. |
| E5 | scale_tail | `scale-tail` | Tailscale | Connect and disconnect, account and operator status, exit node, accept routes, LAN access through the exit node, the peers list, sign out. |
| E6 | uq (this repo) | — | — | Lists the pages in the docs, the CHANGELOG, `optdepends`, the live-check note and a cross-repo check of all five `uq/` folders. |

**Order:** E1, E2, E3, then E4 and E5 (spec §11 decision 5, and the user's earlier choice of
scdr, crumBS and noti-fi first). E6 lands after E3 and gains one row tick each when E4 and E5
merge. Each sub-slice is one PR in its own repository. They don't depend on each other: the order
is priority, not a dependency.

**Architecture:**
- **No uQ code changes.** The plugin-page foundation (another thread's work, merged) already gives
  pages everything they need:
  - `page.toml` with `[requires]`, `[dbus]` (with `bus = "session" | "system"`) and
    `[[command]]` (with `args` patterns and `confirm`);
  - `ctx.command.run`, `ctx.dbus.call` and `ctx.dbus.get`;
  - live discovery of `/usr/share/uq/settings/pages/<id>/`;
  - `uqctl integrations check <dir>`.

  Where a page needs more, this plan asks the foundation thread (section *Requests to the
  foundation thread*) and uses a workaround that works today.
- **Per repo**, `uq/pages/<id>/` holds:
  - `page.toml`;
  - `Page.qml`, the view only;
  - `logic.js` (`.pragma library`), the page's pure logic: parsing command output and D-Bus
    replies, building arguments, validation and the plain-language messages.

  `logic.js` is tested with Node's built-in runner, so it is tested without Qt. `Page.qml` is
  tested offscreen in each repo's CI with a fake `ctx` and a stub `org.uq.settings` module.
- **Reads and writes take different routes (E-F1).**
  - Reads that never ask for a password use `ctx.dbus` where the app has a D-Bus API (scdr,
    XZram) and a `[[command]]` otherwise (crumBS, noti-fi, scale_tail).
  - Every write that may ask for a password is a `[[command]]` that runs the app's own CLI, so
    the polkit prompt has up to 10 minutes rather than D-Bus's 2 seconds.
- **New CLI surface**, only where a page needs a write the CLI doesn't offer:
  - scdr: `scdrctl settings scheduler | rule-add | rule-remove` (E1 Task 1);
  - noti-fi: possibly `notify set <key> <value>` (E3 Task 2, after discovery).
- **Packaging.** Each package installs its `uq/pages/<id>/` to
  `/usr/share/uq/settings/pages/<id>/` unconditionally, and lists `uq` as an `optdepends`.
- **CI.** Each repo's CI:
  - builds `uqctl` from a pinned uQ revision;
  - runs `uqctl integrations check uq/` (with the page's programs present, see E-F3);
  - runs the Node tests and the offscreen QML test.

**Tech Stack:** QML (QtQuick, QtQuick.Controls, QtQuick.Layouts; `org.uq.settings` for
`UqLabel` and `UqComboBox`), JavaScript (`.pragma library`, Node 22 `node:test`), Rust (scdr's
`scdrctl`; crumBS and the others unchanged unless noted), POSIX sh (install scripts), PKGBUILD,
GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. §8 and §12
row E are the authority. This plan also covers:
- Phase 4 spec (`2026-11-10-uq-phase4-integrations-design.md`):
  - §2.6, where files live and how they install;
  - §2.7, the tooling;
  - §3, the per-repo page contents;
  - §10, the companion pages moved earlier.
- §9, the quality rules: plain names, two clicks, no silent failure.

It builds on the foundation thread's merged work, read at uq `f2f6368`:
- `docs/settings-pages.md` and `docs/integrating.md`;
- `templates/settings-page/`;
- `crates/uq-core/src/integration/`;
- `apps/uq-settings/src/plugins.rs`, `plugin_host.rs` and `qml/PluginPage.qml`;
- `qml/components/PluginContext.qml`;
- `crates/uqctl/src/integrations.rs`.

It reads the companion repos at scdr `189704d` and crumbs `e465346`. For noti-fi, XZram and
scale_tail it uses GitHub code-search fragments only (E-F17).

**Paths.**
- The companion clones are `/home/user/scdr` and `/home/user/crumbs`.
- noti-fi, XZram and scale_tail are not cloned here. Their tasks are marked **needs repo
  access**: clone `asafelobotomy/noti-fi`, `asafelobotomy/XZram` and
  `asafelobotomy/scale_tail` to `/home/user/<repo>` before starting them.
- In this repo, the settings app is `apps/uq-settings`.

**Ownership.** The foundation thread owns these paths. This plan doesn't edit them; anything they
lack is a request (below):
- `docs/settings-pages.md` and `docs/integrating.md`;
- `templates/` (all of it, including `templates/settings-page/`);
- `crates/uq-core/src/integration/`;
- `apps/uq-settings/src/plugins.rs`, `apps/uq-settings/src/plugin_host.rs`;
- `apps/uq-settings/qml/PluginPage.qml` and `apps/uq-settings/qml/components/PluginContext.qml`,
  `CommandConfirmDialog.qml`;
- `crates/uqctl/src/integrations.rs`;
- `tests/shell/integrations_gate.sh` and its fixtures.

These shared uQ files get **additive edits only** (new lines, table rows, list entries; no
reformatting or reordering):
- `README.md`;
- `CHANGELOG.md`;
- `packaging/uq/PKGBUILD` (`optdepends` lines);
- `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` (a note in §8).

In each companion repo, follow that repo's `AGENTS.md`. Never tag, release or push to the AUR:
those are the user's steps in every one of these repos.

## Requests to the foundation thread

These are things a companion page would use that the foundation doesn't offer today. **This plan
doesn't build them.** Task E0 sends them to the foundation thread as requests. Every sub-slice
works without them through the workaround named, and switches over only if and when a request
lands (Notes question 2).

- **R-A, longer D-Bus deadlines for writes that may ask for a password.**
  - **Today:** "Every request ends within 2 seconds" (`docs/settings-pages.md`), but scdrd and
    xzramd call polkit with user interaction allowed. A password prompt takes longer, so the
    page would get `{error}` while the write still goes ahead. That is a silent success that
    reads as a failure.
  - **Ask:** an optional per-page `[dbus] slow_methods = [...]`, or `timeout_ms` up to 120 s,
    for listed methods only.
  - **Workaround (used):** writes go through the app's CLI as `[[command]]` (E-F1).
- **R-B, change notifications for pages.**
  - **Today:** widgets get `ctx.dbus.props`, `onProps` and `onSignal`; pages get only `call`
    and `get`.
  - **Ask:** the same `props`/`onProps` for pages, with `[dbus] signals` honoured.
  - **Workaround (used):** poll a read every 5 s while the page is visible and the window is
    active (R4).
- **R-C, `uqctl integrations check` in CI without the app installed.**
  - **Today:** `check` reports `command "<id>": /usr/bin/<prog> does not exist` for every
    program that isn't installed. scale_tail's `uq/README.md` already says "clean apart from the
    programs it can't find".
  - **Ask:** `--programs-optional`, which reports missing programs as notes rather than problems.
    Also a static `uqctl` binary as a uQ release asset, so CI doesn't build uQ.
  - **Workaround (used):** CI installs the repo's freshly built binaries into `/usr/bin` of the
    throwaway runner, plus empty executable stubs for programs from other packages
    (`systemctl`, `tailscale`), and builds `uqctl` from a pinned uQ revision (E-F3, E-F4).
- **R-D, a stable list of components pages may use.**
  - **Today:** only `UqLabel` is documented. `org.uq.settings` also exports these, which pages
    could use:
    - `UqComboBox` (plain-text combo);
    - `SettingRow` and `SwitchRow`;
    - `ErrorCard`;
    - `AdvancedDetails`;
    - `ValueSlider`.

    Spec §8 says companion pages "use the same row and label components".
  - **Ask:** document a frozen subset as part of `uq_api = 1`, and ship a test stub module
    (`templates/settings-page/tests/stub/org/uq/settings/`) that repos can vendor.
  - **Workaround (used):** pages use only `UqLabel` and `UqComboBox`, plus stock `QQC2.Button`,
    `QQC2.Switch`, `QQC2.CheckBox`, `QQC2.SpinBox`, `QQC2.TextField` (user input only) and
    layouts. Buttons and switches carry fixed strings only. Each repo vendors a 3-file stub
    module for its QML test (R6).
- **R-E, sibling files in a page folder.**
  - **Today:** the loader resolves `qml` inside the folder and doesn't forbid other files, and
    `check` lints every `.qml` and `.js` in the folder. But the docs say "a `page.toml` and one
    QML file".
  - **Ask:** confirm, and document, that a page may ship a sibling `logic.js` (imported as
    `import "logic.js" as Logic`) and further `.qml` component files.
  - **Workaround (used):** one `Page.qml` plus one `logic.js`. If R-E is refused, `logic.js` is
    inlined into `Page.qml` as functions, and the Node tests read the functions out of
    `Page.qml` between `// logic:begin` and `// logic:end` markers (R5).
- **R-F, opening an app from a page.**
  - **Today:** a command's stdout is read to EOF. A launched GUI app (`gtk-launch`, `notify-panel`)
    inherits the pipe, so the command hangs until its timeout and the app is then killed with
    the process group.
  - **Ask:** `ctx.openApp(desktopId)` through uQ's own launch path.
  - **Workaround (used):** no launch buttons. Pages say where the full app is ("More in noti-fi's
    own app, in the app menu"). Nothing tries to launch.

## Pre-flight rulings

The plan's author read the foundation code and the two cloned companion repos before writing the
tasks. Each finding has a ruling, and the task text below carries it out. This list is the index.

- **E-F1 Polkit and the 2 s D-Bus deadline.**
  - scdrd's `Pause`, `Resume`, `SetRoleEnabled`, `SetSettings`, `TryScheduler` and `Reload` call
    `authorize()`, which allows user interaction. So do xzramd's write methods
    (`zbus_polkit` per method, `docs/AUDIT.md`).
  - Page D-Bus requests end at 2 s (`dbus_json.rs`).

  **Ruling:**
  - `[dbus] methods` lists **read** methods only.
  - Every write is a `[[command]]` that runs the app's CLI, which waits for polkit as long as it
    takes. Timeouts: 120 s for writes that may prompt, 600 s for crumBS removals.
  - This also keeps each page's D-Bus allowlist to methods that can't change anything.
- **E-F2 Pages get no change signals (R-B).**
  - **Ruling:** a page refreshes when it opens, after each command finishes, and every 5 s while
    `visible && Qt.application.state === Qt.ApplicationActive`.
  - Only the cheap read polls:
    - scdr's `State` property;
    - noti-fi's `status`;
    - scale_tail's `status`.

    Heavy reads (crumBS `scan`, the history, apps, peers) run on open and on a **Refresh**
    button, never on a timer.
- **E-F3 `check` needs each `argv[0]` to exist** (`integrations.rs` `commands()`).
  - **Ruling:** each repo's CI job runs in a throwaway runner. It installs the repo's own release
    binaries to `/usr/bin` (`sudo install`), and `sudo install -m755 /dev/null /usr/bin/<prog>`
    for each program from another package.
  - A local run here filters only the "does not exist" lines. It must print nothing else.
  - The CI script derives the stub list from the manifest, so a typo in an `argv[0]` path under
    `/usr/bin` still fails. The script checks the path is in a fixed list per repo before
    stubbing it.
- **E-F4 No uQ release has `integrations check` yet.** The workspace is 0.2.1; `check` sits in
  `## [Unreleased]`. **Ruling:**
  - CI builds `uqctl` with
    `cargo install --locked --git https://github.com/asafelobotomy/uq --rev "$UQ_REV" uqctl --root "$HOME/.uq"`.
    The build is cached by `actions/cache`, keyed on `UQ_REV`.
  - `UQ_REV` is the uq `main` commit that the implementer reads at the start of each sub-slice
    (`git -C /home/user/uq rev-parse origin/main`), written in the workflow with a comment.
  - When a uQ tag ships `check`, a one-line follow-up switches to `--tag`.
- **E-F5 `requires.bus_name` turns off D-Bus activation** (`plugin_host.rs:204`,
  `auto_start = m.requires.bus_name.is_none()`). The page would also disappear while the daemon
  is stopped.
  - scdrd is a plain system service (not activatable).
  - xzramd **is** activatable (`io.github.XZram1.service`) and "starts on demand".

  **Ruling:**
  - Both pages use `[requires] bins` only (`scdrctl`, `xzram`), so they show while the app is
    installed.
  - The scdr page shows a "scdr isn't running" state with a **Start scdr** button (`systemctl
    enable --now scdrd.service`, systemd's own polkit prompt).
  - The XZram page lets activation start xzramd.
- **E-F6 Plain text.**
  - Every string from a command or D-Bus is untrusted: process names, rule globs, app names,
    file paths, peer names, account names.
  - **Ruling:** shown only through `UqLabel`, or `UqComboBox` for lists. Never through a
    stock control's `text:`, a `ToolTip`, `Kirigami.FormData.label` or a dialog title. Button
    and switch texts are fixed literals. No Kirigami imports at all (keeps pages
    style-independent and avoids `FormData.label`).
- **E-F7 Sibling `logic.js` (R-E).** **Ruling:**
  - `uq/pages/<id>/logic.js` starts `.pragma library` and holds no `Qt.*` calls, so Node can
    load it.
  - `check` lints it too (it lints `.js`), so it must never contain `Text {`-style text either.
- **E-F8 Argument limits.** Each argument is at most 4096 bytes, at most 16 per command, and
  never starts with `-`. Fixed `argv` may hold flags. **Ruling:**
  - No JSON documents pass as arguments.
  - Writes take small positional arguments, each with the tightest anchored pattern that works.
  - Where a CLI only takes a flag form (`--exit-node=<x>`), the flag goes in `argv` and the value
    follows as its own argument (`tailscale set --exit-node <x>`; Go's flag package accepts both).
- **E-F9 `confirm` substitutes only the first argument.** **Ruling:** argument order puts the
  thing the user recognises first (the app id, the category, the path, the rule's name). A
  confirm that would read badly with `%s` has no `%s`.
- **E-F10 scdr's session agent.** `config/hypr/base.lua:43` restarts `scdr-session` at login
  only when `is-enabled`. **Ruling:**
  - "Follow the focused window" is `systemctl --user enable --now scdr-session.service` /
    `disable --now`, and its state is `systemctl --user is-enabled`. That persists across logins
    and matches uQ's own start rule.
  - The `--user` manager is reached through `XDG_RUNTIME_DIR` and `DBUS_SESSION_BUS_ADDRESS`,
    both kept by the scrubbed environment (`docs/settings-pages.md`).
- **E-F11 scdr's settings layer is one JSON document.** `SetSettings(json)` replaces the whole
  layer (`/var/lib/scdr/settings.toml`). Layer rules come before the shipped and `/etc` rules,
  and the first match wins (`ConfigRules.qml`). **Ruling:**
  - scdr gains `scdrctl settings` subcommands that read `Settings`, change one thing in the
    layer, and write it with `SetSettings` (E1 Task 1). The page never sends a document.
  - A rule from the page is `{profile, name = "<glob>"}`. `rule-remove` checks the name at the
    index before removing, so a concurrent edit (the Plasma widget) can't remove the wrong rule.
- **E-F12 crumBS needs `--yes` with `--json`** (exit 2 otherwise). Without a terminal it refuses:
  - a running app (exit 1, unless `--force`);
  - a uQ desktop package (exit 1, unless `--desktop`).

  **Ruling:** the page passes `--yes --json` and **never** `--force`, `--desktop`, `--permanent`
  or `-i`. The refusal text is shown as is (`errors[0].message`, plain text).
- **E-F13 crumBS exit codes** (0, 1, 2, 3 partial, 4, 5 no admin rights).
  - `ctx.command.run` gives `{code, error, output}` for non-zero codes, with `output` the parsed
    envelope when it parses. **Ruling:** `logic.js` reads the envelope for every code. On 5 it
    shows "Couldn't get administrator rights", with `data.manual_commands` under **Advanced
    details** (a plain list).
- **E-F14 Changers that stream JSON lines.** noti-fi's changing commands "stream progress events,
  one JSON object per line" (its README), so `output = "json"` would fail to parse. **Ruling:**
  - changers use `output = "text"` (or `"none"`) and the page re-reads `status`;
  - only single-document reads use `"json"`.
- **E-F15 Removal semantics differ by crumBS category.** `trash` and `temp` are always permanent
  deletes. `aur-build` is permanent. `containers` runs engine prunes. `flatpak-unused` runs
  Flatpak. The others go to the trash and can be put back. **Ruling:** two commands with
  different confirms:
  - `clean` covers the trash-backed categories;
  - `clean-for-good` covers the permanent ones.

  System categories are a third, `clean-system`, whose confirm mentions the password. The
  argument patterns split the category names exactly (R7), so a category can't be cleaned
  under the gentler confirm.
- **E-F16 scale_tail already has `uq/`.** It has `uq/widgets/scale-tail/` and a README, and its
  AUR PKGBUILD "installs `scale_tail`, its desktop entry and the uQ integration". **Ruling:** E5
  adds `uq/pages/scale-tail/` only, after confirming the PKGBUILD copies the whole `uq/` tree
  (and fixing it if it copies `widgets/` only).
- **E-F17 Repo access.**
  - scdr and crumbs are cloned and read in full where relevant.
  - noti-fi, XZram and scale_tail were seen only through code-search fragments: noti-fi's
    protocol, client, panel draft keys and packaging; XZram's read method list and packaging;
    scale_tail's widget manifest, README and CI.

  **Ruling:** E3, E4 and E5 each start with a **discovery task** that confirms or corrects every
  argv, method and key this plan assumes. The tasks are written against what the fragments show,
  and marked *(confirm in discovery)* where they guess.
- **E-F18 Licences.**
  - crumBS is GPL-3.0-or-later, scdr MPL-2.0, uQ, noti-fi and scale_tail MIT.
  - Each page is written fresh in its own repo, under that repo's licence. uQ loads it at runtime
    as a separate file; nothing is copied between repos.
  - uQ's docs name commands and page contents only.
- **E-F19 Ids.** A page id is global across installed pages and must equal its folder name.
  **Ruling:** ids are the app's own short name, as in the table above. This avoids a clash with
  any future uQ page and with scale_tail's widget id `scale-tail` (different directory, so
  allowed).
- **E-F20 The dev session.** `uq-dev session` shares the real user manager and the real system bus
  (`docs/developing.md`), so these pages act on the real scdrd, xzramd, tailscaled and real files.
  **Ruling:** no special case; the live-check note says so.
- **E-F21 File-length rules.**
  - scdr limits `.rs` files to 400 lines (`tests/file_length.rs`); crumBS splits large modules.
  - **Ruling:** `Page.qml` stays at or under about 400 lines; the logic lives in `logic.js`.
    If crumBS's page outgrows that, split it into sibling components **only after R-E is
    confirmed**. Otherwise show fewer sections, with apps and uninstall going first (Notes
    question 4).

## Plan rulings

These were decided when the plan was written.

- **R1, the folder.** `uq/pages/<id>/` holds `page.toml`, `Page.qml` and `logic.js`. Beside it,
  `uq/README.md` says what the folder adds and how it was tested (Phase 4 §2.6). Tests live in
  `tests/uq/`:
  - `logic.test.mjs`;
  - `load-logic.mjs`;
  - `tst_page.qml`;
  - `stub/org/uq/settings/{qmldir,UqLabel.qml,UqComboBox.qml}`.

  Nothing under `tests/` is installed.
- **R2, page states.** Every page shows exactly one of these at its top, in a `UqLabel`:
  - **Loading:** "Checking…".
  - **Not running:** the app's service isn't answering. One plain sentence, plus the fix as a
    button where one exists.
  - **Error:** a read failed. The message comes from `logic.js`, with the raw error under
    **Advanced details** (a `QQC2.Button` "Advanced details" that shows a `UqLabel`).
  - **Ready.**

  A command in flight disables its own control and shows "Working…" beside it. A failed write
  puts the control back to the last value read and shows why (§9 "no silent failure"). It never
  just snaps back.
- **R3, plain names (§9).**
  - Labels never show `D-Bus`, `polkit`, `JSON`, `systemctl`, daemon binary names (`scdrd`,
    `xzramd`, `notifd`, `tailscaled`), `sched_ext`, `EEVDF`, `zram-generator`, `sysctl` or bus
    names.
  - The app's own product name (scdr, crumBS, noti-fi, Tailscale) may appear.
  - Raw names go under Advanced details only.
  - `tests/uq/logic.test.mjs` has a `page_strings_are_plain` test. It extracts every string
    literal on lines of `Page.qml` that contain `text:` or `confirm` and checks them against
    that word list. The same check runs on `page.toml`'s `name`, `label` and `confirm` values.
- **R4, polling.** One `Timer { interval: 5000; repeat: true }` per page, with
  `running: page.visible && Qt.application.state === Qt.ApplicationActive && !page.busy`. It
  runs only the cheap read named in E-F2.
- **R5, the Node harness.** `tests/uq/load-logic.mjs` reads `logic.js`, drops lines starting
  `.pragma` or `.import`, and runs it in `vm.createContext({})`. It returns the context, whose
  top-level `function`s are the API. If R-E is refused, it reads the text between
  `// logic:begin` and `// logic:end` in `Page.qml` instead (same API).
  `node --test tests/uq/` runs it.
- **R6, the QML test.** `tests/uq/tst_page.qml` (`TestCase`, `QT_QPA_PLATFORM=offscreen`,
  `qmltestrunner -import tests/uq/stub -input tests/uq/tst_page.qml`):
  - creates `Page.qml` with `Qt.createComponent("../../uq/pages/<id>/Page.qml")`;
  - sets a fake `ctx`: `{theme: {}, command: {run(id, args, cb)}, dbus: {call(m, a, cb), get(p, cb)}}`,
    which records every call and answers from fixtures;
  - asserts which calls happen and which `UqLabel` texts show.

  The stub `UqLabel` is a `Label` with `textFormat: Text.PlainText`, and the stub `UqComboBox`
  is a plain `ComboBox`. Both are allowed because `tests/` is outside `uq/`.
- **R7, crumBS categories.**
  - **`clean` (to the trash):** `cache|thumbnails|browser-cache|dev-cache|appimage-old|logs|app-cache|build-output`.
  - **`clean-for-good`:** `trash|temp|aur-build|containers|flatpak-unused`.
  - **`clean-system`:** `pkg-cache|orphan-pkgs|journal|coredumps|system-temp|flatpak-unused-system|snapshots`.

  `logic.js` `cleanCommandFor(category)` returns the command id. It returns `null` for an
  unknown name, and the page then hides the button. A test checks that the three lists are
  disjoint and together equal crumBS's twenty documented names (`docs/json-schema.md`).
- **R8, confirms.** Exact texts (no markup). `%s` is the first argument, cleaned by uQ:

  | Repo | Command | Confirm |
  |---|---|---|
  | scdr | `pause` | "Pause scdr? Every app's priority goes back to normal until you resume." |
  | scdr | `agent-off` | "Stop following the focused window? The app you're using no longer gets a boost." |
  | scdr | `rule-remove` | "Remove your rule for %s?" |
  | crumbs | `clean` | "Clean up %s? Items go to the trash, and crumBS can put them back." |
  | crumbs | `clean-for-good` | "Clean up %s? This deletes them for good." |
  | crumbs | `clean-system` | "Clean up %s for the whole system? This needs your password and can't be undone." |
  | crumbs | `orphan-trash` | "Move %s to the trash?" |
  | crumbs | `uninstall` | "Uninstall %s and remove its leftovers? A removed package can't be put back by crumBS." |
  | crumbs | `restore` | "Put back the files from this cleanup?" |
  | noti-fi | `history-clear` | "Clear the notification history? This can't be undone." |
  | XZram | each write that changes active swap | *(text set in E4 discovery)* "Change the zram size? Swap in use is moved first; this can take a while." |
  | scale_tail | `down` | — (not destructive: no confirm) |
  | scale_tail | `logout` | "Sign out of Tailscale on this computer? You'll need to sign in again in a browser." |

  Anything that only reads, or that the user can undo with the same control (switches), has no
  confirm.
- **R9, packaging.** In each repo the main package installs:
  ```sh
  install -Dm644 -t "$pkgdir/usr/share/uq/settings/pages/<id>" uq/pages/<id>/page.toml uq/pages/<id>/Page.qml uq/pages/<id>/logic.js
  ```
  through the repo's shared install script where one exists (scdr `packaging/install.sh`, XZram
  `Makefile`, noti-fi `packaging/install.sh`). It also adds to `optdepends`:
  `'uq: the <Page name> page in uQ Settings'`. Files are installed one by one, so nothing from
  `tests/` or a stray file is ever shipped. `pkgrel` and `pkgver` are not bumped: releases are
  the user's step.
- **R10, the CI job.** Each repo gets a job `uq` (GitHub Actions, `ubuntu-latest`, actions pinned
  by full SHA with the version in a comment, as each repo already does). It:
  1. installs Qt 6 QML test tools with apt (scale_tail's list: `qt6-declarative-dev-tools`,
     `qml6-module-qtquick`, `qml6-module-qtquick-layouts`, `qml6-module-qtquick-controls`,
     `qml6-module-qttest`, `qml6-module-qtqml-workerscript`, `qml6-module-qtquick-window`,
     `qml6-module-qtquick-templates`);
  2. runs `node --test tests/uq/`;
  3. runs `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import tests/uq/stub -input tests/uq/tst_page.qml`;
  4. restores or builds `uqctl` at `UQ_REV` (E-F4);
  5. builds the repo's binaries and installs them, plus the fixed stub list, into `/usr/bin`
     (E-F3);
  6. runs `uqctl integrations check uq/` and expects `ok:` and exit 0.
- **R11, uQ's own cross-check.** `.github/workflows/companions.yml` in this repo runs weekly, on
  `workflow_dispatch`, and on pull requests that touch the foundation paths. It:
  - builds `uqctl` from the checkout;
  - checks out each public companion repo at `main`;
  - stubs every `argv[0]` its manifests name;
  - runs `uqctl integrations check <repo>/uq`.

  A repo without `uq/pages/` yet is skipped with a note. This catches a foundation change that
  would break an installed companion page. It never gates unrelated uQ PRs.

**Spec deviations (recorded here and in each PR):**
1. **Writes go through each app's CLI, not D-Bus (E-F1).** The spec names D-Bus backends for scdr
   and XZram. Reads still use D-Bus there.
2. **Companion repos gain CLI surface.** scdr gets `scdrctl settings …` (E-F11). noti-fi possibly
   gets `notify set` (E3). §8 said "the `uq/` folder only" for some repos.
3. **The noti-fi page has no do-not-disturb schedule.** noti-fi has none (E3 discovery confirms).
   It offers the automatic DND it has (full-screen, screen sharing) and focus modes instead
   (Notes question 1).
4. **Per-app rules on the noti-fi page are read-only.** Editing rules stays in noti-fi's own app
   (R-F: no launch button).
5. **Pages poll** (R-B, R4) rather than follow signals.
6. **scdr: no "try a scheduler" and no per-power-profile scheduler choice** on the page. Those
   stay in scdr's own config and Plasma widget.
7. **Page ids are app names** (E-F19), not the page titles in §8's table.

## Global Constraints

These are copied from the spec and the user's standing rules, except the lines marked
**ruling**, which this plan decides.

### Process

- **K4, no Qt here.** The build container has no Qt and no `qmltestrunner`, so `Page.qml` and
  `tst_page.qml` run in CI only. Node 22 is at `/opt/node22/bin/node`, so `node --test
  tests/uq/` runs locally.
- **Build parallelism (earlyoom).** Every cargo command uses `-j 2`, and test runs add
  `-- --test-threads=2`.
  - Set `CARGO_BUILD_JOBS=2` and `CARGO_INCREMENTAL=0` in the shell before the first build.
  - Don't run two cargo commands at once.
  - Run builds in the foreground and leave nothing running.
- **Local `check` (E-F3).** Build `uqctl` once in this repo
  (`cargo build -j 2 -p uqctl`, from `/home/user/uq`). Then run
  `/home/user/uq/target/debug/uqctl integrations check /home/user/<repo>/uq | grep -v 'does not exist'`.
  Expected output: the summary line only, plus the trailing `N problem(s)` line when programs are
  missing.
- **K5, commits.** Every commit message, in every repo, ends with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc, so the trailers sit on their own lines after a blank line.
  PR descriptions end with the Claude Code line from the session's attribution rules.
- **One branch and one PR per repo:** `uq-settings-page` in each companion repo, and
  `settings-slice-e` here. Commit only when a task says so. Don't push to `main`, tag, release
  or touch the AUR.
- **Each repo's own rules hold** (its `AGENTS.md`):
  - **scdr:** edition 2024, MSRV 1.85, no `.rs` file over 400 lines, and
    `cargo clippy --locked --all-targets -- -D warnings` plus `cargo test --locked` before each
    commit. Don't commit build output under `packaging/arch`.
  - **crumbs:**
    - `cargo fmt --all` and clippy with `--workspace --all-targets --all-features`;
    - `cargo test --workspace --all-features --locked`;
    - user-facing changes in `README.md` and `CHANGELOG.md` under `## Unreleased`;
    - actions pinned by SHA;
    - no new JSON fields without `docs/json-schema.md` (this plan adds none).
  - **noti-fi, XZram, scale_tail:** read their `AGENTS.md` in the discovery task and follow it.
- **No new crates** in any repo. `Cargo.lock` changes only if a repo's own new code needs a
  dependency it already has; it should not change at all.
- **Ownership (above).** No edits to foundation paths. Make additive edits only to the shared uQ
  files.

### The page contract (from `docs/settings-pages.md`; checked by `uqctl integrations check`)

- `uq_api = 1`, `id` equal to the folder name, `qml = "Page.qml"`, an `icon` that is a theme icon
  name, and `keywords`.
- `[[setting]]` rows have ids `[A-Za-z0-9._-]{1,64}`, and **every row has keywords** (§9 "two
  clicks": search must find each one).
- `argv[0]` is absolute under `/usr/bin` or `/usr/lib`. `args` patterns are anchored `^…$`, as
  tight as works, and never match a leading `-`.
- `[dbus]`:
  - `bus = "system"` for scdr and XZram;
  - `methods` and `properties` list only what the page reads;
  - `requires.bus_name` is not used (E-F5).
- All text goes through `UqLabel`. No `Text`, `Label`, `TextEdit`, `TextInput` or
  `Qt.createQmlObject` anywhere under `uq/`, including `logic.js`.
- Every callback handles `{error}` and `{cancelled: true}`. On Cancel the page does nothing and
  shows nothing.

### Untrusted input

- **What's untrusted:** command output, D-Bus replies, and everything the user types. Only
  `logic.js` parses, and it parses defensively:
  - wrong types become a "couldn't read" state, never an exception;
  - arrays are capped at 500 rows for display;
  - strings are cut to 200 characters for display.
- **Typed input** (a rule's app name, a zram size) is checked by `logic.js` against the **same
  pattern** as the `args` in `page.toml`, before `run` is called. The page shows the reason
  inline and doesn't call `run`. A test asserts the patterns in `logic.js` and `page.toml` are
  identical strings (`patterns_match_manifest`).

## Review Focus

1. **Nothing destructive without a confirm.** Every command in R8 has exactly that `confirm`. No
   other command deletes, uninstalls, pauses, signs out or clears. Tests:
   - each repo's `confirms_match_plan` (a Node test that parses `page.toml` with a tiny TOML
     subset reader and compares against a table);
   - `uqctl integrations check` for the manifest itself.
2. **Arguments can't escape.** Every `args` pattern is anchored and rejects these, both in
   `logic.js` and in the manifest:
   - a leading `-`;
   - a newline;
   - an empty value;
   - an over-long value.

   No command takes a free path except crumBS `orphan-trash` (absolute, no control characters),
   and that path comes from crumBS's own `orphans --json`.

   Tests: `patterns_match_manifest`, plus `rejects_hostile_args`. That test checks `-rf`,
   `--force`, `a\nb`, `x` × 5000, `../x` where a name is expected, and `$(id)` (accepted as
   data where the pattern allows it: no shell is ever involved).
3. **Writes take the CLI route (E-F1).** `[dbus] methods` holds no method that changes state:
   - scdr: `Status`, `Processes`, `Settings`;
   - XZram: the `Get*`/`List*` methods only.

   Test: `dbus_is_read_only` in each Node suite checks the method list against an allowlist.
4. **No silent failure (§9).** Each write's failure path returns the control to the last read
   value and shows the message, as a `tst_page.qml` case with the fake `ctx` answering
   `{code: 1, error: "…"}`. Cancel is not a failure: `{cancelled: true}` changes nothing and
   shows nothing.
5. **crumBS never forces.** No argv holds `--force`, `--desktop`, `--permanent` or `-i`. The
   categories split by R7 can't be cleaned under the wrong confirm. Tests:
   `no_dangerous_flags` and `category_lists_partition`.
6. **Plain text (E-F6, R3).** No stock control shows untrusted text. Test: each repo's
   `page_strings_are_plain`, and `tst_page.qml` feeding a rule name `<b>x</b>` and a peer name
   with `‮` and checking the shown `UqLabel` text equals the input exactly. `UqLabel` is
   plain text, so a match proves no interpretation.
7. **Packaging installs exactly three files** per page, to the right directory, from the main
   package. Test: each repo's packaging check (scdr: a `tests/uq/install.sh.test` shell check run
   by the CI job; crumBS: `scripts/tests/aur.sh` gains the assertions; the others: a step in the
   CI job that runs the install script into a temp root and lists it).

---

### Task E0: Send the requests to the foundation thread (uQ, no file changes)

**Files:** none.

- [ ] **Step 1:** Send R-A to R-F (the section above, verbatim, with the "workaround used" lines)
  to the foundation thread through the coordinator. Say clearly that none blocks slice E.
- [ ] **Step 2:** Record the answers in this plan's Notes question 2 when they arrive. Don't wait
  for them: E1 starts at once.

---

## E1: scdr (`/home/user/scdr`, branch `uq-settings-page`)

### Task E1.1: `scdrctl settings`: change one thing in the settings layer

**Files:**
- Create:
  - `src/config/edit.rs`: pure layer edits on `serde_json::Value`;
  - `src/config/edit/tests.rs`;
  - `src/bin/scdrctl/settings.rs`: the subcommand, D-Bus round trip.
- Modify:
  - `src/config/mod.rs` (`pub mod edit;`);
  - `src/bin/scdrctl/main.rs`: one `Settings { #[command(subcommand)] action: settings::Action }`
    arm, its `match` arm and `mod settings;`. Keep `main.rs` under 400 lines. It is near the
    limit, so if it would pass 400, move the new parsing into `settings.rs`.
  - `src/dbus.rs`: no change. The proxy already has `settings()` and `set_settings()`.

**Interfaces (produced, `scdr::config::edit`):**
- `pub fn set_scheduler(layer: &mut Value, choice: &str) -> Result<(), EditError>`:
  - with `choice == "eevdf"`, sets `sched_ext.enable = false` and `sched_ext.default = "eevdf"`;
  - otherwise sets `sched_ext.enable = true` and `sched_ext.default = choice`;
  - it validates with `SchedChoice::from_str` first;
  - other `sched_ext` keys (gaming, power) are untouched.
- `pub fn add_rule(layer: &mut Value, profile: &str, name: &str) -> Result<usize, EditError>`:
  - appends `{"profile": profile, "name": name}` to `layer.rule` (creating the array) and
    returns its index;
  - refuses an empty name, a name with `/` (use exe rules in a file instead), more than 64
    characters, and a duplicate rule (same profile and name).
- `pub fn remove_rule(layer: &mut Value, name: &str, index: usize) -> Result<(), EditError>`:
  - removes `layer.rule[index]` only if its `name` equals `name` (a string, or a one-element
    list);
  - otherwise returns `EditError::Changed` ("Your rules changed elsewhere; refresh and try
    again.");
  - drops the `rule` key when the array becomes empty, as `ConfigRules.qml` does.
- `pub enum EditError` (`thiserror`, plain one-sentence messages): `BadScheduler`, `BadName`,
  `Duplicate`, `Changed`, `NoSuchRule`.

**Interfaces (produced, CLI):**
- `scdrctl settings scheduler <choice>`;
- `scdrctl settings rule-add <profile> <name>`;
- `scdrctl settings rule-remove <name> <index>`.

Each one:
1. connects to the system bus (`daemon()`);
2. calls `Settings`, takes `.layer`, and applies the edit;
3. calls `SetSettings(layer)`;
4. prints scdrd's summary.

On `AccessDenied` it uses the existing `not_running` mapping (the polkit hint). Exit 1 on any
error, with the message on stderr. The profile is checked by scdrd's own `SetSettings`
validation: an unknown profile comes back as its error.

- [ ] **Step 1: Write the failing tests** in `src/config/edit/tests.rs`:
  - `scheduler_eevdf_disables` and `scheduler_named_enables_and_keeps_power_keys`: a layer with
    `sched_ext.power.performance` keeps it.
  - `scheduler_rejects_bad_choice`: `"bpf land"`, `"x:turbo"` and `""` are refused.
  - `add_rule_appends_and_creates`: an empty layer gets `rule = [{…}]`, index 0; a second add
    gets index 1.
  - `add_rule_refuses`: `""`, `"a/b"`, 65 characters, and a duplicate.
  - `remove_rule_checks_name`: the right name removes the rule; a wrong name gives `Changed` and
    leaves the layer untouched. Removing the last rule drops the key. A rule whose `name` is a
    one-element list matches; a two-element list never does.
  - `messages_are_plain`: no `serde`, `JSON`, `D-Bus`, `{` in any `EditError` message.
- [ ] **Step 2:** Run `cargo test --locked -j 2 config::edit -- --test-threads=2`. Expected: FAIL
  (module missing).
- [ ] **Step 3:** Implement `edit.rs` (under 150 lines) and `settings.rs` (under 150 lines). Use
  the existing `SchedChoice` parser; add no new parsing of scheduler names.
- [ ] **Step 4:** Run:
  - `cargo test --locked -j 2 -- --test-threads=2` (including `tests/file_length.rs`);
  - `cargo clippy --locked -j 2 --all-targets -- -D warnings`;
  - `cargo run --bin scdrctl -- settings --help`.

  Expected: PASS, and the help lists the three subcommands.
- [ ] **Step 5:** Commit (K5 trailers): `feat(scdrctl): settings scheduler, rule-add and rule-remove for one-change edits`

### Task E1.2: The page manifest and `logic.js`, test first

**Files:**
- Create:
  - `uq/pages/scdr/page.toml`;
  - `uq/pages/scdr/logic.js`;
  - `tests/uq/load-logic.mjs`;
  - `tests/uq/logic.test.mjs`.

**`page.toml`** (exact):
```toml
# uQ Settings page for scdr. Installed to /usr/share/uq/settings/pages/scdr/ by scdr's
# package; inert without uQ. Reference: docs/settings-pages.md in the uQ repository.
uq_api = 1
id = "scdr"
name = "Scheduling"
icon = "preferences-system-performance"
category = "apps"
qml = "Page.qml"
keywords = ["scdr", "cpu", "priority", "scheduler", "focus", "games", "audio", "performance"]

[[setting]]
id = "status"
label = "Scheduling on or paused"
keywords = ["pause", "resume", "running"]

[[setting]]
id = "roles"
label = "Boost the app you use, audio and games"
keywords = ["focus", "foreground", "game", "audio", "background", "boost"]

[[setting]]
id = "scheduler"
label = "CPU scheduler"
keywords = ["scheduler", "sched_ext", "scx", "bpfland", "lavd", "eevdf"]

[[setting]]
id = "rules"
label = "Your app rules"
keywords = ["rule", "priority", "app", "nice"]

[[setting]]
id = "follow-focus"
label = "Follow the focused window"
keywords = ["agent", "session", "focus", "window"]

[requires]
bins = ["scdrctl"]

[dbus]
name = "org.scdr.Daemon1"
path = "/org/scdr/Daemon1"
interface = "org.scdr.Daemon1"
methods = ["Status", "Processes", "Settings"]
properties = ["State"]
bus = "system"

[[command]]
id = "pause"
argv = ["/usr/bin/scdrctl", "pause"]
output = "text"
timeout_ms = 120000
confirm = "Pause scdr? Every app's priority goes back to normal until you resume."

[[command]]
id = "resume"
argv = ["/usr/bin/scdrctl", "resume"]
output = "text"
timeout_ms = 120000

[[command]]
id = "role"
argv = ["/usr/bin/scdrctl", "role"]
output = "text"
timeout_ms = 120000
args = [
  { name = "role", pattern = "^(game|audio|foreground|background)$" },
  { name = "state", pattern = "^(on|off)$" },
]

[[command]]
id = "scheduler"
argv = ["/usr/bin/scdrctl", "settings", "scheduler"]
output = "text"
timeout_ms = 120000
args = [{ name = "choice", pattern = "^(eevdf|[a-z0-9_]{1,32}(:(auto|gaming|powersave|lowlatency|server))?)$" }]

[[command]]
id = "rule-add"
argv = ["/usr/bin/scdrctl", "settings", "rule-add"]
output = "text"
timeout_ms = 120000
args = [
  { name = "profile", pattern = "^[a-z0-9][a-z0-9_-]{0,39}$" },
  { name = "name", pattern = "^[A-Za-z0-9][A-Za-z0-9._+*?@ -]{0,63}$" },
]

[[command]]
id = "rule-remove"
argv = ["/usr/bin/scdrctl", "settings", "rule-remove"]
output = "text"
timeout_ms = 120000
args = [
  { name = "name", pattern = "^[A-Za-z0-9][A-Za-z0-9._+*?@ -]{0,63}$" },
  { name = "index", pattern = "^[0-9]{1,4}$" },
]
confirm = "Remove your rule for %s?"

[[command]]
id = "agent-state"
argv = ["/usr/bin/systemctl", "--user", "is-enabled", "scdr-session.service"]
output = "text"
timeout_ms = 5000

[[command]]
id = "agent-on"
argv = ["/usr/bin/systemctl", "--user", "enable", "--now", "scdr-session.service"]
output = "none"
timeout_ms = 20000

[[command]]
id = "agent-off"
argv = ["/usr/bin/systemctl", "--user", "disable", "--now", "scdr-session.service"]
output = "none"
timeout_ms = 20000
confirm = "Stop following the focused window? The app you're using no longer gets a boost."

[[command]]
id = "start"
argv = ["/usr/bin/systemctl", "enable", "--now", "scdrd.service"]
output = "none"
timeout_ms = 120000
```

**Interfaces (produced, `logic.js`, all pure):**
- `parseState(text) → {paused, managed, powerProfile, cpuPressure, pressureActive, scheduler: [..], roles: [{role, profile, enabled}], agents, boosted: [{pid, name, profile, why}]} | {error}`.
  It parses the `State` property, which is `WidgetState` from `service/control.rs`; its `roles`
  are `[role, profile, enabled]` triples.
- `parseSettings(text) → {layerRules: [{index, name, profile, simple}], profiles: [names], schedulers: [{value, label}], currentScheduler, choice, error?}`:
  - `simple` is true when the rule has exactly `profile` plus a one-element `name`; only simple
    rules get a Remove button;
  - `profiles` is the union of base and layer `profile` keys, sorted;
  - `schedulers` comes from `.schedulers` (the `inventory` shape; read `src/schedulers.rs` to
    confirm the field names), plus `{value: "eevdf", label: "Kernel default"}` first.
- `roleLabel(role)`:
  - `foreground` → "The app you're using";
  - `audio` → "Apps playing sound";
  - `game` → "Games";
  - `background` → "Apps in the background when the CPU is busy".
- `agentEnabled(result)`: `result.output` trimmed is `enabled` (code 0); `disabled` or code 1 is
  false; anything else is `null` (unknown).
- `notRunning(error)`: true for `ServiceUnknown`, `NameHasNoOwner` or "not activatable" errors.
- `checkRuleName(s) → "" | message` and `checkProfile(s)`. They use the same regex strings as
  `page.toml`, exported as `RULE_NAME`, `PROFILE`, `CHOICE`.
- `writeError(result) → string`. For `AccessDenied` or polkit text it gives "You need to enter
  your password to change this." Otherwise it gives the first line, cut to 200 characters.

- [ ] **Step 1: Write the failing tests** in `tests/uq/logic.test.mjs`:
  - `parses_state_fixture`: a `State` JSON captured with `busctl --system get-property
    org.scdr.Daemon1 /org/scdr/Daemon1 org.scdr.Daemon1 State` shape, written by hand from
    `WidgetState`.
  - `bad_state_is_an_error`: `"{"`, `"null"`, `"[]"` and a `roles` entry of the wrong arity.
  - `settings_fixture`: layer and base rules, simple vs. not, profiles union, and `eevdf` first.
  - `agent_state_cases`: `enabled`, `disabled`, `masked`, `static`, and an error.
  - `patterns_match_manifest`: a minimal reader pulls each `pattern = "…"` from `page.toml`, and
    each equals the `logic.js` constant.
  - `rejects_hostile_args` (Review Focus 2).
  - `confirms_match_plan` (R8 rows for scdr).
  - `dbus_is_read_only` (Review Focus 3).
  - `page_strings_are_plain` (R3). This runs once `Page.qml` exists (Task E1.3). Until then it is
    `test.skip` with the reason "Page.qml in E1.3", and E1.3 removes the skip.
- [ ] **Step 2:** Run `node --test tests/uq/`. Expected: FAIL (`logic.js` missing).
- [ ] **Step 3:** Write `logic.js` (`.pragma library`, no `Qt.*`) and `page.toml`.
- [ ] **Step 4:** Run `node --test tests/uq/` and the local `check` (Global Constraints). Expected:
  Node passes. `check` prints only the summary and the missing-program lines that are filtered
  out (`scdrctl` and `systemctl` exist here only if installed).
- [ ] **Step 5:** Commit (K5 trailers): `feat(uq): Scheduling page manifest and its tested logic`

### Task E1.3: `Page.qml` and its offscreen test

**Files:**
- Create:
  - `uq/pages/scdr/Page.qml`;
  - `tests/uq/tst_page.qml`;
  - `tests/uq/stub/org/uq/settings/qmldir`, `UqLabel.qml` and `UqComboBox.qml`;
  - `tests/uq/fixtures/` (`state.json`, `settings.json`).

**Layout** (a `QQC2.ScrollView` holding a `ColumnLayout`; sections are `UqLabel` headings):
1. **Status.**
   - "scdr is running" / "scdr is paused" / "scdr isn't running".
   - CPU pressure ("CPU busy: 34 %") and "N apps managed".
   - **Pause** / **Resume** button.
   - When not running, a **Start scdr** button (`start`), and the Advanced details hold the raw
     error.
2. **Boosts** (`setting` id `roles`). One `QQC2.Switch` row per role in `parseState().roles`.
   The label is a `UqLabel` with `roleLabel(role)`, plus "uses the <profile> profile" muted
   below. Below the switches, "Boosted now": up to 20 `UqLabel` rows `name — profile`, from
   `boosted`.
3. **Follow the focused window** (`follow-focus`). A switch backed by `agent-state`; it turns on
   with `agent-on` and off with `agent-off` (confirm). When the agent count from `State` is 0
   while the switch is on, a muted note says "Not following any window yet. It starts with your
   next login, or now if your session is running."
4. **CPU scheduler** (`scheduler`). A `UqComboBox` over `parseSettings().schedulers`; a choice
   runs `scheduler`. When `scx_loader_error` is set, the note reads "No extra schedulers are
   installed." and the raw error goes under Advanced details. The combo shows the layer's
   `sched_ext.default`, falling back to "Kernel default".
5. **Your app rules** (`rules`).
   - The list of `layerRules`: `UqLabel` "<name> → <profile>", with **Remove** on simple rules.
   - Below it an "App name" `QQC2.TextField`, a profile `UqComboBox` and an **Add** button, with
     inline validation from `checkRuleName`.
   - A muted line says "Rules from scdr's files come after yours. See them with scdrctl list."
     (`scdrctl` as a command name is allowed in that hint; it's not in the R3 word list).

**Behaviour:**
- On `ctx` set:
  - `dbus.get("State")`, `dbus.call("Settings", [])` and `command.run("agent-state", [])`;
  - the R4 timer re-reads `State` only.
- After any write: re-read `State` and `Settings`. On failure show `writeError` beside the
  control and restore it.
- `highlight` scrolls to the section whose id matches, and flashes it by animating opacity once.
- Theme colours come from `ctx.theme.colors`, with fallbacks (`docs/settings-pages.md`).

- [ ] **Step 1: Write the failing test** `tests/uq/tst_page.qml`:
  - `test_initial_reads`: three calls, in any order.
  - `test_shows_state`: a fixture with `paused: false` and two boosted apps shows "scdr is
    running" and both names.
  - `test_role_switch_runs_command`: flipping "Games" off runs `role ["game","off"]`.
  - `test_failed_write_restores`: the fake answers `{code: 1, error: "AccessDenied: …"}`. The
    switch is back on, and "You need to enter your password to change this." shows.
  - `test_cancel_is_silent`: `{error: "Cancelled", cancelled: true}` shows nothing new.
  - `test_not_running`: `Settings` answers `{error: "org.freedesktop.DBus.Error.ServiceUnknown…"}`.
    The page shows "scdr isn't running" and the Start button.
  - `test_rule_add_validates`: name `-rf` shows the inline reason and makes no call; `firefox`
    with profile `desktop` runs `rule-add ["desktop","firefox"]`.
  - `test_rule_remove_args`: Remove on rule 2 named `steam*` runs `rule-remove ["steam*","2"]`.
  - `test_untrusted_text_plain`: a boosted name `<b>x</b>` shows exactly that text.
  - `test_highlight_scrolls`: `highlight = "rules"` makes the rules heading visible in the
    viewport.
- [ ] **Step 2:** Locally (K4): `node --test tests/uq/` (with `page_strings_are_plain` now
  enabled). Expected: FAIL until `Page.qml` exists, then PASS. The QML test runs in CI (Task
  E1.5).
- [ ] **Step 3:** Write `Page.qml` (target ≤ 400 lines; E-F21) and the stub module.
- [ ] **Step 4:** Run `node --test tests/uq/` and the local `check`. Expected: PASS and a clean
  `check` (after filtering).
- [ ] **Step 5:** Commit (K5 trailers): `feat(uq): the Scheduling page for uQ Settings`

### Task E1.4: Packaging

**Files:**
- Modify:
  - `packaging/install.sh`: one line,
    `install -Dm644 -t "$dest/usr/share/uq/settings/pages/scdr" uq/pages/scdr/page.toml uq/pages/scdr/Page.qml uq/pages/scdr/logic.js`
    (R9);
  - `packaging/arch/PKGBUILD` and `packaging/aur/scdr/PKGBUILD`: add the line
    `'uq: the Scheduling page in uQ Settings'` to `package_scdr`'s `optdepends` (the AUR file:
    its own `optdepends`). `scdr-git` likewise.
- Create:
  - `uq/README.md`: what the folder adds, where it installs, how it was tested, and that it is
    inert without uQ;
  - `tests/uq/install_test.sh`. It runs `packaging/install.sh "$tmp" <fake target>` with empty
    fake binaries, then asserts the three page files exist with mode 644 and that nothing else
    is under `usr/share/uq/`.

- [ ] **Step 1:** Write `tests/uq/install_test.sh` (POSIX sh, `set -eu`; prints `PASS`/`FAIL`).
  Run it. Expected: FAIL (no page files installed).
- [ ] **Step 2:** Edit `install.sh` and the PKGBUILDs; write `uq/README.md`.
- [ ] **Step 3:** Run `sh tests/uq/install_test.sh` and `sh -n packaging/install.sh`. Expected:
  PASS.
- [ ] **Step 4:** Commit (K5 trailers): `packaging: install the uQ Settings page; uq as an optional dependency`

### Task E1.5: CI job and docs

**Files:**
- Modify:
  - `.github/workflows/ci.yml`: a new job `uq` (R10), with `UQ_REV` per E-F4. The programs
    installed for E-F3 are:
    - built `scdrctl` and `scdrd` from `cargo build --locked --release`;
    - a stub for `/usr/bin/systemctl` if the runner lacks it (ubuntu has it, so the step only
      checks that it exists).

    The job also runs `sh tests/uq/install_test.sh`.
  - `README.md`: a "## uQ desktop" section. It says what the page does, that it appears in
    Settings → Apps when uQ is installed, that writes ask for the `org.scdr.control` password as
    in the widget, and how to try it (`uq-settings --plugin-dir uq/pages`).
  - `AGENTS.md`, under `## uQ integration`: one bullet on `uq/pages/scdr/` (`node --test
    tests/uq/`, the QML test, `uqctl integrations check uq/`), and that `scdrctl settings` exists
    for it.

- [ ] **Step 1:** Write the job. Validate the YAML locally with
  `python3 -c 'import yaml,sys; yaml.safe_load(open(".github/workflows/ci.yml"))'`; if PyYAML is
  missing, use `node -e` with a minimal check that the file parses as YAML through
  `npx --yes yaml@2.6.1`. If neither is available offline, review it by eye and rely on CI.
- [ ] **Step 2:** Docs.
- [ ] **Step 3:** Run the whole local gate:
  - `cargo clippy --locked -j 2 --all-targets -- -D warnings`;
  - `cargo test --locked -j 2 -- --test-threads=2`;
  - `node --test tests/uq/`;
  - `sh tests/uq/install_test.sh`;
  - the local `check`.

  Expected: all PASS.
- [ ] **Step 4:** Commit (K5 trailers): `ci: check the uQ page; docs for the uQ desktop`
- [ ] **Step 5:** Open the PR (`uq-settings-page` → `main`) with the spec deviations that apply
  (1, 2, 5, 6, 7). Watch CI. The QML test and `check` first run there, so fix any failure in the
  task it belongs to.

---

## E2: crumBS (`/home/user/crumbs`, branch `uq-settings-page`)

No new CLI is needed: every read and write the page uses exists and is documented in
`docs/json-schema.md`.

### Task E2.1: The page manifest and `logic.js`, test first

**Files:**
- Create:
  - `uq/pages/crumbs/page.toml`;
  - `uq/pages/crumbs/logic.js`;
  - `tests/uq/load-logic.mjs` (same as scdr's);
  - `tests/uq/logic.test.mjs`;
  - `tests/uq/fixtures/`: `scan.json`, `scan-system.json`, `orphans.json`, `history.json`,
    `apps.json`, `clean-ok.json`, `clean-exit5.json`, `uninstall-refused-desktop.json` and
    `uninstall-running.json`. Copy them from the examples in `docs/json-schema.md`, trimmed.
    Where an example is missing, generate the fixture with the debug build against
    `crumbs_core::testing::FixtureTree`, as the schema doc does, and say so in a comment file.

**`page.toml`**:
- `id = "crumbs"`, `name = "Storage and cleanup"`, `icon = "user-trash-full"`;
- `keywords = ["crumbs", "disk", "space", "cache", "cleanup", "leftovers", "uninstall", "trash"]`;
- `[requires] bins = ["crumbs"]`, and no `[dbus]`.

`[[setting]]` rows, each with keywords:

| id | label |
|---|---|
| `space` | Space you can free |
| `system-space` | System space |
| `leftovers` | Leftovers of removed apps |
| `history` | Recent cleanups |
| `apps` | Apps and their size |

`[[command]]` tables (every `argv[0]` is `/usr/bin/crumbs`):

| id | argv after `crumbs` | output | timeout_ms | args | confirm |
|---|---|---|---|---|---|
| `scan` | `scan --json` | json | 180000 | — | — |
| `scan-system` | `scan --system --json` | json | 180000 | — | — |
| `clean` | `clean --yes --json` | json | 600000 | `category` = `^(cache\|thumbnails\|browser-cache\|dev-cache\|appimage-old\|logs\|app-cache\|build-output)$` | R8 |
| `clean-for-good` | `clean --yes --json` | json | 600000 | `category` = `^(trash\|temp\|aur-build\|containers\|flatpak-unused)$` | R8 |
| `clean-system` | `clean --system --yes --json` | json | 600000 | `category` = `^(pkg-cache\|orphan-pkgs\|journal\|coredumps\|system-temp\|flatpak-unused-system\|snapshots)$` | R8 |
| `orphans` | `orphans --json` | json | 120000 | — | — |
| `orphan-trash` | `orphans --clean --yes --json --select` | json | 300000 | `path` = `^/[^\x00-\x1f\x7f]{1,4095}$` | R8 |
| `history` | `history --limit 20 --json` | json | 30000 | — | — |
| `restore` | `restore --json` | json | 300000 | `batch` = `^[0-9]{1,20}$` | R8 |
| `apps` | `apps list --sort size --json` | json | 120000 | — | — |
| `uninstall` | `uninstall --yes --json` | json | 600000 | `app` = `^(pacman\|dpkg\|rpm\|snap\|flatpak\|appimage\|unmanaged):[^\x00-\x1f\x7f]{1,400}$` | R8 |

The `|` inside patterns is escaped only in this table; `page.toml` has plain `|`. Note that
`--select` is the last fixed argv entry, and the path follows as its own argument (E-F8).
`orphans --json` without `--show-low` lists medium-confidence findings only, which is what the
page offers.

**Interfaces (produced, `logic.js`):**
- `envelope(result) → {ok, code, data, errors: [{source, message}], cli: string|null}`. It reads
  `result.output` for every code (E-F13), and `cli` is the `errors[]` entry with
  `source == "cli"`.
- `categories(scanData) → [{category, label, total, items, unknown, skipped, unavailable, error, command}]`:
  - `label` is the plain name: `cache` → "App caches", `thumbnails` → "Thumbnails",
    `browser-cache` → "Browser caches", … (one per R7 name, all twenty);
  - `command` is `cleanCommandFor(category)`.
- `cleanCommandFor(category)` (R7), and `USER_TRASH`, `USER_FOR_GOOD`, `SYSTEM` (the three
  lists).
- `outcome(result) → {message, manual: [string]}`. It writes:
  - "Freed 1.2 GB" (sums `report[].size` where `ok`);
  - "Some items couldn't be removed" for exit 3;
  - "Couldn't get administrator rights" plus `data.manual_commands` for exit 5;
  - the `cli` message for exits 1 and 2;
  - "Cancelled at a prompt" for exit 4.
- `orphansList(data)`, `batches(data)` and `apps(data)`, each capped and plain (Untrusted input).
  - `batches` marks `canRestore` when the batch has any `ok` trash or `edit-file` entry whose id
    no later `restore` entry `restores`.
  - `apps` keeps `id`, `display_name || name`, `install_size`, `source`, and `part_of`. `part_of`
    is `"uQ"` for the desktop's own packages, and the page then shows "Part of uQ" with no
    Uninstall button. crumBS would refuse anyway (E-F12).
- `size(bytes)` gives "1.2 GB" (base 1000, one decimal, "—" for null).

- [ ] **Step 1: Write the failing tests**:
  - one per fixture: it parses, and the plain labels and messages come out as expected;
  - `category_lists_partition`: the three R7 lists are disjoint, and their union equals the
    twenty names listed in `docs/json-schema.md` §`scan` (read from the doc at test time with a
    regex over its "The thirteen user categories" and "The seven system categories" sentences,
    so a new crumBS category fails this test until the page knows it);
  - `exit5_shows_manual_commands`;
  - `desktop_package_has_no_uninstall`;
  - `no_dangerous_flags` (Review Focus 5): no argv contains `--force`, `--desktop`,
    `--permanent` or `-i`;
  - `patterns_match_manifest`, `rejects_hostile_args`, `confirms_match_plan` and
    `page_strings_are_plain` (skipped until E2.2).
- [ ] **Step 2:** Run `node --test tests/uq/`. Expected: FAIL.
- [ ] **Step 3:** Write `logic.js` and `page.toml`.
- [ ] **Step 4:** Run `node --test tests/uq/` and the local `check`. Expected: PASS; `check`
  clean after filtering.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uq): Storage and cleanup page manifest and its tested logic`

### Task E2.2: `Page.qml` and its offscreen test

**Files:**
- Create:
  - `uq/pages/crumbs/Page.qml`;
  - `tests/uq/tst_page.qml`;
  - `tests/uq/stub/org/uq/settings/{qmldir,UqLabel.qml,UqComboBox.qml}`.

**Layout:**
1. **Space you can free** (`space`).
   - `scan` runs on open and on **Refresh**, with "Looking for space you can free…" while it
     runs (it can take a minute).
   - One row per category that isn't skipped or unavailable:
     - plain label, size, and "N items"; a muted note "goes to the trash" or "deleted for good";
     - **Clean up** (disabled at 0 items) runs the category's command;
     - categories with only non-preselected items (`trash`, `flatpak-unused`) still clean when
       named, as crumBS documents.
   - Unavailable categories are listed in a muted line ("Not on this computer: …").
2. **System space** (`system-space`).
   - A **Check system space** button runs `scan-system` (only on demand).
   - The rows have **Clean up…** (`clean-system`), and the opt-in categories say "Only when you
     ask".
3. **Leftovers of removed apps** (`leftovers`).
   - `orphans` runs on demand (**Find leftovers**).
   - Each row shows the path, size and reason, with **Move to trash** (`orphan-trash`).
4. **Recent cleanups** (`history`). Up to 20 batches: label, time, size, and **Put back** where
   `canRestore`.
5. **Apps** (`apps`).
   - `apps` runs on demand (**List apps**), sorted by size.
   - A filter `QQC2.TextField` matches the name, case-insensitively, in `logic.js`.
   - **Uninstall…** (`uninstall`), except for "Part of uQ".
   - A muted line: "AppImages kept by appOFholding are managed there."

Each write shows `outcome()` beside its row and then re-runs only the read for that section.
Only one write runs at a time (`busy`); other write buttons are disabled meanwhile.

- [ ] **Step 1: Write the failing test** `tests/uq/tst_page.qml`:
  - `test_scan_on_open_only`: only `scan` runs on open.
  - `test_clean_routes_by_category`: Clean up on `browser-cache` runs `clean`, on `trash` runs
    `clean-for-good`, and on `journal` runs `clean-system`.
  - `test_exit5`: the fake answers `{code: 5, error: "…", output: <clean-exit5>}`. The page shows
    "Couldn't get administrator rights", and Advanced details holds the manual commands.
  - `test_uninstall_refused_running`: exit 1 with the running-app `cli` message shows that message
    verbatim.
  - `test_part_of_uq_has_no_button`.
  - `test_cancel_is_silent`.
  - `test_untrusted_path_plain`: an orphan path holding `<i>` and `‮` shows verbatim.
  - `test_one_write_at_a_time`: while a clean runs, the other Clean up buttons are disabled.
- [ ] **Step 2:** Locally: `node --test tests/uq/` with `page_strings_are_plain` enabled.
  Expected: FAIL, then PASS once `Page.qml` exists.
- [ ] **Step 3:** Write `Page.qml`, ≤ 400 lines (E-F21). If it doesn't fit, ship sections 1, 2, 5
  and 4 and leave Leftovers to the CLI, recording that in the PR (Notes question 4).
- [ ] **Step 4:** Run `node --test tests/uq/` and the local `check`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uq): the Storage and cleanup page for uQ Settings`

### Task E2.3: Packaging, CI and docs

**Files:**
- Modify:
  - `packaging/aur/crum-bs/PKGBUILD`:
    - `package()` gains the R9 `install` line (from `$_pkgname-$pkgver/uq/pages/crumbs/`);
    - `optdepends=('uq: the Storage and cleanup page in uQ Settings')`.
  - `packaging/aur/crum-bs-bin/PKGBUILD`:
    - add the release source tarball as a second source,
      `"$_pkgname-$pkgver.tar.gz::$url/releases/download/v$pkgver/$_pkgname-$pkgver.tar.gz"`,
      with its `sha256sums` entry `SKIP`, filled by `scripts/release.sh`;
    - install the three page files from it;
    - the same `optdepends`.
  - `scripts/release.sh`: fill the new checksum for `crum-bs-bin`. Additive, following how it
    fills `crum-bs`'s `tarball_sum`.
  - `scripts/tests/aur.sh`: assert that both packages contain
    `usr/share/uq/settings/pages/crumbs/{page.toml,Page.qml,logic.js}`, and nothing else under
    `usr/share/uq`.
  - `.github/workflows/ci.yml`: job `uq` (R10). Programs: `cargo build --workspace --release
    --locked`, then `sudo install target/release/crumbs /usr/bin/crumbs`.
  - `README.md`: a "## uQ desktop" section (what the page does, that it never forces or removes
    uQ's own packages, how to try it).
  - `CHANGELOG.md` `## Unreleased`: "**uQ Settings page:** Storage and cleanup, …".
  - `AGENTS.md` `## uQ integration`: one bullet.
- Create: `uq/README.md`.
- Unchanged: `packaging/deb`, `packaging/rpm` and the AppImage. uQ is Arch-only, and the files
  would be inert anyway (Notes question 6).

- [ ] **Step 1:** Extend `scripts/tests/aur.sh` first. Run `scripts/test-packaging.sh aur` if
  its prerequisites (a built tarball, `makepkg`) exist here. Expected: FAIL. If `makepkg` isn't
  available in this container, say so, and leave it to CI and the user's release check.
- [ ] **Step 2:** Edit the PKGBUILDs, `release.sh`, CI and the docs.
- [ ] **Step 3:** Run the local gate:
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features -- -D warnings`;
  - `cargo test -j 2 --workspace --all-features --locked -- --test-threads=2`;
  - `node --test tests/uq/`;
  - `scripts/check-licenses.sh`;
  - the local `check`;
  - `scripts/test-packaging.sh aur` where possible.

  Expected: PASS.
- [ ] **Step 4:** Commit (K5 trailers): `packaging, ci, docs: ship and check the uQ Settings page`
- [ ] **Step 5:** Open the PR with deviations 1, 5 and 7. Watch CI.

---

## E3: noti-fi (`/home/user/noti-fi`, branch `uq-settings-page`) — **needs repo access**

### Task E3.1: Discovery (no commit)

- [ ] **Step 1:** Clone `asafelobotomy/noti-fi` to `/home/user/noti-fi`. Read:
  - `AGENTS.md`;
  - `crates/notify-cli/src/cli.rs`, `main.rs` and `controls.rs`;
  - `docs/protocol.md`;
  - `notifd/Config.qml` (the defaults);
  - `crates/notify-panel-core/src/draft.rs` (the settable keys);
  - `packaging/PKGBUILD`, `packaging/install.sh` and `packaging/build-tarball.sh`;
  - the CI workflow.
- [ ] **Step 2:** Confirm or correct each assumption below. Write the confirmed table into the PR
  description, and adjust Task E3.2 and E3.3 before writing code:
  - `/usr/bin/notify status --json` prints one `Status` document (`dnd`, `muted[]`, `zones[]`,
    `focus`, `auto{fullscreen,share}`, `history`, `cfgError`);
  - `notify dnd on|off --json` exists (seen in `controls.rs`) and streams JSON lines (E-F14);
  - how to mute or unmute a zone (`notify mute <zone>`: a toggle or `on|off`?);
  - how to choose a focus mode (`focus` in `Api.qml`; a CLI subcommand?);
  - how to set `autoDnd` fullscreen and share (`setAuto` in the client; a CLI subcommand?);
  - `notify history --clear` exists (`HistoryArgs.clear`);
  - `notify rules --json` prints the rules report;
  - whether a DND **schedule** exists anywhere (the fragments show none; spec deviation 3);
  - how `history.limit`, `history.persist` and `dnd.allowCritical` can be set without the panel.
    The panel uses `settingSet` on a draft, then `Save` through the socket's config path.
- [ ] **Step 3:** Decide Task E3.2's scope:
  - if every setter exists in the CLI, E3.2 is empty: skip it and say so in the PR;
  - otherwise E3.2 adds only the missing ones.

### Task E3.2: `notify set` and any missing setters (only if E3.1 finds them missing)

**Files** *(confirm in discovery)*:
- `crates/notify-cli/src/cli.rs`: a `Set { key, value }` subcommand;
- `crates/notify-cli/src/controls.rs`: `set()`;
- tests where noti-fi keeps CLI tests (its smoke test uses a private bus and home, per its
  CHANGELOG);
- `README.md`.

**Interface:** `notify set <key> <value> [--json]`, with `key` from a fixed allowlist:

| key | values |
|---|---|
| `history.limit` | `50`, `100`, `200`, `500`, `1000` (the panel's choices) |
| `history.persist` | `on`, `off` |
| `dnd.allowCritical` | `on`, `off` |
| `autoDnd.fullscreen` | `on`, `off` (if not a `setAuto` CLI already) |
| `autoDnd.share` | `on`, `off` (if not a `setAuto` CLI already) |

It reads the config with `configGet`, sets the one key in the user file's JSON, validates through
the daemon (`validate`), and writes through the same save path the panel uses, so a bad value is
refused by the daemon's own checks. Without a running daemon it exits non-zero with "noti-fi
isn't running".

- [ ] **Step 1: Write the failing tests:**
  - each key round-trips through `notify set` and then `status` or `configGet`;
  - an unknown key and a bad value are refused, with exit 2 and nothing written;
  - a config with an error elsewhere is left untouched.
- [ ] **Step 2:** Run noti-fi's test command (from its `AGENTS.md`) with `-j 2`. Expected: FAIL.
- [ ] **Step 3:** Implement. No new crates.
- [ ] **Step 4:** Run its full gate. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(cli): notify set for the settings a desktop page changes`

### Task E3.3: The page: manifest, `logic.js`, `Page.qml` and tests

**Files:**
- Create:
  - `uq/pages/noti-fi/page.toml`, `logic.js` and `Page.qml`;
  - `uq/README.md`;
  - `tests/uq/{load-logic.mjs,logic.test.mjs,tst_page.qml,stub/…,fixtures/status.json}`.

**`page.toml`** *(argv confirmed in E3.1)*:
- `id = "noti-fi"`, `name = "Notifications"`, `icon = "preferences-desktop-notification"`;
- `keywords = ["notifications", "noti-fi", "do not disturb", "dnd", "focus", "mute", "history"]`;
- `[requires] bins = ["notify"]`.

`[[setting]]` rows: `dnd`, `focus`, `groups`, `auto-dnd`, `history` and `rules`, each with
keywords.

`[[command]]` tables:

| id | argv | output | args |
|---|---|---|---|
| `status` | `/usr/bin/notify status --json` | json | — |
| `dnd` | `/usr/bin/notify dnd` | text | `mode` = `^(on\|off)$` |
| `focus` | *(E3.1)* | text | `mode` = `^(off\|[a-z0-9_-]{1,40})$` |
| `mute` | *(E3.1)* | text | `zone` = `^[A-Za-z0-9_-]{1,40}$`, `state` = `^(on\|off)$` |
| `set` | `/usr/bin/notify set` | text | `key` = `^(history\.limit\|history\.persist\|dnd\.allowCritical\|autoDnd\.fullscreen\|autoDnd\.share)$`, `value` = `^(on\|off\|50\|100\|200\|500\|1000)$` |
| `history-clear` | `/usr/bin/notify history --clear` | text | — (confirm R8) |
| `rules` | `/usr/bin/notify rules --json` | json | — |

**Page:**
- A Do not disturb switch.
- A focus mode `UqComboBox`: "Off" plus the modes' labels from the config (or `status`). If
  neither carries labels, show the mode ids; they are the user's own names.
- Groups: a mute switch per zone.
- Automatic Do not disturb: two switches, "When an app is full screen" and "When you share your
  screen".
- History:
  - a `UqComboBox` "Remember at most" (50…1000);
  - "Keep after restart";
  - "Let urgent notifications through Do not disturb";
  - **Clear history…**.
- Your rules: a read-only list of rule names, with the muted line "Change rules in noti-fi's own
  app, in the app menu." (R-F).
- When `status` fails with "not running": "noti-fi isn't running. uQ's own notifications are in
  use." There is no start button: uQ's `notif.backend` decides who runs (uQ README).

Notification text is never shown on this page: no summaries, no bodies.

**Tests** (as in E1.2 and E1.3):
- `parses_status`;
- `focus_modes_listed`;
- `set_values_partition`: the value pattern only allows the right values per key, and `logic.js`
  sends no other pair;
- `patterns_match_manifest`, `rejects_hostile_args`, `confirms_match_plan`,
  `page_strings_are_plain`;
- in `tst_page.qml`:
  - `test_dnd_switch`;
  - `test_failed_write_restores`;
  - `test_no_notification_text` (a `status` with `latest.summary = "secret"` never shows
    "secret");
  - `test_not_running`.

- [ ] **Step 1:** Write the failing Node and QML tests. Run `node --test tests/uq/`. Expected:
  FAIL.
- [ ] **Step 2:** Write `page.toml`, `logic.js` and `Page.qml`.
- [ ] **Step 3:** Run `node --test tests/uq/` and the local `check`. Expected: PASS, clean.
- [ ] **Step 4:** Commit (K5 trailers): `feat(uq): the Notifications page for uQ Settings`

### Task E3.4: Packaging, CI and docs

**Files** *(confirm in discovery)*:
- `packaging/PKGBUILD`: the R9 `install` line and `optdepends` `'uq: the Notifications page in
  uQ Settings'`;
- `packaging/build-tarball.sh` and `packaging/install.sh`: carry
  `share/uq/settings/pages/noti-fi/`, so the tarball install (`PREFIX=/usr`) installs it too.
  A `~/.local` install must **not** install it: uQ never reads pages from the home folder. The
  script says so in a comment;
- CI: job `uq` (R10), installing the built `notify` to `/usr/bin`;
- `README.md` (its "For desktop shells (uQ and others)" paragraph gains the page), `AGENTS.md`
  and `CHANGELOG.md`.

- [ ] **Step 1:** Add packaging assertions to whatever packaging test noti-fi has. If it has
  none, use a `tests/uq/install_test.sh` like scdr's, run against `install.sh` with
  `PREFIX=/usr DESTDIR=$tmp`. Expected: FAIL.
- [ ] **Step 2:** Edit the packaging, CI and docs.
- [ ] **Step 3:** Run noti-fi's full gate, `node --test tests/uq/`, the install test and `check`.
  Expected: PASS.
- [ ] **Step 4:** Commit (K5 trailers): `packaging, ci, docs: ship and check the uQ Settings page`
- [ ] **Step 5:** Open the PR with deviations 2 (if E3.2 ran), 3, 4, 5 and 7.

---

## E6: uQ side (this repo, branch `settings-slice-e`; after E1–E3 merge)

### Task E6.1: Docs: the pages list, README, spec note and `optdepends`

**Files:**
- Create: `docs/companion-pages.md`. The list of companion pages, one row each:
  - page name, id and repo;
  - what it does (the Goal table, shortened);
  - which package installs it;
  - what it needs running (scdrd, xzramd, noti-fi, tailscaled);
  - where its writes ask for a password.

  A "Status" column says merged / planned, and E4 and E5 tick theirs. It links
  `docs/settings-pages.md` for authors and this plan for the design.
- Modify (additive):
  - `README.md`: under `### Settings`, a short `#### Pages from other apps` paragraph. It names
    the five pages, says they appear under **Apps** when the app is installed (no restart), and
    links `docs/companion-pages.md`.
  - `packaging/uq/PKGBUILD` `optdepends`: new lines
    `'crum-bs: the Storage and cleanup page in Settings'`,
    `'xzram: the Memory and swap page in Settings'` and
    `'scale_tail: the Tailscale page and quick-settings tile'`. The existing `scdr`, `tailscale`
    and `noti-fi` lines stay as they are (additive only). The AUR name of scale_tail's package is
    confirmed in E5.1; until then that line is left out (Notes question 7).
  - `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` §8: one paragraph
    "Implemented by slice E (plan `2026-11-11-uq-settings-slice-e-companion-pages.md`)" listing
    deviations 1–7.

- [ ] **Step 1:** Write the doc and the edits.
- [ ] **Step 2:** Run `scripts/lint-settings.sh`. It doesn't lint docs, but it must stay green.
  Run `sh -n packaging/uq/PKGBUILD` (syntax only).
- [ ] **Step 3:** Commit (K5 trailers): `docs: companion app pages in uQ Settings`

### Task E6.2: CHANGELOG and the live-check note

**Files:**
- Modify: `CHANGELOG.md`, `## [Unreleased]` → `### Added` (additive).
- Create: `docs/notes/settings-slice-e-live-check.md`.

- [ ] **Step 1:** Add the CHANGELOG entry: **Pages from scdr, crumBS and noti-fi in Settings.**
  One sentence per page. Say that the pages ship with those apps (update the app to get its
  page), and that XZram's and Tailscale's follow. E4 and E5 each change "follow" to name their
  page.
- [ ] **Step 2:** Write `docs/notes/settings-slice-e-live-check.md` in the style of slice D's: a
  real uQ session, with the apps installed from their PR branches (`makepkg -si` in each repo).
  Tick each item:
  1. **Discovery.** Install scdr's package while Settings is open. **Scheduling** appears under
     Apps within a few seconds; remove the package and it goes. Same for crumBS and noti-fi.
     `uqctl integrations list` shows each page as visible.
  2. **Search.** Settings search for "boost", "cache", "do not disturb" and "swap" (after E4)
     each opens the right page with the row highlighted.
  3. **scdr.**
     - Pause asks first. After Confirm, a password prompt appears from uQ's agent; after the
       password, the page says paused, and `scdrctl status` agrees. Resume.
     - Turn Games off: the prompt appears and the switch stays off. Cancel the password prompt
       instead: the switch returns on with "You need to enter your password…".
     - Add a rule for `xeyes` with profile `background`, start `xeyes`, and
       `scdrctl explain xeyes` names the rule. Remove it (confirm).
     - Pick a scheduler, if `scx-scheds` is installed.
     - Follow the focused window off, then log out and in: it stays off. Turn it on.
     - `sudo systemctl stop scdrd`: within 5 s the page says scdr isn't running; **Start scdr**
       starts it after the password.
  4. **crumBS.**
     - Open the page: one scan runs (`pgrep -a crumbs` shows one).
     - Clean up **Thumbnails**: the confirm says "go to the trash". After it,
       `crumbs history --limit 1` shows the batch, and **Put back** restores it.
     - Clean up **Trash** says "for good".
     - **Journal** prompts for the password; cancel it, and the page says "Couldn't get
       administrator rights", with the commands under Advanced details.
     - Uninstall a running app: refused, with crumBS's message.
     - Uninstall `uq` is not offered ("Part of uQ").
     - An appOFholding AppImage never appears.
  5. **noti-fi.**
     - Do not disturb on: the bar island shows the mark within 5 s. Off again.
     - Mute a group: `notify-send` into that group shows nothing.
     - Set history to 50: noti-fi's own panel shows 50.
     - **Clear history…** asks first.
     - Stop notifd: the page says noti-fi isn't running.
  6. **Plain text.** Add an scdr rule named `<b>bold</b>`. It's refused by the pattern
     (inline reason), and nothing is sent.
  7. **Dev session.** In `uq-dev session`, the pages act on the real services (E-F20); note
     anything surprising.
  8. **Criterion 8.** `stat ~/.config/hypr/uq/user.lua` is unchanged by every step.
  9. **After E4/E5:** the same for Memory and swap (zram size change asks first and prompts for
     the password; swappiness applies) and Tailscale (connect, disconnect, exit node, accept
     routes, sign out asks first).
- [ ] **Step 3:** Commit (K5 trailers): `docs: settings slice E — changelog and live check`

### Task E6.3: The cross-repo check workflow

**Files:**
- Create:
  - `.github/workflows/companions.yml` (R11);
  - `scripts/check-companion-pages.sh`. It takes a list of `uq/` folders and a `uqctl` path.
    For each folder it stubs the `/usr/bin/*` and `/usr/lib/*` `argv[0]`s its `page.toml` and
    `widget.toml` files name, in a throwaway root only (CI), then runs `check`. It refuses to
    stub anything when `CI` isn't set. Locally it runs `check` and filters "does not exist", as
    in Global Constraints.

- [ ] **Step 1:** Run `scripts/lint-shell.sh` on the script first. It should fail until the
  script is written, then pass. Then run the script locally against `/home/user/scdr/uq` and
  `/home/user/crumbs/uq` (from their branches) with `target/debug/uqctl`. Expected: `ok` lines
  after filtering.
- [ ] **Step 2:** Write the workflow:
  - `on: schedule` (weekly), `workflow_dispatch`, and `pull_request` with `paths` covering the
    foundation paths listed under Ownership;
  - `permissions: contents: read`;
  - actions pinned by SHA as in `ci.yml`;
  - `actions/checkout` of each public companion repo at `main`, into `companions/<repo>`;
  - `cargo build -j 2 -p uqctl --locked`, then the script.
- [ ] **Step 3:** Run the uQ gate in the container (K4):
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features --locked --exclude uq-settings -- -D warnings`;
  - `scripts/lint-shell.sh`, `scripts/lint-settings.sh`.

  This slice changes no Rust in uQ, so the test suite is unaffected. Run
  `cargo test -j 2 -p uqctl -- --test-threads=2` once as a sanity check. Expected: PASS.
- [ ] **Step 4:** Commit (K5 trailers): `ci: check every companion app's uq/ folder against this uQ`
- [ ] **Step 5:** Open the PR (`settings-slice-e`) with the deviations list, linking the three
  merged companion PRs.

---

## E4: XZram (`/home/user/XZram`, branch `uq-settings-page`) — **needs repo access**

### Task E4.1: Discovery (no commit)

- [ ] **Step 1:** Clone `asafelobotomy/XZram` to `/home/user/XZram`. Read:
  - `AGENTS.md`, `docs/SCOPE.md`, `docs/AUDIT.md` and `docs/GUI-PHASE2.md` (the D-Bus list);
  - `crates/xzramd/src/dbus/mod.rs` and `apply_now.rs`;
  - `crates/xzram-cli/src/` (subcommands and `--json`);
  - `data/io.github.XZram.conf` (bus policy for unprivileged reads) and
    `data/io.github.xzram.policy`;
  - `PKGBUILD`, `packaging/aur/PKGBUILD` and `Makefile`;
  - the CI workflow (it greps PKGBUILD strings: keep them true).
- [ ] **Step 2:** Confirm:
  - the read methods `GetStatus`, `GetZramConfig`, `GetSysctl` and `ListSwaps` (`→ a{sv}`).
    Each value must be within uQ's supported types (`a{sv}` with `s`/`x`/`d`/`b`/arrays); if a
    reply nests `a{sv}` inside arrays beyond what `dbus_json.rs` supports, read that one through
    the CLI instead;
  - the unprivileged bus policy allows those methods, `Introspectable` and `Properties.Get`
    (the CHANGELOG says so);
  - the CLI write subcommands and their exact argv for:
    - setting the zram size (as a fraction of RAM or an absolute size);
    - setting the zram algorithm;
    - setting swappiness;
    - "apply now" against "stage" (the GUI has a "pending" banner);
  - which writes change active swap (they get the R8 confirm), and which run through pkexec
    (`xzram-helper`) rather than xzramd. Either way they are `[[command]]`s (E-F1).
- [ ] **Step 3:** Write the confirmed argv table into the PR description, and fix Task E4.2's
  table.

### Task E4.2: The page, packaging, CI and docs

**`page.toml`** *(argv confirmed in E4.1)*:
- `id = "xzram"`, `name = "Memory and swap"`, `icon = "media-flash"`;
- `keywords = ["swap", "zram", "memory", "ram", "swappiness", "compression"]`;
- `[requires] bins = ["xzram"]` (no `bus_name`: xzramd is activatable, E-F5);
- `[dbus]`: `name = "io.github.XZram1"`, `path` *(E4.1, likely `/io/github/XZram1`)*,
  `interface = "io.github.XZram.Manager"`,
  `methods = ["GetStatus", "GetZramConfig", "GetSysctl", "ListSwaps"]`, `bus = "system"`.

`[[setting]]` rows: `zram-size`, `zram-algorithm`, `swappiness` and `swaps`, each with keywords.

`[[command]]` tables *(E4.1)*:

| id | argv | args |
|---|---|---|
| `zram-size` | `/usr/bin/xzram …` | `size` = `^([1-9][0-9]?\|100)%$` or the CLI's own size syntax |
| `zram-algorithm` | `/usr/bin/xzram …` | `algorithm` = `^(zstd\|lz4\|lzo-rle\|lzo\|842)$` (the kernel's names; confirm XZram's list) |
| `swappiness` | `/usr/bin/xzram …` | `value` = `^([0-9]\|[1-9][0-9]\|1[0-9][0-9]\|200)$` |

Timeouts are 120000. A confirm goes on the writes that change active swap (R8). No swap-file
create or remove on this page in v1: those are disk writes better done in XZram's own app (Notes
question 5).

**Page:**
- memory and swap status;
- zram: size and algorithm;
- swappiness: a `QQC2.SpinBox` 0–200, applied on **Apply**, not on every step;
- active swap devices and files: read-only.

**Files:**
- `uq/pages/xzram/{page.toml,Page.qml,logic.js}` and `uq/README.md`;
- `tests/uq/…` as in E1;
- `PKGBUILD` and `packaging/aur/PKGBUILD` (package `xzram`, not `xzram-gui`): the R9 line and
  `optdepends`;
- `Makefile` install target: the same three files;
- CI job `uq` (programs: the built `xzram`);
- `README.md`, `AGENTS.md`, `CHANGELOG.md`.

- [ ] **Step 1:** Write the failing Node tests:
  - `parses_status` and the `a{sv}` fixtures;
  - `size_and_swappiness_validation`;
  - `patterns_match_manifest`, `rejects_hostile_args`, `confirms_match_plan`, `dbus_is_read_only`
    and `page_strings_are_plain`.

  Write the QML test: `test_reads_on_open`, `test_failed_write_restores`, `test_cancel_is_silent`
  and `test_activation_error_shown`. Run `node --test tests/uq/`. Expected: FAIL.
- [ ] **Step 2:** Implement the page. Run `node --test tests/uq/` and the local `check`.
  Expected: PASS.
- [ ] **Step 3:** Commit (K5 trailers): `feat(uq): the Memory and swap page for uQ Settings`
- [ ] **Step 4:** Packaging, CI and docs. Run XZram's full gate and the install check. Commit
  (K5 trailers): `packaging, ci, docs: ship and check the uQ Settings page`
- [ ] **Step 5:** Open the PR. In this repo, tick the E4 row in `docs/companion-pages.md` and
  update the CHANGELOG sentence (additive). Commit (K5 trailers):
  `docs: the Memory and swap page is in XZram`

---

## E5: scale_tail (`/home/user/scale_tail`, branch `uq-settings-page`) — **needs repo access**

### Task E5.1: Discovery (no commit)

- [ ] **Step 1:** Clone `asafelobotomy/scale_tail`. Read:
  - `AGENTS.md`;
  - `src/main.rs` (the `status --json` shape and the `tailscale` calls);
  - `uq/README.md` and `uq/widgets/scale-tail/widget.toml`;
  - `packaging/aur/PKGBUILD` (does it install the whole `uq/` tree, or `widgets/` only?);
  - `.github/workflows/ci.yml` and `tests/uq/tst_widget.qml`.
- [ ] **Step 2:** Confirm:
  - `scale_tail status --json` fields (state, exit node, exit-node options);
  - that `tailscale status --json` gives the peers, the account (`User`, `Self`), and
    `BackendState`;
  - that `tailscale set` takes `--accept-routes=true|false`,
    `--exit-node-allow-lan-access=true|false` and `--exit-node <x>`;
  - what an operator-less `tailscale up` prints, so `logic.js` can tell "needs operator" apart
    (it says "Access denied");
  - whether scale_tail has an "auto-connect" setting, and how it is set. If it isn't settable
    from a CLI, drop it from the page (Phase 4 §3 lists it; record the deviation);
  - the AUR package name, for E6.1's `optdepends` line.

### Task E5.2: The page, packaging, CI and docs

**`page.toml`**:
- `id = "scale-tail"`, `name = "Tailscale"`, `icon = "network-vpn"`;
- `keywords = ["tailscale", "vpn", "exit node", "network", "peers"]`;
- `[requires] bins = ["tailscale", "scale_tail"]`.

`[[setting]]` rows: `connection`, `exit-node`, `routes`, `lan-access`, `peers` and `account`,
each with keywords.

`[[command]]` tables:

| id | argv | output | args | confirm |
|---|---|---|---|---|
| `status` | `/usr/bin/scale_tail status --json` | json | — | — |
| `peers` | `/usr/bin/tailscale status --json` | json | — | — |
| `up` | `/usr/bin/tailscale up` | none | — | — |
| `down` | `/usr/bin/tailscale down` | none | — | — |
| `exit-node` | `/usr/bin/tailscale set --exit-node` | none | `target` = `^[A-Za-z0-9][A-Za-z0-9.:-]{0,252}$` (the widget's) | — |
| `exit-node-off` | `/usr/bin/tailscale set --exit-node=` | none | — | — |
| `routes-on` / `routes-off` | `/usr/bin/tailscale set --accept-routes=true` / `=false` | none | — | — |
| `lan-on` / `lan-off` | `/usr/bin/tailscale set --exit-node-allow-lan-access=true` / `=false` | none | — | — |
| `logout` | `/usr/bin/tailscale logout` | none | — | R8 |

Timeouts are 30000.

**Page:**
- Connection switch and state.
- Account: the user's login name, as plain text.
- Operator status. When a write is refused with "Access denied", the page shows: "Tailscale needs
  you to be its operator once. In a terminal, run: sudo tailscale set --operator=$USER". The
  text is fixed, with `$USER` left literal, since the operator is the local account rather than
  the Tailscale login. There is no pkexec (Notes question 8).
- Exit node: a `UqComboBox` of the exit-node options, plus "None".
- Accept routes and LAN access switches.
- Peers: a read-only list (name, online, OS), capped at 200.
- **Sign out…**.

**Files:**
- `uq/pages/scale-tail/{page.toml,Page.qml,logic.js}`;
- `uq/README.md` (gains the page row);
- `tests/uq/tst_page.qml` beside the existing `tst_widget.qml`;
- `tests/uq/{load-logic.mjs,logic.test.mjs,stub/…}`;
- `packaging/aur/PKGBUILD`, only if E5.1 found it installs `widgets/` only;
- CI: the existing Qt job gains the page test and `node --test`, plus the `uqctl` check job
  (R10), which replaces the "not run here yet" comment;
- `README.md`.

- [ ] **Step 1:** Write the failing tests:
  - Node: `parses_status_and_peers`, `operator_needed_detected`, `exit_node_options`,
    `patterns_match_manifest`, `rejects_hostile_args`, `confirms_match_plan`,
    `page_strings_are_plain`;
  - QML: `test_switch_up_down`, `test_exit_node_pick`, `test_operator_hint`,
    `test_logout_confirm_only`, `test_untrusted_peer_name_plain`.

  Run `node --test tests/uq/`. Expected: FAIL.
- [ ] **Step 2:** Implement. Run `node --test tests/uq/` and the local `check`. Expected: PASS.
- [ ] **Step 3:** Commit (K5 trailers): `feat(uq): the Tailscale page for uQ Settings`
- [ ] **Step 4:** Packaging, CI and docs. Commit (K5 trailers):
  `ci, docs: check the uQ Settings page`
- [ ] **Step 5:** Open the PR. In this repo:
  - tick the E5 row;
  - add the `optdepends` line from E5.1 to `packaging/uq/PKGBUILD` (additive);
  - update the CHANGELOG sentence.

  Commit (K5 trailers): `docs: the Tailscale page is in scale_tail`

## Notes and open questions

1. **noti-fi's do-not-disturb schedule (deviation 3).** Spec §8 lists one; noti-fi shows no
   schedule feature, only automatic DND for full-screen and sharing, and focus modes. Should
   noti-fi gain a schedule (a noti-fi feature, outside this slice), or is the automatic DND
   enough?
2. **Foundation requests R-A to R-F.** None blocks this slice. When R-A lands, scdr's and XZram's
   writes could move to D-Bus. When R-B lands, polling stops. When R-C lands, CI drops its stubs.
   When R-D lands, pages use `SwitchRow` and `AdvancedDetails`. Each is a small follow-up per
   repo. *(Record the foundation thread's answers here.)*
3. **Pinning `uqctl` to a commit (E-F4).** It's fine until a tag ships `check`. Which uQ tag will
   that be (0.3.0?), so the follow-up knows what to pin?
4. **crumBS page size (E-F21).** Five sections may not fit in about 400 lines without sibling
   components (R-E). If it doesn't fit, which section should wait: Leftovers (CLI only) or Apps
   (Phase 4 also plans an "Uninstall…" app action on every app row)?
5. **XZram swap files.** The v1 page leaves swap-file create, resize and remove to XZram's app.
   Should Settings offer them?
6. **Other distros.** crumBS's deb and rpm packages, and XZram's spec and debian files, don't
   ship the page; uQ is Arch-only. Ship it there too (it is inert)?
7. **scale_tail's AUR package name** isn't known here. E5.1 finds it, and E6.1's `optdepends`
   line waits for it.
8. **Tailscale operator.** The page tells the user the one-time `sudo tailscale set
   --operator=$USER` instead of running it through pkexec, because uQ's rule is that nothing runs
   as root except through the app's own polkit action, and Tailscale has none. Is the hint
   enough?
9. **Repo access.** noti-fi, XZram and scale_tail must be cloned under `/home/user` before E3, E4
   and E5. Their tables in this plan come from code-search fragments and are marked *(confirm in
   discovery)*.
10. **Commit trailer.** As instructed for this slice: `Claude Opus 5.5` and this session's
    `Claude-Session` URL, in every repo.
