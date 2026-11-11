//! Welcome's Fingerprint step (Phase 5 spec §5.1 step 7): whether there is a reader with no
//! finger enrolled, and enrolling one through fprintd's D-Bus API, so the lock screen can use it.
//!
//! fprintd asks polkit (`net.reactivated.fprint.device.enroll`) on `Claim`; the active local
//! user is normally allowed without a password.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use futures_util::StreamExt;
use serde::Serialize;
use tokio::sync::Notify;
use zbus::zvariant::OwnedObjectPath;

use crate::pages::sysbus::{READ_TIMEOUT, SystemBus};

/// fprintd's bus name (the proxies name it too).
#[cfg(test)]
pub const FPRINT: &str = "net.reactivated.Fprint";

#[zbus::proxy(
    interface = "net.reactivated.Fprint.Manager",
    default_service = "net.reactivated.Fprint",
    default_path = "/net/reactivated/Fprint/Manager"
)]
pub trait FprintManager {
    fn get_default_device(&self) -> zbus::Result<OwnedObjectPath>;
}

#[zbus::proxy(
    interface = "net.reactivated.Fprint.Device",
    default_service = "net.reactivated.Fprint"
)]
pub trait FprintDevice {
    /// "" is the calling user.
    fn list_enrolled_fingers(&self, username: &str) -> zbus::Result<Vec<String>>;
    #[zbus(allow_interactive_auth)]
    fn claim(&self, username: &str) -> zbus::Result<()>;
    fn release(&self) -> zbus::Result<()>;
    #[zbus(allow_interactive_auth)]
    fn enroll_start(&self, finger_name: &str) -> zbus::Result<()>;
    fn enroll_stop(&self) -> zbus::Result<()>;
    #[zbus(signal)]
    fn enroll_status(&self, result: &str, done: bool) -> zbus::Result<()>;
    #[zbus(property, name = "num-enroll-stages")]
    fn num_enroll_stages(&self) -> zbus::Result<i32>;
}

/// The finger Welcome enrols. Settings → Users can add others later.
pub const FINGER: &str = "right-index-finger";

/// What fprintd says about this computer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct Reader {
    /// fprintd answers and has a reader.
    pub present: bool,
    /// The user has at least one finger enrolled.
    pub enrolled: bool,
}

impl Reader {
    /// Welcome offers the step: a reader, and nothing enrolled yet.
    pub fn offer(&self) -> bool {
        self.present && !self.enrolled
    }
}

async fn device(sys: &SystemBus) -> zbus::Result<FprintDeviceProxy<'static>> {
    let conn = sys.conn().await?;
    let path = FprintManagerProxy::new(&conn)
        .await?
        .get_default_device()
        .await?;
    FprintDeviceProxy::builder(&conn).path(path)?.build().await
}

/// Ask fprintd, within a few seconds. No fprintd, no reader or no answer: no reader.
pub async fn reader(sys: &SystemBus) -> Reader {
    let ask = async {
        let dev = device(sys).await?;
        // fprintd answers NoEnrolledPrints when there are none.
        let enrolled = dev
            .list_enrolled_fingers("")
            .await
            .map(|f| !f.is_empty())
            .unwrap_or(false);
        zbus::Result::Ok(Reader {
            present: true,
            enrolled,
        })
    };
    tokio::time::timeout(READ_TIMEOUT, ask)
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default()
}

/// Where an enrolment is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    #[default]
    Idle,
    /// Touching the reader, stage by stage.
    Scanning,
    Done,
    Failed,
}

/// The enrolment as the step shows it.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize)]
pub struct Enroll {
    pub phase: Phase,
    /// Stages passed so far, and how many there are (0 when fprintd doesn't say).
    pub stage: u32,
    pub stages: u32,
    /// A plain line on what to do next, or what went wrong.
    pub message: String,
}

/// What an `EnrollStatus` result means: (phase, message, a stage passed).
pub fn status(result: &str) -> (Phase, &'static str, bool) {
    match result {
        "enroll-completed" => (Phase::Done, "Your fingerprint is saved.", true),
        "enroll-stage-passed" => (
            Phase::Scanning,
            "Good. Lift your finger and touch again.",
            true,
        ),
        "enroll-retry-scan" => (Phase::Scanning, "Try that again.", false),
        "enroll-swipe-too-short" => (
            Phase::Scanning,
            "That swipe was too short. Try again.",
            false,
        ),
        "enroll-finger-not-centered" => (
            Phase::Scanning,
            "Put your finger in the middle of the reader.",
            false,
        ),
        "enroll-remove-and-retry" => (Phase::Scanning, "Lift your finger and try again.", false),
        "enroll-data-full" => (
            Phase::Failed,
            "The reader has no room for another fingerprint.",
            false,
        ),
        "enroll-duplicate" => (Phase::Failed, "That finger is already saved.", false),
        "enroll-disconnected" => (Phase::Failed, "The reader was disconnected.", false),
        _ => (Phase::Failed, "The fingerprint couldn't be saved.", false),
    }
}

/// One enrolment at a time, shared with the page: `state` is what the step shows, `changed`
/// wakes whoever shows it, `cancel` stops a scan in progress.
#[derive(Clone, Default)]
pub struct Enrolment {
    pub state: Arc<Mutex<Enroll>>,
    pub changed: Arc<Notify>,
    cancel: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
}

impl Enrolment {
    pub fn get(&self) -> Enroll {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set(&self, e: Enroll) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = e;
        self.changed.notify_waiters();
    }

    pub fn busy(&self) -> bool {
        self.get().phase == Phase::Scanning
    }

    /// Stop a scan in progress (the run ends quietly, back to idle).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.cancel.notify_waiters();
    }

    /// Claim the reader, enrol [`FINGER`] and release it, reporting each step. Errors end in
    /// [`Phase::Failed`] with a plain message.
    pub async fn run(&self, sys: &SystemBus) {
        self.cancelled.store(false, Ordering::SeqCst);
        self.set(Enroll {
            phase: Phase::Scanning,
            message: "Touch the reader with your right index finger.".into(),
            ..Enroll::default()
        });
        let dev = match device(sys).await {
            Ok(d) => d,
            Err(_) => return self.fail("The fingerprint reader isn't answering."),
        };
        let stages = dev
            .num_enroll_stages()
            .await
            .ok()
            .and_then(|n| u32::try_from(n).ok())
            .unwrap_or(0);
        if dev.claim("").await.is_err() {
            return self.fail("The fingerprint reader is busy or wasn't allowed.");
        }
        // Subscribe before starting, so the first result isn't missed.
        let mut statuses = match dev.receive_enroll_status().await {
            Ok(s) => s,
            Err(_) => {
                let _ = dev.release().await;
                return self.fail("The fingerprint reader isn't answering.");
            }
        };
        if dev.enroll_start(FINGER).await.is_err() {
            let _ = dev.release().await;
            return self.fail("The fingerprint couldn't be saved.");
        }
        let mut stage = 0;
        self.set(Enroll {
            phase: Phase::Scanning,
            stage,
            stages,
            message: "Touch the reader with your right index finger.".into(),
        });
        let cancelled = loop {
            // Registered before the flag is read, so a cancel in between still wakes it.
            let cancel = self.cancel.notified();
            if self.cancelled.load(Ordering::SeqCst) {
                break true;
            }
            let next = tokio::select! {
                _ = cancel => break true,
                s = statuses.next() => s,
            };
            let Some(signal) = next else {
                self.fail("The fingerprint reader stopped answering.");
                break false;
            };
            let Ok(args) = signal.args() else { continue };
            let (phase, message, passed) = status(args.result());
            if passed {
                stage += 1;
            }
            let done = args.done || phase != Phase::Scanning;
            self.set(Enroll {
                phase,
                stage: if stages > 0 { stage.min(stages) } else { stage },
                stages,
                message: message.into(),
            });
            if done {
                break false;
            }
        };
        let _ = dev.enroll_stop().await;
        let _ = dev.release().await;
        if cancelled {
            self.set(Enroll::default());
        }
    }

    fn fail(&self, message: &str) {
        self.set(Enroll {
            phase: Phase::Failed,
            message: message.into(),
            ..self.get()
        });
    }
}

#[cfg(test)]
pub mod fake {
    //! A fake fprintd on a private "system" bus: one reader, scripted enrol results.

    use std::sync::{Arc, Mutex};

    use zbus::object_server::SignalEmitter;
    use zbus::zvariant::OwnedObjectPath;

    pub const DEVICE: &str = "/net/reactivated/Fprint/Device/0";

    #[derive(Clone, Default)]
    pub struct Calls(pub Arc<Mutex<Vec<String>>>);

    impl Calls {
        pub fn push(&self, s: impl Into<String>) {
            self.0.lock().unwrap().push(s.into());
        }
        pub fn all(&self) -> Vec<String> {
            self.0.lock().unwrap().clone()
        }
    }

    pub struct Manager;

    #[zbus::interface(name = "net.reactivated.Fprint.Manager")]
    impl Manager {
        fn get_default_device(&self) -> OwnedObjectPath {
            OwnedObjectPath::try_from(DEVICE).unwrap()
        }
    }

    pub struct Device {
        pub enrolled: Arc<Mutex<Vec<String>>>,
        /// What `EnrollStart` reports, in order; an empty script reports nothing (a scan that
        /// waits for the finger).
        pub script: Vec<(&'static str, bool)>,
        pub calls: Calls,
    }

    #[zbus::interface(name = "net.reactivated.Fprint.Device")]
    impl Device {
        fn list_enrolled_fingers(&self, _username: &str) -> zbus::fdo::Result<Vec<String>> {
            let f = self.enrolled.lock().unwrap().clone();
            if f.is_empty() {
                return Err(zbus::fdo::Error::Failed("NoEnrolledPrints".into()));
            }
            Ok(f)
        }
        fn claim(&self, username: &str) {
            self.calls.push(format!("claim {username}"));
        }
        fn release(&self) {
            self.calls.push("release");
        }
        async fn enroll_start(
            &self,
            finger_name: &str,
            #[zbus(signal_emitter)] e: SignalEmitter<'_>,
        ) {
            self.calls.push(format!("enroll {finger_name}"));
            let script = self.script.clone();
            let enrolled = self.enrolled.clone();
            let finger = finger_name.to_string();
            let e = e.to_owned();
            tokio::spawn(async move {
                for (result, done) in script {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                    if result == "enroll-completed" {
                        enrolled.lock().unwrap().push(finger.clone());
                    }
                    let _ = Device::enroll_status(&e, result, done).await;
                }
            });
        }
        fn enroll_stop(&self) {
            self.calls.push("stop");
        }
        #[zbus(signal)]
        async fn enroll_status(e: &SignalEmitter<'_>, result: &str, done: bool)
        -> zbus::Result<()>;
        #[zbus(property, name = "num-enroll-stages")]
        fn num_enroll_stages(&self) -> i32 {
            3
        }
    }

    /// Serve the fake on `conn` and own fprintd's name.
    pub async fn serve(conn: &zbus::Connection, device: Device) {
        conn.object_server()
            .at("/net/reactivated/Fprint/Manager", Manager)
            .await
            .unwrap();
        conn.object_server().at(DEVICE, device).await.unwrap();
        conn.request_name(super::FPRINT).await.unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::fake::{Calls, Device, serve};
    use super::*;
    use crate::pages::fake_system::SysRig;

    async fn rig(enrolled: &[&str], script: Vec<(&'static str, bool)>) -> (SysRig, Calls) {
        let mut sys = SysRig::new();
        let calls = Calls::default();
        let conn = sys.own("org.example.Unused").await;
        serve(
            &conn,
            Device {
                enrolled: Arc::new(Mutex::new(enrolled.iter().map(|s| s.to_string()).collect())),
                script,
                calls: calls.clone(),
            },
        )
        .await;
        (sys, calls)
    }

    #[tokio::test]
    async fn no_fprintd_is_no_reader() {
        let sys = SysRig::new();
        let r = reader(&sys.system()).await;
        assert_eq!(r, Reader::default());
        assert!(!r.offer());
    }

    #[tokio::test]
    async fn a_reader_without_fingers_is_offered() {
        let (sys, _) = rig(&[], vec![]).await;
        let r = reader(&sys.system()).await;
        assert!(r.present && !r.enrolled && r.offer());
        let (sys, _) = rig(&["left-thumb"], vec![]).await;
        let r = reader(&sys.system()).await;
        assert!(r.present && r.enrolled && !r.offer());
    }

    #[tokio::test]
    async fn enrol_reports_each_stage_then_releases() {
        let (sys, calls) = rig(
            &[],
            vec![
                ("enroll-stage-passed", false),
                ("enroll-retry-scan", false),
                ("enroll-stage-passed", false),
                ("enroll-completed", true),
            ],
        )
        .await;
        let e = Enrolment::default();
        e.run(&sys.system()).await;
        let got = e.get();
        assert_eq!(got.phase, Phase::Done);
        assert_eq!((got.stage, got.stages), (3, 3));
        assert_eq!(got.message, "Your fingerprint is saved.");
        assert_eq!(
            calls.all(),
            ["claim ", "enroll right-index-finger", "stop", "release"]
        );
        assert!(reader(&sys.system()).await.enrolled);
    }

    #[tokio::test]
    async fn a_failed_enrol_says_why() {
        let (sys, calls) = rig(&[], vec![("enroll-duplicate", true)]).await;
        let e = Enrolment::default();
        e.run(&sys.system()).await;
        assert_eq!(e.get().phase, Phase::Failed);
        assert_eq!(e.get().message, "That finger is already saved.");
        assert_eq!(calls.all().last().map(String::as_str), Some("release"));
    }

    #[tokio::test]
    async fn cancel_stops_and_releases() {
        let (sys, calls) = rig(&[], vec![]).await;
        let e = Enrolment::default();
        let run = {
            let e = e.clone();
            let sys = sys.system();
            tokio::spawn(async move { e.run(&sys).await })
        };
        for _ in 0..100 {
            if calls.all().iter().any(|c| c.starts_with("enroll")) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(e.busy());
        e.cancel();
        tokio::time::timeout(std::time::Duration::from_secs(5), run)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(e.get(), Enroll::default());
        assert_eq!(calls.all()[2..], ["stop", "release"]);
    }

    #[test]
    fn every_result_has_a_plain_message() {
        for r in [
            "enroll-completed",
            "enroll-stage-passed",
            "enroll-retry-scan",
            "enroll-swipe-too-short",
            "enroll-finger-not-centered",
            "enroll-remove-and-retry",
            "enroll-data-full",
            "enroll-duplicate",
            "enroll-disconnected",
            "enroll-failed",
            "something-new",
        ] {
            let (_, m, _) = status(r);
            assert!(m.ends_with('.') && !m.contains("enroll"), "{r}: {m}");
        }
    }
}
