//! The Printers page: the printers CUPS knows, which one is the default, what each is printing
//! and what is waiting; pause, resume, print a test page, cancel documents; find printers on the
//! network or plugged in and add them (driverless when the printer can, else with a matching
//! driver); change a printer's name, location and default options; remove it.
//!
//! Everything goes to CUPS over its own protocol on its local socket ([`cups`]), as the person
//! running Settings. CUPS decides who may change printers (its administrators: the `wheel`
//! group on Arch, which is what uQ's Administrator account type is); a refusal is shown in
//! words. Printer names, messages and document titles come from printers and other people's
//! apps and are shown through UqLabel (plain text) only.
//!
//! Looking for printers takes several seconds (CUPS asks the network and every USB port), so it
//! runs in the background: the page stays usable and shows "Looking…" until it ends. While the
//! page is open it reads CUPS's state every few seconds, so a document that finishes or a
//! printer that runs out of paper shows without a refresh.

pub(crate) mod cups;
pub(crate) mod ipp;
pub(crate) mod ppd;

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;

use super::runner::PageLogic;
use super::sysbus::{self, Service, SystemBus, failure};
use cups::{CHANGE_TIMEOUT, Cups, CupsError, READ_TIMEOUT, Socket, Transport};
use ipp::{Out, op, tag};

pub const PRINTING: Service = Service {
    name: "Printing",
    package: "cups",
};

/// How often the page reads CUPS's state while it is open.
pub const POLL: Duration = Duration::from_secs(3);
/// How long CUPS looks for printers (its own limit; the call allows a little more).
const LOOK_SECONDS: i32 = 8;
/// CUPS's own test page, under the page's root.
const TEST_PAGE: &str = "usr/share/cups/data/testprint";
/// The classic printer tool, offered for the rare printer this page can't set up.
const CLASSIC_TOOL: &str = "usr/bin/system-config-printer";
/// The most drivers offered for one printer.
const DRIVER_LIMIT: i32 = 30;

#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Refresh,
    /// Look for printers on the network and the USB ports (in the background).
    Look,
    /// Find the drivers that suit a found printer.
    Drivers {
        uri: String,
        device_id: String,
        make_model: String,
    },
    Add {
        uri: String,
        /// The name people see; the queue's own name is made from it.
        name: String,
        location: String,
        /// `everywhere` (driverless) or a driver's name from [`Op::Drivers`].
        driver: String,
    },
    Remove(String),
    SetDefault(String),
    Pause(String),
    Resume(String),
    TestPage(String),
    CancelJob(i32),
    /// Cancel every document waiting on one printer.
    CancelAll(String),
    /// Read a printer's options for its settings dialog.
    LoadOptions(String),
    Save {
        printer: String,
        name: String,
        location: String,
        /// Option key → choice; only the ones that changed.
        options: BTreeMap<String, String>,
    },
    /// Switch the printing service on (`cups.socket`), asking for an administrator.
    TurnOn,
}

/// What the page and its background search share.
#[derive(Default)]
struct Shared {
    looking: AtomicBool,
    /// The last search's printers; `None` before the first.
    found: StdMutex<Option<Vec<Value>>>,
    /// `{printer, options: […]}` for the open settings dialog.
    options: StdMutex<Value>,
    /// `{uri, drivers: […]}` for the open add dialog.
    drivers: StdMutex<Value>,
    notice: StdMutex<(u64, String)>,
    changed: StdMutex<Option<Arc<Notify>>>,
}

impl Shared {
    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    fn ping(&self) {
        if let Some(n) = &*self.changed.lock().unwrap() {
            n.notify_one();
        }
    }
}

pub struct PrintersState {
    cups: Arc<Cups>,
    root: PathBuf,
    system: SystemBus,
    write: Mutex<()>,
    shared: Arc<Shared>,
    tasks: StdMutex<Vec<JoinHandle<()>>>,
}

impl Drop for PrintersState {
    fn drop(&mut self) {
        for t in self.tasks.lock().unwrap().drain(..) {
            t.abort();
        }
    }
}

/// The login name of the person running Settings: CUPS checks it against the socket's peer.
pub fn login_name() -> String {
    let uid = sysbus::my_uid();
    std::fs::read_to_string("/etc/passwd")
        .ok()
        .and_then(|p| {
            p.lines().find_map(|l| {
                let f: Vec<&str> = l.split(':').collect();
                (f.len() > 2 && f[2].parse::<u32>().ok() == Some(uid)).then(|| f[0].to_string())
            })
        })
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("LOGNAME").ok())
        .unwrap_or_default()
}

/// What a printer is doing, in words: `(state key, status words)`. CUPS drops the `paused`
/// reason when a paused printer's settings change, but keeps its "Paused" message.
fn state_words(state: i32, reasons: &[String], message: &str) -> (&'static str, &'static str) {
    let paused_message = message
        .trim()
        .trim_end_matches('.')
        .eq_ignore_ascii_case("paused");
    match state {
        4 => ("printing", "Printing"),
        5 if paused_message || reasons.iter().any(|r| r == "paused") => ("paused", "Paused"),
        5 => ("stopped", "Stopped"),
        _ => ("idle", "Ready"),
    }
}

/// A printer's reason (`media-empty-error`) in words; `None` for ones not worth showing.
pub fn reason_words(reason: &str) -> Option<&'static str> {
    let r = reason
        .strip_suffix("-error")
        .or_else(|| reason.strip_suffix("-warning"))
        .or_else(|| reason.strip_suffix("-report"))
        .unwrap_or(reason);
    Some(match r {
        "offline" | "shutdown" | "timed-out" => "The printer can't be reached",
        "connecting-to-device" => "Connecting to the printer",
        "media-empty" | "media-needed" => "Out of paper",
        "media-low" => "Paper is low",
        "media-jam" => "Paper jam",
        "toner-low" | "marker-supply-low" => "Ink or toner is low",
        "toner-empty" | "marker-supply-empty" => "Out of ink or toner",
        "door-open" | "cover-open" | "interlock-open" => "A cover or door is open",
        "output-area-full" => "The output tray is full",
        "spool-area-full" => "Too many documents are waiting",
        "cups-missing-filter" => "Part of its driver is missing",
        "cups-insecure-filter" => "Part of its driver was refused as unsafe",
        _ => return None,
    })
}

/// How a printer is connected, in words, from its address.
pub fn connection_words(uri: &str) -> &'static str {
    let scheme = uri.split(':').next().unwrap_or("");
    match scheme {
        "usb" | "hp" | "hpfax" => "USB",
        "cups-pdf" => "Saves PDF files",
        "file" => "Saves to a file",
        "ipp" | "ipps" | "http" | "https" | "dnssd" | "socket" | "lpd" | "smb"
        | "implicitclass" => {
            if uri.contains("://localhost:6") {
                // ipp-usb serves USB printers on localhost ports from 60000.
                "USB"
            } else {
                "Network"
            }
        }
        "" => "Not set",
        _ => "Other",
    }
}

/// Whether a printer at `uri` can be added without a driver (IPP Everywhere).
pub fn driverless(uri: &str) -> bool {
    let scheme = uri.split(':').next().unwrap_or("");
    match scheme {
        "ipp" | "ipps" => true,
        "dnssd" => uri.contains("._ipp.") || uri.contains("._ipps."),
        _ => false,
    }
}

/// A queue name for `name`: CUPS takes up to 127 printable characters other than space, `/`,
/// `\`, `#`, `?` and quotes; anything else becomes `_`. Not one of `taken` (case ignored).
pub fn queue_name(name: &str, taken: &[String]) -> String {
    let mut base: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_graphic() && !"/\\#?'\"".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    while base.contains("__") {
        base = base.replace("__", "_");
    }
    let base = base.trim_matches('_').to_string();
    let base = if base.is_empty() {
        "Printer".to_string()
    } else {
        base
    };
    let free = |n: &str| !taken.iter().any(|t| t.eq_ignore_ascii_case(n));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|i| format!("{base}_{i}"))
        .find(|n| free(n))
        .expect("an unused name")
}

fn job_state_words(state: i32) -> &'static str {
    match state {
        3 => "Waiting",
        4 => "On hold",
        5 => "Printing",
        6 => "Stopped",
        7 => "Cancelled",
        8 => "Failed",
        9 => "Done",
        _ => "",
    }
}

fn str_out(t: u8, s: &str) -> Out {
    Out::Str(t, s.to_string())
}

fn keywords(list: &[&str]) -> Vec<Out> {
    list.iter().map(|k| str_out(tag::KEYWORD, k)).collect()
}

fn shown(e: CupsError) -> zbus::Error {
    failure(e.text())
}

impl PrintersState {
    pub fn new(_session: zbus::Connection) -> Self {
        let user = login_name();
        Self::with(
            Arc::new(Socket::system(user.clone())),
            user,
            PathBuf::from("/"),
            SystemBus::system(),
        )
    }

    pub fn with(t: Arc<dyn Transport>, user: String, root: PathBuf, system: SystemBus) -> Self {
        Self {
            cups: Arc::new(Cups { t, user }),
            root,
            system,
            write: Mutex::new(()),
            shared: Arc::new(Shared::default()),
            tasks: StdMutex::new(Vec::new()),
        }
    }

    async fn printers(&self) -> Result<Vec<ipp::Group>, CupsError> {
        let mut r = self.cups.request(op::CUPS_GET_PRINTERS);
        r.op_attrs(
            "requested-attributes",
            keywords(&[
                "printer-name",
                "printer-info",
                "printer-location",
                "printer-make-and-model",
                "device-uri",
                "printer-state",
                "printer-state-reasons",
                "printer-state-message",
                "printer-is-accepting-jobs",
                "printer-type",
            ]),
        );
        let resp = self.cups.send("/", &r, READ_TIMEOUT).await?;
        Ok(resp.groups(tag::PRINTER).cloned().collect())
    }

    async fn default_name(&self) -> Result<Option<String>, CupsError> {
        let r = self.cups.request(op::CUPS_GET_DEFAULT);
        match self.cups.send("/", &r, READ_TIMEOUT).await {
            Ok(resp) => Ok(resp
                .groups(tag::PRINTER)
                .find_map(|g| g.str("printer-name").map(str::to_string))),
            // No default printer is "not found".
            Err(CupsError::Failed(_)) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn jobs(&self) -> Result<Vec<ipp::Group>, CupsError> {
        let mut r = self.cups.request(op::GET_JOBS);
        r.op_attr("printer-uri", str_out(tag::URI, "ipp://localhost/"))
            .op_attr("which-jobs", str_out(tag::KEYWORD, "not-completed"))
            .op_attr("my-jobs", Out::Bool(false))
            .op_attrs(
                "requested-attributes",
                keywords(&[
                    "job-id",
                    "job-name",
                    "job-state",
                    "job-originating-user-name",
                    "job-printer-uri",
                    "job-k-octets",
                ]),
            );
        let resp = self.cups.send("/", &r, READ_TIMEOUT).await?;
        Ok(resp.groups(tag::JOB).cloned().collect())
    }

    async fn read(&self) -> Result<Value, CupsError> {
        let printers = self.printers().await?;
        let default = self.default_name().await?;
        let jobs = self.jobs().await?;
        let mut by_printer: HashMap<String, Vec<Value>> = HashMap::new();
        for j in &jobs {
            let printer = j
                .text("job-printer-uri")
                .rsplit('/')
                .next()
                .map(|s| {
                    percent_encoding::percent_decode_str(s)
                        .decode_utf8_lossy()
                        .into_owned()
                })
                .unwrap_or_default();
            let owner = j.text("job-originating-user-name");
            let title = j.text("job-name");
            let state = j.int("job-state").unwrap_or(0);
            by_printer.entry(printer).or_default().push(json!({
                "id": j.int("job-id").unwrap_or(0),
                "title": if title.is_empty() { "Untitled document".to_string() } else { title },
                "owner": owner,
                "mine": owner == self.cups.user,
                "kb": j.int("job-k-octets").unwrap_or(0),
                "state": job_state_words(state),
                "printing": state == 5,
            }));
        }
        let mut list: Vec<Value> = printers
            .iter()
            .map(|p| {
                let name = p.text("printer-name");
                let reasons = p.strs("printer-state-reasons");
                let (state, status) = state_words(
                    p.int("printer-state").unwrap_or(3),
                    &reasons,
                    &p.text("printer-state-message"),
                );
                let mut problems: Vec<&str> =
                    reasons.iter().filter_map(|r| reason_words(r)).collect();
                problems.dedup();
                let uri = p.text("device-uri");
                let info = p.text("printer-info");
                // CUPS's own "Paused." or "Printing." says no more than the status does.
                let message = p.text("printer-state-message");
                let said = message.trim().trim_end_matches('.').to_lowercase();
                let message = if ["", "paused", "idle", "ready", "printing", "stopped"]
                    .contains(&said.as_str())
                    || said == status.to_lowercase()
                {
                    String::new()
                } else {
                    message
                };
                json!({
                    "name": name,
                    "info": if info.is_empty() { name.clone() } else { info },
                    "location": p.text("printer-location"),
                    "model": p.text("printer-make-and-model"),
                    "uri": uri,
                    "connection": connection_words(&uri),
                    "state": state,
                    "status": status,
                    "message": message,
                    "problems": problems,
                    "accepting": p.bool("printer-is-accepting-jobs").unwrap_or(true),
                    "default": default.as_deref() == Some(name.as_str()),
                    // printer-type bit 0: a class (a group of printers), not one printer.
                    "group": p.int("printer-type").unwrap_or(0) & 1 == 1,
                    "jobs": by_printer.remove(&name).unwrap_or_default(),
                })
            })
            .collect();
        list.sort_by_key(|p| {
            (
                p["default"] != true,
                p["info"].as_str().unwrap_or("").to_lowercase(),
            )
        });
        let added: Vec<String> = list
            .iter()
            .filter_map(|p| p["uri"].as_str().map(str::to_string))
            .collect();
        let found = self.shared.found.lock().unwrap().clone().map(|f| {
            f.into_iter()
                .map(|mut d| {
                    let uri = d["uri"].as_str().unwrap_or("").to_string();
                    d["added"] = json!(added.contains(&uri));
                    d
                })
                .collect::<Vec<_>>()
        });
        Ok(json!({
            "available": true,
            "printers": list,
            "default": default,
            "found": found,
        }))
    }

    async fn admin(&self, r: &ipp::Request) -> zbus::Result<ipp::Response> {
        self.cups
            .send("/admin/", r, CHANGE_TIMEOUT)
            .await
            .map_err(shown)
    }

    fn printer_request(&self, op: u16, printer: &str) -> ipp::Request {
        let mut r = self.cups.request(op);
        r.op_attr(
            "printer-uri",
            str_out(tag::URI, &Cups::printer_uri(printer)),
        );
        r
    }

    /// Look for printers in the background; the result lands in `found`.
    fn look(&self) {
        if self.shared.looking.swap(true, Ordering::SeqCst) {
            return;
        }
        let cups = self.cups.clone();
        let shared = self.shared.clone();
        let task = async move {
            let mut r = cups.request(op::CUPS_GET_DEVICES);
            r.op_attr("timeout", Out::Int(LOOK_SECONDS))
                .op_attrs("exclude-schemes", vec![str_out(tag::NAME, "beh")]);
            let wait = Duration::from_secs(u64::try_from(LOOK_SECONDS).unwrap_or(8) + 20);
            match cups.send("/admin/", &r, wait).await {
                Ok(resp) => {
                    let mut seen: Vec<String> = Vec::new();
                    let mut list = Vec::new();
                    for d in resp.groups(tag::PRINTER) {
                        let uri = d.text("device-uri");
                        // A bare scheme ("ipp", "socket") is a way to add one by hand, not a
                        // printer that was found.
                        if !uri.contains(':') || seen.contains(&uri) {
                            continue;
                        }
                        seen.push(uri.clone());
                        let model = d.text("device-make-and-model");
                        let model = if model == "Unknown" {
                            String::new()
                        } else {
                            model
                        };
                        let info = d.text("device-info");
                        list.push(json!({
                            "uri": uri,
                            "info": if info.is_empty() { model.clone() } else { info },
                            "model": model,
                            "device_id": d.text("device-id"),
                            "location": d.text("device-location"),
                            "connection": connection_words(&uri),
                            "driverless": driverless(&uri),
                        }));
                    }
                    list.sort_by_key(|d| {
                        (
                            d["driverless"] != true,
                            d["info"].as_str().unwrap_or("").to_lowercase(),
                        )
                    });
                    *shared.found.lock().unwrap() = Some(list);
                }
                Err(e) => shared.notice(&format!("Looking for printers didn't work: {}", e.text())),
            }
            shared.looking.store(false, Ordering::SeqCst);
            shared.ping();
        };
        let mut tasks = self.tasks.lock().unwrap();
        tasks.retain(|t| !t.is_finished());
        tasks.push(tokio::spawn(task));
    }

    async fn drivers(&self, uri: &str, device_id: &str, make_model: &str) -> zbus::Result<()> {
        let mut list = Vec::new();
        if driverless(uri) {
            list.push(json!({"name": "everywhere", "label": "Driverless (recommended)"}));
        }
        {
            let mut r = self.cups.request(op::CUPS_GET_PPDS);
            r.op_attr("limit", Out::Int(DRIVER_LIMIT)).op_attrs(
                "requested-attributes",
                keywords(&["ppd-name", "ppd-make-and-model", "ppd-natural-language"]),
            );
            if !device_id.is_empty() {
                r.op_attr("ppd-device-id", str_out(tag::TEXT, device_id));
            } else if !make_model.is_empty() {
                r.op_attr("ppd-make-and-model", str_out(tag::TEXT, make_model));
            }
            if !device_id.is_empty() || !make_model.is_empty() {
                let resp = self
                    .cups
                    .send("/", &r, CHANGE_TIMEOUT)
                    .await
                    .map_err(shown)?;
                for d in resp.groups(tag::PRINTER) {
                    let name = d.text("ppd-name");
                    if name.is_empty() || name == "everywhere" {
                        continue;
                    }
                    let lang = d.text("ppd-natural-language");
                    if !lang.is_empty() && lang != "en" && !lang.starts_with("en") {
                        continue;
                    }
                    list.push(json!({"name": name, "label": d.text("ppd-make-and-model")}));
                }
            }
        }
        *self.shared.drivers.lock().unwrap() = json!({"uri": uri, "drivers": list});
        Ok(())
    }

    async fn add(&self, uri: &str, name: &str, location: &str, driver: &str) -> zbus::Result<()> {
        let uri = uri.trim();
        if uri.is_empty() || !uri.contains(':') {
            return Err(failure("Give the printer's address."));
        }
        if driver.is_empty() {
            return Err(failure("Choose a driver for this printer."));
        }
        let printers = self.printers().await.map_err(shown)?;
        let taken: Vec<String> = printers.iter().map(|p| p.text("printer-name")).collect();
        let queue = queue_name(name, &taken);
        let info = if name.trim().is_empty() {
            queue.clone()
        } else {
            name.trim().to_string()
        };
        let mut r = self.printer_request(op::CUPS_ADD_MODIFY_PRINTER, &queue);
        r.printer_attr("device-uri", str_out(tag::URI, uri))
            .printer_attr("printer-info", str_out(tag::TEXT, &info))
            .printer_attr("printer-location", str_out(tag::TEXT, location.trim()))
            .printer_attr("ppd-name", str_out(tag::NAME, driver))
            .printer_attr("printer-is-accepting-jobs", Out::Bool(true))
            .printer_attr("printer-state", Out::Enum(3));
        self.admin(&r).await?;
        // The first printer becomes the default, as people expect.
        if self.default_name().await.ok().flatten().is_none() {
            let r = self.printer_request(op::CUPS_SET_DEFAULT, &queue);
            self.admin(&r).await?;
        }
        self.shared.notice(&format!("{info} was added."));
        Ok(())
    }

    async fn test_page(&self, printer: &str) -> zbus::Result<()> {
        let data = std::fs::read(self.root.join(TEST_PAGE))
            .map_err(|_| failure("The printing service's test page is missing."))?;
        let mut r = self.printer_request(op::PRINT_JOB, printer);
        // "Find out yourself": CUPS types the file as its banner format and converts it. Naming
        // that format outright is refused, since printers don't list it as one they take.
        r.op_attr("job-name", str_out(tag::NAME, "Test page"))
            .op_attr(
                "document-format",
                str_out(tag::MIME_TYPE, "application/octet-stream"),
            );
        r.data = data;
        self.cups
            .send("/", &r, CHANGE_TIMEOUT)
            .await
            .map_err(shown)?;
        self.shared.notice("The test page is on its way.");
        Ok(())
    }

    async fn cancel(&self, id: i32) -> Result<(), CupsError> {
        let mut r = self.cups.request(op::CANCEL_JOB);
        r.op_attr(
            "job-uri",
            str_out(tag::URI, &format!("ipp://localhost/jobs/{id}")),
        );
        self.cups.send("/", &r, CHANGE_TIMEOUT).await.map(|_| ())
    }

    async fn load_options(&self, printer: &str) -> zbus::Result<()> {
        let ppd = self
            .cups
            .t
            .get(format!("/printers/{}.ppd", cups::uri_escape(printer)))
            .await;
        let value = match ppd {
            Ok(ppd) => json!({
                "printer": printer,
                "options": ppd::options(&ppd).iter().map(ppd::PpdOption::to_json).collect::<Vec<_>>(),
            }),
            // A printer without a driver file (a raw queue) has no options to show.
            Err(CupsError::Failed(_)) => json!({"printer": printer, "options": []}),
            Err(e) => return Err(shown(e)),
        };
        *self.shared.options.lock().unwrap() = value;
        Ok(())
    }

    async fn save(
        &self,
        printer: &str,
        name: &str,
        location: &str,
        options: &BTreeMap<String, String>,
    ) -> zbus::Result<()> {
        let mut r = self.printer_request(op::CUPS_ADD_MODIFY_PRINTER, printer);
        let name = name.trim();
        r.printer_attr(
            "printer-info",
            str_out(tag::TEXT, if name.is_empty() { printer } else { name }),
        )
        .printer_attr("printer-location", str_out(tag::TEXT, location.trim()));
        if !options.is_empty() {
            let ppd = self
                .cups
                .t
                .get(format!("/printers/{}.ppd", cups::uri_escape(printer)))
                .await
                .map_err(shown)?;
            r.data = ppd::with_defaults(&ppd, options).map_err(failure)?;
        }
        self.admin(&r).await?;
        *self.shared.options.lock().unwrap() = Value::Null;
        Ok(())
    }

    /// Enable and start `cups.socket` through systemd, which asks polkit (an administrator).
    async fn turn_on(&self) -> zbus::Result<()> {
        let conn = self.system.conn().await?;
        let manager = zbus::proxy::Builder::<zbus::Proxy<'static>>::new(&conn)
            .destination("org.freedesktop.systemd1")
            .path("/org/freedesktop/systemd1")
            .interface("org.freedesktop.systemd1.Manager")
            .cache_properties(zbus::proxy::CacheProperties::No)
            .build()
            .await?;
        let r = async {
            manager
                .call_method("EnableUnitFiles", &(vec!["cups.socket"], false, true))
                .await?;
            manager
                .call_method("StartUnit", &("cups.socket", "replace"))
                .await?;
            zbus::Result::Ok(())
        }
        .await;
        sysbus::settle(r, &PRINTING)
    }
}

impl PageLogic for PrintersState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::Look => {
                self.look();
                Ok(())
            }
            Op::Drivers {
                uri,
                device_id,
                make_model,
            } => self.drivers(&uri, &device_id, &make_model).await,
            Op::Add {
                uri,
                name,
                location,
                driver,
            } => self.add(&uri, &name, &location, &driver).await,
            Op::Remove(p) => {
                let r = self.printer_request(op::CUPS_DELETE_PRINTER, &p);
                self.admin(&r).await.map(|_| ())
            }
            Op::SetDefault(p) => {
                let r = self.printer_request(op::CUPS_SET_DEFAULT, &p);
                self.admin(&r).await.map(|_| ())
            }
            Op::Pause(p) => {
                let r = self.printer_request(op::PAUSE_PRINTER, &p);
                self.admin(&r).await.map(|_| ())
            }
            Op::Resume(p) => {
                // Resume printing and take documents again: a printer can be stopped either way.
                let r = self.printer_request(op::RESUME_PRINTER, &p);
                self.admin(&r).await?;
                let r = self.printer_request(op::CUPS_ACCEPT_JOBS, &p);
                self.admin(&r).await.map(|_| ())
            }
            Op::TestPage(p) => self.test_page(&p).await,
            Op::CancelJob(id) => self.cancel(id).await.map_err(shown),
            Op::CancelAll(p) => {
                let ids: Vec<i32> = self
                    .jobs()
                    .await
                    .map_err(shown)?
                    .iter()
                    .filter(|j| {
                        j.text("job-printer-uri") == Cups::printer_uri(&p)
                            || j.text("job-printer-uri")
                                .ends_with(&format!("/printers/{}", cups::uri_escape(&p)))
                    })
                    .filter_map(|j| j.int("job-id"))
                    .collect();
                for id in ids {
                    self.cancel(id).await.map_err(shown)?;
                }
                Ok(())
            }
            Op::LoadOptions(p) => self.load_options(&p).await,
            Op::Save {
                printer,
                name,
                location,
                options,
            } => self.save(&printer, &name, &location, &options).await,
            Op::TurnOn => self.turn_on().await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let mut state = match self.read().await {
            Ok(v) => v,
            Err(e) => json!({
                "available": false,
                "off": e == CupsError::Off,
                "text": e.text(),
            }),
        };
        let (seq, text) = self.shared.notice.lock().unwrap().clone();
        state["notice"] = json!({"seq": seq, "text": text});
        state["looking"] = json!(self.shared.looking.load(Ordering::SeqCst));
        state["options"] = self.shared.options.lock().unwrap().clone();
        state["drivers"] = self.shared.drivers.lock().unwrap().clone();
        state["classic"] = json!(self.root.join(CLASSIC_TOOL).exists());
        Ok(state)
    }

    async fn watch(&self, changed: Arc<Notify>) {
        *self.shared.changed.lock().unwrap() = Some(changed.clone());
        loop {
            tokio::time::sleep(POLL).await;
            changed.notify_one();
        }
    }
}

#[cfg(test)]
mod tests;
