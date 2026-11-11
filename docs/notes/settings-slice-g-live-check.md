# Settings slice G (your own themes): manual live check

Run on a real Hyprland session with uQ installed (and a second machine or home for item 1). Not
CI. Tick each item. Before you start, note `user.lua`'s modification time:
`stat -c %y ~/.config/hypr/uq/user.lua`. It must be the same at the end (item 10).

- [ ] **1. Spec §1 criterion 7.** On machine A run `uqctl theme export gruvbox ~/g.uqtheme
  --with-look`, and from the app use **Export…** on one of your own themes. Copy both to
  machine B and use **Import…** in the app. Each appears in the Appearance theme list and in
  quick settings at once, with no relog. **Use it now** re-themes the bar, a new kitty, GTK and
  Qt apps.
- [ ] **2. Restore point.** History shows a "Before adding the … theme" point. Restoring it
  removes nothing from the theme folder, and the settings match.
- [ ] **3. Look.** Import with the box ticked: gaps and corners change. One History row says so,
  and Ctrl+Z puts them back.
- [ ] **4. Editor.** Open **New theme from this one…** and set Dark text to the dark background:
  the editor says the text is hard to read and Save is greyed. Fix it and save "Ocean":
  `~/.local/share/uq/themes/user-ocean/{dark,light}.toml` exist and Ocean is in the list. Edit
  Ocean while it is active: the desktop changes live. Click into a colour box: its text stays.
- [ ] **5. Hostile files.** Import each file in `crates/uq-core/tests/fixtures/uqtheme/bad/`.
  Each is refused with one plain sentence shown on the page, and no folder appears. A FIFO named
  `x.uqtheme` (`mkfifo`) doesn't hang the dialog.
- [ ] **6. Links.** `ln -s /etc ~/.local/share/uq/themes/user-etc`: nothing new in the list (the "couldn't be used" line says it is a link).
  Replace the themes folder with a symlink: the page says the folder is a link, and imports are
  refused.
- [ ] **7. By hand.** Copy a good theme folder in with `cp -r`. It appears within 60 s, or at
  once when the Appearance page is opened.
- [ ] **8. Delete.** Delete the active user theme: uQ switches to the default first, and
  History shows the switch. A profile that used it is named in the result. Applying that profile
  says "That theme isn't installed any more." (so does Ctrl+Z on the delete's switch).
- [ ] **9. Gone while active.** `rm -r` the active theme's folder. The list says its files were
  removed, other settings still save, and after `systemctl --user restart uqd` the default
  theme is in use.
- [ ] **10. Criterion 8.** `stat ~/.config/hypr/uq/user.lua` is unchanged by every step.
- [ ] **11. Dev session.** In `uq-dev session`, Import and Delete say "Not available in a test
  session".
- [ ] **12. VM.** `tests/vm/run.sh --only 'T[0-9]'` passes (checks T1 to T3 of
  `tests/vm/checks/60-themes.sh`).

## Results

Not run yet. Tests that ran in the build container (no Qt, no VM): `uqtheme_corpus` (7 good fixtures, 29
hostile files, a 65 537-byte file and a FIFO), `theme_cli` (two homes, the corpus through `uqctl`), and the
page logic through the scratch crate. Items 1 to 12 above still need the real desktop or the VM.
