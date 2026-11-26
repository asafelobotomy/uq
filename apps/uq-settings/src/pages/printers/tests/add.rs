//! Listing and adding printers.

use super::*;

#[tokio::test]
async fn shows_printers_their_state_and_queue() {
    let r = rig(office());
    let out = run(&r.page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["available"], true);
    assert_eq!(s["default"], "Hall_Laser");
    let ps = s["printers"].as_array().unwrap();
    assert_eq!(ps[0]["name"], "Hall_Laser", "the default comes first");
    assert_eq!(ps[0]["default"], true);
    assert_eq!(ps[0]["state"], "paused");
    assert_eq!(ps[0]["status"], "Paused");
    assert_eq!(ps[0]["connection"], "Network");
    assert_eq!(
        ps[0]["problems"],
        json!(["Out of paper", "Ink or toner is low"])
    );
    let jobs = ps[0]["jobs"].as_array().unwrap();
    assert_eq!(jobs.len(), 2);
    assert_eq!(jobs[0]["title"], "Quarterly report.pdf");
    assert_eq!(jobs[0]["mine"], true);
    assert_eq!(jobs[0]["state"], "Waiting");
    assert_eq!(jobs[1]["title"], "Untitled document");
    assert_eq!(jobs[1]["mine"], false);
    assert_eq!(ps[1]["connection"], "Saves PDF files");
    assert_eq!(ps[1]["jobs"][0]["printing"], true);
    assert_eq!(s["found"], Value::Null, "nothing looked for yet");
    assert_eq!(s["classic"], false);
}

#[tokio::test]
async fn adds_a_driverless_printer_and_makes_the_first_one_default() {
    let r = rig(World {
        admin: true,
        ..Default::default()
    });
    let out = run(
        &r.page,
        Op::Add {
            uri: "ipps://Hall%20Laser._ipps._tcp.local/".into(),
            name: "Hall Laser (2nd floor)".into(),
            location: " Hall ".into(),
            driver: "everywhere".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let w = r.world();
    let p = &w.printers[0];
    assert_eq!(p.name, "Hall_Laser_(2nd_floor)");
    assert_eq!(p.info, "Hall Laser (2nd floor)");
    assert_eq!(p.location, "Hall");
    assert_eq!(p.driver, "everywhere");
    assert_eq!(w.default.as_deref(), Some("Hall_Laser_(2nd_floor)"));
    assert_eq!(
        out.state["notice"]["text"],
        "Hall Laser (2nd floor) was added."
    );
    assert!(
        w.calls
            .iter()
            .any(|c| c == &("/admin/".to_string(), op::CUPS_ADD_MODIFY_PRINTER))
    );
}

#[tokio::test]
async fn a_second_printer_keeps_the_default_and_gets_a_free_name() {
    let r = rig(office());
    let out = run(
        &r.page,
        Op::Add {
            uri: "usb://Example/Laser".into(),
            name: "PDF".into(),
            location: String::new(),
            driver: "drv:///sample.drv/laser.ppd".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    let w = r.world();
    assert_eq!(w.printers[2].name, "PDF_2");
    assert_eq!(w.default.as_deref(), Some("Hall_Laser"));
}

#[tokio::test]
async fn people_who_arent_administrators_are_told() {
    let mut w = office();
    w.admin = false;
    let r = rig(w);
    let out = run(&r.page, Op::Remove("PDF".into())).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Only an administrator can change printers, and your account isn't one.")
    );
    assert_eq!(out.state["printers"].as_array().unwrap().len(), 2);
    // Reading and their own documents still work.
    let out = run(&r.page, Op::CancelJob(3)).await;
    assert_eq!(out.error, None);
}

#[test]
fn queue_names() {
    let taken = vec!["Office".to_string(), "office_2".to_string()];
    assert_eq!(queue_name("Office", &taken), "Office_3");
    assert_eq!(queue_name("  Hall laser / #2 ", &[]), "Hall_laser_2");
    assert_eq!(queue_name("Ünïcode", &[]), "n_code");
    assert_eq!(queue_name("   ", &[]), "Printer");
    assert!(queue_name(&"x".repeat(300), &[]).len() <= 100);
}

#[test]
fn words() {
    let none: Vec<String> = Vec::new();
    assert_eq!(state_words(5, &none, "Paused."), ("paused", "Paused"));
    assert_eq!(
        state_words(5, &none, "Backend failed"),
        ("stopped", "Stopped")
    );
    assert_eq!(state_words(4, &none, ""), ("printing", "Printing"));
    assert_eq!(reason_words("media-jam-error"), Some("Paper jam"));
    assert_eq!(
        reason_words("offline-report"),
        Some("The printer can't be reached")
    );
    assert_eq!(reason_words("none"), None);
    assert_eq!(reason_words("paused"), None);
    assert_eq!(connection_words("usb://HP/LaserJet"), "USB");
    assert_eq!(connection_words("ipp://localhost:60000/ipp/print"), "USB");
    assert_eq!(connection_words("dnssd://Hall._ipp._tcp.local/"), "Network");
    assert_eq!(connection_words("cups-pdf:/"), "Saves PDF files");
    assert!(driverless("dnssd://Hall._ipps._tcp.local/?uuid=1"));
    assert!(!driverless("dnssd://Hall._pdl-datastream._tcp.local/"));
    assert!(!driverless("usb://HP/LaserJet"));
}
