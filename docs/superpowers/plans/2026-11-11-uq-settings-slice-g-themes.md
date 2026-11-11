# uQ Settings enhancements slice G: theme export, import and custom themes — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Themes stop being something only a package can add.
- **User themes.** uqd also loads `~/.local/share/uq/themes/user-<slug>/{dark,light}.toml`.
  Their ids always start with `user-`, so they can never shadow one of the seven system themes.
  They pass the same parser as system themes, plus stricter checks for text, sizes and WCAG AA
  contrast, because the files are untrusted.
- **New theme from the current one** (Appearance page). The user changes seven base colours
  (background, panels, text, accent, good, warning, bad) for each of dark and light. The editor
  shows the contrast check live and won't save a variant that fails it. Saving an existing user
  theme replaces it.
- **Export** writes one `.uqtheme` file: TOML with `format = 1`, the id, name, author, both
  variants, and optionally the current `look` keys. Wallpaper is never included.
- **Import** takes a `.uqtheme` from a file dialog or `uqctl theme import <file>`. uqd checks it,
  saves a restore point, and installs it as a user theme. It shows up in the theme list at once.
  Files over 64 KiB are refused, as is anything outside the schema. Names are shown only as plain
  text.
- **Delete** removes a user theme. If it's the active theme, uQ switches to the default first.

**Architecture:**
- **uq-core** (`crates/uq-core/src/theme.rs` becomes the parent of three new files):
  - `theme/user.rs`: the user-theme rules. It holds the id and slug rules, the checks for text,
    sizes, contrast and brightness, the safe folder scan (no symlinks, size caps), the variant
    TOML writer and the folder fingerprint.
  - `theme/uqtheme.rs`: the `.uqtheme` format: `parse`, `to_text`, `read_file`, `from_pair`
    and `install_files`.
  - `theme/derive.rs`: seven base colours become a full theme, with plain contrast messages.
  - `ThemeRegistry` keeps a `ThemeInfo` per id (name, author, user or system), and
    `Paths.user_themes_dir` is new.
- **uqd:**
  - `crates/uqd/src/themes.rs` holds the user theme store on `Desktop`. It loads at startup,
    re-scans on the 60 s tick, and has export, import (with a restore point) and delete (with
    the switch to the default).
  - New D-Bus calls: `ListThemeInfo() → s`, `ExportTheme(s id, b with_look) → s`,
    `ImportTheme(s text, b replace, b with_look, s origin) → s id` and
    `DeleteTheme(s id, s origin) → s`, plus the signal `ThemesChanged()`.
- **uqctl:**
  - `theme_cmd.rs` has `theme list [--long]`, `theme import <file> [--replace] [--with-look]`,
    `theme export <id> [file] [--with-look]` and `theme delete <id>`.
  - The bridge passes `ListThemeInfo` through, so the shell's quick-settings chips show names.
- **uq-settings:**
  - Qt-free page logic in `pages/themes.rs` (list, import, export, delete, editor session) and
    `pages/themes/editor.rs` (pure preview and file building), tested against the fake uqd;
  - a thin `themes_model.rs`;
  - the theme row on `AppearancePage.qml` grows Import…, Export…, New theme… and a menu per user
    theme. `ThemeEditor.qml` and `ThemeChip.qml` are new components;
  - three search entries in the registry.

**Tech Stack:** Rust edition 2024, zbus 5, tokio, `toml`, `serde`, minijinja (unchanged
templates), cxx-qt 0.10, Qt 6 (`QtQuick.Dialogs` `FileDialog` and `ColorDialog`), Kirigami 6,
Quickshell.

**Spec:** `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`. §7.4 is the
authority. This plan also covers:
- §1 success criterion 7 ("A theme exported on one machine imports on another and appears in the
  theme list, with no relog") and criterion 8 (nothing writes `user.lua`);
- §3: the `BeforeImport` restore point, and journalling of the optional `look` write;
- §9: the quality rules;
- §10: the `uqctl theme import|export` row, and the security line "Imports … are data-only,
  size-capped, schema-checked and validated per key";
- the testing line "`.uqtheme` … import fuzzing for size and schema";
- §12 row G.

It builds on slice A (`2026-11-10-uq-settings-slice-a-trust.md`, merged) and follows the layout of
slices C and D (`…-slice-c-rules.md`, `…-slice-d-startup-apps.md`, merged; HEAD `bb26a80`).

**Paths.** The settings app lives at `apps/uq-settings` (not `crates/`). Its QML tests are in
`tests/settings-qml`, and the shell's are in `tests/qml`.

**Ownership.** Another thread owns these paths, so this plan doesn't edit them:
- `crates/uq-core/src/integration/`;
- `apps/uq-settings/src/plugins.rs`;
- `shell/widgets/`;
- `templates/`;
- `web/uq-tokens/`.

These shared files get **additive edits only** (new lines, enum arms, list entries; no
reformatting or reordering of what's there):
- `apps/uq-settings/src/registry.rs`;
- `apps/uq-settings/qml/Sidebar.qml` and `Main.qml` (this slice should need neither);
- uqd's D-Bus files (`crates/uqd/src/dbus.rs`, `dbus/signals.rs`, `crates/uq-core/src/client.rs`);
- `uq_core::history::label` (`history/labels.rs`);
- `CHANGELOG.md`;
- `packaging/PKGBUILD` (this slice should need no change).

## Pre-flight rulings

The plan's author scanned the code at `bb26a80` for conflicts with §7.4 before writing the tasks.
Each finding has a ruling, and the task text below carries it out. This list is the index.

- **G-F1 Theme names reach config comments.** `templates/{kitty.conf,foot.ini,alacritty.toml,
  qt6ct.conf}.j2` print `{{ theme.name }}` on a `#` comment line, and `gtk{3,4}.css.j2` print it
  inside `/* … */`. A name holding a newline would start a new kitty config line. kitty's `shell`
  option runs a program, so that would be code from a theme file. `templates/` belongs to another
  thread. **Ruling:** user theme names and authors are checked at the source, by
  `theme::user::valid_display_text`:
  - 1–64 characters, trimmed;
  - no control characters (`char::is_control`);
  - none of the bidi and invisible format characters U+061C, U+200B–U+200F, U+202A–U+202E or
    U+2060–U+2069;
  - no `*/` and no `{{`, `{%`, `{#`.

  Task 1 tests every template with a hostile name that gets past nothing else (Review Focus 1).
- **G-F2 `ThemeRegistry::load_dir` fails on any bad theme.** `Desktop::new` fails if it errors.
  **Ruling:** the system folder stays strict (packages are trusted, and the test
  `all_fourteen_variants_load` still holds). User themes load through `user::scan`, which skips a
  bad theme and says why; it never fails the registry.
- **G-F3 `Theme` is serialised whole into `tokens.json`** (`templates/tokens.json.j2` is
  `{{ theme | tojson }}`), and the shell and `web/uq-tokens` read that file. **Ruling:** `Theme`
  gains no field. Author and user/system go into a separate `ThemeInfo` map in `ThemeRegistry`.
  Only the private `ThemeFile` gains an optional `author`.
- **G-F4 `tokens.json` holds the appearance overrides** (`with_appearance`). So
  `App.themeJson` isn't the stored theme. **Ruling:** the editor's starting point comes from
  `ExportTheme(id, false)`, parsed with `uqtheme::parse`, never from `tokens.json`.
- **G-F5 `crates/uqd/src/service.rs` is 2270 lines.** **Ruling:** everything new in uqd lives in
  `crates/uqd/src/themes.rs` (tests in `crates/uqd/src/themes/tests.rs`). `service.rs` gains
  only:
  - two fields;
  - the `Event::ThemesChanged` arm;
  - one call in `Desktop::new`.
- **G-F6 No inotify.** No crate in `Cargo.lock` wraps inotify, and uq-core and uqd have no `libc`
  dependency. Slice D ruled "no new crates". **Ruling (spec deviation 1):** there's no file
  watcher.
  - uqd reloads the user themes right after its own import, save and delete, so the app and
    `uqctl` see an imported theme "at once" (criterion 7).
  - A folder copied in by hand is found by `refresh_themes` on uqd's existing 60 s tick. It
    compares a metadata-only fingerprint and reads nothing when nothing changed.
  - `ListThemeInfo` also refreshes first, so opening the Appearance page always shows the folder
    as it is.
- **G-F7 `ListThemes() → as` returns ids only.** The shell's quick-settings chips
  (`shell/quick/ThemeGroup.qml`) and the Appearance page show those ids. **Ruling:**
  - `ListThemes` keeps its shape (for the shell, uqctl scripts and the VM checks).
  - `ListThemeInfo() → s` is new, and the app and the shell switch to it. The shell falls back
    to ids when the call fails (an older uqd).
- **G-F8 `Origin::Import` exists but nothing uses it.** Spec §3 lists `import` as an origin, but
  every uqd setter takes the caller's `origin`, and the app's Ctrl+Z only steps over its own
  `settings` entries. **Ruling (spec deviation 2):** `ImportTheme` takes an `origin`, and the
  optional `look` write is journalled with *that* origin (`settings` or `uqctl`), so it can be
  undone like any page write. Its summary reads as any `look` change does. Installing the theme
  files isn't a state change, so it isn't journalled. The `BeforeImport` restore point is what
  lets the user go back.
- **G-F9 A deleted theme can still be named by state that comes back later:**
  - an undo of an older theme change;
  - a restore point;
  - a profile's `appearance.theme`.

  These paths already refuse an unknown theme (`DesktopError::UnknownTheme`; profiles use
  `THEME_GONE` for "Back to normal"). **Ruling:** nothing about them changes.
  - `DeleteTheme` returns the names of the profiles that use the theme, and the page and `uqctl`
    say "The Gaming profile uses this theme. Pick another theme for it."
  - Profiles keep their domains exactly as slices B, C and D left them.
- **G-F10 The active theme can disappear behind uqd's back** (a hand-deleted folder). If
  `render_candidate` couldn't find it, every later commit would fail with `UnknownTheme`.
  **Ruling:** `ThemeRegistry::replace_user` keeps the active theme's last good copy, marked
  `missing: true` in `ListThemeInfo`, until the user picks another theme. uqd never switches
  themes on its own. At the next uqd start, `Desktop::new` falls back to the default as it does
  today.
- **G-F11 Dev sessions share `XDG_DATA_HOME`** (`docs/developing.md`, slice D R20). **Ruling:**
  with `UQ_DEV_SESSION` set, `ImportTheme` and `DeleteTheme` refuse with "Not available in a test
  session". Listing and export still work.
- **G-F12 The bridge's whitelist.** `crates/uqctl/src/bridge/mod.rs` (the pass-through match)
  and `bridge/uqd.rs` (`Request::parse`) both need `ListThemeInfo`. **Ruling:** only
  `ListThemeInfo` is added. Import, export and delete are never reachable from the shell.
- **G-F13 No new state keys.** Themes live in files, not `state.toml`. The only state change is
  the optional `look` write, whose keys already have labels. **Ruling:** `history::label` gains
  nothing. If an implementer finds they need a state key, it needs a label in
  `uq_core::history::label` in the same task, and
  `every_key_a_summary_can_show_has_a_plain_label` must pass. Task 4 adds
  `no_state_key_from_theme_store` to prove the store writes nothing else into `state.toml`.
- **G-F14 One safe reader already exists.** `uq_core::autostart::scan::read_capped_if` opens
  non-blocking, requires a regular file and caps at 64 KiB, but it decodes lossily and returns
  `Option`. **Ruling:**
  - The folder scan uses it, with a same-device-and-inode check against a `symlink_metadata`
    taken just before (slice D's F14 pattern).
  - `uqtheme::read_file`, for a file the user picked, needs a reason on failure. It is a small
    reader with the same flags. Task 2 makes `autostart::scan`'s `O_NONBLOCK`/`O_NOCTTY`
    `pub(crate)` (an additive visibility change).

## Plan rulings

These were decided when the plan was written.

- **R1, the user folder.** `Paths.user_themes_dir = startup.data_home.join("uq/themes")`
  (`~/.local/share/uq/themes`), so `Paths::under` sandboxes it.
  - A theme's folder is named by its **full id**, `user-<slug>`, as in the spec's
    `…/themes/<id>/`.
  - A folder that doesn't match `valid_user_id` is skipped, with "Its folder name must start with
    user- and use only a–z, 0–9 and dashes."
- **R2, ids.** `slug = [a-z0-9]([a-z0-9-]{0,38}[a-z0-9])?` (1–40 characters), and
  `id = "user-" + slug`, so an id is at most 45 characters and passes `valid_theme_name`.
  - A `.uqtheme` stores the bare slug in `id`. On import, a leading `user-` is stripped once, so
    re-importing an export is stable.
  - Exporting a system theme writes its own id (`gruvbox`), which imports as `user-gruvbox`.
  - New themes from the editor get `user::slug_for(name, taken)`: lower-case ASCII words joined
    by `-`, cut to 40, with `-2`, `-3` … when taken, and `theme` when empty.
- **R3, no symlinks, no recursion.** uqd reads and writes only real folders and regular files,
  and never through a link.
  - **The root.** `symlink_metadata(root)` must be a directory or absent. A symlinked root means
    "no user themes", and imports are refused with "Your themes folder is a link, so uQ won't
    write through it." (Notes question 1.)
  - **Theme folders.** A theme folder that's a symlink is skipped, and so is a variant file
    that's a symlink.
  - **Writes.** An import writes through `atomic_replace_dir(root/<id>, files)` after the
    checks above. Its staging and old siblings sit in the root, beside the target.
  - **Deletes.** A delete removes only `dark.toml` and `light.toml`, then `remove_dir`. It is
    never recursive. A folder that still holds something else stays, but without variants it no
    longer loads.
- **R4, limits.**
  - 64 KiB per `.uqtheme` and per variant file (`MAX_FILE_BYTES = 65_536`, as profiles);
  - 50 user themes (`MAX_USER_THEMES`); at most 200 folder entries looked at
    (`MAX_SCANNED`);
  - names and authors of 1–64 characters (G-F1); an author is optional, and empty means none;
  - `ui.radius` 0–40, `gap_in` 0–50, `gap_out` 0–100 and `border_width` 0–20 (the `look`
    ranges);
  - `fonts.size` 6–32; `fonts.ui` and `fonts.mono` follow `Appearance`'s font rule (1–100
    characters, no control characters);
  - `icons.theme` and `icons.cursor_theme` pass `valid_theme_name`; `icons.cursor_size` 16–96;
  - `terminal.ansi` exactly 16 `#rrggbb` colours, and every other colour `#rrggbb` (the
    existing `Color` parser).

  A file over a limit is refused, never cut.
- **R5, contrast and brightness.** A user theme, imported or saved, must pass:
  - `contrast::check_theme` (WCAG AA 4.5:1 on its six pairs), for **each** variant;
  - the brightness check from `crates/uq-core/tests/themes.rs`: dark `bg` darker than `text`,
    light `bg` lighter than `text`.

  On a failure, import is refused with a plain message naming the variant and the pair. The
  editor shows the same message live. The scan skips a hand-made folder that fails ("Some text
  would be hard to read in the dark version").
- **R6, the `.uqtheme` format.** It's TOML:
  ```toml
  # A uQ theme. Import it on the Appearance page of uQ Settings, or with
  # `uqctl theme import <file>`.
  format = 1
  id = "ocean"
  name = "Ocean"
  author = "Sam"          # optional

  [dark.colors]           # the same tables as themes/<id>/dark.toml, without `name`
  # bg, surface, surface_alt, overlay, text, text_muted, accent, accent_text, border,
  # success, warning, error
  [dark.ui]               # radius, gap_in, gap_out, border_width
  [dark.fonts]            # ui, mono, size
  [dark.icons]            # theme, cursor_theme, cursor_size
  [dark.terminal]         # ansi = [16 colours]
  [light.colors]          # … the same five tables for light
  [look]                  # optional: only keys from settings::Look::FIELDS, validated
  gaps_in = 4
  ```
  - The top-level keys are exactly `format`, `id`, `name`, `author?`, `dark`, `light` and
    `look?`. Anything else is refused with "This file has things a uQ theme can't hold: <key>"
    (the key cleaned of control characters, cut to 40).
  - `format` newer than 1 gives "This theme was made by a newer version of uQ."
  - Both variants are required.
  - Writing is deterministic (`to_text`), so `parse(to_text(t)) == t` and a second export is
    byte-identical.
- **R7, what Export holds.** The stored theme as the registry has it (never `with_appearance`
  overrides, G-F4), its name and author, and, with `with_look`, the non-null keys of the current
  `look` section. Wallpaper, appearance overrides, fonts the user picked and every other domain
  are never included.
- **R8, Import never switches themes.**
  - It installs the theme and returns its id. The page then offers **Use it now** (`SetTheme`).
  - With `with_look` and a `[look]` table, uqd merges the file's keys over the current `look`
    section and writes it through `set_section("look", merged, origin)`, the normal section
    path. It's validated and verified, and `persist(next, origin)` is still the single save
    point. That write is journalled (G-F8).
  - The page's checkbox reads "Also use its window gaps, corners and effects", off by default.
    `uqctl` has `--with-look`.
- **R9, order inside `ImportTheme`:**
  1. dev-session check;
  2. `uqtheme::parse` (every check above);
  3. conflict check: the id exists and `!replace` gives `Conflict("You already have a theme
     called <name>. Replace it?")`, the "Replace it?" suffix convention profiles use;
  4. the count limit (a replace doesn't count);
  5. root checks (R3);
  6. a `BeforeImport` restore point (a failure is logged, as for the other automatic points);
  7. `atomic_replace_dir`;
  8. re-scan;
  9. when the id is the active theme, `commit(self.state.clone(), Change::Theme, origin)`. That
     re-renders the theme files live and journals nothing, since `diff` is empty;
  10. the optional `look` write;
  11. `ThemesChanged`.

  When step 10 fails, the theme stays installed and the error says "The theme was added, but its
  window look couldn't be used: <reason>".
- **R10, the editor saves through `ImportTheme`.** The page builds the file with
  `editor::to_file` and calls `ImportTheme(text, replace, false, "settings")`. `replace` is
  true when editing an existing user theme. So there is **one** write path for user themes, and
  every saved theme gets the same checks as an imported one. A save also takes a `BeforeImport`
  restore point (harmless, and it keeps the code single-path).
- **R11, the seven base colours** map to tokens as follows (`theme::derive`):
  - `bg` → `bg`; `surface` → `surface`; `text` → `text`; `accent` → `accent`;
  - `good` → `success`; `warn` → `warning`; `bad` → `error`.
  - **Derived from those,** with `mix(a, b, t)` a linear blend in sRGB, rounded:
    - `surface_alt = mix(surface, text, 0.08)`;
    - `overlay = mix(surface, text, 0.16)`;
    - `border = mix(surface, text, 0.28)`;
    - `text_muted = mix(text, bg, 0.35)`, then stepped back toward `text` by 0.05 until it
      passes AA on `bg` and `surface`, or reaches `text`;
    - `accent_text`: the base theme's `accent_text` if it passes AA on the new accent, else
      `#000000` or `#ffffff`, whichever has the higher ratio.
  - **Copied from the base theme:** `ui`, `fonts`, `icons` and `terminal.ansi`. Terminal colours
    aren't editable in v1. (Notes question 2.)
  - Derivation is deterministic, so the same inputs always give the same file.
- **R12, Delete.**
  - **Refused:** a system id ("Themes that come with uQ can't be deleted"), an unknown id, and a
    dev session.
  - **The active theme.** When the id is active, uqd first calls `set_theme(DEFAULT, variant,
    origin)` (journalled, as any theme change). If that fails, nothing is deleted.
  - **What it returns:** `{switched: bool, profiles: [name…]}` (G-F9). Profile names are
    untrusted text, shown with `UqLabel`.
- **R13, one save point.** Nothing in this slice writes `state.toml` except through `commit` →
  `install` → `persist(next, origin)` or `set_section` → `persist`. Theme files are written
  beside the state, never inside it. This is the single save point slice A built (Global
  Constraints).
- **R14, Explain.** Nothing new. The theme row keeps no source label. A user theme that `user.lua`
  overrides is out of scope, because a theme isn't one Hyprland option.
- **R15, no MIME type.** `.uqtheme` gets no shared-mime-info entry. The file dialog filters on
  `*.uqtheme`, and `PKGBUILD` is unchanged. (Notes question 3.)
- **R16, shell chips.** `ThemeGroup.qml` shows `name` (with `UqText`, already plain text, as
  `tests/qml/tst_uqtext_plaintext.qml` checks) and keeps using ids for `SetTheme`. Names are cut
  to 24 characters in the chip.

**Spec deviations (recorded here and in the PR):**
1. **No inotify (G-F6).** Imports through uQ show at once; folders copied by hand appear within
   60 s, or when the Appearance page opens.
2. **The import's `look` write uses the caller's origin (G-F8),** not `import`.
3. **Four D-Bus calls and a signal the spec doesn't list:** `ListThemeInfo`, `ExportTheme`,
   `ImportTheme`, `DeleteTheme` and `ThemesChanged`. Spec §10 is updated in Task 10.
4. **User theme folders are named by full id (R1),** and the slug inside a `.uqtheme` has no
   prefix (R2).
5. **Saving from the editor is an import (R10),** so it also takes a restore point.
6. **Editing an existing user theme** is allowed (the spec names only "new from the current
   one"). It replaces the theme after the same checks.

## Global Constraints

These are copied from the spec and the user's standing rules, except the lines marked
**ruling**, which this plan decides.

### Process

- **K4, no Qt here.** The build container has no Qt, no Quickshell and no Lua. These run in CI
  only:
  - `uq-settings`;
  - both `qmltestrunner` suites;
  - the `tests/shell/*.sh` scripts that need Quickshell;
  - the `--ignored` Hyprland verify tests.

  Every piece of logic that can be Qt-free is: the page logic in
  `apps/uq-settings/src/pages/themes.rs` and `pages/themes/editor.rs`, and everything in
  uq-core.
  - Locally, run `cargo test -j 2 -p uq-core -p uqd -p uqctl -- --test-threads=2`.
  - **Page logic locally (as in slices C and D).** The scratch crate
    `/tmp/claude-0/-home-user/b86967db-9e53-50b9-9370-e4f499626b88/scratchpad/t7check` (call it
    `<t7check>`) symlinks:
    - `apps/uq-settings/src/pages`;
    - `desktop_entries.rs`, `registry.rs`, `search.rs`, `system_lists.rs`, `xkb_lists.rs` and
      `plugins.rs`;
    - the app's `tests/` folder.

    It builds them without Qt. Run
    `cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::themes -- --test-threads=2`
    (and `pages::appearance`, `registry`).
    - This slice adds no top-level app module that page logic needs: `pages/themes.rs` and
      `pages/themes/editor.rs` sit under the linked `pages/` folder.
    - If the crate is missing in a new session, recreate it as slice D's Global Constraints
      describe.
    - CI's `cargo test -p uq-settings` stays the authority.
- **Build parallelism (earlyoom).** Every cargo command uses `-j 2`, and test runs add
  `-- --test-threads=2`.
  - Set `CARGO_BUILD_JOBS=2` and `CARGO_INCREMENTAL=0` in the shell before the first build.
  - Don't run two cargo commands at once.
  - Run builds in the foreground and leave nothing running.
- **K5, commits.** Every commit message ends with:
  ```
  Co-Authored-By: Claude Sonnet 5.5 <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01HQqpuHDkHTjpqTLFahZCP1
  ```
  Use `git commit -F -` with a heredoc, so the trailers sit on their own lines after a blank line.
- **Tests never touch the real home.**
  - Use private `dbus-daemon` buses only (`uq_core::test_bus`), never the real session bus or
    `hyprctl`.
  - Every uqd rig builds its paths with `Paths::under(temp)`. That also puts
    `user_themes_dir` under the temp root (Task 1 extends `under_root_puts_everything_inside`).
- **Every commit is clean.** `cargo fmt`, `clippy -D warnings` and the tests pass at every
  commit. Keep new files at or under about 400 lines.
- **Dead code.**
  - uq-core's additions are `pub` in a library, so they need no gating.
  - `pages/themes.rs` and `pages/themes/editor.rs` live in the binary crate and are unused until
    Task 8. Each starts with
    `#![cfg_attr(not(test), allow(dead_code, reason = "wired by its model in Task 8"))]`, and
    Task 8 removes it.
- **CI flake.** `tests/shell/pam_stacks.sh` is known to flake in CI. If it's the only red job,
  re-run it. Every other failure is real.
- **No new crates.** `Cargo.lock` doesn't change.
- **Ownership (above).** Don't edit the other thread's paths. Make additive edits only to the
  shared files.

### Writes, origins and the save point (§3, slice A)

- Every uqd method that changes state takes a last `origin: &str` (`Origin::parse`). Here that
  means `ImportTheme` and `DeleteTheme`. `ExportTheme` and `ListThemeInfo` change nothing.
- `persist(next, origin)` is the single save point for `state.toml`. The only state changes this
  slice makes are:
  - the active-theme switch inside `DeleteTheme` (`set_theme`);
  - the re-render of a replaced active theme (`commit` with unchanged state, journalling
    nothing);
  - the optional `look` write (`set_section`).

  All three go through the existing paths.
- Sections go through `SetSection` / `Desktop::set_section`. No theme-specific section domain is
  added, and `SECTION_DOMAINS` doesn't change.
- **Ruling:** profile domains stay exactly as slices B, C and D left them (`PROFILE_DOMAINS`
  unchanged). A profile's `appearance.theme` may name a user theme, because `user-…` ids pass
  `valid_theme_name`.

### Untrusted input (§7.4, §10)

- **What's untrusted.** These are untrusted: a `.uqtheme` from anywhere, every file under
  `~/.local/share/uq/themes`, and the `text` argument of `ImportTheme`. They are data, never
  code:
  - no value is evaluated;
  - no path comes from a file except through `valid_user_id`;
  - no string reaches a config except a name that passed G-F1, or a font or icon name that passed
    R4.
- **Where strings go.** Colours are parsed into `Color` and written back only as `#rrggbb`.
  Fonts and icon names reach gsettings as argv (no shell) and `generated.lua` through
  `lua_string`.
- **Errors.** Error text built from a file goes through `profile`'s `plain()` pattern (control
  characters dropped), so a message can't carry a terminal escape to `uqctl`'s output.
- **The source file the user picks.** It may be a symlink: the user chose it, and it's read with
  the user's own rights. It must be a regular file once opened, at most 64 KiB, and valid UTF-8.
- **Names in the UI.** Everything shown in the app goes through `UqLabel` (never a stock
  control's `text:`, a `ToolTip`, `Kirigami.FormData.label`, a `Dialog.title` or an
  `InlineMessage`), as slice C's F28 requires:
  - theme names and authors;
  - profile names;
  - folder names in `skipped`.

  Dialog titles are fixed literals: "Import a theme", "Export a theme", "New theme",
  "Edit theme" and "Delete theme".

### Text (§9)

- Every user-facing string passes `scripts/lint-settings.sh`. **Ruling:** add `tokens.json`,
  `uqtheme` (as a word, not inside the file filter literal `*.uqtheme`, which the lint already
  ignores in `nameFilters`; confirm in Task 9), `ThemeRegistry`, `contrast.rs` and `WCAG` to
  `scripts/settings-jargon.txt`.
- **Labels:**
  - "New theme from this one…", "Import…", "Export…", "Edit…" and "Delete…";
  - "Yours" (the user-theme tag);
  - "Background", "Panels", "Text", "Accent", "Good", "Warning" and "Bad";
  - "Hard to read: <pair>" (from `derive::problem_text`).
- **Rust-side text isn't linted.** Tasks 1–3 add `messages_are_plain` tests: no `toml`, `TOML`,
  `serde`, `uqd`, `{`, `[` or `ThemeError` in any `Problem`/`ImportError` message.

## Review Focus

1. **Injection through a theme name or author.** A name such as `X\nshell /bin/sh -c evil`,
   `X */ * { color: red } /*`, `{{ 7*7 }}`, `X\u{202e}`, `X\r`, or a name of 65 characters is
   refused by `parse` and by `scan`. A name that passes renders every template with the name
   inside its comment on one line (Task 1 `hostile_names_never_leave_template_comments` renders
   all of `templates/*.j2` with each accepted edge name and checks the comment line count and
   the CSS comment balance).
2. **Path traversal and links.** These are refused before any path is built:
   - ids `../x`, `user-../x`, `user-`, `user-A`, `user-a/b`, `user-a\0`, a 46-character id;
   - a `.uqtheme` with `id = "../../.config/hypr"`.

   A symlinked root, a theme folder that's a symlink to `/etc`, and a variant file that's a
   symlink to `~/.ssh/id_ed25519` are never read, written or deleted through. Delete is never
   recursive. Tests: Task 1 `scan_skips_links_and_bad_names`, Task 4
   `import_refuses_linked_root`, `delete_never_follows_links` and `delete_is_not_recursive`.
3. **Size and schema.** These are refused with a plain message, never a panic:
   - a 64 KiB + 1 file;
   - a FIFO (it must not hang: non-blocking open);
   - a binary file;
   - a TOML table nested 5000 deep;
   - unknown top-level or nested keys;
   - a 17-colour `ansi`;
   - `radius = 300`;
   - `format = 2`;
   - a `look` key that isn't in `Look::FIELDS`, or one out of range.

   Tests: Task 2 `parse_refuses_*` and `fuzz_parse_never_panics` (seeded mutation, no new crate),
   and Task 11's corpus run end to end through `uqctl` → uqd.
4. **A bad theme can't break the desktop.**
   - A user theme that fails contrast, brightness or ranges never loads.
   - A broken file in the folder never stops uqd from starting.
   - The active theme vanishing never makes later commits fail (G-F10).
   - Deleting the active theme switches first, and a failed switch deletes nothing.

   Tests: Task 4 `startup_survives_broken_user_themes`, `active_theme_kept_when_folder_removed`
   and `delete_active_switches_then_deletes`.
5. **Import is all-or-nothing and undoable.**
   - A refused import writes no file and no restore point.
   - An accepted one saves the `BeforeImport` point before writing.
   - With `with_look`, exactly one `look` journal entry exists, undoable with `Undo`.
   - `user.lua`'s bytes and modification time never change.

   Task 4 tests `import_saves_point_then_installs`, `refused_import_changes_nothing`,
   `import_with_look_is_one_undoable_entry` and `user_lua_untouched`.
6. **Round trip (criterion 7).** Every system theme exports and imports to an equal theme (ids
   aside), and so does every theme the editor saves. Export → delete → import gives the same
   files. Task 2 `every_system_theme_round_trips`, Task 3 `derived_themes_import_cleanly`,
   Task 11 `export_import_on_a_second_home`.
7. **The shell can't reach the writes.** Only `ListThemeInfo` joins the bridge whitelist. Task 6
   tests `bridge_refuses_theme_writes`.

---

### Task 1: `uq_core::theme::user`: user theme rules, the safe scan and the layered registry

**Files:**
- Create: `crates/uq-core/src/theme/user.rs` and `crates/uq-core/src/theme/user/tests.rs`.
- Modify:
  - `crates/uq-core/src/theme.rs`:
    - `pub mod user;`;
    - `ThemeFile` gains `#[serde(default)] author: Option<String>`;
    - `ThemeRegistry` gains `info: BTreeMap<String, ThemeInfo>`, filled by `load_dir`;
    - `load_dir` skips a folder whose name starts with `user-`, with `tracing::warn!` (G-F2,
      R1);
  - `crates/uq-core/src/paths.rs`: `pub user_themes_dir: PathBuf` from `startup.data_home`
    (R1), and the `from_vars`/`under` tests;
  - `crates/uq-core/src/autostart/scan.rs`: `O_NONBLOCK` and `O_NOCTTY` become `pub(crate)`
    (G-F14; no behaviour change).

**Interfaces (produced, `uq_core::theme`):**
- `pub struct ThemeInfo { pub id: String, pub name: String, pub author: Option<String>, pub user: bool, pub missing: bool }`
  (`Serialize`, `Clone`, `PartialEq`).
- These methods are added to `impl ThemeRegistry`:
  - `pub fn info(&self, id: &str) -> Option<&ThemeInfo>`;
  - `pub fn infos(&self) -> Vec<&ThemeInfo>`: system first, then user, each by name
    case-insensitively;
  - `pub fn is_user(&self, id: &str) -> bool`;
  - `pub fn replace_user(&mut self, scan: user::UserScan, keep: Option<&str>)`. It drops every
    user theme, adds the scan's, and, if `keep` is a user id the scan lost, puts the old copy
    back with `missing: true` (G-F10).

**Interfaces (produced, `uq_core::theme::user`):**
- **Constants:**
  - `pub const PREFIX: &str = "user-";`
  - `pub const MAX_USER_THEMES: usize = 50;`
  - `pub const MAX_SCANNED: usize = 200;`
  - `pub const MAX_FILE_BYTES: u64 = 65_536;`
  - `pub const MAX_TEXT_CHARS: usize = 64;`
- **Ids:**
  - `pub fn valid_slug(s: &str) -> bool`;
  - `pub fn valid_user_id(id: &str) -> bool` (R2);
  - `pub fn slug_for(name: &str, taken: &[String]) -> String` (returns a slug; the caller adds
    `PREFIX`).
- `pub fn valid_display_text(s: &str) -> bool` (G-F1).
- **Problems.** `pub enum Problem` (`thiserror`, plain `Display`) has these variants:
  - `FolderName`, `Link`, `NotAFolder`;
  - `MissingVariant(Variant)`, `Unreadable(Variant)`, `BadFile(Variant)`;
  - `NamesDiffer`, `Name`, `Author`, `Font`, `IconName`;
  - `OutOfRange(&'static str)`;
  - `HardToRead { variant: Variant, text: String }`, `WrongBrightness(Variant)`;
  - `TooMany`.

  Each message is one plain sentence from R1–R5.
- `pub fn check_variant(t: &Theme, author: Option<&str>) -> Result<(), Problem>` checks the R4
  ranges and text, then R5 contrast and brightness. It uses `contrast::check_theme` and
  `derive::problem_text` (Task 3 adds `problem_text`; until then this task formats the pair
  itself, and Task 3 switches it over).
- `pub fn check_pair(dark: &Theme, light: &Theme, author: Option<&str>) -> Result<(), Problem>`
  checks both variants, plus the names being equal.
- **The scan:**
  - `pub struct UserTheme { pub dark: Theme, pub light: Theme, pub author: Option<String> }`;
  - `pub struct Skipped { pub folder: Option<String>, pub problem: Problem }`. `folder` is set
    only when the name passes `valid_theme_name`.
  - `pub struct UserScan { pub themes: Vec<UserTheme>, pub skipped: Vec<Skipped>, pub root_is_link: bool }`.
  - `pub fn scan(root: &Path) -> UserScan`. It applies R3: `symlink_metadata` on the root, then
    on each entry. Each variant is read with `read_capped_if(path, |m| same (dev, ino) as the
    no-follow metadata)`, then `Theme::from_toml`, then `check_pair`. Entries are sorted by
    name, at most `MAX_SCANNED` are looked at, and after `MAX_USER_THEMES` good themes the rest
    are `TooMany`.
- **Fingerprint:**
  - `pub type Fingerprint = Vec<(String, u64, i64, i64)>` (folder, size, mtime secs, mtime
    nanos of each variant file).
  - `pub fn fingerprint(root: &Path) -> Fingerprint` is metadata only, with the same no-follow
    rules.
- `pub fn variant_toml(t: &Theme, author: Option<&str>) -> String` writes a variant file that
  `Theme::from_toml` reads back equal. It goes through `toml::to_string` of a `Serialize` mirror
  of `ThemeFile`, so the quoting is the `toml` crate's.

- [ ] **Step 1: Write the failing tests** in `theme/user/tests.rs`:
  - `ids_and_slugs`: `user-ocean` and `user-a1-b2` are accepted. Refused: `user-`, `ocean`,
    `user-Ocean`, `user--x`, `user-x-`, `user-../x`, `user-a/b`, `user-a\0`, and a 46-char id.
    Also `slug_for("My Ocean!", &[])` is `my-ocean`, `slug_for("My Ocean", &["my-ocean"])` is
    `my-ocean-2`, `slug_for("ëë", &[])` is `theme`, and 60 letters give 40.
  - `display_text_rules` (G-F1): each hostile name from Review Focus 1 is refused; `Ocean`,
    `Café Noir` and `Rosé–Pine` (en dash) are accepted.
  - `check_variant_ranges` (R4): each limit at its edge passes and one past it fails, with the
    plain field name in the message.
  - `check_pair_contrast_and_brightness` (R5): the fixture passes; `text_muted == surface` gives
    `HardToRead` naming "dark"; a dark variant with a light background gives `WrongBrightness`.
  - `scan_reads_good_and_skips_bad`. A tempdir root holds:
    - `user-good` (passes);
    - `user-onlydark` (`MissingVariant(Light)`);
    - `ocean` (`FolderName`);
    - `user-big` (a 65 537-byte file: `Unreadable`);
    - `user-dim` (`HardToRead`);
    - a stray file `notes.txt` (ignored).

    The scan returns one theme and four skips, in name order.
  - `scan_skips_links_and_bad_names` (Review Focus 2): a theme folder symlinked to another good
    theme gives `Link`; a variant file symlinked to a good file gives `Link`; a symlinked root
    gives `root_is_link` and nothing read; a FIFO named `dark.toml` doesn't block (it's
    skipped).
  - `scan_caps_themes`: 51 good folders give 50 themes and one `TooMany`.
  - `fingerprint_changes_only_with_files`: touching `dark.toml` changes it; adding a stray file
    in the root doesn't.
  - `variant_toml_round_trips` for every system theme from `themes/`, with and without an
    author.
  - `hostile_names_never_leave_template_comments` (Review Focus 1). Render
    `crate::render::render` with `templates/` for a fixture theme whose name is each *accepted*
    edge name: 64 chars, `a#b`, `a/*b`, `a"b`, `a\\b`, `a'b`, and `é`. Then check:
    - each output's first comment line holds the whole name;
    - the line count equals the plain fixture's;
    - the CSS files' `/*` and `*/` counts are equal.
  - `messages_are_plain` (Global Constraints, Text).
  - In `theme.rs` tests: `registry_infos_mark_user_and_missing`;
    `load_dir_skips_user_prefixed_system_folders`; `author_is_optional_in_theme_files`; and the
    existing system-theme tests unchanged.
  - In `paths.rs` tests: `user_themes_dir` is `/dh/uq/themes` with `XDG_DATA_HOME=/dh`, and is
    inside the root for `under`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core theme -- --test-threads=2`. Expected: FAIL
  (module missing).
- [ ] **Step 3:** Implement. Keep `theme.rs` growth to the field, the `info` map, the
  `user-` skip and the four methods.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and
  `cargo clippy -j 2 -p uq-core --all-targets -- -D warnings`. Expected: PASS, including
  `crates/uq-core/tests/themes.rs` and the golden tests unchanged (G-F3).
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): user themes: id rules, safe folder scan and a layered theme registry`

### Task 2: `uq_core::theme::uqtheme`: the `.uqtheme` file

**Files:**
- Create: `crates/uq-core/src/theme/uqtheme.rs` and `crates/uq-core/src/theme/uqtheme/tests.rs`.
- Modify: `crates/uq-core/src/theme.rs` (`pub mod uqtheme;`).

**Interfaces (produced, `uq_core::theme::uqtheme`):**
- **Constants:**
  - `pub const FORMAT: i64 = 1;`
  - `pub const MAX_FILE_BYTES: u64 = 65_536;`
  - `pub const EXTENSION: &str = "uqtheme";`
  - `pub const HEADER: &str` (R6's two comment lines).
- `pub struct UqTheme { pub slug: String, pub name: String, pub author: Option<String>, pub dark: Theme, pub light: Theme, pub look: Option<serde_json::Map<String, serde_json::Value>> }`.
  - `dark.id` and `light.id` are `user-<slug>`.
  - `name` is copied into both variants.
- **Errors.** `pub enum ImportError` (`thiserror`, plain) has these variants:
  - `TooBig`, `Unreadable(String)`, `NotATheme`, `NewerFormat(i64)`;
  - `BadId`, `BadName`, `BadAuthor`;
  - `Unknown(String)` (cleaned key);
  - `Variant { variant: Variant, problem: user::Problem }`;
  - `BadLook(String)`.

  The messages:
  - "The file is too big to be a theme." (`TooBig`);
  - "This file isn't a uQ theme." (`NotATheme`);
  - "This theme was made by a newer version of uQ." (`NewerFormat`);
  - "The theme's id can only use a–z, 0–9 and dashes." (`BadId`);
  - "A theme needs a name of 1 to 64 characters, without special characters." (`BadName`);
  - "The author's name can't be used." (`BadAuthor`);
  - "This file has things a uQ theme can't hold: <key>." (`Unknown`);
  - "The dark version: <problem>" (`Variant`);
  - "The window look in this theme can't be used: <reason>" (`BadLook`).
- **Reading and parsing:**
  - `pub fn read_file(path: &Path) -> Result<String, ImportError>`: non-blocking open (G-F14),
    regular file once opened, at most `MAX_FILE_BYTES` (else `TooBig`), UTF-8 (else
    `NotATheme`).
  - `pub fn parse(text: &str) -> Result<UqTheme, ImportError>` runs in order:
    - the size check;
    - `toml::Table` parse (any error is `NotATheme`);
    - the `format` check;
    - the exact top-level key set (R6);
    - `id` (strip one `user-`, then `valid_slug`), `name`, `author` (G-F1; an empty author is
      `None`);
    - each variant deserialised into a private `VariantBody` with `deny_unknown_fields`, made
      into a `Theme` with the file's name and the `user-<slug>` id. A colour error is
      `Variant { problem: BadFile }`;
    - `user::check_pair`;
    - `look`: each key must be in `settings::Look::FIELDS`. The table goes through
      `Settings::default().set_section("look", v)` on a scratch `Settings`, which keeps uQ's
      ranges as the one source; a refusal is `BadLook` with the plain reason.
- **Writing:**
  - `pub fn to_text(t: &UqTheme) -> String` is deterministic: `HEADER`, the scalars, then
    `[dark.*]`, `[light.*]` and `[look]`, with each table's keys in `ThemeFile` order.
  - `pub fn from_pair(dark: &Theme, light: &Theme, author: Option<&str>, look: Option<Map>) -> UqTheme`:
    the slug is the id without `user-`, for export.
  - `pub fn install_files(t: &UqTheme) -> RenderedFiles` gives `{"dark.toml", "light.toml"}`
    via `user::variant_toml`.

- [ ] **Step 1: Write the failing tests** in `uqtheme/tests.rs`:
  - `every_system_theme_round_trips` (Review Focus 6): for each of the seven system themes,
    `parse(to_text(from_pair(dark, light, None, None)))` gives back the same colours, ui, fonts,
    icons and terminal, and `to_text` of the result is byte-identical.
  - `look_round_trips_and_is_checked`: `look = {gaps_in = 4, blur = false}` round-trips;
    `gaps_in = 99`, `wallpaper = "x"` and `look = 3` are `BadLook`/`Unknown`.
  - `id_prefix_is_stripped_once`: `id = "user-ocean"` gives slug `ocean`, and
    `id = "user-user-x"` gives slug `user-x`, then id `user-user-x`.
  - The `parse_refuses_*` tests (Review Focus 3), one per case:
    - `parse_refuses_too_big` (64 KiB + 1, including when padded with a comment);
    - `parse_refuses_nested` (5000 nested tables);
    - `parse_refuses_unknown_keys` (top-level `script = "x"`, `[dark.colors] glow = "#000000"`,
      `[dark.shell]`);
    - `parse_refuses_bad_values` (17 ansi colours, `radius = 300`, `#fff`, `rgb(1,2,3)`,
      `size = "11"`);
    - `parse_refuses_format` (missing, `"1"`, `2`);
    - `parse_refuses_hostile_text` (every Review Focus 1 name as `name` and as `author`);
    - `parse_refuses_traversal_ids` (Review Focus 2 ids);
    - `parse_refuses_missing_variant`;
    - `parse_refuses_low_contrast`.
  - `read_file_refuses_big_binary_and_fifo`: a FIFO returns `Unreadable` without blocking (run
    the read with a 2 s `std::thread` timeout guard).
  - `fuzz_parse_never_panics`: a seeded xorshift (in the test, no crate) applies 5000
    mutations to a good file. The mutations are byte flips, truncation, duplicating a line,
    inserting one of a list of hostile snippets (`\n`, `*/`, `{{`, `"""`, `[[`, `\u{202e}`,
    `../`), and deleting a line. `parse` must return without panicking, and every `Ok` result
    must pass `user::check_pair`, have a valid slug, and have names that pass
    `valid_display_text`.
  - `messages_are_plain`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core uqtheme -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and clippy on uq-core.
  Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): the .uqtheme file: strict parse, deterministic export and import fuzzing`

### Task 3: `uq_core::theme::derive`: seven base colours make a theme; plain contrast messages

**Files:**
- Create: `crates/uq-core/src/theme/derive.rs` and `crates/uq-core/src/theme/derive/tests.rs`.
- Modify:
  - `crates/uq-core/src/theme.rs` (`pub mod derive;`);
  - `crates/uq-core/src/contrast.rs` (additive: `pub fn plain_name(token: &str) -> &'static str`);
  - `crates/uq-core/src/theme/user.rs` (`check_variant` uses `derive::problem_text`).

**Interfaces (produced, `uq_core::theme::derive`):**
- `pub struct BaseColors { pub bg: Color, pub surface: Color, pub text: Color, pub accent: Color, pub good: Color, pub warn: Color, pub bad: Color }`.
  It is `Serialize`/`Deserialize` as `#rrggbb` strings, with `deny_unknown_fields`.
- `pub const KEYS: [(&str, &str); 7]`, the field and its plain label ("Background", "Panels",
  "Text", "Accent", "Good", "Warning", "Bad").
- `pub fn base_of(t: &Theme) -> BaseColors`.
- `pub fn mix(a: Color, b: Color, t: f64) -> Color`.
- `pub fn derive(base: &Theme, c: &BaseColors, name: &str) -> Theme` (R11). The id is left for
  the caller.
- `pub fn problems(t: &Theme) -> Vec<String>`: plain lines for every contrast failure and the
  brightness check, such as "Muted text on panels is hard to read (3.1 to 1; it needs 4.5 to
  1)." and "The dark version has a light background."
- `pub fn problem_text(variant: Variant, f: &ContrastFailure) -> String` (used by
  `user::check_variant`).
- In `contrast.rs`, `plain_name` maps:
  - `text` → "Text";
  - `text_muted` → "Muted text";
  - `bg` → "the background";
  - `surface` → "panels";
  - `surface_alt` → "raised panels";
  - `accent` → "the accent";
  - `accent_text` → "Text on the accent".

- [ ] **Step 1: Write the failing tests** in `derive/tests.rs`:
  - `base_of_then_derive_keeps_system_themes_passing`: for each system theme, `derive(t,
    &base_of(t), &t.name)` passes `contrast::check_theme` and the brightness check.
    (The derived tokens may differ from the hand-made ones; that's expected.)
  - `derive_is_deterministic`.
  - `muted_text_steps_until_it_passes`: a pair where `mix(text, bg, 0.35)` fails gives a
    `text_muted` that passes, or equals `text`.
  - `accent_text_picks_the_readable_one`: a yellow accent gives `#000000`, and a navy accent
    gives `#ffffff`. The base's `accent_text` is kept when it passes.
  - `copies_ui_fonts_icons_terminal`.
  - `derived_themes_import_cleanly` (Review Focus 6). Build a `UqTheme` from derived dark and
    light variants with slug `ocean`. Then `uqtheme::parse(uqtheme::to_text(..))` is `Ok` and
    equal.
  - `problems_are_plain_and_name_the_pair`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core derive -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. Switch `user::check_variant` to `problem_text`.
- [ ] **Step 4:** Run `cargo test -j 2 -p uq-core -- --test-threads=2` and clippy. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(core): derive a theme from seven base colours, with plain contrast messages`

### Task 4: uqd: the user theme store (load, re-scan, export, import, delete)

**Files:**
- Create: `crates/uqd/src/themes.rs` and `crates/uqd/src/themes/tests.rs`.
- Modify:
  - `crates/uqd/src/lib.rs` (`mod themes;`);
  - `crates/uqd/src/service.rs` (G-F5):
    - fields `pub(crate) user_themes: uq_core::theme::user::Fingerprint` and
      `pub(crate) skipped_themes: Vec<uq_core::theme::user::Skipped>`;
    - `Event::ThemesChanged`;
    - `Desktop::new` calls `self.load_user_themes()` before the stale-theme fallback, so a
      saved `user-…` theme survives a restart.
  - `crates/uqd/src/dbus/signals.rs`: the `kind` arm `"ThemesChanged"` only. The forward arm
    lands in Task 5, and until then the match uses `Event::ThemesChanged => Ok(())`, so the
    build stays green.

**Interfaces (produced, `impl Desktop` in `themes.rs`):**
- `pub(crate) fn load_user_themes(&mut self)`. It scans `paths.user_themes_dir`, calls
  `themes.replace_user(scan, Some(&state.theme))`, and stores the fingerprint and the skips.
- `pub fn refresh_themes(&mut self) -> bool`. If the fingerprint changed, it reloads and sends
  `ThemesChanged`, and returns whether anything changed (G-F6).
- `pub fn theme_info(&mut self) -> serde_json::Value`. It calls `refresh_themes()` first, then
  returns `{themes: [ThemeInfo + active: bool], skipped: [{folder?, reason}], root_is_link}`.
- `pub fn export_theme(&self, id: &str, with_look: bool) -> Result<String, DesktopError>`
  (R7):
  - an unknown id gives `UnknownTheme`;
  - `with_look` adds the non-null keys of `state.section("look")`.
- `pub async fn import_theme(&mut self, text: &str, replace: bool, with_look: bool, origin: Origin) -> Result<String, DesktopError>`
  follows R9's order exactly.
  - Errors map as: parse → `InvalidArgument(plain)`; exists → `Conflict`; limit, link and dev
    session → `InvalidArgument`; io → `Io`.
- `pub async fn delete_theme(&mut self, id: &str, origin: Origin) -> Result<serde_json::Value, DesktopError>`
  (R12, R3). It finds the names of profiles that use the theme through
  `self.profiles.list()` and `values["appearance"]["theme"]`.
- `fn user_root_ok(&self) -> Result<(), DesktopError>` (R3).
- `pub(crate) const DEFAULT_THEME` is `State::default().theme`, reused.

- [ ] **Step 1: Write the failing tests** in `themes/tests.rs`. Use the existing rig pattern
  (`Paths::under(tempdir)`, `themes_dir = repo/themes`, a fake compositor):
  - `startup_loads_user_themes_and_keeps_active`: a saved state naming `user-ocean`, with that
    folder present, starts with `user-ocean` active.
  - `startup_survives_broken_user_themes` (Review Focus 4): the Task 1 skip folders are present;
    uqd starts; `theme_info` lists them under `skipped`.
  - `import_saves_point_then_installs` (Review Focus 5):
    - a `BeforeImport` restore point exists, with the pre-import state;
    - `<root>/user-ocean/{dark,light}.toml` exist and read back equal;
    - `ListThemes` contains `user-ocean`;
    - `ThemesChanged` was sent;
    - the theme isn't active.
  - `refused_import_changes_nothing`: for a sample of the Task 2 refusals, there's no new
    restore point, no folder and no event.
  - `import_conflict_then_replace`: the second import gives `Conflict` ending "Replace it?";
    with `replace` it succeeds, and the count stays 1.
  - `replacing_active_theme_rerenders_without_history`: the theme dir's `tokens.json` changes,
    and the journal length doesn't.
  - `import_with_look_is_one_undoable_entry`:
    - one `look` entry, with the caller's origin (G-F8);
    - `undo(entry)` puts the old `look` back;
    - the theme stays installed.
  - `import_look_rejected_by_verify_keeps_theme`: the fake compositor rejects the reload. The
    theme is installed, and the error starts "The theme was added, but".
  - `import_refuses_linked_root` and `import_refuses_in_dev_session` (G-F11).
  - `import_caps_at_fifty`.
  - `export_matches_registry_and_never_holds_overrides`: `appearance.ui_font = "Comic"` set;
    the export's fonts are the theme's; with `with_look` the `[look]` table holds only
    non-null keys.
  - `delete_active_switches_then_deletes`:
    - the theme switch is journalled with the origin;
    - the folder is gone;
    - the result is `{switched: true, profiles: []}`.
  - `delete_reports_profiles_using_it`.
  - `delete_refuses_system_and_unknown`.
  - `delete_never_follows_links` (Review Focus 2): `user-x` is a symlink to a folder holding
    `dark.toml`. The target's files survive, and the link is either refused or removed as a
    link only.
  - `delete_is_not_recursive`: a folder with an extra `notes.txt` keeps it. The theme is gone
    from the list.
  - `active_theme_kept_when_folder_removed` (G-F10): remove the active theme's folder by hand
    and call `refresh_themes`. `theme_info` shows it `missing: true`, and a `set_section("look",
    …)` commit still succeeds.
  - `refresh_without_change_reads_nothing`: a second `refresh_themes` returns false and sends
    no event.
  - `user_lua_untouched` (§1 criterion 8): `user.lua`'s bytes and mtime are the same after an
    import, an export and a delete.
  - `no_state_key_from_theme_store` (G-F13): an import without look leaves `state.toml`
    byte-identical.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd themes -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -- --test-threads=2` and
  `cargo clippy -j 2 -p uqd --all-targets -- -D warnings`. Expected: PASS.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): user themes: load, re-scan, export, import with a restore point, delete`

### Task 5: uqd D-Bus: `ListThemeInfo`, `ExportTheme`, `ImportTheme`, `DeleteTheme`, `ThemesChanged`

**Files:**
- Modify (all additive):
  - `crates/uqd/src/dbus.rs` (four methods, one signal);
  - `crates/uqd/src/dbus/signals.rs` (the forward arm);
  - `crates/uqd/src/main.rs` (the 60 s tick also calls `refresh_themes()`);
  - `crates/uq-core/src/client.rs` (the four proxy methods and the signal).
- Create: `crates/uqd/tests/themes_dbus.rs`.

**Interfaces (produced):**
- `async fn list_theme_info(&self) -> Result<String, UqError>` is Task 4's JSON.
- `async fn export_theme(&self, id: &str, with_look: bool) -> Result<String, UqError>`.
- `async fn import_theme(&self, text: &str, replace: bool, with_look: bool, origin: &str) -> Result<String, UqError>`.
  It refuses `text.len() > MAX_FILE_BYTES` before taking the desktop lock.
- `async fn delete_theme(&self, id: &str, origin: &str) -> Result<String, UqError>`.
- `#[zbus(signal)] async fn themes_changed(e) -> zbus::Result<()>`.
- `DesktopProxy` gains `list_theme_info`, `export_theme`, `import_theme`, `delete_theme` and
  `receive_themes_changed`, each doc-commented as in `dbus.rs`.

- [ ] **Step 1: Write the failing tests** in `crates/uqd/tests/themes_dbus.rs` (private bus,
  `Paths::under`):
  - `import_list_set_delete_over_dbus`. Over D-Bus:
    - `ImportTheme` returns `user-…`;
    - a `ThemesChanged` signal arrives;
    - `ListThemeInfo` holds it with `user: true`;
    - `SetTheme(id, "light", "settings")` works;
    - `DeleteTheme` switches and returns `switched: true`;
    - `ListThemes` no longer holds it.
  - `import_errors_have_plain_names`: a conflict is `org.uq.Desktop1.Error.Conflict`, and a bad
    file is `InvalidArgument` with the plain message.
  - `oversized_text_refused_before_lock`.
  - `export_round_trips_over_dbus`.
- [ ] **Step 2:** Run `cargo test -j 2 -p uqd --test themes_dbus -- --test-threads=2`.
  Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqd -p uq-core -- --test-threads=2` and clippy on both.
  Expected: PASS, and the existing `crates/uqd/tests/dbus.rs` is unchanged.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqd): theme list, export, import and delete on D-Bus`

### Task 6: uqctl `theme` commands; the bridge and the shell's chips show names

**Files:**
- Create: `crates/uqctl/src/theme_cmd.rs`.
- Modify:
  - `crates/uqctl/src/main.rs`: `ThemeCmd` and its three arms move to `theme_cmd.rs`, and
    `main` dispatches `Cmd::Theme { cmd }` to `theme_cmd::run`, as for `Cmd::Profile`. The
    existing `cli_parses_theme_set` test moves with it.
  - `crates/uqctl/src/bridge/mod.rs`: `"ListThemeInfo"` joins the pass-through arm (G-F12).
  - `crates/uqctl/src/bridge/uqd.rs`: `Request::ListThemeInfo`, whose reply is parsed as JSON.
  - `shell/quick/ThemeGroup.qml`: R16.
  - `shell/logic/quickui.js`: `themeInfoList(v)`.
  - `tests/qml/tst_quickui.qml` and `crates/uqctl/tests/bridge_dbus.rs`.

**Interfaces (produced):**
- `theme_cmd::ThemeCmd` has these commands:
  - `Get`;
  - `List { #[arg(long)] long: bool }`;
  - `Set { id, variant }`;
  - `Import { file, #[arg(long)] replace, #[arg(long)] with_look }`;
  - `Export { id, file: Option<PathBuf>, #[arg(long)] with_look }`;
  - `Delete { id }`.
- `pub fn info_lines(json: &str, long: bool) -> Result<Vec<String>, String>`:
  - plain `list` prints ids only, unchanged (VM checks and scripts use it, G-F7);
  - `--long` prints `* user-ocean  Ocean  (yours)`, with names through `plain()`.
- `pub async fn run(proxy, cmd) -> Result<(), String>`. The messages never name the service
  (`profile_cmd`'s K6 rule):
  - a conflict adds "(run again with --replace)";
  - `delete` prints "Switched to the default theme first." and one line per profile;
  - export with no file prints to stdout, and with a file uses `atomic_write`.
- In `quickui.js`, `themeInfoList(v)` turns `{themes:[…]}` into `[{id, name}]`:
  - only string ids pass;
  - a name missing or not a string becomes the id;
  - names are cut to 24 characters.

- [ ] **Step 1: Write the failing tests:**
  - In `theme_cmd.rs`:
    - `cli_parses_theme_commands` (all six, with flags);
    - `info_lines_short_and_long`;
    - `info_lines_strip_control_chars` (a name with `\x1b[2J` prints without the escape);
    - `run_imports_exports_deletes` (a private bus and a real uqd, as
      `profile_cmd::run_imports_exports_and_switches` does);
    - `errors_never_name_the_service`;
    - `import_reads_capped_file` (a 64 KiB + 1 file is refused before any call).
  - In `crates/uqctl/tests/bridge_dbus.rs`:
    - `list_theme_info_passes_through`;
    - `bridge_refuses_theme_writes` (Review Focus 7): `ImportTheme`, `ExportTheme` and
      `DeleteTheme` through the bridge give its unknown-method error.
  - In `tests/qml/tst_quickui.qml`: `test_theme_info_list` (a malformed reply gives `[]`; a
    non-string name falls back to the id; a 40-char name is cut). CI only (K4).
- [ ] **Step 2:** Run `cargo test -j 2 -p uqctl -- --test-threads=2`. Expected: FAIL.
- [ ] **Step 3:** Implement. In `ThemeGroup.qml`:
  - `refreshTheme` calls `ListThemeInfo` first and falls back to `ListThemes` on failure;
  - the chip's model holds `{id, name}`;
  - `Accessible.name` is "Theme " + name;
  - `setTheme` gets the id.
- [ ] **Step 4:** Run `cargo test -j 2 -p uqctl -- --test-threads=2`, clippy on uqctl, and
  `scripts/lint-shell.sh`. Expected: PASS. CI runs `tests/qml`.
- [ ] **Step 5:** Commit (K5 trailers): `feat(uqctl): theme import, export and delete; quick settings show theme names`

### Task 7: uq-settings: the themes page logic (Qt-free), the editor logic and the fake uqd

**Files:**
- Create:
  - `apps/uq-settings/src/pages/themes.rs`;
  - `apps/uq-settings/src/pages/themes/editor.rs`;
  - `apps/uq-settings/src/pages/themes/tests.rs`;
  - `apps/uq-settings/src/pages/fake_uqd/themes.rs`.
- Modify:
  - `apps/uq-settings/src/pages/mod.rs` (`pub mod themes;`);
  - `apps/uq-settings/src/pages/fake_uqd.rs`: it serves the four methods and the signal from
    `fake_uqd/themes.rs`, which holds user themes in memory and uses the **real**
    `uqtheme::parse`, so the fake refuses what uqd refuses. It also gains a `refuse_next` hook
    for theme calls, like slice D's F32.

**Interfaces (produced):**
- `pub enum Op` has these operations:
  - `Refresh`;
  - `Import { path: PathBuf, replace: bool, with_look: bool }`;
  - `Export { id: String, path: PathBuf, with_look: bool }`;
  - `Delete { id: String }`;
  - `OpenEditor { from: String, edit: bool }`;
  - `Save { draft: Value }`;
  - `CloseEditor`;
  - `Use { id: String }` (`SetTheme` with the current variant).
- `pub struct ThemesState`. It holds the connection, a write lock, the editor session
  `Mutex<Option<Session>>` and the shown notices. Its sequence numbers follow
  `pages/profiles.rs`'s `Shown` pattern:
  - `notice`;
  - `conflict {seq, path, message}` (uqd's message without "Replace it?");
  - `deleted {seq, switched, profiles}`;
  - `imported {seq, id, name}`, which drives **Use it now**.
- The snapshot has this state:
  ```
  {themes: [{id, name, author, user, active, missing}], skipped: [{folder, reason}],
   rootIsLink, theme, variant, editor: null | {base, edit, id, name, author,
   colors: {dark: {bg…bad}, light: {…}}}, notice, conflict, deleted, imported}
  ```
- **The editor session.** `OpenEditor` calls `ExportTheme(from, false)` and
  `uqtheme::parse`s the result. It keeps both base variants in the session and seeds the colours
  with `derive::base_of`:
  - **New theme:** the name is "<name> copy" and the id is empty.
  - **Edit (`edit: true`, user themes only):** the name and id are kept.
- `pages::themes::editor`, which is pure:
  - `pub fn preview(session: &Session, draft: &Value) -> Value`. It returns
    `{dark: {tokens, problems: [..]}, light: {...}, ok: bool, nameProblem: ""|text}`, where
    `tokens` are the derived `Colors` as hex for the swatches.
  - `pub fn to_file(session: &Session, draft: &Value, taken: &[String]) -> Result<(String, bool), String>`
    returns the `.uqtheme` text and `replace`. A new theme gets
    `user::slug_for(name, taken)`, and an edit keeps its slug. It refuses `!ok` with the first
    problem.
  - `pub fn draft_colors(draft: &Value) -> Result<(BaseColors, BaseColors), String>`: each value
    must be `#rrggbb` (a `#RGB` short form is expanded; anything else gives "Use a colour like
    #1e2a3b").
- **Reading and writing files.** `Import` reads the file with `uqtheme::read_file` (the caps
  apply in the app too) and calls `import_theme(text, replace, with_look, ORIGIN)`. `Export`
  calls `export_theme` and then `atomic_write`s the file, failing with "Can't save the file: …".

- [ ] **Step 1: Write the failing tests** in `pages/themes/tests.rs` (fake uqd):
  - `snapshot_lists_names_and_marks_user_and_active`.
  - `import_then_use_it_now`: an `imported` notice with the name; `Use` makes it active.
  - `import_conflict_then_replace` (the message has no "Replace it?").
  - `import_refusals_are_plain`: a too-big file is refused in the app before any call (the fake
    sees no call), and a low-contrast file shows uqd's plain text.
  - `export_writes_the_file_and_can_include_look`.
  - `delete_active_reports_switch_and_profiles`.
  - `editor_new_from_current_seeds_seven_colours` (the colours equal `base_of` of the current
    theme's stored variants, not `tokens.json`, G-F4).
  - `editor_preview_blocks_failing_variant`: setting dark `text` to the dark `bg` gives
    `ok: false` and a problem naming "Text on the background".
  - `editor_save_new_gets_free_slug` (`taken` holds `user-ocean`, so the result is
    `user-ocean-2`).
  - `editor_edit_replaces_same_id`.
  - `editor_refuses_hostile_name` (G-F1, before any call).
  - `every_op_round_trips` (§9, slice A): after each write op, a fresh `ThemesState` on the same
    fake reads the same `themes`, `theme` and `variant`.
  - `uqd_down_gives_empty_state_without_an_error`.
- [ ] **Step 2:** Run
  `cargo test -j 2 --manifest-path <t7check>/Cargo.toml pages::themes -- --test-threads=2`.
  Expected: FAIL.
- [ ] **Step 3:** Implement. Add the dead-code attribute (Global Constraints).
- [ ] **Step 4:** Run the same t7check command, plus `pages::appearance` and `pages::profiles`
  (the fake changed). Expected: PASS. CI runs `cargo test -p uq-settings` with clippy.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): theme import, export, delete and editor logic`

### Task 8: uq-settings: `ThemesModel`, `Follow.themes` and the registry entries

**Files:**
- Create: `apps/uq-settings/src/themes_model.rs`.
- Modify:
  - `apps/uq-settings/build.rs` (the model file);
  - `apps/uq-settings/src/main.rs` (`mod themes_model;`);
  - `apps/uq-settings/src/model_core.rs`: `Follow` gains `themes: bool`, and `follow_uqd` adds
    `receive_themes_changed` when it's set (slice D's F8 pattern);
  - every existing model's `FOLLOW` literal gains `themes: false`, except `appearance_model.rs`,
    which gets `themes: true` so the theme row refreshes. The models are `appearance_model.rs`,
    `compat_model.rs`, `displays_model.rs`, `history_model.rs`, `input_model.rs`,
    `look_model.rs`, `profiles_model.rs`, `rules_model.rs`, `shell_model.rs`,
    `startup_model.rs` and `wallpaper_model.rs`;
  - `apps/uq-settings/src/pages/appearance.rs`: the snapshot's `themes` becomes
    `[{id, name, user}]` from `ListThemeInfo`, falling back to ids. The round-trip test is
    updated, with `FIELDS` unchanged;
  - `apps/uq-settings/src/registry.rs` (additive): three entries in `APPEARANCE_SETTINGS` (its
    array length grows from 10 to 13);
  - `pages/themes.rs` and `pages/themes/editor.rs`: drop the dead-code attribute.

**Interfaces (produced):**
- `ThemesModel` (`#[qml_element]`):
  - properties `state_json`, `error` and `busy`;
  - invokables:
    - `refresh()`;
    - `import_from(url, replace, with_look)` and `export_to(id, url, with_look)` (both through
      `crate::pages::wallpaper::local_path`, wrapped like `profiles_model.rs`'s `dialog_path`);
    - `remove(id)`;
    - `open_editor(from, edit)`, `close_editor()` and `save(draft_json)`;
    - `use_theme(id)`;
    - the synchronous `preview(draft_json) -> QString` (slice C's F17 pattern).
  - Its `FOLLOW` is `theme: true`, `themes: true` and `history: false`.
- Registry entries:
  - `entry("theme-new", "New theme from this one", &["custom", "create", "colours", "colors", "edit", "make"])`;
  - `entry("theme-import", "Import a theme", &["share", "file", "load", "open", "uqtheme"])`;
  - `entry("theme-export", "Export a theme", &["share", "file", "save", "backup", "uqtheme"])`.

- [ ] **Step 1: Write the failing tests** in `registry.rs`:
  - `theme_entries_on_appearance_page`.
  - The existing `every_setting_has_keywords`, `no_duplicate_setting_ids` and
    `ids_unique_and_categories_ordered` must still pass.
  - In `pages/appearance.rs`: `themes_carry_names_and_user_flag`.
- [ ] **Step 2:** Run `cargo test -j 2 --manifest-path <t7check>/Cargo.toml registry -- --test-threads=2`
  and `pages::appearance`. Expected: FAIL.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Run the t7check commands again. Expected: PASS. CI builds the model.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): ThemesModel and theme search entries`

### Task 9: uq-settings QML: the theme row, `ThemeChip`, `ThemeEditor` and their tests

**Files:**
- Create:
  - `apps/uq-settings/qml/components/ThemeChip.qml` and `ThemeEditor.qml`;
  - `tests/settings-qml/tst_theme_chip.qml` and `tst_theme_editor.qml`, with a fixture
    `tests/settings-qml/fixtures/themes.json`.
- Modify:
  - `apps/uq-settings/qml/pages/AppearancePage.qml`;
  - `apps/uq-settings/build.rs` (the two QML files);
  - `scripts/settings-jargon.txt` (Global Constraints, Text).

**Interfaces (produced):**
- **`ThemeChip.qml`:**
  - `property var theme` (`{id, name, user, active, missing}`);
  - the name shown in a `UqLabel` (`objectName: "themeName"`, `Text.PlainText`);
  - a "Yours" tag for user themes and "Its files were removed" for missing ones;
  - `signal chosen(string id)`;
  - for user themes, a `QQC2.ToolButton` menu: **Edit…**, **Export…**, **Delete…**
    (signals `edit(id)`, `exportTheme(id)`, `remove(id)`).
  - Its `Accessible.name` is "Theme " + name.
- **The Appearance page's "Theme" `SettingRow`** holds:
  - the `ThemeChip` flow;
  - a line for `skipped` ("2 themes in your themes folder couldn't be used." with an
    `AdvancedDetails` listing folder names and reasons in `UqLabel`s);
  - **New theme from this one…** (`settingId` `theme-new`);
  - **Import…** (`theme-import`) and **Export…** (`theme-export`), each its own `SettingRow`,
    so search highlights land.
  - **File dialogs.** Both are `FileDialog`s with `nameFilters: ["uQ themes (*.uqtheme)"]`. The
    export one has `defaultSuffix: "uqtheme"`, a "Include window gaps, corners and effects"
    checkbox in the row (bound into the call), and an `Export` that names the current theme.
  - **After an import,** an inline message (built from `UqLabel`s, as `ProfileMessage`) says
    "Added <name>." with **Use it now** and, when the file had `[look]` and the box was
    unchecked, the line "This theme also has window gaps, corners and effects. Import it again
    with the box ticked to use them."
  - **A conflict** opens a fixed-title dialog ("Import a theme") with uqd's message in a
    `UqLabel` and **Replace** / **Cancel**.
  - **Delete…** confirms in a fixed-title dialog ("Delete theme"). It names the theme in a
    `UqLabel` and, when it's active, adds "uQ will switch to its default theme first." The
    result line lists the profiles.
- **`ThemeEditor.qml`** (`Kirigami.Dialog`, title literal "New theme" or "Edit theme"):
  - a name field (`objectName: "themeNameField"`);
  - an optional author field;
  - Dark/Light tabs, each with seven rows, one per `derive::KEYS` label. Each row has a swatch
    button that opens `ColorDialog` and a hex `TextField` (`objectName: "<variant>-<key>"`);
  - a live preview card painted from `preview()` tokens: background, panel, text, muted text,
    accent with accent text, and the good, warning and bad dots;
  - the variant's `problems` as `UqLabel` lines (`objectName: "problems"`);
  - **Save** is disabled while `!ok` or the name has a problem.
  - It emits `saved(string draftJson)`, built from catalogue keys only (slice C's F29).

- [ ] **Step 1: Write the failing tests:**
  - **`tst_theme_chip.qml`:**
    - `test_name_is_plain_text` (`<b>x</b>` shows literally, found by `objectName`);
    - `test_menu_only_for_user_themes`;
    - `test_missing_line`;
    - `test_accessible_name`.
  - **`tst_theme_editor.qml`:**
    - `test_title_is_fixed` (a theme named `<b>x</b>` leaves the title "Edit theme");
    - `test_hex_field_updates_preview` (with a stub `preview` function from the fixture);
    - `test_save_disabled_on_problems`;
    - `test_draft_json_shape` (seven keys per variant plus `name` and `author`);
    - `test_short_hex_expanded_by_logic_not_qml` (the field passes `#abc` through unchanged;
      the logic expands it).
- [ ] **Step 2:** Run `scripts/lint-settings.sh` and `tests/shell/lint_settings.sh`. Expected:
  FAIL until the words and strings are right. The QML suite runs in CI (K4).
- [ ] **Step 3:** Implement. Check that no new `text:` on a stock control carries a theme name,
  author, profile name or folder name.
- [ ] **Step 4:** Run both lints. Expected: PASS. CI runs
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`
  and `cargo test -p uq-settings` with clippy.
- [ ] **Step 5:** Commit (K5 trailers): `feat(settings): new, edit, import, export and delete themes on the Appearance page`

### Task 10: Docs, changelog and the live-check note

**Files:**
- Modify:
  - `README.md`;
  - `CHANGELOG.md` (`## [Unreleased]` → `### Added`, additive);
  - `docs/superpowers/specs/2026-11-10-uq-settings-enhancements-design.md`:
    - §7.4 notes the full-id folder name, the 60 s pick-up of hand-copied folders and edit;
    - §10 adds the four calls and `ThemesChanged` to the D-Bus summary, plus
      `uqctl theme delete`.
  - `docs/developing.md` (a dev session refuses theme import and delete, G-F11).
- Create: `docs/notes/settings-slice-g-live-check.md`.

- [ ] **Step 1:** Update `README.md` with a "#### Your own themes" paragraph:
  - where user themes live, and the `user-` prefix;
  - the checks (size, names, ranges, contrast, both variants), and that files are data only;
  - New theme from this one and the seven colours;
  - what Export holds (and never: wallpaper, your font and cursor picks);
  - that Import saves a restore point and doesn't switch;
  - Delete and the switch to the default;
  - `uqctl theme list --long | import | export | delete`.

  Add the four calls and `ThemesChanged` to the Settings D-Bus paragraph.
- [ ] **Step 2:** Add a `CHANGELOG.md` entry, **Your own themes: make, import, export and
  delete.** It names the editor, the `.uqtheme` format, the checks, the `uqctl` commands, the
  shell chips showing names, and the new D-Bus calls.
- [ ] **Step 3:** Write `docs/notes/settings-slice-g-live-check.md` in the style of slice D's.
  Run it on the real desktop, and tick each item:
  1. **§1 criterion 7.**
     - Machine (or user) A: `uqctl theme export gruvbox ~/g.uqtheme --with-look` and, from the
       app, **Export…** of a user theme.
     - Copy both to machine B and **Import…** in the app. Each appears in the Appearance theme
       list and in quick settings at once, with no relog.
     - **Use it now** re-themes the bar, a new kitty, GTK and Qt apps.
  2. **Restore point.** History shows a "Before importing a theme" point. Restoring it removes
     nothing from the theme folder, and the state matches.
  3. **Look.** Import with the box ticked: gaps and corners change. One History row says so,
     and Ctrl+Z puts them back.
  4. **Editor.**
     - Open New theme from this one and set Dark text to the dark background: the editor says
       "hard to read" and Save is greyed.
     - Fix it and save "Ocean": `~/.local/share/uq/themes/user-ocean/{dark,light}.toml` exist,
       and Ocean is in the list.
     - Edit Ocean while it's active: the desktop changes live.
  5. **Hostile files.** Import each file from the Task 11 corpus folder
     (`crates/uq-core/tests/fixtures/uqtheme/bad/`). Each is refused with one plain sentence,
     and no folder appears. A FIFO named `x.uqtheme` doesn't hang the dialog.
  6. **Links.**
     - `ln -s /etc ~/.local/share/uq/themes/user-etc`: nothing new in the list, and Delete
       isn't offered for it.
     - Replace the root with a symlink: imports say the folder is a link.
  7. **By hand.** Copy a good theme folder in with `cp -r`. It appears within 60 s, or at once
     when the Appearance page is opened.
  8. **Delete.**
     - Delete the active user theme: uQ switches to the default first, and History shows the
       switch.
     - A profile that used it is named. Applying that profile gives the plain unknown-theme
       message.
  9. **Gone while active.** `rm -r` the active theme's folder. The list says its files were
     removed, other settings still save, and after `systemctl --user restart uqd` the default
     theme is in use.
  10. **Criterion 8.** `stat ~/.config/hypr/uq/user.lua` is unchanged by every step.
  11. **Dev session.** In `uq-dev session`, Import and Delete say "Not available in a test
      session".
  12. **VM.** `tests/vm/run.sh --only 'T[0-9]'` passes (Task 11's check).
- [ ] **Step 4:** Run `scripts/lint-settings.sh` (README and notes aren't linted, but the
  strings quoted from the app must match the app).
- [ ] **Step 5:** Commit (K5 trailers): `docs: settings slice G — your own themes`

### Task 11: Cross-cutting tests, the VM check and the whole gate

**Files:**
- Create:
  - `crates/uq-core/tests/fixtures/uqtheme/good/*.uqtheme`: one per system theme, generated by
    the test below with `UQ_BLESS=1` and checked in;
  - `crates/uq-core/tests/fixtures/uqtheme/bad/*.uqtheme`, the hostile corpus (one file per
    Review Focus 1–3 case, each with a one-line comment saying what it tests; the FIFO and the
    65 537-byte file are made at test time, not checked in);
  - `crates/uq-core/tests/uqtheme_corpus.rs`;
  - `crates/uqctl/tests/theme_cli.rs`;
  - `tests/vm/checks/60-themes.sh`.

**Interfaces:** none new. These tests consume Tasks 1–9.

- [ ] **Step 1: Write the tests:**
  - **`uqtheme_corpus.rs`:**
    - `good_fixtures_parse_and_match_system_themes` (with `UQ_BLESS=1`, it rewrites them);
    - `bad_fixtures_are_all_refused_plainly`. Every file is `Err`, and the message has no
      control characters and none of the Text-section jargon. The test also checks that the
      folder holds at least one file per Review Focus 1–3 case, by name prefix.
  - **`theme_cli.rs`** (private bus, a real uqd on `Paths::under`, the built `uqctl`):
    - `export_import_on_a_second_home` (Review Focus 6): export every system theme from home A,
      import into a fresh home B, `list --long` shows all seven as yours, and B's files equal
      `install_files` of A's export;
    - `bad_corpus_through_cli_changes_nothing`: each bad file exits non-zero with one line on
      stderr; B's theme folder and restore points are unchanged.
  - **`tests/vm/checks/60-themes.sh`.** It registers checks `T1`–`T3` with `check`, as
    `30-quick.sh` does:
    - **T1:** `uqctl theme export graphite /tmp/g.uqtheme`, `uqctl theme import /tmp/g.uqtheme`,
      then `uqctl theme set user-graphite dark`. The `ThemeChanged` tokens path's `tokens.json`
      `bg` matches.
    - **T2:** open quick settings, and a chip named "Graphite" appears twice (system and yours).
      This is a screenshot note, not pixel-exact.
    - **T3:** `uqctl theme delete user-graphite` while it's active prints the switch line, and
      `uqctl theme get` is the default.
- [ ] **Step 2:** Run `cargo test -j 2 -p uq-core --test uqtheme_corpus -- --test-threads=2` and
  `cargo test -j 2 -p uqctl --test theme_cli -- --test-threads=2`. Expected: PASS (Tasks 1–9
  are in), or FAIL with a real finding to fix in the task it belongs to before going on.
- [ ] **Step 3:** Run the full gate, exactly as CI does:
  - `cargo fmt --all --check`;
  - `cargo clippy -j 2 --workspace --all-targets --all-features --locked -- -D warnings`;
  - `cargo test -j 2 --workspace --locked -- --include-ignored --test-threads=2`;
  - `scripts/lint-shell.sh` and `scripts/lint-settings.sh`;
  - `tests/shell/*.sh` (`pam_stacks.sh`'s flake handled as in Global Constraints);
  - both `qmltestrunner` suites.

  In the build container (K4), run everything without Qt (`--exclude uq-settings`) and without
  `--include-ignored`, with `CARGO_INCREMENTAL=0` and `-j 2`. Run the page logic and registry
  tests through `<t7check>` (`pages::themes`, `pages::appearance`, `pages::profiles`,
  `registry`). Leave these to CI: `uq-settings`, the QML suites, the Quickshell shell tests, the
  `--ignored` Hyprland verify, and `tests/vm/run.sh`. Expected: all PASS.
- [ ] **Step 4:** On the real desktop (or the VM), run `tests/vm/run.sh --only 'T[0-9]'` and
  the live-check note from Task 10. Record the results in the note.
- [ ] **Step 5:** Write the PR body's "Spec deviations" section from the list near the top of
  this plan. Commit (K5 trailers): `test: theme import corpus, a two-home round trip and the VM theme check`

## Notes and open questions

1. **A symlinked themes root (R3).** People who keep dotfiles in git often symlink
   `~/.local/share/<app>`. v1 treats a linked root as "no user themes" and refuses imports.
   Should uqd follow a root link that the user owns and that points inside `$HOME`?
2. **Terminal colours (R11)** are copied from the base theme, so a light theme made from a dark
   one keeps dark-tuned ANSI colours. A later version could derive or edit them.
3. **No `.uqtheme` MIME type (R15).** A double-click in a file manager won't open uQ Settings.
   Adding `packaging/uq-theme.xml` and a `MimeType=` line is a small follow-up, if wanted.
4. **History shows ids for user themes.** A theme switch reads "Theme: gruvbox → user-ocean",
   because `history::summarize` has no registry. It's readable, but not the name. Fixing it
   means passing names into the summary; that's left for later.
5. **Commit trailer.** This plan uses `Claude Sonnet 5.5` as instructed; slices C and D used
   `Claude Opus 5.5`.
