//! Managing printers: queues, test pages, drivers and options.

use super::*;

#[tokio::test]
async fn pause_resume_default_remove() {
    let r = rig(office());
    run(&r.page, Op::Pause("PDF".into())).await;
    assert_eq!(r.world().printers[0].state, 5);
    r.world().printers[0].accepting = false;
    let out = run(&r.page, Op::Resume("PDF".into())).await;
    assert_eq!(out.error, None);
    {
        let w = r.world();
        assert_eq!(w.printers[0].state, 3);
        assert!(w.printers[0].accepting, "resume takes documents again too");
    }
    let out = run(&r.page, Op::SetDefault("PDF".into())).await;
    assert_eq!(out.state["default"], "PDF");
    let out = run(&r.page, Op::Remove("PDF".into())).await;
    assert_eq!(out.state["default"], Value::Null);
    assert_eq!(out.state["printers"].as_array().unwrap().len(), 1);
    let out = run(&r.page, Op::Remove("Gone".into())).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Printer \"Gone\" does not exist.")
    );
}

#[tokio::test]
async fn test_page_and_cancelling() {
    let r = rig(office());
    let out = run(&r.page, Op::TestPage("Hall_Laser".into())).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "The test page is on its way.");
    assert_eq!(
        r.world().printed,
        [(
            "Hall_Laser".to_string(),
            "Test page".to_string(),
            "application/octet-stream".to_string(),
            28
        )]
    );
    let out = run(&r.page, Op::CancelAll("Hall_Laser".into())).await;
    assert_eq!(out.error, None);
    let left: Vec<i32> = r.world().jobs.iter().map(|j| j.0).collect();
    assert_eq!(left, [5], "only that printer's documents");
}

#[tokio::test]
async fn looks_for_printers_in_the_background() {
    let mut w = office();
    w.devices = vec![
        (
            "socket".into(),
            "AppSocket/HP JetDirect".into(),
            "Unknown".into(),
            String::new(),
        ),
        (
            "usb://Example/Old?serial=1".into(),
            "Example Old".into(),
            "Example Old".into(),
            "MFG:Example;MDL:Old;".into(),
        ),
        (
            "ipps://Hall%20Laser._ipps._tcp.local/".into(),
            "Example Laser 3000 (driverless)".into(),
            "Example Laser 3000".into(),
            "MFG:Example;MDL:Laser 3000;".into(),
        ),
        (
            "ipp://192.0.2.7/ipp/print".into(),
            "Hall".into(),
            "Unknown".into(),
            String::new(),
        ),
    ];
    let r = rig(w);
    let out = run(&r.page, Op::Look).await;
    assert_eq!(out.error, None);
    let mut state = out.state;
    for _ in 0..100 {
        if state["looking"] == false && state["found"].is_array() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        state = run(&r.page, Op::Refresh).await.state;
    }
    let found = state["found"].as_array().unwrap();
    let uris: Vec<_> = found.iter().map(|d| d["uri"].as_str().unwrap()).collect();
    assert_eq!(
        uris,
        [
            "ipps://Hall%20Laser._ipps._tcp.local/",
            "ipp://192.0.2.7/ipp/print",
            "usb://Example/Old?serial=1"
        ],
        "driverless first; bare schemes left out"
    );
    assert_eq!(found[0]["driverless"], true);
    assert_eq!(found[1]["added"], true, "already a printer here");
    assert_eq!(found[1]["model"], "", "\"Unknown\" isn't a model");
    assert_eq!(found[2]["connection"], "USB");
}

#[tokio::test]
async fn drivers_offer_driverless_first() {
    let mut w = office();
    w.ppds = vec![
        (
            "drv:///sample.drv/laser.ppd".into(),
            "Example Laser, 1.0".into(),
        ),
        ("everywhere".into(), "IPP Everywhere".into()),
    ];
    let r = rig(w);
    let out = run(
        &r.page,
        Op::Drivers {
            uri: "ipp://192.0.2.9/ipp/print".into(),
            device_id: "MFG:Example;MDL:Laser;".into(),
            make_model: String::new(),
        },
    )
    .await;
    assert_eq!(
        out.state["drivers"],
        json!({"uri": "ipp://192.0.2.9/ipp/print", "drivers": [
            {"name": "everywhere", "label": "Driverless (recommended)"},
            {"name": "drv:///sample.drv/laser.ppd", "label": "Example Laser, 1.0"},
        ]})
    );
    let out = run(
        &r.page,
        Op::Drivers {
            uri: "usb://Example/Old".into(),
            device_id: String::new(),
            make_model: "Example Old".into(),
        },
    )
    .await;
    assert_eq!(
        out.state["drivers"]["drivers"][0]["name"],
        "drv:///sample.drv/laser.ppd"
    );
}

#[tokio::test]
async fn options_load_and_save_into_the_driver_file() {
    let r = rig(office());
    let out = run(&r.page, Op::LoadOptions("PDF".into())).await;
    let o = &out.state["options"];
    assert_eq!(o["printer"], "PDF");
    assert_eq!(o["options"][0]["key"], "PageSize");
    assert_eq!(o["options"][0]["default"], "Letter");
    assert_eq!(o["options"][1]["choices"][1]["label"], "Long edge");

    let out = run(
        &r.page,
        Op::Save {
            printer: "PDF".into(),
            name: "My PDFs".into(),
            location: "Desk".into(),
            options: BTreeMap::from([("PageSize".into(), "A4".into())]),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["options"], Value::Null, "the dialog closes");
    {
        let w = r.world();
        assert_eq!(w.printers[0].info, "My PDFs");
        assert_eq!(w.printers[0].location, "Desk");
        let up = String::from_utf8(w.uploaded.clone().unwrap()).unwrap();
        assert!(up.contains("*DefaultPageSize: A4\n"), "{up}");
    }

    // Only the name: the driver file isn't touched.
    r.world().uploaded = None;
    run(
        &r.page,
        Op::Save {
            printer: "PDF".into(),
            name: "PDF".into(),
            location: String::new(),
            options: BTreeMap::new(),
        },
    )
    .await;
    assert!(r.world().uploaded.is_none());

    let out = run(
        &r.page,
        Op::Save {
            printer: "PDF".into(),
            name: "PDF".into(),
            location: String::new(),
            options: BTreeMap::from([("PageSize".into(), "Tabloid".into())]),
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("Media Size can't be set to Tabloid on this printer.")
    );
}

#[tokio::test]
async fn missing_or_switched_off_printing_reads_as_such() {
    for (e, off) in [(CupsError::Missing, false), (CupsError::Off, true)] {
        let page = PrintersState::with(
            Arc::new(Down(e.clone())),
            "me".into(),
            PathBuf::from("/nonexistent"),
            SystemBus::at("unix:path=/nonexistent/bus"),
        );
        let out = run(&page, Op::Refresh).await;
        assert_eq!(out.error, None);
        assert_eq!(out.state["available"], false);
        assert_eq!(out.state["off"], off);
        assert_eq!(out.state["text"], e.text());
    }
}
