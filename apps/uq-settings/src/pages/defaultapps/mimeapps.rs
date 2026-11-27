//! Reading and editing `mimeapps.list` without touching anything but the keys being set: every
//! other line, comment, group and key stays exactly as it was.

pub use uq_core::default_apps::{remove_defaults, set_defaults};
pub use uq_core::mimeapps::default_for;

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
