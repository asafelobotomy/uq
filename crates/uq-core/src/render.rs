//! Render a theme's tokens into every app's config format (spec §2.5).

use crate::theme::Theme;
use minijinja::{Environment, UndefinedBehavior, context};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Rendered output, keyed by path relative to the theme directory.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedFiles(pub BTreeMap<PathBuf, String>);

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    #[error("template {name}: {source}")]
    Read {
        name: String,
        source: std::io::Error,
    },
    #[error("unknown Kvantum base {0:?}")]
    BadKvantumBase(String),
    #[error("template {name}: {message}")]
    Template { name: String, message: String },
    #[error("{path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

/// (template file in `templates_dir`, output path in the theme directory)
pub const TARGETS: [(&str, &str); 7] = [
    ("tokens.json.j2", "tokens.json"),
    ("qt6ct.conf.j2", "qt6ct/colors/uq.conf"),
    ("gtk3.css.j2", "gtk-3.0/gtk.css"),
    ("gtk4.css.j2", "gtk-4.0/gtk.css"),
    ("foot.ini.j2", "terminal/foot.ini"),
    ("kitty.conf.j2", "terminal/kitty.conf"),
    ("alacritty.toml.j2", "terminal/alacritty.toml"),
];

fn bare(s: String) -> String {
    s.trim_start_matches('#').to_string()
}

/// `#rrggbb` → `#ffrrggbb` (Qt's ARGB form).
fn argb(s: String) -> String {
    format!("#ff{}", s.trim_start_matches('#'))
}

/// Kvantum bases uQ's theme can inherit from, in preference order.
pub const KVANTUM_BASES: [&str; 4] = ["KvGnome", "KvAdapta", "KvArc", "KvFlat"];

/// The environment file uQ's own Quickshell units (`uq-shell`, `uq-lock`) load through
/// `EnvironmentFile=`, relative to the theme directory.
pub const SHELL_ENV: &str = "shell.env";

/// `shell.env`: the icon theme for uQ's Quickshell processes. The session runs Qt through qt6ct,
/// which knows no icon theme until the user's own qt6ct.conf names one (`uqctl setup --apps`
/// doesn't), so without this the shell finds no `audio-volume-*`, `battery-*` or fallback app
/// icons at all. Quickshell reads `QS_ICON_THEME` once, at start. A name that isn't a valid theme
/// name (an imported theme could hold anything) writes no variable: a stray newline or quote must
/// never reach a systemd environment file.
pub fn shell_env(theme: &Theme) -> String {
    let name = &theme.icons.theme;
    let mut out = String::from("# Written by uqd from the theme's icons; edits are replaced.\n");
    if crate::settings::valid_theme_name(name) {
        out.push_str(&format!("QS_ICON_THEME=\"{name}\"\n"));
    }
    out
}

/// Template and output path of the Kvantum theme, rendered only when a base exists.
const KVANTUM_TARGET: (&str, &str) = ("kvantum.kvconfig.j2", "Kvantum/uq/uq.kvconfig");

/// The first of [`KVANTUM_BASES`] installed under `dir` (`dir/<name>/<name>.kvconfig`).
pub fn kvantum_base(dir: &Path) -> Option<&'static str> {
    KVANTUM_BASES
        .into_iter()
        .find(|n| dir.join(n).join(format!("{n}.kvconfig")).is_file())
}

pub fn render(
    theme: &Theme,
    templates_dir: &Path,
    kvantum_base: Option<&'static str>,
) -> Result<RenderedFiles, RenderError> {
    if let Some(b) = kvantum_base
        && !KVANTUM_BASES.contains(&b)
    {
        return Err(RenderError::BadKvantumBase(b.to_string()));
    }
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_keep_trailing_newline(true);
    env.add_filter("bare", bare);
    env.add_filter("argb", argb);
    let ctx = context! { theme => theme, variant => theme.variant.as_str(), is_dark => theme.variant == crate::theme::Variant::Dark, base => kvantum_base };
    let mut out = BTreeMap::new();
    let extra = kvantum_base.map(|_| KVANTUM_TARGET);
    for (name, rel) in TARGETS.into_iter().chain(extra) {
        let src = std::fs::read_to_string(templates_dir.join(name)).map_err(|source| {
            RenderError::Read {
                name: name.to_string(),
                source,
            }
        })?;
        let tmpl_err = |e: minijinja::Error| RenderError::Template {
            name: name.to_string(),
            message: format!("{e:#}"),
        };
        let body = env.render_str(&src, &ctx).map_err(tmpl_err)?;
        out.insert(PathBuf::from(rel), body);
    }
    out.insert(PathBuf::from(SHELL_ENV), shell_env(theme));
    Ok(RenderedFiles(out))
}

/// Render every theme variant's `tokens.json` into `out/<id>-<variant>/tokens.json`, for the
/// greeter, which can't read a user's rendered theme (spec phase 5 §2.2). Same renderer and
/// templates as uqd, with no appearance overrides. Returns the files written, sorted.
pub fn prerender_tokens(
    themes: &crate::theme::ThemeRegistry,
    templates_dir: &Path,
    out: &Path,
) -> Result<Vec<PathBuf>, RenderError> {
    let mut written = Vec::new();
    for theme in themes.iter() {
        let files = render(theme, templates_dir, None)?;
        let body = &files.0[Path::new("tokens.json")];
        let dir = out.join(format!("{}-{}", theme.id, theme.variant));
        let path = dir.join("tokens.json");
        let write_err = |source| RenderError::Write {
            path: path.clone(),
            source,
        };
        std::fs::create_dir_all(&dir).map_err(write_err)?;
        std::fs::write(&path, body).map_err(write_err)?;
        written.push(path);
    }
    written.sort();
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::{Variant, tests::FIXTURE};

    fn templates() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../templates")
    }

    fn theme() -> Theme {
        Theme::from_toml(FIXTURE, "fixture", Variant::Dark).unwrap()
    }

    #[test]
    fn kvantum_rendered_only_with_base() {
        let none = render(&theme(), &templates(), None).unwrap();
        assert!(!none.0.contains_key(Path::new("Kvantum/uq/uq.kvconfig")));
        let some = render(&theme(), &templates(), Some("KvArc")).unwrap();
        let body = &some.0[Path::new("Kvantum/uq/uq.kvconfig")];
        assert!(body.starts_with("[%General]\ninherits=KvArc\n"), "{body}");
        assert!(body.contains("[GeneralColors]"), "{body}");
        assert!(body.contains("window.color=#282828"), "{body}");
        assert!(body.contains("highlight.color=#d79921"), "{body}");
        assert!(body.contains("disabled.text.color=#bdae93"), "{body}");
    }

    #[test]
    fn shell_env_names_the_icon_theme() {
        let mut t = theme();
        t.icons.theme = "Papirus-Dark".into();
        let files = render(&t, &templates(), None).unwrap();
        assert_eq!(
            files.0[Path::new(SHELL_ENV)],
            "# Written by uqd from the theme's icons; edits are replaced.\nQS_ICON_THEME=\"Papirus-Dark\"\n"
        );
    }

    #[test]
    fn shell_env_skips_an_unsafe_icon_theme() {
        let mut t = theme();
        for bad in ["Evil\nLD_PRELOAD=/x.so", "a\"b", ""] {
            t.icons.theme = bad.into();
            let env = shell_env(&t);
            assert!(!env.contains("QS_ICON_THEME"), "{bad:?}: {env}");
            assert_eq!(env.lines().count(), 1, "{env}");
        }
    }

    #[test]
    fn kvantum_base_outside_list_is_rejected() {
        let err = render(&theme(), &templates(), Some("Evil\n[x]")).unwrap_err();
        assert!(matches!(err, RenderError::BadKvantumBase(_)), "{err:?}");
    }

    #[test]
    fn kvantum_base_order() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(kvantum_base(d.path()), None);
        for n in ["KvFlat", "KvArc"] {
            std::fs::create_dir(d.path().join(n)).unwrap();
            std::fs::write(d.path().join(n).join(format!("{n}.kvconfig")), "").unwrap();
        }
        // A directory without its .kvconfig doesn't count.
        std::fs::create_dir(d.path().join("KvGnome")).unwrap();
        assert_eq!(kvantum_base(d.path()), Some("KvArc"));
    }
}
