use super::*;

fn def(prefix: Option<&str>) -> ProviderDef {
    ProviderDef {
        bus_name: "org.example.P".into(),
        name: "P".into(),
        prefix: prefix.map(String::from),
        source_dir: "/x".into(),
        activatable: false,
    }
}

fn raw(id: &str, title: &str, icon: &str, score: f64) -> RawItem {
    (
        id.into(),
        title.into(),
        String::new(),
        icon.into(),
        "[]".into(),
        score,
    )
}

#[test]
fn starts_at_most_every_30_s_and_logs_every_10_min() {
    let mut t = StartThrottle::default();
    let t0 = Instant::now();
    assert!(t.try_start("a", t0));
    assert!(!t.try_start("a", t0 + Duration::from_secs(29)));
    assert!(t.try_start("b", t0), "per provider");
    assert!(t.try_start("a", t0 + Duration::from_secs(30)));
    assert!(t.should_log("a", t0));
    assert!(!t.should_log("a", t0 + Duration::from_secs(599)));
    assert!(t.should_log("a", t0 + Duration::from_secs(600)));
}

#[test]
fn route_prefix_and_gating() {
    let p = def(Some(":"));
    let n = def(None);
    let all = [p.clone(), n.clone()];
    assert_eq!(route(":smi", &p, &all).as_deref(), Some("smi"));
    assert_eq!(route(":smi", &n, &all), None);
    assert_eq!(route("smi", &p, &all), None);
    assert_eq!(route("smi", &n, &all).as_deref(), Some("smi"));
    assert_eq!(route("a", &n, &all), None);
    assert_eq!(route(">ls", &n, &all), None);
    assert_eq!(route("=1+1", &n, &all), None);
    assert_eq!(
        route("ab", &n, std::slice::from_ref(&n)).as_deref(),
        Some("ab")
    );
}

#[test]
fn clamp_limits() {
    let items: Vec<RawItem> = (0..25).map(|i| raw(&i.to_string(), "t", "", 0.5)).collect();
    assert_eq!(clamp_all(items).len(), 20);
    let it = clamp(raw("a", &"é".repeat(150), "", 0.5));
    assert_eq!(it.title.len(), 200);
    assert!(it.title.chars().all(|c| c == 'é'));
    let mut r = raw("a", "t", "", 0.5);
    r.2 = "s".repeat(400);
    assert_eq!(clamp(r).subtitle.len(), 300);
    let mut r = raw("a", "t", "", 0.5);
    r.4 = serde_json::to_string(&vec!["x".repeat(100); 40]).unwrap();
    let a = clamp(r).actions;
    assert!(!a.is_empty() && serde_json::to_string(&a).unwrap().len() <= 2048);
    let mut r = raw("a", "t", "", 0.5);
    r.4 = "{not a list".into();
    assert!(clamp(r).actions.is_empty());
    assert!(clamp_all(vec![raw(&"i".repeat(513), "t", "", 0.5)]).is_empty());
}

#[test]
fn score_clamped_and_scaled() {
    assert!((clamp(raw("a", "t", "", 5.0)).score - 0.9).abs() < 1e-12);
    assert_eq!(clamp(raw("a", "t", "", -1.0)).score, 0.0);
    assert_eq!(clamp(raw("a", "t", "", f64::NAN)).score, 0.0);
}

#[test]
fn icon_clamped_to_theme_name_or_local_path() {
    for ok in ["firefox", "/usr/share/x.png", "org.kde.app-1_x"] {
        assert_eq!(clamp(raw("a", "t", ok, 0.5)).icon, ok);
    }
    for bad in [
        "http://x/y.png",
        "file:///x",
        "../x",
        "<img>",
        "",
        "/a/../b",
        "/a?b",
        "//host/x.png",
        "/a//b",
        ".",
        "..",
        "...",
        "/dev/zero",
        "/proc/self/fd/0",
        "/sys/kernel/x.png",
        "/dev",
    ] {
        assert_eq!(
            clamp(raw("a", "t", bad, 0.5)).icon,
            "system-search",
            "{bad}"
        );
    }
}
