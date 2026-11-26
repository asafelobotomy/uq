use super::*;

#[test]
fn redirect_policy() {
    let u = |s: &str| reqwest::Url::parse(s).unwrap();
    let a = u("https://a.org/feed");
    assert!(may_follow(
        std::slice::from_ref(&a),
        &u("https://b.org/feed")
    ));
    assert!(!may_follow(
        std::slice::from_ref(&a),
        &u("http://b.org/feed")
    ));
    assert!(!may_follow(&[u("http://a.org/")], &u("https://b.org/")));
    let three = vec![a.clone(), a.clone(), a.clone()];
    assert!(may_follow(&three, &a), "third redirect");
    let four = vec![a.clone(), a.clone(), a.clone(), a.clone()];
    assert!(!may_follow(&four, &a), "fourth redirect");
}

#[test]
fn write_private_is_0600_and_atomic() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("uq/feeds/x.json");
    write_private(&p, "one").unwrap();
    write_private(&p, "two").unwrap();
    assert_eq!(std::fs::read_to_string(&p).unwrap(), "two");
    let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let dmode = std::fs::metadata(p.parent().unwrap())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dmode, 0o700);
    let names: Vec<_> = std::fs::read_dir(p.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(names.len(), 1, "no temp file left: {names:?}");
}

#[test]
fn status_of_a_round() {
    assert_eq!(status(0, 0), "ok");
    assert_eq!(status(2, 2), "ok");
    assert_eq!(status(1, 2), "partial");
    assert_eq!(status(0, 2), "error");
}

#[tokio::test]
async fn capped_reads() {
    use test_http::*;
    let (base, _) = serve(routes(&[("*", Reply::ok(vec![b'x'; 100]))]), |_| None).await;
    let c = client(reqwest::redirect::Policy::none());
    assert_eq!(get_capped(&c, &base, 50, false).await, Err("too large"));
    assert_eq!(get_capped(&c, &base, 50, true).await.unwrap().len(), 50);
    assert_eq!(get_capped(&c, &base, 100, false).await.unwrap().len(), 100);
}
