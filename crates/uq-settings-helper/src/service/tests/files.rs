//! Wallpaper and monitor files: fds, caps and validation.

use super::*;

#[tokio::test]
async fn wallpaper_is_copied_byte_for_byte_from_offset_zero() {
    let f = Fixture::new(true);
    // Not an image at all: it is never decoded, only copied.
    let bytes: Vec<u8> = (0..300_000u32).map(|i| (i * 7 % 256) as u8).collect();
    let mut file = tmpfile(&bytes);
    // The caller's offset is left wherever it was, and doesn't change what is copied.
    file.seek(SeekFrom::Start(1234)).unwrap();
    f.helper
        .set_greeter_wallpaper(":1.5", fd(file))
        .await
        .unwrap();
    let out = f.path("var/lib/uq-greeter/wallpaper");
    assert_eq!(fs::read(&out).unwrap(), bytes);
    use std::os::unix::fs::MetadataExt;
    assert_eq!(fs::metadata(&out).unwrap().mode() & 0o7777, 0o644);
    assert_eq!(f.checks(), [(":1.5".to_string(), Action::Greeter)]);
}

#[tokio::test]
async fn wallpaper_at_exactly_the_cap_is_accepted_and_one_byte_more_is_not() {
    let f = Fixture::new(true);
    let file = tempfile::tempfile().unwrap();
    file.set_len(MAX_WALLPAPER).unwrap();
    f.helper
        .set_greeter_wallpaper(":1.5", fd(file))
        .await
        .unwrap();
    assert_eq!(
        fs::metadata(f.path("var/lib/uq-greeter/wallpaper"))
            .unwrap()
            .len(),
        MAX_WALLPAPER
    );

    let f = Fixture::new(true);
    let file = tempfile::tempfile().unwrap();
    file.set_len(MAX_WALLPAPER + 1).unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(file)).await
    ));
    assert!(f.checks().is_empty());
    assert!(f.written().is_empty());
}

#[test]
fn read_capped_refuses_a_file_that_grew_past_the_cap() {
    let file = tmpfile(&[1u8; 100]);
    assert_eq!(read_capped(&file, 100).unwrap().len(), 100);
    assert!(read_capped(&file, 99).is_err());
}

#[tokio::test]
async fn wallpaper_refuses_directories_fifos_sockets_and_devices() {
    let f = Fixture::new(true);
    let scratch = tempfile::tempdir().unwrap();

    // A directory fd.
    let dir = File::open(scratch.path()).unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(dir)).await
    ));

    // A FIFO, opened without blocking: refused by fstat, never read (a read would hang).
    let fifo = scratch.path().join("fifo");
    let c = std::ffi::CString::new(fifo.as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
    let fifo_fd = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&fifo)
        .unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(fifo_fd)).await
    ));

    // A pipe and a socket.
    let (r, _w) = std::io::pipe().unwrap();
    assert!(is_invalid(
        f.helper
            .set_greeter_wallpaper(":1.5", OwnedFd::from(r))
            .await
    ));
    let (s, _t) = std::os::unix::net::UnixStream::pair().unwrap();
    assert!(is_invalid(
        f.helper
            .set_greeter_wallpaper(":1.5", OwnedFd::from(s))
            .await
    ));

    // Character devices.
    for dev in ["/dev/null", "/dev/zero", "/dev/urandom"] {
        let d = File::open(dev).unwrap();
        assert!(is_invalid(
            f.helper.set_greeter_wallpaper(":1.5", fd(d)).await
        ));
    }

    // A file opened through a symlink is whatever the caller opened: still a regular file,
    // and still only readable with the caller's own rights. It is copied like any other.
    let real = scratch.path().join("real");
    fs::write(&real, b"img").unwrap();
    std::os::unix::fs::symlink(&real, scratch.path().join("link")).unwrap();
    let via_link = File::open(scratch.path().join("link")).unwrap();
    f.helper
        .set_greeter_wallpaper(":1.5", fd(via_link))
        .await
        .unwrap();
    assert_eq!(
        fs::read(f.path("var/lib/uq-greeter/wallpaper")).unwrap(),
        b"img"
    );
    assert_eq!(f.checks().len(), 1, "only the regular file reached polkit");
}

#[tokio::test]
async fn wallpaper_refuses_an_unreadable_fd() {
    let f = Fixture::new(true);
    let scratch = tempfile::tempdir().unwrap();
    let p = scratch.path().join("w");
    fs::write(&p, b"data").unwrap();
    let write_only = OpenOptions::new().write(true).open(&p).unwrap();
    assert!(is_invalid(
        f.helper.set_greeter_wallpaper(":1.5", fd(write_only)).await
    ));
    assert!(f.written().is_empty());
}

#[tokio::test]
async fn clear_wallpaper_removes_it_and_is_idempotent() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_wallpaper(":1.5", fd(tmpfile(b"x")))
        .await
        .unwrap();
    f.helper.clear_greeter_wallpaper(":1.5").await.unwrap();
    assert!(!f.path("var/lib/uq-greeter/wallpaper").exists());
    f.helper.clear_greeter_wallpaper(":1.5").await.unwrap();
    assert_eq!(
        f.checks().iter().map(|c| c.1).collect::<Vec<_>>(),
        [Action::Greeter; 3]
    );
}

#[tokio::test]
async fn clear_monitors_removes_the_file_and_is_idempotent() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_monitors(":1.5", r#"{"outputs":{"DP-1":{"mode":"1920x1080@60"}}}"#)
        .await
        .unwrap();
    assert!(f.path("etc/uq/greeter-monitors.toml").exists());
    f.helper.clear_greeter_monitors(":1.5").await.unwrap();
    assert!(!f.path("etc/uq/greeter-monitors.toml").exists());
    f.helper.clear_greeter_monitors(":1.5").await.unwrap();
    assert_eq!(
        f.checks().iter().map(|c| c.1).collect::<Vec<_>>(),
        [Action::Greeter; 3]
    );
}

#[tokio::test]
async fn monitors_go_through_uq_core_validation() {
    let f = Fixture::new(true);
    f.helper
        .set_greeter_monitors(
            ":1.5",
            r#"{"outputs":{"DP-1":{"mode":"1920x1080@60","scale":1.5,"transform":1}}}"#,
        )
        .await
        .unwrap();
    let back: Monitors =
        toml::from_str(&fs::read_to_string(f.path("etc/uq/greeter-monitors.toml")).unwrap())
            .unwrap();
    assert_eq!(back.outputs["DP-1"].mode.as_deref(), Some("1920x1080@60"));
    assert_eq!(back.outputs["DP-1"].transform, Some(1));

    let huge = format!(
        r#"{{"outputs":{{"DP-1":{{"description":"{}"}}}}}}"#,
        "a".repeat(70_000)
    );
    for bad in [
        "",
        "null",
        "[]",
        "not json",
        r#"{"outputs":{"../etc":{}}}"#,
        r#"{"outputs":{"DP 1":{}}}"#,
        r#"{"outputs":{"DP-1\n[x]":{}}}"#,
        r#"{"outputs":{"DP-1":{"mode":"1920x1080\"\nevil=1"}}}"#,
        r#"{"outputs":{"DP-1":{"scale":99}}}"#,
        r#"{"outputs":{"DP-1":{"transform":8}}}"#,
        r#"{"outputs":{"DP-1":{"mirror_of":"DP-1"}}}"#,
        r#"{"outputs":{"DP-1":{"unknown":1}}}"#,
        r#"{"outputs":{},"extra":1}"#,
        r#"{"outputs":{"DP-1":{"scale":"1"}}}"#,
        huge.as_str(),
    ] {
        assert!(
            is_invalid(f.helper.set_greeter_monitors(":1.5", bad).await),
            "{}",
            &bad[..bad.len().min(60)]
        );
    }
    assert_eq!(f.checks().len(), 1);
}
