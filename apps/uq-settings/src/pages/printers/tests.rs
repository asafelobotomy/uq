//! The Printers page against a fake CUPS that decodes each request and answers like CUPS.

use std::sync::Mutex as StdMutex;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;

use super::cups::{CupsError, Transport};
use super::ipp::{self, Group, Out, Request, op, tag};
use super::*;
use crate::pages::runner::run;

#[derive(Clone, Default)]
struct Printer {
    name: String,
    info: String,
    location: String,
    uri: String,
    driver: String,
    state: i32,
    reasons: Vec<String>,
    accepting: bool,
    ppd: Option<Vec<u8>>,
}

#[derive(Default)]
struct World {
    printers: Vec<Printer>,
    default: Option<String>,
    /// (id, printer, title, owner, state)
    jobs: Vec<(i32, String, String, String, i32)>,
    /// (uri, info, make and model, device id)
    devices: Vec<(String, String, String, String)>,
    /// (ppd-name, make and model)
    ppds: Vec<(String, String)>,
    /// Whether the person is one of CUPS's administrators.
    admin: bool,
    calls: Vec<(String, u16)>,
    /// (printer, job name, format, bytes)
    printed: Vec<(String, String, String, usize)>,
    /// The last driver file given back with a change.
    uploaded: Option<Vec<u8>>,
    /// The printer group of the request being handled.
    last_printer_group: Option<Group>,
}

#[derive(Default)]
struct FakeCups(StdMutex<World>);

fn name_of(uri: &str) -> String {
    let last = uri.rsplit('/').next().unwrap_or("");
    percent_encoding::percent_decode_str(last)
        .decode_utf8_lossy()
        .into_owned()
}

fn answer(status: u16, groups: Vec<(u8, ipp::Attrs)>) -> Vec<u8> {
    let mut r = Request::new(status);
    r.groups.extend(groups);
    r.encode()
}

fn s(t: u8, v: &str) -> Vec<Out> {
    vec![Out::Str(t, v.to_string())]
}

fn not_found(text: &str) -> Vec<u8> {
    let mut r = Request::new(0x0406);
    r.op_attr("status-message", Out::Str(tag::TEXT, text.into()));
    r.encode()
}

impl World {
    fn printer_group(&self, p: &Printer) -> (u8, ipp::Attrs) {
        let mut reasons: Vec<Out> = p
            .reasons
            .iter()
            .map(|r| Out::Str(tag::KEYWORD, r.clone()))
            .collect();
        if reasons.is_empty() {
            reasons.push(Out::Str(tag::KEYWORD, "none".into()));
        }
        (
            tag::PRINTER,
            vec![
                ("printer-name".into(), s(tag::NAME, &p.name)),
                ("printer-info".into(), s(tag::TEXT, &p.info)),
                ("printer-location".into(), s(tag::TEXT, &p.location)),
                (
                    "printer-make-and-model".into(),
                    s(tag::TEXT, "Example Laser"),
                ),
                ("device-uri".into(), s(tag::URI, &p.uri)),
                ("printer-state".into(), vec![Out::Enum(p.state)]),
                ("printer-state-reasons".into(), reasons),
                (
                    "printer-is-accepting-jobs".into(),
                    vec![Out::Bool(p.accepting)],
                ),
            ],
        )
    }

    fn handle(&mut self, path: &str, req: &Group, op_id: u16, data: Vec<u8>) -> Vec<u8> {
        self.calls.push((path.to_string(), op_id));
        let target = name_of(&req.text("printer-uri"));
        let find = |w: &World| w.printers.iter().position(|p| p.name == target);
        match op_id {
            op::CUPS_GET_PRINTERS => {
                let groups = self
                    .printers
                    .iter()
                    .map(|p| self.printer_group(p))
                    .collect();
                answer(0, groups)
            }
            op::CUPS_GET_DEFAULT => match &self.default {
                Some(d) => answer(
                    0,
                    vec![(tag::PRINTER, vec![("printer-name".into(), s(tag::NAME, d))])],
                ),
                None => not_found("no default destination"),
            },
            op::GET_JOBS => {
                let groups = self
                    .jobs
                    .iter()
                    .map(|(id, p, title, owner, st)| {
                        (
                            tag::JOB,
                            vec![
                                ("job-id".into(), vec![Out::Int(*id)]),
                                ("job-name".into(), s(tag::NAME, title)),
                                ("job-state".into(), vec![Out::Enum(*st)]),
                                ("job-originating-user-name".into(), s(tag::NAME, owner)),
                                (
                                    "job-printer-uri".into(),
                                    s(tag::URI, &format!("ipp://localhost/printers/{p}")),
                                ),
                                ("job-k-octets".into(), vec![Out::Int(12)]),
                            ],
                        )
                    })
                    .collect();
                answer(0, groups)
            }
            op::CUPS_ADD_MODIFY_PRINTER => {
                let attrs = |name: &str| {
                    // The printer group's attributes come after the operation group.
                    self.last_printer_group
                        .as_ref()
                        .and_then(|g| g.str(name).map(str::to_string))
                };
                let info = attrs("printer-info");
                let location = attrs("printer-location");
                let uri = attrs("device-uri");
                let driver = attrs("ppd-name");
                let i = match find(self) {
                    Some(i) => i,
                    None => {
                        self.printers.push(Printer {
                            name: target.clone(),
                            state: 3,
                            accepting: true,
                            ppd: Some(PPD.as_bytes().to_vec()),
                            ..Default::default()
                        });
                        self.printers.len() - 1
                    }
                };
                let p = &mut self.printers[i];
                if let Some(v) = info {
                    p.info = v;
                }
                if let Some(v) = location {
                    p.location = v;
                }
                if let Some(v) = uri {
                    p.uri = v;
                }
                if let Some(v) = driver {
                    p.driver = v;
                }
                if !data.is_empty() {
                    p.ppd = Some(data.clone());
                    self.uploaded = Some(data);
                }
                answer(0, vec![])
            }
            op::CUPS_DELETE_PRINTER
            | op::CUPS_SET_DEFAULT
            | op::PAUSE_PRINTER
            | op::RESUME_PRINTER
            | op::CUPS_ACCEPT_JOBS
            | op::PRINT_JOB => {
                let Some(i) = find(self) else {
                    return not_found(&format!("printer \"{target}\" does not exist"));
                };
                match op_id {
                    op::CUPS_DELETE_PRINTER => {
                        self.printers.remove(i);
                        if self.default.as_deref() == Some(target.as_str()) {
                            self.default = None;
                        }
                    }
                    op::CUPS_SET_DEFAULT => self.default = Some(target),
                    op::PAUSE_PRINTER => {
                        self.printers[i].state = 5;
                        self.printers[i].reasons = vec!["paused".into()];
                    }
                    op::RESUME_PRINTER => {
                        self.printers[i].state = 3;
                        self.printers[i].reasons.clear();
                    }
                    op::CUPS_ACCEPT_JOBS => self.printers[i].accepting = true,
                    _ => self.printed.push((
                        target,
                        req.text("job-name"),
                        req.text("document-format"),
                        data.len(),
                    )),
                }
                answer(0, vec![])
            }
            op::CANCEL_JOB => {
                let id: i32 = name_of(&req.text("job-uri")).parse().unwrap_or(-1);
                self.jobs.retain(|j| j.0 != id);
                answer(0, vec![])
            }
            op::CUPS_GET_DEVICES => {
                let groups = self
                    .devices
                    .iter()
                    .map(|(uri, info, mm, id)| {
                        (
                            tag::PRINTER,
                            vec![
                                ("device-class".into(), s(tag::KEYWORD, "network")),
                                ("device-info".into(), s(tag::TEXT, info)),
                                ("device-make-and-model".into(), s(tag::TEXT, mm)),
                                ("device-uri".into(), s(tag::URI, uri)),
                                ("device-id".into(), s(tag::TEXT, id)),
                            ],
                        )
                    })
                    .collect();
                answer(0, groups)
            }
            op::CUPS_GET_PPDS => {
                let groups = self
                    .ppds
                    .iter()
                    .map(|(n, mm)| {
                        (
                            tag::PRINTER,
                            vec![
                                ("ppd-name".into(), s(tag::NAME, n)),
                                ("ppd-make-and-model".into(), s(tag::TEXT, mm)),
                                ("ppd-natural-language".into(), s(tag::LANGUAGE, "en")),
                            ],
                        )
                    })
                    .collect();
                answer(0, groups)
            }
            other => panic!("the fake doesn't know operation {other:#06x}"),
        }
    }
}

impl Transport for FakeCups {
    fn ipp(
        &self,
        path: &'static str,
        body: Vec<u8>,
        _timeout: Duration,
    ) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let (req, data) = ipp::decode_request(&body).expect("a well-formed request");
        let mut w = self.0.lock().unwrap();
        let out = if path == "/admin/" && !w.admin {
            w.calls.push((path.to_string(), req.status));
            Err(CupsError::NotAllowed)
        } else {
            let opg = req.groups(tag::OPERATION).next().cloned().unwrap();
            assert_eq!(opg.str("requesting-user-name"), Some("me"));
            w.last_printer_group = req.groups(tag::PRINTER).next().cloned();
            Ok(w.handle(path, &opg, req.status, data))
        };
        async move { out }.boxed()
    }

    fn get(&self, path: String) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let w = self.0.lock().unwrap();
        let name = path
            .strip_prefix("/printers/")
            .and_then(|p| p.strip_suffix(".ppd"))
            .map(name_of)
            .unwrap_or_default();
        let out = w
            .printers
            .iter()
            .find(|p| p.name == name)
            .and_then(|p| p.ppd.clone())
            .ok_or_else(|| CupsError::Failed("no driver file".into()));
        async move { out }.boxed()
    }
}

const PPD: &str = "*PPD-Adobe: \"4.3\"\n\
*OpenUI *PageSize/Media Size: PickOne\n\
*DefaultPageSize: Letter\n\
*PageSize Letter/US Letter: \"\"\n\
*PageSize A4/A4: \"\"\n\
*CloseUI: *PageSize\n\
*OpenUI *Duplex/2-Sided Printing: PickOne\n\
*DefaultDuplex: None\n\
*Duplex None/Off: \"\"\n\
*Duplex DuplexNoTumble/Long edge: \"\"\n\
*CloseUI: *Duplex\n";

struct Rig {
    cups: Arc<FakeCups>,
    page: PrintersState,
    _root: tempfile::TempDir,
}

impl Rig {
    fn world(&self) -> std::sync::MutexGuard<'_, World> {
        self.cups.0.lock().unwrap()
    }
}

fn rig(world: World) -> Rig {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(root.path().join("usr/share/cups/data")).unwrap();
    std::fs::write(
        root.path().join(TEST_PAGE),
        "#CUPS-BANNER\nShow test-page\n",
    )
    .unwrap();
    let cups = Arc::new(FakeCups(StdMutex::new(world)));
    let page = PrintersState::with(
        cups.clone(),
        "me".into(),
        root.path().to_path_buf(),
        SystemBus::at("unix:path=/nonexistent/bus"),
    );
    Rig {
        cups,
        page,
        _root: root,
    }
}

fn office() -> World {
    World {
        printers: vec![
            Printer {
                name: "PDF".into(),
                info: "PDF".into(),
                uri: "cups-pdf:/".into(),
                state: 3,
                accepting: true,
                ppd: Some(PPD.as_bytes().to_vec()),
                ..Default::default()
            },
            Printer {
                name: "Hall_Laser".into(),
                info: "Hall Laser".into(),
                location: "Hall".into(),
                uri: "ipp://192.0.2.7/ipp/print".into(),
                state: 5,
                reasons: vec![
                    "paused".into(),
                    "media-empty-error".into(),
                    "toner-low-report".into(),
                ],
                accepting: true,
                ppd: Some(PPD.as_bytes().to_vec()),
                ..Default::default()
            },
        ],
        default: Some("Hall_Laser".into()),
        jobs: vec![
            (
                3,
                "Hall_Laser".into(),
                "Quarterly report.pdf".into(),
                "me".into(),
                3,
            ),
            (4, "Hall_Laser".into(), String::new(), "sam".into(), 3),
            (5, "PDF".into(), "Letter".into(), "me".into(), 5),
        ],
        admin: true,
        ..Default::default()
    }
}

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

struct Down(CupsError);

impl Transport for Down {
    fn ipp(
        &self,
        _: &'static str,
        _: Vec<u8>,
        _: Duration,
    ) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let e = self.0.clone();
        async move { Err(e) }.boxed()
    }
    fn get(&self, _: String) -> BoxFuture<'static, Result<Vec<u8>, CupsError>> {
        let e = self.0.clone();
        async move { Err(e) }.boxed()
    }
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
