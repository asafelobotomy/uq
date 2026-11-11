# uQ Settings enhancements slice D: startup and apps — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two new pages in `uq-settings`.
- **Startup** (under System) lists the apps that start at login, from the two XDG autostart
  folders and the user's own additions. uqd runs them itself, once per session, each in its own
  `app.slice` scope. Each row shows where it came from, a switch, a status and a start delay.
  The same page edits the user's environment variables.
- **Apps and scaling** (under Devices, after Displays) has five global switches for blurry
  XWayland, Electron, Chromium, Qt and Steam apps. It lists the X11 windows open now
  ("Blurry right now"), each with **Fix this app**. That opens a per-app fix: Wayland or X11,
  a size, extra variables and extra options. uQ's own launch path applies the fix, and a
  desktop-file override is written only when the user asks for it.

All three new domains (`env`, `compat`, `autostart`) are normal uQ domains: validated, saved
through slice A's single save path, journalled for undo and restore points, and explained. A
switch that `user.lua` overrides says so. **Profiles can hold them** (D1, Task 13): environment
variables, the app switches and each fix's platform and size, and each startup app's on/off and
delay. A profile can never add a startup command, an app's launch options, or a launcher-file
override.

**Architecture:**
- **uq-core:**
  - `desktop_entry.rs` is the one Desktop Entry parser. It moves out of the bridge and gains
    field codes, `OnlyShowIn`/`NotShowIn`, an `Exec` writer, a display quoter, the scope-name
    sanitiser and the scope-name random suffix.
  - `env.rs` is the `env` domain: validation, the blocklist, `session_env` and the
    `environment.d` text.
  - `compat/` is the `compat` domain:
    - `compat/hypr.rs`, the Hyprland and toolkit name table;
    - `compat/mod.rs`, the model and validation;
    - `compat/launch.rs`, the per-app argv builder that both the bridge and uqd use;
    - `compat/browsers.rs`, the Chromium flag-file table.
  - `autostart/` is the `autostart` domain plus the pure folder scan and run plan.
  - `lua/startup.rs` renders the xwayland option and the `hl.env` lines.
  - `startup_explain.rs` holds the pure `Explain` rows for all three domains, which uqd and the
    fake uqd share.
  - `profile/startup.rs` holds what a profile may hold of the three domains (Task 13).
- **uqd:**
  - The three domains become section domains (`GetSection`/`SetSection`/`Explain`/`Reapply`).
  - `files.rs` keeps every file outside `state.toml` in step with state:
    `environment.d/60-uq.conf`, Chromium flag files, desktop-file overrides, and the hidden
    copies of autostart entries.
  - `autostart.rs` is the runner. It adds `ListAutostart() → s`, `ShellReady()` and the signal
    `AutostartChanged()`. Only `main.rs` gives it a real process spawner.
- **uqctl:**
  - The bridge's `Launch` applies per-app fixes and calls `ShellReady` once the shell has sent
    its first request.
  - New command `uqctl launch <desktop-id>`, the path a `launch` keybind will use.
- **uq-settings:**
  - Qt-free page logic in `pages/startup.rs` and `pages/compat.rs`, tested against the fake uqd;
  - thin `startup_model.rs` and `compat_model.rs`;
  - `StartupPage.qml`, `AppsScalingPage.qml` and their components;
  - two registry entries;
  - the Profiles page offers the new keys (Task 13).

**Tech Stack:** Rust edition 2024, zbus 5, tokio, cxx-qt 0.10, Qt 6, Kirigami 6, Quickshell,
Hyprland 0.56.2–0.57.x (Lua config), systemd user manager (`systemd-run`, `systemd-cat`,
`journalctl`).

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. The authority
is §6 (Apps and scaling) and §7.3 (autostart and environment variables). This plan also covers:
- §2: the `user_lua`, `error` and `profile` sources for the new rows, and Explain;
- §3: history entries, labels and restore points for `env`, `compat` and `autostart`;
- §4: these domains in profiles (D1), within §10's rule that imports can't add autostart
  commands;
- §9: the quality rules;
- §10: the D-Bus summary `ListAutostart`, `SetAutostart` and `AutostartChanged`, and its
  security lines;
- §11 decisions 3 and 4;
- §12 row D.

It builds on slice A (`2026-11-10-uq-settings-slice-a-trust.md`, merged) and follows the layout
of slices B and C (`…-slice-b-profiles.md` and `…-slice-c-rules.md`, merged at `ac1cd3f`).
Task 13 follows slice C's Task 11 (profiles switch rules), whose code is in
`crates/uq-core/src/profile/rules.rs` and `crates/uqd/src/profiles.rs`.

**Paths.** The settings app lives at `apps/uq-settings` (not `crates/`). Its QML tests are in
`tests/settings-qml`, and the shell's are in `tests/qml`.

## Pre-flight rulings

A conflict scan against the code at `f3fb1e1`
(`.superpowers/sdd/2026-11-10-uq-settings-slice-d-startup-apps/preflight.md`) found 45 issues.
All rulings are accepted and folded into the task text below; this list is the index. The scan's
task count was 1–13; D1 added Task 13, so the docs task is now Task 14.

- **D1 (user decision, 2026-11-11, decision card).** Profiles can hold startup apps,
  environment variables and app fixes, in this slice. This replaces the earlier ruling (old R1)
  that profiles refuse them. Task 13 does it.

- **F0** Old R1 and Task 6's "profiles refuse these domains" tests are gone; Task 13 adds
  profile support with §10's limit: a profile never adds a startup entry, a command, launch
  options or an override (Tasks 6, 13).
- **F1** `StartupPaths` comes from `Paths` (`Paths::under` sandboxes tests); `serve()` keeps a
  disabled runner and only `main.rs` calls `serve_with` with `SystemSpawner` (Tasks 4, 7, 8).
- **F2** The bridge adds `Request::GetSection` and `Request::ShellReady` to `bridge/uqd.rs` (not
  to the shell's pass-through list); "shell ready" is the shell's first request line, and again
  on each `UqdUp`; `GetSection` is bounded by 500 ms (Task 9).
- **F3** `check_compat_files` refuses with `DesktopError::InvalidSetting`, never `Conflict`
  (which means "undo anyway?") (Tasks 7, 11).
- **F4** `render_candidate` puts `main_scale` in `Candidate` (stored monitor scale first, else
  live); `install` stores it before `persist` (Task 7).
- **F5** `refresh_overrides` and `with_dev_session` are `pub`; `main.rs` reads `UQ_DEV_SESSION`
  once, in Task 7 (Tasks 7, 8).
- **F6** `sharp_x11` reports `default` when off: a remap in `startup_explain`, used by uqd and
  the fake; `expected()`'s test is in Task 3 (Tasks 3, 6).
- **F7** Task 3 adds a skip for `compat.sharp_x11` in `provenance_rows_name_registry_settings`;
  Task 12 removes it (Tasks 3, 12).
- **F8** `model_core::Follow` gains `autostart`; every model's `FOLLOW` literal sets it (Task 12).
- **F9** `pages/startup.rs` and `pages/compat.rs` carry `cfg_attr(not(test), allow(dead_code))`
  until Task 12 wires them (Tasks 10, 11, 12).
- **F10** uqd's dev-dependencies enable tokio `test-util` for paused clocks (Task 8).
- **F11** `write_exec` writes `\n`, `\t`, `\r` as key-value escapes (Task 1).
- **F12** `exec_tokens` reports `lossy` (a field code inside a word or quotes, or an unquoted
  `'`); `override_text` refuses a lossy source; an unquoted `'` makes an autostart `Exec`
  unreadable (Tasks 1, 3, 4).
- **F13** Every marked-file write re-checks "ours or absent" just before it lands; an absent
  target is created with `hard_link` (fails if something appeared) (Task 7).
- **F14** Folder and desktop-file reads open the file, require a regular file, open
  non-blocking, and read at most 64 KiB (Tasks 4, 7).
- **F15** The run record has `ran`; a uqd restarted in the same session starts nothing new;
  `alive` is re-checked on every `ListAutostart` (Task 8).
- **F16** The journal tail filters on the run's unit and start time, caps output at 64 KiB, runs
  at most 4 at once, and is cached per run (Task 8).
- **F17** The env blocklist adds `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`,
  `XDG_CACHE_HOME`, `XDG_SEAT`, `XDG_VTNR`; `XDG_DATA_DIRS`, `XDG_CONFIG_DIRS` and `LD_AUDIT`
  are warned (Task 2).
- **F18** `environment.d` holds the env domain and the fixed-value switch lines only;
  `GDK_SCALE` and `STEAM_FORCE_DESKTOPUI_SCALING` go only to `hl.env` and autostart children
  (Tasks 5, 7).
- **F19** One `env::session_env(env, compat, main)` (no cursor) and `env::environment_d_lines`;
  uqd's `Desktop::child_env()` adds the cursor for autostart children (Tasks 5, 7, 8).
- **F20** R19's source path must be `<XDG_DATA_DIRS entry>/applications/<id>`; with no fix
  fetched, the override's own `Exec` runs (Task 9).
- **F21** Pages pre-check writes with the core validators and show `reason` only; uqd refusals
  lose a leading `invalid value for <key>: ` (Tasks 10, 11).
- **F22** `command_text` uses `desktop_entry::quote_command` (no `%%`, no key-value escape);
  `status_text` includes "Off in uQ" (Tasks 1, 10).
- **F23** Deprecated field codes stay errors (spec deviation 8) (Tasks 1, 4).
- **F24** A quoted arg holding only a field code is that field code (Task 1).
- **F25** The override wrapper is `env -- K=V …`; with no env there is no wrapper, but browser
  and extra args are still appended (Task 3).
- **F26** The bridge keeps a `LaunchEntry { exec: String, … }` built from `read_entry`
  (Task 1).
- **F27** File lists completed: `random_suffix` moves in Task 1; the D-Bus proxy test is
  extended in Task 8 (Tasks 1, 7, 8).
- **F28** Profile tests live in `profile/overlay.rs`, `profile/mod.rs` and the new
  `profile/startup.rs` (Task 13).
- **F29** Global Constraints use the task signatures: `argv(fix, id, exec)`,
  `override_text(source, path, fix, id)`, `explain_compat_extra` (Global).
- **F30** Each domain's `summary` lands in the task that adds it to `SECTION_DOMAINS`
  (Tasks 2, 3, 4, 6).
- **F31** `compat::preview` (page logic, pure) and `CompatModel.preview`; `set_var(-1, …)`
  appends (Tasks 11, 12).
- **F32** The fake uqd gains `refuse_next` for scripted `SetSection` refusals (Task 11).
- **F33** `emit_startup_env` writes its header only when it has lines (Task 5).
- **F34** `dbus.rs` logs only the event's kind on an emit error; `SectionChanged` does carry env
  values to bus clients, and the plan says so (Task 8, Global).
- **F35** Autostart children lose uqd's service variables; `systemd-run` is run by absolute path;
  `--quiet` (Task 8).
- **F36** `desktop_entry::clean_display` maps control and bidi format characters to spaces for
  names and journal lines (Tasks 1, 8).
- **F37** The 60 s tick also re-runs the flag-file sync (Task 7).
- **F38** In a dev session the override sync neither writes nor deletes (Task 7).
- **F39** `Runner::shell_ready_calls()` lets the bridge test observe `ShellReady` (Tasks 8, 9).
- **F40** `environment.d` is written with `atomic_write`, not created when empty, and removed
  only when it carries uQ's header (Task 7).
- **F41** `state.rs`'s full `State {}` literal gains each new field (Tasks 2–4).
- **F42** An added entry's `name` refuses control characters (Task 4).
- **F43** `AddApp` cuts the name to 100 chars; `exec_name` is the file name of `exec_argv[0]`;
  the app list's dirs differ from uqd's (accepted for v1) (Tasks 10, 11).
- **F44** Conflict labels read as singular: "The list of environment variables", "Apps and
  scaling", "The list of startup apps" (Task 7).

## Plan rulings

These were decided when the plan was written, before the scan.

- **R1, profiles (replaced by D1).** Profiles hold the three domains as Task 13 describes.
  `from_journal` stays `None` for them: startup keys reach "Create from history" through
  `startup_keys`, as rule switches go through `rule_toggles`.
- **R2, no `SetAutostart` method.** Writes go through `SetSection("autostart", json, origin)`,
  like every section. Slice C set the precedent: it added no rule-specific write methods. The
  runner and the file sync react to the committed state. Spec deviation 1.
- **R3, three section domains.** `env`, `compat` and `autostart` are `State` fields beside
  `rules`, not `Settings` fields (slice C's F21 ruling). So `SettingsChanged` doesn't carry them,
  and the bridge reads `compat` with `GetSection` when it needs it. `SECTION_DOMAINS` grows from
  8 to 11.
- **R4, one Desktop Entry parser.** The bridge's `desktop_entry.rs` moves to
  `uq_core::desktop_entry`. The bridge, uqd's runner, the override writer and the hidden-copy
  writer all use it. `uqctl::bridge::desktop_entry` becomes a re-export plus the bridge-only
  `Dirs`, `binary_exists` and `LaunchEntry` (F26).
- **R5, the shell is "ready" when it has sent the bridge its first request** (F2). The bridge
  writes `Hello` itself, so `Hello` proves nothing about the shell. The bridge calls the new
  `ShellReady()` once the first request line has been read, and again after each `UqdUp`. If no
  call comes, uqd starts autostart 20 s after it starts. No QML change.
- **R6, uqd is the parent of each autostart process.** `systemd-run --scope` execs the
  command, so uqd's child *is* the app, in its own scope.
  - uqd waits on the child for the exit status.
  - `systemd-cat -t uq-autostart-<sid>` puts the app's output in the journal under its own tag,
    so the last lines can be read back.
  - The scope moves the process out of `uqd.service`'s cgroup, so restarting uqd never kills
    autostarted apps.
- **R7, once per session (F15).** `$XDG_RUNTIME_DIR/uq/autostart-run.json` records
  `{session, ran, started: {id: {unit, pid, at}}}`. `session` is `HYPRLAND_INSTANCE_SIGNATURE`.
  - A uqd whose record has the same session and `ran: true` starts **nothing**; it only adopts
    the recorded entries' statuses.
  - In any session, uqd doesn't run an entry whose recorded pid is still alive in its recorded
    scope (`/proc/<pid>/cgroup` ends in `/<unit>.scope`).
- **R8, env reaches services through Hyprland's own export.** uQ's `hl.env` lines pass
  `true` as the third argument, so Hyprland runs `systemctl --user import-environment` and
  `dbus-update-activation-environment` for each name (the source is cited below). uqd adds no
  systemd client.
  - In a `uq-dev session` (`UQ_DEV_SESSION` set), the third argument is `false`. The nested
    session shares the real user manager.
- **R9, removed variables linger until logout.** Hyprland's `hl.env` only `setenv`s; nothing
  unsets a removed name. The page says: "Removed variables stay for apps started before you
  log out."
- **R10, no `$` in values.** `environment.d` expands `$VAR` and `${VAR}`, but `hl.env` stores
  the text literally, so one value would mean two things. v1 refuses `$` in values with "can't
  contain $". Notes question 1.
- **R11, compat wins over env.** When a compat switch and an `env` variable set the same name,
  the switch's line renders last and wins, in both `generated.lua` and `environment.d`. The env
  row's Explain detail says which switch replaced it. There's no cross-domain refusal, so undo
  and restore never get stuck.
- **R12, `QT_SCALE_FACTOR` isn't set globally.** It also scales Qt apps that run on Wayland, so
  with "Qt apps prefer Wayland" on they would be drawn twice as big. "Sharp X11 apps" sets
  `GDK_SCALE` only. The per-app fix's size sets `QT_SCALE_FACTOR` for that one app. Spec
  deviation 2; notes question 2.
- **R13, the "main monitor"** is the enabled output at position 0,0; if there's none, the first
  enabled output by name (F4: the candidate's stored monitor config is used when it sets that
  output's scale, else the live `monitors all` scale). Slice F's `monitors.primary` replaces
  this rule when it lands. With no scale known, the scale-derived lines (`GDK_SCALE`,
  `STEAM_FORCE_DESKTOPUI_SCALING`) are left out.
- **R14, `GDK_SCALE` is a whole number:** the scale rounded to nearest, with .5 rounding down
  (1.25 → 1, 1.5 → 1, 1.75 → 2, 2 → 2). The minimum is 1. GTK reads only integers. Notes
  question 3.
- **R15, autostart truth lives in `state.toml`.** It holds the on/off state, the delays and the
  added entries, so history, undo and restore cover them.
  - Turning off a *system* entry also writes the XDG-standard user copy with `Hidden=true` and
    the marker `X-uQ-Autostart=true`, as the spec asks. It's written only when no file with that
    id is in the user folder.
  - uqd never edits an unmarked file in `~/.config/autostart`. Turning off the user's own entry
    is recorded in state only, and the page says "Off in uQ".
  - The file sync rebuilds the marked copies from state after every commit (including undo,
    restore and profile switches) and at startup.
- **R16, added entries are stored, not written as files.** An added app (desktop id) or command
  (argv) lives in `autostart.added`. Commands are split with the Desktop Entry quoting rules and
  stored as argv, so they never pass through a shell.
- **R17, `ListWindows` is reused** for "Blurry right now" (slice C, `crates/uqd/src/rules.rs`
  `Desktop::windows_of`), filtered on `xwayland`. Window class is matched to a desktop id in the
  app, from installed desktop files (`StartupWMClass`, then file id, then the `Exec` program's
  name). No new uqd call.
- **R18, overrides and flag files are re-checked on uqd's 60 s tick** (F37), with no file
  watcher. That covers a system desktop file changed by an upgrade and a browser installed
  later. The sync recomputes each wanted file and rewrites it only when the text differs. It's
  at most 100 small reads a minute, and the check also runs at startup and after every commit.
- **R19, a marked override is never applied twice (F20).** When the bridge's lookup finds a
  desktop file with `X-uQ-Override=true`:
  - if a fix was fetched from uqd, it launches from the `X-uQ-Source` file and applies the fix
    itself. The source counts only if it equals `<d>/applications/<id>` for a `d` in
    `XDG_DATA_DIRS` (never the data home), is a regular file, and has the same id;
  - otherwise (no fix fetched, or a bad source) it launches the override's `Exec` as written,
    which already carries the fix.
- **R20, a dev session doesn't touch the real desktop.** With `UQ_DEV_SESSION` set:
  - the runner starts nothing (status "Not started in a test session");
  - desktop-file overrides are refused ("Not available in a test session"), because
    `XDG_DATA_HOME` isn't sandboxed there (`docs/developing.md`), and the override sync neither
    writes nor deletes anything (F38);
  - `hl.env` doesn't export (R8).
- **R21, `uqctl launch <desktop-id>`** is added. Spec §6 says `launch` keybinds use uQ's launch
  path, but no `launch` keybind exists yet (no keybinds domain; `base.lua` binds `exec_cmd`
  directly). This command is that path, for Phase 3's keybind editor and for live checks.
- **R22, scopes name their slice.** Both the bridge's `scope_command` and the runner pass
  `--slice=app.slice`. uqd runs in `session.slice` (`packaging/uqd.service`), and the slice must
  not depend on systemd's default for transient units. The existing launch tests are updated
  for the new flag.

**Spec deviations (recorded here and in the PR):**
1. **No `SetAutostart` (R2).** Autostart writes use `SetSection("autostart")`; spec §10's D-Bus
   summary is updated in Task 14.
2. **`QT_SCALE_FACTOR` is per-app only (R12).** "Sharp X11 apps" sets `GDK_SCALE` (whole
   numbers, R14), never `QT_SCALE_FACTOR` globally.
3. **Two D-Bus calls the spec doesn't list:** `ShellReady()` (R5), and `ListAutostart() → s`'s
   shape (it *is* in the spec).
4. **No `$` in env values (R10).** The spec allows any plain string.
5. **The env blocklist is longer** than the spec's (see "The env domain", F17).
6. **One launch builder for fixes and autostart.** A "Fix this app" fix also applies when that
   app is autostarted. The spec names only uQ's launch path; this is a consistent extension.
7. **uQ changes only files it marked.** Launcher overrides (`X-uQ-Override=true`), Chromium flag
   files (first-line marker) and hidden autostart copies (`X-uQ-Autostart=true`) are the only
   files uqd writes or deletes outside its own state, `generated.lua` and
   `environment.d/60-uq.conf`; an unmarked file or a symlink in their place is never changed.
8. **Deprecated `Exec` field codes are errors (F23).** The Desktop Entry spec says to drop
   `%d %D %n %N %v %m`; uQ keeps the bridge's behaviour and skips such an autostart entry with
   "Its command can't be read".
9. **Scale lines stay out of `environment.d` (F18).** The spec routes "Sharp X11 apps" through
   the §7.3 env path; `environment.d` applies to every login of the account (another desktop
   too), so the scale-derived lines go only to `hl.env` and autostart children.
10. **Profiles hold a subset (D1).** Spec §4's list doesn't name these domains. A profile holds
    env variables, the five switches, each fix's platform and size, and each startup entry's
    on/off and delay, keyed by uQ's ids; never commands, launch options, extra variables of a
    fix, or overrides (§10).

## Global Constraints

These are copied from the spec and the user's standing rules, except the lines marked
**ruling**, which this plan decides.

### Process

- **K4, no Qt here.** The build container has no Qt, no Quickshell and no Lua. These run in CI
  only: `uq-settings`, both `qmltestrunner` suites, the `tests/shell/*.sh` scripts that need
  Quickshell, and the `--ignored` Hyprland verify tests.
  - Every piece of logic that can be Qt-free is:
    - the page logic in `apps/uq-settings/src/pages/startup.rs` and `pages/compat.rs`;
    - the parsers, validators, scan and argv builders in `uq-core`.
  - Locally, run `cargo test -j 2 -p uq-core -p uqd -p uqctl`.
  - **Page logic locally (as in slice C).** The scratch crate at
    `/tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/t7check`
    symlinks `apps/uq-settings/src/pages`, `desktop_entries.rs`, `registry.rs`, `search.rs`
    and the app's `tests/` folder, and builds them without Qt. Run
    `cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::startup -- --test-threads=2`
    (and `pages::compat`, `pages::profiles`, `registry`).
    - A new top-level app module that page logic needs must be symlinked and listed in
      t7check's `lib.rs`. This slice needs none: it extends `desktop_entries.rs`, which is
      already linked.
    - CI's `cargo test -p uq-settings` stays the authority.
- **Build parallelism (earlyoom).** Every cargo command in this plan uses `-j 2`, and test runs
  add `-- --test-threads=2`. Set `CARGO_BUILD_JOBS=2` in the shell before the first build. Don't
  run two cargo commands at once.
- **K5, commits.** Every commit message ends with:
  ```
  Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc, so the trailers sit on their own lines after a blank line.
- **Process (as slice 2):**
  - use private `dbus-daemon` buses only (`uq_core::test_bus`), never the real session bus,
    `hyprctl`, `systemd-run` or `journalctl` (tests use the fakes below);
  - **ruling (F1):** no test may read or write the real home. Every uqd rig builds its paths
    with `Paths::under(temp)`, which also sandboxes `StartupPaths`; `serve()` never spawns a
    process;
  - `cargo fmt`, `clippy -D warnings` and the tests must be clean **at every commit**;
  - keep new files at or under about 400 lines;
  - run builds in the foreground and leave nothing running.
- **Dead code (F9).** uq-core's additions are `pub` in a library and need no gating. The
  uq-settings page logic of Tasks 10 and 11 lives in a binary crate and is unused until Task 12,
  so each of `pages/startup.rs` and `pages/compat.rs` starts with
  `#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 12"))]`, which
  Task 12 removes. uqd's `refresh_overrides` and `with_dev_session` are `pub` (the binary calls
  them, F5); the `files.rs` swap test hook is `#[cfg(test)]`.
- **CI flake.** `tests/shell/pam_stacks.sh` is known to flake in CI. If it's the only red job,
  re-run it rather than chasing it in this slice. Every other failure is real.
- **No new crates.** Hashing, inotify and systemd clients aren't needed (R7, R8, R18). Process
  spawning uses `tokio::process`, which uqd already enables. Task 8 enables tokio's `test-util`
  *feature* in uqd's dev-dependencies (F10); `Cargo.lock` doesn't change.

### The env domain (§7.3)

- **Shape.** The section JSON is `{"vars": [{"name": "…", "value": "…"}]}`, in list order, at
  most 100 entries.
  - It's stored as `[[env.vars]]` in `state.toml`.
  - Loading is lenient per entry (slice C's F1 pattern): an entry that fails to parse or
    validate is dropped with `tracing::warn!`, and the others are kept.
- **Names:** `^[A-Z_][A-Z0-9_]*$`, 1–128 chars, unique within the list.
- **Values:**
  - plain strings of at most 4096 bytes;
  - no control characters (newline, CR, NUL, tab, U+0000–U+001F, U+007F);
  - no `$` (R10).

  An empty value is allowed: it sets the variable to "".
- **Blocked names** (refused with "uQ needs this one to run your session, so it can't be
  changed here"):
  - from the spec: `WAYLAND_DISPLAY`, `DISPLAY`, `XDG_RUNTIME_DIR`, `XDG_SESSION_*`,
    `HYPRLAND_*`, `DBUS_SESSION_BUS_ADDRESS`;
  - **ruling, added (spec deviation 5):**
    - `XDG_CURRENT_DESKTOP`, which autostart and portals match against;
    - `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_STATE_HOME`, `XDG_CACHE_HOME` (F17: they would
      move uQ's own settings, and the `environment.d` folder itself, at the next login);
    - `XDG_SEAT`, `XDG_VTNR`;
    - `UQ_*`, since `UQ_DEV_SESSION` turns off `base.lua`'s session hooks;
    - `HOME`, `USER`, `LOGNAME`;
    - `XCURSOR_THEME`, `XCURSOR_SIZE`, `HYPRCURSOR_SIZE` (the Appearance page sets these);
    - `QT_QPA_PLATFORMTHEME` (`base.lua` sets it for uQ's Qt colours).

    The refusal for these last four says "This is set by the Appearance page."
- **Warned names:** `LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `PATH`, `XDG_DATA_DIRS` and
  `XDG_CONFIG_DIRS` are allowed. The page shows a warning under the row: "This can stop apps
  from starting. Remove it if apps break." Nothing extra is confirmed.
- **Per-app fixes and profiles reuse this check.** `compat.apps[].env` entries and a profile's
  `[env]` keys go through the same `env::check_var` function (name, value, blocklist), and the
  same warnings apply.
- **Where it applies (§7.3):**
  - `hl.env(name, value, export)` lines in `generated.lua` (R8), so `renders_section("env")` is
    true and `change_for("env")` is `Change::Reload`;
  - `~/.config/environment.d/60-uq.conf`, written by uqd's file sync (Task 7).
- **One list, three places (F19, ruling).**
  - `env::session_env(env, compat, main) -> Vec<(String, String)>`: env vars in list order, then
    the switch lines (`compat::launch::switch_env`), a switch's name replacing an env one (R11).
    No cursor lines. `generated.lua` renders exactly this list with `hl.env`.
  - `env::environment_d_lines(env, compat) -> Vec<(String, String)>`: the same merge without
    the scale-derived lines (`GDK_SCALE`, `STEAM_FORCE_DESKTOPUI_SCALING`; F18, spec deviation 9).
  - uqd's `Desktop::child_env()`: the theme's cursor lines (`XCURSOR_THEME`, `XCURSOR_SIZE`,
    `HYPRCURSOR_SIZE`, as `emit_generated` renders them) followed by `session_env`. Autostart
    children get uqd's environment (minus F35's service variables) plus this.
- **`environment.d` text (ruling).**
  - The file starts with the header
    `# Written by uQ (Startup page). Changes here are replaced; use uQ Settings.`
  - Then there's one line per entry of `environment_d_lines`.
  - Each value is single-quoted: `NAME='value'`.
  - A value holding `'` is double-quoted instead, with `\`, `"` and `` ` `` backslash-escaped.
    `$` can't occur (R10), so systemd's expansion never fires.
  - The test `environment_d_round_trips` parses the output with a small reference parser that
    follows `systemd`'s `env-file.c` rules for these two quoting forms.
  - **Ruling:** uqd owns this file name and rewrites it whether or not it carries the header,
    like `generated.lua`. It's written with `atomic_write`. With no lines it isn't created, and
    an existing one is removed only when it starts with the header (F40).
- **The page says (§7.3):** "Changes apply to apps started after the change, and fully after you
  log out and back in." The §6 switches are listed read-only, with a link to Apps and scaling.
- **Bus visibility (F34).** `SectionChanged("env")` carries the values to every session-bus
  client of the same user (the bridge forwards it to the shell), as `state.toml` holds them. Logs
  and history summaries never print values.

### The compat domain (§6)

- **Shape (ruling).** The section JSON is:
  ```
  { "sharp_x11": false, "electron_wayland": false, "chromium_wayland": false,
    "qt_wayland": false, "steam_scale": false,
    "apps": [ { "id": "code.desktop", "platform": "auto", "scale": null,
                "env": [{"name": "…", "value": "…"}], "args": ["…"], "elsewhere": false } ] }
  ```
  - `apps` is a list (one history leaf, as rules are), at most 100 entries, unique by `id`.
  - Ids pass `desktop_entry::valid_desktop_id` (`settings::valid_pinned_id`, no leading `.`, no
    `..`) and are normalised to end in `.desktop`, so the joined path's file name is always the
    id itself.
  - `platform` ∈ `auto | wayland | x11`.
  - `scale` is `null` or 1.0–3.0 in steps of 0.25.
  - `env`: at most 20 entries, each checked by `env::check_var`.
  - `args`: at most 20 strings, each 1–1024 chars, with no control characters.
  - An app entry with nothing set (`auto`, `null`, no env, no args) is refused: "This fix
    doesn't change anything."
- **Global switches (§6), as the name table renders them** (`compat/hypr.rs`, R12–R14):

  | Switch | `generated.lua` | `hl.env` and autostart children | `environment.d` |
  |---|---|---|---|
  | `sharp_x11` "Sharp X11 apps" | `xwayland = { force_zero_scaling = true }` | `GDK_SCALE=<R14 of main scale>` | — (F18) |
  | `electron_wayland` "Run Electron apps on Wayland" | — | `ELECTRON_OZONE_PLATFORM_HINT=auto` | same |
  | `chromium_wayland` "Run Chromium-based browsers on Wayland" | — | — (flag files, below) | — |
  | `qt_wayland` "Qt apps prefer Wayland" | — | `QT_QPA_PLATFORM=wayland;xcb` | same |
  | `steam_scale` "Steam UI scale" | — | `STEAM_FORCE_DESKTOPUI_SCALING=<main scale, ≤2 places>` | — (F18) |

  Each switch's description is one plain sentence ending in "Applies to apps started after the
  change."
- **Per-app fix (§6), the launch argv (`compat::launch::argv(fix, id, exec)`, F29).** The inputs
  are the entry's argv (from `Exec`, field codes handled), the fix and the desktop id. A per-app
  scale is explicit, so no main scale is needed. The output is
  `["env", "--", "K=V", …, <exec argv…>, <browser args…>, <extra args…>]`. With no env, there's
  no `env` wrapper, but browser args and extra args are still appended (F25). The pieces, in
  order:
  - `platform = wayland` adds `GDK_BACKEND=wayland,x11`, `QT_QPA_PLATFORM=wayland;xcb`,
    `SDL_VIDEODRIVER=wayland,x11` and `ELECTRON_OZONE_PLATFORM_HINT=wayland`. Each has a
    fallback, so a toolkit without Wayland still starts.
  - `platform = x11` adds `GDK_BACKEND=x11`, `QT_QPA_PLATFORM=xcb`, `SDL_VIDEODRIVER=x11` and
    `ELECTRON_OZONE_PLATFORM_HINT=x11`.
  - `scale = s` adds `GDK_SCALE=<R14 of s>` and `QT_SCALE_FACTOR=<s>`, plus
    `STEAM_FORCE_DESKTOPUI_SCALING=<s>` for `steam.desktop`.
  - The fix's own `env` comes last, so the user's own entries win over the platform and scale
    ones.
  - For a desktop id in the browser table (`compat/browsers.rs`), it adds `--ozone-platform=…`
    for wayland or x11, and `--force-device-scale-factor=<s>` for a scale.
  - Extra args are appended after the program's own args.

  It's refused (`CompatError::Unsafe`) when an env wrapper is needed and the exec argv's first
  element contains `=`, since GNU `env` would read it as an assignment. The launch then goes
  ahead without the fix, and the bridge logs it.
- **"Also when started elsewhere" (`elsewhere = true`, §6, desktop-file overrides).** Task 7's
  file sync writes `$XDG_DATA_HOME/applications/<id>`, only when:
  - the target doesn't exist, or it exists, is a regular file (not a symlink), and its
    `[Desktop Entry]` group has `X-uQ-Override=true`;
  - a source exists: the first `applications/<id>` in `$XDG_DATA_DIRS`. The user's own folder
    never counts as a source.

  The content (`compat::launch::override_text(source_text, source_path, fix, id)`, F29):
  - The source text is copied line for line.
  - Every `Exec=` key in `[Desktop Entry]` and in each `[Desktop Action *]` group becomes
    `Exec=env -- "K=V" … <original tokens, browser args and extra args inserted before the first
    %f/%F/%u/%U token>` (no `env --` when the fix has no env, F25).
    - It's built from `desktop_entry::exec_tokens` and written with `desktop_entry::write_exec`
      (Task 1).
    - Field codes are kept, so other launchers still pass files.
    - **Ruling (F12):** a source `Exec` that `exec_tokens` marks `lossy` (a field code inside a
      word or in quotes, or an unquoted `'`) isn't rewritten: `override_text` returns
      `CompatError::BadExec`, recorded as `OverrideState::Error("uQ can't safely change this
      app's launcher file.")`.
  - `DBusActivatable=true` becomes `DBusActivatable=false`, or launchers would skip `Exec`.
  - `X-uQ-Override=true` and `X-uQ-Source=<absolute source path>` are added to
    `[Desktop Entry]`.
  - A header comment comes first:
    `# Written by uQ Settings (Apps and scaling). Remove the fix there; uQ replaces this file when the app updates.`
  - With no env, no browser args and no extra args, the file isn't written. A fix that only
    sets `auto`/`null` can't be saved anyway.
  - **Writes (F13):** a temp file in the same folder, then, immediately before it lands, the
    target is checked again: if it's still marked (`is_ours`), `rename`; if it's still absent,
    `hard_link(temp, target)` (fails when something appeared) and unlink the temp; otherwise
    nothing is written and Explain says `own_file`. A symlink at the target is never followed or
    replaced: it counts as "not ours".
  - **Deletion:** when the fix is removed, or `elsewhere` is turned off, uqd deletes the file.
    It re-checks the marker immediately before `remove_file` and never deletes an unmarked
    file.
  - **Refusals before commit (ruling, §9 "no silent failure", F3).**
    `SetSection("compat")` refuses turning `elsewhere` on, with
    `DesktopError::InvalidSetting` (never `Conflict`, which History and `uqctl undo` read as
    "undo anyway?"), when:
    - an unmarked file already exists: "You already have your own copy of this app's file, so
      uQ won't replace it.";
    - there's no source: "uQ can't find this app's file.";
    - the session is a dev session (R20): "Not available in a test session".

    Later failures (I/O, a source removed by uninstall, a write that found a file appeared, or a
    state reached by restore that skips these checks) are reported by Explain (Task 6), never
    silently.
- **Chromium flag files (`chromium_wayland`, §6).**
  - `compat/browsers.rs` has one row per browser: `{desktop_ids, flags_file, name}`:
    - Chromium: `chromium.desktop` → `chromium-flags.conf`;
    - Google Chrome: `google-chrome.desktop` → `chrome-flags.conf`;
    - Brave: `brave-browser.desktop` → `brave-flags.conf`;
    - Vivaldi: `vivaldi-stable.desktop` → `vivaldi-stable.conf`;
    - Microsoft Edge: `microsoft-edge.desktop` → `microsoft-edge-stable-flags.conf`.
  - A browser is "installed" when one of its desktop ids exists in the data dirs.
  - Which flags file each Arch package reads is a **live-check item**, as is whether
    Chromium ≥ 140 still takes `--ozone-platform-hint`.
  - When the switch is on, uqd writes `$XDG_CONFIG_HOME/<flags_file>` for each installed
    browser (F13's write rule). The first line is the marker
    `# Managed by uQ (Apps and scaling). uQ only changes files that start with this line.`,
    then `--ozone-platform-hint=auto`.
  - An existing file whose first line isn't the marker is never touched. Explain reports it
    (detail `own_file`), and the page says "Brave has its own flags file, so uQ left it alone."
  - Turning the switch off deletes only marked files, re-checking the marker immediately
    before deleting.
  - The 60 s tick re-runs this sync, so a browser installed later gets its file (F37).
- **Validation is in uq-core; filesystem checks are in uqd.** `Compat::from_section` is pure.
  The `elsewhere` checks above need the disk, so they run in uqd's `set_section` hook (Task 7).

### The autostart domain and runner (§7.3, §10, §11 decision 3)

- **Shape (R15, R16).** The section JSON is:
  ```
  { "files": [ { "id": "nm-applet.desktop", "enabled": false, "delay": null } ],
    "added": [ { "id": "a-3f9c", "name": "Syncthing", "app": "syncthing-start.desktop",
                 "enabled": true, "delay": 10 },
               { "id": "a-0b12", "name": "Backup check", "argv": ["backup-check", "--quiet"],
                 "enabled": true, "delay": null } ] }
  ```
  - `files` holds the user's choices about entries found in the folders, keyed by file id
    (`valid_desktop_id`). It has at most 200 entries, unique, and each needs `enabled` or
    `delay` set (a page or profile write that empties one removes it).
  - `added` has at most 50 entries:
    - `id` is `^a-[0-9a-f]{4,8}$`, made by uq-core like rule ids (`rules::fresh_prefixed`, the
      generator behind `fresh_id`, with the prefix as a parameter) and assigned when missing.
    - `name` is 1–100 chars of plain text, with no control characters (F42).
    - Each entry has exactly one of `app` (a valid desktop id) or `argv`. `argv` has 1–64 args,
      each 1–1024 chars with no control characters, and the first arg is non-empty with no `=`.
  - `delay` is `null` or 0–600 seconds.
- **The folders (§7.3, §10).**
  - `$XDG_CONFIG_HOME/autostart` (the user's, default `~/.config/autostart`);
  - then each `$XDG_CONFIG_DIRS` entry plus `/autostart` (default `/etc/xdg/autostart`).
    **Ruling:** honouring `XDG_CONFIG_DIRS` *is* the XDG system folder.
  - Only files named `<valid_desktop_id>` ending in `.desktop` are read: at most 200 per
    folder, at most 64 KiB each.
  - **Reads (F14):** symlinks are followed for reading only; the file is opened with
    `O_NONBLOCK`, must be a regular file once opened (`File::metadata().is_file()`), and is read
    through `take(64 KiB)`. A FIFO, device or directory is skipped.
  - A user file shadows system files with the same id.
- **Effective entry for an id (`autostart::scan`, pure, Task 4):**
  - a user file without `X-uQ-Autostart=true` is source `yours`;
  - otherwise the first system file is source `system`;
  - a user file *with* the marker is uQ's hidden copy of the system entry. It's ignored for
    reading, and it's an orphan for the sync when no system file exists.
  - Added entries are source `added`.
- **Rules applied, in order, each giving a plain skip reason when it skips:**
  1. `Type` must be `Application`.
  2. `Hidden=true` in the effective file: "Turned off by a file in your autostart folder" for
     `yours`, "Turned off by the system" for `system`. The switch is disabled for these.
  3. `OnlyShowIn`/`NotShowIn` are matched against the fixed desktop list `["Hyprland", "uQ"]`
     (the spec's `XDG_CURRENT_DESKTOP=Hyprland:uQ`). **Ruling:** the fixed list is used, not
     uqd's environment, so the result doesn't depend on how the session was started. Skip
     reason: "Not for this desktop".
  4. `TryExec`: the program must exist (an absolute path, or found on uqd's `PATH`). Skip
     reason: "Not installed".
  5. `Terminal=true`: skipped with "Needs a terminal, which uQ doesn't open at login".
  6. `Exec` must parse (Task 1) and must not be `lossy` through an unquoted `'` (F12); a
     deprecated field code is an error too (F23). Skip reason: "Its command can't be read".
  7. Enabled: the state's `enabled` if set, else `X-GNOME-Autostart-enabled` (absent means
     true). When off, the status is `off`.
  8. Delay: the state's `delay` if set, else `X-GNOME-Autostart-Delay` (clamped to 0–600),
     else 0.

  Skipped entries (rules 1–6) can't be toggled (`can_toggle: false`).
- **How each runs (§7.3, R6, R22, F35).** It's spawned directly (no shell) with
  `tokio::process`:
  ```
  /usr/bin/systemd-run --user --scope --quiet --collect --slice=app.slice
      --unit=app-uq-autostart-<sid>-<rand8hex>
      -- systemd-cat -t uq-autostart-<sid> -- <argv from compat::launch::argv>
  ```
  - `systemd-run` is resolved to an absolute path once, at runner start, from uqd's own `PATH`,
    so a user `PATH` in the child env never changes which program uqd runs.
  - `<sid>` is the entry id without `.desktop`, sanitised by `desktop_entry::unit_id`:
    `[A-Za-z0-9_-]`, others become `_`, at most 64 chars (today's `scope_command` rule).
  - `argv` comes from `exec_argv` with the field codes for no files: `%f %F %u %U` are
    dropped; `%i` becomes `--icon <Icon>` if set; `%c` becomes the `Name`; `%k` becomes the
    file path. A per-app compat fix for the same id applies (spec deviation 6).
  - For an added app, the desktop file comes from the applications dirs, as the launcher finds
    it (F14's read rule).
  - Working directory: `Path=` when it's an absolute existing directory, else `$HOME`.
  - Environment: uqd's own, minus `INVOCATION_ID`, `JOURNAL_STREAM`, `SYSTEMD_EXEC_PID`,
    `MANAGERPID`, `NOTIFY_SOCKET` and `LISTEN_*`, plus `Desktop::child_env()` (F19), so
    autostarted apps see what shell-launched apps see.
- **Status** (`ListAutostart`, per entry):
  - `waiting` (delay not over);
  - `running`;
  - `exited` (status 0);
  - `failed` (non-zero or a signal), with `exit` and `log` (F16):
    `journalctl --user _SYSTEMD_USER_UNIT=<unit>.scope -t uq-autostart-<sid> --since @<at> -n 10 -o cat --no-pager -q`,
    run with a 2 s timeout, stdout capped at 64 KiB, at most 4 at once, cached per
    `(unit, exit)`, read only for failed entries, each line cleaned with
    `desktop_entry::clean_display` (F36) and cut to 200 chars;
  - `off`;
  - `skipped` (with `reason`);
  - `not_started`: on, but not run this session ("Starts next time you log in");
  - `unknown_exit` ("Started earlier; finished"): a recorded entry whose pid is gone, so the
    exit code is unknown. `alive` is re-checked on every `ListAutostart` (F15).
- **When.** On `ShellReady()` or 20 s after uqd starts, whichever is first, once per uqd
  process, subject to R7 (a same-session restart starts nothing).
  - `SetSection("autostart")` and profile switches never start or stop anything. Turning an
    entry on reads "Starts next time you log in", and turning one off doesn't close the running
    app.
  - **Ruling:** there's no "Start now" button in v1 (notes question 4).
- **Not in a dev session** (R20).
- **Security (§10).** The runner runs only effective entries from the folders above and
  `autostart.added`, each in its own scope.
  - The argv never passes through a shell. The ids, the unit name and the journal tag are
    sanitised.
  - Profiles can only turn existing entries on or off and set their delay (D1, Task 13); they
    and imports can't add entries or commands.
  - Any session client can call `SetSection("autostart")`, as it can write
    `~/.config/autostart` itself, so this adds no new reach. Review Focus 4.
- **The hidden copy (R15).** `autostart::hidden_copy(system_text)` copies the system file,
  sets `Hidden=true` (replacing an existing `Hidden=` line in `[Desktop Entry]`) and adds
  `X-uQ-Autostart=true`. A header comment comes first:
  `# Written by uQ Settings: turned off on the Startup page. uQ removes this file when you turn it back on.`
  The file sync (Task 7) keeps one for each system entry that is off in state and has no user
  file, using F13's write rule. It deletes marked copies that are no longer wanted, re-checking
  the marker immediately before deleting.

### Hyprland names (Task 3 holds them)

- **Source.** The names come from the pinned series' own source. CI's `uq-hyprland-pin` check
  enforces 0.56.2–0.57.x, as in slice C:
  - Hyprland `v0.56.2` (tag `efb50993780079460b0cbed1363e2166a2de1d9f`);
  - Hyprland `v0.57.0` (tag `0aa6745f705f2c4b0a5004eb019ef76e5bbdb426`).

  This plan's author read these files at both tags:
  - `src/config/values/ConfigValues.cpp`:
    `MS<Bool>("xwayland:force_zero_scaling", "forces a scale of 1 on xwayland windows on scaled displays.", false)`
    (line 549 at 0.56.2, line 653 at 0.57.0). In the Lua table form uQ renders, that's
    `hl.config({ xwayland = { force_zero_scaling = true } })`, the same path-to-table mapping
    `lua/look.rs` uses for `decoration:blur:enabled`.
  - `src/config/lua/bindings/LuaBindingsConfigRules.cpp`, `hlEnv` (registered as
    `setMgrFn(L, mgr, "env", hlEnv)`, line 1400 at 0.56.2 and 1374 at 0.57.0):
    - `hl.env(name, value[, dbus])`: `name` and `value` are strings, and an empty name is an
      error.
    - On a reload (not the first launch) it returns early when the value is unchanged.
      Otherwise it calls `setenv`.
    - When `dbus` is true it runs
      `systemctl --user import-environment '<name>' && hash dbus-update-activation-environment 2>/dev/null && dbus-update-activation-environment --systemd '<name>'`.
      That's an exec-once on the first launch and a spawn on a reload.
    - Nothing ever unsets a name (R9). The name is pasted inside single quotes, which uQ's name
      rule (`[A-Z_][A-Z0-9_]*`) makes safe; Review Focus 1.
- **Not confirmed from source** (each is a live-check item, and CI's `--verify-config` case
  covers the syntax):
  - that `getoption xwayland:force_zero_scaling` answers on 0.56 and 0.57 (needed for
    Explain's `runtime` source);
  - that toggling `force_zero_scaling` on reload applies to *new* X11 windows without
    restarting XWayland;
  - that GTK 3 and GTK 4 on Wayland ignore `GDK_SCALE`, so the global line doesn't double
    their size (R12, R14);
  - that `ELECTRON_OZONE_PLATFORM_HINT=auto` still selects Wayland on the installed Electron
    (spec: "Electron 28 and newer");
  - the browser flag-file names, and whether Chromium still reads `--ozone-platform-hint`;
  - `STEAM_FORCE_DESKTOPUI_SCALING` with a fractional value.
- **Ruling, one place to fix.** Every Hyprland key, env name and value spelling lives in
  `crates/uq-core/src/compat/hypr.rs` (`XWAYLAND_ZERO_SCALING`, `SWITCH_ENV`, `PLATFORM_ENV`,
  `SCALE_ENV`) or in `compat/browsers.rs`. The renderer, the launch builder, the overrides,
  Explain, the profile labels and the page's catalogue all read them from there. If a live check
  fails, the fix is one row.

### Sources and history (§2, §3, §4)

- **Explain (§2).** Three pure functions in `uq_core::startup_explain` are shared by uqd and the
  fake uqd (slice C's F7 pattern). Rows are `{source, live, stored, detail}`.
  - `explain_env(state, user_lua, rejected, report, path)`. There's one row per variable,
    keyed `vars.<NAME>`:
    - `error` when the last `env` write was rejected by verification, or the
      `environment.d` write failed (detail `{message}`);
    - `user_lua` (with `line` and `path`) when `user.lua` holds `hl.env(` followed by the
      quoted name. The scan uses the shared comment cutter (`strip_lua_comment`); F8 says `--`
      never counts.
    - otherwise `uq`, with detail `{replaced_by: "<switch key>"}` when R11 applies.

    It also gives a section row `"env"` with `error` after a rejected write (slice C's F9).
  - `explain_compat_extra(state, user_lua, rejected, report, path)`: every compat row except
    `sharp_x11`:
    - Rows for the env-based switches check `user.lua` for each of their names (`user_lua`).
    - `chromium_wayland` has detail
      `{browsers: [{name, file: "ours" | "own_file" | "none" | "error", message?}]}`.
    - One `apps.<id>` row per fix. Its detail is
      `{override: "written" | "own_file" | "no_source" | "error" | "off", message?}`. The
      source is `error` for `error`, and for `no_source` while `elsewhere` is on.
    - There's a section row `"compat"` after a rejected write.
  - `sharp_x11` comes from slice A's generic option path (a new `provenance::OPTIONS` row,
    `compat.sharp_x11` → `xwayland:force_zero_scaling`, `NullSource::Default`, setting id
    `sharp-x11`). `provenance::expected` learns it (`Some(true)` when on, `None` when off), so
    `runtime` works through `getoption`. **Ruling (F6):** `startup_explain::default_when_off`
    rewrites that row's source from `uq` to `default` when it's off, since uQ renders nothing
    then; uqd and the fake both call it.
  - `explain_autostart(state, report)`: a section row `"autostart"` with `error` when a hidden
    copy couldn't be written or removed, else `uq` rows by entry id.
  - **Ruling:** there's no `runtime` source for env or the env-based switches. A running
    process's environment isn't something uqd reads.
  - **`profile` source (D1, Task 13).** `profile::mark_startup` turns a `uq` row into
    `{source: "profile", detail: {name}}` while the active profile's value still holds, as
    `mark_rules` does for rules.
- **History (§3).** All three are journal domains through `SECTION_DOMAINS`. Each domain's
  `summary` lands with the domain (F30). Summaries never print values (env values can be
  secrets) or JSON:
  - `env::summary`: "Environment variable GDK_BACKEND added", "… changed" or "… removed", up to
    three and then "and N more".
  - `compat::summary`:
    - "Sharp X11 apps turned on" (per switch);
    - "Fix for code added", "… changed" or "… removed". This uses the id without `.desktop`;
      the core has no app names.
  - `autostart::summary`:
    - "nm-applet turned off at login", "… turned on at login";
    - "Start delay for nm-applet changed";
    - "Syncthing added to startup" or "… removed from startup". Added entries use their
      name, and file entries use the id without `.desktop`.
  - **Labels:** `history::label` gains these in the same task that adds each domain to
    `SECTION_DOMAINS` (Tasks 2–4), because `every_key_a_summary_can_show_has_a_plain_label`
    needs them at once (slice C's F11):
    - `("env", "vars")` → "Environment variables";
    - `("compat", "sharp_x11")` → "Sharp X11 apps";
    - `("compat", "electron_wayland")` → "Run Electron apps on Wayland";
    - `("compat", "chromium_wayland")` → "Run Chromium-based browsers on Wayland";
    - `("compat", "qt_wayland")` → "Qt apps prefer Wayland";
    - `("compat", "steam_scale")` → "Steam UI scale";
    - `("compat", "apps")` → "App fixes";
    - `("autostart", "files")` → "Apps that start at login";
    - `("autostart", "added")` → "Your own startup apps".
  - uqd's `domain_label` (the undo-conflict sentence "{label} was changed again after this")
    gains "The list of environment variables", "Apps and scaling" and "The list of startup
    apps" (F44).
  - Undo puts back a whole list (`vars`, `apps`, `files`, `added`), as for rules (F36). The
    README says so.
  - A profile switch journals one entry per changed domain with origin `profile:<id>`, through
    the same summaries (Task 13).

### Launch path (§6, §11 decision 4)

- **What's "uQ's launch path".** The bridge's `Launch {desktop_id}`
  (`crates/uqctl/src/bridge/launch.rs::plan`), used by:
  - the app menu (`shell/launcher/AppMenu.qml`);
  - Spotlight (`shell/logic/launcher.js`);
  - pins (`shell/bar/SystemIsland.qml`, `shell/logic/dock.js`);
  - from Task 9 on, `uqctl launch` (R21).

  `Launch {argv}` and `Launch {run_id}` have no desktop id, so no fix applies.
- **Ruling, how the bridge gets the fix (F2).**
  - `Link::call` accepts only `Request::parse`'s whitelist (`crates/uqctl/src/bridge/uqd.rs`),
    so Task 9 adds `Request::GetSection(domain)` (its string reply parsed as JSON) and
    `Request::ShellReady`. Neither joins `handle()`'s pass-through list, so the shell can't call
    them.
  - Per launch, `tokio::time::timeout(500 ms, link.call("GetSection", ["compat"]))`. A failure, a
    timeout, garbage or uqd being down means no fix: a launch never fails over a fix.
  - The fix is matched by normalised id.
- **Ruling, order inside `plan`:**
  1. find the file (R19 for marked overrides);
  2. `TryExec`;
  3. `exec_argv` (Task 1, with `%i`/`%c`/`%k`);
  4. the terminal prefix;
  5. `compat::launch::argv`;
  6. `scope_command`.

  For `Terminal=true` apps, the env wrapper goes before the terminal argv. So the env reaches
  the terminal and, through it, the app.
- **Env changes reach apps started after the change.**
  - `hl.env` updates Hyprland's environment on reload. The bridge launches through Hyprland's
    `exec_cmd`, so the next launch sees it.
  - systemd user services get it through Hyprland's export (R8).
  - Autostart children get it from `child_env` (above).
  - Already-running apps keep their environment. The page says so, and removals linger (R9).
    A profile switch is the same kind of change (Task 13).

### Text and safety (§9, §10)

- **`user.lua` is never written.** uqd reads it for `Explain` only.
- **Plain words (§9).** Every user-facing string passes `scripts/lint-settings.sh`.
  - **Ruling:** add `xdg`, `environment.d`, `ozone`, `force_zero_scaling`, `systemd`,
    `systemd-run`, `desktop file`, `hl.env` and `exec` to `scripts/settings-jargon.txt`.
  - Variable names (`ELECTRON_OZONE_PLATFORM_HINT` and others) and raw file paths appear only
    in the `AdvancedDetails` disclosure (`components/AdvancedDetails.qml`, already exempt) or as
    data the user typed.
  - Labels:
    - "Apps that start when you log in" (not "autostart");
    - "Older X11 apps" for XWayland;
    - "Also when started from other launchers" for `elsewhere`;
    - "Run on: Automatic / Wayland / X11";
    - "Size" for scale;
    - "Extra settings for this app" for env;
    - "Extra options" for args.
  - **Rust-side text isn't linted.** The tests `refusals_are_plain` (Tasks 2–4) and
    `summaries_are_plain` (Tasks 2–4) cover it.
  - **Refusals shown on a page (F21).** The pages pre-check each write with the uq-core
    validator and show the `SettingError::Invalid` `reason` only (as `pages/rules.rs::reason`
    does). For uqd's own refusals, `pages::runner::plain_refusal` drops a leading
    `invalid value for <key>: `.
- **Untrusted text.** These come from other packages, other apps or the user, and from
  `state.toml`, a restore point or a profile:
  - desktop-file `Name`s;
  - window titles and classes;
  - journal lines;
  - env names and values;
  - args and commands;
  - profile names.

  Desktop-file names and journal lines go through `desktop_entry::clean_display` (control and
  bidi format characters, U+202A–U+202E and U+2066–U+2069, become spaces; F36). All of it is
  shown through `UqLabel`, never a stock control's `text:`, a `Kirigami.InlineMessage`, a
  `ToolTip`, a `Kirigami.FormData.label` or a `Dialog.title` (slice C's F28).
  - Dialog titles are fixed literals: "Fix this app", "Add an app", "Add a command" and
    "Add a variable".
  - Messages use the existing plain-text `components/RuleMessage.qml`. **Ruling:** it's
    reused as is; it holds generic lines.
- **Values aren't secrets, but they're kept quiet.** Summaries and logs never print env values;
  `dbus.rs` logs only an event's kind when a signal fails to send (F34). `ListAutostart` returns
  the argv of added commands only to the page (the session bus, same user). The journal
  (`~/.local/state/uq/history`) and profile files store values just as `state.toml` stores them;
  the Profiles page says so when it exports a profile holding variables (Task 13).

## Review Focus

1. **Injection into `generated.lua`, `environment.d` or `Exec`.**
   - An env value can hold `"`, `\`, `'`, `]]`, or `") hl.exec_cmd("x`.
   - An arg or command can hold quotes, `%`, `;` or spaces.
   - A system `Exec` can hold `\n`, `\t` escapes or single quotes.

   Every Lua string goes through `lua_string`. Env names are `[A-Z_][A-Z0-9_]*`, so Hyprland's
   `import-environment '<name>'` can't be broken out of. The `environment.d` quoting
   round-trips. The `Exec` writer round-trips through `exec_argv` for every arg and never writes
   a raw newline (F11); a lossy source isn't rewritten (F12).
   Tests: Task 5's `hostile_env_stays_inside_lua_strings`, Task 2's `environment_d_round_trips`
   and `refuses_hostile_vars`, Task 1's `write_exec_round_trips` and
   `write_exec_escapes_control_chars`, and Task 3's `override_text_refuses_lossy_exec`.
2. **Touching files uQ didn't make.** These cases must hold:
   - an unmarked desktop file at the override path;
   - a symlink there;
   - an unmarked Chromium flags file;
   - an unmarked user autostart file;
   - a marked file replaced by an unmarked one between check and delete;
   - a file appearing, or a marked one replaced, between check and write (F13).

   In every case the file is never changed or deleted. Task 7's `never_touches_unmarked_*`,
   `never_deletes_file_swapped_after_check` and `never_replaces_file_swapped_after_check` cover
   them. No test touches the real home (F1).
3. **Path traversal.** Ids like `../x`, `..desktop`, `.hidden.desktop`, `a/b.desktop` and
   `x.desktop\0` are refused by `valid_desktop_id` before any path is built. Every written path
   is re-checked to have the folder as its parent and the id as its file name. Task 1's
   `desktop_ids_refuse_traversal` and Task 7's `override_path_stays_in_folder` test it; Task 13's
   tree checks refuse the same ids in profiles.
4. **Autostart runs only what it should, once.** These cases must hold:
   - an entry `OnlyShowIn=GNOME;`;
   - a `TryExec` that's missing;
   - `Hidden=true`;
   - `Terminal=true`;
   - an `Exec` with `%d`;
   - an entry off in state;
   - a restarted uqd in the same session, including an entry turned on since login (F15);
   - a new session while the old pid still lives;
   - a dev session;
   - a profile that turns an entry on (it waits for the next login).

   None of these starts. A good entry starts once, with the delay, in its own
   `app.slice` scope, without a shell. Task 4's `scan_*`, Task 8's `runner_*` and Task 13's
   `profile_autostart_never_starts_or_stops` test it.
5. **Blocked env names and launch safety.** `XDG_SESSION_TYPE`, `HYPRLAND_FOO`, `UQ_DEV_SESSION`,
   `XCURSOR_SIZE`, `XDG_CONFIG_HOME`, a lowercase name and a name with `=` are refused, in
   `env`, in per-app fixes and in profiles alike. `LD_PRELOAD` is warned, not refused. A fix
   whose program's argv[0] contains `=` launches without the fix rather than as an assignment.
   Tasks 2, 3, 9 and 13 test it.
6. **The launch path stays reliable.** With uqd down, slow (over 500 ms) or answering garbage,
   `Launch` still launches, without the fix (a marked override's own `Exec` carries it). A
   marked override never gets the fix twice (R19). Task 9 tests `launch_*`.
7. **Profiles can't add commands (D1, §10).** A profile with `[autostart."a-1234"] argv = […]`,
   `[compat.apps."x.desktop"] args = […]`, `env = …` or `elsewhere = true` is refused on import;
   an unknown startup id is skipped with a notice and kept. Task 13 tests it.

---

### Task 1: `uq_core::desktop_entry`: one parser for `Exec`, entries and ids

**Files:**
- Create: `crates/uq-core/src/desktop_entry.rs` and `crates/uq-core/src/desktop_entry/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs` (`pub mod desktop_entry;`);
  - `crates/uqctl/src/bridge/desktop_entry.rs` (it becomes
    `pub use uq_core::desktop_entry::*;` plus the bridge-only `Dirs`, `binary_exists`,
    `LaunchEntry` and its existing tests);
  - `crates/uqctl/src/bridge/launch.rs` (`scope_command` calls
    `uq_core::desktop_entry::unit_id`; add `--slice=app.slice`, R22; `random_suffix` moves out
    and is re-exported; `parse_entry` gives a `LaunchEntry`);
  - `crates/uqctl/src/bridge/launch_tests.rs` and `crates/uqctl/src/bridge/run.rs`'s test
    (expected strings gain `--slice=app.slice`);
  - `scripts/smoke-shell.sh` (nothing to change: it matches `app-uq-sleep-*.scope`; confirm).

**Interfaces (produced, `uq_core::desktop_entry`):**
- `pub enum Token { Arg(String), Field(char) }`.
- `pub struct Tokens { pub tokens: Vec<Token>, pub lossy: bool }`.
- `pub fn exec_tokens(exec: &str) -> Result<Tokens, ExecError>`.
  - It uses today's quoting rules (`"`, and `\` escaping `" ` $ \` inside quotes) and `%%`.
  - A field code that's a whole argument becomes `Field(c)`; so does a quoted argument that
    holds only a field code (`"%f"`, F24).
  - A field code inside a word (`--f=%F`) keeps today's behaviour (dropped from the word) and
    sets `lossy`. An unquoted `'` sets `lossy` (F12).
  - Deprecated or unknown codes are errors (spec deviation 8).
- `pub struct FieldCtx<'a> { pub icon: Option<&'a str>, pub name: Option<&'a str>, pub path: Option<&'a str> }`.
- `pub fn exec_argv(exec: &str, ctx: &FieldCtx) -> Result<Vec<String>, ExecError>`:
  - `%f %F %u %U` are dropped;
  - `%i` becomes `["--icon", icon]` when there's an icon;
  - `%c` becomes `name`;
  - `%k` becomes `path`.
  - `parse_exec(exec)` stays as `exec_argv(exec, &FieldCtx::default())` for the bridge's
    callers.
- `pub fn split_command(cmd: &str) -> Result<Vec<String>, ExecError>`: the same quoting with
  `%` literal, for user-typed commands (R16).
- `pub fn write_exec(tokens: &[Token]) -> String`.
  - An arg holding a space, tab, `"`, `'`, `\`, `>`, `<`, `~`, `|`, `&`, `;`, `$`, `*`, `?`,
    `#`, `(`, `)` or `` ` ``, or an empty arg, is double-quoted, with `"`, `` ` ``, `$` and `\`
    backslash-escaped.
  - `%` becomes `%%`.
  - `Field(c)` becomes `%c`.
  - Then the key-value escape is applied: `\` → `\\`, newline → `\n`, tab → `\t`, CR → `\r`
    (F11), so the value is always one line.
- `pub fn quote_command(argv: &[String]) -> String`: the same quoting as `write_exec` without
  `%` doubling and without the key-value escape, for showing a command; it round-trips through
  `split_command` (F22).
- `pub fn clean_display(s: &str, max: usize) -> String`: control characters and the bidi
  format characters U+202A–U+202E, U+2066–U+2069 become spaces; cut to `max` chars (F36).
- `pub struct Entry { pub kind: Option<String>, pub name: Option<String>, pub icon: Option<String>, pub exec: Option<String>, pub try_exec: Option<String>, pub terminal: bool, pub terminal_arg: Option<String>, pub hidden: bool, pub only_show_in: Option<Vec<String>>, pub not_show_in: Option<Vec<String>>, pub dbus_activatable: bool, pub path: Option<String>, pub wm_class: Option<String>, pub gnome_enabled: Option<bool>, pub gnome_delay: Option<u32>, pub uq_override: bool, pub uq_source: Option<String>, pub uq_autostart: bool }`.
  - `pub fn read_entry(text: &str) -> Entry` reads only `[Desktop Entry]`. Lists are
    `;`-separated with the trailing `;` optional, and values are unescaped.
- `pub fn shows_in(e: &Entry, desktops: &[&str]) -> bool`, following the Desktop Entry spec:
  `OnlyShowIn` needs an overlap, and `NotShowIn` refuses one.
- `pub const UQ_DESKTOPS: [&str; 2] = ["Hyprland", "uQ"]`.
- `pub fn valid_desktop_id(id: &str) -> bool`: `valid_pinned_id`, ends in `.desktop` after
  normalising, doesn't start with `.`, doesn't contain `..`.
  `pub fn normalise_id(id: &str) -> String` adds `.desktop`.
- `pub fn unit_id(app_id: &str) -> String`: today's sanitiser from `scope_command`.
- `pub fn random_suffix() -> u32`: moved from the bridge (F27); the bridge re-exports it.
- `pub fn set_key(text: &str, group: &str, key: &str, value: &str) -> String`: replaces or
  appends one key in a group, keeping every other line byte for byte. The override and
  hidden-copy writers use it.
- `pub fn groups_with_exec(text: &str) -> Vec<String>`: `Desktop Entry` and each
  `Desktop Action *` group that has an `Exec`.
- Bridge-only (F26): `pub(super) struct LaunchEntry { exec: String, terminal: bool, try_exec: Option<String>, terminal_arg: Option<String> }`
  and `parse_entry(text) -> Result<LaunchEntry, String>` built on `read_entry`, keeping today's
  contract (it refuses hidden, non-Application and `Exec`-less entries), so `launch.rs` changes
  only its imports.

- [ ] **Step 1: Write the failing tests** in `desktop_entry/tests.rs`:
  - `exec_tokens_keep_field_codes` (`code %F` → `[Arg(code), Field('F')]`, not lossy;
    `app "%f"` → `[Arg(app), Field('f')]`, F24);
  - `exec_tokens_mark_lossy` (`app --f=%F`, `sh -c 'a b'` and `app "x%fy"` are lossy);
  - `exec_argv_expands_icon_name_and_path`: `app %i %c %k` with icon `x`, name `My App`, path
    `/p` gives `["app", "--icon", "x", "My App", "/p"]`, and with no icon `%i` gives nothing.
  - `exec_argv_matches_parse_exec_cases` (the three existing bridge cases);
  - `split_command_keeps_percent` (`"echo 100%"` → `["echo", "100%"]`);
  - `write_exec_round_trips`: for a fixed list of hostile args, `exec_argv(unescape(write_exec(tokens)))`
    gives them back. The args are `a b`, `"`, `\`, `$HOME`, `` `x` ``, `50%`, `''`, `;rm -rf`,
    `é`, `""` and `--x=%U`.
  - `write_exec_escapes_control_chars` (F11): `a\nb` and `x\tY=1` give a one-line value that
    round-trips.
  - `quote_command_round_trips` (F22): `split_command(quote_command(argv))` is `argv` for
    `["backup-check", "--quiet", "a b", "100%", "c\\d"]`, and the text shows `100%`, not `100%%`.
  - `clean_display_strips_controls_and_bidi` (F36).
  - `read_entry_reads_lists_and_gnome_keys` (`OnlyShowIn=GNOME;KDE;`, `X-GNOME-Autostart-Delay=5`,
    `X-GNOME-Autostart-enabled=false`, `X-uQ-Override=true`, `StartupWMClass=Code`);
  - `shows_in_rules`: no keys → shown; `OnlyShowIn=GNOME;` → hidden; `OnlyShowIn=Hyprland;` →
    shown; `NotShowIn=uQ;` → hidden; both keys → `OnlyShowIn` decides first.
  - `desktop_ids_refuse_traversal` (Review Focus 3): `../x.desktop`, `..desktop`,
    `.hidden.desktop`, `a/b.desktop`, `x.desktop\0`, `""`, and a 129-char id are all refused;
    `org.mozilla.firefox.desktop` and `code` (normalised) are accepted.
  - `set_key_replaces_only_that_key` (comments, other groups and localised keys stay byte for
    byte);
  - `unit_id_matches_old_scope_names` (the bridge's existing expectations).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core desktop_entry -- --test-threads=2`. Expected:
  FAIL (module missing).
- [ ] **Step 3:** Implement. Move the code. Don't copy it.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -p uqctl -- --test-threads=2` and
  `cargo clippy -j 2 -p uq-core -p uqctl --all-targets -- -D warnings`. Expected: PASS, with the
  launch tests passing once their expected strings have `--slice=app.slice`.
- [ ] **Step 5:** Commit (K5 trailers): `refactor(core): one Desktop Entry parser with field codes, an Exec writer and safe ids`

### Task 2: `uq_core::env`: the environment variables domain

**Files:**
- Create: `crates/uq-core/src/env.rs` and `crates/uq-core/src/env/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/state.rs` (`pub env: Env`, lenient load, `section`/`set_section`
    dispatch for `"env"`, `parse_strict` calls `Env::repaired`; the full `State {}` literal in
    `state_round_trip` gains `env`, F41);
  - `crates/uq-core/src/settings/mod.rs` (`SECTION_DOMAINS` gains `"env"`;
    `renders_section("env")` is true);
  - `crates/uq-core/src/history/labels.rs` (the `("env", "vars")` label, slice C's F11);
  - `crates/uq-core/src/history/diff.rs` (`summarize` hands `env` to `env::summary`, F30);
  - `apps/uq-settings/src/registry.rs` (`every_journal_domain_has_one_page` skips `env` until
    Task 12, as slice C's F11 did).

**Interfaces (produced):**
- `pub struct Var { pub name: String, pub value: String }`.
- `pub struct Env { pub vars: Vec<Var> }`, with `Default`, `Serialize`, and a lenient
  `Deserialize`.
- `pub const MAX_VARS: usize = 100;`, `pub const MAX_VALUE_BYTES: usize = 4096;`.
- `pub enum Warn { BreaksApps }`.
- `pub fn check_var(key: &str, name: &str, value: &str) -> Result<Option<Warn>, SettingError>`.
  `key` is the field key for messages (`env.vars[3]`, `compat.apps[1].env[0]` or
  `env.<NAME>` in a profile).
- `pub fn check_name(key: &str, name: &str) -> Result<Option<Warn>, SettingError>` (the name
  rule and the blocklist; `check_var` calls it; profiles use it for removals).
- `pub fn blocked(name: &str) -> Option<&'static str>` returns the plain reason.
- `impl Env { pub fn from_section(v: &Value) -> Result<Env, SettingError>; pub fn repaired(self) -> Env; pub fn section(&self) -> Value; pub fn warnings(&self) -> Vec<(String, Warn)>; pub fn get(&self, name: &str) -> Option<&str> }`.
- `pub fn environment_d(lines: &[(String, String)]) -> String`: the header and the quoting from
  Global Constraints.
- `pub fn summary(before: &Value, after: &Value) -> String` (F30).

- [ ] **Step 1: Write the failing tests** in `env/tests.rs`:
  - `names_follow_the_rule` (`A`, `_X`, `GDK_SCALE2` pass; `a`, `1A`, `A-B`, `A=B`, `""`, and
    129 chars are refused);
  - `blocked_names_refused` (every spec name plus each ruling addition, including
    `XDG_SESSION_TYPE`, `HYPRLAND_X`, `XDG_CONFIG_HOME`, `XDG_CACHE_HOME`, `XDG_SEAT`, with the
    right plain reason; F17);
  - `warned_names_allowed` (`LD_PRELOAD`, `LD_LIBRARY_PATH`, `LD_AUDIT`, `PATH`,
    `XDG_DATA_DIRS`, `XDG_CONFIG_DIRS` → `Ok(Some(BreaksApps))`);
  - `values_checked` (4096 bytes pass and 4097 are refused; `\n`, `\0`, `\t`, `\u{7f}` and `$`
    are refused; `""` passes);
  - `hostile_values_are_data` (Review Focus 1: the value `") hl.exec_cmd("x` is *accepted*;
    it's only data, and Task 5 proves it stays inside the string);
  - `duplicates_and_limit_refused` (the 101st variable);
  - `lenient_load_keeps_good_entries` (one bad entry among three loads two);
  - `section_round_trips` (§9: `from_section(section())` is identity, and through `state.toml`);
  - `environment_d_round_trips`. A reference parser in the test follows systemd's `env-file.c`
    single- and double-quote rules. Values `a b`, `it's`, `"q"`, `` `x` ``, `back\slash`, `#x`
    and `é` all round-trip.
  - `refusals_are_plain` (no message contains `regex`, `[A-Z`, or `state.toml`).
  - In `history/diff.rs` tests: `env_summary_names_never_values` (a value `s3cret` never
    appears; added, changed, removed; "and N more") and `env_summaries_are_plain` (no `{`, `[`
    or `env.`).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core env -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `state.rs` growth to the field, the dispatch arms and the
  repair call.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and clippy on uq-core.
  Expected: PASS, including `every_key_a_summary_can_show_has_a_plain_label`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): environment variables as a validated uQ domain`

### Task 3: `uq_core::compat`: switches, per-app fixes, the name table and the launch argv

**Files:**
- Create:
  - `crates/uq-core/src/compat/mod.rs` (the model, validation and `summary`);
  - `compat/hypr.rs` (the name table, with the header citing both tags and files);
  - `compat/browsers.rs`;
  - `compat/launch.rs` (`argv`, `override_text`, `main_scale`, `gdk_scale`, `fmt_scale`,
    `switch_env`);
  - `compat/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/state.rs` (`pub compat: Compat`, as in Task 2, including the
    `State {}` literal, F41);
  - `crates/uq-core/src/settings/mod.rs` (`SECTION_DOMAINS` gains `"compat"`;
    `renders_section("compat")` is true);
  - `crates/uq-core/src/settings/provenance.rs` (the `compat.sharp_x11` row, setting id
    `sharp-x11`, and `expected` for it; the OPTIONS test's populated state sets compat);
  - `crates/uq-core/src/history/labels.rs` (the compat labels);
  - `crates/uq-core/src/history/diff.rs` (`summarize` hands `compat` to `compat::summary`, F30);
  - `apps/uq-settings/src/registry.rs`: skip `compat` in `every_journal_domain_has_one_page`,
    and skip the `compat.sharp_x11` row in `provenance_rows_name_registry_settings` (F7), both
    until Task 12.

**Interfaces (produced):**
- `compat::hypr`:
  - `pub const XWAYLAND_ZERO_SCALING: &str = "xwayland:force_zero_scaling";`
  - `pub enum SwitchKey { SharpX11, ElectronWayland, ChromiumWayland, QtWayland, SteamScale }`
    with `key()`, `label()`, `description()`, `ALL` and `from_key(&str)`.
  - `pub enum Value { Fixed(&'static str), MainScaleInt, MainScale }`, with
    `scale_derived(&self) -> bool` (F18).
  - `pub const SWITCH_ENV: [(SwitchKey, &str, Value); 4]` holds
    `(SharpX11, "GDK_SCALE", MainScaleInt)`, `(ElectronWayland, "ELECTRON_OZONE_PLATFORM_HINT", Fixed("auto"))`,
    `(QtWayland, "QT_QPA_PLATFORM", Fixed("wayland;xcb"))` and
    `(SteamScale, "STEAM_FORCE_DESKTOPUI_SCALING", MainScale)`.
  - `pub const PLATFORM_ENV: [(&str, [(&str, &str); 4]); 2]` (`wayland`, `x11`), and
    `pub const SCALE_ENV` (`GDK_SCALE`, `QT_SCALE_FACTOR`), `STEAM_ID = "steam.desktop"`.
- `compat::browsers`: `pub struct Browser { pub name: &'static str, pub desktop_ids: &'static [&'static str], pub flags_file: &'static str }`,
  `pub const BROWSERS: [Browser; 5]`, `pub const FLAGS_MARKER: &str`,
  `pub const FLAGS_LINE: &str = "--ozone-platform-hint=auto"`, `pub fn browser_for(id) -> Option<&Browser>`,
  `pub fn flags_text() -> String`.
- `compat::mod`:
  - `pub enum Platform { Auto, Wayland, X11 }` (serialised lowercase);
  - `pub struct AppFix { pub id, pub platform, pub scale: Option<f64>, pub env: Vec<env::Var>, pub args: Vec<String>, pub elsewhere: bool }`
    with `is_empty()` (nothing set);
  - `pub struct Compat { pub sharp_x11: bool, pub electron_wayland: bool, pub chromium_wayland: bool, pub qt_wayland: bool, pub steam_scale: bool, pub apps: Vec<AppFix> }`,
    with `from_section`, `repaired`, `section`, `switch(SwitchKey) -> bool`,
    `set_switch(SwitchKey, bool)` and `fix_for(&self, id) -> Option<&AppFix>` (by normalised id);
  - `pub fn valid_scale(s: f64) -> bool` (1.0–3.0 in 0.25 steps);
  - `pub const MAX_APPS: usize = 100`, `MAX_APP_ENV = 20`, `MAX_ARGS = 20`, `MAX_ARG_CHARS = 1024`;
  - `pub fn summary(before: &Value, after: &Value) -> String` (F30).
- `compat::launch`:
  - `pub fn main_scale(live: &[LiveOutput]) -> Option<f64>` (R13; `LiveOutput` and
    `parse_live` are `uq_core::settings`'s);
  - `pub fn gdk_scale(s: f64) -> u32` (R14);
  - `pub fn fmt_scale(s: f64) -> String`: at most 2 places, no trailing zeros (`1.25`, `2`);
  - `pub fn switch_env(c: &Compat, main: Option<f64>) -> Vec<(String, String)>`: lines whose
    value needs a scale are left out when `main` is `None`;
  - `pub fn argv(fix: Option<&AppFix>, id: &str, exec: Vec<String>) -> Result<Vec<String>, CompatError>`;
  - `pub fn override_text(source: &str, source_path: &str, fix: &AppFix, id: &str) -> Result<Option<String>, CompatError>`;
  - `pub enum CompatError { Unsafe(String), BadExec(String) }`, with plain messages.

- [ ] **Step 1: Write the failing tests** in `compat/tests.rs`:
  - `switches_and_apps_round_trip` (§9 round trip, and through `state.toml`);
  - `app_ids_normalised_and_checked` (`code` → `code.desktop`; `../x` refused; a duplicate id is
    refused);
  - `app_env_uses_env_rules` (`XDG_SESSION_ID` is refused with the env reason, keyed
    `compat.apps[0].env[0]`);
  - `empty_fix_refused`, `scale_steps_checked` (1.1 is refused; 1.25 and 3.0 pass);
  - `gdk_scale_rounds_half_down` (1.0→1, 1.25→1, 1.5→1, 1.75→2, 2.0→2, 2.5→2, 3.0→3);
  - `main_scale_picks_origin_output` (a 0,0 output at 1.5 beside a 1.0 one gives 1.5; with no
    output at 0,0 it's the first by name; a disabled output is skipped; an empty list gives
    `None`);
  - `switch_env_follows_table` (each switch alone gives its one line; with `main = None`,
    `GDK_SCALE` and `STEAM_…` are absent);
  - `launch_argv_wayland_with_env_and_args`: for
    `code.desktop` + `{wayland, env FOO=1, args ["--x"]}` with exec `["code"]`, the result is
    `["env", "--", "GDK_BACKEND=wayland,x11", "QT_QPA_PLATFORM=wayland;xcb", "SDL_VIDEODRIVER=wayland,x11", "ELECTRON_OZONE_PLATFORM_HINT=wayland", "FOO=1", "code", "--x"]`.
  - `launch_argv_user_env_wins` (a user `GDK_BACKEND=x11` with platform wayland: both appear,
    and the user's comes later);
  - `launch_argv_browser_gets_flag` (`chromium.desktop` + x11 + scale 1.5 adds
    `--ozone-platform=x11 --force-device-scale-factor=1.5`);
  - `launch_argv_steam_scale`;
  - `launch_argv_no_fix_is_unchanged`;
  - `launch_argv_args_only_has_no_wrapper` (F25: `{args ["--x"]}` gives `["code", "--x"]`);
  - `launch_argv_refuses_equals_program` (Review Focus 5);
  - `override_text_wraps_every_exec`. The fixture has `[Desktop Entry]` with `Exec=code %F`,
    `DBusActivatable=true` and `[Desktop Action new-window]` with `Exec=code --new-window %F`.
    Both Execs become `env -- "GDK_BACKEND=…" … code <args> %F`; `DBusActivatable=false`; the
    marker and `X-uQ-Source` are present; the localised `Name[de]` line is unchanged; and the
    result parses back with `read_entry` to `uq_override == true`.
  - `override_text_inserts_args_before_field_code`;
  - `override_text_refuses_lossy_exec` (F12: `Exec=sh -c 'a b'` and `Exec=app --f=%F` give
    `BadExec`);
  - `override_text_never_writes_a_newline` (F11: a source `Exec=app "a\nb"` gives a file with
    no extra line);
  - `browsers_table_unique` and `labels_are_plain` (no label holds `xwayland`, `env`, `Qt_`,
    `ozone` or `hl.`);
  - `refusals_are_plain`.
  - In `provenance.rs`: `compat_sharp_x11_row_expected` (`expected` is `true` when on, `None`
    when off; F6).
  - In `history/diff.rs` tests: `compat_summary` (switches, and a fix added, changed or
    removed) and `compat_summaries_are_plain` (no `{`, `[`, `.desktop` or `compat.`).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core compat -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Cite the source file, tag and line above each table.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`, clippy on uq-core, and
  (t7check) the `registry` tests. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): Apps and scaling switches and per-app fixes, checked against 0.56.2 and 0.57.0`

### Task 4: `uq_core::autostart`: the domain, the folder scan and the run plan

**Files:**
- Create:
  - `crates/uq-core/src/autostart/mod.rs` (the model, validation and `summary`);
  - `autostart/scan.rs` (the folder read and the pure resolve over in-memory listings);
  - `autostart/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/state.rs` (`pub autostart: Autostart`, including the `State {}`
    literal, F41);
  - `crates/uq-core/src/settings/mod.rs` (`SECTION_DOMAINS` gains `"autostart"`;
    `renders_section("autostart")` is false);
  - `crates/uq-core/src/rules/mod.rs` (`fresh_id` gains a sibling
    `fresh_prefixed(prefix, taken)`; `fresh_id` calls it with `"r-"`);
  - `crates/uq-core/src/paths.rs` (F1): `pub struct StartupPaths { config_home, config_dirs, data_home, data_dirs, runtime_dir, home }`
    with `from_vars(get)` and `under(root)`; `Paths` gains `pub startup: StartupPaths`, filled by
    `Paths::from_vars` from the same lookup and by `Paths::under` under the test root
    (`config/`, `xdg/`, `data/`, `share-dirs/`, `run/`, `home/`), so every existing test rig is
    sandboxed without edits;
  - `crates/uq-core/src/history/labels.rs` (the autostart labels);
  - `crates/uq-core/src/history/diff.rs` (`summarize` hands `autostart` to
    `autostart::summary`, F30);
  - `apps/uq-settings/src/registry.rs` (skip `autostart` until Task 12).

**Interfaces (produced):**
- `pub struct FileChoice { pub id: String, pub enabled: Option<bool>, pub delay: Option<u32> }`.
- `pub enum Added { App { id, name, app, enabled, delay }, Command { id, name, argv, enabled, delay } }`
  (serialised with exactly one of `app` or `argv`), with `id()`, `name()`, `enabled()`,
  `delay()` and setters for `enabled`/`delay`.
- `pub struct Autostart { pub files: Vec<FileChoice>, pub added: Vec<Added> }` with
  `from_section(v, current)`, `repaired`, `section`, `choice(id)` and
  `prune_empty_choices(&mut self)` (drops `files` entries with neither field set).
- `pub fn valid_added_id(id: &str) -> bool` (`^a-[0-9a-f]{4,8}$`).
- `pub fn summary(before: &Value, after: &Value) -> String` (F30).
- `scan.rs`:
  - `pub struct Found { pub id: String, pub user: Option<(PathBuf, String)>, pub system: Option<(PathBuf, String)> }`;
    the texts are already read and capped.
  - `pub fn read_capped(path: &Path) -> Option<String>`: F14's read (non-blocking open, regular
    file once opened, `take(64 KiB)`, lossy UTF-8). The folder read, the added-app lookup and
    uqd's override source reads all use it.
  - `pub fn read_folders(user: &Path, system: &[PathBuf]) -> Vec<Found>`, the one function that
    lists folders here.
  - `pub enum Source { System, Yours, Added }`.
  - `pub enum Plan { Run { argv: Vec<String>, cwd: Option<PathBuf>, delay: u32 }, Off, Skip(&'static str) }`.
  - `pub struct Resolved { pub id, pub name, pub icon, pub source, pub enabled: bool, pub can_toggle: bool, pub delay: u32, pub plan: Plan, pub orphan_copy: Option<PathBuf> }`.
    `name` and `icon` pass through `clean_display` (F36).
  - `pub fn resolve(found: &[Found], state: &Autostart, compat: &Compat, apps: &dyn Fn(&str) -> Option<(PathBuf, String)>, exists: &dyn Fn(&str) -> bool) -> Vec<Resolved>`.
    It's pure. `apps` finds an added app's desktop file, and `exists` answers `TryExec`. The
    `argv` already went through `compat::launch::argv`.
  - `pub fn known_ids(found: &[Found], state: &Autostart) -> BTreeSet<String>`: every found id
    plus every added id (Task 13 uses it).
  - `pub fn hidden_copy(system_text: &str) -> String`, and
    `pub fn wanted_copies(found, state) -> (Vec<(String, String)>, Vec<PathBuf>)`, which gives
    the copies to write and the marked copies to delete.

- [ ] **Step 1: Write the failing tests** in `autostart/tests.rs`:
  - `section_round_trips_and_ids_assigned` (an `added` entry without an id gets `a-xxxx`;
    round trip through `state.toml`);
  - `added_needs_app_or_argv_not_both`, `argv_checked` (the first arg can't contain `=`; 65
    args are refused), `delay_range`, `name_refuses_control_chars` (F42);
  - `scan_user_file_shadows_system`;
  - `scan_marked_copy_is_ignored_and_orphan_reported`;
  - `scan_skips_with_plain_reasons` (Review Focus 4): `Hidden`, `OnlyShowIn=GNOME;`, missing
    `TryExec`, `Terminal=true`, `Exec=app %d`, `Exec=sh -c 'x y'` (F12) and `Type=Link` each
    give their reason and `can_toggle: false`;
  - `scan_enabled_and_delay_precedence` (`X-GNOME-Autostart-enabled=false` is off unless state
    says on; the state delay beats `X-GNOME-Autostart-Delay=700`, which clamps to 600);
  - `scan_applies_compat_fix` (a fix for `foo.desktop` wraps the argv);
  - `scan_expands_field_codes` (`%k` → the file path);
  - `hidden_copy_sets_hidden_and_marker` (an existing `Hidden=false` is replaced, other lines are
    kept, and `read_entry` gives `hidden && uq_autostart`);
  - `wanted_copies_rules`: a system entry off with no user file gives one copy; a user file
    exists gives none; on again deletes the marked copy; an unmarked file is never listed for
    deletion.
  - `read_folders_caps_and_filters` (a temp dir with a 70 KiB file, `bad name.desktop`, a
    symlink to a file, a FIFO named `fifo.desktop` (F14), and 205 good files gives exactly 200
    readable entries; the bad name, the big file and the FIFO are skipped, and the test ends
    within a second);
  - `paths_under_sandboxes_startup` (in `paths.rs`: every `StartupPaths` field of
    `Paths::under(root)` is inside `root`; F1);
  - `refusals_are_plain`.
  - In `history/diff.rs` tests: `autostart_summary` (off, on, delay, added or removed, by name)
    and `autostart_summaries_are_plain`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core autostart -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and clippy on uq-core.
  Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): autostart entries from the XDG folders and your own, as a uQ domain`

### Task 5: Render the xwayland option and `hl.env` lines; the session env lists

**Files:**
- Create: `crates/uq-core/src/lua/startup.rs` (with its tests).
- Modify:
  - `crates/uq-core/src/lua/mod.rs`. `emit_generated` gains a last parameter
    `startup: &StartupRender`. The xwayland table goes inside `hl.config({…})`, after the input
    tables. The env block goes after the cursor `hl.env` lines and before the workspace rules.
  - Every caller passes `&StartupRender::default()` for now: `crates/uqd/src/service.rs`
    (`render_candidate`), `lua/look.rs`, `lua/input.rs`, `lua/mod.rs` and `lua/rules_tests.rs`
    tests, `crates/uq-core/tests/generated_golden.rs`, `crates/uqd/tests/verify_config.rs`.
  - The golden files under `crates/uq-core/tests/golden/generated/`: add `startup-all.lua`; the
    existing ones stay byte for byte.
  - `crates/uqd/tests/verify_config.rs` (a CI `--ignored` case that verifies a config with
    every switch on and three env vars on the real Hyprland).
  - `crates/uq-core/src/env.rs` (`session_env` and `environment_d_lines`, F19).

**Interfaces (produced):**
- `pub struct StartupRender { pub zero_scaling: bool, pub env: Vec<(String, String)>, pub export: bool }`,
  with `Default` (all off, empty, `export: true`).
- `pub fn startup_render(state: &State, main: Option<f64>, dev_session: bool) -> StartupRender`.
  The env is `env::session_env(&state.env, &state.compat, main)`, and `export` is
  `!dev_session`.
- `pub fn emit_startup_env(out: &mut String, r: &StartupRender)` writes nothing when `r.env` is
  empty; else `-- Environment (Startup and Apps and scaling pages)` and then one
  `hl.env(<lua_string name>, <lua_string value>, true|false)` per line (F33).
- `pub fn emit_xwayland(out: &mut String, r: &StartupRender)` writes
  `    xwayland = {\n        force_zero_scaling = true,\n    },` only when it's on.
- In `env.rs` (F19):
  - `pub fn session_env(env: &Env, compat: &Compat, main: Option<f64>) -> Vec<(String, String)>`:
    env vars in order, then `switch_env`, a switch's name replacing an env one (R11). No cursor.
  - `pub fn environment_d_lines(env: &Env, compat: &Compat) -> Vec<(String, String)>`: the same
    merge with the scale-derived switch lines left out (F18).
- uqd and the autostart runner use these, so the lists can't drift: `generated.lua`,
  `environment.d` and the autostart children's environment (`Desktop::child_env`, Task 7).

- [ ] **Step 1: Write the failing tests:**
  - in `lua/startup.rs`:
    - `xwayland_only_when_on`;
    - `env_lines_in_order_compat_last`;
    - `export_false_in_dev_session`;
    - `no_env_block_when_empty` (F33);
    - `hostile_env_stays_inside_lua_strings` (Review Focus 1). The value
      `") hl.exec_cmd("x` and `]]--` give exactly one `hl.env(` per variable. The file loaded
      under the Lua mock of slice C's F19 calls `hl.env` with the literal value and makes no
      other call. The Lua mock runs in CI only (K4); locally assert on the text.
  - in `env.rs`:
    - `session_env_compat_replaces_env` (R11);
    - `session_env_has_no_cursor` (the cursor stays in `emit_generated`'s own block);
    - `environment_d_lines_leave_out_scale_lines` (F18: with `sharp_x11` and `steam_scale` on
      and a main scale, neither `GDK_SCALE` nor `STEAM_FORCE_DESKTOPUI_SCALING` is listed;
      `ELECTRON_OZONE_PLATFORM_HINT` is).
  - `generated_golden`: the existing goldens unchanged, plus the new `startup-all.lua`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core lua -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Bless `startup-all.lua` with the existing `UQ_BLESS=1` flow of
  `generated_golden.rs`, and confirm with `git diff --stat` that no other golden changed.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -p uqd -- --test-threads=2`. Expected: PASS.
  `verify_config`'s new case is `--ignored` (CI with Hyprland).
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): render sharp X11 apps and environment variables into generated.lua`

### Task 6: `Explain` rows and the `user.lua` env finder

Summaries landed with their domains (F30). Profiles are Task 13 (D1); this task adds no profile
tests.

**Files:**
- Create: `crates/uq-core/src/startup_explain.rs` (with its tests).
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/settings/provenance.rs`
    (`pub fn find_env_in_user_lua(text, name) -> Option<u32>`, using `strip_lua_comment`).

**Interfaces (produced):**
- `pub struct FileReport { pub env_file: Option<String>, pub browsers: Vec<(String, BrowserFile)>, pub overrides: BTreeMap<String, OverrideState>, pub autostart: Option<String> }`
  (filled by uqd's sync, Task 7). The fake uqd builds one by hand.
  - `pub enum BrowserFile { Ours, OwnFile, None, Error(String) }`.
  - `pub enum OverrideState { Written, OwnFile, NoSource, Error(String), Off }`.
- `pub fn explain_env(state, user_lua: Option<&str>, rejected: Option<&str>, report: &FileReport, path: &str) -> Value`.
- `pub fn explain_compat_extra(state, user_lua, rejected, report, path) -> Value`: every compat
  row except `sharp_x11`, which the generic option path gives.
- `pub fn default_when_off(rows: &mut Value, state: &State)` (F6): the `sharp_x11` row's source
  `uq` becomes `default` while it's off.
- `pub fn explain_autostart(state, report) -> Value`.

- [ ] **Step 1: Write the failing tests** in `startup_explain.rs`:
  - `env_row_user_lua_line` (`user.lua` line 3 has `hl.env("FOO", "x")` → `user_lua`, line
    3; `-- hl.env("FOO", "x")` doesn't count);
  - `env_row_replaced_by_switch` (R11);
  - `env_rejected_gives_section_row` (slice C's F9);
  - `env_file_error_reported`;
  - `compat_switch_user_lua` (`hl.env("ELECTRON_OZONE_PLATFORM_HINT", …)` in `user.lua`);
  - `compat_browser_details` (one row per installed browser, with its state);
  - `compat_app_override_states` (each `OverrideState` maps to its source and detail);
  - `sharp_x11_off_is_default` (F6: off → `default`; on → `uq`; a `user_lua` or `error` row is
    left alone);
  - `autostart_error_row`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run, with clippy. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): explanations for startup apps, environment variables and app fixes`

### Task 7: uqd: the three sections, the main scale and the file sync

**Files:**
- Create:
  - `crates/uqd/src/files.rs` (the sync: `environment.d`, flag files, overrides, hidden copies);
  - `crates/uqd/src/files/tests.rs`;
  - `crates/uqd/src/startup.rs` (`impl Desktop`: explain hooks, the compat pre-check,
    `child_env`, `sync_files`, `refresh_overrides`, `with_dev_session`);
  - `crates/uqd/src/startup/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`mod files; mod startup;`);
  - `crates/uqd/src/service.rs`, limited to:
    - `Desktop` gains `pub(crate) dev_session: bool` (default `false`),
      `pub(crate) main_scale: Option<f64>` and `pub(crate) files: FileReport`; the startup paths
      come from `self.paths.startup` (F1), so `Desktop::new` keeps its signature;
    - `Candidate` gains `main_scale: Option<f64>`. `render_candidate` (still `&self`, F4)
      computes it only when `sharp_x11` or `steam_scale` is on: the scale of R13's main output
      from `next.settings.monitors` when that config sets it, else from one `comp.monitors()`
      read through `parse_live` and `compat::launch::main_scale`. It passes
      `startup_render(next, main, self.dev_session)` to `emit_generated`;
    - `install` sets `self.main_scale = c.main_scale` before `persist`;
    - `persist` calls `self.sync_files(&old)` after adopting `next` and before the
      `Schedule` early return;
    - `explain` hands `env`, `compat` and `autostart` to `Desktop::explain_startup`, after the
      `rules` hook. For `compat`, it runs the generic option loop for `sharp_x11`, calls
      `default_when_off`, then merges in `explain_compat_extra`'s rows;
    - `domain_label` gains "The list of environment variables", "Apps and scaling" and "The
      list of startup apps" (F44);
    - `startup()` runs a full sync once (its own commit has `old == next`, so the change
      detection in `sync_files` sees nothing).
  - `crates/uqd/src/sections.rs` (`set_section` calls `self.check_compat_files(&next)` before
    `commit_section` when the domain is `compat`);
  - `crates/uqd/src/main.rs`: `.with_dev_session(std::env::var_os("UQ_DEV_SESSION").is_some())`
    on the `Desktop`, and the 60 s tick also calls `refresh_overrides()` (R18, F5). Task 8
    doesn't touch this again.
  - `crates/uqd/tests/dbus.rs` (`startup_sections_over_dbus`).

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces:
  - `pub fn with_dev_session(self, on: bool) -> Self` (F5).
  - `pub(crate) fn child_env(&self) -> Vec<(String, String)>`: the theme's cursor lines, then
    `env::session_env(&state.env, &state.compat, self.main_scale)` (F19).
  - `pub(crate) fn sync_files(&mut self, old: &State)`. It runs each part whose inputs changed:
    `env`, `compat`, `autostart`, or `main_scale` versus the last sync. It records the outcome
    in `self.files`. An I/O error goes to `FileReport` and `tracing::warn!`, and never fails the
    write. The parts are:
    - `files::write_environment_d(&paths, &env::environment_d_lines(..))` (F18, F40);
    - `files::sync_flags(&paths, &compat, installed)`;
    - `files::sync_overrides(&paths, &compat, dev)`: in a dev session it neither writes nor
      deletes (F38);
    - `files::sync_hidden_copies(&paths, &found, &state.autostart)`.
  - `pub fn refresh_overrides(&mut self)`: `sync_overrides` and `sync_flags` again, for R18 and
    F37.
  - `pub(crate) fn check_compat_files(&self, next: &State) -> Result<(), DesktopError>`. The
    refusals from Global Constraints come back as `DesktopError::InvalidSetting` with plain text
    (F3), and only for fixes whose `elsewhere` turns on in this write.
  - The marker check is `files::is_ours(path, MarkerKind) -> bool`. It uses `symlink_metadata`
    (a symlink is never ours) and `read_capped`, and it runs again immediately before each
    write lands and each delete (F13).
  - `files::write_marked(path, text, kind)`: F13's write (temp, re-check, `rename` over a marked
    file or `hard_link` into an absent name). Overrides, flag files and hidden copies all use it.
  - `files::safe_join(dir, id) -> Option<PathBuf>`: `valid_desktop_id`, then the result's
    parent must equal `dir` and its file name must equal the id.

- [ ] **Step 1: Write the failing tests:**
  - In `files/tests.rs` (temp dirs, no uqd):
    - `environment_d_written_and_rewritten` (atomic; no file when there are no lines; an empty
      list removes a file with the header and leaves one without it; F40);
    - `environment_d_has_no_scale_lines` (F18);
    - `flags_written_only_for_installed_browsers`;
    - `flags_for_browser_installed_later` (F37: `refresh` writes it);
    - `never_touches_unmarked_flags_file`;
    - `flags_removed_when_off`;
    - `override_written_with_marker_and_source`;
    - `override_regenerated_when_source_changes` (change the source's `Exec`, then call
      `refresh`);
    - `override_deleted_when_fix_removed`;
    - `override_lossy_source_reported` (F12: `OverrideState::Error`, no file);
    - `never_touches_unmarked_override`;
    - `never_follows_override_symlink` (a symlink to a file outside the folder stays, and its
      target is unchanged);
    - `never_deletes_file_swapped_after_check`: a marked file is replaced by an unmarked one
      before the delete step (driven through a `#[cfg(test)]` hook between check and delete),
      and it survives.
    - `never_replaces_file_swapped_after_check` (F13): the same hook between check and write,
      for both "a marked file swapped" and "a file appeared"; the user's file survives and the
      report says `own_file`.
    - `override_path_stays_in_folder`;
    - `dev_session_override_sync_touches_nothing` (F38: a marked override stays, none is
      written);
    - `hidden_copy_written_and_removed`;
    - `never_touches_unmarked_autostart_file`;
    - `orphan_copy_removed`.
  - In `startup/tests.rs` (`FakeCompositor`, `Paths::under(temp)`):
    - `env_section_renders_hl_env_and_journals` (the verified `generated.lua` has the line, the
      journal entry's origin is `settings`, the summary names the variable, and
      `environment.d` is written);
    - `env_dev_session_does_not_export` (`, false)`);
    - `compat_section_renders_zero_scaling_with_main_scale` (a fake `monitors` reply with
      `DP-1` at 0,0 and scale 1.5 gives `GDK_SCALE` "1");
    - `stored_monitor_scale_wins` (F4: a stored `DP-1` scale of 2 in `next` gives `GDK_SCALE`
      "2" before any live change);
    - `compat_without_hyprland_leaves_scale_lines_out`;
    - `compat_elsewhere_refused_for_own_file`, `…_without_source` and `…_in_dev_session`
      (`InvalidSetting`, F3; `state.toml` is unchanged and no journal entry is added);
    - `undo_compat_removes_override` (undo the entry that added a fix with `elsewhere`: the
      override file is gone);
    - `restore_point_brings_flags_back`;
    - `autostart_section_writes_hidden_copy_and_never_renders` (no `verify` call is recorded);
    - `explain_env_user_lua_and_file_error`;
    - `explain_compat_sharp_x11_runtime` (the fake `getoption` says false while state says
      true gives `runtime`) and `explain_compat_sharp_x11_off_is_default` (F6);
    - `rejected_env_keeps_previous_config` (`verify_error` gives `error` rows, and
      `environment.d` is unchanged);
    - `env_not_in_settings_changed` (R3);
    - `tests_never_touch_the_real_home` (F1: a rig's `paths.startup` lies inside its temp dir,
      and a full `startup()` writes nothing outside it).
  - In `crates/uqd/tests/dbus.rs`: `startup_sections_over_dbus` (Get, Set and Explain for all
    three). The D-Bus proxy test is extended in Task 8, which adds the methods (F27).
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd files startup -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `service.rs` growth to the lines listed. The sync and the
  explain hooks live in `files.rs` and `startup.rs`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2` and clippy on uqd. Expected:
  PASS, including every slice A, B and C test.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): environment variables and app fixes applied, with marked files only`

### Task 8: uqd: the autostart runner, `ListAutostart`, `ShellReady` and `AutostartChanged`

**Files:**
- Create: `crates/uqd/src/autostart.rs`, `crates/uqd/src/autostart/spawn.rs` (the trait and
  the system impl) and `crates/uqd/src/autostart/tests.rs`.
- Modify:
  - `crates/uqd/Cargo.toml` (dev-dependency `tokio = { workspace = true, features = ["test-util"] }`, F10);
  - `crates/uqd/src/lib.rs` (`pub mod autostart;`);
  - `crates/uqd/src/service.rs` (`Event::AutostartChanged`, one line);
  - `crates/uqd/src/startup.rs` (`Desktop::autostart_plan(&self) -> (Vec<Resolved>, Vec<(String, String)>)`,
    which reads the folders with `paths.startup` and calls `autostart::scan::resolve`, and
    returns `child_env()`);
  - `crates/uqd/src/dbus.rs`:
    - the runner lives on `DesktopIface` beside the pick broker, outside the desktop lock;
    - `list_autostart`, `shell_ready`, the `autostart_changed` signal and the `serve()` arm;
    - `serve(builder, desktop)` keeps its signature and gives the interface a **disabled**
      runner (no spawner, no fallback timer; `ShellReady` only counts); new
      `pub async fn serve_with(builder, desktop, cfg: AutostartConfig)` spawns the 20 s fallback
      task (F1);
    - the emit-error log prints only the event kind (F34);
  - `crates/uqd/src/main.rs`: `serve_with(…, AutostartConfig::system(desktop.dev_session, session))`
    (the only place `SystemSpawner` is built; F1);
  - `crates/uq-core/src/client.rs` (`list_autostart`, `shell_ready`, the `autostart_changed`
    signal, so `receive_autostart_changed` exists);
  - `crates/uqd/tests/dbus.rs` (and extend `desktop_proxy_matches_the_interface`, F27).

**Interfaces (produced):**
- `spawn.rs`:
  - `#[async_trait] pub trait Spawner: Send + Sync`, with:
    - `async fn spawn(&self, unit: &str, tag: &str, argv: &[String], env: &[(String, String)], cwd: &Path) -> std::io::Result<Child>`;
    - `async fn alive(&self, pid: u32, unit: &str) -> bool`;
    - `async fn journal_tail(&self, unit: &str, tag: &str, since: i64, lines: usize) -> Vec<String>`.
  - `pub struct Child { pub pid: u32, pub wait: Pin<Box<dyn Future<Output = ExitInfo> + Send>> }`
    (`futures-util` is only a dev-dependency of uqd, so no `BoxFuture`), and
    `pub enum ExitInfo { Code(i32), Signal(i32), Unknown }`.
  - `SystemSpawner` builds exactly the argv in Global Constraints with
    `tokio::process::Command` (no shell), `systemd-run` by the absolute path found at
    construction. It sets `stdin` to null, uses `kill_on_drop(false)`, removes F35's service
    variables and gives the env as `envs()`. `alive` reads `/proc/<pid>/cgroup`. `journal_tail`
    runs F16's `journalctl` line with a 2 s timeout and a 64 KiB stdout cap, behind a
    semaphore of 4.
  - `FakeSpawner` (under `cfg(any(test, feature = "fake"))`) records calls and has scripted
    exits and journal lines, and an optional journal delay.
- `autostart.rs`:
  - `pub const SHELL_WAIT: Duration = 20 s`, and `pub const MAX_LOG_LINES = 10`.
  - `pub struct AutostartConfig { spawner: Option<Arc<dyn Spawner>>, record: PathBuf, session: Option<String>, dev: bool }`
    with `system(dev, session)` and `disabled()`.
  - `pub struct Runner` holds:
    - the config;
    - `started: bool`, `statuses: BTreeMap<String, Status>`, `shell_ready_calls: u32` and a log
      cache keyed by `(unit, exit)` (F16).
  - `pub fn shell_ready_calls(&self) -> u32` (F39).
  - `pub async fn start_once(runner: &Mutex<Runner>, plan: Vec<Resolved>, env: Vec<(String, String)>, emit: impl Fn() + Send + Sync + 'static)`.
    The caller builds `plan` and `env` with `Desktop::autostart_plan` under the desktop lock
    and drops the lock before calling. Then, per entry: honour R7, sleep the delay in a task,
    spawn, record, and wait for the exit in a task. Each status change calls `emit` (a sync
    broadcast send of `Event::AutostartChanged`). The runner mutex is never held across a
    sleep, a spawn's wait or a journal read.
  - The record file (R7: `{session, ran, started}`) is written with `atomic_write` under
    `$XDG_RUNTIME_DIR/uq/`. `ran` is set once `start_once` has handled every entry. The session
    id must be `[A-Za-z0-9_]{1,128}`, or the runner doesn't run and logs why.
  - D-Bus `ListAutostart() → s` gives
    `[{id, name, icon, source, enabled, can_toggle, delay, status, reason?, exit?, log?, argv?}]`.
    `argv` is present only for `added` commands. `alive` is re-checked for `running` entries
    started by an earlier uqd (F15). `log` is read only for `failed` entries, outside both locks.
    Text is cleaned with `clean_display` (F36).
  - `ShellReady()` is idempotent and returns at once; the run proceeds in a task.
  - The signal is `AutostartChanged()`.

- [ ] **Step 1: Write the failing tests** in `autostart/tests.rs` (FakeSpawner, temp dirs):
  - `runner_starts_each_entry_once_with_scope_and_tag`: the argv starts
    `<abs>/systemd-run --user --scope --quiet --collect --slice=app.slice --unit=app-uq-autostart-nm-applet-`
    and contains `systemd-cat -t uq-autostart-nm-applet --`.
  - `runner_waits_for_delay` (`#[tokio::test(start_paused = true)]`: `waiting` at 4 s,
    `running` after 5 s);
  - `runner_skips_off_and_skipped_entries` (Review Focus 4);
  - `runner_env_matches_child_env` (the env passed equals `child_env()`, and has no
    `INVOCATION_ID` or `JOURNAL_STREAM`; F19, F35);
  - `runner_records_and_does_not_rerun_after_restart` (a second `Runner` with the same session
    and the same record file starts nothing, and reports `running` when `alive` is true or
    `unknown_exit` when false);
  - `runner_restart_starts_nothing_new` (F15: an entry not in the record, enabled since login,
    stays `not_started` after a same-session restart);
  - `runner_new_session_skips_live_pid` (a different session id, with `alive` true for the old
    record, doesn't start it again);
  - `runner_failed_entry_has_exit_and_log` (exit 3 gives `failed`, `exit: 3`, and the last lines
    cleaned of control and bidi characters; the fake saw the run's unit and start time);
  - `journal_tail_is_cached_and_bounded` (F16: two lists read the journal once; five failed
    entries never run more than 4 tails at once);
  - `runner_does_nothing_in_dev_session` (`not_started` with the plain reason);
  - `runner_bad_session_id_runs_nothing`;
  - `shell_ready_twice_runs_once`;
  - `fallback_starts_after_shell_wait` (paused clock);
  - `disabled_runner_never_spawns` (F1: `serve()`'s runner, after `ShellReady` and 25 paused
    seconds, recorded no spawn);
  - `set_section_does_not_start_or_stop` (turning an entry on gives `not_started`, and no spawn
    is recorded).
  - In `crates/uqd/tests/dbus.rs`:
    - `list_autostart_over_dbus`;
    - `autostart_changed_signal`;
    - `list_autostart_does_not_hold_the_desktop_lock` (a `SetSection("look")` during a slow fake
      `journal_tail` returns);
    - `desktop_proxy_matches_the_interface` gains the three members.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd autostart -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Don't add a `rand` dependency: unit suffixes use
  `uq_core::desktop_entry::random_suffix` (Task 1).
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -p uqctl -- --test-threads=2` and clippy. Expected:
  PASS (uqctl's tests use `serve()`, so they stay process-free).
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): run apps from the autostart folders once per session, each in its own scope`

### Task 9: The bridge's launch path applies per-app fixes; `uqctl launch`; `ShellReady`

**Files:**
- Modify:
  - `crates/uqctl/src/bridge/uqd.rs` (F2): `Request::GetSection(String)` (one string arg; the
    reply string parsed as JSON) and `Request::ShellReady` (no args) in `Request::parse` and
    `send`; `Link::state_changed(up = true)` calls `ShellReady` once (errors ignored).
  - `crates/uqctl/src/bridge/launch.rs`:
    - `plan` takes `fix: Option<&AppFix>`;
    - R19 marked-override handling (F20);
    - `exec_argv` with a `FieldCtx`;
    - `compat::launch::argv`;
    - `launch()` fetches the fix (500 ms, never fails).
  - `crates/uqctl/src/bridge/launch_tests.rs`;
  - `crates/uqctl/src/bridge/mod.rs`: when the first inbound request line has been read, call
    `link.call("ShellReady", &[])` once (errors ignored; R5). `"ShellReady"` and `"GetSection"`
    aren't added to `handle()`'s pass-through list; the shell never calls them.
  - `crates/uqctl/src/main.rs` (the `Launch { id: String }` subcommand → `launch_cmd::run`);
  - `crates/uqctl/tests/bridge_dbus.rs`.
- Create: `crates/uqctl/src/launch_cmd.rs`. It reads the fix with `GetSection("compat")`
  (`DesktopProxy`, 500 ms), plans, and dispatches through `HyprSocket::from_env()`, so the scope
  is Hyprland's child like every shell launch. It records the launch with `RecordLaunch` and
  exits non-zero with a plain message on failure.

**Interfaces:**
- Consumes: Task 1 (`exec_argv`, `read_entry`, `normalise_id`, `LaunchEntry`), Task 3 (`Compat`,
  `launch::argv`) and Task 8 (`ShellReady`, `Runner::shell_ready_calls`).
- Produces:
  - `pub fn plan(arg, dirs, rand, runs, terminal, fix: Option<&AppFix>) -> Result<Plan, String>`;
  - `pub async fn fix_for(link: &Link, id: &str) -> Option<AppFix>`;
  - `pub fn valid_source(dirs: &Dirs, id: &str, source: &str) -> Option<PathBuf>` (F20: equals
    `<d>/applications/<id>` for a data dir other than the data home, and is a regular file);
  - `uqctl launch <desktop-id>`.

- [ ] **Step 1: Write the failing tests:**
  - In `launch_tests.rs`:
    - `launch_applies_fix_env_and_args` (the scope command holds `'env' '--'
      'GDK_BACKEND=wayland,x11' … 'code' '--x'`);
    - `launch_without_fix_unchanged`;
    - `launch_terminal_app_wraps_env_before_terminal`;
    - `launch_marked_override_uses_source` (R19: the override in a temp data home names a
      system source, a fix is given, and it's applied once);
    - `launch_marked_override_without_fix_runs_override` (F20: no fix → the override's own
      `Exec`, as written);
    - `launch_marked_override_with_bad_source_uses_override_as_is` (a source outside the data
      dirs, in the data home, or missing: no second fix);
    - `launch_fix_refused_for_equals_program` (Review Focus 5: it launches without the fix);
    - `launch_icon_field_code` (`%i` → `--icon x`).
  - In `uqd.rs` tests: `request_parse_accepts_get_section_and_shell_ready` (and refuses wrong
    arities).
  - In `bridge_dbus.rs` (real uqd served with `serve()`):
    - `launch_fetches_compat_fix` (against a uqd with a fix: the dispatched Lua contains the
      env wrapper);
    - `launch_with_uqd_down_still_launches`;
    - `launch_with_slow_uqd_times_out_and_launches` (a test object at `org.uq.Desktop1` that
      answers `GetSection` after 2 s: the launch dispatches within about 600 ms, without the
      fix);
    - `bridge_calls_shell_ready_after_first_request` (F39: `Runner::shell_ready_calls()` on the
      served uqd is 0 after `Hello`, 1 after the first request, and one more after a uqd
      restart's `UqdUp`).
  - In `crates/uqctl` CLI tests (the existing pattern in `tests/common`):
    `uqctl_launch_unknown_id_fails_plainly`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqctl launch -- --test-threads=2` and
  `cargo test -j 2 -p uqctl --test bridge_dbus -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqctl -- --test-threads=2`, clippy and
  `scripts/lint-shell.sh`. Expected: PASS. Where Quickshell is installed, also run
  `tests/shell/bar_loads.sh`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(shell): launches apply each app's fix; uqctl launch; tell uqd the shell is up`

### Task 10: uq-settings: the Startup page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/startup.rs` (starting with F9's
    `#![cfg_attr(not(test), allow(dead_code, …))]`);
  - `apps/uq-settings/src/pages/startup/tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/autostart.rs` (`FakeAutostart`: a scripted
    `ListAutostart` list, a switch that makes it fail, and a `shell_ready` counter).
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod startup;`);
  - `apps/uq-settings/src/pages/runner.rs` (`pub fn plain_refusal(text: &str) -> String`, F21);
  - `apps/uq-settings/src/pages/fake_uqd.rs`:
    - `list_autostart` and `shell_ready`;
    - `explain("env")`, `explain("autostart")` through `startup_explain` with a
      `FakeUqd.files: Arc<Mutex<FileReport>>`, then `Explain.overrides` (slice C's F7);
    - the three sections go through the generic section write;
    - `Rig.autostart` and `Rig.files`.

**Interfaces:**
- Consumes:
  - `runner::{PageLogic, parse_json, plain_refusal}`;
  - `DesktopProxy::{get_section, set_section, explain, list_autostart}`;
  - `uq_core::{env, autostart, desktop_entry::{split_command, quote_command}}`;
  - `crate::desktop_entries` (the installed apps for "Add an app").
- Produces:
  - `pub enum Op`:
    - `Refresh`;
    - `SetEnabled { id: String, on: bool }`: a file entry writes `files[id].enabled`, and an
      added one writes its own `enabled`;
    - `SetDelay { id, delay: Option<u32> }` (a `files` entry left with neither field is
      removed);
    - `AddApp { desktop_id: String }`: the name comes from `desktop_entries`, cut to 100 chars
      (F43);
    - `AddCommand { name: String, command: String }`, split with `split_command`; a split
      error is the op's error, in plain words;
    - `Remove { id }`: added entries only; for a file entry the op is refused ("Turn it off
      instead");
    - `SetVar { index: Option<usize>, name: String, value: String }` (`None` appends);
    - `RemoveVar { name }`;
    - `MoveVar { name, to }`.

    Every write reads the section, edits it, pre-checks it with the uq-core validator (showing
    only `reason`, F21), and writes it with origin `settings` under the write lock. A refusal
    from uqd is shown through `plain_refusal`.
  - `pub struct StartupState`, a `PageLogic` with `new(conn)`.
  - **The snapshot:**
    ```
    { apps:    [{id, name, icon, source, enabled, can_toggle, delay, status, status_text,
                 reason, exit, log, command_text, origin}],
      vars:    [{name, value, warn: bool, origin, line, replaced_by_label}],
      switches:[{key, label, on}],            // §6 switches, read-only (from GetSection("compat"))
      section: {env: {source, message}|null, autostart: {source, message}|null},
      installed: [{id, name, icon}],          // for Add an app
      limits:  {vars: 100, value_bytes: 4096, added: 50, delay_max: 600},
      notice:  {seq, text} }
    ```
    - `origin` is the row's Explain entry as uqd gives it (`{source, detail}`), passed whole so
      `SettingRow`-style text ("Overridden in user.lua, line N", and from Task 13 "From the X
      profile") is worded once, by the QML component.
    - `status_text` is the plain line, for example "Running", "Started, then finished",
      "Failed (exit 3)", "Starts next time you log in", "Waiting 10 s", "Off", "Off in uQ" (a
      `yours` entry off in state, R15, F22), the skip reason, or "Not started in a test
      session".
    - `command_text` is `quote_command(argv)` for an added command, for display only (F22).
    - `notice` is set after any env write: "Changes apply to apps started after the change, and
      fully after you log out and back in." After removing a variable it adds "Removed variables
      stay for apps started before you log out." (R9).

- [ ] **Step 1: Write the failing tests** in `pages/startup/tests.rs`:
  - `toggle_system_entry_round_trips` (§9: `files[id].enabled = false`; reopening the state is
    identical; history names it);
  - `toggle_hidden_entry_refused_plainly` (`can_toggle: false`);
  - `add_app_and_command_round_trip` (the command `backup-check --quiet "a b" 100%` is stored as
    four args, and `command_text` shows `backup-check --quiet "a b" 100%`);
  - `add_app_long_name_is_cut` (F43);
  - `bad_command_is_a_plain_error` (an unclosed quote);
  - `remove_file_entry_refused`;
  - `delay_round_trips_and_range_refused` (601 is refused with a message that holds no
    `autostart.` or `[`; F21);
  - `clearing_delay_and_enabled_removes_choice`;
  - `set_var_blocked_name_shows_message` (the `XDG_SESSION_TYPE` refusal's reason is shown,
    with no `env.vars`, and the state is unchanged);
  - `set_var_warns_ld_preload`;
  - `var_with_dollar_refused` (R10);
  - `notice_after_env_write_and_removal` (R9);
  - `vars_show_user_lua_override` (`Explain.set("env.vars.FOO", …)` reaches the row's
    `origin`);
  - `switches_listed_read_only`;
  - `failed_entry_shows_exit_and_log` (scripted `FakeAutostart`);
  - `yours_off_says_off_in_uq`;
  - `uqd_down_gives_empty_state_without_an_error`;
  - `list_autostart_failure_still_shows_vars` (FakeAutostart fails, and `apps: []` with an
    error).
- [ ] **Step 2:** Run the tests through t7check:
  `cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::startup -- --test-threads=2`.
  Expected: FAIL.
- [ ] **Step 3:** Implement. No `cxx_qt` or Qt types in `pages/startup.rs`.
- [ ] **Step 4:** Re-run. Expected: PASS. CI runs `cargo test -p uq-settings` and clippy (F9's
  attribute keeps the not-yet-wired logic warning-free).
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): Startup page logic for login apps and environment variables`

### Task 11: uq-settings: the Apps and scaling page logic (Blurry right now, Fix this app)

**Files:**
- Create: `apps/uq-settings/src/pages/compat.rs` (with F9's attribute) and
  `apps/uq-settings/src/pages/compat/tests.rs`.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs`;
  - `apps/uq-settings/src/desktop_entries.rs` (`AppEntry` gains `wm_class: Option<String>` and
    `exec_name: Option<String>`, from `uq_core::desktop_entry::read_entry`; `exec_name` is the
    file name of `exec_argv(exec)[0]` (F43); both are cleaned like `name`);
  - `apps/uq-settings/src/pages/fake_uqd.rs` (`explain("compat")`, built from slice A's generic
    rows for `sharp_x11`, then `default_when_off`, plus `explain_compat_extra`; and
    `refuse_next: Arc<Mutex<Option<String>>>`, which makes the next `SetSection` fail with
    `InvalidSetting` and that text, F32).

**Interfaces:**
- Consumes:
  - `ListWindows` (slice C) and `ListOutputs` (Phase 3 slice 3);
  - `GetSection`/`SetSection("compat")` and `Explain("compat")`;
  - `uq_core::compat`;
  - `crate::desktop_entries`.
- Produces:
  - `pub enum Op`:
    - `Refresh`, `RefreshWindows`;
    - `SetSwitch { key: String, on: bool }`;
    - `SaveFix { fix: Value }`: an add or a replace by id;
    - `RemoveFix { id }`;
    - `SetElsewhere { id, on }`.

    Writes pre-check with `Compat::from_section` and show `reason` only; uqd refusals go through
    `plain_refusal` (F21).
  - `pub fn app_for_class(class: &str, apps: &[AppEntry]) -> Option<&AppEntry>`, which tries in
    order:
    1. `StartupWMClass` equal ignoring ASCII case;
    2. the id without `.desktop` equal ignoring case;
    3. the last dot part of the id equal ignoring case (`org.gnome.Foo` ↔ `foo`);
    4. `exec_name` equal.
  - `pub fn fields() -> Value`: the catalogue built from `compat::hypr` (switch keys, labels and
    descriptions; the platform choices; the scale steps 1.0–3.0). QML never copies the tables
    (slice C's F6).
  - `pub fn preview(fix: &Value) -> Value` (F31): pure; runs `compat::launch::argv` on the dummy
    exec `["app"]` for the draft and returns `{env: [names], args: [..], problem: text|null}`.
  - **The snapshot:**
    ```
    { switches: [{key, label, description, on, origin}],
      browsers: [{name, file}],                         // from Explain chromium_wayland detail
      blurry:   {scaled: bool, sharp_on: bool,
                 windows: [{title, class, app_id|null, app_name|null, has_fix: bool}]},
      fixes:    [{id, name, icon, platform, scale, env, args, elsewhere, override, message, origin}],
      installed:[{id, name, icon}],
      fields:   {...},
      reopen:   {seq, names: [..], more: n},
      section:  {source, message} | null }
    ```
    - `origin` is the row's Explain entry (`{source, detail}`), as on the Startup page.
    - `blurry.windows` holds the `xwayland` windows from `ListWindows`, de-duplicated by class.
      `scaled` is true when any enabled output from `ListOutputs` has a scale ≠ 1.
    - The heading text comes from `(scaled, sharp_on)`:
      - "These older apps may look blurry." when scaled and not sharp;
      - "Older apps open now" when sharp is on;
      - "Your screens aren't scaled, so nothing is blurry." when not scaled.
    - `reopen` is set after `SaveFix` or `RemoveFix` for an app with open windows:
      "Close and reopen {names} to apply." (slice C's ruling: no button closes windows).
    - `override` and `message` come from the `apps.<id>` Explain row. A refusal from
      `check_compat_files` (`InvalidSetting`, F3) is the op's error, and the switch snaps back
      with that text (§9).

- [ ] **Step 1: Write the failing tests** in `pages/compat/tests.rs`:
  - `switch_round_trips_and_journals` (all five);
  - `blurry_lists_only_xwayland_and_names_the_app`. Three windows (one XWayland Code with class
    `Code`, one Wayland Firefox, one XWayland with an unknown class) and installed `code.desktop`
    with `StartupWMClass=Code` give two rows: one with `app_id: "code.desktop"` and one with
    `app_id: null`.
  - `blurry_heading_states` (the three cases);
  - `app_for_class_order` (each of the four matches, and an earlier rule wins);
  - `save_fix_round_trips` (§9);
  - `fix_with_blocked_env_shows_message` (the reason only, no `compat.apps`);
  - `elsewhere_refusal_snaps_back_with_reason` (the fake's `refuse_next` holds uqd's text);
  - `save_fix_asks_to_reopen_open_app`;
  - `fields_come_from_the_table`;
  - `preview_names_env_and_args` (F31);
  - `browsers_from_explain_detail`;
  - `sharp_x11_off_reads_default` (F6, through the fake);
  - `uqd_down_gives_empty_state_without_an_error`.
- [ ] **Step 2:** Run through t7check: `pages::compat`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): Apps and scaling page logic with blurry-app fixes`

### Task 12: uq-settings: the two pages, their models and their registry entries

**Files:**
- Create:
  - `apps/uq-settings/src/startup_model.rs`, `apps/uq-settings/src/compat_model.rs`;
  - `apps/uq-settings/qml/pages/StartupPage.qml`, `apps/uq-settings/qml/pages/AppsScalingPage.qml`;
  - `apps/uq-settings/qml/components/AutostartRow.qml`, `EnvVarRow.qml`, `AppFixDialog.qml`,
    `AddStartupDialog.qml`;
  - `tests/settings-qml/tst_autostart_row.qml`, `tst_env_var_row.qml` and `tst_app_fix_dialog.qml`,
    with a `fields` fixture.
- Modify:
  - `apps/uq-settings/build.rs` (the QML files and the two model files);
  - `apps/uq-settings/src/main.rs` (`mod startup_model; mod compat_model;`);
  - `apps/uq-settings/src/model_core.rs` (F8): `Follow` gains `autostart: bool`; `follow_uqd`
    adds `receive_autostart_changed` when it's set;
  - every existing model's `FOLLOW` literal (`appearance_model.rs`, `displays_model.rs`,
    `history_model.rs`, `input_model.rs`, `look_model.rs`, `profiles_model.rs`,
    `rules_model.rs`, `shell_model.rs`, `wallpaper_model.rs`) gains `autostart: false`;
  - `apps/uq-settings/src/pages/startup.rs` and `pages/compat.rs` (drop F9's attribute);
  - `apps/uq-settings/src/registry.rs`:
    - two pages;
    - remove the Task 2–4 skips in `every_journal_domain_has_one_page` and Task 3's skip in
      `provenance_rows_name_registry_settings` (F7);
    - update `pages_json_lists_pages_in_order` and the per-page order tests (slice C's F24
      pattern);
  - `scripts/settings-jargon.txt` (Global Constraints).

**Interfaces:**
- Consumes: Tasks 10 and 11.
- Produces:
  - `StartupModel` (`#[qml_element]`):
    - properties `state_json`, `error` and `busy`;
    - invokables `refresh()`, `set_enabled(id, on)`, `set_delay(id, secs)` (−1 means null),
      `add_app(id)`, `add_command(name, command)`, `remove(id)`, `set_var(index, name, value)`
      (−1 appends, F31), `remove_var(name)` and `move_var(name, to)`.

    Its `FOLLOW` is sections `env`, `autostart` and `compat`, plus `history`, plus
    `autostart: true` (F8). It re-reads on window activation (slice A's ruling).
  - `CompatModel`:
    - the same three properties;
    - invokables `refresh()`, `refresh_windows()`, `set_switch(key, on)`, `save_fix(json)`,
      `remove_fix(id)`, `set_elsewhere(id, on)`, and the synchronous `preview(json) -> QString`
      (F31, slice C's F17 pattern).

    Its `FOLLOW` is sections `compat` and `env`, plus `history`.
  - **Registry:**
    - `PageInfo { id: "apps-scaling", name: "Apps and scaling", icon: "preferences-desktop-display-randr", category: Category::Devices, qml: "pages/AppsScalingPage.qml", settings: &COMPAT_SETTINGS, domains: &["compat"] }`,
      placed right after `displays`.
    - `PageInfo { id: "startup", name: "Startup", icon: "system-run", category: Category::System, qml: "pages/StartupPage.qml", settings: &STARTUP_SETTINGS, domains: &["autostart", "env"] }`,
      the first System page.
    - `COMPAT_SETTINGS`:
      - `entry("sharp-x11", "Sharp X11 apps", &["blurry", "xwayland", "scaling", "hidpi", "fractional"])`;
      - `entry("electron-wayland", "Run Electron apps on Wayland", &["electron", "blurry", "discord", "code"])`;
      - `entry("chromium-wayland", "Run Chromium-based browsers on Wayland", &["chrome", "brave", "browser", "blurry"])`;
      - `entry("qt-wayland", "Qt apps prefer Wayland", &["qt", "kde"])`;
      - `entry("steam-scale", "Steam UI scale", &["steam", "games", "size"])`;
      - `entry("blurry-now", "Blurry right now", &["fix", "app", "blurry", "x11"])`;
      - `entry("app-fixes", "App fixes", &["per-app", "wayland", "x11", "scale", "arguments"])`.
    - `STARTUP_SETTINGS`:
      - `entry("login-apps", "Apps that start when you log in", &["autostart", "login", "startup", "boot"])`;
      - `entry("add-startup", "Add an app or command", &["autostart", "add", "command"])`;
      - `entry("start-delay", "Start delay", &["delay", "wait", "later"])`;
      - `entry("env-vars", "Environment variables", &["env", "variable", "path", "export"])`.
  - **`AppsScalingPage.qml`:**
    - The five switches (`SettingRow` with `source: row.origin`, so its own text reads
      "Overridden in user.lua, line N" with **Open user.lua**, or from Task 13 "From the X
      profile"). Each has its sentence and "Applies to apps started after the change."
    - Under the Chromium switch, the per-browser `own_file` lines.
    - A **Blurry right now** card: the heading, the rows, **Fix this app** and **Refresh**.
      Unmatched rows open the dialog with an app chooser ("uQ couldn't tell which app this is.
      Choose it:").
    - An **App fixes** list with **Edit** and **Remove**, the override state, the row's origin
      line, and the `reopen` message in a `RuleMessage`.
  - **`AppFixDialog.qml`** (`Kirigami.Dialog`, title literal "Fix this app"):
    - **Run on** (Automatic / Wayland / X11) with the hint "Most blurry apps are sharp with
      Wayland.";
    - **Size** (Automatic, 100 %…300 % in 25 % steps);
    - **Extra settings for this app** (`EnvVarRow` list);
    - **Extra options** (one per line);
    - **Also when started from other launchers**, with its consequence sentence: "uQ saves its
      own copy of this app's launcher file, and removes it when you remove the fix.";
    - an `AdvancedDetails` disclosure with the env names and args this fix produces (from
      `CompatModel.preview(json)`).

    It emits `accepted(string fixJson)`. The JSON is built from catalogue keys (slice C's F29).
  - **`StartupPage.qml`** has two sections:
    - **Apps that start when you log in**: `AutostartRow` per app, **Add an app or command**,
      and the line "Turning an app on takes effect next time you log in; turning it off doesn't
      close it now.";
    - **Environment variables**: `EnvVarRow`s, **Add a variable**, the `notice`, and the §6
      switches read-only with **Open Apps and scaling** (`ShowPage("apps-scaling")`).
  - **`AutostartRow.qml`** has:
    - `property var app`;
    - a name `UqLabel` (`objectName: "appName"`);
    - a source line ("From the system" / "Yours" / "Added in uQ");
    - an origin line (`objectName: "appOrigin"`) worded as `SettingRow` words a source, from
      `app.origin` (only for `user_lua`, `error` and, from Task 13, `profile`);
    - a switch with a fixed literal or no `text:`, disabled when `!can_toggle`;
    - a status `UqLabel` (`objectName: "appStatus"`);
    - a delay spin box (0–600 s);
    - on failure, the `log` lines in `UqLabel`s inside a disclosure;
    - **Remove** for added entries.
  - **`EnvVarRow.qml`** has a name field (uppercase hint) and a value field, the warning line for
    `warn`, the origin line (as above), and the "replaced by" line.

- [ ] **Step 1: Write the failing tests:**
  - In `registry.rs`:
    - `startup_page_registered_with_search_entries`;
    - `apps_scaling_page_after_displays`;
    - `every_journal_domain_has_one_page` with no skips;
    - `provenance_rows_name_registry_settings` with no skips;
    - updated order tests.
    - The existing `every_setting_has_keywords`, `no_duplicate_setting_ids` and
      `ids_unique_and_categories_ordered` must still pass.
  - In `tst_autostart_row.qml`:
    - `test_name_is_plain_text` (a `Name` of `<b>x</b>` shows literally, with
      `Text.PlainText`, found by `objectName`; slice C's F26);
    - `test_log_lines_plain`;
    - `test_switch_disabled_when_cannot_toggle`;
    - `test_remove_only_for_added`;
    - `test_origin_line_from_source` (a `user_lua` origin shows the line text once).
  - In `tst_env_var_row.qml`: `test_warning_shown_for_warned`, `test_value_plain_text` and
    `test_replaced_line`.
  - In `tst_app_fix_dialog.qml`:
    - `test_builds_fix_json_from_fields`;
    - `test_title_is_fixed` (an app named `<b>x</b>` leaves the title "Fix this app");
    - `test_scale_steps`;
    - `test_elsewhere_sentence_shown`.
- [ ] **Step 2:** Run `cargo test -j 2 --manifest-path <t7check>/Cargo.toml registry -- --test-threads=2`.
  Expected: FAIL. The QML suite runs in CI (K4).
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `scripts/lint-settings.sh` and `tests/shell/lint_settings.sh`. Expected:
  PASS. CI runs
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`
  and `cargo test -p uq-settings` with clippy.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): the Startup and Apps and scaling pages`

### Task 13: Profiles hold startup apps, environment variables and app fixes

D1 (user decision, 2026-11-11). This task builds on slice B's overlay and slice C's Task 11
(`crates/uq-core/src/profile/rules.rs`, `crates/uqd/src/profiles.rs`) without changing the file
format, `MAX_KEYS` (500), the restore point, or the K3 logic. A profile holds only values the
pages themselves accept and validate, and never adds an entry, a command, launch options or a
file (spec §10).

**What a profile may hold (ruling):**

| Domain | Profile tree | Leaf | Meaning | Refused |
|---|---|---|---|---|
| `env` | `[env]` | `NAME = "value"` | set it (added at the end when absent, replaced in place when present) | a name failing `env::check_name` (rule or blocklist); a value failing `env::check_var` (4096 bytes, controls, `$`); any other type |
| `env` | `[env]` | `NAME = false` | remove it while the profile is on | `true` |
| `compat` | `[compat]` | `sharp_x11` … `steam_scale` `= true\|false` | the five switches | any other key |
| `compat` | `[compat.apps."<id>"]` | `platform = "auto"\|"wayland"\|"x11"`, `scale = 1.0–3.0 (0.25 steps)` | that fix's platform and size; a missing fix is created with only these, and a fix left with nothing set is removed | `env`, `args`, `elsewhere` ("a profile can only set an app's Run on and Size"); a bad id (`valid_desktop_id`) |
| `autostart` | `[autostart."<id>"]` | `enabled = true\|false`, `delay = 0–600` | an existing entry's on/off and start delay; `<id>` is a file id or an added id (`a-…`) | `app`, `argv`, `name` or any other key ("a profile can only turn a startup app on or off or set its delay"); a bad id |

- Removing a variable reads back as `false`, so an absent variable's "before" is `false` and
  Back to normal removes a variable the profile added, by the same K3 rule as every key.
- Scale has no "Automatic" in a profile (TOML has no null); Back to normal puts back what was
  there before.
- **Unknown ids.** An `autostart` id that is neither found in the folders nor in
  `autostart.added` on this computer is left out of the apply (no write, no memo), counted by
  `missing_startup` and reported with `missing_startup_notice`; the file keeps it, as for rules.
  A `compat.apps` id isn't checked for being installed: a fix for an app that isn't there does
  nothing, so it's applied like any other.
- **Limits.** 500 keys per profile (slice B); applying must keep the domains valid: more than
  100 variables after the apply refuses it with "This profile would make more than 100
  environment variables." (no change, no journal entry), and a fix created by the profile counts
  toward the 100 fixes.
- **Applying (one write).** `apply_profile` goes through the same `commit(next, Change::Theme,
  Origin::Profile(id))` as before: the restore point is saved first, `generated.lua` is rendered
  (the `hl.env` lines and the xwayland option) and verified by Hyprland, then `install` →
  `persist` journals one entry per changed domain with origin `profile:<id>` and the domain's
  own summary, and `sync_files` rewrites `environment.d`, the flag files, any affected launcher
  override (a fix with `elsewhere` already on gets its new platform or size) and the hidden
  autostart copies. A rejected verify changes nothing. Back to normal is the same path.
- **What "after the change" means for a switch.** Like a page write: apps started after the
  switch get the new environment and fixes; apps already open keep theirs (removed variables
  linger, R9); startup app changes take effect next time you log in, and the runner never starts
  or stops anything on a switch. `apply_profile` adds `startup_notice` to the report's notices
  when the profile holds `env` or `compat` keys ("Apps you start from now on use this profile's
  settings; apps already open keep theirs.") or `autostart` keys ("Startup apps change next time
  you log in.").
- **The `elsewhere` pre-check isn't needed**, since a profile can't turn `elsewhere` on; a
  profile can't write any file the page couldn't.

**Files:**
- Create: `crates/uq-core/src/profile/startup.rs` (the helpers below) and
  `crates/uq-core/src/profile/startup/tests.rs` (F28).
- Modify:
  - `crates/uq-core/src/profile/mod.rs`:
    - `DomainKind` gains `Env`, `Compat` and `Autostart`; `PROFILE_DOMAINS` (now 11) maps
      `("env", Env)`, `("compat", Compat)`, `("autostart", Autostart)`;
    - `validate` checks each tree with `startup::check_env_tree`, `check_compat_tree` and
      `check_autostart_tree` (they aren't written to a default `State`);
    - `read_leaf` and `write_leaves` dispatch the three kinds to `startup::read_leaf` and
      `startup::write_leaves`;
    - `key_label` and `value_text` learn the three domains (below);
    - test `refuses_unknown_domain` keeps refusing a made-up domain.
  - `crates/uq-core/src/profile/overlay.rs`:
    - `apply` leaves out `Autostart` keys whose id isn't in `known` — `apply` gains a
      `known: &BTreeSet<String>` argument (`autostart::scan::known_ids`; uqd's and the fake's
      callers pass it);
    - `created_leaves` skips the three kinds (their memos come from the keys);
    - `turn_off` skips an `Autostart` memo whose entry is gone, and labels Kept keys with
      `startup::startup_label`;
    - `changed_since` sends `env`, `compat` and `autostart` entries through
      `startup::startup_keys`;
    - test `from_journal_maps_domains` gains `"env"`, `"compat"`, `"autostart"` → `None`.
  - `crates/uqd/src/profiles.rs`: `apply_profile` computes `known` with
    `autostart::scan::known_ids` (the folder read under the lock, bounded by F14), passes it,
    and adds `missing_startup_notice` and `startup_notice` to `notices`.
  - `crates/uqd/src/startup.rs`: `explain_startup` calls `profile::mark_startup` when a profile
    is on.
  - `crates/uqd/src/profiles/tests.rs` and `crates/uqd/src/startup/tests.rs`.
  - `apps/uq-settings/src/pages/profiles.rs`:
    - `current` reads `GetSection` for `env`, `compat` and `autostart` (and `ListAutostart` for
      entry names) and turns them into keys with `startup::startup_keys_of`;
      `profile_journal_domains` and `holds_journal_domain` include them, so the Startup and Apps
      and scaling pages are offered under "The current settings of these pages" and their keys
      in **Add settings**;
    - the snapshot labels these keys with `startup_label` (names from `ListAutostart`, app names
      from `desktop_entries`);
    - the export path says "This profile includes your variables' values." when the profile
      holds `env` keys.
  - `apps/uq-settings/src/profiles_model.rs`: `FOLLOW.sections` gains `"env"`, `"compat"`,
    `"autostart"`.
  - `apps/uq-settings/src/pages/fake_uqd/profiles.rs`: `apply` passes `known` (from
    `FakeAutostart`'s list plus `added`) and adds the same notices.
  - `apps/uq-settings/src/pages/fake_uqd.rs`: `explain("env"|"compat"|"autostart")` calls
    `profile::mark_startup` before applying `Explain.overrides`.
  - `apps/uq-settings/src/startup_model.rs` and `compat_model.rs`: `FOLLOW.profiles = true`, so
    the origin line follows a profile turning on or off even when no value changed.
  - `apps/uq-settings/src/pages/profiles/tests.rs`, `pages/startup/tests.rs` and
    `pages/compat/tests.rs`.

**Interfaces (produced, `uq_core::profile`, re-exported from `profile/startup.rs`):**
- `pub fn check_env_tree(tree: &Value) -> Result<(), ProfileError>`,
  `pub fn check_compat_tree(tree: &Value) -> Result<(), ProfileError>`,
  `pub fn check_autostart_tree(tree: &Value) -> Result<(), ProfileError>`: the table above; keys
  in `Invalid` are `env.<NAME>`, `compat.<switch>`, `compat.apps.<id>.<field>`,
  `autostart.<id>.<field>`, read through `plain` (no control character from the file reaches the
  text).
- `pub fn read_leaf(state: &State, domain: &str, path: &[String]) -> Option<Value>`:
  `env [NAME]` → the value, or `false` when absent; `compat [switch]` → bool;
  `compat ["apps", id, "platform"|"scale"]` → the fix's value, `None` when there's no fix or no
  scale; `autostart [id, "enabled"|"delay"]` → an added entry's own value, a `files` choice's
  value, or `None` when unset.
- `pub fn write_leaves(state: &mut State, domain: &str, leaves: &[(Vec<String>, Option<Value>)]) -> Result<(), SettingError>`:
  one validated `set_section` per domain (the section's own `from_section` runs, so every
  limit holds); `None` removes (an env var, a fix's platform → `auto`, scale → `null`, a file
  choice's field); a fix or a file choice left empty is removed; an autostart id with no entry
  is refused (callers leave those out). On an error `state` is unchanged.
- `pub fn startup_keys_of(state: &State) -> Vec<Key>` (every variable, every switch, every fix's
  platform and scale, every entry's `enabled` and `delay` as the state holds them; file entries
  without a choice are offered from `ListAutostart` by the page).
- `pub fn startup_label(state: &State, domain: &str, path: &[String], names: &dyn Fn(&str) -> Option<String>) -> String`:
  "Environment variable FOO"; the switch's `SwitchKey::label()`; "Run on for “Code”" and "Size
  for “Code”"; "Start “Syncthing” at login" and "Start delay for “Syncthing”". `names` gives an
  app or entry name; the fallback is the id without `.desktop`, and an unknown startup id reads
  "A startup app that isn't on this computer". `key_label` (no state) uses the fallbacks.
- `value_text` learns: `env` `false` → "Removed"; `platform` → "Automatic" / "Wayland" / "X11";
  `scale` → "150 %"; `delay` → "10 s".
- `pub fn missing_startup(known: &BTreeSet<String>, profile: &Profile) -> usize` and
  `pub fn missing_startup_notice(n: usize) -> String` ("1 startup app in this profile isn't on
  this computer, so it was left out." / "N startup apps …").
- `pub fn startup_notice(profile: &Profile) -> Vec<String>` (the two sentences above, as they
  apply).
- `pub fn startup_keys(domain: &str, before: &Value, after: &Value) -> (Vec<(String, Vec<String>)>, Vec<String>)`:
  for "Create from history": env names whose value changed or were removed; switches that
  changed; a fix's `platform`/`scale` changes; an entry's `enabled`/`delay` changes for ids on
  both sides. Other changes (added or removed entries, a fix's env, args or `elsewhere`, order)
  are tags counted once in "can't go in a profile".
- `pub fn mark_startup(rows: &mut Value, domain: &str, state: &State, active: &ActiveProfile)`:
  each `uq` row whose key the active profile still holds (`holds`) becomes
  `{source: "profile", detail: {name}}`: `vars.<NAME>`, the switch rows, `apps.<id>` (when its
  platform or scale holds), and autostart rows by id. `user_lua` and `error` rows stay.

- [ ] **Step 1: Write the failing tests:**
  - In `profile/startup/tests.rs`:
    - `env_tree_validation` (Review Focus 5, 7): `[env]\nFOO = "bar"\nOLD = false` parses and
      round-trips through `to_toml`; refused, each with a plain one-line message and its key:
      `foo = "x"`, `XDG_SESSION_TYPE = "x"`, `XDG_CONFIG_HOME = "x"`, `UQ_X = false`,
      `A = "$HOME"`, `A = "a\u0001b"`, a 4097-byte value, `A = true`, `A = 3`; `LD_PRELOAD = "x"`
      is accepted.
    - `compat_tree_validation`: switches and `[compat.apps."code.desktop"] platform = "wayland",
      scale = 1.5` pass; refused: `scale = 1.1`, `platform = "mir"`, `env = []`, `args = ["--x"]`,
      `elsewhere = true`, an id `../x`, a key `blur = true`.
    - `autostart_tree_validation` (Review Focus 7): `[autostart."nm-applet.desktop"] enabled =
      false` and `[autostart."a-3f9c"] delay = 10` pass; refused: `argv = ["x"]`, `app =
      "x.desktop"`, `name = "x"`, `delay = 601`, `enabled = "no"`, an id `../x`, an id
      `a-ZZ`.
    - `read_and_write_startup_leaves`: setting and removing a variable (position kept on
      replace, appended on add); a switch; creating and emptying a fix (the entry disappears);
      a file choice and an added entry's `enabled`; an unknown autostart id refused with the
      state unchanged.
    - `labels_and_values_are_plain` (no `.desktop`, `env.`, `{`; a `<b>x</b>` name stays
      literal text for `UqLabel`).
    - `startup_keys_classify` (a variable changed and one removed are keys; an added entry, a
      fix's args and an order change are "other", each once).
    - `mark_startup_only_overrides_uq`.
  - In `profile/overlay.rs`:
    - `apply_skips_unknown_startup_ids`: one known and one unknown id: the known one is set and
      has one memo, `missing_startup` is 1, the file text is unchanged.
    - `turn_off_env_k3`: profile sets `FOO=a`, removes `BAR`, adds `NEW=1`. By hand: `FOO`
      changed to `b`. Back to normal keeps `FOO=b` (named in Kept as "Environment variable
      FOO"), puts `BAR` back, removes `NEW`.
    - `turn_off_compat_created_fix`: a profile creates a fix (`platform = "x11"`); Back to
      normal removes the whole fix; a fix whose `args` the user edited meanwhile keeps its args
      and loses only the profile's platform.
    - `too_many_variables_refused` (101 after the apply: refused, state unchanged).
    - `changed_since_offers_startup_keys`.
    - `from_journal_maps_domains` gains the three domains → `None`.
  - In `profile/mod.rs`: `profile_holds_startup_domains` (a file with all three parses, counts
    toward `MAX_KEYS`, and 501 keys across them is `TooManyKeys`).
  - In `crates/uqd/src/profiles/tests.rs`:
    - `profile_sets_env_and_back` (apply journals an `env` entry with origin `profile:<id>`
      and summary "Environment variable FOO added"; the verified `generated.lua` has the
      `hl.env` line; `environment.d` has it; a restore point was saved first; Back to normal
      removes it from all three);
    - `profile_switch_updates_override` (a fix with `elsewhere` on: the profile's platform
      change rewrites the marked launcher file; Back to normal rewrites it back);
    - `profile_turns_system_entry_off_and_back` (the hidden copy appears, then goes);
    - `profile_autostart_never_starts_or_stops` (Review Focus 4: no spawn recorded; status
      `not_started` for an entry the profile turned on);
    - `unknown_startup_id_is_a_notice` and `startup_notice_in_report`;
    - `rejected_verify_changes_nothing` (no journal entry, `environment.d` unchanged).
  - In `crates/uqd/src/startup/tests.rs`: `explain_says_from_the_profile` (env, a switch, a fix
    and an entry read `profile` with the name while they hold; `uq` after a hand change;
    `user_lua` wins).
  - In `apps/uq-settings/src/pages/profiles/tests.rs`:
    - `startup_keys_offered_in_add_settings` (variables, switches, fixes' platform and size,
      entries' on/off and delay, labelled by name; no command, args or `elsewhere` key);
    - `create_from_startup_and_apps_pages`;
    - `create_from_history_takes_startup_keys`;
    - `missing_startup_key_is_labelled_and_removable`;
    - `export_with_env_warns_about_values`;
    - `import_and_export_round_trip`.
  - In `pages/startup/tests.rs` and `pages/compat/tests.rs`: `row_says_from_the_profile` (the
    fake's profile on → the row's `origin.source == "profile"` and `origin.detail.name ==
    "Gaming"`, carried once, with no separate name field).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core profile -- --test-threads=2` and
  `cargo test -j 2 -p uqd profile -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. The Startup and Apps and scaling rows already show `origin`
  through `SettingRow` or the rows' origin line (Task 12), so "From the X profile" needs no new
  QML; the profile's name reaches it only as data in a `UqLabel`. The Profiles page needs no QML
  change: the new keys arrive in `available` and in each profile's `keys` like any other, shown
  through `UqLabel`; its page picker lists the two new pages from `profile_journal_domains`.
  uqctl needs no change (`profile apply` prints notices and Kept labels; names have no control
  characters).
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -p uqd -p uqctl -- --test-threads=2` and clippy.
  Expected: PASS, every slice B and C profile test included. Through t7check:
  `pages::profiles`, `pages::startup`, `pages::compat`. CI runs `cargo test -p uq-settings`, the
  settings QML suite and `scripts/lint-settings.sh`.
- [ ] **Step 5:** Commit (K5 trailers): `feat: profiles hold startup apps, environment variables and app fixes`

### Task 14: Docs, changelog, live checks and the whole gate

**Files:**
- Modify:
  - `README.md`;
  - `CHANGELOG.md` (`## [Unreleased]` → `### Added`);
  - `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` (§10 D-Bus summary:
    `SetAutostart` → `SetSection("autostart")`, plus `ShellReady`; §4 names what a profile can
    hold of `env`, `compat` and `autostart`; §12 row D unchanged);
  - `docs/developing.md` (a dev session starts no autostart apps and writes or deletes no
    launcher overrides, R20).
- Create: `docs/notes/settings-slice-d-live-check.md`.

- [ ] **Step 1:** Update `README.md`:
  - a "#### Startup page" paragraph:
    - which folders are read;
    - once per session (a uqd restart starts nothing new);
    - the status words;
    - the hidden copy uQ writes for a system entry you turn off, and that your own files are
      never edited;
    - added apps and commands (no shell);
    - environment variables, the blocklist, "after the change / after logging out", no `$`,
      that removed variables linger until logout, and that `environment.d` doesn't get the
      scale lines;
    - undo puts back a whole list (F36).
  - a "#### Apps and scaling page" paragraph:
    - the five switches;
    - Blurry right now and Fix this app;
    - where fixes apply (uQ's launcher, Spotlight, pins, `uqctl launch`, and autostart);
    - "Also when started from other launchers" and the `X-uQ-Override` marker;
    - Chromium flag files and their marker line.
  - Add `ListAutostart`, `ShellReady` and `AutostartChanged` to the Settings D-Bus paragraph,
    and say that the three domains use `GetSection`/`SetSection`.
  - In the Profiles paragraph, say what a profile can hold of startup apps, environment
    variables and app fixes (D1), that it can't add a startup command, launch options or a
    launcher file, and that startup changes take effect at the next login.
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Startup apps, environment variables and app
  scaling.** Name both pages, the runner, the switches, the fixes, the markers, profiles holding
  them, and the new D-Bus calls.
- [ ] **Step 3:** Write `docs/notes/settings-slice-d-live-check.md` in the style of slice C's
  (run on real Hyprland; tick each item):
  1. **§1 criterion 6.** Put a `.desktop` for `foot --title AutoTest` in `~/.config/autostart`
     and log out and in. The app is open; `systemctl --user status 'app-uq-autostart-*'` shows
     the scope under `app.slice`; `journalctl --user -t uq-autostart-<id>` has its output.
     **Startup** shows it as Running. Turn it off and log out and in: it doesn't start.
  2. **System entry off.** Turn off a `/etc/xdg/autostart` entry. Check that
     `~/.config/autostart/<id>` has `Hidden=true` and `X-uQ-Autostart=true`. Turn it on: the
     file is gone. An entry `OnlyShowIn=GNOME;` shows "Not for this desktop".
  3. **Once per session.** `systemctl --user restart uqd` doesn't start a second copy of any
     app, nor one you turned on since login. Kill an autostarted app, restart uqd, and the page
     says "Started earlier; finished".
  4. **Failure.** An entry with `Exec=sh -c "echo boom; exit 3"` shows "Failed (exit 3)" with
     `boom` in its log lines, and none from an earlier login.
  5. **Delay.** A 10 s delay starts the app about 10 s after the shell appears.
  6. **§1 criterion 5.** At 125 % scale, turn on **Run Electron apps on Wayland** and start
     VS Code or another repo Electron app from Spotlight. It's sharp, and `hyprctl clients`
     shows `xwayland: 0`.
  7. **Sharp X11 apps.** Turn it on. A new `xeyes`/`xterm` is sharp (`getoption
     xwayland:force_zero_scaling` is 1); `GDK_SCALE` is in a new app's environment
     (`cat /proc/<pid>/environ`) and not in `~/.config/environment.d/60-uq.conf`. Record whether
     GTK Wayland apps changed size (R12/R14 live item).
  8. **Fix this app.** Open an X11-only app. It's in **Blurry right now**. Fix it with Run on
     X11 and size 150 %, close and reopen it from the launcher, and its environment has
     `QT_SCALE_FACTOR=1.5` / `GDK_SCALE=1`. `uqctl launch <id>` does the same.
  9. **Elsewhere.** Turn on "Also when started from other launchers". Check that
     `~/.local/share/applications/<id>.desktop` has the marker and an `env --` Exec, and that
     another launcher (`gtk-launch <id>`) gets the fix. Stop uqd and launch from Spotlight: the
     fix still applies (R19). Remove the fix: the file is gone. Put your own copy there first,
     and the switch is refused with the plain message.
  10. **Chromium.** With Chromium installed, the switch writes `~/.config/chromium-flags.conf`
      with the marker; with your own file there, it's left alone and the page says so. Record
      which flag files each installed browser really reads, and whether
      `--ozone-platform-hint=auto` still works.
  11. **Environment variables.** Add `FOO=bar`. A new terminal from Spotlight has it; a
      `systemctl --user show-environment` has it (R8); after logout and login
      `~/.config/environment.d/60-uq.conf` provides it. `XDG_SESSION_TYPE`, `XDG_CONFIG_HOME`
      and `A$B` are refused plainly. Add `hl.env("FOO", "x")` to `user.lua` and reload: the row
      says "Overridden in user.lua, line N". `user.lua`'s modification time is unchanged by
      every step of this checklist.
  12. **History and restore.** Undo a switch, a fix and a startup toggle from **History**; each
      row reads plainly and the files follow (the override and the hidden copy appear and go).
      Restore yesterday's point, and all three pages match it.
  13. **Profiles.** Make a profile "Work" from the Startup and Apps and scaling pages (a
      variable, a switch, a fix's size, a startup entry off). Apply it: rows say "From the Work
      profile", History shows the entries "from the Work profile", a new terminal has the
      variable, nothing starts or stops, and the notice says startup apps change at next login.
      Back to normal puts everything back. Import a hand-edited profile with `argv = ["x"]` under
      `[autostart."a-1234"]`: it's refused plainly.
  14. **Dev session.** In `uq-dev session`, no autostart app starts, the page says "Not started
      in a test session", and "Also when started from other launchers" is refused.
- [ ] **Step 4:** Run the full gate, exactly as CI does:
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features --locked -- -D warnings`;
  - `cargo test -j 2 --workspace --locked -- --include-ignored --test-threads=2`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh` (with `pam_stacks.sh`'s known flake handled as in Global Constraints);
  - both `qmltestrunner` suites.

  In the build container (K4), run everything without Qt (`--exclude uq-settings`) and without
  `--include-ignored`. Run the page logic and registry tests through t7check. Leave
  `uq-settings`, the QML suites, the Quickshell-based shell tests and the `--ignored` Hyprland
  verify to CI. Expected: all PASS.
- [ ] **Step 5:** Write the PR body's "Spec deviations" section from the list near the top of
  this plan. Commit (K5 trailers): `docs: settings slice D — startup apps, environment variables, apps and scaling`
