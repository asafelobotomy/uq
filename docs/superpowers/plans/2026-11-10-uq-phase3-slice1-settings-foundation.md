# uQ Phase 3 slice 1: Settings app foundation, Appearance and Wallpaper — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `uq-settings` opens as a single-instance Kirigami window with a category sidebar, global
search and a plugin-page loader. Its Appearance page changes the theme, variant, schedule, fonts,
cursor, icons and Kvantum live. Its Wallpaper page sets a per-monitor wallpaper that `uq-shell`
draws on a background layer.

**Architecture:**
- **uq-core** gains two `Settings` domains, `appearance` and `wallpaper`, validated as whole
  sections in `uq_core::settings::{appearance, wallpaper}`. It also gains
  `Theme::with_appearance`, a Kvantum render target, and `uq_core::client`, a typed zbus proxy.
- **uqd** gains `GetSection`/`SetSection` and the `SectionChanged` signal.
  - Appearance commits like a theme change: render, verify, reload, gsettings and cursor.
  - Wallpaper takes the light path: save and emit.
- **The shell** draws `uq-wallpaper` layers from the mirrored settings
  (`Settings.values.wallpaper`). The pure logic lives in `shell/logic/wallpaper.js`.
- **The app** (`apps/uq-settings`, cxx-qt 0.10 + Kirigami 6) keeps all state in Rust QObjects that
  expose JSON-string properties, following `bindle-gui`'s backend pattern. The QML stays thin.
  - A zbus `org.uq.Settings1` service forwards `ShowPage` to the Qt thread.
  - Search and plugin manifests are plain Rust and unit-tested.

**Tech Stack:** Rust edition 2024, zbus 5, cxx-qt / cxx-qt-lib / cxx-qt-build 0.10, Qt 6, Kirigami
6 (Arch `kirigami` 6.30), Quickshell 0.3, minijinja 2.

**Spec:** `docs/superpowers/specs/2026-11-08-uq-phase3-settings-design.md`. The relevant parts
are §1, §2.1–2.3 (the `appearance` and `wallpaper` rows, wallpaper, Kvantum), §2.5 (loader and
error card only), §3, §4, §5, §6, §7 and §8 slice 1.

## Global Constraints

- **Plain text everywhere (§2.1, §3):** all text in `apps/uq-settings/qml` goes through
  `UqLabel` (a Kirigami `Controls.Label` with `textFormat: Text.PlainText`).
  `scripts/lint-settings.sh` forbids bare `Text`, `Label`, `TextEdit` and `TextInput` there, the
  same way `scripts/lint-shell.sh` does for the shell.
- **Single instance (§2.1):**
  - The session bus name is `org.uq.Settings1` at object path `/org/uq/Settings1`, with the
    method `ShowPage(s page_id, s setting_id)`. An empty `page_id` means "just raise".
  - `uqctl settings [<page>] [<setting>]` calls it. If no one owns the name, it starts
    `uq-settings --page <p> --setting <s>` detached.
- **Pages in this slice:** `appearance` and `wallpaper`. The sidebar lists every §2.2 category.
  Categories with no page yet are hidden, except Apps, which shows when plugin pages exist.
- **Search (§2.2):** fuzzy over labels and keywords. A result opens the page and highlights the
  row for 2 s. Setting ids are the `ShowPage` ids.
- **Section D-Bus API (§2.3):**
  - `GetSection(s domain) → s json` and `SetSection(s domain, s json)`, where `SetSection`
    replaces the whole domain.
  - Errors: `InvalidSetting` for an unknown domain or invalid content, and `ConfigInvalid` when
    Hyprland rejects the config.
  - The signal is `SectionChanged(s domain, s json)`.
  - The domains are `appearance` and `wallpaper`.
- **`appearance` (§2.3):**
  - Fields: `ui_font`, `mono_font`, `font_size`, `cursor_theme`, `cursor_size`, `icon_theme`
    (each `null` = use the theme), plus `kvantum: bool` (default `true`).
  - Limits: font names 1–100 chars; theme names match `^[A-Za-z0-9._+ -]{1,64}$` and are not
    `.` or `..`; `font_size` 6–32; `cursor_size` 16–96. No control characters anywhere.
- **`wallpaper` (§2.3):**
  - The domain is `{ outputs: { "<name>" | "*": Entry } }`, where `"*"` is "every display
    without its own entry". At most 16 keys. Output names match `^[A-Za-z0-9._-]{1,32}$`.
  - `Entry` is `{ image, fit, folder, interval, color }`:
    - `image` and `folder`: absolute local path or null; at most 4096 bytes, no `..`
      component, no control characters, not starting with `~`;
    - `fit`: `fill` | `fit` | `center` | `tile`, default `fill`;
    - `interval`: 10–86400 s, default 300;
    - `color`: `#rrggbb` or null, where null means the theme's `bg`.
- **Wallpaper drawing (§2.3):**
  - One `PanelWindow` per screen with `WlrLayer.Background` and namespace `uq-wallpaper`.
  - Slideshow: `jpg`, `jpeg`, `png` and `webp` in name order (`jxl` and `avif` only if Qt lists
    the format).
  - A 300 ms crossfade, skipped when `reduceMotion` (there's no token yet, so it's `false`, as
    in `shell/idle/Dim.qml`).
  - A missing or unreadable image shows the fallback colour.
- **Kvantum (§2.3):**
  - Rendered to `~/.local/state/uq/theme/Kvantum/uq/uq.kvconfig` with `[%General]
    inherits=<base>`, where the base is the first of `KvGnome`, `KvAdapta`, `KvArc`, `KvFlat`
    found under `/usr/share/Kvantum/`.
  - With no base, or with `appearance.kvantum = false`, the file is not rendered.
  - `uqctl setup --apps` creates `~/.config/Kvantum/kvantum.kvconfig` (`[General]\ntheme=uq\n`)
    and the symlink `~/.config/Kvantum/uq → <theme_dir>/Kvantum/uq`, each only if missing.
  - New `qt6ct.conf` files get `style=kvantum` when a base exists, and `style=Fusion` otherwise.
- **Error handling (§4):**
  - uqd absent: uQ pages show "uQ's settings service isn't running" and their controls are
    disabled.
  - A rejected write puts the control back to its last good value and shows the message as plain
    text.
  - A broken plugin page shows the "This page couldn't load" card with the error text.
- **Plugin pages (§2.5):** read only from `/usr/share/uq/settings/pages/<id>/page.toml`. Tests
  inject the directory.
  - The `page.toml` keys are `id`, `name`, `icon`, `category = "apps"`, `qml`, `dbus_name`,
    `keywords` and `[[setting]] {id, label, keywords}`.
  - `id` matches `^[a-z0-9-]{1,40}$`; `qml` is a bare file name ending `.qml`. A page loads only
    when opened.
  - The `dbus` context helper and the SDK are slice 6. Here, a page gets `Theme` and `UqLabel`
    only.
- **Nothing runs as root** in this slice.
- **Process and test rules (as in Phase 2):**
  - Never run the bridge, uqd, the shell or `uq-settings` against the real session bus or
    `hyprctl`. Use private `dbus-daemon` buses, with socket paths under 108 bytes.
  - QML tests: offscreen `/usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml`. App QML
    tests: `-import apps/uq-settings/qml -input tests/settings-qml`.
  - Don't run `tests/shell/bar_loads.sh` or `scripts/smoke-shell.sh` locally; CI runs them.
  - `cargo fmt`, `clippy -D warnings` and `cargo test --workspace` must be clean.
  - Keep files at or under about 400 lines; `settings.rs` (537 lines) is split in Task 1.

## Review Focus

1. **A `state.toml` from 0.2.0 has no `[appearance]` or `[wallpaper]`, and TOML has no null.**
   It must load with every override unset. A saved `null` must round-trip as "absent", not as `""`.
   → `old_state_loads_with_default_sections` and `null_overrides_round_trip` (Task 1).
2. **A wallpaper path that is hostile or gone.** `../`, `~/x`, a relative path, a 5 KB path or a
   newline in the path is rejected by uqd. A valid path whose file was deleted shows the fallback
   colour in the shell and the error in the page; it never leaves a blank or black layer.
   → `rejects_bad_paths` (Task 1), `test_fallback_when_unreadable` (Task 5),
   `missing_image_is_reported` (Task 10).
3. **An appearance override Hyprland or GTK can't use.** For example, an installed-looking but
   missing cursor theme. The render and verify still pass (cursor and icon names are free text to
   Hyprland), and gsettings failures stay a warning, not a failed write.
   → `appearance_commit_survives_gsettings_failure` (Task 3).
4. **A second `uq-settings` while one is open, or the first one crashing.** The second asks the
   first to show the page and exits 0. If the name's owner is gone, the second becomes the
   instance. → `second_instance_forwards_and_exits` (Task 7).
5. **A plugin directory with junk:** a folder without `page.toml`, a `page.toml` with an unknown
   key, a duplicate id, `qml = "../x.qml"`, or a QML file with a syntax error. Bad manifests are
   skipped with a logged reason. The core pages are still listed. A QML error shows the card, not
   a crash. → `scan_skips_bad_manifests` (Task 8), `tst_plugin_error_card` (Task 8).

---

### Task 1: `appearance` and `wallpaper` domains in uq-core

**Files:**
- Move: `crates/uq-core/src/settings.rs` → `crates/uq-core/src/settings/mod.rs` (unchanged code)
- Create: `crates/uq-core/src/settings/appearance.rs`, `crates/uq-core/src/settings/wallpaper.rs`
- Modify: `crates/uq-core/src/settings/mod.rs` (fields, section API), `crates/uq-core/src/state.rs` (tests only)

**Interfaces:**
- Produces:
  - `pub struct Appearance { ui_font: Option<String>, mono_font: Option<String>, font_size: Option<u8>, cursor_theme: Option<String>, cursor_size: Option<u8>, icon_theme: Option<String>, kvantum: bool }`,
    with `#[serde(default, skip_serializing_if = "Option::is_none")]` on each `Option` and
    `Default` `kvantum = true`.
  - `pub enum Fit { Fill, Fit, Center, Tile }` (lowercase serde, default `Fill`).
  - `pub struct WallpaperEntry { image: Option<PathBuf>, fit: Fit, folder: Option<PathBuf>, interval: u32, color: Option<String> }`.
  - `pub struct Wallpaper { outputs: BTreeMap<String, WallpaperEntry> }`.
  - `Settings` gains `pub appearance: Appearance` and `pub wallpaper: Wallpaper`, stored as
    `[appearance]` and `[wallpaper.outputs."<name>"]` in `state.toml`.
  - `pub const SECTION_DOMAINS: [&str; 2] = ["appearance", "wallpaper"];`
  - `impl Settings { pub fn section(&self, domain: &str) -> Result<serde_json::Value, SettingError>; pub fn set_section(&mut self, domain: &str, value: serde_json::Value) -> Result<(), SettingError>; }`.
    `set_section` deserialises with `deny_unknown_fields`, then calls `Appearance::validate` or
    `Wallpaper::validate`, and replaces the field only if both pass.
  - `pub fn renders_section(domain: &str) -> bool`, true for `appearance` only.

- [ ] **Step 1: Move the file and write the failing tests.** The tests are
  `appearance::tests::{defaults_are_unset_and_kvantum_on, rejects_bad_names, font_and_cursor_ranges}`,
  `wallpaper::tests::{rejects_bad_paths, rejects_bad_outputs_and_too_many, interval_and_color}`,
  and `tests::{section_round_trip, unknown_domain_rejected, invalid_section_leaves_settings_unchanged}`
  in `mod.rs`, plus `state::tests::{old_state_loads_with_default_sections, null_overrides_round_trip}`.
  Key assertions:

```rust
assert!(s.set_section("appearance", json!({"cursor_theme": ".."})).is_err());
assert!(s.set_section("appearance", json!({"font_size": 5})).is_err());
assert!(s.set_section("appearance", json!({"ui_font": "Inter\nX"})).is_err());
for bad in ["../a.png", "/a/../b.png", "~/a.png", "a.png", "/a\n.png"] {
    assert!(s.set_section("wallpaper", json!({"outputs": {"*": {"image": bad}}})).is_err(), "{bad}");
}
assert!(s.set_section("wallpaper", json!({"outputs": {"DP 1": {}}})).is_err());
assert!(s.set_section("wallpaper", json!({"outputs": {"*": {"interval": 9}}})).is_err());
assert!(s.set_section("wallpaper", json!({"outputs": {"*": {"color": "#12345"}}})).is_err());
assert!(s.set_section("look", json!({})).is_err());
// state.toml without the new tables
let st = State::parse("theme = \"graphite\"\nvariant = \"dark\"\n").unwrap();
assert_eq!(st.settings.appearance, Appearance::default());
```

- [ ] **Step 2: Run them.** `cargo test -p uq-core settings state`. Expected: FAIL (the types
  don't exist).
- [ ] **Step 3: Implement** the types, `validate()` on each, `section`, `set_section` and
  `renders_section`. The path check is `Path::is_absolute`, no `Component::ParentDir`,
  `len() <= 4096`, and no `char::is_control`.
- [ ] **Step 4: Run them.** `cargo test -p uq-core`. Expected: PASS (all old tests too).
- [ ] **Step 5: Commit.** `feat(core): appearance and wallpaper settings sections`

### Task 2: Appearance overrides in the theme render, and the Kvantum target

**Files:**
- Modify: `crates/uq-core/src/theme.rs`, `crates/uq-core/src/render.rs`, `crates/uq-core/src/lua.rs`
- Create: `templates/kvantum.kvconfig.j2`

**Interfaces:**
- Consumes: `Appearance` (Task 1).
- Produces:
  - `impl Theme { pub fn with_appearance(&self, a: &Appearance) -> Theme }`: each `Some` replaces
    the matching `fonts.ui`, `fonts.mono`, `fonts.size`, `icons.cursor_theme`,
    `icons.cursor_size` or `icons.theme`.
  - `pub const KVANTUM_BASES: [&str; 4] = ["KvGnome", "KvAdapta", "KvArc", "KvFlat"];` and
    `pub fn kvantum_base(dir: &Path) -> Option<&'static str>`, which returns the first base whose
    `dir/<name>/<name>.kvconfig` exists.
  - `pub fn render(theme: &Theme, templates_dir: &Path, kvantum_base: Option<&str>) -> Result<RenderedFiles, RenderError>`.
    With `Some(base)` it also renders `Kvantum/uq/uq.kvconfig` from `kvantum.kvconfig.j2`, with
    `base` in the context. Every existing caller passes `None` until Task 3.
  - `emit_generated` emits `hl.env("XCURSOR_THEME", …)`, `hl.env("XCURSOR_SIZE", …)` and
    `hl.env("HYPRCURSOR_SIZE", …)` from `theme.icons`, through `lua_string`. They come after
    `base.lua`'s fixed size, so they win.

- [ ] **Step 1: Write the failing tests.**
  - `theme::tests::with_appearance_overrides_only_set_fields`
  - `render::tests::{kvantum_rendered_only_with_base, kvantum_base_order}`: the second makes
    temp dirs holding only `KvArc` and `KvFlat` and expects `KvArc`.
  - `lua::tests::generated_sets_cursor_env`: asserts the exact line
    `hl.env("XCURSOR_THEME", "Bibata-Modern-Classic")` for an override.
  - The golden test for `generated.lua` is updated to include the three env lines.
- [ ] **Step 2: Run.** `cargo test -p uq-core`. Expected: FAIL.
- [ ] **Step 3: Implement.** The template is `[%General]\ninherits={{ base }}\n` plus a
  `[GeneralColors]` section mapping these tokens (Kvantum key ← token):
  - `window.color` ← `bg`, `base.color` ← `surface`, `alt.base.color` ← `surface_alt`;
  - `button.color` ← `surface_alt`, `light.color` ← `overlay`, `mid.color` ← `border`;
  - `dark.color` ← `bg`, `highlight.color` ← `accent`, `inactive.highlight.color` ← `accent`;
  - `text.color` ← `text`, `window.text.color` ← `text`, `button.text.color` ← `text`;
  - `disabled.text.color` ← `text_muted`, `tooltip.text.color` ← `text`;
  - `highlight.text.color` ← `accent_text`, `link.color` ← `accent`,
    `link.visited.color` ← `accent`, `progress.indicator.text.color` ← `accent_text`.
- [ ] **Step 4: Run.** `cargo test -p uq-core` and `cargo test -p uqd -- --include-ignored`. The
  second includes the `--verify-config` tests and needs Hyprland. Where Hyprland isn't installed
  locally, CI runs it. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(core): appearance overrides, cursor env and the Kvantum theme`

### Task 3: uqd `GetSection`, `SetSection`, `SectionChanged`

**Files:**
- Modify: `crates/uqd/src/service.rs`, `crates/uqd/src/dbus.rs`, `crates/uqd/src/compositor.rs`, `crates/uqd/src/system.rs`
- Create: `crates/uqd/src/sections.rs` (holds the new `Desktop` methods, so `service.rs` stops growing)

**Interfaces:**
- Consumes: Tasks 1–2.
- Produces:
  - `Event::SectionChanged { domain: String, json: String }`.
  - `Desktop::get_section(&self, domain) -> Result<String, DesktopError>` and
    `Desktop::set_section(&mut self, domain, json: &str) -> Result<(), DesktopError>`.
    - `appearance` → `commit(next, Change::Theme)`.
    - `wallpaper` → save, then emit `SectionChanged` and `SettingsChanged`.
  - `commit` renders, emits and applies gsettings for `theme.with_appearance(&next.settings.appearance)`,
    and passes `kvantum_base(Path::new("/usr/share/Kvantum"))` only when `appearance.kvantum` is
    set.
    - That directory is a new `Paths`-like field `kvantum_dir` on `Desktop`, injectable in tests.
  - On `Change::Theme`, after a live reload, it calls the new
    `Compositor::set_cursor(&self, theme: &str, size: u8)` (socket request
    `setcursor <theme> <size>`; the fake records it).
  - D-Bus: `GetSection`, `SetSection` and `#[zbus(signal)] section_changed(domain, json)`. The
    signal pump that turns `Event`s into signals forwards the new event.
  - Errors map as in §Global Constraints.

- [ ] **Step 1: Write the failing tests** in `sections.rs`, using `FakeCompositor` and
  `FakeSystem`:
  - `set_appearance_renders_overrides_and_sets_cursor`: `fonts.ui` shows in `tokens.json`, and
    the fake recorded `setcursor`;
  - `appearance_commit_survives_gsettings_failure`;
  - `kvantum_off_removes_the_kvconfig`;
  - `set_wallpaper_saves_and_emits_without_render`: `generated.lua`'s mtime is unchanged, and
    `SectionChanged` plus `SettingsChanged` were received;
  - `invalid_section_is_rejected_and_state_unchanged`;
  - `rejected_config_keeps_previous_appearance`: the fake verify rejects.

  In `dbus.rs` tests, using the existing private-bus pattern: `section_round_trip_over_dbus`.
- [ ] **Step 2: Run.** `cargo test -p uqd`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `cargo test -p uqd`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(uqd): GetSection/SetSection for appearance and wallpaper`

### Task 4: `uq_core::client`, bridge passthrough, and `uqctl settings`

**Files:**
- Create: `crates/uq-core/src/client.rs`
- Modify: `crates/uq-core/Cargo.toml` (adds `zbus = { version = "5", default-features = false, features = ["tokio"] }`), `crates/uq-core/src/lib.rs`, `crates/uqctl/src/bridge/uqd.rs`, `crates/uqctl/src/main.rs`
- Create: `crates/uqctl/src/settings_app.rs`

**Interfaces:**
- Produces:
  - `#[zbus::proxy(interface = "org.uq.Desktop1", default_service = "org.uq.Desktop1", default_path = "/org/uq/Desktop1")] pub trait Desktop`.
    It covers `get_theme`, `set_theme`, `list_themes`, `get_schedule`, `set_schedule`,
    `get_settings`, `get_section`, `set_section`, and the `section_changed`, `theme_changed` and
    `settings_changed` signals. It generates `DesktopProxy` (async).
  - `#[zbus::proxy(interface = "org.uq.Settings1", default_service = "org.uq.Settings1", default_path = "/org/uq/Settings1")] pub trait SettingsApp { fn show_page(&self, page_id: &str, setting_id: &str) -> zbus::Result<()>; }`.
  - The bridge forwards `SectionChanged` as event `"SectionChanged"` with data
    `{"domain": d, "value": <parsed json>}`.
  - `uqctl settings [PAGE] [SETTING]`:
    - calls `SettingsAppProxy::show_page`;
    - on `org.freedesktop.DBus.Error.ServiceUnknown` (or `NameHasNoOwner`) spawns
      `uq-settings --page PAGE --setting SETTING` with `setsid`-style detach (stdin, stdout and
      stderr to null);
    - exits 0.
- Existing `uqctl` subcommands keep their `zbus::Proxy` calls. Migrating them is not part of
  this slice.

- [ ] **Step 1: Write the failing tests:**
  - `bridge::uqd::tests::section_changed_is_forwarded`, in the existing private-bus forward tests;
  - `settings_app::tests::show_page_on_running_instance`, with a fake `org.uq.Settings1` on a
    private bus that records the call;
  - `settings_app::tests::spawns_when_absent`, with an injected spawner closure that records
    argv `["uq-settings", "--page", "wallpaper", "--setting", ""]`.
- [ ] **Step 2: Run.** `cargo test -p uqctl`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run.** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat: typed uqd client, SectionChanged in the bridge, uqctl settings`

### Task 5: Shell wallpaper layer

**Files:**
- Create: `shell/logic/wallpaper.js`, `shell/wallpaper/Wallpaper.qml`, `shell/wallpaper/qmldir`, `tests/qml/tst_wallpaper.qml`
- Modify: `shell/shell.qml` (a second `Variants` over `Quickshell.screens` with `delegate: Wallpaper {}`), `shell/core/Settings.qml` (refresh `values` on a `SectionChanged` event too)

**Interfaces:**
- Consumes: `Settings.values.wallpaper` (Task 1's shape), and `Theme.colors.bg` from
  `shell/core/Theme.qml`.
- Produces (`wallpaper.js`, pure):
  - `entryFor(wallpaper, outputName)` returns the entry for that name, else `"*"`'s, else
    `null`;
  - `slides(fileNames, readableFormats)` returns the names filtered by extension
    (case-insensitive), sorted by name with `localeCompare` and `{numeric: true}`;
  - `nextIndex(i, n)` and `nextReadable(i, n, errored)`;
  - `fillMode(fit)` maps `fill`, `fit`, `center` and `tile` to `Image.PreserveAspectCrop`,
    `PreserveAspectFit`, `Pad` and `Tile`;
  - `fallbackColor(entry, themeBg)`.
- `Wallpaper.qml`: `PanelWindow { WlrLayershell.layer: WlrLayer.Background; WlrLayershell.namespace: "uq-wallpaper"; anchors all; exclusionZone: -1 }`.
  - It holds two `Image`s for the crossfade (`asynchronous: true`, `cache: false`,
    `sourceSize` at the screen size) over a `Rectangle` in the fallback colour. An image whose
    `status === Image.Error` stays transparent, so the colour shows.
  - The slideshow lists the folder with `Qt.labs.folderlistmodel`'s `FolderListModel`
    (`showDirs: false`) and advances on a `Timer` every `interval` s.
  - `readableFormats` is `["jpg","jpeg","png","webp","jxl","avif"]`. QML can't ask Qt which
    plugins are installed, so a slide whose `Image.status` becomes `Error` (for example a `jxl`
    without the plugin) is skipped at once, without waiting for the interval. That meets "only if
    Qt can read them". If every slide errors, the fallback colour shows.

- [ ] **Step 1: Write the failing tests** in `tst_wallpaper.qml`: `test_entry_for_name_then_star`,
  `test_slides_filter_and_order` (`["b.PNG","a.jpg","c.txt","10.png","2.png"]` →
  `["2.png","10.png","a.jpg","b.PNG"]`), `test_fill_modes`, `test_fallback_when_unreadable` (a
  null entry or null color → `themeBg`), and `test_next_index_wraps`, and `test_skip_errored_slide` (`nextReadable(i, n, errored)` skips the indices in `errored`, and returns -1 when all have errored).
- [ ] **Step 2: Run.** `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import shell -input tests/qml/tst_wallpaper.qml`. Expected: FAIL.
- [ ] **Step 3: Implement** `wallpaper.js` and `Wallpaper.qml`, and wire it into `shell.qml`.
  Text isn't needed, so the lint is unaffected.
- [ ] **Step 4: Run** the QML tests and `scripts/lint-shell.sh`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(shell): per-monitor wallpaper layer`

### Task 6: Kvantum in `uqctl setup --apps`

**Files:**
- Modify: `crates/uq-core/src/stub.rs` (or a new `crates/uq-core/src/consumers.rs` if `stub.rs` would pass 400 lines), `crates/uqctl/src/setup.rs` (print the Kvantum lines)

**Interfaces:**
- Consumes: `kvantum_base` (Task 2).
- Produces:
  - `consumer_files(p: &Paths, kvantum: bool) -> Vec<(PathBuf, String)>`, where `kvantum` means
    a base exists. With `kvantum`, it adds `~/.config/Kvantum/kvantum.kvconfig` =
    `"[General]\ntheme=uq\n"`, and the `qt6ct` content uses `style=kvantum`.
  - `install_consumers` also creates the symlink `~/.config/Kvantum/uq` →
    `<theme_dir>/Kvantum/uq` when `kvantum` is set and the link path doesn't exist. An existing
    entry, of any type, is reported as `AlreadyPresent`.

- [ ] **Step 1: Write the failing tests:** `kvantum_files_only_with_base`,
  `kvantum_symlink_created_once_and_existing_kept`, and `qt6ct_style_follows_kvantum`.
- [ ] **Step 2: Run.** `cargo test -p uq-core stub`. Expected: FAIL.
- [ ] **Step 3: Implement.** `uqctl setup` passes
  `kvantum_base(Path::new("/usr/share/Kvantum")).is_some()`.
- [ ] **Step 4: Run.** `cargo test --workspace`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(setup): theme Qt apps through Kvantum when a base theme is installed`

### Task 7: `uq-settings` skeleton: single instance, window, sidebar

**Files:**
- Create: `apps/uq-settings/Cargo.toml`, `apps/uq-settings/build.rs`, and these sources:
  - `apps/uq-settings/src/main.rs`: argument parsing (`--page`, `--setting`, `--screenshots <dir>`
    hidden);
  - `apps/uq-settings/src/app.rs`: the cxx-qt `App` QObject (properties `page`, `highlight`,
    `uqd_up`, `pages_json`; invokable `showPage(page, setting)`);
  - `apps/uq-settings/src/instance.rs`: the bus name and `Settings1` interface;
  - `apps/uq-settings/src/pages.rs`: the core page registry.
- Create (QML): `apps/uq-settings/qml/Main.qml`, `qml/Sidebar.qml`, `qml/components/UqLabel.qml`,
  `qml/components/SettingRow.qml` (a row with an `id`, which highlights for 2 s on request).
- Create: `scripts/lint-settings.sh`, a copy of `lint-shell.sh`'s rules over `apps/uq-settings/qml`
  that allows only `qml/components/UqLabel.qml`. Factor the shared grep into
  `scripts/lib-qml-lint.sh` sourced by both, if that keeps each script short.
- Modify: root `Cargo.toml` (`members += "apps/uq-settings"`), `.github/workflows/ci.yml`
  (pacman adds `kirigami qqc2-desktop-style kvantum`; the lint step adds
  `scripts/lint-settings.sh`).

**Interfaces:**
- Produces:
  - `pub struct PageInfo { id: &'static str, name: &'static str, icon: &'static str, category: Category, settings: &'static [SearchEntry] }`.
  - `pub enum Category { Personal, Devices, Connections, Uq, System, Apps }`, in that order.
  - `pub struct SearchEntry { id: &'static str, label: &'static str, keywords: &'static [&'static str] }`.
  - `pub fn core_pages() -> &'static [PageInfo]`, holding `appearance` and `wallpaper`. Their
    setting ids are listed in Tasks 9 and 10.
  - `instance::claim(conn) -> Result<Claim, zbus::Error>`, where
    `enum Claim { Owner, Forwarded }`. Forwarded means another process owns the name and has
    been sent `ShowPage`. Use `RequestNameFlags::DoNotQueue`.
  - Calls on the zbus `Settings1` interface go to the Qt thread through cxx-qt's
    `CxxQtThread::queue`, which sets `page`/`highlight` and raises the window
    (`requestActivate`).
  - `App.uqd_up` watches `NameOwnerChanged` for `org.uq.Desktop1`.

- [ ] **Step 1: Write the failing tests:**
  - `instance::tests::second_instance_forwards_and_exits` and
    `instance::tests::stale_owner_gone_means_owner`, on a private bus: claim, then drop the
    first connection, then claim again → `Owner`;
  - `pages::tests::ids_unique_and_categories_ordered`;
  - `scripts/lint-settings.sh` is run against a temp dir containing a bare `Text {}`, and must
    exit 1 (add it as `tests/shell/lint_settings.sh`).
- [ ] **Step 2: Run.** `cargo test -p uq-settings` and `bash tests/shell/lint_settings.sh`.
  Expected: FAIL.
- [ ] **Step 3: Implement.** `Main.qml` is a `Kirigami.ApplicationWindow` with a
  `Kirigami.GlobalDrawer`-free layout: a `Sidebar` (a `ListView` grouped by category, an empty
  category hidden) and a `StackView` showing the current page. The `--screenshots` mode follows
  `bindle-gui`'s: visit each page, grab it to `<dir>/<page>.png`, then quit.
- [ ] **Step 4: Run** the tests, `cargo clippy -p uq-settings -- -D warnings`, and
  `scripts/lint-settings.sh`. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(settings): uq-settings window, single instance and sidebar`

### Task 8: Search and plugin pages

**Files:**
- Create: `apps/uq-settings/src/search.rs`, `apps/uq-settings/src/plugins.rs`,
  `apps/uq-settings/qml/Search.qml`, `apps/uq-settings/qml/PluginPage.qml`,
  `apps/uq-settings/qml/components/ErrorCard.qml`, `tests/settings-qml/tst_plugin_error_card.qml`,
  `tests/settings-qml/fixtures/broken/Page.qml` (a deliberate syntax error)
- Modify: `apps/uq-settings/src/app.rs` (`search(query) -> QString` json invokable; `pages_json` includes plugin pages)

**Interfaces:**
- Produces:
  - `pub fn score(query: &str, text: &str) -> Option<u32>`: a case-insensitive subsequence match.
    Bonuses go to a match at a word start and to consecutive characters. `None` when it isn't a
    subsequence.
  - `pub fn search(pages: &[PageEntry], query: &str) -> Vec<Hit>`, where
    `Hit { page_id, setting_id, label, page_name, score }`. Labels count over keywords, the
    results are sorted by score then label, and capped at 20.
  - `PageEntry` is the owned form shared by core and plugin pages.
  - `pub fn scan(dir: &Path) -> (Vec<PluginPage>, Vec<String /* skipped: reason */>)`, where
    `PluginPage { id, name, icon, qml_path: PathBuf, dbus_name, keywords, settings: Vec<OwnedSearchEntry> }`.
    The `page.toml` parse uses `deny_unknown_fields`. A plugin id that collides with a core id or
    an earlier plugin is skipped. Scan order is by directory name.
  - `PluginPage.qml` loads `qml_path` with a `Loader` (`asynchronous: false`) only when shown.
    On `Loader.Error` it shows `ErrorCard { title: "This page couldn't load"; detail: <errorString> }`.
    The page gets `Theme` and `UqLabel` through the Loader's context (`setSource` with
    properties `theme`).

- [ ] **Step 1: Write the failing tests:**
  - `search::tests::{subsequence_and_word_start_rank_higher, keywords_match_but_rank_below_labels, empty_query_returns_nothing, caps_at_20}`;
  - `plugins::tests::scan_skips_bad_manifests`, with temp dirs for each Review Focus #5 case
    plus one valid page;
  - `tst_plugin_error_card.qml`, which loads `PluginPage` with `fixtures/broken/Page.qml` and
    asserts the card's detail text is non-empty and the card is visible.
- [ ] **Step 2: Run.** `cargo test -p uq-settings` and
  `QT_QPA_PLATFORM=offscreen /usr/lib/qt6/bin/qmltestrunner -import apps/uq-settings/qml -input tests/settings-qml`.
  Expected: FAIL.
- [ ] **Step 3: Implement.** `Search.qml` is a `Kirigami.SearchField` in the header with a results
  popup. Choosing a result calls `App.showPage(page, setting)`. Add the qmltestrunner line for
  `tests/settings-qml` to CI.
- [ ] **Step 4: Run** both. Expected: PASS.
- [ ] **Step 5: Commit.** `feat(settings): global search and the plugin page loader`

### Task 9: Appearance page

**Files:**
- Create: `apps/uq-settings/src/pages/appearance.rs` (cxx-qt QObject `AppearanceModel`), `apps/uq-settings/src/pages/mod.rs`, `apps/uq-settings/src/system_lists.rs`, `apps/uq-settings/qml/pages/AppearancePage.qml`
- Modify: `apps/uq-settings/src/pages.rs` (Appearance search entries)

**Interfaces:**
- Consumes: `DesktopProxy` (Task 4).
- Produces:
  - `system_lists::cursor_themes(dirs: &[PathBuf]) -> Vec<String>`: directories that contain
    `cursors/`. `system_lists::icon_themes(dirs) -> Vec<String>`: directories with an
    `index.theme` and at least one subdirectory other than `cursors`. Both are de-duplicated and
    sorted. The real dirs are `/usr/share/icons`, `~/.local/share/icons` and `~/.icons`.
  - `AppearanceModel` properties are `state_json`, `error` and `busy`. `state_json` holds
    `{themes, theme, variant, schedule, appearance, cursorThemes, iconThemes}`. Invokables:
    `setTheme(id, variant)`, `setSchedule(json)`, and `setAppearance(field, json_value)`, which
    reads the current section, replaces one field and calls `SetSection`.
  - On error, `error` gets uqd's message and `state_json` is re-read, so the controls snap back.
  - It listens to `SectionChanged`, `ThemeChanged` and `SettingsChanged`.
  - Search ids: `theme`, `variant`, `schedule`, `ui-font`, `mono-font`, `font-size`,
    `cursor-theme`, `cursor-size`, `icon-theme`, `kvantum`.
  - Fonts are chosen in QML from `Qt.fontFamilies()`. Each override row has "Use theme" (sends
    `null`).

- [ ] **Step 1: Write the failing tests:**
  - `system_lists::tests::{cursor_and_icon_themes_from_dirs}`;
  - `pages::appearance::tests::{set_field_keeps_other_fields, error_restores_state}`. The model
    logic lives in a plain `AppearanceState` struct tested against a fake `org.uq.Desktop1` on a
    private bus, the same fake pattern as Task 4.
- [ ] **Step 2: Run.** `cargo test -p uq-settings`. Expected: FAIL.
- [ ] **Step 3: Implement** the model and the page: theme swatches from `ListThemes`, a variant
  switch, schedule mode (off, fixed times, sun), and the override rows. With uqd down, the page
  shows the §4 message and disables its controls.
- [ ] **Step 4: Run** the tests, `scripts/lint-settings.sh`, and the `--screenshots` run offscreen
  into `/tmp/uqs-shots`. Check `appearance.png` by eye.
- [ ] **Step 5: Commit.** `feat(settings): Appearance page`

### Task 10: Wallpaper page

**Files:**
- Create: `apps/uq-settings/src/pages/wallpaper.rs` (`WallpaperModel`), `apps/uq-settings/qml/pages/WallpaperPage.qml`
- Modify: `apps/uq-settings/src/pages.rs` (Wallpaper search entries)

**Interfaces:**
- Consumes: `DesktopProxy`, and the `Wallpaper` types (Task 1).
- Produces:
  - `WallpaperModel` properties: `state_json`, holding `{outputs: {...}, problems: {"<name>": "<message>"}}`, and
    `error`. Invokable: `setEntry(output, json_entry)`, which replaces one output's entry
    (`"*"` = all displays) and calls `SetSection`; `clearEntry(output)`.
  - `fn problems(w: &Wallpaper) -> BTreeMap<String, String>` reports an image that doesn't
    exist or isn't readable ("The image can't be read: <path>"), and a folder with no readable
    images.
  - The output list comes from `Qt.application.screens` names in QML, plus "All displays"
    (`"*"`).
  - Image and folder pickers are `QtQuick.Dialogs` `FileDialog` (name filters for the
    extensions) and `FolderDialog`; `fit` is a combo; the interval is a spin box in minutes and
    seconds; the colour field accepts `#rrggbb`, with "Use theme".
  - Search ids: `wallpaper-image`, `wallpaper-fit`, `wallpaper-slideshow`, `wallpaper-interval`,
    `wallpaper-color`.

- [ ] **Step 1: Write the failing tests:** `pages::wallpaper::tests::{missing_image_is_reported, set_entry_keeps_other_outputs, clear_entry_removes_only_that_output}`.
- [ ] **Step 2: Run.** `cargo test -p uq-settings`. Expected: FAIL.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run** the tests, the lint and `--screenshots`. Check `wallpaper.png`.
- [ ] **Step 5: Commit.** `feat(settings): Wallpaper page`

### Task 11: Packaging, the shell's Settings entry, docs

**Files:**
- Modify: `packaging/uq/PKGBUILD`:
  - `depends` adds `kirigami`, `qqc2-desktop-style`, `qt6-declarative` and `kvantum`, as §7
    lists. The other §7 entries arrive with the pages that use them (slices 5–6).
  - build and install `uq-settings`;
  - install `packaging/org.uq.Settings.desktop` and
    `packaging/dbus/org.uq.Settings1.service` (`Exec=/usr/bin/uq-settings`).
- Modify: `shell/quick/` (a "Settings…" button at the bottom of quick settings runs
  `uqctl settings` with a fixed argv, then closes the panel), `README.md` (a short Settings
  section), `CHANGELOG.md` (Unreleased: Settings app, wallpaper, Kvantum).
- Create: `docs/notes/settings-slice1-live-check.md`. The manual checklist covers: the theme,
  variant and font change live in the app, the bar and a GTK app; the cursor changes in a new
  kitty window; a wallpaper per monitor, the slideshow and a deleted image (fallback colour and
  the page's error); a second `uqctl settings wallpaper` raises the same window; a broken plugin
  page shows the card.

- [ ] **Step 1: Write the failing test:** extend `tests/shell/` with
  `settings_entry_argv.sh`. It asserts `quick` invokes exactly `["uqctl","settings"]` (a grep of
  the QML for the argv array, in the style of the existing shell tests).
- [ ] **Step 2: Run.** `bash tests/shell/settings_entry_argv.sh`. Expected: FAIL.
- [ ] **Step 3: Implement**, then check `bash -n packaging/uq/PKGBUILD` and, if installed,
  `desktop-file-validate packaging/org.uq.Settings.desktop`.
- [ ] **Step 4: Run** the whole gate: `cargo fmt --all --check`, `cargo clippy --workspace
  --all-targets -- -D warnings`, `cargo test --workspace`, `scripts/lint-shell.sh`,
  `scripts/lint-settings.sh`, both qmltestrunner suites, and `tests/shell/*.sh` except
  `bar_loads.sh`. Expected: all PASS.
- [ ] **Step 5: Commit.** `build: package uq-settings; quick settings opens it`
