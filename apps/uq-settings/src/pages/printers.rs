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
mod add;
mod read;
mod words;
pub use words::*;

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
