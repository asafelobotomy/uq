# Phase 5 (uQ session, login screen, Welcome, installer hand-off): manual live check

> Needs a real machine with greetd. Do it on the fresh install (docs/fresh-install-plan.md §1),
> because most items happen only once. Keep a TTY ready (`CTRL+ALT+F3`). Tick each item.

## Session and first login

- [ ] **1. Session entry.** Before uQ's login screen is on, SDDM lists a "uQ" session. Its first
  login sets uQ up without `uqctl setup`; `~/.local/state/uq/session.log` has no problem
  lines.
- [ ] **2. Hand-off.** On a bindle install, bindle's first-login window shows and Welcome waits
  (`$XDG_RUNTIME_DIR/uq/welcome.defer` exists). Close bindle's window: Welcome opens, once.
- [ ] **3. Stale defer.** `touch -d '-31 min' $XDG_RUNTIME_DIR/uq/welcome.defer` and
  `uqctl welcome --reset`, then log in again: Welcome shows anyway.

## Welcome

- [ ] **4. Steps.** Go through every step: the theme (with automatic light and dark), wallpaper,
  window arrangement, screens (only with two connected), the installed uQ apps and where to get
  the others, and the fingerprint (only with a reader and no finger saved). Each choice applies
  at once.
- [ ] **5. Once.** Close it halfway: it doesn't come back at the next login. Settings → About →
  Show Welcome and `uqctl welcome --reset` bring it back.

## Login screen

- [ ] **6. Turn on.** `run0 uqctl greeter enable`, restart: uQ's login screen shows in the chosen
  theme and wallpaper, with users and pictures and the uQ session first.
- [ ] **7. Messages.** A wrong password shows a plain message; after the faillock limit the
  account says it's locked. An expired password (`sudo chage -d 0 <user>`) asks for a new one
  twice and checks they match.
- [ ] **8. Power buttons.** Suspend, Restart and Shut down each need two clicks.
- [ ] **9. Fallback.** Make the greeter fail to start 3 times in a minute (for example, move
  `/usr/share/uq/greeter/shell.qml` aside): greetd's text login takes over, and you can still
  log in. Put the file back.
- [ ] **10. Privacy.** While the login screen shows, from a TTY: it runs as the `greeter` user
  (`ps -o user=,args= -u greeter`), and `sudo lsof -u greeter | grep /home` prints nothing.
- [ ] **11. Turn off.** `run0 uqctl greeter disable`, restart: the display manager it replaced is
  back.
