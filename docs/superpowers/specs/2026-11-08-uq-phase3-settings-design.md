# uQ Phase 3: Settings design

Date: 2026-11-08 · Status: approved in brainstorming, awaiting spec review
Parent spec: `2026-11-08-uq-desktop-design.md` (§2.4 apps and extension points, §7 phase 3).
Builds on: `2026-11-08-uq-phase2-shell-design.md`. Settings needs Phase 2 complete, including the
slice 5 polkit agent.

## 1. Intent

`uq-settings` is uQ's central settings app. It is a normal desktop window for everything uQ owns
(appearance, look, layouts, displays, input, keybinds, shell) and for the system itself (sound,
network, Bluetooth, power, users, date and time, region, default apps, about). Other repos' apps
can add pages to it.

**What the user decided**

- **Pages:** Appearance and layouts; displays and input; shell and keybinds; system basics.
- **Stack:** Rust + cxx-qt + Kirigami, following the stack convention.
- **Hyprland settings** (monitors, input, keybinds, look) are owned by uqd, stored in
  `state.toml`, rendered into `generated.lua` and verified before they apply. `user.lua` still
  overrides them.
- **Organisation:** a category sidebar with a global search that jumps to any setting.
- **Wallpaper** is drawn by `uq-shell`, as a background layer on each monitor.
- **System pages are fully native.** This includes VPN and 802.1x editing, per-app audio routing,
  and Bluetooth pairing.
- **Displays** apply immediately and revert on their own after 15 s unless confirmed.
- **Kvantum** is themed through a uQ kvconfig that inherits an SVG theme the `kvantum` package
  already installs.
- **Keybinds:** a full editor covering every uQ default and custom binds, with conflict detection.
- **Plugin pages** load only from system-installed packages.
- **Users:** full user management.
- **Build order:** Phase 2 slices 2–5 come first, then Settings.

**Assumptions**

- One spec for Phase 3, and one plan per slice (§8).
- The display revert lives in uqd, not the app, so it still happens if the app crashes.
- PipeWire keeps its own defaults (default devices, routing, volumes). uqd does not store sound
  settings.

**Success criteria**

- Every uQ setting from Phases 1–2 can be changed from the app. Each change takes effect live and
  survives a reboot.
- A display change that blacks out the screen comes back on its own within 15 s, with no input
  from the user.
- Rebinding a default key works at once. A conflicting bind cannot be saved.
- `Terminal=true` apps open in the terminal chosen under Default apps.
- From the app you can join Wi-Fi, a WPA-Enterprise network, and an imported WireGuard or OpenVPN
  connection. You can also pair a Bluetooth device, move one app's audio to another device, add a
  user, and set the time zone.
- A broken plugin page shows an error card. It never breaks the app.
- Nothing in the app runs as root. Admin actions go through polkit.

**Non-goals**

- A greeter or login settings (Phase 5).
- Settings sync or profiles.
- Firewall, printers, disks, and software updates. bindle and the other repos cover those, and
  can add plugin pages.
- Accessibility services such as a screen reader or magnifier. The app itself stays
  keyboard-navigable and labelled.

## 2. Architecture

### 2.1 App

- `apps/uq-settings` is a Rust binary using cxx-qt and Kirigami.
- It is single-instance through the session bus name `org.uq.Settings1`, which offers
  `ShowPage(s page_id, s setting_id)`. The quick-settings "Settings…" entry, the launcher, and
  `uqctl settings [<page>] [<setting>]` all use it.
- Each page's state lives in Rust QObjects (cxx-qt) holding plain Rust models that can be unit
  tested. The QML views are thin.
- It talks to uqd through a typed zbus proxy, `uq_core::client` (shared with `uqctl`).
- All text goes through `UqLabel`, a Kirigami-side `Label` with `textFormat: Text.PlainText`. A
  lint covering `apps/uq-settings/qml` forbids bare `Text`, `Label`, `TextEdit` and `TextInput`, as
  in Phase 2.

### 2.2 Pages, categories and search

| Category | Pages |
|---|---|
| Personal | Appearance, Wallpaper, Look and layouts |
| Devices | Displays, Input, Sound, Bluetooth |
| Connections | Network |
| uQ | Shell, Keybinds |
| System | Power, Users, Date and time, Region, Default apps, About |
| Apps | plugin pages (§2.5) |

- **Search index:** each page declares searchable settings as `{id, label, keywords[]}`. Core
  pages declare them in Rust; plugin pages declare them in `page.toml`.
- **Search behaviour:** search is fuzzy over labels and keywords. A result opens the page and
  highlights the row for 2 s. `setting_id` in `ShowPage` uses the same ids.

### 2.3 uqd additions

New state domains, validated in `uq_core::settings` alongside the Phase 2 keys. The ones that touch
Hyprland render into new sections of `generated.lua`, which is verified before it is applied.

| Domain | Contents | Applied through |
|---|---|---|
| `look` | `gaps_in`, `gaps_out`, `border_width`, `rounding`, blur on/off/size/passes, shadow on/off, animations on/off/speed | `general`, `decoration` and `animations` in `generated.lua`. These override the theme's `ui` tokens; `null` means use the theme. |
| `layouts` | default layout, per-workspace rules (existing), dwindle/master/scrolling options | `generated.lua` |
| `monitors` | per output name: mode, position, scale, transform, enabled, mirror-of, VRR | `hl.monitor({…})` lines; applied live through `eval` |
| `input` | keyboard layouts and variants, the layout-switch combo, repeat rate and delay, natural scroll, tap-to-click, pointer speed and acceleration profile, all per device class | `input` block and `hl.device` |
| `keybinds` | overrides of defaults (rebind or disable) plus custom binds (§2.4) | rendered binds in `generated.lua` |
| `wallpaper` | per output: image path (absolute, local), fit (fill/fit/center/tile), slideshow folder and interval, fallback colour | bridge `WallpaperChanged` event to the shell |
| `shell` | island on/off per island, launcher providers on/off and order, `clock_24h`, `terminal` (a desktop id, used for `Terminal=true` launches instead of the hard-coded kitty) | bridge `SettingsChanged` |
| `appearance` | UI font, monospace font, size, cursor theme and size, icon theme, Kvantum on/off. These override theme tokens; `null` means use the theme. | theme render + gsettings |

**D-Bus additions:**

- `GetSection(s domain) → s` and `SetSection(s domain, s json)`. Writes are validated per domain.
  An invalid write returns `InvalidSetting` with the reason.
- `ListOutputs() → s`: live outputs from Hyprland (`monitors all -j`) together with the stored
  config.
- `ApplyMonitors(s json) → s token` and `ConfirmMonitors(s token)`.
- `ListBinds() → s`: defaults, overrides, customs, and read-only binds from `user.lua` where known.
- Signals: `SectionChanged(s domain, s json)`, `MonitorsChanged`, `BindsChanged`.

**Monitor apply and revert:**

1. `ApplyMonitors` validates the request.
2. It renders a candidate and verifies it.
3. It saves the current monitor config as a rollback point.
4. It applies the new config live and returns a token.
5. `ConfirmMonitors(token)` writes the change to `state.toml` and drops the rollback point.
6. If no confirm arrives within 15 s, uqd re-applies the rollback point, emits `MonitorsChanged`,
   and discards the token.

The state file is written only on confirm, so a reboot during the revert window comes back on the
old config. Only one apply can be pending at a time; a second `ApplyMonitors` call replaces the
first, keeping the original rollback point.

**Wallpaper:** the shell gains a per-monitor `PanelWindow` with `WlrLayer.Background`, namespace
`uq-wallpaper`.

- **Image source:** images load from local absolute paths only. The slideshow cycles through the
  folder's `jpg`, `jpeg`, `png` and `webp` files in name order; `jxl` and `avif` are included if Qt
  can read them.
- **Fallback:** a missing or unreadable image shows the fallback colour, which is the theme `bg`
  unless set.
- **Transitions:** a 300 ms crossfade, skipped when the user setting `ui.reduce_motion` is on
  (Look and layouts → Reduce motion).

**Kvantum:** uqd renders `~/.local/state/uq/theme/Kvantum/uq/uq.kvconfig`.

- **Base theme:** `[%General] inherits=` points to the base theme, chosen from those installed
  under `/usr/share/Kvantum/` in the order `KvGnome`, `KvAdapta`, `KvArc`, `KvFlat`. If none of
  them is installed, uQ falls back to the qt6ct Fusion palette.
- **Colours:** a colour section filled from the theme tokens.
- **Wiring:** `uqctl setup --apps` (Phase 1 consumers) also writes `~/.config/Kvantum/kvantum.kvconfig`
  (`theme=uq`, symlinking the folder) and sets qt6ct `style=kvantum`. As in Phase 1, these files
  are only created if they don't already exist.

### 2.4 Keybinds

- **Rendering:** `base.lua` no longer defines binds. uqd renders every bind from one table: uQ's
  defaults with the user's overrides applied, followed by the user's custom binds.
- **Bind model:** each bind has a stable id (`uq.spotlight`, `uq.ws.focus.3`, …), a combo
  (modifiers + key, or mouse button), `locked`, `repeating`, and an action from a closed set:
  - `shell` — `hl.dsp.global("uq:<name>")`, where the name comes from the shell's registered
    shortcuts.
  - `window` / `workspace` — the Phase 2 Dispatch set.
  - `launch` — `{desktop_id}`, through the bridge-equivalent launch path in uqd.
  - `command` — a string run with `sh -c` in a `systemd-run --user --scope`. It can only be
    entered in the editor's command field.
- **Conflicts:** two enabled binds with the same normalised combo and submap conflict. uqd rejects
  a conflicting save, and the editor warns as you type and names both binds.
- **Escaping:** all rendered strings go through `lua_string`. Combos are validated against
  Hyprland's key names, with the list taken from the pinned source at plan time.
- **Binds from `user.lua`:** uqd cannot parse them in general, so they are listed read-only and
  only if the user marks them with a `-- uq:bind` comment. Otherwise they are not shown.
- **Migration:** this replaces Phase 1/2 bind lines in `base.lua`. The plan includes a migration
  test that checks the rendered default table equals the old binds.

### 2.5 Plugin pages

- **Location and manifest:** each page is a folder `/usr/share/uq/settings/pages/<id>/` containing
  a `page.toml` and a QML module. The only search directory is under `/usr`; user-writable
  directories are never searched.
- **`page.toml` keys:** `id`, `name`, `icon`, `category = "apps"`, `qml = "Page.qml"`,
  `dbus_name`, `keywords = []`, and `[[setting]] id/label/keywords` entries.
- **What the page gets:** a context with `Theme` (tokens), `UqLabel`, and `dbus`, a helper that can
  only call methods and read properties on `dbus_name`. The page is otherwise trusted package
  code, so this limit prevents mistakes; it is not a sandbox.
- **Failure:** if `page.toml` fails to parse or the QML fails to load, the page shows a "This page
  couldn't load" card with the error text in plain text. A page is loaded only when opened.
- **SDK:** `docs/settings-pages.md` plus a template page under `templates/settings-page/`.

### 2.6 System pages

| Page | Backend | Scope |
|---|---|---|
| Sound | `pipewire` crate in a worker thread | outputs and inputs, profiles, default sink/source, per-device volume/mute, per-app stream volume and routing, test sound |
| Network | NetworkManager D-Bus (zbus) | Wi-Fi scan, connect, forget; saved connections; wired; hotspot; VPN (import and edit WireGuard; OpenVPN through the NM plugin if installed); 802.1x (PEAP, TTLS, TLS); IPv4/IPv6 method, DNS, metered |
| Bluetooth | BlueZ D-Bus | power, discoverable, scan, pair, trust, connect, forget, battery level |
| Power | power-profiles-daemon, UPower, logind | profile, battery health/state, lid and power-key actions through `uq-settings-helper` (§3) |
| Users | AccountsService | avatar, real name, password, account type, add user, remove user (with or without files); auto-login arrives with the Phase 5 greeter |
| Date and time | timedated | time zone, NTP on/off, manual date and time, 24 h clock (uqd `shell` key) |
| Region | localed | system locale, formats locale, console keymap |
| Default apps | `~/.config/mimeapps.list`; terminal through uqd `shell.terminal` | browser, mail, files, terminal, text, images, video, music, PDF |
| About | os-release, hostnamed, `/proc`, `lspci` | hostname (editable), OS, kernel, CPU/GPU/RAM, uQ/Hyprland/Quickshell versions |

**Agents while open:**

- The Network page registers as a NetworkManager secret agent only while open, and only for
  connections the user starts from it.
- The Bluetooth page registers a BlueZ agent (`KeyboardDisplay`) only while open.

## 3. Security

- **Nothing runs as root** except `uq-settings-helper`.
- **Admin actions use the owning service's own polkit actions:** NetworkManager, AccountsService,
  timedated, localed, hostnamed and BlueZ. uQ's polkit agent (Phase 2 slice 5) shows the prompts.
- **`uq-settings-helper`** is a Rust daemon, D-Bus-activated on the system bus as
  `org.uq.SettingsHelper1`. It exposes exactly one method:
  - `SetLogindPowerActions(s lid, s lid_on_ac, s power_key)`. Each argument must be one of
    `suspend`, `hibernate`, `poweroff`, `lock`, `ignore`. It requires polkit
    `org.uq.settings.power` (`auth_admin_keep`).

  Auto-login is set by the Phase 5 greeter's own mechanism, not by this helper.
- **What the helper writes:** only `/etc/systemd/logind.conf.d/50-uq.conf`. Writes are atomic and
  rendered from a fixed template, so caller-supplied strings never become paths or file content
  beyond the closed values. It never restarts logind; changes take
  effect at the next logind reload, which the page explains.
- **Helper unit:** `ProtectSystem=strict`, `ReadWritePaths=/etc/systemd/logind.conf.d`,
  `ProtectHome=yes`, `PrivateNetwork=yes`, `NoNewPrivileges=yes`, a minimal
  `CapabilityBoundingSet`, `RestrictAddressFamilies=AF_UNIX`, and an exit after 30 s idle.
- **Users:**
  - Removing a user requires typing the user's name.
  - You cannot remove yourself or the last administrator, or demote the last administrator.
  - Password fields set `ImhSensitiveData | ImhNoPredictiveText | ImhHiddenText` and are cleared
    after each attempt.
  - Your own password is changed through `/usr/bin/passwd` (as GNOME Settings does): the page
    asks for your current password and answers passwd's prompts one at a time on its standard
    input, in the C locale. Passwords never go in argv or the environment, and the copies are
    zeroed. Other people's passwords go through AccountsService `SetPassword` (administrator).
- **Network secrets:** passed directly to NetworkManager, with "available to all users" off by
  default. They are never logged and never stored by uQ.
- **Bluetooth:** passkeys are shown in plain text and need an explicit Confirm. There is no
  auto-accept, and pairing works only while the page is open.
- **Untrusted text:** SSIDs, device names, user real names, plugin text, app names and
  `mimeapps` entries all go through `UqLabel` (plain text).
- **Commands:** keybind `command` actions are entered only in the editor and run in a scope. They
  are never built from other strings.

## 4. Error handling

| Failure | Behaviour |
|---|---|
| uqd not running | uQ pages show "uQ's settings service isn't running" and become read-only; system pages keep working |
| System service missing (NetworkManager, BlueZ, PipeWire, AccountsService, power-profiles-daemon) | The page shows "Not available" and names the package to install |
| `generated.lua` verify fails | uqd rejects the change; the page shows Hyprland's message in plain text and the control returns to its last good value |
| Display change not confirmed | uqd reverts after 15 s, whether or not the app is open |
| polkit denied or cancelled | The control returns to its previous value quietly; no retry loop |
| Helper missing or failing | Power action controls are hidden, with a note |
| Keybind conflict | Save is blocked in the editor; uqd rejects it as well |
| Plugin page broken | Error card; the rest of the app works |
| Wallpaper image missing or unreadable | Fallback colour; the page shows the error |

## 5. Testing

- **uqd:**
  - Validation for every new domain.
  - Golden tests of the rendered `generated.lua` sections; CI also verifies them with Hyprland
    `--verify-config`.
  - The monitor apply/revert state machine: confirm, timeout, crash during the revert window,
    reboot during the revert window, and a second apply.
  - Keybind conflict detection.
  - A test that the default bind table matches the old `base.lua` binds.
- **Helper:**
  - The closed value sets.
  - Template output.
  - Rejecting path-like or unknown input.
  - polkit checks against a mock authority.
- **App:**
  - Rust unit tests for every page model.
  - Offscreen QML tests for the page loader, search, plugin error card and the revert dialog.
  - The text lint over `apps/uq-settings/qml`.
- **System pages:** integration tests against fake D-Bus services on a private bus. The fakes
  cover NetworkManager (Wi-Fi, a VPN connection, 802.1x), BlueZ (device, agent pairing),
  AccountsService, timedated, localed and power-profiles-daemon. PipeWire uses a headless
  `pipewire` instance where CI allows it, otherwise a recorded `pw-dump` fixture.
- **Live checklist per slice** (manual):
  - monitor revert on real hardware
  - pairing a real device
  - joining WPA-Enterprise
  - importing a real WireGuard config
  - moving one app's audio to another device

## 6. Code layout

```
apps/uq-settings/src/            main.rs, app.rs (single instance, ShowPage), search.rs, plugins.rs
apps/uq-settings/src/pages/      appearance.rs, wallpaper.rs, look.rs, displays.rs, input.rs, keybinds.rs,
                                 shell.rs, sound.rs, network.rs, bluetooth.rs, power.rs, users.rs,
                                 datetime.rs, region.rs, defaultapps.rs, about.rs
apps/uq-settings/qml/            Main.qml, Sidebar.qml, Search.qml, components/ (UqLabel…), pages/*.qml
crates/uq-core/src/client.rs     typed zbus proxy for org.uq.Desktop1 (shared with uqctl)
crates/uq-core/src/settings/     domain modules: look, layouts, monitors, input, keybinds, wallpaper, shell, appearance
crates/uq-settings-helper/       root helper + polkit policy + systemd unit
shell/wallpaper/                 Wallpaper.qml (background layer), logic/wallpaper.js
docs/settings-pages.md, templates/settings-page/
scripts/lint-settings.sh
```

## 7. Packaging

- **`uq` package depends on:** `kirigami`, `qqc2-desktop-style`, `qt6-declarative`, `pipewire`,
  `networkmanager`, `bluez`, `accountsservice`, `power-profiles-daemon` and `kvantum`.
- **Optional dependencies:** `networkmanager-openvpn` (OpenVPN connections) and `fprintd`
  (already listed).
- **New files:** the helper's D-Bus service, its polkit policy
  `/usr/share/polkit-1/actions/org.uq.settings.policy`, and the systemd system unit.

## 8. Slices (one plan each)

1. **App foundation:** app skeleton, single instance with `ShowPage`, sidebar, search, the plugin
   page loader, the `uq_core::client` proxy, and the text lint. Pages: Appearance (theme,
   variant, schedule, fonts, cursor, icons, Kvantum) and Wallpaper (with the shell background
   layer).
2. **Look and shell:** the Look and layouts page, plus the Shell page (pins, providers, islands,
   notification backend, idle, privacy).
3. **Displays and input:** the Displays page (arrangement, modes, scale, mirroring, apply and
   revert) and the Input page.
4. **Keybinds:** moving binds from `base.lua` into uqd, and the keybinding editor.
5. **System part one:** Sound, Bluetooth and Network.
6. **System part two:** Power (with `uq-settings-helper`), Users, Date and time, Region, Default
   apps and About, plus the plugin-page SDK, docs and template.
