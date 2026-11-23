# Problems, logs and crash reports

How uQ says what went wrong, so a problem can be found, explained and reported without reading
source code. Settings › Problems shows it to people; `uqctl log`, `uqctl explain`, `uqctl crashes`
and `uqctl report` show it in a terminal. Apps join in with [`native/uq-diag`](../native/uq-diag).

## Codes

Every warning and error uQ logs starts with a code in brackets: an area, a dash and three digits.

```
<4>uqd: [HYPR-003] getoption general:gaps_in: timed out after 2s
  WARN qml: uq[LAUNCH-001] launching org.example.Gone failed: no such app
```

The codes are listed once, in [`crates/uq-core/src/diag/codes.toml`](../crates/uq-core/src/diag/codes.toml),
each with a severity, a title, what it means and what to do, in plain words. `uqctl explain CODE`
prints one, `uqctl explain` lists them all.

| Area | Who logs it |
|---|---|
| `STATE` | saved settings, profiles, history and restore points |
| `THEME` | themes and the look files uQ writes for other apps |
| `HYPR` | Hyprland's config and socket |
| `DISPLAY` | screens and workspaces |
| `START` | startup apps, launcher files, environment |
| `INTEG` | other apps' window rules |
| `WATCH`, `DBUS`, `NOTIF` | uqd's file watches, change signals and notifications |
| `CLIP` | uqlip, the clipboard history |
| `HELPER` | the root helper behind Settings |
| `SHELL`, `WIN`, `BAR`, `LAUNCH`, `POWER`, `LOCK`, `QUICK`, `WIDGET` | the shell |
| `SETTINGS` | the Settings app |
| `CRASH` | crashes and crash reports |

Severity: **error** (something asked for, or a part of uQ, isn't working), **warn** (uQ worked
around it; it may need a look), **info** (worth knowing, not a fault). The severity is the code's,
whatever level the line was logged at, so the shell's lines (which Quickshell writes at one
level) sort like uqd's.

### Rules

- Rust: `tracing::warn!("[AREA-NNN] …")` and `tracing::error!("[AREA-NNN] …")`, code first.
- QML and JS: `console.warn("uq[AREA-NNN] …")` (Settings: `"uq-settings[AREA-NNN] …"`).
  `console.info` and `console.debug` need no code.
- A new kind of problem gets a new code in `codes.toml`, in its area's block, with the next number.
  Reuse a code only for the same problem seen from another place.
- A code never changes meaning once released. When nothing logs it any more, set `retired = true`;
  don't delete it or reuse its number.
- `cargo test -p uq-core --test diag_codes` fails on a warning or error without a known code and
  on a code nothing uses.

Apps use their own prefix (`BINDLE-003`, `REELO-010`) and install their list as
`/usr/share/uq/diag/<app>.toml` (kept in the app's repo as `uq/diag/<app>.toml`, which
`uqctl integrations check` checks); see [`native/uq-diag/README.md`](../native/uq-diag/README.md).
An app that uQ started logs into uQ's unit (usually the shell's); its coded lines are put down
to the app by name. An app run from a terminal or its own window logs outside any unit it lists:
its file's `identifiers` (syslog identifiers, as `systemd-cat -t bindle` or `logger -t bindle`
write) are read too, in a second query, so those problems reach `uqctl log` and Settings ›
Problems as well.

## Where logs go

Everything goes to the systemd user journal, one unit per part:

| Unit | What |
|---|---|
| `uqd.service` | the session daemon |
| `uq-shell.service` | the shell (Quickshell) |
| `uq-lock.service` | the lock screen |
| `uqlip.service` | the clipboard history |
| `uq-settings-helper.service` (system) | the root helper |

uQ's Rust programs (and apps using uq-diag) write `<p>program: message` under systemd: the
`<p>` is the syslog priority, which journald stores as the entry's `PRIORITY`. In a terminal they
write `WARN program: message`. `RUST_LOG` changes what is logged as before.

```sh
uqctl log                      # warnings and errors since this boot, with codes
uqctl log --since 2h --min info
uqctl log --only HYPR          # one area (or --only HYPR-003 for one code)
uqctl log --coded              # leave out Qt's and other libraries' lines
uqctl log --summary            # one line per problem: how often, first and last time, what to do
uqctl log --json
```

## Crashes

Two kinds are caught:

- **Quickshell crashes**: the shell, the lock screen, the login screen and other apps' Quickshell
  parts (noti-fi's notification daemon). Quickshell writes its own report to
  `~/.cache/quickshell/crashes/<run id>/` (`report.txt`, `log.qslog.log`); uQ reads its
  `Config Path` to tell which part it was.
- **Program crashes**: a panic in uqd, uqlip, uqctl, the settings helper or Settings, and any
  app built with uq-diag (a Rust panic or a C++ crash signal). They leave `crash.json` in
  `~/.local/state/uq/crashes/<program>-<time>-<pid>/`.

uqd watches both folders. For each crash it hasn't seen it writes `report.md` into
`~/.local/state/uq/crashes/<id>/` (that file existing means "seen"), logs the crash under its code
(`CRASH-001` shell, `CRASH-002` lock or login screen, `CRASH-003` another app's Quickshell part,
`CRASH-004` a program), and, if the crash is less than six hours old, shows a notification that
points to Settings › Problems. Several crashes at once give two notifications at most.

```sh
uqctl crashes                       # newest first: code, id, what crashed, why and where
uqctl report --crash latest         # the newest crash's report; prints its path
uqctl report                        # a report on now, for a problem that isn't a crash
uqctl report --stdout | wl-copy     # straight to the clipboard
uqctl crashes dismiss <id>          # hide it from Settings › Problems (files stay)
```

### The report

One Markdown file, made to be pasted into an issue or handed to someone helping:

- what crashed, the signal or panic message, and the first telling frames of the stack;
- the versions of uQ, Hyprland, Quickshell, Qt, Mesa or NVIDIA, Bindle, noti-fi and kyko, the
  kernel and the distribution, and the GPUs;
- the session: the screens, the theme and variant, the active profile, how much of `user.lua`
  is your own, and which apps add Settings pages or window rules;
- the warnings and errors logged in the 15 minutes before (grouped, with codes);
- the last ten setting changes;
- the last 150 lines the crashed part logged (Quickshell's own log, decoded with `qs log`), its
  backtrace, Quickshell's report, and uQ's journal around the crash.

**Taken out before it is saved:** window titles (in Hyprland's events and Quickshell's lines), the
home folder (shown as `~`), the user and computer names, e-mail addresses, IP and MAC addresses.
App classes, workspace numbers and uQ's own paths stay. `uqctl report --no-redact` keeps
everything, for your own debugging; it is never the saved crash report.

## Settings › Problems

Under System. Crashes first, each with **Copy report** (needs `wl-clipboard`; without it the error
says where the report was saved), **Open** and **Dismiss**, and a **Copy a report** button for
problems that aren't crashes. Then one card per warning or error in the last hour, day or week:
its title, code, which part logged it, how often and when, what it means and what to do, and the
logged line itself. Lines without a code (Qt's, other libraries') are counted, not listed.

## Code map

| | |
|---|---|
| `crates/uq-core/src/diag/` | the registry (`mod.rs`, `codes.toml`), journal reading, crash finding, the report and redaction |
| `native/uq-diag/uq_diag.rs` | the log format and panic hook; uq-core and uqlip use it in place, apps vendor it |
| `native/uq-diag/uqdiag.{h,cpp}` | the same for Qt 6 C++ apps, with a crash signal handler |
| `crates/uqd/src/crashes.rs` | the crash watch, reports and notifications |
| `crates/uqctl/src/diag_cmd.rs` | `uqctl log`, `explain`, `crashes`, `report` |
| `apps/uq-settings/src/pages/problems.rs`, `qml/pages/ProblemsPage.qml` | the Problems page |
| `crates/uq-core/tests/diag_codes.rs` | every warning has a known code; every code is used |
