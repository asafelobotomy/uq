//! `uqctl wallpaper`: show the wallpaper, or set a picture as it (what an image viewer's "Set as
//! wallpaper" calls). Only the wallpaper section is touched, through the same API Settings uses,
//! so the change shows in Settings and `uqctl undo` takes it back.
//!
//! Every message here is for a person, so none names the background service (K6).

use crate::profile_cmd::describe;
use clap::Subcommand;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};
use uq_core::settings::Fit;

/// The origin uqd's history records for uqctl's writes.
const ORIGIN: &str = "uqctl";
const UNDERSTAND: &str = "Got an answer uqctl doesn't understand.";
/// The entry every display without its own uses.
const ALL: &str = "*";

#[derive(Subcommand, Debug, PartialEq)]
pub enum WallpaperCmd {
    /// Print each display's picture (or folder) and how it's fitted.
    Get,
    /// Use a picture as the wallpaper, on every display or on one. A folder slideshow there
    /// stops.
    Set {
        /// The picture.
        path: PathBuf,
        /// One display (as `hyprctl monitors` names it), not every display.
        #[arg(long)]
        output: Option<String>,
        /// fill, fit, center, tile or span. Kept as it is when left out.
        #[arg(long, value_parser = Fit::NAMES)]
        fit: Option<String>,
    },
}

/// The picture as the setting stores it: absolute, with links and `..` resolved.
fn picture(path: &Path) -> Result<PathBuf, String> {
    let full = path
        .canonicalize()
        .map_err(|e| format!("Can't open {}: {e}", path.display()))?;
    if !full.is_file() {
        return Err(format!("{} isn't a picture file.", path.display()));
    }
    Ok(full)
}

/// The wallpaper section with `image` used on `output` (or every display). Other displays' entries
/// and unknown fields stay as they are. A new entry for one display starts from the fit every
/// display uses.
fn with_image(
    section: &Value,
    image: &Path,
    output: Option<&str>,
    fit: Option<&str>,
) -> Result<Value, String> {
    let mut next = section.clone();
    let root = next.as_object_mut().ok_or(UNDERSTAND)?;
    let outputs = root
        .entry("outputs")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or(UNDERSTAND)?;
    let key = output.unwrap_or(ALL);
    let inherited = outputs
        .get(ALL)
        .and_then(|e| e.get("fit"))
        .cloned()
        .unwrap_or_else(|| json!("fill"));
    let entry = outputs
        .entry(key)
        .or_insert_with(|| json!({ "fit": inherited }))
        .as_object_mut()
        .ok_or(UNDERSTAND)?;
    let image = image
        .to_str()
        .ok_or("The picture's path isn't valid text.")?;
    entry.insert("image".into(), json!(image));
    entry.remove("folder");
    if let Some(fit) = fit {
        entry.insert("fit".into(), json!(fit));
    }
    Ok(next)
}

/// One line per display: name, then the picture or folder, then the fit.
fn lines(section: &Value) -> Result<Vec<String>, String> {
    let outputs = section
        .get("outputs")
        .and_then(Value::as_object)
        .ok_or(UNDERSTAND)?;
    let text = |e: &Value, k: &str| e.get(k).and_then(Value::as_str).map(str::to_string);
    let mut out: Vec<String> = outputs
        .iter()
        .map(|(name, e)| {
            let what = text(e, "image")
                .or_else(|| text(e, "folder").map(|f| format!("{f}/ (slideshow)")))
                .or_else(|| text(e, "color"))
                .unwrap_or_else(|| "(theme)".into());
            let fit = text(e, "fit").unwrap_or_else(|| "fill".into());
            let name = if name == ALL { "every display" } else { name };
            format!("{name}\t{what}\t{fit}")
        })
        .collect();
    if out.is_empty() {
        out.push("every display\t(theme)\tfill".into());
    }
    Ok(out)
}

pub async fn run(proxy: &zbus::Proxy<'_>, cmd: WallpaperCmd) -> Result<Vec<String>, String> {
    let got: String = proxy
        .call("GetSection", &("wallpaper",))
        .await
        .map_err(describe)?;
    let section: Value = serde_json::from_str(&got).map_err(|_| UNDERSTAND.to_string())?;
    match cmd {
        WallpaperCmd::Get => lines(&section),
        WallpaperCmd::Set { path, output, fit } => {
            let image = picture(&path)?;
            let next = with_image(&section, &image, output.as_deref(), fit.as_deref())?;
            if next != section {
                proxy
                    .call::<_, _, ()>("SetSection", &("wallpaper", next.to_string(), ORIGIN))
                    .await
                    .map_err(describe)?;
            }
            Ok(vec![])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Cli, Cmd};
    use clap::Parser;

    fn parse(args: &[&str]) -> Result<WallpaperCmd, clap::Error> {
        let mut v = vec!["uqctl", "wallpaper"];
        v.extend_from_slice(args);
        Cli::try_parse_from(v).map(|c| match c.cmd {
            Cmd::Wallpaper { cmd } => cmd,
            _ => unreachable!(),
        })
    }

    #[test]
    fn parses_set() {
        assert_eq!(
            parse(&["set", "--", "-odd.png"]).unwrap(),
            WallpaperCmd::Set {
                path: "-odd.png".into(),
                output: None,
                fit: None
            }
        );
        assert_eq!(
            parse(&["set", "--output", "DP-1", "--fit", "fit", "/a.png"]).unwrap(),
            WallpaperCmd::Set {
                path: "/a.png".into(),
                output: Some("DP-1".into()),
                fit: Some("fit".into())
            }
        );
        assert!(parse(&["set", "--fit", "stretch", "/a.png"]).is_err());
        assert!(parse(&["set"]).is_err());
    }

    #[test]
    fn sets_every_display_and_stops_the_slideshow() {
        let s = json!({"outputs": {"*": {"folder": "/p", "fit": "fit", "interval": 60},
                                    "DP-1": {"image": "/x.png", "fit": "fill"}}});
        let n = with_image(&s, Path::new("/new.jpg"), None, None).unwrap();
        assert_eq!(
            n,
            json!({"outputs": {"*": {"image": "/new.jpg", "fit": "fit", "interval": 60},
                                "DP-1": {"image": "/x.png", "fit": "fill"}}})
        );
    }

    #[test]
    fn a_new_display_entry_takes_the_shared_fit() {
        let s = json!({"outputs": {"*": {"image": "/a.png", "fit": "center"}}});
        let n = with_image(&s, Path::new("/b.png"), Some("HDMI-A-1"), None).unwrap();
        assert_eq!(
            n["outputs"]["HDMI-A-1"],
            json!({"image": "/b.png", "fit": "center"})
        );
        let n = with_image(&json!({}), Path::new("/b.png"), None, Some("tile")).unwrap();
        assert_eq!(
            n,
            json!({"outputs": {"*": {"image": "/b.png", "fit": "tile"}}})
        );
    }

    #[test]
    fn rejects_an_odd_section() {
        assert!(with_image(&json!([]), Path::new("/a"), None, None).is_err());
        assert!(with_image(&json!({"outputs": 3}), Path::new("/a"), None, None).is_err());
    }

    #[test]
    fn picture_must_be_a_file() {
        let dir = std::env::temp_dir();
        assert!(picture(&dir).is_err());
        assert!(picture(Path::new("/no/such/picture.png")).is_err());
    }

    #[test]
    fn lists_displays() {
        let s = json!({"outputs": {"*": {"folder": "/p", "fit": "fill"},
                                    "DP-1": {"image": "/x.png", "fit": "fit"}}});
        assert_eq!(
            lines(&s).unwrap(),
            vec!["every display\t/p/ (slideshow)\tfill", "DP-1\t/x.png\tfit"]
        );
        assert_eq!(
            lines(&json!({"outputs": {}})).unwrap(),
            vec!["every display\t(theme)\tfill"]
        );
    }
}
