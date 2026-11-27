use std::time::Duration;

use super::read::{band, state_text};
use super::*;
use crate::pages::fake_system::nm::{FakeNm, NmState, WIFI_DEV};
use crate::pages::fake_system::{SysRig, pw};
use crate::pages::runner::run;

async fn setup() -> (SysRig, FakeNm, NetworkState, tempfile::TempDir) {
    let mut rig = SysRig::new();
    let nm = FakeNm::start(&mut rig).await;
    let root = tempfile::tempdir().unwrap();
    let page = NetworkState::with(rig.system(), root.path().to_path_buf(), "ana".into());
    (rig, nm, page, root)
}

fn home_settings(uuid: &str, password: &str) -> Settings {
    let mut s = profile::wifi_settings(
        uuid,
        "ana",
        &WifiInput {
            ssid: b"Home".to_vec(),
            security: Some(Security::Password { sae_only: false }),
            password: if password.is_empty() {
                pw()
            } else {
                password.to_string()
            },
            ..Default::default()
        },
    )
    .unwrap();
    if password.is_empty() {
        s.get_mut("802-11-wireless-security").unwrap().remove("psk");
    }
    s
}

fn ap_of(state: &Value, ssid: &str) -> Value {
    state["wifi"]["networks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["ssid"] == ssid)
        .cloned()
        .unwrap_or_else(|| panic!("{ssid} not listed: {state}"))
}

fn saved_of(state: &Value, uuid: &str) -> Value {
    state["saved"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["uuid"] == uuid)
        .cloned()
        .unwrap_or(Value::Null)
}

/// Wait until `f` holds for the fake's state (an agent call runs in the background).
async fn until(nm: &FakeNm, f: impl Fn(&NmState) -> bool) {
    for _ in 0..200 {
        if f(&nm.state.lock().unwrap()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out");
}

/// Wait until the page's state satisfies `f`, and return it.
async fn page_until(page: &NetworkState, f: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..200 {
        let s = page.snapshot().await.unwrap();
        if f(&s) {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out: {}", page.snapshot().await.unwrap());
}

mod connections;
mod devices;
mod saved;
