//! The commit sequence: persist, render a candidate, verify, install, reload.

use super::*;

pub(crate) const USER_LUA_STUB: &str =
    "-- Your own Hyprland settings. uQ reads this file last and never rewrites it.\n";

/// With uQ's login screen installed, match its theme to the desktop's: `uqctl greeter sync-theme`
/// asks the root helper, which polkit lets an active local administrator do without a prompt.
/// Best effort and in the background; a theme the login screen doesn't ship (your own) or a
/// missing helper just leaves the login screen as it was.
pub(crate) fn sync_login_theme(name: &str, variant: Variant) {
    if !std::path::Path::new("/etc/uq/greeter.toml").is_file() {
        return;
    }
    let (name, variant) = (name.to_string(), variant.as_str());
    tokio::spawn(async move {
        let _ = tokio::process::Command::new("uqctl")
            .args([
                "greeter",
                "sync-theme",
                "--theme",
                &name,
                "--variant",
                variant,
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .kill_on_drop(false)
            .status()
            .await;
    });
}

impl Desktop {
    /// After a `Change::Theme` commit, which announces only the theme: every section and the
    /// settings may have changed too.
    pub(crate) fn announce_all(&self) {
        for domain in SECTION_DOMAINS {
            if let Ok(json) = self.get_section(domain) {
                let _ = self.events.send(Event::SectionChanged {
                    domain: domain.to_string(),
                    json,
                });
            }
        }
        let _ = self.events.send(Event::SettingsChanged(Box::new(
            self.state.settings.clone(),
        )));
    }

    pub(crate) fn ensure_user_lua(&self) -> Result<(), DesktopError> {
        let p = &self.paths.user_lua;
        if p.exists() {
            return Ok(());
        }
        atomic_write(p, USER_LUA_STUB).map_err(io_err("creating user.lua"))
    }

    /// The one save point: write `state.toml`, adopt `next`, and record what changed. Journal and
    /// restore-point failures are logged and never fail the write.
    pub(crate) fn persist(&mut self, next: State, origin: Origin) -> Result<(), DesktopError> {
        next.save(&self.paths.state_file)
            .map_err(io_err("saving state"))?;
        let old = std::mem::replace(&mut self.state, next);
        self.sync_files(&old);
        // A schedule tick keeps the last entry ids, so it never moves the undo target.
        // A hotplug switch is automatic, like a schedule flip: never journalled, no restore point.
        if matches!(origin, Origin::Schedule | Origin::Hotplug) {
            return Ok(());
        }
        self.last_entry_ids.clear();
        let changes = diff(&old, &self.state, &uq_core::history::label);
        if changes.is_empty() {
            return Ok(());
        }
        let now = Utc::now().timestamp();
        let today = Local
            .timestamp_opt(now, 0)
            .earliest()
            .map(|t| t.date_naive());
        let have_today = self.restore.last_auto(AutoReason::Daily).is_some_and(|p| {
            Local
                .timestamp_opt(p.time, 0)
                .earliest()
                .map(|t| t.date_naive())
                == today
        });
        if !have_today
            && let Err(e) = self.restore.save(
                &old,
                "Before today's changes",
                PointKind::Auto(AutoReason::Daily),
                now,
            )
        {
            tracing::warn!("[STATE-005] saving daily restore point: {e}");
        }
        let origin = origin.as_str();
        let entries: Vec<Entry> = changes
            .into_iter()
            .map(|c| Entry::new(now, origin.clone(), c.domain, c.summary, c.before, c.after))
            .collect();
        match self.journal.append(&entries) {
            Ok(()) => {
                self.last_entry_ids = entries.into_iter().map(|e| e.id).collect();
                let _ = self.events.send(Event::HistoryChanged);
            }
            Err(e) => tracing::warn!("[STATE-006] writing history: {e}"),
        }
        self.journal.prune(now);
        self.restore.prune();
        Ok(())
    }

    /// Validate, then write theme dir + generated.lua + state; then apply live and notify.
    pub(crate) async fn commit(
        &mut self,
        next: State,
        change: Change,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let c = self.render_candidate(&next).await?;
        self.install(c, next, change, origin).await
    }

    /// Render `next`, write the candidate `generated.lua` and have Hyprland verify it. The
    /// candidate stays on disk for `install`; a rejected one is removed.
    pub(crate) async fn render_candidate(&self, next: &State) -> Result<Candidate, DesktopError> {
        let theme = self
            .themes
            .get(&next.theme, next.variant)
            .ok_or_else(|| DesktopError::UnknownTheme(next.theme.clone()))?
            .clone();
        // The user's appearance overrides win over the theme's fonts, cursor and icons.
        let mut theme = theme.with_appearance(&next.settings.appearance);
        // Windows' corners set on the Look page are every app's corners too: the shell, uQ's
        // apps (tokens.json), Qt (Kvantum) and GTK all draw with the radius Hyprland rounds to.
        if let Some(r) = next.settings.look.rounding {
            theme.ui.radius = r;
        }
        let base = if next.settings.appearance.kvantum {
            kvantum_base(Path::new(&self.kvantum_dir))
        } else {
            None
        };
        // tokens.json carries Reduce motion, so uQ's native apps follow it too.
        let motion = Motion::new(next.settings.ui.reduce_motion);
        let files = render_with(&theme, &self.paths.templates_dir, base, motion)
            .map_err(|e| DesktopError::Io(e.to_string()))?;
        // One read of the screens serves the main scale, the plan and the default monitor,
        // and only when one of them is in use (no read otherwise, as before).
        let monitors = &next.settings.monitors;
        let scaled = next.compat.sharp_x11 || next.compat.steam_scale;
        let live = if scaled
            || monitors.primary.is_some()
            || monitors.workspaces.strategy != Strategy::Off
        {
            self.live_outputs_for(next).await
        } else {
            None
        };
        let main_scale = live
            .as_deref()
            .filter(|_| scaled)
            .and_then(|l| plan::main_output(monitors, l))
            .map(|o| o.scale);
        let startup = StartupRender {
            monitors: live
                .as_deref()
                .map(|l| monitor_render(monitors, l))
                .unwrap_or_default(),
            ..startup_render(next, main_scale, self.dev_session)
        };
        let generated = emit_generated_with_apps(
            &theme,
            &next.layouts,
            &next.rules,
            &self.app_rules_for(next),
            &next.keybinds,
            &next.settings,
            &startup,
        );

        self.ensure_user_lua()?;
        let path = self
            .paths
            .generated_lua
            .with_file_name("generated.lua.candidate");
        // The verify config sits beside the real hyprland.lua and mirrors the stub, so relative
        // requires in user.lua resolve exactly as they do in the session.
        let verify_main = self
            .paths
            .hypr_dir
            .join(format!(".uq-verify-{}.lua", std::process::id()));
        atomic_write(&path, &generated).map_err(io_err("writing candidate config"))?;
        let main_src = format!(
            "require({})\nrequire(\"./uq/generated.lua.candidate\")\nrequire(\"./uq/user.lua\")\n",
            lua_string(&self.paths.base_lua.to_string_lossy())
        );
        let verdict = match atomic_write(&verify_main, &main_src) {
            Ok(()) => self.comp.verify(&verify_main).await,
            Err(e) => Err(CompositorError::Io(e)),
        };
        let _ = std::fs::remove_file(&verify_main);
        match verdict {
            Ok(()) => {}
            Err(CompositorError::Rejected(msg)) => {
                let _ = std::fs::remove_file(&path);
                let _ = self.events.send(Event::ConfigError(msg.clone()));
                return Err(DesktopError::ConfigInvalid(msg));
            }
            Err(e) => {
                tracing::warn!("[HYPR-004] could not validate config ({e}); applying unvalidated")
            }
        }
        Ok(Candidate {
            files,
            generated,
            path,
            theme,
            main_scale,
        })
    }

    /// Pass `shell.terminal` on to KDE apps and xdg-terminal-exec (`uq_core::terminal_pref`).
    /// `previous` is the choice before this change; `None` re-applies the current one.
    pub(crate) fn sync_terminal(&self, previous: Option<&str>) {
        let chosen = self.state.settings.shell.terminal.as_deref();
        let previous = previous.or(chosen);
        let dirs = self.paths.startup.app_dirs();
        if let Err(e) = uq_core::terminal_pref::apply(&self.paths, &dirs, chosen, previous) {
            tracing::warn!("[THEME-003] terminal for other apps: {e}");
        }
    }

    /// Write a verified candidate (theme dir, `generated.lua`, state), then apply `change` live.
    pub(crate) async fn install(
        &mut self,
        c: Candidate,
        next: State,
        change: Change,
        origin: Origin,
    ) -> Result<(), DesktopError> {
        let theme = c.theme.clone();
        // The shell reads its icon theme only at start: note a change to restart it below.
        let shell_env = self.paths.theme_dir.join(uq_core::render::SHELL_ENV);
        let shell_env_changed = std::fs::read_to_string(&shell_env)
            .ok()
            .is_some_and(|old| c.files.0.get(Path::new(uq_core::render::SHELL_ENV)) != Some(&old));
        let committed = atomic_replace_dir(&self.paths.theme_dir, &c.files)
            .map_err(io_err("writing theme files"))
            .and_then(|()| {
                std::fs::rename(&c.path, &self.paths.generated_lua)
                    .map_err(io_err("installing generated.lua"))
            })
            .and_then(|()| {
                self.main_scale = c.main_scale;
                self.persist(next, origin)
            });
        if let Err(e) = committed {
            let _ = std::fs::remove_file(&c.path);
            return Err(e);
        }
        // Every section is rendered into the config just installed, so no rejection is pending.
        self.last_rejected = None;

        match change {
            Change::Quiet => {}
            Change::Layout(ws, layout) => {
                let _ = self.live(self.comp.eval(&emit_workspace_rule(&ws, layout)).await);
                let _ = self.events.send(Event::LayoutChanged {
                    workspace: ws,
                    layout,
                });
                // The Layouts page follows keybind changes through the section.
                if let Ok(v) = self.state.section("layouts") {
                    let _ = self.events.send(Event::SectionChanged {
                        domain: "layouts".to_string(),
                        json: v.to_string(),
                    });
                }
            }
            Change::Reload => {
                let _ = self.live(self.comp.reload().await);
                self.reapply_pending().await;
            }
            Change::Input => {
                let _ = self.live(self.comp.reload().await);
                self.reapply_pending().await;
                let _ = self.events.send(Event::SettingsChanged(Box::new(
                    self.state.settings.clone(),
                )));
            }
            Change::Theme => {
                // gsettings are desktop-wide: only touch them while a Hyprland session is live,
                // so a uqd left running never re-themes another desktop.
                let live = self.live(self.comp.reload().await);
                self.reapply_pending().await;
                if live {
                    if let Err(e) = self.sys.apply(&theme) {
                        tracing::warn!("[THEME-003] system settings: {e}");
                    }
                    // Every Qt and GTK app follows the theme without a manual step: create
                    // the per-app config files that are missing (never replacing one the user
                    // wrote), then write the look into them. Kvantum is used when installed
                    // and not switched off in Settings.
                    let kvantum = self.state.settings.appearance.kvantum
                        && kvantum_base(Path::new(&self.kvantum_dir)).is_some();
                    let theme_apps = self.state.settings.appearance.theme_apps;
                    if theme_apps {
                        self.theme_apps(&theme, kvantum);
                    }
                    // KDE's and xdg-terminal-exec's terminal (Dolphin's "Open Terminal Here",
                    // GTK apps): the one chosen in Settings, else uQ's unless one is named.
                    self.sync_terminal(None);
                    let cursor = self
                        .comp
                        .set_cursor(&theme.icons.cursor_theme, theme.icons.cursor_size)
                        .await;
                    self.live(cursor);
                    if shell_env_changed && let Err(e) = self.sys.restart_shell() {
                        tracing::warn!("[THEME-006] restarting the shell for its new icons: {e}");
                    }
                }
                sync_login_theme(&theme.id, theme.variant);
                let _ = self.events.send(Event::ThemeChanged {
                    name: theme.id.clone(),
                    variant: theme.variant,
                    tokens_path: self.paths.theme_dir.join("tokens.json"),
                });
            }
        }
        Ok(())
    }

    /// Report the outcome of a live-apply call; the change is already persisted either way.
    /// Returns whether a live Hyprland answered.
    /// Other apps' look ("Theme other apps"): create the per-app config files that are missing,
    /// put uQ's keys back into those another app rewrote (keeping a copy), then write the theme's
    /// colours, icons, fonts and style into them. Kvantum is used when installed and switched on.
    pub(crate) fn theme_apps(&self, theme: &uq_core::theme::Theme, kvantum: bool) {
        if let Err(e) = uq_core::stub::install_consumers(&self.paths, kvantum) {
            tracing::warn!("[THEME-003] app theme files: {e}");
        }
        match uq_core::reclaim::reclaim(&self.paths, kvantum) {
            Ok(changed) => {
                for f in changed {
                    tracing::info!("{} had lost uQ's theme; put it back", f.display());
                }
            }
            Err(e) => tracing::warn!("[THEME-003] app theme files: {e}"),
        }
        // Qt's icons, fonts and palette marker, in a qt6ct.conf uQ wrote.
        if let Err(e) = uq_core::qt6ct::sync(&self.paths, theme) {
            tracing::warn!("[THEME-003] qt6ct look: {e}");
        }
        // KDE apps (Dolphin, Kirigami) read colours, icons, style and fonts from
        // kdeglobals through the `kde` platform theme.
        let look = uq_core::kdeglobals::Look {
            icons: &theme.icons.theme,
            ui_font: &theme.fonts.ui,
            mono_font: &theme.fonts.mono,
            size: theme.fonts.size,
            style: if kvantum { "kvantum" } else { "Fusion" },
        };
        match uq_core::kdeglobals::sync_look(&self.paths, &look, true) {
            // Open KDE/Qt windows reload kdeglobals only when told to.
            Ok(true) => {
                if let Err(e) = self.sys.notify_kde_apps() {
                    tracing::warn!("[THEME-003] telling KDE apps about the new look: {e}");
                }
            }
            Ok(false) => {}
            Err(e) => tracing::warn!("[THEME-003] kdeglobals look: {e}"),
        }
        // Flatpak apps: read-only access to the theme files, so they follow too.
        if let Err(e) = uq_core::flatpak::sync(&self.paths) {
            tracing::warn!("[THEME-003] flatpak override: {e}");
        }
    }

    pub(crate) fn live(&self, res: Result<(), CompositorError>) -> bool {
        match res {
            Ok(()) => true,
            Err(CompositorError::NotRunning) => {
                tracing::warn!("[HYPR-002] Hyprland not running; change saved for next start");
                false
            }
            Err(CompositorError::Rejected(msg)) => {
                tracing::warn!("[HYPR-001] Hyprland rejected live change: {msg}");
                let _ = self.events.send(Event::ConfigError(msg));
                true
            }
            Err(e) => {
                tracing::warn!("[HYPR-003] Hyprland: {e}");
                false
            }
        }
    }
}
