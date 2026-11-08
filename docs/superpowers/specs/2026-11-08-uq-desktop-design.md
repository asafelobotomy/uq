# uQ desktop — design

Date: 2026-11-08 · Status: approved in brainstorming, awaiting spec review

## 1. Intent

uQ is a personal, cohesive daily-driver desktop built around Hyprland. It ships a shell, a
settings app, a system-wide theme system and the author's own apps as native, integrated parts.

**Stated requirements**

- Audience: the author's own daily driver; polish for others is secondary.
- Window management: layout chosen **per workspace** (dwindle, master, scrolling), switchable from
  the bar or a keybind, persisting across reloads and reboots.
- Configuration: GUI settings app for common options; text config remains the source of truth.
- Theming: one theme drives the whole system (compositor, shell, Qt, GTK, terminal, icons,
  cursor). Built-in themes, each with a dark and a light variant: **Gruvbox, Dracula, Matcha,
  Forest, Graphite, Rosewood, Velvet** (14 variants). Variant switching is manual (quick settings)
  with an optional schedule (fixed times or sunset/sunrise).
- Shell components: bar + tray, launcher, notifications + OSD, lock screen, greeter, power menu.
- Integrated repos: noti-fi, scale_tail, XZram, scdr, bindle, appOFholding, crumBS, hauler,
  emobie, oysterAV, romforge.
- Integration modes: shared theme file, settings-app pages, bar/quick-settings widgets, launcher
  providers.
- Stack: Quickshell (QML) for the shell; Rust + cxx-qt / Kirigami for apps.
- Lock and login: both in Quickshell (ext-session-lock; greeter on greetd).
- Packaging: Arch-family packages (Arch, Manjaro).
- Relationship to Hyprland: **stock upstream, pinned** to a tested release. uQ is a distribution of
  Hyprland, not a code fork.

**Assumptions**

- Development and primary target is Arch/Manjaro on the author's machine.
- noti-fi is ported from KDE Plasma to Hyprland rather than used unchanged.
- bindle becomes uQ's first-run installer step.

**Success criteria**

- A fresh Arch install plus `pacman -S uq` (or bindle's uq profile) yields a working, themed
  desktop on next login.
- Changing theme or variant re-themes compositor, shell, Qt and GTK apps without logging out.
- Workspace layouts set at runtime survive `hyprctl reload` and reboot.
- uQ runs with none of the 11 integrated apps installed; each one installed adds its widget, page
  and/or launcher results automatically.

**Non-goals**

- Forking or patching Hyprland (revisit only if a concrete need appears that plugins/IPC cannot
  meet).
- A custom ISO, non-Arch distros, or Nix/Flatpak packaging of the desktop itself.
- Supporting compositors other than Hyprland.

## 2. Architecture

Four layers; each talks only to the layer below through a defined interface.

### 2.1 Compositor

- Upstream `hyprland`, version-pinned by the `uq-hyprland-pin` meta-package.
- Lua configuration (Hyprland ≥ 0.55). `~/.config/hypr/hyprland.lua` is a three-line stub that
  `require`s, in this order:
  1. `/usr/share/uq/hypr/base.lua` — uQ defaults (packaged, read-only).
  2. `~/.config/hypr/uq/generated.lua` — written only by `uqd` (colours, workspace layout rules,
     monitor choices made in settings).
  3. `~/.config/hypr/uq/user.lua` — the user's hand edits; included last, never written by uQ.
  (Files live under `~/.config/hypr/` so Hyprland's relative `require("./uq/…")` resolves them.)
- Per-workspace layouts use Hyprland's native workspace rules (`layout = "dwindle" | "master" |
  "scrolling"`, available since 0.54).

### 2.2 `uqd` — session daemon (Rust)

- systemd user unit, `Restart=on-failure`.
- Sole owner of desktop state: current theme + variant, schedule, per-workspace layouts,
  generated config.
- Sole writer of `generated.lua` and rendered theme files.
- Talks to Hyprland over its IPC socket (events + `hyprctl`-equivalent requests).
- D-Bus API `org.uq.Desktop1` on the session bus:
  - Methods: `SetTheme(name, variant)`, `GetTheme()`, `SetSchedule(spec)`,
    `SetLayout(workspace, layout)`, `GetLayouts()`, `Reload()`.
  - Signals: `ThemeChanged(name, variant, tokens_path)`, `LayoutChanged(workspace, layout)`,
    `ConfigError(message)`.
- Feeds the focused window's PID to scdr (if present) from Hyprland `activewindow` events.
- `uqctl`: CLI client for the same D-Bus API.

### 2.3 Shell — `uq-shell` (Quickshell)

- Bar (workspaces with layout indicator, tray, clock, status widgets), quick settings, launcher,
  OSDs, power menu, lock screen (ext-session-lock).
- Notification daemon and centre: noti-fi, ported to Hyprland (layer-shell placement, per-monitor).
- Reads state from and sends requests to `uqd`; never edits config files.
- Discovers optional app widgets and launcher providers at runtime via D-Bus name presence.

### 2.4 Apps (Rust + cxx-qt / Kirigami)

- `uq-settings`: central settings app. Core pages: Appearance (theme, variant, schedule), Layouts,
  Display, Input, Welcome (first run).
- Integrated apps connect through four **extension points**:

| Extension point | Mechanism |
|---|---|
| Theme | Read `~/.local/state/uq/theme/tokens.json` (and the Qt/GTK files below); reload on `ThemeChanged` |
| Settings page | QML module installed to `/usr/share/uq/settings/pages/<id>/` with a `page.toml` manifest; loaded by `uq-settings` |
| Widget | QML component in `/usr/share/uq/shell/widgets/<id>/` plus the app's own D-Bus status interface |
| Launcher provider | D-Bus interface `org.uq.Launcher1.Provider` (`Search(query) → results`, `Activate(id, action)`); registered via a `.provider` file in `/usr/share/uq/launcher/` |

### 2.5 Theme pipeline

- Theme source: `uq-themes/<name>/<dark|light>.toml` — semantic tokens (surface levels, text,
  accent, success/warn/error, border, radius, gap, animation curve, fonts, icon theme, cursor).
- Templates render tokens into: Hyprland colour block (in `generated.lua`), QML `Theme` singleton
  (`tokens.json`), Kvantum + qt6ct, GTK 3/4 CSS, terminal palettes (foot, kitty, alacritty),
  cursor and icon settings via gsettings.
- Render target: `~/.local/state/uq/theme/` via staging directory + atomic rename.

## 3. Components per repo

Each repo keeps its own releases. Integration lives either in the repo (`uq/` folder) or in a
`uq-<name>` adapter package.

| Repo | Role in uQ | Extension points |
|---|---|---|
| noti-fi | Notification daemon + centre (Plasma → Hyprland port) | theme, widget (bell + do-not-disturb), settings page |
| scale_tail | Tailscale toggle in quick settings; its tray app becomes optional | widget, settings page |
| XZram | "Memory & swap" settings page over existing `xzramd` D-Bus | settings page |
| scdr | Scheduler service on by default; receives focused-window PID from `uqd` | settings page |
| bindle | First-run installer: installs the `uq` profile, then hands off to Welcome in `uq-settings` | settings page (re-run setup) |
| appOFholding | AppImages as launcher results; update badge in quick settings | launcher, widget, settings page |
| crumBS | "Storage & cleanup" settings page; "uninstall…" launcher action | settings page, launcher action |
| hauler | Default download manager; bar progress widget; completion notifications | widget, notifications |
| emobie | Emoji launcher mode (`:` prefix) + global hotkey | launcher |
| oysterAV | Security status in quick settings; scans page | widget, settings page |
| romforge | Games as launcher results; optional game-mode workspace rule | launcher |

New uQ components: `uqd`, `uqctl`, `uq-shell`, `uq-settings`, `uq-themes`, `uq-greeter` (QML on
greetd), `uq` meta-package, `uq-hyprland-pin`.

## 4. Data flows

**Theme change.** User picks a theme/variant → shell calls `uqd.SetTheme` → `uqd` renders all
templates into staging → validates generated Lua → atomic rename into place → rewrites colour block
in `generated.lua` → `hyprctl reload` → emits `ThemeChanged` → QML/Qt apps re-read tokens; GTK
apps update via gsettings. The schedule is a `uqd` timer calling the same path.

**Layout change.** Keybind or bar → shell calls `uqd.SetLayout(ws, layout)` → `uqd` applies live via
Hyprland IPC → records the workspace rule in `generated.lua` → emits `LayoutChanged`.

**App status.** Apps publish on their own D-Bus interfaces; widgets subscribe directly. `uqd` is not
a hub for app data.

## 5. Error handling

| Failure | Behaviour |
|---|---|
| Generated config invalid | `uqd` validates before swap; on failure keeps last good `generated.lua`, emits `ConfigError`, shows a notification, logs to journal. `user.lua` is never rewritten. |
| Theme render fails mid-way | Nothing is swapped (staging + atomic rename); previous theme stays. |
| `uqd` down | systemd restarts it; shell shows last known state and greys out state-changing controls. Hyprland unaffected. |
| Integrated app missing/crashed | Its widget, page and launcher results don't appear; no hard dependencies. |
| Lock screen crash | ext-session-lock keeps the session locked; a fallback `hyprlock` keybind provides recovery. |
| Hyprland upgrade | Pin moves only after integration tests pass on the new release. |

## 6. Testing

- `uqd` unit tests: template rendering, schedule maths, `generated.lua` generation.
- Golden-file tests for all 14 theme variants.
- Integration: headless Hyprland session in CI (`WLR_BACKENDS=headless`), scripted D-Bus calls,
  assertions via `hyprctl` that layouts and theme colours applied.
- Shell: `qmltestrunner` component tests; screenshot tests per theme variant for bar, quick
  settings and launcher.
- Token linter enforcing WCAG AA contrast for text/surface pairs.

## 7. Phases

Each phase leaves a usable daily driver.

1. **Foundation:** `uq` meta-package, Hyprland pin, `base.lua`, `uqd` (theme + layouts + D-Bus),
   `uqctl`, 7×2 themes and templates.
2. **Shell:** bar, quick settings, launcher, OSDs, power menu, noti-fi port, lock screen.
3. **Settings:** `uq-settings` core pages; extension-point SDK, manifests and templates.
4. **Repo integrations**, in order: scale_tail, appOFholding, hauler, emobie, XZram, scdr, crumBS,
   oysterAV, romforge (noti-fi lands in phase 2).
5. **Login and first run:** `uq-greeter`, bindle hand-off, Welcome flow.

## 8. Repository layout

New monorepo `asafelobotomy/uq`:

```
crates/uqd/        session daemon
crates/uqctl/      CLI
apps/uq-settings/  Rust + cxx-qt / Kirigami
shell/             Quickshell config (bar, launcher, OSD, lock, notifications)
greeter/           QML greeter for greetd
themes/            7 themes × dark/light TOML + templates
packaging/         PKGBUILDs (uq, uq-hyprland-pin, uq-<name> adapters)
docs/
```

Existing repos stay separate; each gains a `uq/` folder or a `uq-<name>` adapter package.

## 9. Open items for planning

- Exact Hyprland version to pin (latest stable with Lua config at plan time).
- Whether noti-fi's port lives in noti-fi itself (Hyprland backend) or in `uq-shell`.
- Licences of upstream palettes (Gruvbox, Dracula) to credit in `themes/`; Matcha, Forest,
  Graphite, Rosewood, Velvet palettes to be authored for uQ.
