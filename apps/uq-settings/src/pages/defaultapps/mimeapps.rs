//! Reading and editing `mimeapps.list` without touching anything but the keys being set: every
//! other line, comment, group and key stays exactly as it was.

pub use uq_core::mimeapps::{GROUP, default_for};

/// `text` with each of `mimes` set to `id` in `[Default Applications]`: an existing key is
/// replaced where it is (a repeat of it is dropped), a missing one is added at the end of the
/// group, and the group is added at the end when there is none.
pub fn set_defaults(text: &str, mimes: &[&str], id: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut inside = false;
    let mut seen_group = false;
    let mut done: Vec<&str> = Vec::new();
    // Where the group's last non-blank line is, to add missing keys after it.
    let mut group_end: Option<usize> = None;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            inside = t == GROUP;
            if inside {
                seen_group = true;
                group_end = Some(out.len() + 1);
            }
            out.push(line.to_string());
            continue;
        }
        if inside
            && !t.starts_with('#')
            && let Some((k, _)) = t.split_once('=')
            && let Some(m) = mimes.iter().find(|m| **m == k.trim())
        {
            if done.contains(m) {
                continue;
            }
            done.push(m);
            out.push(format!("{m}={id};"));
            group_end = Some(out.len());
            continue;
        }
        out.push(line.to_string());
        if inside && !t.is_empty() {
            group_end = Some(out.len());
        }
    }
    let missing: Vec<String> = mimes
        .iter()
        .filter(|m| !done.contains(m))
        .map(|m| format!("{m}={id};"))
        .collect();
    if !missing.is_empty() {
        match (seen_group, group_end) {
            (true, Some(at)) => {
                for (i, l) in missing.into_iter().enumerate() {
                    out.insert(at + i, l);
                }
            }
            _ => {
                if out.last().is_some_and(|l| !l.trim().is_empty()) {
                    out.push(String::new());
                }
                out.push(GROUP.to_string());
                out.extend(missing);
            }
        }
    }
    let mut s = out.join("\n");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE: &str = "# my notes\n[Added Associations]\ntext/plain=gedit.desktop;\n\n[Default Applications]\ntext/plain=nano.desktop;vim.desktop;\nimage/png=feh.desktop;\n# keep me\n\n[Removed Associations]\nimage/png=gimp.desktop;\n";

    #[test]
    fn reads_only_the_default_group() {
        assert_eq!(
            default_for(FILE, "text/plain").as_deref(),
            Some("nano.desktop")
        );
        assert_eq!(
            default_for(FILE, "image/png").as_deref(),
            Some("feh.desktop")
        );
        assert_eq!(default_for(FILE, "image/jpeg"), None);
        assert_eq!(default_for("", "text/plain"), None);
    }

    #[test]
    fn replaces_in_place_and_adds_in_the_group() {
        let out = set_defaults(FILE, &["text/plain", "text/x-log"], "kate.desktop");
        assert_eq!(
            out,
            "# my notes\n[Added Associations]\ntext/plain=gedit.desktop;\n\n[Default Applications]\ntext/plain=kate.desktop;\nimage/png=feh.desktop;\n# keep me\ntext/x-log=kate.desktop;\n\n[Removed Associations]\nimage/png=gimp.desktop;\n"
        );
        assert_eq!(
            default_for(&out, "text/x-log").as_deref(),
            Some("kate.desktop")
        );
    }

    #[test]
    fn adds_the_group_when_missing_and_drops_repeats() {
        assert_eq!(
            set_defaults(
                "[Added Associations]\na/b=x.desktop;",
                &["a/b"],
                "y.desktop"
            ),
            "[Added Associations]\na/b=x.desktop;\n\n[Default Applications]\na/b=y.desktop;\n"
        );
        assert_eq!(
            set_defaults("", &["a/b"], "y.desktop"),
            "[Default Applications]\na/b=y.desktop;\n"
        );
        assert_eq!(
            set_defaults(
                "[Default Applications]\na/b=1;\na/b=2;\n",
                &["a/b"],
                "y.desktop"
            ),
            "[Default Applications]\na/b=y.desktop;\n"
        );
    }
}
