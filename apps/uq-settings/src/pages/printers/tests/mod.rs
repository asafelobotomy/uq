//! The Printers page against a fake CUPS that decodes each request and answers like CUPS.

use std::sync::Mutex as StdMutex;

use futures_util::FutureExt;
use futures_util::future::BoxFuture;

use super::cups::{CupsError, Transport};
use super::ipp::{self, Group, Out, Request, op, tag};
use super::*;
use crate::pages::runner::run;
mod fake_cups;
use fake_cups::*;

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

mod add;
mod manage;
