# Settings slice D (startup apps, environment variables, apps and scaling): manual live check

Run on a real Hyprland session (0.56.2 or newer, up to 0.57.x) with uQ installed. Not CI, not a
private bus, except item 14. Tick each item. The pages are **Startup** and **Apps and scaling**
in `uq-settings` (`uqctl settings startup`, `uqctl settings apps-scaling`). Have `foot`, `xeyes`
or `xterm`, VS Code or another Electron app from the repos, Chromium, and ideally a screen scaled
to 125 %. Before you start, note `user.lua`'s modification time:
`stat -c %y ~/.config/hypr/uq/user.lua`. It must be the same at the end.

Where a step says "check Hyprland's errors", run `hyprctl configerrors`. Empty output means no
errors.

- [ ] **1. Apps start at login (spec §1, criterion 6).** Put a `.desktop` file for
  `foot --title AutoTest` in `~/.config/autostart` and log out and in. The app is open.
  `systemctl --user status 'app-uq-autostart-*'` shows the scope under `app.slice`, and
  `journalctl --user -t uq-autostart-<id>` has its output. **Startup** shows the app as Running.
  Turn it off and log out and in: it doesn't start.
- [ ] **2. A system entry turned off.** Turn off an entry from `/etc/xdg/autostart`. Check that
  `~/.config/autostart/<id>` has `Hidden=true` and `X-uQ-Autostart=true`. Turn it on: the file is
  gone. An entry with `OnlyShowIn=GNOME;` shows "Not for this desktop". Turn off an app of your
  own in `~/.config/autostart`: the row says "Off in uQ" and your file is unchanged.
- [ ] **3. Once per session.** `systemctl --user restart uqd` doesn't start a second copy of any
  app, nor one you turned on since you logged in (that row says "Starts next time you log in").
  Kill an autostarted app, restart uqd, and the page says "Started earlier; finished".
- [ ] **4. Failure.** An entry with `Exec=sh -c "echo boom; exit 3"` shows "Failed (exit 3)" with
  `boom` in its last lines, and none from an earlier login.
- [ ] **5. Delay.** Give an entry a 10 second delay. It starts about 10 seconds after the shell
  appears, and the row says "Waiting 10 s" until then.
- [ ] **6. Electron on Wayland (spec §1, criterion 5).** At 125 % scale, turn on **Run Electron
  apps on Wayland** and start VS Code or another repo Electron app from Spotlight. It's sharp, and
  `hyprctl clients` shows `xwayland: 0` for it.
- [ ] **7. Sharp X11 apps.** Turn it on. A new `xeyes` or `xterm` is sharp
  (`hyprctl getoption xwayland:force_zero_scaling` is 1). `GDK_SCALE` is in a new app's
  environment (`tr '\0' '\n' < /proc/<pid>/environ | grep GDK_SCALE`) and not in
  `~/.config/environment.d/60-uq.conf`. Record whether GTK Wayland apps changed size (the
  R12/R14 live item; `GDK_SCALE` is a whole number).
- [ ] **8. Fix this app.** Open an X11-only app. It's listed under **Blurry right now**. Fix it
  with Run on X11 and size 150 %, close it and open it again from the launcher: its environment
  has `QT_SCALE_FACTOR=1.5` and `GDK_SCALE=1`. `uqctl launch <id>` does the same, and so does the
  app when it's added to Startup.
- [ ] **9. Other launchers.** Turn on "Also when started from other launchers". Check that
  `~/.local/share/applications/<id>.desktop` has `X-uQ-Override=true` and an `env --` `Exec`, and
  that another launcher (`gtk-launch <id>`) gets the fix. Stop uqd and launch from Spotlight: the
  fix still applies once, not twice (R19). Remove the fix: the file is gone. Put your own copy
  there first, and the switch is refused with a plain message.
- [ ] **10. Chromium.** With Chromium installed, turning on **Run Chromium-based browsers on
  Wayland** writes `~/.config/chromium-flags.conf` with the marker first line. With your own file
  there, it's left alone and the page says so. Record which flag files each installed browser
  really reads (Chrome, Brave, Vivaldi, Edge), and whether `--ozone-platform-hint=auto` still
  works in the Chromium you have.
- [ ] **11. Environment variables.** Add `FOO=bar`. A new terminal from Spotlight has it, and
  `systemctl --user show-environment` has it (R8). After you log out and in,
  `~/.config/environment.d/60-uq.conf` provides it. `XDG_SESSION_TYPE`, `XDG_CONFIG_HOME` and a
  value of `A$B` are refused plainly. Remove `FOO`: a terminal that was already open still has it,
  as the page says. Add `hl.env("FOO", "x")` to `user.lua` and reload: the row says "Overridden in
  user.lua, line N". `user.lua`'s modification time is unchanged by every step of this checklist.
- [ ] **12. History and restore.** Undo a switch, a fix and a startup toggle from **History**.
  Each row reads plainly and the files follow (the override and the hidden copy appear and go).
  Restore yesterday's point, and all three pages match it.
- [ ] **13. Profiles.** Make a profile "Work" from the Startup and Apps and scaling pages (a
  variable, a switch, a fix's size, a startup entry off). Apply it: rows say "From the Work
  profile", History shows the changes "from the Work profile", a new terminal has the variable,
  nothing starts or stops, and the notice says startup apps change at next login. **Back to
  normal** puts everything back. Import a hand-edited profile with `argv = ["x"]` under
  `[autostart."a-1234"]`: it's refused plainly. A profile with a startup entry this computer
  hasn't is applied with "isn't on this computer, so it was left out".
- [ ] **14. Dev session.** In `uq-dev session`, no autostart app starts, the Startup page says
  "Not started in a test session", and "Also when started from other launchers" is refused with
  "Not available in a test session". The real `~/.local/share/applications` is untouched.
- [ ] **15. Flatpak launchers.** Install a Chromium-based browser from Flatpak (its exported
  `Exec` ends in `--file-forwarding <app-id> @@u %U @@`). Give it a fix with an option such as
  `--ozone-platform=x11` and turn on "Also when started from other launchers". Look at the
  override's `Exec`: uQ puts the browser flags and options just before `%U`, so they land inside
  Flatpak's `@@u … @@` forwarding block. Start the browser from another launcher, with and
  without a URL, and record what happens: whether the option reaches the browser, is treated as
  a file or URL to open, or makes the start fail. Don't assume it works; this is unverified.
