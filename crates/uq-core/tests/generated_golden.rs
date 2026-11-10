use std::collections::BTreeMap;
use std::path::PathBuf;
use uq_core::lua::{Layout, emit_generated};
use uq_core::settings::{ForceSplit, NewStatus, Orientation, Settings};
use uq_core::theme::{ThemeRegistry, Variant};

/// Every look and layout option away from "unset".
fn all_set() -> Settings {
    let mut s = Settings::default();
    s.look.gaps_in = Some(8);
    s.look.gaps_out = Some(20);
    s.look.border_width = Some(3);
    s.look.rounding = Some(14);
    s.look.blur = Some(true);
    s.look.blur_size = Some(6);
    s.look.blur_passes = Some(3);
    s.look.shadow = Some(false);
    s.look.animations = Some(true);
    s.look.animation_speed = Some(1.5);
    let lo = &mut s.layout_options;
    lo.default = Some(Layout::Master);
    lo.dwindle.preserve_split = Some(true);
    lo.dwindle.smart_split = Some(false);
    lo.dwindle.force_split = Some(ForceSplit::Right);
    lo.master.mfact = Some(0.6);
    lo.master.new_status = Some(NewStatus::Inherit);
    lo.master.orientation = Some(Orientation::Right);
    lo.scrolling.column_width = Some(0.7);
    lo.scrolling.fullscreen_on_one_column = Some(true);
    s
}

#[test]
fn golden_default_and_all_set() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let reg = ThemeRegistry::load_dir(&root.join("../../themes")).unwrap();
    let theme = reg.get("graphite", Variant::Dark).unwrap();
    let rules = BTreeMap::from([("2".to_string(), Layout::Scrolling)]);
    let bless = std::env::var_os("UQ_BLESS").is_some();
    let mut bad = Vec::new();
    for (name, settings) in [("default", Settings::default()), ("all-set", all_set())] {
        let out = emit_generated(theme, &rules, &settings);
        let path = root.join(format!("tests/golden/generated/{name}.lua"));
        if bless {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &out).unwrap();
        } else if std::fs::read_to_string(&path).ok().as_deref() != Some(out.as_str()) {
            bad.push(path.display().to_string());
        }
    }
    assert!(
        bad.is_empty(),
        "golden mismatch (UQ_BLESS=1 to update):\n{}",
        bad.join("\n")
    );
}
