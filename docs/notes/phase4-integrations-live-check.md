# Phase 4 (integrations: widgets, app actions, providers, rules from apps): manual live check

> Partly covered by unit and QML tests and `uqctl integrations check`. This list is what needs a
> real session with real apps installed. Tick each item.

- [ ] **1. Inventory.** `uqctl integrations list` shows every widget, settings page, provider,
  app action and rules file that the installed companion apps ship, and says why any is hidden.
- [ ] **2. Bar widget.** Install an app with a widget (scale_tail, scdr): its icon appears in the
  Status island within a few seconds, with no log out. Remove the package: it goes. With more
  than four, the rest sit under "…".
- [ ] **3. Quick-settings tile.** A tile from an app shows in the **Apps** group of quick settings
  and acts.
- [ ] **4. Broken widget.** Put a widget with a QML error in `/usr/share/uq/shell/widgets/`
  (copy `templates/widget/` and break it): it is hidden, the bar stays up, and it's retried a
  minute later.
- [ ] **5. App actions.** Right-click an app in Spotlight and in the app menu: "Uninstall…"
  (crumBS) is listed and opens in a terminal with that app's id. An app action from another
  app runs as its file says.
- [ ] **6. Providers.** With an `Activatable=true` provider installed and not running, open
  Spotlight: it starts within a few seconds (`busctl --user list`), and results from it appear.
  It isn't started again within 30 s, nor while typing.
- [ ] **7. Emoji key.** `Super + .` opens Spotlight at `:`. With emobie installed, emoji results
  show and pasting one works; without it Spotlight says "No emoji provider installed". Pressing
  `Super + .` again starts the search over.
- [ ] **8. Rules from apps.** An app's `rules.toml` shows under **Rules from apps** on the Window
  rules page, off. Turn one on: it applies to that app's window and History has a row that
  undoes it. Remove the app: the rule stops applying; reinstall: it's back.
- [ ] **9. Web apps.** An app using `uq-tokens` (gutcheck, emobie) follows a theme change live.
- [ ] **10. Failure messages.** Point a launcher entry at a missing program and open it: Spotlight
  stays open and says why.
