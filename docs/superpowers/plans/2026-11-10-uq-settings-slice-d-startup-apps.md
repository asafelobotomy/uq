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
switch that `user.lua` overrides says so.

**Architecture:**
- **uq-core:**
  - `desktop_entry.rs` is the one Desktop Entry parser. It moves out of the bridge and gains
    field codes, `OnlyShowIn`/`NotShowIn` and an `Exec` writer.
  - `env.rs` is the `env` domain: validation, the blocklist, and the `environment.d` text.
  - `compat/` is the `compat` domain:
    - `compat/hypr.rs`, the Hyprland and toolkit name table;
    - `compat/mod.rs`, the model and validation;
    - `compat/launch.rs`, the per-app argv builder that both the bridge and uqd use;
    - `compat/browsers.rs`, the Chromium flag-file table.
  - `autostart/` is the `autostart` domain plus the pure folder scan and run plan.
  - `lua/startup.rs` renders the xwayland option and the `hl.env` lines.
  - `startup_explain.rs` holds the pure `Explain` rows for all three domains, which uqd and the
    fake uqd share.
- **uqd:**
  - The three domains become section domains (`GetSection`/`SetSection`/`Explain`/`Reapply`).
  - `files.rs` keeps every file outside `state.toml` in step with state:
    `environment.d/60-uq.conf`, Chromium flag files, desktop-file overrides, and the hidden
    copies of autostart entries.
  - `autostart.rs` is the runner. It adds `ListAutostart() → s`, `ShellReady()` and the signal
    `AutostartChanged()`.
- **uqctl:**
  - The bridge's `Launch` applies per-app fixes and calls `ShellReady` once the shell has said
    `Hello`.
  - New command `uqctl launch <desktop-id>`, the path a `launch` keybind will use.
- **uq-settings:**
  - Qt-free page logic in `pages/startup.rs` and `pages/compat.rs`, tested against the fake uqd;
  - thin `startup_model.rs` and `compat_model.rs`;
  - `StartupPage.qml`, `AppsScalingPage.qml` and their components;
  - two registry entries.

**Tech Stack:** Rust edition 2024, zbus 5, tokio, cxx-qt 0.10, Qt 6, Kirigami 6, Quickshell,
Hyprland 0.56.2–0.57.x (Lua config), systemd user manager (`systemd-run`, `systemd-cat`,
`journalctl`).

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. The authority
is §6 (Apps and scaling) and §7.3 (autostart and environment variables). This plan also covers:
- §2: the `user_lua` and `error` sources for the new rows, and Explain;
- §3: history entries, labels and restore points for `env`, `compat` and `autostart`;
- §9: the quality rules;
- §10: the D-Bus summary `ListAutostart`, `SetAutostart` and `AutostartChanged`, and its
  security lines;
- §11 decisions 3 and 4;
- §12 row D.

It builds on slice A (`2026-11-10-uq-settings-slice-a-trust.md`, merged) and follows the layout
of slices B and C (`…-slice-b-profiles.md` and `…-slice-c-rules.md`, merged at `ac1cd3f`).

**Paths.** The settings app lives at `apps/uq-settings` (not `crates/`). Its QML tests are in
`tests/settings-qml`, and the shell's are in `tests/qml`.

## Pre-flight rulings

A read of the code at `ac1cd3f` against the spec found the issues below. Each is decided here
and folded into the task text; this list is the index.

- **R1, profiles (decided here).** `env`, `compat` and `autostart` **can't** be in a profile in
  this slice.
  - `PROFILE_DOMAINS` is unchanged, so a profile file with `[env]`, `[compat]` or `[autostart]`
    is refused as an unknown domain ("This profile has settings uQ doesn't know: …").
  - `profile::from_journal` already gives `None` for them; Task 6 pins both with tests.
  - Reasons:
    - env changes reach only apps started afterwards, so a profile switch would half-apply;
    - per-app argument lists and autostart commands are code-adjacent;
    - spec §4's list doesn't name them.
- **R2, no `SetAutostart` method.** Writes go through `SetSection("autostart", json, origin)`,
  like every section. Slice C set the precedent: it added no rule-specific write methods. The
  runner and the file sync react to the committed state. Spec deviation 1.
- **R3, three section domains.** `env`, `compat` and `autostart` are `State` fields beside
  `rules`, not `Settings` fields (slice C's F21 ruling). So `SettingsChanged` doesn't carry them,
  and the bridge reads `compat` with `GetSection` when it needs it. `SECTION_DOMAINS` grows from
  8 to 11.
- **R4, one Desktop Entry parser.** The bridge's `desktop_entry.rs` moves to
  `uq_core::desktop_entry`. The bridge, uqd's runner, the override writer and the hidden-copy
  writer all use it. `uqctl::bridge::desktop_entry` becomes a re-export, so the existing launch
  tests stay unchanged.
- **R5, the shell is "ready" when the bridge has its `Hello`.** uqd has no shell-ready signal
  today. The bridge calls the new `ShellReady()` once per uqd connection, after the shell's
  `Hello`. If no call comes, uqd starts autostart 20 s after it starts. No QML change.
- **R6, uqd is the parent of each autostart process.** `systemd-run --scope` execs the
  command, so uqd's child *is* the app, in its own scope.
  - uqd waits on the child for the exit status.
  - `systemd-cat -t uq-autostart-<id>` puts the app's output in the journal under its own tag,
    so the last lines can be read back.
  - The scope moves the process out of `uqd.service`'s cgroup, so restarting uqd never kills
    autostarted apps.
- **R7, once per session.** `$XDG_RUNTIME_DIR/uq/autostart-run.json` records
  `{session, started: {id: {unit, pid, at}}}`. `session` is `HYPRLAND_INSTANCE_SIGNATURE`.
  - A restarted uqd doesn't run an entry that the record lists for the same session.
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
- **R13, the "main monitor"** is the enabled output that Hyprland reports at position 0,0. If
  there's none, it's the first enabled output by name. Its *live* scale (`j/monitors`) is used.
  Slice F's `monitors.primary` replaces this rule when it lands. With Hyprland down, the
  scale-derived lines (`GDK_SCALE`, `STEAM_FORCE_DESKTOPUI_SCALING`) are left out.
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
  - The file sync rebuilds the marked copies from state after every commit (including undo and
    restore) and at startup.
- **R16, added entries are stored, not written as files.** An added app (desktop id) or command
  (argv) lives in `autostart.added`. Commands are split with the Desktop Entry quoting rules and
  stored as argv, so they never pass through a shell.
- **R17, `ListWindows` is reused** for "Blurry right now" (slice C, `crates/uqd/src/rules.rs`
  `Desktop::windows_of`), filtered on `xwayland`. Window class is matched to a desktop id in the
  app, from installed desktop files (`StartupWMClass`, then file id, then the `Exec` program's
  name). No new uqd call.
- **R18, overrides are re-checked on uqd's 60 s tick**, with no file watcher. That covers a
  system desktop file changed by an upgrade. The sync recomputes each wanted override from its
  source and rewrites it only when the text differs. It's at most 100 small reads a minute,
  and the check also runs at startup and after every commit.
- **R19, a marked override is never applied twice.** When the bridge's lookup finds a desktop
  file with `X-uQ-Override=true`, it launches from the `X-uQ-Source` file that the override
  names, and applies the fix itself. The source must be a validated system path. If it isn't,
  the bridge launches the override's `Exec` as written, with no second fix.
- **R20, a dev session doesn't touch the real desktop.** With `UQ_DEV_SESSION` set:
  - the runner starts nothing (status "Not started in a test session");
  - desktop-file overrides are refused ("Not available in a test session"), because
    `XDG_DATA_HOME` isn't sandboxed there (`docs/developing.md`);
  - `hl.env` doesn't export (R8).
- **R21, `uqctl launch <desktop-id>`** is added. Spec §6 says `launch` keybinds use uQ's launch
  path, but no `launch` keybind exists yet (no keybinds domain; `base.lua` binds `exec_cmd`
  directly). This command is that path, for Phase 3's keybind editor and for live checks.
- **R22, scopes name their slice.** Both the bridge's `scope_command` and the runner pass
  `--slice=app.slice`. uqd runs in `session.slice` (`packaging/uqd.service`), and the slice must
  not depend on systemd's default for transient units. The existing launch tests are updated
  for the new flag.

**Spec deviations (recorded here and in the PR):**
1. No `SetAutostart` (R2). Autostart writes use `SetSection("autostart")`.
2. "Sharp X11 apps" doesn't set `QT_SCALE_FACTOR` globally (R12).
3. Two D-Bus calls the spec doesn't list: `ShellReady()` (R5) and `ListAutostart() → s`. The
   latter *is* in the spec, but its shape is decided here.
4. Values can't hold `$` (R10). The spec allows any plain string.
5. The env blocklist adds a few names to the spec's list (see "The env domain").
6. Per-app fixes and autostart entries share one launch builder. A "Fix this app" fix also
   applies when that app is autostarted. The spec names only uQ's launch path; this is a
   consistent extension.

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
    (and `pages::compat`, `registry`).
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
  - `cargo fmt`, `clippy -D warnings` and the tests must be clean;
  - keep files at or under about 400 lines;
  - run builds in the foreground and leave nothing running.
- **CI flake.** `tests/shell/pam_stacks.sh` is known to flake in CI. If it's the only red job,
  re-run it rather than chasing it in this slice. Every other failure is real.
- **No new crates.** Hashing, inotify and systemd clients aren't needed (R7, R8, R18). Process
  spawning uses `tokio::process`, which uqd already enables.

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
    - `UQ_*`, since `UQ_DEV_SESSION` turns off `base.lua`'s session hooks;
    - `HOME`, `USER`, `LOGNAME`;
    - `XCURSOR_THEME`, `XCURSOR_SIZE`, `HYPRCURSOR_SIZE` (the Appearance page sets these);
    - `QT_QPA_PLATFORMTHEME` (`base.lua` sets it for uQ's Qt colours).

    The refusal for these last four says "This is set by the Appearance page."
- **Warned names:** `LD_PRELOAD`, `LD_LIBRARY_PATH` and `PATH` are allowed. The page shows a
  warning under the row: "This can stop apps from starting. Remove it if apps break." Nothing
  extra is confirmed.
- **Per-app fixes reuse this check.** `compat.apps[].env` entries go through the same
  `env::check_var` function (name, value, blocklist), and the same warnings apply.
- **Where it applies (§7.3):**
  - `hl.env(name, value, export)` lines in `generated.lua` (R8), so `renders_section("env")` is
    true and `change_for("env")` is `Change::Reload`;
  - `~/.config/environment.d/60-uq.conf`, written by uqd's file sync (Task 7).
- **`environment.d` text (ruling).**
  - The file starts with the header
    `# Written by uQ (Startup page). Changes here are replaced; use uQ Settings.`
  - Then there's one line per variable: env vars first, then compat's, with a compat name
    replacing an env one (R11).
  - Each value is single-quoted: `NAME='value'`.
  - A value holding `'` is double-quoted instead, with `\`, `"` and `` ` `` backslash-escaped.
    `$` can't occur (R10), so systemd's expansion never fires.
  - The test `environment_d_round_trips` parses the output with a small reference parser that
    follows `systemd`'s `env-file.c` rules for these two quoting forms.
  - **Ruling:** uqd owns this file name and rewrites it whether or not it carries the header,
    like `generated.lua`.
- **The page says (§7.3):** "Changes apply to apps started after the change, and fully after you
  log out and back in." The §6 switches are listed read-only, with a link to Apps and scaling.

### The compat domain (§6)

- **Shape (ruling).** The section JSON is:
  ```
  { "sharp_x11": false, "electron_wayland": false, "chromium_wayland": false,
    "qt_wayland": false, "steam_scale": false,
    "apps": [ { "id": "code.desktop", "platform": "auto", "scale": null,
                "env": [{"name": "…", "value": "…"}], "args": ["…"], "elsewhere": false } ] }
  ```
  - `apps` is a list (one history leaf, as rules are), at most 100 entries, unique by `id`.
  - Ids pass `settings::valid_pinned_id` and are normalised to end in `.desktop`.
    - **Ruling:** they also must not start with `.` or contain `..`, so the joined path's file
      name is always the id itself (Task 1's `valid_desktop_id`).
  - `platform` ∈ `auto | wayland | x11`.
  - `scale` is `null` or 1.0–3.0 in steps of 0.25.
  - `env`: at most 20 entries, each checked by `env::check_var`.
  - `args`: at most 20 strings, each 1–1024 chars, with no control characters.
    - **Ruling:** an arg can't contain `=` *and* be the first arg. That would never come up for
      an extra arg; the rule protects the `env` wrapper (Task 3).
  - An app entry with nothing set (`auto`, `null`, no env, no args) is refused: "This fix
    doesn't change anything."
- **Global switches (§6), as the name table renders them** (`compat/hypr.rs`, R12–R14):

  | Switch | `generated.lua` | env (both places) |
  |---|---|---|
  | `sharp_x11` "Sharp X11 apps" | `xwayland = { force_zero_scaling = true }` | `GDK_SCALE=<R14 of main scale>` |
  | `electron_wayland` "Run Electron apps on Wayland" | — | `ELECTRON_OZONE_PLATFORM_HINT=auto` |
  | `chromium_wayland` "Run Chromium-based browsers on Wayland" | — | — (flag files, below) |
  | `qt_wayland` "Qt apps prefer Wayland" | — | `QT_QPA_PLATFORM=wayland;xcb` |
  | `steam_scale` "Steam UI scale" | — | `STEAM_FORCE_DESKTOPUI_SCALING=<main scale, ≤2 places>` |

  Each switch's description is one plain sentence ending in "Applies to apps started after the
  change."
- **Per-app fix (§6), the launch argv (`compat::launch::argv`).** The inputs are:
  - the entry's argv (from `Exec`, field codes handled);
  - the fix;
  - the desktop id;
  - the main scale.

  The output is:
  `["env", "--", "K=V", …, <exec argv…>, <browser args…>, <extra args…>]`, or the argv
  unchanged when the fix sets no env. The pieces, in order:
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

  It's refused (`CompatError::Unsafe`) when the exec argv's first element contains `=`, since
  GNU `env` would read it as an assignment. The launch then goes ahead without the fix, and the
  bridge logs it.
- **"Also when started elsewhere" (`elsewhere = true`, §6, desktop-file overrides).** Task 7's
  file sync writes `$XDG_DATA_HOME/applications/<id>`, only when:
  - the target doesn't exist, or it exists, is a regular file (not a symlink), and its
    `[Desktop Entry]` group has `X-uQ-Override=true`;
  - a source exists: the first `applications/<id>` in `$XDG_DATA_DIRS`. The user's own folder
    never counts as a source.

  The content (`compat::override_text(source_text, source_path, fix, id, main_scale)`):
  - The source text is copied line for line.
  - Every `Exec=` key in `[Desktop Entry]` and in each `[Desktop Action *]` group becomes
    `Exec=env "K=V" … <original tokens, browser args and extra args inserted before the first
    %f/%F/%u/%U token>`.
    - It's built from `desktop_entry::exec_tokens` and written with `desktop_entry::write_exec`
      (Task 1).
    - Field codes are kept, so other launchers still pass files.
  - `DBusActivatable=true` becomes `DBusActivatable=false`, or launchers would skip `Exec`.
  - `X-uQ-Override=true` and `X-uQ-Source=<absolute source path>` are added to
    `[Desktop Entry]`.
  - A header comment comes first:
    `# Written by uQ Settings (Apps and scaling). Remove the fix there; uQ replaces this file when the app updates.`
  - With no env, no browser args and no extra args, the file isn't written. A fix that only
    sets `auto`/`null` can't be saved anyway.
  - **Writes:** `atomic_write` (temp file and rename in the same folder). A symlink at the
    target is never followed or replaced: it counts as "not ours".
  - **Deletion:** when the fix is removed, or `elsewhere` is turned off, uqd deletes the file.
    It re-checks the marker immediately before `remove_file` and never deletes an unmarked
    file.
  - **Refusals before commit (ruling, §9 "no silent failure").**
    `SetSection("compat")` refuses turning `elsewhere` on when:
    - an unmarked file already exists: "You already have your own copy of this app's file, so
      uQ won't replace it.";
    - there's no source: "uQ can't find this app's file.";
    - the session is a dev session (R20).

    Later failures (I/O, a source removed by uninstall) are reported by Explain (Task 6), never
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
    browser. The first line is the marker
    `# Managed by uQ (Apps and scaling). uQ only changes files that start with this line.`,
    then `--ozone-platform-hint=auto`.
  - An existing file whose first line isn't the marker is never touched. Explain reports it
    (detail `own_file`), and the page says "Brave has its own flags file, so uQ left it alone."
  - Turning the switch off deletes only marked files, re-checking the marker immediately
    before deleting.
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
    `delay` set.
  - `added` has at most 50 entries:
    - `id` is `^a-[0-9a-f]{4,8}$`, made by uq-core like rule ids (`rules::fresh_id`'s
      generator, with the prefix as a parameter) and assigned when missing.
    - `name` is 1–100 chars of plain text.
    - Each entry has exactly one of `app` (a valid desktop id) or `argv`. `argv` has 1–64 args,
      each 1–1024 chars with no control characters, and the first arg is non-empty with no `=`.
  - `delay` is `null` or 0–600 seconds.
- **The folders (§7.3, §10).**
  - `$XDG_CONFIG_HOME/autostart` (the user's, default `~/.config/autostart`);
  - then each `$XDG_CONFIG_DIRS` entry plus `/autostart` (default `/etc/xdg/autostart`).
    **Ruling:** honouring `XDG_CONFIG_DIRS` *is* the XDG system folder.
  - Only files named `<valid_desktop_id>` ending in `.desktop` are read: at most 200 per
    folder, at most 64 KiB each.
  - Symlinks are followed for reading only.
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
  6. `Exec` must parse (Task 1). Skip reason: "Its command can't be read".
  7. Enabled: the state's `enabled` if set, else `X-GNOME-Autostart-enabled` (absent means
     true). When off, the status is `off`.
  8. Delay: the state's `delay` if set, else `X-GNOME-Autostart-Delay` (clamped to 0–600),
     else 0.
- **How each runs (§7.3, R6, R22).** It's spawned directly (no shell) with `tokio::process`:
  ```
  systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-autostart-<sid>-<rand8hex>
      -- systemd-cat -t uq-autostart-<sid> -- <argv from compat::launch::argv>
  ```
  - `<sid>` is the id sanitised exactly as `scope_command` does now: `[A-Za-z0-9_-]`, others
    become `_`, at most 64 chars. That function moves to `uq_core::desktop_entry::unit_id`.
  - `argv` comes from `exec_argv` with the field codes for no files: `%f %F %u %U` are
    dropped; `%i` becomes `--icon <Icon>` if set; `%c` becomes the `Name`; `%k` becomes the
    file path. A per-app compat fix for the same id applies (spec deviation 6).
  - For an added app, the desktop file comes from the applications dirs, as the launcher finds
    it.
  - Working directory: `Path=` when it's an absolute existing directory, else `$HOME`.
  - Environment: uqd's own, plus `startup::session_env(state, theme, main_scale)`. That's the
    exact list that `generated.lua` renders with `hl.env` (cursor, env, compat), so autostarted
    apps see what shell-launched apps see.
- **Status** (`ListAutostart`, per entry):
  - `waiting` (delay not over);
  - `running`;
  - `exited` (status 0);
  - `failed` (non-zero or a signal), with `exit` and `log`: the last 10 journal lines from
    `journalctl --user -t uq-autostart-<sid> -n 10 -o cat --no-pager -q`, read only for failed
    entries, each cleaned with `clean_text_spaced` and cut to 200 chars;
  - `off`;
  - `skipped` (with `reason`);
  - `not_started`: on, but not run this session ("Starts next time you log in");
  - `unknown_exit` ("Started earlier; finished"): a record from before a uqd restart whose pid
    is gone, so the exit code is unknown.
- **When.** On `ShellReady()` or 20 s after uqd starts, whichever is first, once per uqd
  process, subject to R7.
  - `SetSection("autostart")` never starts or stops anything. Turning an entry on reads
    "Starts next time you log in", and turning one off doesn't close the running app.
  - **Ruling:** there's no "Start now" button in v1 (notes question 4).
- **Not in a dev session** (R20).
- **Security (§10).** The runner runs only effective entries from the folders above and
  `autostart.added`, each in its own scope.
  - The argv never passes through a shell. The ids, the unit name and the journal tag are
    sanitised.
  - Profiles can't carry autostart (R1), and imports can't add it.
  - Any session client can call `SetSection("autostart")`, as it can write
    `~/.config/autostart` itself, so this adds no new reach. Review Focus 4.
- **The hidden copy (R15).** `autostart::hidden_copy(system_text)` copies the system file,
  sets `Hidden=true` (replacing an existing `Hidden=` line in `[Desktop Entry]`) and adds
  `X-uQ-Autostart=true`. A header comment comes first:
  `# Written by uQ Settings: turned off on the Startup page. uQ removes this file when you turn it back on.`
  The file sync (Task 7) keeps one for each system entry that is off in state and has no user
  file. It deletes marked copies that are no longer wanted, re-checking the marker immediately
  before deleting.

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
  Explain and the page's catalogue all read them from there. If a live check fails, the fix is
  one row.

### Sources and history (§2, §3)

- **Explain (§2).** Three pure functions in `uq_core::startup_explain` are shared by uqd and the
  fake uqd (slice C's F7 pattern). Rows are `{source, live, stored, detail}`.
  - `explain_env(state, user_lua, rejected, file_issue, path)`. There's one row per variable,
    keyed `vars.<NAME>`:
    - `error` when the last `env` write was rejected by verification, or the
      `environment.d` write failed (detail `{message}`);
    - `user_lua` (with `line` and `path`) when `user.lua` holds `hl.env(` followed by the
      quoted name. The scan uses the shared comment-cutting helper; F8 says `--` never counts.
    - otherwise `uq`, with detail `{replaced_by: "<switch key>"}` when R11 applies.

    It also gives a section row `"env"` with `error` after a rejected write (slice C's F9).
  - `explain_compat(state, user_lua, rejected, files, path)`:
    - `sharp_x11` comes from slice A's generic option path (a new `provenance::OPTIONS` row,
      `compat.sharp_x11` → `xwayland:force_zero_scaling`, `NullSource::Default`).
      `provenance::expected` learns the `compat` section, so `runtime` works through
      `getoption`.
    - Rows for the env-based switches check `user.lua` for each of their names (`user_lua`).
    - `chromium_wayland` has detail
      `{browsers: [{name, file: "ours" | "own_file" | "none" | "error", message?}]}`.
    - One `apps.<id>` row per fix. Its detail is
      `{override: "written" | "own_file" | "no_source" | "error" | "off", message?}`. The
      source is `error` for `error` and `no_source` while `elsewhere` is on.
    - There's a section row `"compat"` after a rejected write.
  - `explain_autostart(state, files)`: a section row `"autostart"` with `error` when a hidden
    copy couldn't be written or removed, else `uq` rows by entry id.
  - **Ruling:** there's no `runtime` source for env or the env-based switches. A running
    process's environment isn't something uqd reads.
  - **Ruling:** these domains have no `profile` source (R1).
- **History (§3).** All three are journal domains through `SECTION_DOMAINS`. Summaries never
  print values (env values can be secrets) or JSON:
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
  - **Labels:** `history::label` gains these (Task 6):
    - `("env", "vars")` → "Environment variables";
    - `("compat", "sharp_x11")` → "Sharp X11 apps";
    - `("compat", "electron_wayland")` → "Run Electron apps on Wayland";
    - `("compat", "chromium_wayland")` → "Run Chromium-based browsers on Wayland";
    - `("compat", "qt_wayland")` → "Qt apps prefer Wayland";
    - `("compat", "steam_scale")` → "Steam UI scale";
    - `("compat", "apps")` → "App fixes";
    - `("autostart", "files")` → "Apps that start at login";
    - `("autostart", "added")` → "Your own startup apps".

    They land in the same task that adds each domain to `SECTION_DOMAINS` (Tasks 2–4), because
    `every_key_a_summary_can_show_has_a_plain_label` needs them at once (slice C's F11). Task 6
    only adds the summaries.
  - `domain_label` gains "Environment variables", "Apps and scaling" and "Startup apps".
  - Undo puts back a whole list (`vars`, `apps`, `files`, `added`), as for rules (F36). The
    README says so.

### Launch path (§6, §11 decision 4)

- **What's "uQ's launch path".** The bridge's `Launch {desktop_id}`
  (`crates/uqctl/src/bridge/launch.rs::plan`), used by:
  - the app menu (`shell/launcher/AppMenu.qml`);
  - Spotlight (`shell/logic/launcher.js`);
  - pins (`shell/bar/SystemIsland.qml`, `shell/logic/dock.js`);
  - from Task 9 on, `uqctl launch` (R21).

  `Launch {argv}` and `Launch {run_id}` have no desktop id, so no fix applies.
- **Ruling, how the bridge gets the fix.**
  - Per launch, `link.call("GetSection", ["compat"])`, bounded by 500 ms. A failure, a timeout
    or uqd being down means no fix: a launch never fails over a fix.
  - The fix is matched by normalised id.
  - The main scale isn't needed: per-app scale is explicit.
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
  - Autostart children get it from `session_env` (above).
  - Already-running apps keep their environment. The page says so, and removals linger (R9).

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
    `summaries_are_plain` (Task 6) cover it.
- **Untrusted text.** These come from other packages, other apps or the user, and from
  `state.toml` or a restore point:
  - desktop-file `Name`s;
  - window titles and classes;
  - journal lines;
  - env names and values;
  - args and commands.

  All of it is shown through `UqLabel`, never a stock control's `text:`, a
  `Kirigami.InlineMessage`, a `ToolTip`, a `Kirigami.FormData.label` or a `Dialog.title`
  (slice C's F28).
  - Dialog titles are fixed literals: "Fix this app", "Add an app", "Add a command" and
    "Add a variable".
  - Messages use the existing plain-text `components/RuleMessage.qml`. **Ruling:** it's
    reused as is; it holds generic lines.
- **Values aren't secrets, but they're kept quiet.** Summaries and logs never print env values.
  `ListAutostart` returns the argv of added commands only to the page (the session bus, same
  user). The journal (`~/.local/state/uq/history`) stores before/after values just as
  `state.toml` stores them.

## Review Focus

1. **Injection into `generated.lua`, `environment.d` or `Exec`.**
   - An env value can hold `"`, `\`, `'`, `]]`, or `") hl.exec_cmd("x`.
   - An arg or command can hold quotes, `%`, `;` or spaces.

   Every Lua string goes through `lua_string`. Env names are `[A-Z_][A-Z0-9_]*`, so Hyprland's
   `import-environment '<name>'` can't be broken out of. The `environment.d` quoting
   round-trips. The `Exec` writer round-trips through `exec_argv` for every arg.
   Tests: Task 5's `hostile_env_stays_inside_lua_strings`, Task 2's `environment_d_round_trips`
   and `refuses_hostile_vars`, and Task 1's `write_exec_round_trips`.
2. **Touching files uQ didn't make.** These cases must hold:
   - an unmarked desktop file at the override path;
   - a symlink there;
   - an unmarked Chromium flags file;
   - an unmarked user autostart file;
   - a marked file replaced by an unmarked one between check and delete.

   In every case the file is never changed or deleted. Task 7's `never_touches_unmarked_*`
   tests cover them.
3. **Path traversal.** Ids like `../x`, `..desktop`, `.hidden.desktop`, `a/b.desktop` and
   `x.desktop\0` are refused by `valid_desktop_id` before any path is built. Every written path
   is re-checked to have the folder as its parent and the id as its file name. Task 1's
   `desktop_ids_refuse_traversal` and Task 7's `override_path_stays_in_folder` test it.
4. **Autostart runs only what it should, once.** These cases must hold:
   - an entry `OnlyShowIn=GNOME;`;
   - a `TryExec` that's missing;
   - `Hidden=true`;
   - `Terminal=true`;
   - an `Exec` with `%d`;
   - an entry off in state;
   - a restarted uqd in the same session;
   - a new session while the old pid still lives;
   - a dev session.

   None of these starts. A good entry starts once, with the delay, in its own
   `app.slice` scope, without a shell. Task 4's `scan_*` and Task 8's `runner_*` test it.
5. **Blocked env names and launch safety.** `XDG_SESSION_TYPE`, `HYPRLAND_FOO`, `UQ_DEV_SESSION`,
   `XCURSOR_SIZE`, a lowercase name and a name with `=` are refused, in `env` and in per-app
   fixes alike. `LD_PRELOAD` is warned, not refused. A fix whose program's argv[0] contains
   `=` launches without the fix rather than as an assignment. Tasks 2, 3 and 9 test it.
6. **The launch path stays reliable.** With uqd down, slow (over 500 ms) or answering garbage,
   `Launch` still launches, without the fix. A marked override never gets the fix twice
   (R19). Task 9 tests `launch_*`.

---

### Task 1: `uq_core::desktop_entry`: one parser for `Exec`, entries and ids

**Files:**
- Create: `crates/uq-core/src/desktop_entry.rs` and `crates/uq-core/src/desktop_entry/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs` (`pub mod desktop_entry;`);
  - `crates/uqctl/src/bridge/desktop_entry.rs` (it becomes
    `pub use uq_core::desktop_entry::*;` plus `Dirs`, `binary_exists` and its existing tests);
  - `crates/uqctl/src/bridge/launch.rs` (`scope_command` calls
    `uq_core::desktop_entry::unit_id`; add `--slice=app.slice`, R22);
  - `crates/uqctl/src/bridge/launch_tests.rs` and the `run.rs` test (expected strings gain
    `--slice=app.slice`);
  - `scripts/smoke-shell.sh` (nothing to change: it matches `app-uq-sleep-*.scope`; confirm).

**Interfaces (produced, `uq_core::desktop_entry`):**
- `pub enum Token { Arg(String), Field(char) }`.
- `pub fn exec_tokens(exec: &str) -> Result<Vec<Token>, ExecError>`.
  - It uses today's quoting rules (`"`, and `\` escaping `" ` $ \` inside quotes) and `%%`.
  - A field code that's a whole argument becomes `Field(c)`. One inside a word (`--f=%F`) keeps
    today's behaviour: it's dropped from the word.
  - Deprecated or unknown codes are errors.
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
  - Then the key-value escape is applied (`\` → `\\`). Newlines can't occur: callers refuse
    control characters.
- `pub struct Entry { pub kind: Option<String>, pub name: Option<String>, pub icon: Option<String>, pub exec: Option<String>, pub try_exec: Option<String>, pub terminal: bool, pub terminal_arg: Option<String>, pub hidden: bool, pub only_show_in: Option<Vec<String>>, pub not_show_in: Option<Vec<String>>, pub dbus_activatable: bool, pub path: Option<String>, pub wm_class: Option<String>, pub gnome_enabled: Option<bool>, pub gnome_delay: Option<u32>, pub uq_override: bool, pub uq_source: Option<String>, pub uq_autostart: bool }`.
  - `pub fn read_entry(text: &str) -> Entry` reads only `[Desktop Entry]`. Lists are
    `;`-separated with the trailing `;` optional, and values are unescaped.
  - The existing `parse_entry` keeps its contract (it refuses hidden and non-Application
    entries) on top of `read_entry`.
- `pub fn shows_in(e: &Entry, desktops: &[&str]) -> bool`, following the Desktop Entry spec:
  `OnlyShowIn` needs an overlap, and `NotShowIn` refuses one.
- `pub const UQ_DESKTOPS: [&str; 2] = ["Hyprland", "uQ"]`.
- `pub fn valid_desktop_id(id: &str) -> bool`: `valid_pinned_id`, ends in `.desktop` after
  normalising, doesn't start with `.`, doesn't contain `..`.
  `pub fn normalise_id(id: &str) -> String` adds `.desktop`.
- `pub fn unit_id(app_id: &str) -> String`: today's sanitiser from `scope_command`.
- `pub fn set_key(text: &str, group: &str, key: &str, value: &str) -> String`: replaces or
  appends one key in a group, keeping every other line byte for byte. The override and
  hidden-copy writers use it.
- `pub fn groups_with_exec(text: &str) -> Vec<String>`: `Desktop Entry` and each
  `Desktop Action *` group that has an `Exec`.

- [ ] **Step 1: Write the failing tests** in `desktop_entry/tests.rs`:
  - `exec_tokens_keep_field_codes` (`code %F` → `[Arg(code), Field('F')]`);
  - `exec_argv_expands_icon_name_and_path`: `app %i %c %k` with icon `x`, name `My App`, path
    `/p` gives `["app", "--icon", "x", "My App", "/p"]`, and with no icon `%i` gives nothing.
  - `exec_argv_matches_parse_exec_cases` (the three existing bridge cases);
  - `split_command_keeps_percent` (`"echo 100%"` → `["echo", "100%"]`);
  - `write_exec_round_trips`: for a fixed list of hostile args, `exec_argv(unescape(write_exec(tokens)))`
    gives them back. The args are `a b`, `"`, `\`, `$HOME`, `` `x` ``, `50%`, `''`, `;rm -rf`,
    `é`, `""` and `--x=%U`.
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
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -p uqctl -- --test-threads=2`. Expected:
  PASS, with the launch tests passing once their expected strings have `--slice=app.slice`.
- [ ] **Step 5:** Commit (K5 trailers): `refactor(core): one Desktop Entry parser with field codes, an Exec writer and safe ids`

### Task 2: `uq_core::env`: the environment variables domain

**Files:**
- Create: `crates/uq-core/src/env.rs` and `crates/uq-core/src/env/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/state.rs` (`pub env: Env`, lenient load, `section`/`set_section`
    dispatch for `"env"`, `parse_strict` calls `Env::repaired`);
  - `crates/uq-core/src/settings/mod.rs` (`SECTION_DOMAINS` gains `"env"`;
    `renders_section("env")` is true);
  - `crates/uq-core/src/history/labels.rs` (the `("env", "vars")` label, F11);
  - `apps/uq-settings/src/registry.rs` (`every_journal_domain_has_one_page` skips `env` until
    Task 12, as slice C's F11 did).

**Interfaces (produced):**
- `pub struct Var { pub name: String, pub value: String }`.
- `pub struct Env { pub vars: Vec<Var> }`, with `Default`, `Serialize`, and a lenient
  `Deserialize`.
- `pub const MAX_VARS: usize = 100;`, `pub const MAX_VALUE_BYTES: usize = 4096;`.
- `pub enum Warn { BreaksApps }`.
- `pub fn check_var(key: &str, name: &str, value: &str) -> Result<Option<Warn>, SettingError>`.
  `key` is the field key for messages (`env.vars[3]` or `compat.apps[1].env[0]`).
- `pub fn blocked(name: &str) -> Option<&'static str>` returns the plain reason.
- `impl Env { pub fn from_section(v: &Value) -> Result<Env, SettingError>; pub fn repaired(self) -> Env; pub fn section(&self) -> Value; pub fn warnings(&self) -> Vec<(String, Warn)> }`.
- `pub fn environment_d(lines: &[(String, String)]) -> String`: the header and the quoting from
  Global Constraints.

- [ ] **Step 1: Write the failing tests** in `env/tests.rs`:
  - `names_follow_the_rule` (`A`, `_X`, `GDK_SCALE2` pass; `a`, `1A`, `A-B`, `A=B`, `""`, and
    129 chars are refused);
  - `blocked_names_refused` (every spec name plus each ruling addition, including
    `XDG_SESSION_TYPE` and `HYPRLAND_X`, with the right plain reason);
  - `warned_names_allowed` (`LD_PRELOAD`, `LD_LIBRARY_PATH`, `PATH` → `Ok(Some(BreaksApps))`);
  - `values_checked` (4096 bytes pass and 4097 are refused; `\n`, `\0`, `\t`, `\u{7f}` and `$`
    are refused; `""` passes);
  - `refuses_hostile_vars` (Review Focus 1: the value `") hl.exec_cmd("x` is *accepted*; it's
    only data, and Task 5 proves it stays inside the string);
  - `duplicates_and_limit_refused` (the 101st variable);
  - `lenient_load_keeps_good_entries` (one bad entry among three loads two);
  - `section_round_trips` (§9: `from_section(section())` is identity);
  - `environment_d_round_trips`. A reference parser in the test follows systemd's `env-file.c`
    single- and double-quote rules. Values `a b`, `it's`, `"q"`, `` `x` ``, `back\slash`, `#x`
    and `é` all round-trip.
  - `refusals_are_plain` (no message contains `regex`, `[A-Z`, or `state.toml`).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core env -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `state.rs` growth to the field, the dispatch arms and the
  repair call.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS, including
  `every_key_a_summary_can_show_has_a_plain_label`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): environment variables as a validated uQ domain`

### Task 3: `uq_core::compat`: switches, per-app fixes, the name table and the launch argv

**Files:**
- Create:
  - `crates/uq-core/src/compat/mod.rs` (the model and validation);
  - `compat/hypr.rs` (the name table, with the header citing both tags and files);
  - `compat/browsers.rs`;
  - `compat/launch.rs` (`argv`, `override_text`, `main_scale`, `gdk_scale`);
  - `compat/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/state.rs` (`pub compat: Compat`, as in Task 2);
  - `crates/uq-core/src/settings/mod.rs` (`SECTION_DOMAINS` gains `"compat"`;
    `renders_section("compat")` is true);
  - `crates/uq-core/src/settings/provenance.rs` (the `compat.sharp_x11` row and `expected` for
    it);
  - `crates/uq-core/src/history/labels.rs` (the compat labels);
  - `apps/uq-settings/src/registry.rs` (skip `compat` until Task 12).

**Interfaces (produced):**
- `compat::hypr`:
  - `pub const XWAYLAND_ZERO_SCALING: &str = "xwayland:force_zero_scaling";`
  - `pub enum SwitchKey { SharpX11, ElectronWayland, ChromiumWayland, QtWayland, SteamScale }`
    with `key()` and `label()`.
  - `pub enum Value { Fixed(&'static str), MainScaleInt, MainScale }`.
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
  - `pub enum Platform { Auto, Wayland, X11 }`;
  - `pub struct AppFix { pub id, pub platform, pub scale: Option<f64>, pub env: Vec<env::Var>, pub args: Vec<String>, pub elsewhere: bool }`;
  - `pub struct Compat { pub sharp_x11: bool, pub electron_wayland: bool, pub chromium_wayland: bool, pub qt_wayland: bool, pub steam_scale: bool, pub apps: Vec<AppFix> }`,
    with `from_section`, `repaired`, `section` and `fix_for(&self, id) -> Option<&AppFix>`
    (by normalised id);
  - `pub const MAX_APPS: usize = 100`, `MAX_APP_ENV = 20`, `MAX_ARGS = 20`, `MAX_ARG_CHARS = 1024`.
- `compat::launch`:
  - `pub fn main_scale(live: &[LiveOutput]) -> Option<f64>` (R13);
  - `pub fn gdk_scale(s: f64) -> u32` (R14);
  - `pub fn fmt_scale(s: f64) -> String`: at most 2 places, no trailing zeros (`1.25`, `2`);
  - `pub fn switch_env(c: &Compat, main: Option<f64>) -> Vec<(String, String)>`: lines whose
    value needs a scale are left out when `main` is `None`;
  - `pub fn argv(fix: Option<&AppFix>, id: &str, exec: Vec<String>) -> Result<Vec<String>, CompatError>`;
  - `pub fn override_text(source: &str, source_path: &str, fix: &AppFix, id: &str) -> Result<Option<String>, CompatError>`;
  - `pub enum CompatError { Unsafe(String), BadExec(String) }`, with plain messages.

- [ ] **Step 1: Write the failing tests** in `compat/tests.rs`:
  - `switches_and_apps_round_trip` (§9 round trip);
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
  - `launch_argv_refuses_equals_program` (Review Focus 5);
  - `override_text_wraps_every_exec`. The fixture has `[Desktop Entry]` with `Exec=code %F`,
    `DBusActivatable=true` and `[Desktop Action new-window]` with `Exec=code --new-window %F`.
    Both Execs become `env "GDK_BACKEND=…" … code <args> %F`; `DBusActivatable=false`; the
    marker and `X-uQ-Source` are present; the localised `Name[de]` line is unchanged; and the
    result parses back with `read_entry` to `uq_override == true`.
  - `override_text_inserts_args_before_field_code`;
  - `browsers_table_unique` and `labels_are_plain` (no label holds `xwayland`, `env`, `Qt_`,
    `ozone` or `hl.`);
  - `refusals_are_plain`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core compat -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Cite the source file, tag and line above each table.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): Apps and scaling switches and per-app fixes, checked against 0.56.2 and 0.57.0`

### Task 4: `uq_core::autostart`: the domain, the folder scan and the run plan

**Files:**
- Create:
  - `crates/uq-core/src/autostart/mod.rs` (the model and validation);
  - `autostart/scan.rs` (the pure scan over in-memory folder listings);
  - `autostart/tests.rs`.
- Modify:
  - `crates/uq-core/src/lib.rs`;
  - `crates/uq-core/src/state.rs` (`pub autostart: Autostart`);
  - `crates/uq-core/src/settings/mod.rs` (`SECTION_DOMAINS` gains `"autostart"`;
    `renders_section("autostart")` is false);
  - `crates/uq-core/src/rules/mod.rs` (`fresh_id` gains a sibling
    `fresh_prefixed(prefix, taken)`; `fresh_id` calls it with `"r-"`);
  - `crates/uq-core/src/paths.rs` (a `StartupPaths { config_home, config_dirs, data_home, data_dirs, runtime_dir, home }`
    with `from_vars`, so tests use temp dirs);
  - `crates/uq-core/src/history/labels.rs` (the autostart labels);
  - `apps/uq-settings/src/registry.rs` (skip `autostart` until Task 12).

**Interfaces (produced):**
- `pub struct FileChoice { pub id: String, pub enabled: Option<bool>, pub delay: Option<u32> }`.
- `pub enum Added { App { id, name, app, enabled, delay }, Command { id, name, argv, enabled, delay } }`
  (serialised with exactly one of `app` or `argv`).
- `pub struct Autostart { pub files: Vec<FileChoice>, pub added: Vec<Added> }` with
  `from_section(v, current)`, `repaired`, `section` and `choice(id)`.
- `scan.rs`:
  - `pub struct Found { pub id: String, pub user: Option<(PathBuf, String)>, pub system: Option<(PathBuf, String)> }`;
    the texts are already read and capped.
  - `pub fn read_folders(user: &Path, system: &[PathBuf]) -> Vec<Found>`, the one function that
    touches the disk here.
  - `pub enum Source { System, Yours, Added }`.
  - `pub enum Plan { Run { argv: Vec<String>, cwd: Option<PathBuf>, delay: u32 }, Off, Skip(&'static str) }`.
  - `pub struct Resolved { pub id, pub name, pub icon, pub source, pub enabled: bool, pub can_toggle: bool, pub delay: u32, pub plan: Plan, pub orphan_copy: Option<PathBuf> }`.
  - `pub fn resolve(found: &[Found], state: &Autostart, compat: &Compat, apps: &dyn Fn(&str) -> Option<(PathBuf, String)>, exists: &dyn Fn(&str) -> bool) -> Vec<Resolved>`.
    It's pure. `apps` finds an added app's desktop file, and `exists` answers `TryExec`. The
    `argv` already went through `compat::launch::argv`.
  - `pub fn hidden_copy(system_text: &str) -> String`, and
    `pub fn wanted_copies(found, state) -> (Vec<(String, String)>, Vec<PathBuf>)`, which gives
    the copies to write and the marked copies to delete.

- [ ] **Step 1: Write the failing tests** in `autostart/tests.rs`:
  - `section_round_trips_and_ids_assigned` (an `added` entry without an id gets `a-xxxx`);
  - `added_needs_app_or_argv_not_both`, `argv_checked` (the first arg can't contain `=`; 65
    args are refused), `delay_range`;
  - `scan_user_file_shadows_system`;
  - `scan_marked_copy_is_ignored_and_orphan_reported`;
  - `scan_skips_with_plain_reasons` (Review Focus 4): `Hidden`, `OnlyShowIn=GNOME;`, missing
    `TryExec`, `Terminal=true`, `Exec=app %d` and `Type=Link` each give their reason;
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
    symlink to a file and 205 files gives 200, with the bad name and the big file skipped);
  - `refusals_are_plain`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core autostart -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): autostart entries from the XDG folders and your own, as a uQ domain`

### Task 5: Render the xwayland option and `hl.env` lines; the `environment.d` lines

**Files:**
- Create: `crates/uq-core/src/lua/startup.rs` (with its tests).
- Modify:
  - `crates/uq-core/src/lua/mod.rs`. `emit_generated` gains a last parameter
    `startup: &StartupRender`, and every caller and test passes `&StartupRender::default()`.
    The xwayland table goes inside `hl.config({…})`, after the input tables. The env block goes
    after the cursor `hl.env` lines and before the workspace rules.
  - `crates/uq-core/tests/generated_golden.rs` and the golden files under
    `crates/uq-core/tests/golden/generated/`: re-bless; add `startup-all.lua`.
  - `crates/uqd/tests/verify_config.rs` (a CI `--ignored` case that verifies a config with
    every switch on and three env vars on the real Hyprland).
  - `crates/uq-core/src/env.rs` (`pub fn session_env(env, compat, main) -> Vec<(String, String)>`,
    which merges per R11).

**Interfaces (produced):**
- `pub struct StartupRender { pub zero_scaling: bool, pub env: Vec<(String, String)>, pub export: bool }`,
  with `Default` (all off, empty, `export: true`).
- `pub fn startup_render(state: &State, main: Option<f64>, dev_session: bool) -> StartupRender`.
  The env is `env::session_env(&state.env, &state.compat, main)`, and `export` is
  `!dev_session`.
- `pub fn emit_startup_env(out: &mut String, r: &StartupRender)` writes
  `-- Environment (Startup and Apps and scaling pages)` and then one
  `hl.env(<lua_string name>, <lua_string value>, true|false)` per line.
- `pub fn emit_xwayland(out: &mut String, r: &StartupRender)` writes
  `    xwayland = {\n        force_zero_scaling = true,\n    },` only when it's on.
- uqd and the autostart runner use the same `session_env`, so the three lists can't drift:
  `generated.lua`, `environment.d` and the autostart children's environment.

- [ ] **Step 1: Write the failing tests:**
  - in `lua/startup.rs`:
    - `xwayland_only_when_on`;
    - `env_lines_in_order_compat_last`;
    - `export_false_in_dev_session`;
    - `hostile_env_stays_inside_lua_strings` (Review Focus 1). The value
      `") hl.exec_cmd("x` and `]]--` give exactly one `hl.env(` per variable. The file loaded
      under the Lua mock of slice C's F19 calls `hl.env` with the literal value and makes no
      other call. The Lua mock runs in CI only (K4); locally assert on the text.
  - in `env.rs`:
    - `session_env_compat_replaces_env` (R11);
    - `session_env_cursor_not_included`. Cursor lines stay in `emit_generated`'s existing
      block; `session_env` adds the theme cursor for the autostart children only, through
      uqd (Task 8).
  - `generated_golden`: the existing goldens unchanged except where a new block appears
    (none in defaults), plus the new `startup-all.lua`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core lua -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Re-bless with the existing `UQ_BLESS=1` flow of
  `generated_golden.rs`. Diff the re-blessed files and confirm that only the intended lines
  changed.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -p uqd -- --test-threads=2`. Expected: PASS.
  `verify_config`'s new case is `--ignored` (CI with Hyprland).
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): render sharp X11 apps and environment variables into generated.lua`

### Task 6: History summaries, `Explain` rows, the `user.lua` env finder and profile pins

**Files:**
- Create: `crates/uq-core/src/startup_explain.rs` (with its tests).
- Modify:
  - `crates/uq-core/src/history/diff.rs` (`summarize` hands `env`, `compat` and `autostart` to
    their `summary` functions);
  - `crates/uq-core/src/env.rs`, `compat/mod.rs` and `autostart/mod.rs` (`pub fn summary`);
  - `crates/uq-core/src/settings/provenance.rs`
    (`pub fn find_env_in_user_lua(text, name) -> Option<u32>`, using the shared comment cutter);
  - `crates/uq-core/src/profile/tests` (wherever slice B's domain tests live:
    `crates/uq-core/src/profile/mod.rs` tests).

**Interfaces (produced):**
- `pub struct FileReport { pub env_file: Option<String>, pub browsers: Vec<(String, BrowserFile)>, pub overrides: BTreeMap<String, OverrideState>, pub autostart: Option<String> }`
  (filled by uqd's sync, Task 7). The fake uqd builds one by hand.
  - `pub enum BrowserFile { Ours, OwnFile, None, Error(String) }`.
  - `pub enum OverrideState { Written, OwnFile, NoSource, Error(String), Off }`.
- `pub fn explain_env(state, user_lua: Option<&str>, rejected: Option<&str>, report: &FileReport, path: &str) -> Value`.
- `pub fn explain_compat_extra(state, user_lua, rejected, report, path) -> Value`: every compat
  row except `sharp_x11`, which the generic option path gives.
- `pub fn explain_autostart(state, report) -> Value`.

- [ ] **Step 1: Write the failing tests:**
  - In `startup_explain.rs`:
    - `env_row_user_lua_line` (`user.lua` line 3 has `hl.env("FOO", "x")` → `user_lua`, line
      3; `-- hl.env("FOO", "x")` doesn't count);
    - `env_row_replaced_by_switch` (R11);
    - `env_rejected_gives_section_row` (F9);
    - `env_file_error_reported`;
    - `compat_switch_user_lua` (`hl.env("ELECTRON_OZONE_PLATFORM_HINT", …)` in `user.lua`);
    - `compat_browser_details` (one row per installed browser, with its state);
    - `compat_app_override_states` (each `OverrideState` maps to its source and detail);
    - `autostart_error_row`.
  - In `diff.rs` tests:
    - `env_summary_names_never_values` (a value `s3cret` never appears in any summary);
    - `compat_summary` (switches, and a fix added, changed or removed);
    - `autostart_summary` (off, on, delay, added or removed, by name);
    - `summaries_are_plain` (no `{`, `[`, `.desktop` or `env.` in any summary).
  - In `labels.rs`, the existing `every_key_a_summary_can_show_has_a_plain_label` passes with
    the new domains and their labels from Tasks 2–4.
  - In the profile tests (R1):
    - `profile_refuses_startup_domains` (`[env]`, `[compat]` or `[autostart]` gives
      `UnknownDomain`);
    - `from_journal_skips_startup_domains` (all three are `None`).
  - In `provenance.rs`: `compat_sharp_x11_row_expected` (true when on, `null` when off with the
    `default` source).
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Re-run. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): history and explanations for startup apps, environment variables and app fixes`

### Task 7: uqd: the three sections, the main scale and the file sync

**Files:**
- Create:
  - `crates/uqd/src/files.rs` (the sync: `environment.d`, flag files, overrides, hidden copies);
  - `crates/uqd/src/files/tests.rs`;
  - `crates/uqd/src/startup.rs` (`impl Desktop`: explain hooks, the compat pre-check);
  - `crates/uqd/src/startup/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`mod files; mod startup;`);
  - `crates/uqd/src/service.rs`, limited to:
    - `Desktop` gains `startup_paths: StartupPaths`, `dev_session: bool`,
      `main_scale: Option<f64>` and `files: FileReport`;
    - `render_candidate` computes `main_scale` (one `comp.monitors()` read, only when
      `sharp_x11` or `steam_scale` is on), stores it, and passes `startup_render(...)` to
      `emit_generated`;
    - `persist` calls `self.sync_files(&old)` after adopting `next` and before the
      `Schedule` early return;
    - `explain` hands `env`, `compat` and `autostart` to `Desktop::explain_startup`, after the
      `rules` hook. For `compat`, it runs the generic option loop for `sharp_x11`, then merges
      in the extra rows;
    - `domain_label` gains the three labels;
    - `startup()` runs a full sync once.
  - `crates/uqd/src/sections.rs` (`set_section` calls `self.check_compat_files(&next)` before
    `commit_section` when the domain is `compat`);
  - `crates/uqd/src/main.rs` (the 60 s tick also calls `refresh_overrides()`, R18;
    `dev_session` comes from `UQ_DEV_SESSION`);
  - `crates/uqd/tests/dbus.rs` (F23-style list completion).

**Interfaces:**
- Consumes: Tasks 1–6.
- Produces:
  - `pub(crate) fn sync_files(&mut self, old: &State)`. It runs each part whose inputs changed:
    `env`, `compat`, `autostart`, or `main_scale` versus the last sync. It records the outcome
    in `self.files`. An I/O error goes to `FileReport` and `tracing::warn!`, and never fails the
    write. The parts are:
    - `files::write_environment_d(&paths, &lines)`;
    - `files::sync_flags(&paths, &compat, installed)`;
    - `files::sync_overrides(&paths, &compat, dev)`;
    - `files::sync_hidden_copies(&paths, &found, &state.autostart)`.
  - `pub(crate) fn refresh_overrides(&mut self)`: `sync_overrides` again, for R18.
  - `pub(crate) fn check_compat_files(&self, next: &State) -> Result<(), DesktopError>`. The
    refusals from Global Constraints come back as `DesktopError::Conflict` with plain text, and
    only for fixes whose `elsewhere` turns on in this write.
  - The marker check is `files::is_ours(path, MarkerKind) -> bool`. It uses `symlink_metadata`
    (a symlink is never ours) and a capped read, and it runs again immediately before each
    delete.
  - `files::safe_join(dir, id) -> Option<PathBuf>`: `valid_desktop_id`, then the result's
    parent must equal `dir` and its file name must equal the id.

- [ ] **Step 1: Write the failing tests:**
  - In `files/tests.rs` (temp dirs, no uqd):
    - `environment_d_written_and_rewritten`;
    - `flags_written_only_for_installed_browsers`;
    - `never_touches_unmarked_flags_file`;
    - `flags_removed_when_off`;
    - `override_written_with_marker_and_source`;
    - `override_regenerated_when_source_changes` (change the source's `Exec`, then call
      `refresh`);
    - `override_deleted_when_fix_removed`;
    - `never_touches_unmarked_override`;
    - `never_follows_override_symlink` (a symlink to a file outside the folder stays, and its
      target is unchanged);
    - `never_deletes_file_swapped_after_check`: a marked file is replaced by an unmarked one
      before the delete step (driven through a test hook between check and delete), and it
      survives.
    - `override_path_stays_in_folder`;
    - `hidden_copy_written_and_removed`;
    - `never_touches_unmarked_autostart_file`;
    - `orphan_copy_removed`.
  - In `startup/tests.rs` (`FakeCompositor`, temp `StartupPaths`):
    - `env_section_renders_hl_env_and_journals` (the verified `generated.lua` has the line, the
      journal entry's origin is `settings`, the summary names the variable, and
      `environment.d` is written);
    - `env_dev_session_does_not_export` (`, false)`);
    - `compat_section_renders_zero_scaling_with_main_scale` (a fake `monitors` reply with
      `DP-1` at 0,0 and scale 1.5 gives `GDK_SCALE` "1");
    - `compat_without_hyprland_leaves_scale_lines_out`;
    - `compat_elsewhere_refused_for_own_file`, `…_without_source` and `…_in_dev_session`
      (`state.toml` is unchanged and no journal entry is added);
    - `undo_compat_removes_override` (undo the entry that added a fix with `elsewhere`: the
      override file is gone);
    - `restore_point_brings_flags_back`;
    - `autostart_section_writes_hidden_copy_and_never_renders` (no `verify` call is recorded);
    - `explain_env_user_lua_and_file_error`;
    - `explain_compat_sharp_x11_runtime` (the fake `getoption` says false while state says
      true gives `runtime`);
    - `rejected_env_keeps_previous_config` (`verify_error` gives `error` rows, and
      `environment.d` is unchanged);
    - `env_not_in_settings_changed` (R3).
  - In `crates/uqd/tests/dbus.rs`: `startup_sections_over_dbus` (Get, Set and Explain for all
    three), and extend `desktop_proxy_matches_the_interface`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd files startup -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Keep `service.rs` growth to the lines listed. The sync and the
  explain hooks live in `files.rs` and `startup.rs`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS, including
  every slice A, B and C test.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): environment variables and app fixes applied, with marked files only`

### Task 8: uqd: the autostart runner, `ListAutostart`, `ShellReady` and `AutostartChanged`

**Files:**
- Create: `crates/uqd/src/autostart.rs`, `crates/uqd/src/autostart/spawn.rs` (the trait and
  the system impl) and `crates/uqd/src/autostart/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`pub mod autostart;`);
  - `crates/uqd/src/service.rs` (`Event::AutostartChanged`, one line);
  - `crates/uqd/src/dbus.rs`:
    - the runner lives on `DesktopIface` beside the pick broker, outside the desktop lock;
    - `list_autostart`, `shell_ready`, the `autostart_changed` signal and the `serve()` arm;
    - the 20 s fallback task is spawned in `serve()`;
  - `crates/uq-core/src/client.rs` (`list_autostart`, `shell_ready`,
    `receive_autostart_changed`);
  - `crates/uqd/src/main.rs` (pass `UQ_DEV_SESSION`);
  - `crates/uqd/tests/dbus.rs`.

**Interfaces (produced):**
- `spawn.rs`:
  - `#[async_trait] pub trait Spawner: Send + Sync`, with:
    - `async fn spawn(&self, unit: &str, tag: &str, argv: &[String], env: &[(String, String)], cwd: &Path) -> std::io::Result<Child>`;
    - `async fn alive(&self, pid: u32, unit: &str) -> bool`;
    - `async fn journal_tail(&self, tag: &str, lines: usize) -> Vec<String>`.
  - `pub struct Child { pub pid: u32, pub wait: Pin<Box<dyn Future<Output = ExitInfo> + Send>> }`
    (`futures-util` is only a dev-dependency of uqd, so no `BoxFuture`), and
    `pub enum ExitInfo { Code(i32), Signal(i32), Unknown }`.
  - `SystemSpawner` builds exactly the argv in Global Constraints with
    `tokio::process::Command` (no shell). It sets `stdin` to null, uses `kill_on_drop(false)`,
    and gives the env as `envs()` on top of the inherited environment. `alive` reads
    `/proc/<pid>/cgroup`. `journal_tail` runs `journalctl` with a 2 s timeout.
  - `FakeSpawner` (under `cfg(any(test, feature = "fake"))`) records calls and has scripted
    exits and journal lines.
- `autostart.rs`:
  - `pub const SHELL_WAIT: Duration = 20 s`, and `pub const MAX_LOG_LINES = 10`.
  - `pub struct Runner` holds:
    - the spawner, the record path and the session id;
    - `started: bool` and `statuses: BTreeMap<String, Status>`;
    - `dev: bool`.
  - `pub async fn start_once(runner: &Mutex<Runner>, plan: Vec<Resolved>, env: Vec<(String, String)>, emit: impl Fn())`:
    - It takes the desktop lock only to build `plan` and `env`
      (`Desktop::autostart_plan(&self)`, which calls `autostart::scan::resolve` with
      `startup_paths`), then drops it.
    - Then, per entry: honour R7, sleep the delay in a task, spawn, record, and wait for the
      exit in a task. Each status change calls `emit` (a sync broadcast send of
      `Event::AutostartChanged`).
  - The record file is written with `atomic_write` under `$XDG_RUNTIME_DIR/uq/`. The session id
    must be `[A-Za-z0-9_]{1,128}`, or the runner doesn't run and logs why.
  - D-Bus `ListAutostart() → s` gives
    `[{id, name, icon, source, enabled, can_toggle, delay, status, reason?, exit?, log?, argv?}]`.
    `argv` is present only for `added` commands. `log` is read only for `failed` entries. Text
    is cleaned with `clean_text_spaced`.
  - `ShellReady()` is idempotent and returns at once; the run proceeds in a task.
  - The signal is `AutostartChanged()`.

- [ ] **Step 1: Write the failing tests** in `autostart/tests.rs` (FakeSpawner, temp dirs):
  - `runner_starts_each_entry_once_with_scope_and_tag`: the argv starts
    `systemd-run --user --scope --collect --slice=app.slice --unit=app-uq-autostart-nm-applet-`
    and contains `systemd-cat -t uq-autostart-nm-applet --`.
  - `runner_waits_for_delay` (with tokio's paused clock: `waiting` at 4 s, `running` after 5 s);
  - `runner_skips_off_and_skipped_entries` (Review Focus 4);
  - `runner_env_matches_generated` (the env passed equals `session_env` plus the theme cursor);
  - `runner_records_and_does_not_rerun_after_restart` (a second `Runner` with the same session
    and the same record file starts nothing, and reports `running` when `alive` is true or
    `unknown_exit` when false);
  - `runner_new_session_skips_live_pid` (a different session id, with `alive` true for the old
    record, doesn't start it again);
  - `runner_failed_entry_has_exit_and_log` (exit 3 gives `failed`, `exit: 3`, and the last lines
    cleaned of control characters);
  - `runner_does_nothing_in_dev_session` (`not_started` with the plain reason);
  - `runner_bad_session_id_runs_nothing`;
  - `shell_ready_twice_runs_once`;
  - `fallback_starts_after_shell_wait` (paused clock);
  - `set_section_does_not_start_or_stop` (turning an entry on gives `not_started`, and no spawn
    is recorded).
  - In `crates/uqd/tests/dbus.rs`:
    - `list_autostart_over_dbus`;
    - `autostart_changed_signal`;
    - `list_autostart_does_not_hold_the_desktop_lock` (a `SetSection("look")` during a slow fake
      `journal_tail` returns).
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd autostart -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Don't add a `rand` dependency: unit suffixes use the bridge's
  `random_suffix` approach, moved to `uq_core::desktop_entry::random_suffix`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): run apps from the autostart folders once per session, each in its own scope`

### Task 9: The bridge's launch path applies per-app fixes; `uqctl launch`; `ShellReady`

**Files:**
- Modify:
  - `crates/uqctl/src/bridge/launch.rs`:
    - `plan` takes `fix: Option<&AppFix>`;
    - R19 marked-override handling;
    - `exec_argv` with a `FieldCtx`;
    - `compat::launch::argv`;
    - `launch()` fetches the fix (500 ms, never fails).
  - `crates/uqctl/src/bridge/launch_tests.rs`;
  - `crates/uqctl/src/bridge/mod.rs`: after the shell's `Hello`, and again on each `UqdUp`, call
    `link.call("ShellReady", &[])` once (errors ignored). `"ShellReady"` isn't added to the
    shell's pass-through list; the shell never needs to call it.
  - `crates/uqctl/src/main.rs` (the `Launch { id: String }` subcommand → `launch_cmd::run`);
  - `crates/uqctl/tests/bridge_dbus.rs`.
- Create: `crates/uqctl/src/launch_cmd.rs`. It reads the fix with `GetSection("compat")`,
  plans, and dispatches through `HyprSocket::from_env()`, so the scope is Hyprland's child like
  every shell launch. It records the launch with `RecordLaunch` and exits non-zero with a plain
  message on failure.

**Interfaces:**
- Consumes: Task 1 (`exec_argv`, `read_entry`, `normalise_id`) and Task 3 (`Compat`,
  `launch::argv`).
- Produces:
  - `pub fn plan(arg, dirs, rand, runs, terminal, fix: Option<&AppFix>) -> Result<Plan, String>`;
  - `pub async fn fix_for(link: &Link, id: &str) -> Option<AppFix>`;
  - `uqctl launch <desktop-id>`.

- [ ] **Step 1: Write the failing tests:**
  - In `launch_tests.rs`:
    - `launch_applies_fix_env_and_args` (the scope command holds `'env' '--'
      'GDK_BACKEND=wayland,x11' … 'code' '--x'`);
    - `launch_without_fix_unchanged`;
    - `launch_terminal_app_wraps_env_before_terminal`;
    - `launch_marked_override_uses_source` (R19: the override in a temp data home names a
      system source, and the fix is applied once);
    - `launch_marked_override_with_bad_source_uses_override_as_is` (a source outside the data
      dirs, or a missing one: no second fix);
    - `launch_fix_refused_for_equals_program` (Review Focus 5: it launches without the fix);
    - `launch_icon_field_code` (`%i` → `--icon x`).
  - In `bridge_dbus.rs`:
    - `launch_fetches_compat_fix` (against a uqd with a fix: the dispatched Lua contains the
      env wrapper);
    - `launch_with_uqd_down_still_launches`;
    - `launch_with_slow_uqd_times_out_and_launches` (a fake that answers `GetSection` after
      2 s: the launch dispatches within about 600 ms, without the fix);
    - `bridge_calls_shell_ready_after_hello` (the fake uqd records one `ShellReady`, and one
      more after a uqd restart).
  - In `crates/uqctl` CLI tests (the existing pattern in `tests/common`):
    `uqctl_launch_unknown_id_fails_plainly`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqctl launch -- --test-threads=2` and
  `cargo test -j 2 -p uqctl --test bridge_dbus -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqctl -- --test-threads=2` and `scripts/lint-shell.sh`.
  Expected: PASS. Where Quickshell is installed, also run `tests/shell/bar_loads.sh`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(shell): launches apply each app's fix; uqctl launch; tell uqd the shell is up`

### Task 10: uq-settings: the Startup page logic (Qt-free) and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/startup.rs`;
  - `apps/uq-settings/src/pages/startup/tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/autostart.rs` (`FakeAutostart`: a scripted
    `ListAutostart` list and a switch that makes it fail).
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod startup;`);
  - `apps/uq-settings/src/pages/fake_uqd.rs`:
    - `list_autostart` and `shell_ready`;
    - `explain("env")`, `explain("autostart")` through `startup_explain` with a
      `FakeUqd.files: Arc<Mutex<FileReport>>`, then `Explain.overrides` (F7);
    - the three sections go through the generic section write;
    - `Rig.autostart`.

**Interfaces:**
- Consumes:
  - `runner::{PageLogic, parse_json}`;
  - `DesktopProxy::{get_section, set_section, explain, list_autostart}`;
  - `uq_core::{env, autostart, desktop_entry::split_command}`;
  - `crate::desktop_entries` (the installed apps for "Add an app").
- Produces:
  - `pub enum Op`:
    - `Refresh`;
    - `SetEnabled { id: String, on: bool }`: a file entry writes `files[id].enabled`, and an
      added one writes its own `enabled`;
    - `SetDelay { id, delay: Option<u32> }`;
    - `AddApp { desktop_id: String }`: the name comes from `desktop_entries`;
    - `AddCommand { name: String, command: String }`, split with `split_command`; a split
      error is the op's error, in plain words;
    - `Remove { id }`: added entries only; for a file entry the op is refused ("Turn it off
      instead");
    - `SetVar { index: Option<usize>, name: String, value: String }`;
    - `RemoveVar { name }`;
    - `MoveVar { name, to }`.

    Every write reads the section, edits it, and writes it with origin `settings` under the
    write lock.
  - `pub struct StartupState`, a `PageLogic` with `new(conn)`.
  - **The snapshot:**
    ```
    { apps:    [{id, name, icon, source, enabled, can_toggle, delay, status, status_text,
                 reason, exit, log, command_text}],
      vars:    [{name, value, warn: bool, source, line, replaced_by_label}],
      switches:[{key, label, on}],            // §6 switches, read-only (from GetSection("compat"))
      section: {env: {source, message}|null, autostart: {source, message}|null},
      installed: [{id, name, icon}],          // for Add an app
      limits:  {vars: 100, value_bytes: 4096, added: 50, delay_max: 600},
      notice:  {seq, text} }
    ```
    - `status_text` is the plain line, for example "Running", "Started, then finished",
      "Failed (exit 3)", "Starts next time you log in", "Waiting 10 s", "Off", the skip reason,
      or "Not started in a test session".
    - `command_text` re-quotes an added command with `write_exec`, for display only.
    - `notice` is set after any env write: "Changes apply to apps started after the change, and
      fully after you log out and back in." After removing a variable it adds "Removed variables
      stay for apps started before you log out." (R9).

- [ ] **Step 1: Write the failing tests** in `pages/startup/tests.rs`:
  - `toggle_system_entry_round_trips` (§9: `files[id].enabled = false`; reopening the state is
    identical; history names it);
  - `toggle_hidden_entry_refused_plainly` (`can_toggle: false`);
  - `add_app_and_command_round_trip` (the command `backup-check --quiet "a b"` is stored as
    three args, and `command_text` shows it re-quoted);
  - `bad_command_is_a_plain_error` (an unclosed quote);
  - `remove_file_entry_refused`;
  - `delay_round_trips_and_range_refused` (601 is refused with uqd's message);
  - `set_var_blocked_name_shows_message` (the `XDG_SESSION_TYPE` refusal text is shown, and the
    state is unchanged);
  - `set_var_warns_ld_preload`;
  - `var_with_dollar_refused` (R10);
  - `notice_after_env_write_and_removal` (R9);
  - `vars_show_user_lua_override` (`Explain.set("env.vars.FOO", …)`);
  - `switches_listed_read_only`;
  - `failed_entry_shows_exit_and_log` (scripted `FakeAutostart`);
  - `uqd_down_gives_empty_state_without_an_error`;
  - `list_autostart_failure_still_shows_vars` (FakeAutostart fails, and `apps: []` with an
    error).
- [ ] **Step 2:** Run the tests through t7check:
  `cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::startup -- --test-threads=2`.
  Expected: FAIL.
- [ ] **Step 3:** Implement. No `cxx_qt` or Qt types in `pages/startup.rs`.
- [ ] **Step 4:** Re-run. Expected: PASS. CI runs `cargo test -p uq-settings` and clippy.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): Startup page logic for login apps and environment variables`

### Task 11: uq-settings: the Apps and scaling page logic (Blurry right now, Fix this app)

**Files:**
- Create: `apps/uq-settings/src/pages/compat.rs` and `apps/uq-settings/src/pages/compat/tests.rs`.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs`;
  - `apps/uq-settings/src/desktop_entries.rs` (`AppEntry` gains `wm_class: Option<String>` and
    `exec_name: Option<String>`, from `uq_core::desktop_entry::read_entry`; both are cleaned like
    `name`);
  - `apps/uq-settings/src/pages/fake_uqd.rs` (`explain("compat")`, built from slice A's generic
    rows for `sharp_x11` plus `explain_compat_extra`).

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
  - `pub fn app_for_class(class: &str, apps: &[AppEntry]) -> Option<&AppEntry>`, which tries in
    order:
    1. `StartupWMClass` equal ignoring ASCII case;
    2. the id without `.desktop` equal ignoring case;
    3. the last dot part of the id equal ignoring case (`org.gnome.Foo` ↔ `foo`);
    4. `exec_name` equal.
  - `pub fn fields() -> Value`: the catalogue built from `compat::hypr` (switch keys, labels and
    descriptions; the platform choices; the scale steps 1.0–3.0). QML never copies the tables
    (slice C's F6).
  - **The snapshot:**
    ```
    { switches: [{key, label, description, on, source, line, detail}],
      browsers: [{name, file}],                         // from Explain chromium_wayland detail
      blurry:   {scaled: bool, sharp_on: bool,
                 windows: [{title, class, app_id|null, app_name|null, has_fix: bool}]},
      fixes:    [{id, name, icon, platform, scale, env, args, elsewhere, override, message}],
      installed:[{id, name, icon}],
      fields:   {...},
      reopen:   {seq, names: [..], more: n},
      section:  {source, message} | null }
    ```
    - `blurry.windows` holds the `xwayland` windows from `ListWindows`, de-duplicated by class.
      `scaled` is true when any enabled output from `ListOutputs` has a scale ≠ 1.
    - The heading text comes from `(scaled, sharp_on)`:
      - "These older apps may look blurry." when scaled and not sharp;
      - "Older apps open now" when sharp is on;
      - "Your screens aren't scaled, so nothing is blurry." when not scaled.
    - `reopen` is set after `SaveFix` or `RemoveFix` for an app with open windows:
      "Close and reopen {names} to apply." (slice C's ruling: no button closes windows).
    - `override` and `message` come from the `apps.<id>` Explain row. A refusal from
      `check_compat_files` is the op's error, and the switch snaps back with that text (§9).

- [ ] **Step 1: Write the failing tests** in `pages/compat/tests.rs`:
  - `switch_round_trips_and_journals` (all five);
  - `blurry_lists_only_xwayland_and_names_the_app`. Three windows (one XWayland Code with class
    `Code`, one Wayland Firefox, one XWayland with an unknown class) and installed `code.desktop`
    with `StartupWMClass=Code` give two rows: one with `app_id: "code.desktop"` and one with
    `app_id: null`.
  - `blurry_heading_states` (the three cases);
  - `app_for_class_order` (each of the four matches, and an earlier rule wins);
  - `save_fix_round_trips` (§9);
  - `fix_with_blocked_env_shows_message`;
  - `elsewhere_refusal_snaps_back_with_reason` (the fake's `SetSection` is scripted to refuse
    with uqd's `Conflict` text);
  - `save_fix_asks_to_reopen_open_app`;
  - `fields_come_from_the_table`;
  - `browsers_from_explain_detail`;
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
  - `apps/uq-settings/src/registry.rs`:
    - two pages;
    - remove the Task 2–4 skips in `every_journal_domain_has_one_page`;
    - update `pages_json_lists_pages_in_order` and the per-page order tests (slice C's F24
      pattern);
  - `scripts/settings-jargon.txt` (Global Constraints).

**Interfaces:**
- Consumes: Tasks 10 and 11.
- Produces:
  - `StartupModel` (`#[qml_element]`):
    - properties `state_json`, `error` and `busy`;
    - invokables `refresh()`, `set_enabled(id, on)`, `set_delay(id, secs)` (−1 means null),
      `add_app(id)`, `add_command(name, command)`, `remove(id)`, `set_var(index, name, value)`,
      `remove_var(name)` and `move_var(name, to)`.

    Its `FOLLOW` is sections `env`, `autostart` and `compat`, plus `history`, plus the
    `AutostartChanged` signal. It re-reads on window activation (slice A's ruling).
  - `CompatModel`:
    - the same three properties;
    - invokables `refresh()`, `refresh_windows()`, `set_switch(key, on)`, `save_fix(json)`,
      `remove_fix(id)` and `set_elsewhere(id, on)`.

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
    - The five switches (`SettingRow` with source labels: "Overridden in user.lua, line N" with
      **Open user.lua**). Each has its sentence and "Applies to apps started after the change."
    - Under the Chromium switch, the per-browser `own_file` lines.
    - A **Blurry right now** card: the heading, the rows, **Fix this app** and **Refresh**.
      Unmatched rows open the dialog with an app chooser ("uQ couldn't tell which app this is.
      Choose it:").
    - An **App fixes** list with **Edit** and **Remove**, the override state, and the `reopen`
      message in a `RuleMessage`.
  - **`AppFixDialog.qml`** (`Kirigami.Dialog`, title literal "Fix this app"):
    - **Run on** (Automatic / Wayland / X11) with the hint "Most blurry apps are sharp with
      Wayland.";
    - **Size** (Automatic, 100 %…300 % in 25 % steps);
    - **Extra settings for this app** (`EnvVarRow` list);
    - **Extra options** (one per line);
    - **Also when started from other launchers**, with its consequence sentence: "uQ saves its
      own copy of this app's launcher file, and removes it when you remove the fix.";
    - an `AdvancedDetails` disclosure with the env names and args this fix produces (from a
      synchronous `CompatModel.preview(json)`, which calls `compat::launch::argv` with a dummy
      exec; F17 style).

    It emits `accepted(string fixJson)`. The JSON is built from catalogue keys (F29).
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
    - a switch with a fixed literal or no `text:`, disabled when `!can_toggle`;
    - a status `UqLabel` (`objectName: "appStatus"`);
    - a delay spin box (0–600 s);
    - on failure, the `log` lines in `UqLabel`s inside a disclosure;
    - **Remove** for added entries.
  - **`EnvVarRow.qml`** has a name field (uppercase hint) and a value field, the warning line for
    `warn`, the source line, and the "replaced by" line.

- [ ] **Step 1: Write the failing tests:**
  - In `registry.rs`:
    - `startup_page_registered_with_search_entries`;
    - `apps_scaling_page_after_displays`;
    - `every_journal_domain_has_one_page` with no skips;
    - updated order tests.
    - The existing `every_setting_has_keywords`, `no_duplicate_setting_ids` and
      `ids_unique_and_categories_ordered` must still pass.
  - In `tst_autostart_row.qml`:
    - `test_name_is_plain_text` (a `Name` of `<b>x</b>` shows literally, with
      `Text.PlainText`, found by `objectName`; F26);
    - `test_log_lines_plain`;
    - `test_switch_disabled_when_cannot_toggle`;
    - `test_remove_only_for_added`.
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
  and `cargo test -p uq-settings`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): the Startup and Apps and scaling pages`

### Task 13: Docs, changelog, live checks and the whole gate

**Files:**
- Modify:
  - `README.md`;
  - `CHANGELOG.md` (`## [Unreleased]` → `### Added`);
  - `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md` (§10 D-Bus summary:
    `SetAutostart` → `SetSection("autostart")`, plus `ShellReady`; §12 row D unchanged);
  - `docs/developing.md` (a dev session starts no autostart apps and writes no launcher
    overrides, R20).
- Create: `docs/notes/settings-slice-d-live-check.md`.

- [ ] **Step 1:** Update `README.md`:
  - a "#### Startup page" paragraph:
    - which folders are read;
    - once per session;
    - the status words;
    - the hidden copy uQ writes for a system entry you turn off, and that your own files are
      never edited;
    - added apps and commands (no shell);
    - environment variables, the blocklist, "after the change / after logging out", no `$`,
      and that removed variables linger until logout;
    - undo puts back a whole list (F36).
  - a "#### Apps and scaling page" paragraph:
    - the five switches;
    - Blurry right now and Fix this app;
    - where fixes apply (uQ's launcher, Spotlight, pins, `uqctl launch`, and autostart);
    - "Also when started from other launchers" and the `X-uQ-Override` marker;
    - Chromium flag files and their marker line.
  - Add `ListAutostart`, `ShellReady` and `AutostartChanged` to the Settings D-Bus paragraph,
    and say that the three domains use `GetSection`/`SetSection`.
  - In the Profiles paragraph, say that startup apps, environment variables and app fixes can't
    be in a profile (R1).
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Startup apps, environment variables and app
  scaling.** Name both pages, the runner, the switches, the fixes, the markers and the new
  D-Bus calls.
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
     app. Kill an autostarted app, restart uqd, and the page says "Started earlier; finished".
  4. **Failure.** An entry with `Exec=sh -c "echo boom; exit 3"` shows "Failed (exit 3)" with
     `boom` in its log lines.
  5. **Delay.** A 10 s delay starts the app about 10 s after the shell appears.
  6. **§1 criterion 5.** At 125 % scale, turn on **Run Electron apps on Wayland** and start
     VS Code or another repo Electron app from Spotlight. It's sharp, and `hyprctl clients`
     shows `xwayland: 0`.
  7. **Sharp X11 apps.** Turn it on. A new `xeyes`/`xterm` is sharp (`getoption
     xwayland:force_zero_scaling` is 1); `GDK_SCALE` is in a new app's environment
     (`cat /proc/<pid>/environ`). Record whether GTK Wayland apps changed size (R12/R14 live
     item).
  8. **Fix this app.** Open an X11-only app. It's in **Blurry right now**. Fix it with Run on
     X11 and size 150 %, close and reopen it from the launcher, and its environment has
     `QT_SCALE_FACTOR=1.5` / `GDK_SCALE=1`. `uqctl launch <id>` does the same.
  9. **Elsewhere.** Turn on "Also when started from other launchers". Check that
     `~/.local/share/applications/<id>.desktop` has the marker and an `env` Exec, and that
     another launcher (`gtk-launch <id>`) gets the fix. Remove the fix: the file is gone. Put
     your own copy there first, and the switch is refused with the plain message.
  10. **Chromium.** With Chromium installed, the switch writes `~/.config/chromium-flags.conf`
      with the marker; with your own file there, it's left alone and the page says so. Record
      which flag files each installed browser really reads, and whether
      `--ozone-platform-hint=auto` still works.
  11. **Environment variables.** Add `FOO=bar`. A new terminal from Spotlight has it; a
      `systemctl --user show-environment` has it (R8); after logout and login
      `~/.config/environment.d/60-uq.conf` provides it. `XDG_SESSION_TYPE` and `A$B` are refused
      plainly. Add `hl.env("FOO", "x")` to `user.lua` and reload: the row says "Overridden in
      user.lua, line N". `user.lua`'s modification time is unchanged by every step of this
      checklist.
  12. **History and restore.** Undo a switch, a fix and a startup toggle from **History**; each
      row reads plainly and the files follow (the override and the hidden copy appear and go).
      Restore yesterday's point, and all three pages match it.
  13. **Dev session.** In `uq-dev session`, no autostart app starts, the page says "Not started
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
