# Settings slice B (profiles): manual live check

Run on a real Hyprland session with uQ installed (not CI, not a private bus). Tick each item.
`uqctl` here means the installed one; the app is `uq-settings` (`uqctl settings profiles`).
Have `power-profiles-daemon` running and a laptop or machine that offers a Performance mode.

- [ ] **1. Create and turn on.** On **Look**, set gaps between windows to 0 and animations off.
  On **Profiles**, press **New profile**, name it "Gaming", keep "Settings I changed" and press
  **Create**. Then add a power mode, which the app can't do: `uqctl profile export gaming
  /tmp/g.toml`, add these two lines at the end of the file:
  ```toml
  [power]
  profile = "performance"
  ```
  and run `uqctl profile import /tmp/g.toml --replace`. Turn Gaming on from the quick-settings
  **Profile** row (open quick settings, press "Gaming"):
  - gaps and animations change at once;
  - the quick-settings power row shows Performance;
  - **History** lists the changes "from the Gaming profile";
  - a "Before the Gaming profile" restore point exists;
  - the Look rows say "From the Gaming profile".
- [ ] **2. A hand change survives Back to normal.** While Gaming is on, set gaps to 4 on **Look**,
  then press **Back to normal** (Profiles page or the **Normal** chip). Gaps stay 4 and the message
  names "Gaps between windows"; animations and the power mode come back.
- [ ] **3. Restarts keep it.** Turn Gaming on, then `systemctl --user restart uqd`. The quick
  settings row and the Profiles page still show it on, and **Back to normal** still works.
- [ ] **4. No power service.** Run `sudo systemctl mask --now power-profiles-daemon`, then turn
  Gaming on. Everything else applies, and the message says "Power mode wasn't changed: ...". The
  call returns within about 3 seconds. Afterwards run `sudo systemctl unmask power-profiles-daemon`
  and start it again.
- [ ] **5. Export and import.** Run `uqctl profile export gaming /tmp/g.toml`, then
  `uqctl profile delete gaming` (turn it off first), then `uqctl profile import /tmp/g.toml`: it
  comes back. A second import says it already exists, and `--replace` works. Importing a 1 MiB
  file (`head -c 1048576 /dev/zero > /tmp/big.toml`) says "The file is too big to be a profile.",
  and a file with a `[keybinds]` table is refused with a plain message that says it can't do that
  yet. Nothing is added in either case.
- [ ] **6. `next` and a keybind.** With two profiles, `uqctl profile next` goes first, then
  second, then back to normal (by name, ignoring case). Add this line to
  `~/.config/hypr/uq/user.lua`, reload, and press `SUPER+ALT+P`: it does the same:
  ```lua
  hl.bind("SUPER + ALT + P", hl.dsp.exec_cmd("uqctl profile next"))
  ```
- [ ] **7. Restore ends a profile.** With Gaming on, restore the "Before the Gaming profile" point
  from **History** (or `uqctl restore <id>`). The settings come back and no profile shows as on.
  If the profile set a power mode, the one from before comes back too (unless you changed it by
  hand while the profile was on).
- [ ] **8. Light or dark hold.** Make a profile that holds "Light or dark" (add it with **Add
  settings**) and set a schedule whose next change falls within a few minutes
  (for example `uqctl schedule fixed 07:00 HH:MM`, with HH:MM a few minutes from now). While the profile is on,
  the schedule doesn't switch light or dark, even after HH:MM passes. Click **Back to normal** after
  HH:MM: the variant from before the profile comes back, and within a minute (the schedule checks
  once a minute) it switches to the variant that's due now. Had no change come due while the
  profile was on, nothing would switch until the next one.
