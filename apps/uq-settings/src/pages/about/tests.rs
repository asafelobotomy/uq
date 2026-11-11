use super::*;
use crate::pages::fake_system::{FakeHostname, SysRig};
use crate::pages::runner::run;

const LSPCI: &str = "00:02.0 \"VGA compatible controller\" \"Intel Corporation\" \"Alder Lake-P GT2 [Iris Xe Graphics]\" -r0c \"Lenovo\" \"Device 3f0f\"\n\
00:1f.3 \"Audio device\" \"Intel Corporation\" \"Alder Lake PCH-P High Definition Audio Controller\" -r01 \"Lenovo\" \"Device 3f0f\"\n\
01:00.0 \"3D controller\" \"NVIDIA Corporation\" \"GA107M [GeForce RTX 3050 Mobile]\" -ra1 \"Lenovo\" \"Device 3f0f\"\n";

fn fake_run(program: &str, args: &[&str]) -> Option<String> {
    match (program, args) {
        ("lspci", ["-mm"]) => Some(LSPCI.into()),
        ("Hyprland", ["--version"]) => {
            Some("Hyprland 0.52.1 built from branch  at commit abc (tag v0.52.1).\n".into())
        }
        ("quickshell", ["--version"]) => Some("quickshell 0.2.1, revision 1a2b3c\n".into()),
        _ => None,
    }
}

fn no_commands(_: &str, _: &[&str]) -> Option<String> {
    None
}

fn files() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let w = |rel: &str, text: &str| {
        let p = d.path().join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, text).unwrap();
    };
    w(
        "usr/lib/os-release",
        "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n",
    );
    w(
        "proc/cpuinfo",
        "processor\t: 0\nmodel name\t: 12th Gen Intel(R) Core(TM) i7-1260P\n\nprocessor\t: 1\nmodel name\t: 12th Gen Intel(R) Core(TM) i7-1260P\n",
    );
    w(
        "proc/meminfo",
        "MemTotal:       16219448 kB\nMemFree: 1 kB\n",
    );
    w("proc/sys/kernel/osrelease", "6.18.0-proc\n");
    w("proc/sys/kernel/hostname", "fromproc\n");
    d
}

#[tokio::test]
async fn reads_the_system_hardware_and_versions() {
    let mut rig = SysRig::new();
    let _h = FakeHostname::start(&mut rig).await;
    let d = files();
    let page = AboutState::with(rig.system(), d.path().to_path_buf(), fake_run);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["hostname"]["name"], "desk");
    assert_eq!(s["hostname"]["hardware"], "Framework Laptop 13");
    assert_eq!(s["os"], json!({"name": "Arch Linux", "version": "rolling"}));
    assert_eq!(s["kernel"], "6.18.1-arch1-1", "hostnamed's kernel wins");
    assert_eq!(
        s["cpu"],
        json!({"model": "12th Gen Intel(R) Core(TM) i7-1260P", "cores": 2})
    );
    assert_eq!(s["memory"], "15.5 GiB");
    assert_eq!(
        s["gpus"],
        json!([
            "Intel Corporation Alder Lake-P GT2 [Iris Xe Graphics]",
            "NVIDIA Corporation GA107M [GeForce RTX 3050 Mobile]"
        ])
    );
    assert_eq!(s["versions"]["hyprland"], "0.52.1");
    assert_eq!(s["versions"]["quickshell"], "0.2.1");
    assert_eq!(s["versions"]["uq"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn hostname_round_trip_refusal_and_bad_names() {
    let mut rig = SysRig::new();
    let h = FakeHostname::start(&mut rig).await;
    let d = files();
    let page = AboutState::with(rig.system(), d.path().to_path_buf(), no_commands);
    let set = |n: &str| Op::SetHostname { name: n.into() };
    let out = run(&page, set(" studio-pc ")).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["hostname"]["name"], "studio-pc");
    assert!(
        out.state["notice"]["text"]
            .as_str()
            .unwrap()
            .starts_with("The computer's name was changed")
    );
    for bad in ["", "-a", "a-", "my pc", "a..b", &"x".repeat(65)] {
        let out = run(&page, set(bad)).await;
        assert!(out.error.is_some(), "{bad:?}");
    }
    assert_eq!(h.state.lock().unwrap().static_hostname, "studio-pc");
    h.deny.set(true);
    let out = run(&page, set("other")).await;
    assert_eq!(out.error, None, "a cancelled prompt is quiet");
    assert_eq!(out.state["hostname"]["name"], "studio-pc");
}

#[tokio::test]
async fn without_hostnamed_or_commands() {
    let rig = SysRig::new();
    let d = files();
    let page = AboutState::with(rig.system(), d.path().to_path_buf(), no_commands);
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["hostname"]["available"], false);
    assert_eq!(s["hostname"]["package"], "systemd");
    assert_eq!(s["hostname"]["name"], "fromproc");
    assert_eq!(s["kernel"], "6.18.0-proc");
    assert_eq!(s["gpus"], json!([]));
    assert_eq!(s["versions"]["hyprland"], "");
}

#[test]
fn parsers() {
    assert_eq!(version_in("v0.45.2"), "0.45.2");
    assert_eq!(version_in("no version here"), "");
    assert_eq!(memory("MemTotal: 0 kB"), "");
    let pairs = os_release("# c\nNAME='Foo'\nPRETTY_NAME=\"Foo \\\"X\\\"\"\n");
    assert_eq!(pairs[0], ("NAME".into(), "Foo".into()));
    assert_eq!(pairs[1].1, "Foo \"X\"");
    assert_eq!(cpu("").1, 0);
    assert!(hostname_problem("a.b-c.d").is_none());
}

#[test]
fn run_command_times_out_and_reads_output() {
    assert_eq!(run_command("true", &[]).as_deref(), Some(""));
    assert_eq!(run_command("false", &[]), None);
    assert_eq!(run_command("/nonexistent/program", &[]), None);
    let start = std::time::Instant::now();
    assert_eq!(run_command("sleep", &["10"]), None);
    assert!(start.elapsed() < Duration::from_secs(5));
}
