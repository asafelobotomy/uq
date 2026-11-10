# uQ Phase 5: login and first run design

Date: 2026-11-10 · Status: approved 2026-11-10; the decisions in §9 are confirmed with the
recommended options.
Parent spec: `2026-11-08-uq-desktop-design.md` (§1 success criteria, §3 bindle, §7 phase 5).
Builds on: Phase 2 (lock screen, `lock/Auth.qml`), Phase 3 (`uq-settings`,
`uq-settings-helper`, the Users page), and the roadmap's M0b (bindle installs uQ through its
minimal profile). Roadmap: M3 in `../plans/2026-11-09-uq-road-to-1.0.md`.

## 1. Intent

Phase 5 closes the loop from a bare Arch install to a finished uQ desktop: a themed greeter, a
session that sets itself up the first time, and a Welcome flow that walks the user through the
choices that matter. bindle hands off to that Welcome instead of finishing on its own.

**What the roadmap fixed**

- `uq-greeter`: QML on greetd, themed, reusing the lock screen's auth patterns.
- The bindle hand-off on top of M0b's minimal profile: bindle → Welcome in `uq-settings`, and a
  "re-run setup" page.
- The Welcome flow lives in `uq-settings`.
- **Gate test:** a fresh Arch VM, then `pacman -S uq` or bindle's profile, then reboot. The result
  must be the greeter, then the themed desktop, then Welcome (design success criterion 1).

**Assumptions**

- Phases 3 and 4 are complete. Welcome uses Phase 3's pages and uqd domains; its Apps step uses
  Phase 4's `uqctl integrations list`.
- Quickshell ships `Quickshell.Services.Greetd` (present in the pinned 0.3.x).
- greetd is the display manager. bindle's catalog entry for uQ currently says
  `greeter = "sddm"`; it changes to `uq-greeter` in this phase.
- The greeter runs on the pinned Hyprland, which is already a dependency, so the greeter looks
  and behaves like the session on multi-monitor setups.

**Success criteria**

1. A fresh Arch install plus `pacman -S uq` and `systemctl enable greetd` (or bindle's uQ profile)
   reboots into the uQ greeter, then a themed uQ session, then Welcome, with no manual
   `uqctl setup`.
2. The greeter follows the theme, variant and wallpaper chosen for it in Settings, on every
   monitor, without reading any user's home directory.
3. Password login works for every regular user. A wrong password, a locked account (faillock) and
   an expired password each show a clear message and never leave the greeter stuck.
4. Welcome appears once per user, can be skipped at any step, and can be re-opened from Settings.
   Every choice in it takes effect live, through the same uqd APIs as the full settings pages.
5. When bindle installed uQ, the user sees bindle's app picker first and then Welcome, never both
   at once and never Welcome twice.

**Non-goals**

- Fingerprint login at the greeter by default (see §2.3 and Decision 2). The lock screen keeps
  fingerprint unlock.
- Remote or network logins, LDAP and smart cards beyond what the system PAM stack already does.
- Multi-seat.
- A graphical installer (bindle and archinstall cover installing).

## 2. Greeter

### 2.1 How it runs

- **greetd config.** uQ ships `/etc/uq/greetd.toml` (in `backup=()`) and a drop-in
  `/usr/lib/systemd/system/greetd.service.d/uq.conf` that runs `greetd --config
  /etc/uq/greetd.toml`. The stock `/etc/greetd/config.toml` stays untouched, so removing uQ falls
  back to it. The drop-in only applies when `/etc/uq/greetd.toml` exists
  (`ConditionPathExists`) and uQ is the chosen greeter (§2.6).
- **Greeter session.** `[default_session] command = "/usr/lib/uq/uq-greeter-session"`, `user =
  "greeter"`. That script starts Hyprland with `/usr/share/uq/greeter/hyprland.lua`, a fixed config
  with no binds except VT switching, no animations at start, and an `exec-once` of
  `quickshell -p /usr/share/uq/greeter`. When the greeter's session starts, Quickshell exits and
  Hyprland exits with it.
- **Code.** `greeter/` in this repo, installed to `/usr/share/uq/greeter/`: `shell.qml`,
  `Greeter.qml`, `Auth.qml`, `UserList.qml`, `SessionPicker.qml`, `PowerButtons.qml`, and `core/`
  and `logic/` linked to the shell's, as `lock/` does today.
- **Shared auth UI.** The parts the lock screen and greeter share move into one module,
  `shell/auth/` (`PasswordField.qml`, `MessageView.qml`, `Avatar.qml`, `Clock.qml`, and
  `logic/auth.js` with `messageView`), used by both `lock/` and `greeter/`. The lock screen's
  behaviour does not change; its tests keep passing unchanged.

### 2.2 What the greeter can read

The greeter runs as the `greeter` user and must never read a user's home directory.

- **Theme.** uQ's package ships every variant pre-rendered, as
  `/usr/share/uq/themes/rendered/<name>-<variant>/tokens.json` (generated at build time by the
  same renderer as uqd, covered by the existing golden tests). `/etc/uq/greeter.toml` names the
  theme and variant. Default: the default theme, dark.
- **Wallpaper.** `/var/lib/uq-greeter/wallpaper` (owned by root, mode 0644), or the theme `bg`
  colour when it's missing or unreadable. The greeter decodes it; nothing root-side ever decodes
  an image.
- **Monitors.** Hyprland's automatic layout by default. Optionally `/etc/uq/greeter-monitors.toml`,
  rendered by the greeter session script into `hl.monitor` lines through the same validation as
  uqd's `monitors` domain.
- **Users.** AccountsService on the system bus (`ListCachedUsers`, `UserName`, `RealName`,
  `IconFile`, `SystemAccount`, `Locked`). System accounts and locked accounts are hidden. Avatars
  come from `IconFile` paths under `/var/lib/AccountsService/icons/` only.
- **Sessions.** `.desktop` files in `/usr/share/wayland-sessions/`. Only the `Name` and `Exec`
  keys are used; `Exec` is passed to greetd as the session command split by desktop-entry rules.
- **Its own state.** `/var/cache/uq-greeter/state.toml` (owned by `greeter`): the last user and
  each user's last session. Nothing else.

Settings writes `greeter.toml`, the wallpaper and the monitor file through
`uq-settings-helper` (§4).

### 2.3 Authentication

- **Through greetd.** `Greetd.createSession(user)`, answering prompts with `Greetd.respond()`,
  then `Greetd.launch(command, env)`. PAM runs inside greetd with its own `/etc/pam.d/greetd`
  (from the `greetd` package); uQ does not ship or edit it.
- **Prompts.** The greeter answers echo-off prompts with the typed password, as the lock screen
  does. An echo-on prompt (a username, an OTP from a PAM module) shows a visible text field with
  the prompt as plain text. Info and error messages go through `MessageView` as plain text,
  faillock lockouts included.
- **Expired passwords.** When PAM asks for a new password, the greeter shows "New password" and
  "Confirm" fields and answers each prompt in order. A mismatch is caught in the greeter before it
  responds.
- **Fields and logging.** Password fields set `ImhSensitiveData | ImhNoPredictiveText |
  ImhHiddenText` and are cleared after each attempt. Passwords and PAM messages are never logged.
- **Cancel.** Esc or choosing another user calls `Greetd.cancelSession()`, so a half-finished
  conversation never blocks the next login.
- **Fingerprint (Decision 2).** Off at the greeter. greetd runs one PAM conversation at a time, so
  the lock screen's "password and fingerprint at once" can't be copied. A fingerprint login also
  can't unlock a password-protected keyring. If the user adds `pam_fprintd` to
  `/etc/pam.d/greetd` themselves, the greeter handles its info messages and timeouts without
  getting stuck, and `docs/greeter.md` documents how. A later phase can revisit this.

### 2.4 Layout and behaviour

- The main monitor (the one with the cursor, else the first) shows the clock, the user list (or a
  username field if "Hide user list" is set in `greeter.toml`), the password field, the session
  picker and power buttons. The other monitors show the wallpaper and clock.
- **Power.** Suspend, reboot and power off through logind; each needs a second click to confirm.
  A logind refusal is shown as plain text.
- **Keyboard.** Tab order user → password → session → power; Enter logs in; the keyboard layout
  comes from `localectl`'s X11 layout (read once at start) so passwords type as expected.
- **Reduce motion** from `greeter.toml`, default off.
- **Crash recovery.** If Quickshell exits without a session starting, `uq-greeter-session` exits
  non-zero and greetd restarts the greeter. After 3 failures in 60 s it falls back to `agreety`
  (greetd's text greeter) on the same VT, so a broken greeter never locks the user out.

### 2.5 Auto-login

- Set from Settings → Users (Phase 3 left a placeholder). `uq-settings-helper` writes an
  `[initial_session]` table into `/etc/uq/greetd.toml` with the user and the uQ session command.
- Only one user can have it. Turning it on needs polkit `org.uq.settings.autologin`
  (`auth_admin`, no keep). The page warns that anyone at the machine gets that session, and that
  it's recommended only with full-disk encryption.

### 2.6 Choosing the greeter

- `uqctl greeter enable` and `uqctl greeter disable` (root, through `run0`, never sudo) switch
  `display-manager.service` to greetd with uQ's config, or back. `enable` refuses if greetd isn't
  installed, and names the current display manager it replaces.
- `pacman -S uq` doesn't switch display managers on its own; the post-install message gives the
  one command. bindle switches it as part of the uQ profile (§5).

## 3. The uQ session

- **Session entry.** uQ ships `/usr/share/wayland-sessions/uq.desktop` (`Name=uQ`,
  `Exec=/usr/bin/uq-session`). The greeter lists it first and preselects it.
- **`uq-session`** is a small script:
  1. Runs `uqctl setup --quiet` if `~/.config/hypr/hyprland.lua` doesn't contain uQ's stub. Setup
     is idempotent and never overwrites a user's own files (M0b made it safe unattended). A setup
     failure is written to `~/.local/state/uq/session.log` and the session still starts, so the
     user is never locked out of their desktop by it.
  2. Execs `Hyprland` (start-hyprland when the pinned release needs it).
- This removes the manual `uqctl setup` step from both install paths, which is what success
  criterion 1 needs.

## 4. Settings additions

### 4.1 Login screen page

A new page, **System → Login screen**, shown when uQ is the greeter:

- Theme and variant for the greeter (defaults to "same as mine", copied at save time).
- Wallpaper (defaults to "same as mine", copied at save time; images up to 20 MiB).
- "Use my display layout at login" (copies the current `monitors` section).
- Hide user list; reduce motion.
- Auto-login (the Users page links here).

### 4.2 `uq-settings-helper` additions

Each method is a closed operation with its own polkit action, as in Phase 3 §3.

| Method | Writes | Polkit |
|---|---|---|
| `SetGreeterAppearance(s theme, s variant, b hide_users, b reduce_motion)` | `/etc/uq/greeter.toml` from a fixed template; theme and variant must name a shipped variant | `org.uq.settings.greeter` (`auth_admin_keep`) |
| `SetGreeterWallpaper(h fd)` | `/var/lib/uq-greeter/wallpaper`, copied from the fd (regular file only, ≤ 20 MiB, never decoded), atomic | `org.uq.settings.greeter` |
| `ClearGreeterWallpaper()` | removes it | `org.uq.settings.greeter` |
| `SetGreeterMonitors(s json)` | `/etc/uq/greeter-monitors.toml`, validated by `uq_core::settings::monitors` | `org.uq.settings.greeter` |
| `SetAutoLogin(s user)` | `[initial_session]` in `/etc/uq/greetd.toml` (empty string removes it); the user must exist with uid ≥ 1000 | `org.uq.settings.autologin` (`auth_admin`) |

The helper unit's `ReadWritePaths` grows by `/etc/uq` and `/var/lib/uq-greeter`. All other
hardening from Phase 3 §3 stays.

## 5. Welcome and the bindle hand-off

### 5.1 Welcome in `uq-settings`

- **When it opens.** uqd's state gains `welcome: {done: bool, version: u32}`. On session start
  the bridge's `Hello` event carries `welcome_pending`. If it's true and no hand-off defer is
  active (§5.2), the shell starts `uq-settings --welcome` once. Closing the window at any step
  counts as done; nothing nags.
- **Steps** (each one skippable, each change applied live through the existing uqd APIs):
  1. **Hello:** what uQ is, three keybinds to know (launcher, app menu, quick settings).
  2. **Look:** theme and variant with live preview, and the schedule.
  3. **Wallpaper:** from the theme's set or a file.
  4. **Layouts:** default layout with a short animation of each (static images with reduce
     motion).
  5. **Displays:** only with more than one monitor; arrangement and scale, with Phase 3's 15 s
     revert.
  6. **Apps:** which uQ apps are installed, using `uqctl integrations list`; for each missing one,
     a one-line description. "Install with bindle" opens bindle if it's installed; otherwise the
     page shows the `pacman`/AUR package name to copy.
  7. **Fingerprint:** only with a reader and no enrolled finger; enrols through fprintd's D-Bus
     API (`Device.EnrollStart`), so the lock screen can use it.
  8. **Done:** where to find Settings and the keybind list; a "Show Welcome again" pointer.
- **Re-open.** Settings → About → "Show Welcome", and `uqctl welcome [--reset]`.
- **Version.** `welcome.version` lets a later release add steps and show only the new ones once.

### 5.2 bindle hand-off

M0b's minimal profile installs uQ and runs `uqctl setup`, then bindle's own first-login wizard
installs the apps the user picks. Phase 5 makes the order explicit so the two windows never fight:

1. **Defer.** When bindle's first-login wizard starts in a uQ session, it creates
   `$XDG_RUNTIME_DIR/uq/welcome.defer`. While that file exists and is less than 30 min old, the
   shell doesn't start Welcome. (A stale file is ignored, so a crashed bindle can't hide Welcome
   for good.)
2. **Hand off.** When the wizard finishes, bindle removes the file and runs `uqctl welcome`,
   which opens Welcome at step 1. Welcome's Apps step then reflects what bindle just installed.
3. **No double run.** If the user already finished Welcome, `uqctl welcome` does nothing unless
   `--reset` is passed.

**bindle catalog changes** (bindle repo): `greeter = "uq-greeter"` for the `uq` entry; the
profile runs `uqctl greeter enable` as root after installing; `notes` drop the "log out and back
in" line, because the greeter now starts the session.

### 5.3 "Re-run setup" page

bindle ships a Phase 3 plugin page, `/usr/share/uq/settings/pages/bindle/`, in its own `uq/`
folder (Phase 4 §2.6):

- shows the bindle profile in use and when it last ran;
- "Pick apps again" runs bindle's app picker (a `[[command]]` with `Terminal`-style launch, since
  bindle's picker is a TUI, or bindle's GUI if installed);
- "Check for missing pieces" runs `bindle check --json` (or its equivalent) and lists the result.

Its exact commands are settled in bindle's plan; uQ's side is only the page host from Phase 3
and 4.

## 6. Error handling

| Failure | Behaviour |
|---|---|
| Greeter QML crashes | greetd restarts it; after 3 failures in 60 s, `agreety` on the same VT |
| Greeter theme or wallpaper file missing or unreadable | Default theme, dark; `bg` colour instead of the wallpaper |
| AccountsService missing | Username field instead of the list |
| PAM error, faillock lockout, expired password | Shown as plain text; the field clears; expired passwords go through the new-password form |
| Session command fails at once | greetd returns to the greeter; the greeter shows "The session ended straight away" and keeps the user selected |
| `uqctl setup` fails in `uq-session` | Logged to `session.log`; the session starts anyway; Welcome's first step shows a "setup reported a problem" card with the log path |
| `/etc/uq/greetd.toml` missing | The greetd drop-in doesn't apply; greetd uses its stock config |
| bindle crashes mid-wizard | The defer file goes stale after 30 min, and Welcome opens on the next login |
| Helper missing | The Login screen page is read-only with a note, as Phase 3 does for Power |

## 7. Security

- The greeter runs unprivileged as `greeter`, reads only the paths in §2.2, and never reads a home
  directory. Everything shown from AccountsService and session files goes through plain-text
  labels.
- PAM stays greetd's. uQ ships no PAM file for the greeter and doesn't edit greetd's.
- The helper's new methods (§4.2) write fixed paths from fixed templates; the wallpaper is copied
  byte-for-byte from a passed fd with `O_NOFOLLOW` checks and a size cap, and never parsed as
  root. Auto-login uses its own non-cached polkit action.
- `uq-session` runs as the user and only runs `uqctl setup`, which already never overwrites user
  files.
- The M4 security pass covers the greeter's auth flow, the new helper methods (with fuzzing of
  their D-Bus inputs, as Phase 3 does) and the greetd config template.

## 8. Testing

- **Greeter, offscreen QML:** user list, username fallback, session picker, prompt handling
  against a fake `Greetd` (password, echo-on prompt, info message, faillock text, expired password
  with a mismatch, cancel), power confirmation, multi-monitor layout.
- **Shared auth module:** the lock screen's existing tests must pass unchanged after the move.
- **Rendered themes:** the pre-rendered `tokens.json` files equal uqd's output for every variant
  (golden test).
- **Helper:** each new method's closed inputs, template output, fd checks (symlink, directory,
  oversize, FIFO), and polkit checks against a mock authority.
- **`uq-session`:** a shell test with a temporary `$HOME`: first run sets up, second run doesn't,
  a failing setup still execs the session command.
- **Welcome:** Rust unit tests for the step model (skips, which steps show for one or several
  monitors, with or without a reader); offscreen QML tests for navigation and the defer file.
- **Gate test (VM, manual until M0's VM runner exists):**
  1. Fresh Arch, `pacman -S uq greetd`, `uqctl greeter enable`, reboot → greeter → uQ → Welcome.
  2. Fresh Arch through archinstall + bindle's uQ profile, reboot → greeter → uQ → bindle's app
     picker → Welcome.
  3. Wrong password, faillock lockout, expired password, auto-login on and off, a broken greeter
     (rename a QML file) falling back to `agreety`.

## 9. Decisions (confirmed 2026-11-10)

The user approved the spec with each recommended option below.

1. **The greeter runs on Hyprland** with a fixed config, not on `cage`. It's already installed and
   pinned, and handles multi-monitor the same way as the session.
2. **No fingerprint at the greeter by default** (§2.3). greetd's single PAM conversation can't
   offer password and fingerprint at once, and a fingerprint login can't unlock the keyring.
3. **uQ uses its own greetd config** (`/etc/uq/greetd.toml` via a service drop-in) instead of
   editing `/etc/greetd/config.toml`, so uQ never rewrites another package's file.
4. **Pre-rendered theme files** for the greeter, plus a copied wallpaper in `/var/lib`, so the
   greeter never reads a home directory and root never decodes an image.
5. **A `uq` session entry with `uq-session`** that runs `uqctl setup` on first login. This is what
   makes `pacman -S uq` alone give a themed desktop.
6. **The hand-off uses a defer file** that bindle creates and removes, with a 30 min staleness
   limit, rather than uQ watching bindle's units.
7. **`pacman -S uq` doesn't switch the display manager by itself.** `uqctl greeter enable` (or
   bindle) does.

## 10. Slices (one plan each)

1. **Session entry and shared auth:** `uq.desktop`, `uq-session`, moving the shared auth UI into
   `shell/auth/` with the lock screen's tests unchanged, and pre-rendered theme files.
2. **Greeter:** `greeter/`, the greeter session script and Hyprland config, greetd config and
   drop-in, `uqctl greeter enable|disable`, crash fallback, packaging.
3. **Login screen settings:** the helper methods, polkit actions, the Login screen page and
   auto-login on the Users page.
4. **Welcome:** the `welcome` state, the shell trigger, the Welcome steps, `uqctl welcome`.
5. **bindle hand-off:** the defer file in uQ, bindle's catalog and wizard changes, the bindle
   "Re-run setup" page (in bindle's repo), and the gate test in a VM. Tag `v0.5.0`.

## 11. Packaging

- `greetd` becomes an `optdepends` of `uq` (for the greeter), not a hard dependency, so uQ still
  works under another display manager.
- New files: `/usr/share/wayland-sessions/uq.desktop`, `/usr/bin/uq-session`,
  `/usr/lib/uq/uq-greeter-session`, `/usr/share/uq/greeter/`, `/usr/share/uq/themes/rendered/`,
  `/etc/uq/greetd.toml` and `/etc/uq/greeter.toml` (both in `backup=()`), the greetd drop-in, a
  `tmpfiles.d` entry for `/var/lib/uq-greeter` and `/var/cache/uq-greeter`, and the two new polkit
  actions in `org.uq.settings.policy`.
