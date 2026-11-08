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

pub fn render(theme: &Theme, templates_dir: &Path) -> Result<RenderedFiles, RenderError> {
    let mut env = Environment::new();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_keep_trailing_newline(true);
    env.add_filter("bare", bare);
    env.add_filter("argb", argb);
    let ctx = context! { theme => theme, variant => theme.variant.as_str(), is_dark => theme.variant == crate::theme::Variant::Dark };
    let mut out = BTreeMap::new();
    for (name, rel) in TARGETS {
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
