use super::swatches;

#[test]
fn swatches_come_from_the_rendered_variant() {
    let dir = std::env::temp_dir().join(format!("uq-swatches-{}", std::process::id()));
    let d = dir.join("forest-dark");
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(
        d.join("tokens.json"),
        r##"{"colors":{"bg":"#111111","surface":"#222222","text":"#eeeeee","accent":"#7fb069","success":"#00ff00","warning":"#ffaa00","error":"#ff0000"}}"##,
    )
    .unwrap();
    assert_eq!(swatches(&dir, "forest", "dark").len(), 7);
    // The light rendering isn't there: the dark one stands in.
    assert_eq!(swatches(&dir, "forest", "light")[0], "#111111");
    assert!(
        swatches(&dir, "mine", "dark").is_empty(),
        "a theme that isn't rendered"
    );
    assert!(
        swatches(&dir, "../forest", "dark").is_empty(),
        "no paths in ids"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}
