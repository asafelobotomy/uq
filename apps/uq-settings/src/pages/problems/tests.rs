use super::*;
use std::sync::Mutex as StdMutex;

static COPIED: StdMutex<Vec<String>> = StdMutex::new(Vec::new());

fn copy_ok(t: &str) -> Result<(), String> {
    COPIED.lock().unwrap().push(t.to_string());
    Ok(())
}
fn copy_missing(_: &str) -> Result<(), String> {
    Err("copying needs wl-clipboard, which isn't installed".into())
}
fn open_ok(_: &std::path::Path) -> Result<(), String> {
    Ok(())
}
fn no_journal(_: &Query, _: &Registry) -> std::io::Result<Vec<Entry>> {
    Ok(vec![
        entry(Some("HYPR-003"), 1),
        entry(Some("HYPR-003"), 3),
        entry(Some("LAUNCH-001"), 2),
        entry(None, 4),
    ])
}

fn entry(code: Option<&str>, t: i64) -> Entry {
    let reg = Registry::builtin();
    Entry {
        time_us: t * 1_000_000,
        first_us: t * 1_000_000,
        source: "uqd".into(),
        severity: code
            .and_then(|c| reg.get(c))
            .map_or(Severity::Warn, |c| c.severity),
        code: code.map(str::to_string),
        message: format!("[{}] x", code.unwrap_or("-")),
        count: 1,
    }
}

fn setup() -> (tempfile::TempDir, ProblemsState) {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().to_string_lossy().into_owned();
    let dirs = Dirs::from_vars(&|k| (k == "HOME").then(|| home.clone()));
    let run = dirs.quickshell.join("run9");
    std::fs::create_dir_all(&run).unwrap();
    std::fs::write(
        run.join("report.txt"),
        "===== Instance Information =====\nSignal: Aborted (6)\nConfig Path: /usr/share/uq/shell\n",
    )
    .unwrap();
    let s = ProblemsState::with(dirs, Paths::under(root.path()), Registry::builtin().clone())
        .with_io(copy_ok, open_ok, no_journal);
    (root, s)
}

#[tokio::test]
async fn crashes_and_grouped_problems_are_shown() {
    let (_root, s) = setup();
    let v = s.snapshot().await.unwrap();
    assert_eq!(v["hours"], DEFAULT_HOURS);
    assert_eq!(v["crashes"][0]["title"], "uQ's shell crashed");
    assert_eq!(v["crashes"][0]["code"], "CRASH-001");
    assert_eq!(v["problems"].as_array().unwrap().len(), 2);
    assert_eq!(v["problems"][0]["code"], "HYPR-003");
    assert_eq!(v["problems"][0]["count"], 2);
    assert_eq!(v["problems"][0]["title"], "Talking to Hyprland failed");
    assert_eq!(v["errors"], 1);
    assert_eq!(v["other"], 1);
}

#[tokio::test]
async fn copy_puts_the_crash_report_on_the_clipboard() {
    let (_root, s) = setup();
    s.apply(Op::Copy {
        id: "qs-run9".into(),
    })
    .await
    .unwrap();
    let copied = COPIED.lock().unwrap().pop().unwrap();
    assert!(
        copied.starts_with("# uQ's shell crashed — uQ crash report"),
        "{copied}"
    );
    let v = s.snapshot().await.unwrap();
    assert_eq!(v["notice"]["seq"], 1);
    // Seen now: the report is saved beside the crash.
    assert!(crash::find(&s.dirs, "run9").unwrap().reported);
}

#[tokio::test]
async fn without_a_clipboard_tool_it_says_where_the_report_is() {
    let (_root, s) = setup();
    let s = s.with_io(copy_missing, open_ok, no_journal);
    let e = s
        .apply(Op::Copy {
            id: "qs-run9".into(),
        })
        .await
        .unwrap_err();
    let text = e.to_string();
    assert!(
        text.contains("wl-clipboard") && text.contains("report.md"),
        "{text}"
    );
}

#[tokio::test]
async fn a_dismissed_crash_leaves_the_list() {
    let (_root, s) = setup();
    s.apply(Op::Dismiss {
        id: "qs-run9".into(),
    })
    .await
    .unwrap();
    let v = s.snapshot().await.unwrap();
    assert!(v["crashes"].as_array().unwrap().is_empty());
    assert!(s.apply(Op::Dismiss { id: "gone".into() }).await.is_err());
    s.apply(Op::SetHours { hours: 900 }).await.unwrap();
    assert_eq!(s.snapshot().await.unwrap()["hours"], 168);
}
