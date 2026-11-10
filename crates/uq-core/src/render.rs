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
    #[error("template {name}: {message}")]
    Template { name: String, message: String },
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
    kvantum_base: Option<&str>,
) -> Result<RenderedFiles, RenderError> {
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
    Ok(RenderedFiles(out))
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
