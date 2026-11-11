//! `uqctl render-themes OUT` (hidden; run by the package build): every theme variant's
//! `tokens.json`, rendered by uqd's renderer into `OUT/<id>-<variant>/tokens.json`, so the
//! greeter can follow a theme without reading anyone's home directory.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use uq_core::paths::Paths;
use uq_core::render::prerender_tokens;
use uq_core::theme::ThemeRegistry;

#[derive(clap::Args, Debug)]
pub struct Args {
    /// Where the theme sources are (default: the installed /usr/share/uq/themes).
    #[arg(long)]
    pub themes: Option<PathBuf>,
    /// Where the templates are (default: the installed /usr/share/uq/templates).
    #[arg(long)]
    pub templates: Option<PathBuf>,
    /// The output directory, e.g. $pkgdir/usr/share/uq/themes/rendered.
    pub out: PathBuf,
}

pub fn run(themes: &Path, templates: &Path, out: &Path) -> Result<Vec<PathBuf>, String> {
    let reg = ThemeRegistry::load_dir(themes).map_err(|e| e.to_string())?;
    prerender_tokens(&reg, templates, out).map_err(|e| e.to_string())
}

pub fn main(args: &Args) -> ExitCode {
    let paths = Paths::from_env();
    let themes = args.themes.clone().unwrap_or(paths.themes_dir);
    let templates = args.templates.clone().unwrap_or(paths.templates_dir);
    match run(&themes, &templates, &args.out) {
        Ok(files) => {
            println!(
                "rendered {} theme variants into {}",
                files.len(),
                args.out.display()
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("uqctl: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    #[test]
    fn renders_every_variant() {
        let out = tempfile::tempdir().unwrap();
        let files = run(
            &repo().join("themes"),
            &repo().join("templates"),
            out.path(),
        )
        .unwrap();
        let reg = ThemeRegistry::load_dir(&repo().join("themes")).unwrap();
        assert_eq!(files.len(), reg.iter().count());
        assert!(out.path().join("graphite-dark/tokens.json").is_file());
        assert!(out.path().join("graphite-light/tokens.json").is_file());
    }

    #[test]
    fn missing_themes_is_an_error() {
        let out = tempfile::tempdir().unwrap();
        assert!(
            run(
                Path::new("/nonexistent"),
                &repo().join("templates"),
                out.path()
            )
            .is_err()
        );
    }
}
