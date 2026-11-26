use super::*;
use uq_core::settings::BarItem;

const STAT1: &str = "cpu  100 0 100 800 0 0 0 0 0 0\n\
    cpu0 50 0 50 400 0 0 0 0 0 0\n\
    cpu1 50 0 50 400 0 0 0 0 0 0\n\
    intr 1 2 3\nctxt 5\n";
// +200 total: all: busy +100 (50%); cpu0 busy +90 of 100; cpu1 busy +10 of 100 (iowait idle).
const STAT2: &str = "cpu  180 10 110 880 20 0 0 0 0 0\n\
    cpu0 120 10 60 410 0 0 0 0 0 0\n\
    cpu1 60 0 50 470 20 0 0 0 0 0\n";

fn fixture() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    let p = d.path();
    std::fs::create_dir_all(p.join("proc")).unwrap();
    std::fs::write(p.join("proc/stat"), STAT1).unwrap();
    std::fs::write(
        p.join("proc/meminfo"),
        "MemTotal:       16000000 kB\nMemFree:  1000 kB\nMemAvailable:    4000000 kB\n",
    )
    .unwrap();
    for (dir, name, temps) in [
        ("hwmon0", "nvme", &[("temp1_input", "45000")][..]),
        ("hwmon1", "acpitz", &[("temp1_input", "30000")][..]),
        (
            "hwmon2",
            "k10temp",
            &[("temp3_input", "70000"), ("temp1_input", "52375")][..],
        ),
    ] {
        let d = p.join("sys/class/hwmon").join(dir);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("name"), format!("{name}\n")).unwrap();
        for (f, v) in temps {
            std::fs::write(d.join(f), v).unwrap();
        }
    }
    d
}

#[test]
fn deltas_memory_and_sensor() {
    let d = fixture();
    let m = SysMon::with_roots(d.path().join("proc"), d.path().join("sys"));
    let a = m.stat().unwrap();
    let b = parse_stat(STAT2).unwrap();
    assert_eq!(a.cores.len(), 2);
    let sensor = find_sensor(&d.path().join("sys"));
    assert!(sensor.as_ref().unwrap().ends_with("hwmon2/temp1_input"));
    let v = m.payload(&a, &b, sensor.as_deref(), None);
    assert_eq!(
        v,
        json!({"cpu": 50.0, "cores": [90.0, 10.0], "mem": 75.0,
               "mem_used": 12_000_000u64 * 1024, "mem_total": 16_000_000u64 * 1024,
               "temp_c": 52.4, "gpu": null})
    );
    // Identical samples: 0 %, never a division by zero.
    assert_eq!(m.payload(&a, &a, None, None)["cpu"], 0.0);
    assert!(m.payload(&a, &a, None, None)["temp_c"].is_null());
}

#[test]
fn nvidia_line_and_amd_files() {
    let g = parse_nvidia(
        "NVIDIA GeForce RTX 4070, 37, 1024, 12282, 51\nNVIDIA T400, 90, [N/A], 4096, [N/A]\n",
    )
    .unwrap();
    assert_eq!(g.name, "NVIDIA T400", "the busiest card");
    assert_eq!(
        (g.busy, g.vram_used, g.vram_total, g.temp_c),
        (90.0, 0, 4096 << 20, None)
    );
    let g = parse_nvidia("RTX, 5, 2048, 8192, 40\n").unwrap();
    assert_eq!((g.vram_used, g.temp_c), (2048 << 20, Some(40.0)));
    assert_eq!(parse_nvidia(""), None);
    assert_eq!(parse_nvidia("No devices were found\n"), None);
    assert_eq!(parse_nvidia("x, nan, 1, 1, 1\n"), None);

    let d = tempfile::tempdir().unwrap();
    let dev = d.path().join("class/drm/card0/device");
    std::fs::create_dir_all(dev.join("hwmon/hwmon3")).unwrap();
    std::fs::write(dev.join("gpu_busy_percent"), "42\n").unwrap();
    std::fs::write(dev.join("mem_info_vram_used"), "1000\n").unwrap();
    std::fs::write(dev.join("mem_info_vram_total"), "4000\n").unwrap();
    std::fs::write(dev.join("hwmon/hwmon3/temp1_input"), "61000\n").unwrap();
    let g = read_amd(d.path()).unwrap();
    assert_eq!(
        (g.name.as_str(), g.busy, g.vram_used, g.vram_total, g.temp_c),
        ("AMD graphics", 42.0, 1000, 4000, Some(61.0))
    );
    assert_eq!(read_amd(&d.path().join("none")), None);
}

#[test]
fn no_known_sensor_is_none() {
    let d = tempfile::tempdir().unwrap();
    let h = d.path().join("class/hwmon/hwmon0");
    std::fs::create_dir_all(&h).unwrap();
    std::fs::write(h.join("name"), "amdgpu").unwrap();
    std::fs::write(h.join("temp1_input"), "60000").unwrap();
    assert_eq!(find_sensor(d.path()), None);
    assert_eq!(find_sensor(&d.path().join("missing")), None);
    assert_eq!(parse_stat("intr 1\n"), None);
    assert_eq!(parse_meminfo("MemTotal: 5 kB\n"), None);
}

#[tokio::test]
async fn runs_only_while_placed() {
    let d = fixture();
    let m = SysMon::with_roots(d.path().join("proc"), d.path().join("sys"));
    let off = Some(Settings::default());
    let mut on = Settings::default();
    on.shell
        .bar
        .as_mut()
        .unwrap()
        .start
        .push(BarItem::new("sysmon"));
    let (stx, srx) = watch::channel(off.clone());
    let (tx, mut rx) = mpsc::channel(16);
    let stop = CancellationToken::new();
    let task = tokio::spawn(m.run(tx, srx, stop.clone(), Duration::from_millis(50)));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(rx.try_recv().is_err());
    stx.send_replace(Some(on));
    let got = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await
        .unwrap()
        .unwrap();
    let Outbound::Event { event, data } = got else {
        panic!()
    };
    assert_eq!(event, "SysMon");
    assert_eq!(data["mem"], 75.0);
    stx.send_replace(off);
    // Drain samples until the null.
    loop {
        let Outbound::Event { data, .. } = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await
            .unwrap()
            .unwrap()
        else {
            panic!()
        };
        if data.is_null() {
            break;
        }
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(rx.try_recv().is_err(), "stopped after removal");
    stop.cancel();
    task.await.unwrap();
}
