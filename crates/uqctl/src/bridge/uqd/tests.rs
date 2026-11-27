use super::*;

#[test]
fn set_bar_layout_takes_one_object() {
    assert!(matches!(
        Request::parse("SetBarLayout", &[json!({"start": []})]),
        Ok(Request::SetBarLayout(j)) if j == r#"{"start":[]}"#
    ));
    assert!(Request::parse("SetBarLayout", &[]).is_err());
    assert!(Request::parse("SetBarLayout", &[json!("{}")]).is_err());
    assert!(Request::parse("SetBarLayout", &[json!({}), json!({})]).is_err());
}

#[test]
fn profile_requests_check_arity() {
    assert!(Request::parse("ListProfiles", &[]).is_ok());
    assert!(Request::parse("ListProfiles", &[json!(1)]).is_err());
    assert!(Request::parse("ApplyProfile", &[json!("gaming")]).is_ok());
    assert!(Request::parse("ApplyProfile", &[]).is_err());
    assert!(Request::parse("ApplyProfile", &[json!(1)]).is_err());
    assert!(Request::parse("ApplyProfile", &[json!("a"), json!("b")]).is_err());
    assert!(Request::parse("ProfileOff", &[]).is_ok());
    assert!(Request::parse("ProfileOff", &[json!("x")]).is_err());
}

#[test]
fn list_outputs_is_a_shell_call() {
    assert!(matches!(
        Request::parse("ListOutputs", &[]),
        Ok(Request::ListOutputs)
    ));
    assert!(Request::parse("ListOutputs", &[json!(1)]).is_err());
}

#[test]
fn request_parse_accepts_get_section_and_shell_ready() {
    assert!(matches!(
        Request::parse("GetSection", &[json!("compat")]),
        Ok(Request::GetSection(d)) if d == "compat"
    ));
    assert!(Request::parse("GetSection", &[]).is_err());
    assert!(Request::parse("GetSection", &[json!(1)]).is_err());
    assert!(Request::parse("GetSection", &[json!("a"), json!("b")]).is_err());
    assert!(matches!(
        Request::parse("ShellReady", &[]),
        Ok(Request::ShellReady)
    ));
    assert!(Request::parse("ShellReady", &[json!("x")]).is_err());
}

#[test]
fn pick_done_checks_arity() {
    assert!(Request::parse("PickDone", &[json!("t"), json!("click")]).is_ok());
    assert!(Request::parse("PickDone", &[]).is_err());
    assert!(Request::parse("PickDone", &[json!("t")]).is_err());
    assert!(Request::parse("PickDone", &[json!("t"), json!(1)]).is_err());
    assert!(Request::parse("PickDone", &[json!("t"), json!("x"), json!("y")]).is_err());
}

/// A "bus" that accepts the connection and never speaks: `open` gives up within BUS_WAIT
/// and reports uqd down instead of hanging (so `Hello` still goes out).
#[tokio::test]
async fn open_gives_up_on_a_silent_bus() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("bus");
    let listener = tokio::net::UnixListener::bind(&path).unwrap();
    let held = tokio::spawn(async move {
        let mut conns = Vec::new();
        while let Ok((c, _)) = listener.accept().await {
            conns.push(c);
        }
    });
    let addr = format!("unix:path={}", path.display());
    let started = std::time::Instant::now();
    let link = tokio::time::timeout(Duration::from_secs(10), Link::open(Some(&addr)))
        .await
        .expect("open returns");
    assert!(!link.is_up());
    assert!(link.connection().await.is_none());
    assert!(started.elapsed() < Duration::from_secs(8));
    held.abort();
}
