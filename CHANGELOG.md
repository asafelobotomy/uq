# Changelog

All notable changes to uQ are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and uQ uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Release candidates are tagged
`vX.Y.Z-rc.N`; their package version is `X.Y.ZrcN`.

## [Unreleased]

## [0.3.43] - 2026-11-26

### Changed

- **The lock screen now looks like the login screen.** It shows the login wallpaper (or uQ's backdrop in your theme's colours when none is set), the clock at the top and the avatar, name and password field on the same card as the login screen. The background and card are shared components (`shell/auth/AuthBackdrop.qml`, `AuthCard.qml`), so the two can't drift apart again.

## [0.3.42] - 2026-11-26

### Added

- **Settings › Restore: every way back in one place.** Reset every uQ setting to its default, save the settings to a file and bring a file back, and restore the newest restore points. With Bindle 0.1.47 or later it also runs Bindle's Rebase and Factory Restore and keeps its installer on the computer, each explaining what stays and what goes and showing Bindle's output as it works; with snapper, it lists, takes and deletes snapshots, rolls back to one, and sets how many automatic ones are kept. Tabs Bindle or snapper can't serve on this computer are hidden.
- **`uqctl reset`** puts every setting back to uQ's defaults, and **`uqctl backup save|restore FILE`** saves the settings to a file and brings them back. Each saves a restore point first (`auto:before_reset`, `auto:before_backup`), so it can be undone; launch counts and Welcome stay as they are. uqd has `ResetAll`, `ExportSettings` and `ImportSettings` for them.

## [0.3.41] - 2026-11-25

### Fixed

- **A window can be dropped on the left half of the screen too.** A tab dragged out of the Open windows island onto a page with one window (Full, or alone on half the screen) used to put its window on the right whatever half it was dropped on. It now takes the half it is dropped on, and the window already there slides over to the other half.
- **Drops work on a pair whose sides aren't equal.** After the gap between two windows was dragged with the resize grip, which side a drop landed on was still worked out from the screen's middle, so the preview and the drop could replace the window the pointer wasn't over. The side now follows the gap between the two windows, for tabs and for windows moved with Super+drag, and both sides are made Half after the drop. A pair swapped with Super+drag used to come back with one side at Hyprland's default width and a sliver of the next window showing; it now ends up as two Halves. A window alone at another width (left so by the grip) is made Half too when another joins it.

## [0.3.40] - 2026-11-24

### Fixed

- **An update of uq no longer leaves Hyprland in emergency mode.** When Hyprland reloaded while the package was rewriting `/usr/share/uq/hypr/base.lua`, it could read the file half-written, fail its `require`, and keep only its three emergency shortcuts until the next reload. uqd already reloaded again when such a file was *missing*; it now does the same for a file that failed to load (Hyprland's `require("…"): …` errors), and waits until the files have stopped changing and no pacman transaction is running. Each version of a file gets one such reload at most, so a file that really is broken can't make a reload loop (HYPR-005).

## [0.3.39] - 2026-11-23

### Changed

- **The terminal chosen in Settings › Default apps is everyone's terminal.** It is now written to `~/.config/kdeglobals` (`TerminalService` and `TerminalApplication`, which Dolphin's **Open Terminal Here** and other KDE apps read) and put first in `~/.config/xdg-terminals.list` (which xdg-terminal-exec reads; GLib, and so GTK apps, start terminals through it), replacing the terminal named there. Before, those apps only got kyko or kitty, and only when nothing was named. Going back to uQ's default takes the old choice out again and leaves a terminal set elsewhere alone.

## [0.3.38] - 2026-11-23

### Added

- **F12 shows and hides kyko's drop-down terminal.** A new default shortcut, *Show or hide the drop-down terminal*, in Settings › Shortcuts › Apps and desktop, runs `kyko toggle` (kyko 1.1 and later leave the key to uQ instead of binding it again). Where it comes from, its size and how much the screen behind dims are on kyko's own page, Settings › Apps › kyko.

### Changed

- **kyko's drop-down moves once.** Hyprland's own layer fade is off for kyko's surface (the `uq-base:shell-popups` rule), since kyko slides and dims at uQ's motion speeds itself.

## [0.3.37] - 2026-11-23

### Added

- **Problems from apps run outside their services.** An app's diag list
  (`/usr/share/uq/diag/<app>.toml`) can name syslog `identifiers`. `uqctl log`, crash reports
  and Settings › Problems then also read lines logged under those names, wherever they came
  from (a terminal, the app's window), not only from the app's listed units. Bindle uses it so a
  bindleUP stage that fails shows up on the Problems page (`BINDLE-004`). It merged after the 0.3.36 tag, so it ships here.

### Fixed

- **`uqctl integrations check` accepts `uq/native.toml`.** The file that pins the uQ kits an app vendors (`native/check-uq-native.sh`) sits in the app's `uq/` folder, and the check reported it as not belonging there, failing the app's CI.
- **Super+Return opens the terminal chosen in Settings › Default apps.** The shortcut always opened kyko (or kitty), whatever was chosen there; the desktop menu and look=see>do_ already followed the choice. It now runs `uqctl launch --terminal`, which starts the chosen terminal the way the launcher starts apps, and falls back to kyko, then kitty, when uqctl can't.
- **Flatpak apps, and apps under `/usr/local`, can be chosen in Settings.** Default apps, Startup's **Add an app**, Apps and scaling, Keyboard shortcuts, Profiles and the Shell page read only `~/.local/share/applications` and `/usr/share/applications`, so a Flatpak browser could not be made the default web browser. They now read every `$XDG_DATA_DIRS` folder, as uqd and the launcher do.
- **Default apps gives an app only the file types it opens.** Choosing a picture viewer made it the default for GIF, WebP, TIFF and SVG too even when it can't open them, so those files opened in an app that failed; the types it doesn't list keep the app they had.
- **Settings' sidebar no longer shows a category heading twice** when it opens straight on a page further down the list (`uqctl settings --page default-apps`, a search result, a notification).
- **Startup: Run now** no longer says "Started 90 s after login" for an app started by hand, and is greyed out while an app still waits for its delay (it starts by itself; pressing it gave an error).

## [0.3.36] - 2026-11-22

### Added

- **`native/uq-consts` (1.0.0): one source for the values uQ shares with Bindle and its apps.** `consts.toml` holds uQ's paths, the folders packages install integrations into, uQ's D-Bus names, the `uq_api`, motion, corner and type-scale values and the Graphite fallback palette; `gen.py` writes it out for Rust, C++, QML, Python, JS and shell. uQ compiles the same Rust file, and a test holds the shell's `Motion.qml` and `Theme.qml`, the Graphite theme and uqd's paths to it.
- **`UqThemeBase.qml`**, part of uq-consts: a whole QML `Theme` singleton for native apps (colours with fallbacks, `radiusControl`, `radiusInner`, one type scale, motion), so uQ's apps stop keeping their own copies.
- **`native/check-uq-native.sh`**: one CI check for every kit an app vendors, listed in its `uq/native.toml`, with `--sync` to update them from a uQ checkout. It replaces the per-kit scripts and `check-diag-codes.sh`.
- **uq-theme 1.1.0**: `tokens.json`'s new `motion`, `uqkdetheme.*` (the KDE colour-scheme follower noti-fi and hauler each had a copy of) and `uq_theme.py` for Python apps.

### Changed

- **look=see>do_'s docked terminal takes tabs in kyko too.** With kyko as the terminal, the next `>` command opens a new tab in the docked window (through `kyko @ --to unix:… launch --type tab`), as it already did with kitty, instead of a new docked window. This needs kyko 1.0.0, which added `--listen-on`.
- **`tokens.json` carries Reduce motion.** It gains `"motion": {"fast", "base", "slow", "reduce"}`, all 0 while **Reduce motion** is on, so uQ's native apps can follow it. Its keys are now written in alphabetical order.
- The companion-pages check now covers kyko and uQbit too.

## [0.3.35] - 2026-11-22

### Added

- **Settings › Problems: what went wrong, in plain words.** A new page under System lists every crash of the shell, the lock screen, uQ's programs and apps built on uQ, each with **Copy report**, **Open** and **Dismiss**, and one card per warning or error logged in the last hour, day or week: what it means, what to do, which part logged it and how often. **Copy a report** makes one for a problem that isn't a crash.
- **Every warning and error has a code.** uQ's 179 warnings and errors (uqd, uqlip, the settings helper, the shell and Settings) now start with a code such as `[HYPR-003]`, from one list of 64 codes in 25 areas with a title, what it means and what to do (`crates/uq-core/src/diag/codes.toml`). A test fails on a warning without a known code. `uqctl explain HYPR-003` prints one; `uqctl explain` lists them.
- **`uqctl log`** reads uQ's warnings and errors from the journal with their codes: `--since 2h`, `--min info`, `--only HYPR` (an area or one code), `--coded` (leave out Qt's lines), `--summary` (one line per problem with how often and what to do) and `--json`.
- **Crash reports.** uqd watches Quickshell's crash folder and uQ's own. Each new crash gets a report (`~/.local/state/uq/crashes/<id>/report.md`), a `CRASH-00N` line in the journal and, when it is recent, a notification. The report has what crashed and where, the versions (uQ, Hyprland, Quickshell, Qt, Mesa or NVIDIA, Bindle, noti-fi, kyko, kernel), the screens, the theme and profile, the problems logged in the 15 minutes before, the last setting changes and the crashed part's last 150 log lines. Window titles, the home folder, user and computer names, and e-mail, IP and MAC addresses are taken out. `uqctl crashes` lists them; `uqctl report --crash latest` (or `uqctl report` for now) saves one and prints where.
- **A panic in uqd, uqlip, uqctl, the settings helper or Settings** now leaves a crash record that shows on the Problems page, instead of only a line in the journal.
- **`native/uq-diag`** (1.0.1): the same log format and crash records for other apps, in Rust (`uq_diag.rs`) and Qt 6 C++ (`uqdiag.h`, `uqdiag.cpp`, with a crash signal handler), with a drift check like uq-theme's. Apps list their own codes in `/usr/share/uq/diag/<app>.toml` and appear in `uqctl log`, `uqctl explain` and the Problems page. See `docs/diagnostics.md`. `uqctl integrations check` checks an app's `uq/diag/<app>.toml`. Bindle, noti-fi, uQview, reelo and uQbit use it.

### Changed

- uQ's programs log one way: under systemd each line starts with its priority, so the journal files warnings as warnings (they were all "info" before), with no colour codes or second timestamp. uqlip now logs its warnings by default (it logged only errors).

## [0.3.34] - 2026-11-22

### Fixed

- **Theme icons no longer load off the GUI thread.** The app menu's icons and a notification's picture were decoded asynchronously, and when the source was a theme icon (`image://icon/...`) that ran Quickshell's icon provider, and so `QIcon`, on Qt's image reader thread, racing the GUI thread's own icon loads. That race is what crashed noti-fi's notifd (segfault or abort in `QIcon::pixmap`); uQ's shell had the same pattern. Files and notification image data still decode in the background; theme icons load on the GUI thread (`L.asyncImageOk`).

### Security

- `url` 2.5.9 (RUSTSEC-2026-0592, a stack overflow in `Url::origin()` on deeply nested `blob:` URLs; used by `uqctl` through reqwest).

- **The edge strip covers the sliver after a window is made Full.** When a new window opened Full beside another, the few pixels of the window it pushed off the screen could stay uncovered: Hyprland moves that window a few hundred ms after it reports the Full one, with no event for it. The screen pages now read the windows again twice in the 700 ms after such a change.

## [0.3.33] - 2026-11-22

### Changed

- **Every shell popup comes and goes the same way.** Quick settings, the app menu, the power menu, the island popups, the desktop menu, the clipboard history, the notification centre, the polkit prompt, the volume and brightness OSD and new notifications now share one card (`shell/core/UqCard.qml`): it fades in and grows slightly from the side it opens from in 140 ms, and fades out in 100 ms. Before, three of them faded in 140 ms in QML while the rest appeared at once and got Hyprland's 300 ms layer fade on top, so popups opened at different speeds. Hyprland's layer animation is now off for those surfaces (`base.lua`), the bar and wallpaper keep it, and all of it is instant under Reduce motion.
- **Hover colours fade instead of snapping** on every row, button, tab, chip and switch in the shell and the greeter (100 ms). A quick settings switch's knob now slides across, and the OSD's level bar glides to each new level.
- **One set of corners.** Rows, buttons and fields inside a card use the theme's radius minus 2 (`Theme.radiusControl`), chips inside them minus 4 (`Theme.radiusInner`), never below 0, so a square theme is square everywhere. Notification buttons and the notification centre's buttons were 2 px rounder than every other button.
- **Keyboard focus is the accent colour everywhere.** Text fields (the app menu's search, the clipboard search, the polkit password) showed focus in the text colour while rows, tabs and sliders used the accent.
- The power menu, the polkit prompt and the clipboard history use the same surface colour as every other popup.

## [0.3.32] - 2026-11-22

### Added

- **UK or US dates in the bar.** Settings › Date and time › Clock in the bar has a Date row beside the 12/24-hour Time row: UK ("Sat 21 Nov", weeks from Monday) or US ("Sat, Nov 21", weeks from Sunday). The clock island, its accessible name and the calendar's heading and first weekday follow it (`shell.date_format`).
- **Celsius or Fahrenheit.** Settings › Region › Temperatures sets the unit uQ shows temperatures in: the weather island and its forecast, and the system monitor's processor and graphics card readings (`shell.temp_unit`). The system monitor still warns at 90 °C.
- **The VPN is a lock with details.** While a VPN or Tailscale exit node is up, the status island shows a lock beside the network icon (the icon theme's VPN icon, as the other status icons) in place of the VPN's name and country. A click opens a dropdown with its name, type (WireGuard, OpenVPN over UDP or TCP, OpenConnect…), server, interface and VPN address, and where the internet sees you: city, region and country, public address and provider. With uQbit installed and set to use only the VPN, it also says "Torrents: only through tun0" (or that they are stopped while the VPN is down). The server, place and address are left out while the screen is shared. The public IP lookup (Settings › Shell › Privacy) now keeps ipinfo's address, city, region and provider for this; coordinates and the rest are still dropped. `scripts/barinfo-live.sh` checks it all on Hyprland in CI, with screenshots.
- **Theme other apps (Settings › Appearance › Other apps, on by default).** Every Qt, KDE, GTK, libadwaita and Flatpak app now stays on uQ's colours, fonts, icons, widget style and corners, even after another app changed their settings. Until now uQ only created those settings files and left any file that already existed alone, so pressing OK once in the qt6ct app (Fusion, its own "darker" colours, DejaVu fonts) or in KDE's colour settings took Qt apps such as qBittorrent off uQ's look for good. At every login and theme change uqd now puts its own keys back into `qt6ct.conf`, `kdeglobals`, Kvantum's config and GTK's `gtk.css`, keeps everything else in them, and keeps a copy of each file as it was (`<file>.before-uq`, later ones in `.before-uq.last`). Switch it off to leave those files alone.
- **GTK and libadwaita apps take the theme's corner radius.** Windows, popovers, cards and boxed lists use the theme's radius and buttons, entries and rows two pixels less, as uQ's own apps do; linked button groups stay square inside. The rules are generated from libadwaita's and adw-gtk3's own stylesheets (`scripts/gen-gtk-corners.py`).
- **Flatpak Qt apps use the KDE platform theme**, so apps on the KDE runtimes read uQ's colours, fonts and icons from `kdeglobals` like Qt apps outside Flatpak (a `QT_QPA_PLATFORMTHEME` you set in the global override is kept).

### Changed

- **kyko is uQ's terminal.** SUPER+Return, terminal apps launched from the menu, the desktop's "Open terminal" and the KDE terminal setting now use [kyko](https://github.com/asafelobotomy/kyko) (images, ligatures, the kitty keyboard protocol, splits and sessions that survive a restart). kitty is still used where kyko is not installed, and a terminal you chose in Settings is kept.
- **The calendar under the clock frames today** in the accent colour, instead of only making it bold.
- **Settings pages open at once when you go back to them.** The eight pages shown most recently stay open in the background, so switching back to one shows what it last read straight away and reads again behind it, replacing the values when the answers come, instead of an empty page that loads from scratch each time. Apps' pages (Apps in the sidebar) get the same: a page can declare `refresh()` to read again when it is shown, and `keepAlive: false` to be closed as before (`docs/settings-pages.md`). Bluetooth and Network still close when you leave them, since they answer pairing and password questions while open. A page whose app is uninstalled is closed.
- **The corner size on the Look page is every app's corner size.** It used to round only Hyprland's windows; the shell, uQ's apps, Qt (Kvantum) and GTK apps now draw with the same radius.

### Fixed

- **The strip over a sliver of the next page's window shows at once.** After a window opened, scrolled the screen or changed a column's width, the few pixels of the window beside it at the screen's edge could stay uncovered until the next window event, since uQ worked the strips out before the move. The pages are read again when such a move finishes.

## [0.3.31] - 2026-11-21

### Fixed

- **An app's dialogs share its tab.** Preferences, "Save changes?" and other windows an app opens from one of its windows had a tab of their own in the Open windows island, and while a modal dialog was open the app's own tab did nothing when chosen. Now the dialog sits in the app's tab, the tab shows as focused while either has focus, and choosing it brings the app into view with the dialog in front and focused. Minimising or moving the app's window takes its dialogs along, and restoring it brings them back.
- **Super+drag goes where the preview shows.** Dragging a window to the other half of a shared screen could drop it into the column of a window that slid under the pointer, stacked under it, instead of on the half the screen showed. It now lands on that half.
- **No resize grip at the screen's edge beside a window on another page.** Dragging it made the window on screen shrink from its far side instead of moving its edge.
- **A minimised window's tab greys at once.** It used to drop out of the Open windows island for about 200 ms and come back greyed.
- **No sliver of the next window at the screen's edges.** The gap at a screen's edge is wider than the gap between columns, so 5 px of the window on the page beside the one shown peeked in. A strip of wallpaper now covers it while it's there (not where a floating window reaches the edge, and not over the scratchpad).

### Added

- `scripts/windows-live.sh`, a window handling run on the real shell and Hyprland, with a CI job of its own (`windows`): opening, Half and Full, tabs dragged along the bar and onto the screen, the resize grip, Super+drag, minimise and restore, floating, fullscreen, workspaces, the scratchpad, an app's dialog and closing, each checked and timed (latency, the shell's frames and CPU). The timings land in the job summary.

## [0.3.30] - 2026-11-21

### Fixed

- **The app menu's search box takes your typing when you open it from its bar button.** Opened from the button, the menu's click-outside grab listed both the menu and the bar, and Hyprland hands the keyboard to whichever window comes first on a new grab's list, in no set order. When that was the bar, which takes no keyboard, typing went nowhere until the pointer moved onto the menu. The bar now joins the grab a moment later, so the menu always gets the keyboard. Quick settings had the same problem from its Status button. `scripts/dialog-focus-live.sh` now opens each popup and types straight away, from IPC, from a tap of Super and from a real click on the app-menu button.

## [0.3.29] - 2026-11-21

### Added

- **`native/uq-theme`, one theme-follow module for native apps** (uq-theme 1.0.0). Qt 6 C++ (`uqtheme.h`/`.cpp`) and Rust (`uq_theme.rs`) copies that read and check `tokens.json` and follow it with a file watch, including uqd's swap of the whole `theme/` folder, with a drift check for CI. Bindle, bindaru, uQview, appOFholding, reelo and XZram use it in place of their own copies, and stop checking the file once a second.

### Fixed

- **Shut down in… shows the time in your clock format.** With the 12-hour clock chosen in Settings › Date and time, quick settings said "at 23:05"; it now says "at 11:05 PM".
- uQ's weekly check of the companion Settings pages now covers uQview's page too (13 pages).

## [0.3.28] - 2026-11-20

### Added

- **Shut down in… on quick settings' System tab.** Pick 15 min, 30 min, 1 hour or 2 hours, or Custom to type a time (`45`, `90m`, `1h30`, `1:30`; up to 24 hours), and uQ shuts the computer down then, the same way the power menu's Shut down does. While it's set, the tab shows a live countdown, the clock time it will happen and Cancel. Notifications warn 5 minutes and 1 minute before, each with a Cancel shutdown button. It still goes ahead with the screen locked, survives the shell restarting, and is dropped (with a notification) instead of firing if the computer was asleep when it came due. The package now depends on libnotify for `notify-send`.

## [0.3.27] - 2026-11-20

### Changed

- **look=see>do_'s terminal stays docked under it.** A `>` command opens the terminal beneath the pill at the drop-down's size and keeps it there: clicking it to type in it no longer closes look=see>do_ or turns it into a loose floating window, and the terminal no longer opens tiled or full screen when its window rules don't take. The next `>` command opens a new tab in the same terminal (kitty, over a remote-control socket only that window listens on; other terminals get a new docked window and the old one is minimised to the Open windows island). When look=see>do_ closes (Esc, Super+Space, or focus going to another window) the terminal is tucked away, and it grows back in under the pill the next time look=see>do_ opens.

## [0.3.26] - 2026-11-20

### Added

- **Mark up and Open in the screenshot preview.** With [uQview](https://github.com/asafelobotomy/qview) installed, the preview has a Mark up button that opens the capture straight in uQview's Mark up (arrows, boxes, text, numbers, highlighter, pixelate, crop) and an Open button. Marking up a screenshot had gone with satty in 0.3.9. The caption now says which folder the capture went to next to its name.
- **`uqctl wallpaper set -- PICTURE`** uses a picture as the wallpaper on every display (or one, with `--output`), optionally with `--fit fill|fit|center|tile|span`. It stops a folder slideshow there, shows in Settings, and `uqctl undo` takes it back. `uqctl wallpaper get` lists each display's wallpaper. uQview's Set as wallpaper uses it.

## [0.3.25] - 2026-11-20

### Added

- **A Printers page in Settings.** It shows every printer, which one is the default, what each is doing (ready, printing, paused, out of paper, can't be reached…) and the documents waiting, live. You can make a printer the default, pause and resume it, print a test page, cancel one document or all of them, change its name, location and printing defaults (paper size, two-sided, color, quality, paper type and tray, and the rest of its options), and remove it. **Add a printer** looks for printers on the network and USB ports and adds them without a driver when they can (most printers since about 2010), or with a matching installed driver; a network printer that isn't found can be added by its address. It talks to CUPS directly over its local socket, as you: no password prompt and no terminal, and changing printers is allowed to administrators, as CUPS decides. When printing is installed but switched off, **Turn on printing** starts it. This replaces system-config-printer, the last default settings window that didn't follow the uQ theme; the printing service's own web page is linked for the rare printer that needs more. The package gains an optional dependency on cups.
- **reelo is uQ's media player.** The new [reelo](https://github.com/asafelobotomy/reelo) (libmpv in a Qt Quick window that follows the uQ theme, with MPRIS for the media widget and keys) is an optional dependency, and its **Media player** page under Settings › Apps is checked with the other companion pages. Bindle installs it and makes it the default player in place of VLC/mpv.

## [0.3.24] - 2026-11-20

### Added

- **A calculator that replaces KCalc.** look=see>do_'s `=` calculator now takes `×` `÷` `−` and
  `**`, `mod` and `7 % 3`, implied multiplication (`2pi`, `3(1+1)`), desk-calculator percentages
  (`2450 + 18%` is 2891), factorials (`5!`), degrees (`sin(30°)`), `1,234,567`, `1e-3`,
  `0xff`/`0b101`/`0o17`, a trailing `in hex`, `in bin` or `in oct`, `ans` (the last result you
  copied), and cbrt, asin, acos, atan, sinh, cosh, tanh, log2, exp, floor and ceil. Whole numbers
  up to 2^53 print in full. Bindle no longer installs KCalc on uQ.
- **The Calculator key opens it.** A new default shortcut, "Open the calculator", binds the
  keyboard's Calculator key to look=see>do_ at `=`. Change or turn it off in Settings > Keyboard
  shortcuts.

## [0.3.23] - 2026-11-20

### Fixed

- **Open KDE and Qt apps change with the theme.** uqd rewrote `kdeglobals` on a theme change but never told running apps, so Dolphin, Kate, Okular and other windows using the `kde` platform theme kept the old colours, style and icons until restarted. uqd now sends the signals Plasma sends after changing `kdeglobals` (KGlobalSettings palette and style, KIconLoader icons).

### Changed

- **Every companion page is checked.** The weekly companion check now covers all twelve apps with a uQ Settings page (oysterAV, appOFholding, hauler, bindaru, romforge, emobie and Bindle as well as the first five), and `docs/companion-pages.md` lists them.

## [0.3.22] - 2026-11-20

### Added

- **Bar widgets can hide themselves.** A widget whose root item sets `property bool hidden: true` stays loaded but takes no room in the Status island, the "…" list or quick settings, and keeps watching for the moment to come back (`docs/integrating.md`). Bindle's bindleUP icon uses it to show only when updates are ready.

### Fixed

- **Widget popups get their `ctx`.** A widget popup that declared `property var ctx` (as the template and the docs show) was never handed it, because the property starts out `undefined` and that was read as "this popup has no ctx". Its text and buttons stayed empty. The bar popups, island popups and quick-settings detail views now hand `ctx` to any popup that declares it.

## [0.3.21] - 2026-11-20

### Fixed

- **The shell's helpers stop with the shell.** `uqctl bridge`, `uqctl clip watch`, `nmcli monitor` and the notification `gdbus monitor` kept running after the shell stopped or crashed, until their next write failed, so a restarted shell could briefly run two of each. They now end as soon as the shell does. Apps opened from the shell still outlive a shell restart.
- **No error from a widget answer that arrives too late.** A widget's D-Bus answer that arrived after the shell reloaded or the widget was removed logged `TypeError: Cannot call method 'deliver' of null`. It is now dropped quietly.
- **No more "Unable to assign [undefined] to bool" from the window tabs.** A tab drawn a moment before its window's details arrived logged it, up to dozens of times a day.
- **A clean logout.** Hyprland closed before the shell was stopped, so at every logout and shutdown the shell lost its compositor and `uq-shell.service` ended as failed. The shutdown hook in `base.lua` now stops the shell and uqlip and waits for them (up to 5 s) before Hyprland closes. `scripts/shutdown-live.sh` checks it on a real Hyprland in CI.
- **uqlip no longer logs an error when the session ends.** When the compositor closed, uqlip logged `the Wayland connection ended` and `this compositor can't be watched: no data-control protocol` as errors. It now treats a hung-up connection as the session ending, exits cleanly and stays quiet. Real connection errors are still logged, once, with their actual cause.
- **The lock screen stops looking for fprintd when it isn't installed.** It retried `fprintd-list` for as long as the screen stayed locked, logging "Process failed to start" each time. It now checks once per lock and, with no fprintd, offers the password only.

## [0.3.20] - 2026-11-20

### Changed

- **Dolphin and every other KDE or Qt app now follow the uQ theme by default.** KDE apps ignored qt6ct's palette: they showed Breeze's light backgrounds, a Fusion-style look and recoloured red Breeze folders. The session now uses plasma-integration's `kde` platform theme (falling back to qt6ct if it isn't installed), and uqd writes uQ's colours, icon theme (Papirus), Kvantum widget style and fonts into `~/.config/kdeglobals` on every theme change, so apps switch live between velvet, gruvbox, forest and the rest, dark and light. uqd also creates the GTK, qt6ct and Kvantum files itself, so `uqctl setup --apps` is no longer a manual step (files you wrote are never replaced). The uQ shell keeps qt6ct. The package now depends on plasma-integration, papirus-icon-theme, adw-gtk-theme, inter-font, ttf-jetbrains-mono and xdg-desktop-portal-gtk.
- **Flatpak, Electron and Qt 5 apps follow the theme too.** When Flatpak is installed, uqd adds read-only access to uQ's theme files (GTK css, kdeglobals, Kvantum and the rendered theme) to the user's global Flatpak override, keeping anything already there. xdg-desktop-portal-gtk passes dark or light to Flatpak, Electron and browser apps through the settings portal. Qt 5 apps use the same kdeglobals look with the optional plasma5-integration and kvantum-qt5.

## [0.3.19] - 2026-11-19

### Fixed

- **Windows stay solid when you change workspace or screen.** The workspace animation faded windows out and in, so for a moment the wallpaper showed through. Windows now slide instead, and never turn transparent. `scripts/workspace-solid-live.sh` checks it on a real Hyprland and keeps a before/after GIF.

## [0.3.18] - 2026-11-19

### Fixed

- **A dialog that needs typing keeps the keyboard.** When a window took the focus while a uQ popup (app menu, quick settings, look=see>do_, power menu, clipboard history or a bar popup) was open, the popup kept the keyboard for itself even though Hyprland showed the window as focused, so text typed into a file manager's "New folder" dialog went nowhere. The popup now closes when another window takes the focus. `scripts/dialog-focus-live.sh` checks it on a real Hyprland.

## [0.3.17] - 2026-11-19

### Fixed

- **look=see>do_ finds more documents.** The file index now follows linked folders (a Documents folder that is a link to another drive was never searched), matches several words in any order (`tax return` finds `Tax_Return-2026.pdf`), and fills shallow folders first, so a full index no longer leaves out top-level files.

### Added

- **ROM and retro-platform files are recognised and have icons.** uQ installs `uq-roms.xml` (shared-mime-info) and one icon per platform, so Dolphin, other file managers and `xdg-open` show "Super Nintendo ROM", "Game Boy Advance ROM", "Wii disc image" and so on, with a cartridge, card or disc icon labelled with the platform. It covers NES/Famicom Disk System, SNES, N64, Game Boy, GBC, GBA, DS, 3DS, Switch, GameCube, Wii, Wii U, Sega (SG-1000 to Dreamcast), Sony (PBP, Vita), Xbox, Atari, PC Engine, Neo Geo, WonderSwan, MSX and a few home computers, plus `.cue` and `.chd` discs. Existing freedesktop types (`application/x-nes-rom` and the rest) are reused and only gain extensions, a plainer name and an icon; ambiguous extensions (`.bin`, `.iso`, `.rom`, `.md`) are left alone. `scripts/gen-rom-mime.py` generates the files and `tests/shell/rom_mime.sh` checks them.

## [0.3.16] - 2026-11-19

### Changed

- **The boot and shutdown banner turns itself on.** Installing or updating uQ enables `uq-banner-boot.service` and `uq-banner-shutdown.service`. Settings → Login screen → **Startup banner** switches it off (it creates `/etc/uq/banner-off`, which the units check), and updates keep it off. They still never touch the bootloader or the kernel command line.

## [0.3.15] - 2026-11-19

### Added

- **More for startup apps.** The Startup page can space the apps' starts a few seconds apart, start an app that failed at login once more, say how long after login each app started, and run or stop an app right now (`RunAutostart` and `StopAutostart` on `org.uq.Desktop1`). An app set to start hidden that has its own option for it (Steam, Discord, Telegram, Signal, Element, Slack, Nextcloud, Syncthing GTK, 1Password) is given that option. Profiles can carry Start hidden.
- **Start hidden for startup apps.** Each app on the Startup page has a minimise button: when it is on, the app starts out of the way. An app with a system tray icon is sent to the tray, and an app without one is minimised to its tab on the bar (restore it from there). The row says which happened. It only touches the windows of the app uQ started at login.

### Documentation

- **Providers outside `/usr`.** docs/integrating.md says that `Activatable=true` also works with a service file in the user's own `~/.local/share/dbus-1/services/`, for an app that can't install into `/usr`, and that `Activate` only returns (uQ launches nothing for a provider).

## [0.3.14] - 2026-11-19

### Added

- **The login screen follows your theme.** When you change theme (or its dark and light variant), uqd asks the root helper to match the login screen to it. A polkit rule (`49-uq-greeter-theme.rules`) lets an active local administrator (group `wheel`) do this one thing without a password; everything else about the login screen still asks. A theme the login screen doesn't ship, such as your own, leaves it as it was. Picking a different login theme in Settings is replaced at the next theme change.
- **A uQ backdrop.** With no wallpaper chosen, the desktop and the login screen now show uQ's own faceted backdrop with the uQ mark, drawn in the active theme's colours (dark or light), so the wallpaper changes colour with the theme. Picking a picture, folder or colour in Settings works as before.
- **A uQ boot and shutdown banner.** `uq-banner-boot.service` and `uq-banner-shutdown.service` print the block-letter uQ on the console in the login screen theme's accent. They are off until enabled: `run0 systemctl enable uq-banner-boot uq-banner-shutdown`. They never touch the bootloader or the kernel command line.

## [0.3.13] - 2026-11-18

### Changed

- **Num Lock is on by default,** on the login screen and in the desktop (and so on the lock screen). To turn it off, add `hl.config({ input = { numlock_by_default = false } })` to `user.lua`.
- **The app menu, quick settings and the power menu open without a stall.** They used to create a new window each time, and the shell waited 30 to 40 ms for that window's first frame. Their windows now stay in place, out of sight and out of the way, and the menu fades in and out instead (the shell uses a little more memory at rest).

### Security

- `uqlip.service` can only open Unix sockets and can't create namespaces or change its personality, so a flaw in what it reads from the clipboard can't reach the network.

## [0.3.12] - 2026-11-18

### Added

- The Security island shows a running oysterAV scan: its oysterAV icon takes the accent colour and pulses (steady under Reduce motion), it reads "Scanning 42%", and its popup shows a progress bar. Needs oysterAV with `scan_progress` in `status summary`; an older oysterAV shows "Scanning" without a percentage.

## [0.3.11] - 2026-11-18

### Changed

- **The shell moves at one speed.** Slides, resizes, the bar editor's scrim and the resize grips
  now share two motion tokens (`Motion.fast` 100 ms, `Motion.base` 140 ms, ease-out) instead of
  mixed 100 to 160 ms values and easings, and all of them stop under Reduce motion.
- **The Security island waits for oysterAV's first-time setup.** Until setup has run (oysterAV's
  summary says `setup_completed: false`) the island is red and reads "Set up", sends one
  notification, and a left click opens the Setup tab of oysterAV's Security page in Settings
  instead of the popup. An older oysterAV that doesn't report it is treated as set up.

## [0.3.10] - 2026-11-18

### Added

- **A clipboard history.** `uqlip` follows the Wayland clipboard (the `ext-data-control` protocol,
  or `wlr-data-control` on older compositors) and keeps a private history of text and images in
  `~/.local/share/uq/uqlip/` (owner-only files): newest first, duplicates move to the top, up to
  200 entries and 16 MiB each, pinned entries are never pruned. Copies a password manager marks
  secret (`x-kde-passwordManagerHint`), copies from ignored apps (KeePassXC, Bitwarden, 1Password,
  Proton Pass by default) and anything while paused are never read. `uqctl clip` lists, searches,
  copies back, pins, deletes, clears and pauses; `uqctl clip watch` streams changes and
  `uqctl clip config` changes the settings. **Super + Shift + V** opens the history in the shell:
  search by typing, Up and Down to choose, Enter to put an entry back on the clipboard, Ctrl+P to
  pin, Delete to forget, images shown as thumbnails. A new Clipboard history bar island shows
  the count (add it in the bar editor), and Settings has a Clipboard page for pausing, forgetting,
  the apps that are never kept and the limits. `uq-shell.service` starts the daemon.

## [0.3.9] - 2026-11-18

### Changed

- **Screenshots end in a floating preview, never a new window.** After Print, Shift+Print or
  Super+Print the capture is saved to `~/Pictures/Screenshots`, copied, and shown in a small card
  in the corner of the screen with **Copy**, **Discard** and a close button. It goes away after
  12 seconds, or stays while the pointer is on it. The satty mark-up window and the notification's
  "Open" button are gone; without the shell running, a notification says where the file is.

### Fixed

- **Print stopped working after the first screenshot.** `wl-copy` stays running to serve the
  clipboard and inherited the lock that keeps two captures from running at once, so every later
  press quietly did nothing until something else was copied. The lock is now let go before the copy
  starts. A failing area picker or grim is also reported instead of ending silently.

## [0.3.8] - 2026-11-17

### Changed

- **The Security island shows oysterAV's own icon**, tinted green, yellow or red by the security
  status (a coloured ring if the icon isn't installed), and its popup gets an "Open oysterAV" row.
  oysterAV's tray icon no longer also shows in the Tray or Status island while the bar has a
  Security island.

### Fixed

- **Qt apps no longer show Kvantum's grey bevelled buttons.** uQ's Kvantum theme named a base
  theme that Kvantum never used, so every Qt app fell back to Kvantum's built-in grey 3D style
  with a blue accent, whatever the uQ theme. uqd now also writes `Kvantum/uq/uq.svg`, flat rounded
  artwork drawn from the theme's colours (buttons, the default button, tabs, fields, check boxes,
  radios, sliders, progress bars, list selections, menus), and a full `uq.kvconfig`. OK, Cancel and
  Apply, tabs and the rest now match uQ's own windows in every theme, light or dark, and follow
  theme changes. Apps pick it up when they next start.

## [0.3.7] - 2026-11-17

### Added

- **A text style for every island** (`shell.bar.options.all.text`), set on the Bar editor's "All
  islands" page. Islands with no text of their own use it; an island's own text, even the
  theme's, still wins.
- **Edit mode opens the Bar editor.** Editing the bar (right-click an island, Edit Islands, or
  SUPER+ALT+B) now also opens a dialog with every island on the bar, every island type to add, the
  chosen island's own settings (size, text and what it shows) and an "All islands" page to set one
  size or text style for the whole bar. The screen under the bar dims while it is open, so the
  bar stays lit and shows each change. Esc closes the dialog and edit mode goes on; S on the edit
  panel opens it again.
- **A Storage cleanup island (crumBS).** Shows how much space `crumbs scan` says can be freed and
  turns amber at a size you choose (`shell.bar.options.crumbs.warn_gb`, 5 GB by default). Its popup
  lists the biggest categories and opens crumBS's Storage page. Hidden without crumBS.
- **The System monitor shows the graphics card.** Use, video memory and temperature, for NVIDIA
  (through `nvidia-smi`) and AMD (through sysfs). Options choose which readings the island shows:
  processor, memory, graphics card, temperature (`shell.bar.options.sysmon`).
- **The Security island's notifications can be switched off** (`shell.bar.options.security.notify`).

### Changed

- **Other apps follow the uQ theme.** Sessions now run `uqctl setup --apps`, so GTK, Qt and
  terminal apps get uQ's colours from the first login. When Qt app theming is on, uqd also keeps
  qt6ct's fonts and a per-theme marker in step (so running Qt apps reload their palette when only
  the colours change), and writes uQ's colour scheme into `~/.config/kdeglobals` so Kirigami and
  KDE apps (Dolphin, Kate) stop being Breeze Light. A kdeglobals that already holds another
  colour scheme is left alone. With `adw-gtk-theme` installed (new optional dependency), uqd sets
  the GTK 3 theme to `adw-gtk3` or `adw-gtk3-dark`, and `gtk-3.0/gtk.css` defines libadwaita's
  colour names, so GTK 3 apps follow the theme too.

## [0.3.6] - 2026-11-17

### Added

- **A Security island in the bar.** A green, amber or red ring shows oysterAV's security state,
  checked every two minutes. Click it to see the reasons and open oysterAV's Security page in
  Settings. It sends one notification when the state gets worse, and hides itself when oysterAV
  isn't installed. It is in the default bar of new users; add it to an existing bar with Edit,
  Add an Island.


## [0.3.5] - 2026-11-17

### Changed

- **An unfocused tab's icon no longer shows the close ×.** The × now appears only on the focused
  tab, so a stray click on a background tab switches to it instead of closing it.

## [0.3.4] - 2026-11-17

### Changed

- **Icon-only tabs keep their icon on hover.** The close × appears after the pointer rests on the
  tab for 400 ms, and a click before then goes to the tab instead of closing the window.
- **Settings cards are as tall as their rows**, not stretched to match the card beside them, so
  short cards no longer have blank space under them.
- The look=see>do_ notice after a `>` command now reads "Click the terminal to switch to it."
- **Minimised tabs are a little less faint** (70% opacity, was 50%), so they stay readable.

## [0.3.3] - 2026-11-17

### Added

- **look=see>do_ opens on Recents and five pinned squares.** Under the search pill: the apps you
  opened lately (from look=see>do_ or the app menu), then five squares for the apps you choose.
  A square opens its app (Alt+1 to 5 too); an empty square, **Edit**, or a right click lets you
  pick, change or remove the app. Both go away as soon as you type.
- **A `>` command drops your terminal down from the pill.** Enter opens the default terminal
  floating under the pill with the command running in it. look=see>do_ stays until you click the
  terminal, which focuses it and closes look=see>do_. Shift+Enter still runs the command in the
  background.

### Changed

- **Packaging: building uq skips its test suite by default.** CI runs the full suite before every
  tag, so `makepkg` and AUR helpers no longer spend about a minute on it. `UQ_CHECK=1 makepkg ...`
  runs it.
- The search field is now a pill of its own, with the results card below it.

### Fixed

- **Updating uq can no longer leave Hyprland in emergency mode when `base.lua` is missing for
  more than a second.** Hyprland reloads when a file it read goes away and not when it comes
  back, and uqd's recovery looked once, a second after the failed reload: a package that takes a
  few seconds to unpack left the file missing at that look, and Hyprland stayed on its emergency
  binds ("a lua config error resulted in no binds") until a manual reload. uqd now keeps looking
  every second for up to two minutes and reloads once the file is back.

## [0.3.2] - 2026-11-16

### Changed

- **Settings rows no longer stretch across a wide window.** On a big window a label sat at one
  edge of a card and its switch or field at the other. Every page, and every app's page built on
  the shared components, now lays its cards out in a centred block with one rule: no column is
  wider than 44 grid units (about 800 px), a second column appears once there is room for two
  (about 1400 px), and a card that spans both columns puts its plain rows two to a line.
  `SettingRow` also caps itself at that width wherever it is used.
- **Open-window tabs keep their titles until they are icons.** A tab used to drop its title as
  soon as it narrowed below Large. Now the title stays (elided) at every size but Icon, and an
  icon-sized tab shows the title in a tooltip when you hover it.
- **A minimised window's tab is an icon, greyed, at the end of the tabs.** Whatever the tab order,
  minimised windows come last, the one minimised most recently at the very end. Choosing one brings
  the window back as the last column on the workspace, focused. (Its icon no longer closes the
  window when clicked; middle click and the menu still do.)

## [0.3.1] - 2026-11-16

### Fixed

- **Terminals themed by uQ no longer print a foot error.** foot 1.28 refuses the `[colors]` section
  uQ wrote to `foot.ini` (`invalid section name: colors`), so every new foot window opened with an
  error line. uQ now writes `[colors-dark]`, which foot 1.26 and newer read. foot 1.25 and older do not
  know that name.

- **Choosing a minimised window's tab now leaves that window focused.** The window came back and
  took focus for a moment, then focus snapped back to the window you had been in: the shell's
  column policy sizes a returning window like a new one, which focuses it and hands focus back to
  whatever had it. Restoring a window now ends by focusing it (pointer kept where it is), so the
  window you chose is the one that has focus when the sizing is done.

## [0.3.0] - 2026-11-16

### Fixed

- **Moving a window with Super+drag no longer sets the rest of the desktop off.** Hyprland lifts a
  tiled window out of its layout the moment the move starts, and the shell took that for a window
  closing or turning floating: it scrolled the view, moved focus about, hid and showed pages,
  made the window left alone Full and fitted the lifted window to the screen, all while the
  pointer held it. The mouse binds now send a start and an end event (two extra `hl.bind` lines
  each in `generated.lua` and `base.lua`), and the shell waits for the end and then looks at the
  windows as they are. Hyprland also now needs the pointer to travel 8 px
  (`binds.drag_threshold`) before a click on a window becomes a move, so a plain Super+click no
  longer lifts the window and reflows the others. A Full window that is moved comes back Full
  (Hyprland un-maximizes it for the move). Checked on a live Hyprland in CI
  (`scripts/drag-live.sh`).
- **look=see>do_'s Web, Open windows, Settings and Files groups can be switched off and
  reordered.** Settings → Shell → Search providers only listed the calculator and Run, though the
  launcher already honoured a saved off switch for the others; they are listed now, and the
  Files group orders all its file types together. Open windows follows its switch too.
- **Run's row says what it does.** It read "in a terminal-less scope"; it now says "Runs in the
  background, without a terminal".
- **Print opens one area selection, with no full-screen capture first.** `uq-screenshot` used to
  grab the whole screen, then open satty over it (the screen went lighter) with its own crop
  tool, so you saw a second selector after the first. Now you pick the area with slurp, only
  that area is captured, and satty opens for marking it up. A second press while a capture is
  open does nothing, so a Print binding of your own in `user.lua` can no longer start a second
  selector beside uQ's (remove that binding, uQ's own does the job).

### Added

- **Keyboard shortcuts for windows in the scrolling layout.** Super+F makes the window Full, or
  back to Half. Super+Shift+Left/Right moves its column, and Super+Ctrl+Left/Right makes the
  column narrower or wider. They are listed under Windows in Settings → Keyboard shortcuts, so
  you can change them. Checked on a live Hyprland in CI (`scripts/drag-live.sh --scenario keys`).

- **look=see>do_ can do things: a Commands group.** Type `lock`, `suspend`, `log out`,
  `restart` or `power off` for the power actions, or `profile` / a profile's name to switch
  profiles or go back to normal. The ones that end the session are never the top hit, so a
  half-typed word can't log you out on Enter. It has its own switch under Settings → Shell →
  Search providers.
- **Shift+Enter runs a row's second action.** On a `>` row it runs the command in your chosen
  terminal and keeps the window open until you press Enter; on a provider's row it calls the
  row's second action (emobie's "type it").
- **Settings → Shell → Screenshots → Print key.** Choose what Print captures: an area (the
  default, marked up in satty), the whole screen or the focused window. Shift+Print and
  Super+Print keep taking the screen and the window.
- **App pages in Settings can look like Settings' own pages.** A page that says `uq_api = 2`
  may use `SettingsPage`, `SettingsTab`, `SettingsCard` and `HelpTip`, so it gets the same tabs,
  cards and two-column layout, and `UqAppIcon`, which shows an app's icon from the icon theme
  (or its first letter). The page test kit has stand-ins for all five, and
  `uqctl integrations check` refuses a page that uses them while saying `uq_api = 1`. Pages that
  say `uq_api = 1` work as before.
- **App pages can show progress and preview a change (`uq_api = 3`).** A command with
  `output = "ndjson"` can be run with `ctx.command.stream(id, args, onEvent, onDone)`, which
  hands the page each JSON line as it arrives (for a progress bar) instead of one answer at the
  end. A command with `confirm` can also say `preview = "<command id>"`: Settings first runs that
  read and shows what it would change, grouped, with sizes and whether each can be put back,
  before the person confirms; a plan that would be refused offers no Confirm. The page test kit's
  `FakeContext` has `stream`, and `uqctl integrations check` refuses these in a page that says
  an older `uq_api`.

### Changed

- **A window moved with Super+drag lands by the same rule as a tab dropped on the screen, and
  shows where first.** While you hold it, the screen outlines the half of the page it would take
  (the frame a dragged tab shows); let go over the left or right half and it joins that half. Before,
  Hyprland put it down by its own rule (near an edge a new column, else stacked into the column
  under the pointer), which moved other columns about and could differ from a tab drop. Moved and
  put back where it was, it now stays in its place.

- **Spotlight is now look=see>do_.** The launcher on Super + Space has a new name, and its
  names inside uQ follow: the shortcut is `uq.lookseedo`, the layer namespace `uq-lookseedo`, the
  IPC surface and the Hyprland global `uq:lookseedo`, and the four-finger gesture action
  `uq:lookseedo`. Stored shortcut overrides, custom shortcuts, profiles and gestures using the
  old names are converted when uQ loads them. Scripts calling `qs ipc ... call uq open
  spotlight` need the new name.
- **Tiled windows can't be resized from the screen's outer sides.** Super + right-drag took a
  tiled window by whichever edge was nearest, the outer ones too, moving the window off its side
  of the screen. It is now unbound by default: tiled windows resize only by the grips in the gaps
  between them (side by side, or stacked). To resize floating windows with the mouse, bind
  **Resize the window with the mouse** again in Settings → Shortcuts.
- **Dependencies updated:** zbus 6, minijinja 3, quick-xml 0.42 and toml 1.2, and the CI
  actions (checkout 7, upload-artifact 7, download-artifact 8). No change in behaviour.
- **Packaging: `uq` depends on the pin it is tested with** (`uq-hyprland-pin>=0.56.2`, from
  `_pin` in the PKGBUILD), so an updated uQ can't be installed beside an older pin that allows a
  Hyprland its config doesn't support. After an upgrade, pacman now says to log out and back in:
  uqd and the shell keep running the old version until then.

## [0.3.0-rc.8] - 2026-11-16

### Fixed

- **Dragging tabs in Open windows works in every sort.** Before, a tab could only be dragged
  into a new place while the island sorted by screen order; in any other sort it lifted and
  followed the pointer, then nothing happened. Now the window moves to where its tab was
  dropped (first or last when dropped at an end, otherwise just after the tab on its left), and
  the island switches to screen order so the tabs match the windows.

## [0.3.0-rc.7] - 2026-11-16

### Added

- **Spotlight searches your files.** Files and folders in your home folder and on drives under
  `/mnt`, `/media` and `/run/media` show grouped by type: Folders, Documents, Images, Audio,
  Video, Code, Archives and Other files. With `plocate` installed, files elsewhere on the system
  show as System files.
- **Spotlight searches inside files.** A Contents group lists files whose text holds what you
  typed, with the line that matched. With KDE's Baloo indexing, it uses Baloo, which also reads
  PDFs and office files; without it, Spotlight reads your text and code files for a moment.
- **Spotlight finds settings and open windows.** Every page and setting in Settings shows under
  Settings and opens there. Open windows show under Windows; choosing one focuses it, or brings
  a minimised one back.
- **Type a web address in Spotlight** (`example.com`, `https://…`, `localhost:3000`) to open
  it in your default browser.
- **KDE Wallet unlocks at login.** The login screen's PAM file, `/etc/pam.d/uq-greetd`, opens
  KDE Wallet with your password the way KDE's own login screen does, so Dolphin and other KDE
  apps stop asking for it. It needs `kwallet-pam` (now an optional dependency) and a wallet
  named `kdewallet` with your login password; without them, logging in works as before. If you
  changed that file yourself, pacman keeps yours and leaves a `.pacnew` to merge.

### Changed

- **Spotlight matches more carefully.** An app's description and keywords, and a setting's
  label, match by whole words, not letters scattered through them, so typing "report" no
  longer lists unrelated apps.

### Fixed

- **Spotlight no longer looks broken while you type.** The window resized after the panel did,
  so while results arrived the panel was cut off with no bottom edge; now it grows inside a
  window that stays put. A long list fades out instead of ending on half a row, the type label
  is shown only on the top hit, and a search with no results says "No matches".
- **Dolphin's "Open Terminal Here" opens kitty.** KDE apps look for a terminal in
  `~/.config/kdeglobals` and fall back to Konsole, so without Konsole the action did nothing.
  uQ now sets `TerminalApplication=kitty` there when kitty is installed and no terminal is named;
  a terminal you chose yourself is kept.

## [0.3.0-rc.6] - 2026-11-15

### Added

- **uQ tells logind when you are away.** After the first idle stage you have set (dim, lock
  or screens off; five minutes when none is), the session's idle hint is set, and cleared on
  your next input. Stay awake and other inhibitors hold it off. Schedulers such as scdr read it.

### Changed

- **Every app runs as an app.** The terminal shortcut (SUPER + Return), Settings opened from the
  bar or desktop menu, Welcome, and the browser a news link opens now run in a scope of their
  own in `app.slice`, as the launcher's apps always have. Before, they ran inside Hyprland's
  session or the shell's own service, so a scheduler such as scdr couldn't tell them apart from
  the desktop: focusing the terminal gave it no boost and dropped the boost from the app you
  left.

## [0.3.0-rc.5] - 2026-11-15

### Fixed

- **"Last activity" order in Open windows** no longer counts a notification backend's state
  updates as new notifications, so the newest app isn't shown as always active.
- **"By system usage" with more than one screen**: each screen's island no longer wipes the
  others' readings, which made most windows read 0% processor.
- **Window titles in Open windows tabs** keep updating while a title changes quickly (a progress
  bar in a terminal); before, they froze until it stopped.
- **Progress notifications** (built-in server) update their card in place instead of recreating
  every popup and centre card on each update.
- **uqd** no longer retries a failing file write about every second (its own temporary file
  woke it again); it waits the minute it was meant to.
- **Fewer wakeups**: the bridge follows only uqd's and the notification server's names on the
  bus, not every client joining and leaving; edge indicators do no work while switched off; a
  focus change asks Hyprland only for the window list; the opening-order map of Open windows
  forgets closed windows; Settings' Welcome page reaps the programs it starts; no "Could not
  load icon" warning per tab when the icon theme lacks the fallback icon.

## [0.3.0-rc.4] - 2026-11-15

### Added

- **Full and Half windows.** In the scrolling layout, Open windows tabs have **Full** (the
  screen's width) and **Half** buttons, as the tab's width allows: an Icon-size tab turns into a
  close button under the pointer; Small shows Full or Half (whichever the window isn't) under the
  pointer; Medium shows both; Large adds the title. The focused window's tab grows to Medium.
  A window opened from a Full window gets a screen of its own right after it; one opened from a
  Half window opens Half beside it; the last window left on a workspace becomes Full.
- **Resize windows by dragging the gap between them.** Between two columns, the gap moves and
  only the gap: one column widens by what the other gives up, and their outer edges stay put.
  Between two windows stacked in a column, the upper one's height changes and the lower one gets
  the rest. A small grip and a resize cursor show in the gap under the pointer.
- **Drag tabs to reorder windows.** In the scrolling layout, dragging a tab in Open windows to a
  new place moves its window's column to match, so the tabs and the windows stay in the same
  order.
- **Drop a tab on the screen.** Drag an Open windows tab down onto the screen below and its
  window joins the screen shown there, as half of it: beside a Half window alone it fills the
  empty half; on a Full window both become Half; on two Half windows it takes the half it is
  dropped on, pushing that window out to that side. Dropped on its own screen, a Half window on
  its own moves to the other half (the tabs keep their order) and two Halves swap. While
  dragging, a see-through copy of the tab rides under the pointer and the place the window would
  take is outlined.
- **Windows sharing a screen are framed.** Two Half windows on one screen get a light frame
  round them together; a window on its own has none.
- **Minimise from the tab's close button.** A tab's X first asks "Minimize to Tab?" (Always
  minimize instead of close / Never minimize instead of close / Cancel), and the answer is kept;
  change it in the Open windows island's settings (**A tab's close button**). X on a minimised
  window's tab closes it.
- **Whole screen pages.** In the scrolling layout, windows fall into pages a screen wide (a Full
  window, two Half ones, or a Half on its own), and the view always shows one whole page: no
  window is left cut by the screen's edge. Beside a Half window alone on its page the rest of the
  screen stays empty: the next page's window is hidden until its own page is shown, so it doesn't
  show there or while the view slides past.
- **Fill the rest.** An island can grow into the bar's empty space beside its zone (edit mode →
  the island's settings → **Fill the rest**). Open windows does unless you turn it off; the
  centre zone stays centred.
- **Windows that want attention get a dot.** When a window's app sends a notification, or the
  window asks for attention (Hyprland marks it urgent), its Open windows tab gets a dot in the
  theme's accent colour on the app icon until you focus the window. A window scrolled out of the
  strip marks **+N** instead, and the window list says **Wants attention**. Notifications from
  the app you are using mark nothing.

### Changed

- **Open windows follows the screen.** Tabs are in the order windows sit on screen (the new
  default order, **Screen order**); a minimised window keeps its place, dimmed. Choosing a
  window from **+N** centres the strip on it.
- **The bar fits every screen exactly.** Islands sit on a grid that spans each screen's bar,
  from a laptop to 4K, and keep about the width of their size. They are as far apart as windows
  are, and the bar lines up with the windows below it, following Settings → Look. A separator
  leaves a little more room between its neighbours.

### Fixed

- The screen-edge indicator's window frame matches real windows (their top, height, border,
  rounding and colours) instead of sitting lower and shorter.
- An Open windows tab's close button no longer pokes out past the tab's frame when the tab is
  at its narrowest Medium width.

## [0.3.0-rc.3] - 2026-11-14

### Added

- **A Tray island for other apps' icons.** Other apps' tray icons (Steam, Discord, password
  managers…) can have an island of their own, apart from uQ's Status island (network, sound,
  battery, quick settings). It shows as many icons as fit its size and a **+N** button listing
  the rest; clicks, menus, the wheel and tooltips work as before. While it is on the bar, Status
  shows no tray icons. A new user's bar has it before Status; on your own bar, add **Tray icons
  of other apps** from **Add an Island** (until then Status keeps showing them).
- **Each island's text, your way.** In edit mode, an island's settings now have a **Text**
  section: the **font** (the theme's, its monospaced font, or any installed font, found by
  name), **formatting** (bold, italic, all capitals) and the **text size** (70% to 150% of the
  theme's, in steps of 10%). **Use the theme's** puts it back. Only the island itself changes;
  its popup keeps the theme's text.
- **Keep windows on screen.** A floating window that opens bigger than its screen, or hanging
  under the bar or off an edge, is shrunk and moved so all of it shows, on whichever screen it
  is on (any size or scale). This also happens when it turns floating, moves to another
  workspace, or screens are added or rearranged. A window you drag part-way off screen yourself
  stays where you put it. On by default; Settings → Shell → Windows → **Keep windows on
  screen**.
- **Custom island size.** Drag an island's edge past Large and it becomes **Custom**, growing
  one Icon size at a time (up to 24 wide); drag it back in to shrink it, step by step, back to
  Large. + and − in edit mode and **Narrower**/**Wider** in the island's settings do the same.
  A Custom island shows what it shows at Large, with more room, and still steps down to fit a
  small screen.
- **Edit the bar on the bar.** Right-click any island for the **uQ Island Menu**: **Edit
  Islands**, **Add an Island** and **Remove**. In edit mode (also Super+Alt+B, or **Edit the bar**
  in Settings → Shell) drag islands between the left, centre and right of the bar, drag an
  island's edge to resize it, and click it for its settings; Esc (or **Done**) finishes. From the
  keyboard: ← → choose an island, Shift+← → move it, + and − resize it, Enter opens its settings,
  Delete removes it, A adds one. Changes apply at once on every screen and are kept; your current
  bar carries over as it was.
- **Four island sizes.** Every island is **Icon** (a square), **Small**, **Medium** or **Large**:
  2, 4 and 6 times the Icon size, and each shows more as it grows (the clock adds the date, the
  weather its city and the day's high and low, Status its volume and tray icons). Islands never
  overlap or get cut off: when they don't all fit a screen, the biggest step down a size, and
  only when every island is already an Icon are the least important left out (edit mode says
  how many).
- **New islands for the bar.** Add any of these from the uQ Island Menu or edit mode:
  - **Weather**, now its own island, with today's high and low and the next hours. Only the city
    you choose is sent to Open-Meteo, as before.
  - **Stock ticker**: your symbols scroll by with price and day change (▲/▼ as well as colour);
    click for a list with the day's range. Quotes come from a free, delayed source, and only the
    symbols are sent.
  - **News headlines** from the RSS or Atom feeds you pick (a few are suggested); click for the
    latest headlines and open one in your browser with a click or Enter.
  - Symbols, feeds, time zones, the weather's city and the clock's seconds and date are set in
    edit mode (click the island); the islands' own popups just show their news.
  - **System monitor** (processor, memory and temperature, with every core in its popup),
    **World clock** (up to four time zones), **Now playing** (track with previous, play/pause
    and next) and **Keyboard layout** (shows and switches layouts when you have more than one).
  - **Clock** options: seconds, and no, short or long date.
  Scrolling text stops while the pointer is over it, while the screen is shared, and when
  **Reduce motion** is on (then it changes every few seconds instead). Weather, stocks and news
  are fetched only while their island is on the bar; stocks and news refresh every 5 and 15
  minutes and keep the last results when offline (marked with their age). Every island popup
  works from the keyboard.
- **Screenshots, the Plasma way.** Print freezes the screen and opens it in satty with the crop
  tool: drag an area, mark it up (arrows, boxes, text, blur), then Enter copies and saves it to
  `~/Pictures/Screenshots`. Shift+Print takes the whole screen and Super+Print the focused window,
  both copied and saved at once. All three are on the Keyboard shortcuts page, and **Screenshot**
  is in Spotlight and the app menu. (Spectacle itself only runs on KDE's own compositor.)
- **Open windows, like browser tabs.** The Open windows island has a pill for every window on
  the workspace you're on (windows you minimised from it show dimmed): its icon, its title and a
  close button. As more windows open the pills share the island and shrink, down to the icon and
  close button; past that, arrows scroll the pills and **+N** lists every window. Left-click a
  pill to focus the window (or bring it back), middle-click to close it, right-click for
  Minimise, Maximise, Float, Pin the app, Move to workspace, Close and the uQ Island Menu. Order
  them in the island's settings (edit mode): **Last opened** (the default, oldest first, so new
  windows appear on the right), **Last activity** (the app that last sent a notification),
  **By pin**, **By system usage** (processor and memory) or **By name**, each either way round.
- **The app menu island shows pinned apps that aren't open.** Open an app and it moves to a pill
  on Open windows; close its last window and it comes back.
- **The window beside this one, at the screen's edges.** Push the pointer against the left or
  right edge of a screen, even over a fullscreen app, and the edge lights up with a small tab
  showing the icon of the window on that side. Click, or turn the wheel, to go there: the
  scrolling layout scrolls to it, and past the last window the tab offers the nearest workspace
  on that screen with windows in it (its number is shown). Nothing shows when there is nothing
  on that side, on an edge shared with another screen, or while locked. On the Shell page of
  Settings, **Show the window beside this one at the screen's edges** turns it off, and **Show
  the window's title on the tab** adds the title (hidden while the screen is shared). **Show it
  as: a slice of the window** swaps the tab for a strip of the neighbouring window itself, its
  near edge peeking in with its rounded corners and border, as the scrolling layout shows the
  column beside the one in view. It stays at the edge while a fullscreen or maximized window
  hides its neighbours, so every window can still be reached with a click or the wheel, and
  its border takes the accent colour when you point at it. While the screen is shared the
  slice shows the app icon instead of the window.
- **A right-click menu on the desktop.** Right-click empty desktop space (anywhere no window, bar
  or panel is) for **Create workspace +** (a new empty workspace, switched to), **Open** (your
  terminal, file manager and text editor by their own names and icons, as set on the Default apps
  page, and uQ Settings), **Appearance** (Change Wallpaper, Light Mode, Dark Mode, with the current
  one ticked) and **System** (Log Off, Lock, Restart, Shutdown; Restart and Shutdown go through the
  power menu's 30 s countdown). It opens at the pointer on any monitor and stays on screen near the
  edges; Up/Down, Right/Left, Enter and Esc work, and a click outside closes it. Right-clicks on
  windows, the bar and other panels still go to them.

### Changed

- **No second count beside noti-fi's bell.** With noti-fi, the Notifications island shows only
  noti-fi's icon, which already carries the count, not a number beside it.
- **Screen-edge indicators only point at windows out of view.** A window already wholly on
  screen gets no indicator (click it where it is); one partly or wholly off screen, behind a
  fullscreen window, or on another workspace still does.
- **The screen-edge indicator looks like a window just off screen.** Reaching the edge brings in
  the near edge of a window frame (at that window's own height when it is in this layout) in
  place of the thin line, with the app tab beside it as before.
- **Settings fits small windows.** In a narrow window the page list becomes a column of icons
  (names in tooltips), a page's tabs move under its title and wrap, a setting's controls go under
  its label when they don't fit beside it, and cards that span two columns stack instead of
  overlapping. The login screen preview and the clock on Date and time shrink or wrap to fit.
  Nothing is cut off at the smallest window size.
- **Weather has its own island.** The city is chosen in edit mode (or from the Weather island's
  popup), and the clock's popup keeps the calendar. The old on/off switches for the five islands
  in Settings are replaced by edit mode on the bar, and the Windows island lists the windows it
  doesn't show in a popup instead of growing across the bar.
- **Settings, redesigned.** Every page (except Shell, which follows the bar's own redesign) is
  split into tabs of two-column cards that fill a 1920x1080 window. Rows lead with an icon, labels
  are short, and longer explanations moved into "?" tooltips. Pages show rather than describe:
  theme tiles in each theme's own colours, a live window preview for gaps and corners, layout and
  rotation pictures, a battery gauge, power-mode tiles, a login screen preview, keyboard layouts
  and shortcuts drawn as keycaps, sample dates and money for your formats, and app icons in
  default apps, startup apps and rules. Search still finds every setting and opens its tab.
- **Quick settings has tabs.** The Control Center (Super+A, or the Status island's button) is
  split into **Sound**, **Network** (Wi-Fi and Bluetooth), **Display** (brightness, theme,
  dark/light, layout), **System** (battery, power profile, Do Not Disturb, Stay awake, profiles)
  and **Apps** (shown only when an app adds a tile), instead of one long list. The tab you pick
  stays while the panel is open and next time you open it. Left and Right move between tabs,
  Home and End jump to the first and last, and Ctrl+Tab / Ctrl+Shift+Tab switch tabs from
  anywhere in the panel. **Settings…** stays at the bottom of every tab.
- **Click to focus.** A window takes the keyboard only when you click it; moving the pointer over
  it no longer does (scrolling still reaches the window under the pointer). Put
  `hl.config({ input = { follow_mouse = 1 } })` in `user.lua` to have focus follow the mouse.
- **The login password unlocks the keyring.** The login screen now uses its own PAM service,
  `/etc/pam.d/uq-greetd` (greetd's, plus GNOME Keyring), so saved passwords are unlocked at login
  instead of asking again. `gnome-keyring` is now a dependency. An `/etc/uq/greetd.toml` you
  changed yourself (auto-login) keeps its `.pacnew`; add `[general] service = "uq-greetd"` to it.
- **One Wi-Fi icon.** nm-applet's tray icon is left out, since uQ shows the network beside the
  tray; nm-applet keeps running for VPN and enterprise Wi-Fi password prompts.
- **noti-fi's icon replaces the bell** while noti-fi handles notifications, and leaves the tray:
  left click does what it does in noti-fi, right click opens its menu, middle click toggles Do Not
  Disturb. With uQ's own notifications the bell stays.

### Fixed

- **Choosing a window in Open windows no longer moves the pointer.** The pointer stays where it
  is; a window out of view (a scrolled-away column) is scrolled into view. The screen-edge
  indicators no longer move the pointer for a moment either: Hyprland's pointer warp is now
  switched off for the focus instead of the pointer being put back afterwards.
- **The login screen no longer starts a keyring for itself.** greetd ran the greeter's own
  session (user `greeter`) through `/etc/pam.d/uq-greetd` too, so GNOME Keyring started a daemon
  for `greeter` that failed to create `/.local/share/keyrings` and logged "couldn't unlock the
  login keyring". The greeter now has its own PAM service, `/etc/pam.d/uq-greeter` (greetd's
  stack without the keyring), named by `[default_session] service = "uq-greeter"` in
  `/etc/uq/greetd.toml`; logging in still unlocks your keyring through `uq-greetd`. An
  `/etc/uq/greetd.toml` you changed yourself (auto-login) keeps its `.pacnew`; add
  `service = "uq-greeter"` under its `[default_session]`.
- **Settings fixes:** adding a pointing device under Input → Devices no longer resets its speed
  to 0; keyboard, mouse and touchpad rows show when `user.lua` overrides them; the Login screen
  page's theme box is never blank; deleting a restore point asks first; the Login screen page
  comes and goes with uQ's login screen without restarting Settings (and no longer reappears on
  machines without it); searching for "Metered" or "Addresses and DNS" lands on a row you can see.
- **Packaging metadata matches the PKGBUILD again.** `.SRCINFO` now lists `kcolorscheme` and
  `kconfig` (build and runtime), and the packaging test checks the whole `depends` array for
  `polkit` instead of one exact line.
- **An app's bar or quick-settings widget runs its new code after a package update.** The shell
  kept the widget's old QML until it restarted. The bridge now sends a digest of each widget's
  files, and when it changes the shell loads the widget afresh.
- **Settings looks like the theme.** Outside Plasma, Kirigami took its colours from KDE's
  defaults, so Settings was Breeze Light with dark controls on top, whatever the theme. uqd now
  renders the theme as a KDE colour scheme (`~/.local/state/uq/theme/kde/uq.colors`), and Settings
  uses it, with the theme's icons and interface font. It follows a theme or light/dark change
  while it is open (controls drawn by Kvantum change at the next start). Dropdown lists are at
  least as wide as their box, the selected row in lists and menus uses the theme's text-on-accent
  colour, and Settings falls back to the Fusion style instead of failing when
  qqc2-desktop-style is missing. Plugin pages get the new colours as well.
- **Updating uq no longer leaves Hyprland in emergency mode.** Hyprland reloads its config when
  a file it read changes, and a package update replaces `/usr/share/uq/hypr/base.lua` by
  removing and rewriting it: a reload in between failed its `require` and left only Hyprland's
  emergency binds ("a lua config resulted in no binds"). uqd now follows Hyprland's
  `configreloaded` event, and when a reload failed only because a config file was missing that
  is there again, it reloads once more (at most once every 10 s).
- **`notify takeover --now` works on a uQ session that has been up a while.** uQ keeps the
  notification name with its built-in server until it has connected to noti-fi's socket, and it
  looked for that socket on the bridge backoff, every 30 s after the first half minute; noti-fi's
  takeover waited 10 s and rolled back. While the socket file is missing, uQ now looks every 3 s
  (the backoff still paces failed connects). noti-fi's takeover also waits longer when uQ holds
  the name.
- **Welcome's "Choose a look" buttons** showed `[object V4ReferenceObject]` and applying one said
  the theme wasn't installed; they show each theme's name and apply it (#40).
- **Settings pages no longer draw over each other** when you click through the page list
  quickly: pages switch at once, without the cross-fade (#40).
- **The bar's icons show on a fresh install.** The session runs Qt through qt6ct, which has no
  icon theme until your own `qt6ct.conf` names one, so the volume, battery and network icons,
  Settings' app icon and every app without its own icon were blank or a magenta square. uqd now
  writes the theme's icon theme to `~/.local/state/uq/theme/shell.env`, the shell and lock load it,
  and the shell restarts by itself when the icon theme changes (switching between light and dark
  switches Papirus-Light and Papirus-Dark, so it restarts then too).
- **The login screen no longer starts a desktop session for the `greeter` account.** Hyprland
  started `hyprland-session.target` there, which brought up portals, PipeWire and xdg-user-dirs
  for an account whose home is `/`, and left a failed `xdg-desktop-portal-gtk` at every boot.
- **Settings' drop-down lists no longer log a TypeError each** (`positionToRectangle … is not a
  function`) under the KDE desktop style.
- **The Users page opens again.** It used `Kirigami.Avatar`, which Kirigami 6.31 no longer has, so
  the page didn't load, and every page opened after it was drawn on top of the page before. A
  user's picture is now uQ's own, with their initial when there is none.
- **A Settings page that can't load says so** in its place, with the error, instead of leaving
  the previous page drawn underneath every page after it. `uq-settings --screenshots` exits 1
  over one, and a new test opens every page in CI.
- **Opening Settings on a page no longer toggles its window maximized.** Each `uqctl settings
  <page>` (and each link from the shell into Settings) maximized the open window or restored it,
  and Settings remembered whichever state it ended in.
- **Qt apps get the theme's icons.** With `uqctl setup --apps`, qt6ct loaded uQ's colours but
  named no icon theme, so Qt apps (Dolphin, uQ Settings) found no themed icons. uqd now keeps
  `icon_theme=` in that `qt6ct.conf` in step with the theme, as it does gsettings' for GTK apps,
  and leaves a `qt6ct.conf` of your own alone. Apps pick it up when they next start.
- **Drop-down lists in Settings have a maximum width**, so a long choice no longer squeezes the
  row's label (on Input, "Switch layouts with" wrapped two letters a line).

## [0.3.0-rc.2] - 2026-11-12

### Added

- **Pages from scdr, crumBS, noti-fi, XZram and scale_tail in Settings.** **Scheduling** shows
  whether scdr is on, pauses and resumes it, and edits per-app rules and the scheduler.
  **Storage and cleanup** shows disk use, cleans caches and leftovers, puts a cleanup back and
  uninstalls apps. **Notifications** sets do not disturb, automatic do-not-disturb and how much
  history noti-fi keeps. **Memory and swap** changes the zram size and algorithm and the
  swappiness. **Tailscale** connects, picks an exit node, accepts routes and signs out. The pages
  ship with those apps (update the app to get its page) and appear under **Apps** when the app is
  installed. New optional dependencies `xzram` and `scale_tail`. A weekly check runs each app's
  page against uQ (`.github/workflows/companions.yml`).
- **Display arrangements, the main display, and touchpad gestures.** The Displays page can
  **Remember for these screens**: each arrangement is matched by the make, model and serial of
  every connected screen (not by port), and when screens are plugged in or out uqd switches to
  the one for the new set within about 2 seconds, with no prompt (only when the set of screens
  changes, so a layout you keep stays until then); **Switch arrangements when
  screens change** turns it off, and automatic switches aren't recorded in History. A **main
  display** gets notifications, the volume pop-up and Spotlight, starts the session, and is the X11
  primary when `xrandr` is installed (new optional dependency `xorg-xrandr`). **Span across
  displays** draws one wallpaper over all screens. The **workspace planner** puts workspaces on
  screens one after another, in turn or by hand, and moves them back when a screen returns.
  **Bar per screen** shows the bar on all screens, the main one or chosen ones, and never on none.
  The Input page gains **touchpad gestures** with conflict checks (the editor warns and uqd
  refuses a save that Hyprland would refuse) and the workspace swipe options. New `uqd` call
  `RememberMonitors`; `ListOutputs` gains `roles` and `arrangement`; `SetSection("monitors")` now
  takes every key except the layout. The three-finger swipe moved from `base.lua` into
  `generated.lua`, so a gesture of your own in `user.lua` that overlaps uQ's makes Hyprland
  refuse the config.

### Fixed

- **Recording a shortcut on the Keyboard shortcuts page works on Hyprland.** Capture never
  started, because Hyprland only registers a submap that holds at least one bind; uQ's capture
  submap now has one placeholder bind (`SUPER+SHIFT+CTRL+ALT+XF86Launch9`). Found by the new live
  checks in `tests/live`.

## [0.3.0-rc.1] - 2026-11-11

### Added

- **Settings pages: longer D-Bus deadlines for writes that may ask for a password.** A page's
  `[dbus]` table takes `timeout_ms = { Method = <ms> }`, up to 120 seconds, for listed methods
  only; every other request still ends within 2 seconds. Checked by the manifest loader and
  `uqctl integrations check`. See docs/settings-pages.md.
- **`uqctl integrations check --no-programs`.** Skips only the check that each command's and
  app action's program is installed, so an app's CI can check its `uq/` folder without
  installing the app. Paths are still checked.
- **Settings pages may ship sibling files.** A page folder may hold a `logic.js` imported from
  its QML (`import "logic.js" as Logic`) and further component files; tested and documented in
  docs/settings-pages.md.
- **Settings pages follow changes instead of polling.** A page's read `[[command]]` takes a
  `watch` table (files, absolute or under `~/`, and D-Bus signal matches on the session or system
  bus); `ctx.command.watch(id, callback)` runs it now and again after each debounced burst of
  changes, one run at a time, until `ctx.command.unwatch` or the page closes. See
  docs/settings-pages.md.
- **`ctx.openApp(desktopId)` for Settings pages.** A page can open an app (its own full app,
  say) by its `name.desktop` id, the way the launcher does (`uqctl launch`), detached from
  Settings. Paths and other ids are refused.
- **A stable component list and a test kit for Settings pages.** docs/settings-pages.md lists the
  `org.uq.settings` components pages may use under `uq_api = 1` (`UqLabel`, `UqComboBox`,
  `SettingRow`, `SwitchRow`, `ValueSlider`, `ErrorCard`) with their properties.
  `templates/settings-page-tests/` is a kit an app copies into its repository: a stub
  `org.uq.settings` module, a stand-in `ctx` and an example offscreen `qmltestrunner` test. The
  template page now keeps its logic in `logic.js`.
- **Keyboard shortcuts.** A Keyboard shortcuts page lists every uQ shortcut by what it does:
  change its keys (press them to capture, or pick them), turn it off, reset it, or put all back.
  Add your own shortcuts: open a uQ panel, a window or workspace action, a layout, start an app
  (through `uqctl launch`) or run a command in its own scope. Two shortcuts on the same keys are
  refused, naming both. New default **Open Spotlight for emoji** (Super + period). The lock recovery shortcut stays on. New **Switch to the next profile**
  shortcut, and profiles can turn shortcuts on and off. `uqctl keys list|reset` works from a
  terminal. New `uqd` calls `ListBinds`, `PauseShortcuts` and `ResumeShortcuts`; `keybinds` is a
  `GetSection`/`SetSection` domain. A failed key capture never leaves shortcuts paused, and uqd
  resets the capture state at start.
- **Rules from apps.** Installed apps can offer window rules in
  `/usr/share/uq/integrations/<name>/rules.toml` (app ID patterns, a window title pattern, a
  workspace, full screen, float). The Window rules page lists them under **Rules from apps**,
  each off until you turn it on; turning one on is checked, rendered and verified like your own
  rules, and shows in History, where it can be undone. A rule whose app is removed stops applying
  and comes back with the app. `uqctl integrations check` checks `integrations/<name>/rules.toml`
  and `uqctl integrations list` shows the files. New `uqd` call `ListIntegrationRules` and the
  `integration_rules` section. See docs/integrating.md.
- **Emoji key.** `Super + .` opens Spotlight with `:` already typed, the emoji search an emoji
  provider such as emobie answers. Pressed while Spotlight is open, it starts the search over at
  `:`. The bind is `uq:emoji` in `base.lua`.
- **Your own themes: make, import, export and delete.** The Appearance page has **New theme from
  this one…** (seven colours per version, live contrast check, a version that is hard to read
  can't be saved), **Import…**, **Export…** and **Delete…**, and shows the theme names. Themes
  are `.uqtheme` files (TOML, `format = 1`, size-capped, schema-checked, never run) saved in
  `~/.local/share/uq/themes/user-<slug>/`; imports save a restore point and can bring window
  gaps, corners and effects. `uqctl theme list --long|import|export|delete` do the same, and the
  shell's quick settings show names. New `uqd` calls `ListThemeInfo`, `ExportTheme`,
  `ImportTheme`, `DeleteTheme` and the `ThemesChanged` signal.
- **uQ session.** A "uQ" entry in your display manager's session list. Its first login sets up
  uQ for you (no `uqctl setup` needed); if setup reports a problem, the problem goes to
  `~/.local/state/uq/session.log` and the desktop starts anyway. `uqctl setup` gains `--quiet`.
- **Login screen.** uQ's own greeter for greetd, in uQ's look: users with their pictures, the
  password field, the session to start (uQ first), and Suspend, Restart and Shut down (click
  twice). It follows the theme, variant and wallpaper chosen for it and never reads anyone's home
  folder. Wrong passwords, locked accounts and other login messages show as plain text; an
  expired password asks for a new one twice and checks they match. If it fails 3 times in a
  minute, greetd's text login takes over so nobody is locked out. Turn it on with
  `run0 uqctl greeter enable` (needs `greetd`) and off with `run0 uqctl greeter disable`, which
  brings back the display manager it replaced. See docs/greeter.md.
- **Welcome.** A short tour in Settings the first time you log in: what uQ is and the three keys
  to know, then the theme (with automatic light and dark), the wallpaper, how windows are
  arranged, your screens (only with more than one), the apps made for uQ that are installed and
  where to get the others, and saving a fingerprint for the lock screen (only with a reader and
  no finger saved yet). Every step can be skipped and every choice applies at once. Welcome
  shows once: closing it at any step counts as done. See it again from Settings → About → Show
  Welcome, or with `uqctl welcome --reset`.
- **Installer hand-off.** An installer's first-login window (bindle's app picker) can hold
  Welcome back with `$XDG_RUNTIME_DIR/uq/welcome.defer` and open it afterwards with
  `uqctl welcome`, so the two never show at once. A file older than 30 minutes is ignored, so a
  crashed installer can't hide Welcome for good. See docs/welcome.md.

- **`uq-dev`.** Use uQ and develop it on the same machine: `uq-dev session` opens a nested
  uQ desktop running a checkout's uqd and shell on a private bus with sandboxed config,
  `uq-dev run` runs an app from its checkout with throwaway config, and `uq-dev promote` /
  `rollback` install a package built from a checkout's HEAD and undo it (docs/developing.md).
- **Settings app.** `uq-settings` (Qt, Kirigami; `uqctl settings [page] [setting]` or the new
  "Settings…" button in quick settings) with Appearance and Wallpaper pages, search across
  settings, and a single instance on `org.uq.Settings1`. Third-party pages are read from
  `/usr/share/uq/settings/pages/<id>/page.toml`; a page that fails to load shows an error card.
  `uqd` gains `GetSection` / `SetSection` for the `appearance` and `wallpaper` domains.
- **Appearance.** UI and monospace fonts, font size, cursor theme and size, and icon theme, each
  overriding the theme when set. They reach Hyprland, GTK and the shell.
- **Wallpaper.** A wallpaper per monitor (or one for all): an image with fill, fit, center or
  tile, a folder slideshow with an interval, and a fallback colour. The shell draws it on the
  background layer.
- **Kvantum.** Qt apps follow the uQ theme through a generated Kvantum theme
  (`uqctl setup --apps` links it in; needs the `kvantum` package). An unchanged qt6ct.conf
  from 0.2 is switched to `style=kvantum`; one you edited is left alone with a hint. Turn it
  off with `appearance.kvantum = false`.
- **Look and layouts page.** Gaps, border width, rounding, blur (on, size, passes), shadow, and
  animations (on/off, speed) override the theme's values; workspace rules, the default layout and
  dwindle/master/scrolling options live on the same page. `uqd` gains `look` and `layouts`
  sections (stored as `[look]` and `[layout_options]`), verified with Hyprland before they apply;
  a rejected value keeps the old config and the page shows Hyprland's message. `SetLayout` also
  emits `SectionChanged("layouts")`, so the page follows the `SUPER` keybind.
- **Shell page and `shell` section.** Islands on or off, 24-hour clock, launcher providers on/off
  and order, and the terminal for `Terminal=true` apps. The bridge opens such apps in the chosen
  terminal (`X-TerminalArgExec`, else `-e`; kitty if it is missing or unparseable) and refuses to
  activate or launch from a switched-off provider.
- **Settings history, undo and restore points.** `uqd` records every settings change (who made
  it, and the values before and after) in `~/.local/state/uq/history/` for 30 days or 500
  entries, and keeps restore points of `state.toml` in `~/.local/state/uq/restore/` (automatic
  ones before the first change of each day, before a restore and when uQ is updated, 20 at most;
  your own are never removed). New `uqctl history [--limit N]`, `uqctl undo [<id>] [--force]` and
  `uqctl restore list | save <name> | delete <id> | <id>`. On D-Bus: `ListHistory`, `Undo`,
  `ListRestorePoints`, `SaveRestorePoint`, `DeleteRestorePoint`, `Restore` and a `HistoryChanged`
  signal. **Breaking for D-Bus callers:** `SetSection`, `SetSetting`, `SetTheme`, `SetLayout`
  and `SetSchedule` take a last `origin` string argument (`""` records as unknown).
- **Where a setting's value comes from.** `uqd` can explain a setting's live value (theme,
  default, uQ, `user.lua`, changed while running, or error). Setting rows in uq-settings say
  "Overridden in user.lua, line N" or "Changed while running (not by uQ)", with a Re-apply
  button. uQ reads `user.lua` and never writes it.
- **Undo and History in uq-settings.** Ctrl+Z and Ctrl+Shift+Z undo and redo your changes, a
  "Changed this session" list shows what changed (including changes "from the terminal"), and a
  History page lists restore points and past changes, with Undo, Save and Restore. The window
  remembers its size.
- **Displays page.** Arrange outputs by dragging, and set mode, refresh rate, scale, rotation,
  mirroring, VRR and on/off. A change applies live and a dialog asks to keep it; if you don't
  answer within 15 seconds (or press Escape, or the app or uqd stops), the old layout comes back.
  `uqd` gains `ListOutputs`, `ApplyMonitors`, `ConfirmMonitors`, `RevertMonitors` and a
  `MonitorsChanged` signal; only a confirmed change is saved and recorded in the history.
- **Input page.** Keyboard layouts (from xkeyboard-config's list) with variants, order and a
  switch combo, repeat delay and rate, mouse and touchpad speed, acceleration, natural scroll,
  tap-to-click and per-device overrides. `uqd` gains an `input` section and `ListInputDevices`.
- **Profiles.** A profile is a named set of settings (Look and layouts, Appearance with theme,
  light/dark and fonts, Shell, Wallpaper, and a power mode) that you turn on together and
  turn off with Back to normal, which keeps anything you changed by hand since. New Profiles
  page in uq-settings, a Profile row in quick settings, and `uqctl profile list | apply | off |
  next | export | import | delete`. Each switch saves a "Before the ... profile" restore point and
  shows in History as "from the ... profile". A power mode goes through the system's power
  service; if it can't be set, the rest still applies with a notice. While a profile that sets
  light or dark is on, the light/dark schedule doesn't switch it. Export and import use TOML
  (`format = 1`); keybinds can't be in a profile yet. `uqd` gains `ListProfiles`,
  `GetProfile`, `SetProfile`, `DeleteProfile`, `ApplyProfile`, `ProfileOff` and a
  `ProfileChanged` signal.
- **Window rules.** A Window rules page in uq-settings: rules that match a window by App ID, title,
  state or workspace and make it float, open on a workspace, blur less, and more, with a live
  "Matches now" preview and **Pick a window** (click a window to fill in its App ID). **Why does
  this window…?** lists the rules that apply to a window in order and flags any that `user.lua`
  overrides. Five templates, panel rules (bars, notifications, launchers) and workspace rules that
  join the per-workspace layouts are included. Rules have history, undo and restore points, and a
  profile can turn them on and off. `uqd` gains the `rules` section, `PickWindow`, `PickDone`,
  `ListWindows` and a `PickRequested` signal. uQ never writes `user.lua`; name a rule
  `uq:<id>` there to override it.
- **Startup apps, environment variables and app scaling.** A Startup page (under System) lists
  the apps in the autostart folders, with a status for each (Running, Waiting, Failed with its
  last output, and so on), a start delay, and add-an-app-or-command (run without a shell). uqd now
  starts them once per session, each in its own scope under `app.slice` with its output in the
  journal, and a restarted uqd starts nothing new. Turning off a system entry writes a hidden
  copy marked `X-uQ-Autostart=true`; your own files are never edited. Environment variables set
  here go to Hyprland and `~/.config/environment.d/60-uq.conf`. An Apps and scaling page has five
  switches (Sharp X11 apps, Electron on Wayland, Chromium-based browsers on Wayland, Qt apps prefer
  Wayland, Steam UI scale), a Blurry right now list and per-app fixes (Run on, Size, extra
  variables and options), which also apply to autostart. "Also when started from other
  launchers" writes a desktop-file override marked `X-uQ-Override=true`, and Chromium-based
  browsers get a flags file with a marker line. uQ never writes through a symlink and never edits
  or deletes a file it didn't mark; it replaces its own `60-uq.conf` whatever it holds. All three
  domains have history, undo and restore points, and a profile can hold environment variables,
  the switches, a fix's Run on and Size, and an existing startup app's on/off and delay (never a
  new command, launch options or a launcher file; startup changes apply at next login).
  `uqd` gains `ListAutostart`, `ShellReady` and an `AutostartChanged` signal, and the `env`,
  `compat` and `autostart` sections use `GetSection` / `SetSection`. `uqctl launch <desktop-id>`
  launches an app the way uQ's launcher does.
- **Settings helper.** A small system service, `uq-settings-helper`, lets Settings change a few
  system-wide things after you confirm with an administrator password: what the lid and power
  button do, and the login screen's theme, wallpaper, monitor layout and auto-login. Each change
  writes one fixed file and accepts only a fixed list of values; it never restarts logind (the
  lid and power-button change applies at its next reload). The wallpaper is copied as-is, up to
  20 MiB, and never opened as an image. Turning auto-login on or off asks for the password every
  time. It starts when needed, runs in a tight sandbox with no special privileges beyond root's
  own files, and quits after 30 seconds without use.
- **Settings pages from other apps can do more.** A page's `page.toml` can now list fixed
  commands it may run (`[[command]]`), conditions for it to show (`[requires]`: a program in
  `/usr/bin`, a running service) and the one D-Bus object it talks to (`[dbus]`, on the session
  or system bus). The page gets `ctx.command.run` and `ctx.dbus.call` / `ctx.dbus.get`; a
  command marked `confirm` asks "Are you sure?" with the page's own question first. Commands
  run without a shell, with a clean environment, time limits and output limits. A page's id
  must now match its folder name. Pages installed or removed while Settings is open appear or
  go within a second or two. Writing one: `docs/settings-pages.md` and
  `templates/settings-page/`.
- **`uqctl integrations`.** `uqctl integrations list [--json]` shows every widget, settings
  page, launcher provider and app action uQ found under `/usr/share/uq/`, whether it shows, and
  why not. `uqctl integrations check <dir>` checks an app's `uq/` folder before packaging (the
  layout, every file, the plain-text rule, that each command's program exists).
- **Launcher providers can be started on demand.** A `.provider` file may say
  `Activatable=true`. When Spotlight or the app menu opens, uQ asks the session bus to start each
  such provider that isn't running (at most once every 30 seconds each, never while you type), so
  a provider no longer has to run all the time. Results still come only from running providers,
  so the first keystrokes after a cold start may miss them.
- **`uq-tokens` for web apps.** A small JavaScript module and CSS file (`web/uq-tokens/`) that
  Tauri, Electron and browser apps copy in to follow the uQ theme: colours, rounding, gaps and
  fonts become `--uq-*` CSS properties and update live when the theme changes. Bad values are
  ignored, and without uQ the app's own theme applies. The README has the Tauri and Electron
  recipes and a version check for vendored copies.
- **Sound, Bluetooth and Network in Settings.** Sound and Bluetooth join the Devices pages and
  Network gets its own Connections group. A missing service shows "Not available" with the
  package to install.
  - **Sound:** pick the speakers or headphones to play through and the microphone to record
    from, with volume and mute for each; set each app's volume and which device it plays through
    or records from; choose a sound card's mode; and play a test sound (needs
    `sound-theme-freedesktop`). The page updates by itself when devices or apps come and go.
  - **Bluetooth:** turn Bluetooth on or off, let other devices find this computer, look for new
    devices, pair, connect, disconnect, trust and forget them, and see their battery. When a
    device shows a code, the page shows the same code and waits for you to press Confirm;
    nothing is accepted on its own. The page answers pairing requests only while it is open.
  - **Network:** turn Wi-Fi on or off, see nearby networks, join one (also hidden networks and
    work or school networks that need a user name or certificate), and forget saved ones;
    wired connections; WireGuard VPNs (import a `.conf` file and edit it) and OpenVPN
    connections if NetworkManager's OpenVPN plugin is installed; a hotspot; addresses and DNS
    for IPv4 and IPv6; and marking a connection as metered. Passwords go straight to
    NetworkManager and are saved for you only, unless you tick "Everyone who uses this computer
    can join it too". uQ never writes them down or logs them.
- **System pages in Settings.** Power, Users, Date and time, Region, Default apps and About join
  Startup under System, and a Login screen page appears when uQ is your login screen. They talk
  to the system's own services, so they work even when uQ's settings service isn't running.
  Changes that need an administrator ask for the password; cancelling leaves the setting as it
  was, and a missing service is shown as "Not available" with the package to install.
  - **Power:** power mode, battery charge, time left and health, and what closing the lid (also
    while plugged in) or pressing the power button does. Those three are saved by the settings
    helper and take effect after a restart; without the helper they are hidden with a note.
  - **Users:** change a person's name, picture, password or account type, add people and remove
    them (with or without their files). Removing needs the user name typed; you can't remove
    yourself or the last administrator, or make the last administrator a standard user.
    Password boxes are emptied after every try, and passwords are hashed before they leave the
    app.
  - **Login screen:** the login screen's theme (or "same as mine", copied when saved),
    wallpaper (yours, a picture up to 20 MiB, or none), your display layout, hiding the user
    list, reduce motion, and auto-login, with a warning that anyone at the computer gets your
    session and that it's only for fully encrypted disks. The Users page links here. Without
    the settings helper it is view-only.
  - **Date and time:** time zone, setting the time from the network or by hand, and the bar's
    24-hour clock.
  - **Region:** system language, formats (dates, numbers, money, measurements, paper) and the
    text console's keyboard layout.
  - **Default apps:** web browser, email, files, text editor, pictures, videos, music, PDF
    documents and the terminal. Only the choices you make change your `mimeapps.list`; every
    other line in it stays.
  - **About:** the computer's name (editable), system, kernel, processor, graphics, memory, and
    the uQ, Hyprland and Quickshell versions.
- **Widgets from other apps.** An app can add a small widget to the bar (an icon in the Status
  island; up to four, the rest under a "…" button) or a tile to a new **Apps** group at the bottom
  of quick settings, by installing a folder to `/usr/share/uq/shell/widgets/`. Widgets appear
  and go a second or two after their package is installed or removed, without logging out, and
  only while what they need (a program, a running service) is there. A widget that fails to load
  or breaks is hidden on its own and tried again a minute later; it never takes the bar down. A
  widget can only call the D-Bus methods and run the commands its `widget.toml` lists.
- **App actions.** Right-clicking an app in Spotlight or the app menu lists actions other apps
  added for it, such as "Uninstall…" or "Check for update". The action runs as its file says,
  in your terminal when it asks for one; the clicked app's id is the only thing filled in.
- **Writing integrations.** `docs/integrating.md` covers all four ways an app can plug into uQ
  (theme, settings page, widget, launcher provider and app actions), what uQ checks, and a
  checklist for each. New templates: `templates/widget/` and `templates/provider/` (a small Rust
  launcher provider that quits after a minute without use).

### Changed

- `base.lua`'s start and shutdown hooks do nothing when `UQ_DEV_SESSION` is set, so a nested
  dev session can no longer restart, repoint or stop the installed uqd and shell.
- **Animations moved to `generated.lua`.** The animation lines are no longer in `base.lua`.
  After an upgrade Hyprland shows its own default animations until `uqd` re-renders
  `generated.lua` (restart `uqd` or change any setting). `hl.animation` calls in `user.lua`
  still win.

### Fixed

- **Restore points saved in the same second list newest first.** They used to sort by a
  random suffix, so "the newest point" could be an older one.
- **The lock screen no longer waits for ever on a password check.** If PAM has not answered
  after 60 seconds, the check is stopped and the lock screen says "That took too long. Try
  again." Esc stops a check that is still waiting.
- **A slow session bus or uqd no longer holds up the shell's start.** The bridge gives up on
  connecting, subscribing and asking for uqd after 2 seconds and tries again later, and sends
  `Hello` after at most 2 seconds of waiting for Welcome's status (the shell asks again).
- **`uqctl history | head` no longer panics.** uqctl stops quietly when the program reading its
  output closes the pipe.
- **Reduce motion does something.** It is now your own setting (Settings → Look and layouts →
  Reduce motion, `ui.reduce_motion`): the idle dim and wallpaper changes happen at once, and
  window and panel animations are off. The login screen's own Reduce motion stops its user list
  from coasting when flicked; it already had no other animations.
- **"Use my display layout at login" is used.** The login screen now arranges its screens from
  the copied layout (`/etc/uq/greeter-monitors.toml`, checked like your own; a bad file falls back
  to the automatic layout). Settings → Login screen has **Use automatic layout** to go back
  (new helper call `ClearGreeterMonitors`).
- **Changing your own password asks for your current one, not an administrator's.** Settings →
  Users now changes it through `passwd`, which checks the current password and the password
  rules and says plainly why a password was refused. Other people's passwords still go through
  the accounts service.
- **The Bluetooth page no longer crashes when an adapter goes away while it reads.**
- **The shell says when something you asked for didn't happen.** The app menu and Spotlight
  stay open and say why when an app can't be opened (they close once it has been), Spotlight
  says when search isn't working, and quick settings says why a theme, schedule or layout switch
  failed. Each failure is also logged.
- **The Power page updates as the battery and power mode change.** It follows UPower and
  power-profiles-daemon instead of reading everything again every 30 seconds, so the charge and
  a mode changed elsewhere show at once. Only when the system bus can't be followed does it
  fall back to reading again every minute.
- **Removing the auto-login user turns auto-login off.** The login screen no longer goes on
  trying to log in an account that's gone; if turning it off is refused, the Users page says so.
  `uq-settings-helper` also refuses auto-login for a locked or expired account.
- **Safer file writes.** Every all-or-nothing write in uQ now uses a temp file no other write
  shares (two writes at once can no longer trip over each other) and syncs the folder after the
  rename, so a crash right after a save keeps it. A file that is a symlink is still replaced by
  a regular file, as before; this is now documented.
- **An integration command that starts something in the background no longer times out.** The
  runner used to wait for its output to close, which a background process it started keeps
  open; it now ends the run when the command exits, keeping the output that arrives within
  200 ms.
- **docs/settings-pages.md says what the command path check covers.** The `/usr/bin` and
  `/usr/lib` check is on the path as written; a symlink there that points elsewhere is accepted.

## [0.2.1] - 2026-11-10

### Fixed

- **Displays: the 15 s to keep a change starts when it shows.** uqd counted the window from
  when the request arrived, before the config check (up to 10 s) and the apply, so on a slow or
  busy machine a new layout could show for a second or two before reverting.
- **`uqctl setup` on a fresh install.** An unedited copy of Hyprland's stock `hyprland.lua`
  (written by Hyprland on first start, by hyprland-welcome, or by bindle's installer) is no
  longer treated as the user's own config. It is moved to `hyprland.lua.stock` and replaced
  with uQ's stub, so a fresh install starts uQ on the next login instead of exiting 3.

### Changed

- **scdr alignment.** `uqd`, `uq-shell` and `uq-lock` run in `session.slice` instead of the
  default `app.slice`, so priority daemons such as scdr treat them as the desktop rather than
  apps (scdr would otherwise demote the bar and lock screen under CPU load). When scdr's session
  agent is enabled, Hyprland's start hook (re)starts it with the session environment, so scdr's
  focus roles work on uQ; it stops with the session. scdr is a new optional dependency.

## [0.2.0] - 2026-11-09

Phase 2: the uQ shell, a Quickshell desktop shell that runs as `uq-shell.service`.

### Added

- **Unattended setup.** `uqctl setup` reports its outcome in the exit code (0 uQ config in place,
  1 error, 2 refused as root, 3 your own `hyprland.lua` kept) and with `--json`, so installers
  such as bindle can run it for the user.
- **Bar and islands.** A floating, themed bar on every monitor with five islands:
  - System: workspaces with their layout icon, and a dock of pinned and running apps
    (`uqctl pin add|rm|list`).
  - Work: the focused window's title, with minimise, maximise, close and a window menu.
  - Time and weather: clock, month calendar and the current weather for a city you pick
    (`uqctl weather set`).
  - Status: tray, media, network and VPN (with the exit country, if `uqctl privacy public-ip on`),
    volume and battery.
  - Notifications (see below).
- Window titles, network names and every other outside string are shown as plain text only.
  The bar follows theme changes live and survives uqd restarts and monitor hot-plug.
- **Launchers.**
  - `SUPER+Space` opens Spotlight: apps ranked by match and launch history, a calculator
    (`=2+2`), a command runner (`>cmd`) and search providers that other apps can add over D-Bus.
  - Tapping `SUPER` opens the app menu: categories, favourites and a power row.
  - Apps launched from the shell run in their own systemd scopes, so they survive a shell
    restart. `uqctl launches clear` forgets the launch history.
- **Quick settings and OSD.**
  - `SUPER+A` (or the Status island) opens quick settings: volume and output device, mic,
    brightness, power profile, battery, Do Not Disturb, Stay awake, Wi-Fi, wired, Bluetooth,
    theme, dark/light, the theme schedule and the current workspace's layout.
  - Volume, mic-mute and brightness keys work, including over the lock screen, and show an OSD:
    noti-fi's when it is running, otherwise uQ's own on the focused monitor.
- **Notifications.**
  - The Notifications island shows the latest app, the count and Do Not Disturb. Click it or
    press `SUPER+N` for the notification centre; right-click toggles Do Not Disturb.
  - `uqctl notif backend auto|notifi|builtin|swaync|mako|dunst` picks who shows them. `auto`
    uses noti-fi when it answers, otherwise uQ's built-in server with its own popups and centre.
  - uQ never starts, stops or masks another notification daemon; if one already owns the name,
    the island says so.
- **Lock, sleep, idle, power and polkit.**
  - `SUPER+L`, `loginctl lock-session`, idle and sleep lock the session with `uq-lock`, a
    separate Quickshell instance that unlocks with your password or fingerprint (PAM stacks
    `/etc/pam.d/uq-lock` and `uq-lock-fingerprint`, kept on upgrade). If the lock or the shell is
    killed, the session stays locked. `SUPER+CTRL+L` is the recovery bind.
  - The machine never sleeps before the lock is secure.
  - Idle dims, locks, turns the screens off and, on battery, suspends; each stage is set with
    `uqctl idle set`. Playing media holds off dimming and screens off.
  - `SUPER+Escape` opens the power menu (lock, log out, suspend, restart, shut down); restart and
    shut down wait 30 s and can be cancelled.
  - uQ is a polkit authentication agent: admin prompts from apps open a themed dialog.
- `uqctl mouse` sets pointer speed, acceleration, natural scroll, left-handed mode and scroll
  factor.

### Changed

- The packages build from the tagged GitHub Release tarball; `scripts/build-local-pkg.sh` builds
  one from a checkout.

## [0.1.0] - 2026-11-08

Phase 1: the foundation. Not published as a release.

### Added

- `uqd`, a session daemon that owns the theme, the dark/light schedule and per-workspace
  layouts, writes uQ's generated Hyprland config and rendered theme files, and serves
  `org.uq.Desktop1` on the session bus.
- `uqctl`, its command-line client: `theme list|set`, `layout set`, `schedule fixed|sun|off`
  and `setup` (writes `~/.config/hypr/hyprland.lua`; `--apps` also themes GTK, qt6ct, kitty,
  foot and alacritty, never overwriting your files).
- Seven themes, each in dark and light: Gruvbox, Dracula, Matcha, Forest, Graphite, Rosewood and
  Velvet. Every text and background pair meets WCAG AA contrast.
- A base Hyprland Lua config with per-workspace layouts (`SUPER+ALT+D` dwindle, `SUPER+ALT+M`
  master, `SUPER+ALT+S` scrolling) kept across reloads and reboots, and your own settings in
  `~/.config/hypr/uq/user.lua`.
- Arch packages `uq` and `uq-hyprland-pin`, which holds Hyprland to 0.56.2–0.57.x.
