# Settings slice 6 (Power, Users, Login screen, Date and time, Region, Default apps, About): manual live check

> Not automated yet. Run on a real uQ session. Items that need an administrator ask for your
> password through polkit; cancelling must leave the setting as it was. Tick each item.

## Power and the settings helper

- [ ] **1. Power mode.** Switch Power saver / Balanced / Performance on the page and from
  `powerprofilesctl`: each shows on the other at once.
- [ ] **2. Battery.** On a laptop, unplug and plug the charger: charge, time left and health
  update without reopening the page.
- [ ] **3. Lid and power button.** Change what closing the lid (on battery and plugged in) and
  the power button do. Cancel the password once: nothing changes. Confirm: the file under
  `/etc/systemd/logind.conf.d/` is written, and after a restart the lid does what you chose.
- [ ] **4. Helper idle.** `systemctl status uq-settings-helper` shows it stopped about 30 s after
  the last change.

## Users

- [ ] **5. Your own account.** Change your picture and name. Change your password: it asks for
  the current one, and a weak or wrong one is refused with a plain reason.
- [ ] **6. Another person.** Add a standard user, set their password, make them an administrator
  and back, then remove them (typing the user name), once keeping their files and once not.
- [ ] **7. Refusals.** You can't remove yourself, remove the last administrator, or make the last
  administrator standard.

## Login screen (only when uQ is the login screen)

- [ ] **8. Look.** Set its theme ("same as mine" and another), a wallpaper (yours, a picture,
  none) and hide the user list. Log out: the login screen shows each.
- [ ] **9. Display layout.** **Use my display layout at login** with two screens: the login screen
  uses it. **Use automatic layout** puts it back.
- [ ] **10. Auto-login.** Turn it on (it warns, and asks for the password), restart: you're logged
  in. Turn it off. Remove the auto-login user from Users: auto-login turns off.
- [ ] **11. No helper.** Stop and mask `uq-settings-helper`: the page is view-only and says why.
  Unmask it afterwards.

## Date and time, Region, Default apps, About

- [ ] **12. Time.** Change the time zone; turn network time off, set the time by hand, turn it
  back on. The bar's clock follows, and the 24-hour switch changes it.
- [ ] **13. Region.** Change formats (dates, numbers) and check an app started afterwards uses
  them; change the console keyboard layout and check a TTY (`CTRL+ALT+F3`).
- [ ] **14. Default apps.** Change the browser and the PDF viewer. `xdg-open` a link and a PDF:
  they open in the new choices. Only those lines changed in `~/.config/mimeapps.list`.
- [ ] **15. About.** Versions of uQ, Hyprland and Quickshell match `pacman -Q`. Renaming the
  computer asks for the password and `hostnamectl` shows the new name. **Show Welcome** opens it.
