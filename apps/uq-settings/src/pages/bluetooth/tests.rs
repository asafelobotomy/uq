use std::time::Duration;

use super::*;
use crate::pages::fake_system::SysRig;
use crate::pages::fake_system::bluez::{
    BluezState, FakeBluez, HEADPHONES, KEYBOARD, NAMELESS, SPEAKER,
};
use crate::pages::runner::run;

async fn setup() -> (SysRig, FakeBluez, Arc<BluetoothState>) {
    let mut rig = SysRig::new();
    let bluez = FakeBluez::start(&mut rig).await;
    let page = Arc::new(BluetoothState::with(rig.system()));
    (rig, bluez, page)
}

/// Open the page: its watch registers the agent.
async fn open(page: &Arc<BluetoothState>, bluez: &FakeBluez) -> JoinHandle<()> {
    let watcher = {
        let page = page.clone();
        tokio::spawn(async move { page.watch(Arc::new(Notify::new())).await })
    };
    until(bluez, |s| s.default_agent.is_some()).await;
    watcher
}

async fn until(bluez: &FakeBluez, f: impl Fn(&BluezState) -> bool) {
    for _ in 0..200 {
        if f(&bluez.state.lock().unwrap()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out: {:?}", bluez.state.lock().unwrap());
}

async fn page_until(page: &BluetoothState, f: impl Fn(&Value) -> bool) -> Value {
    for _ in 0..200 {
        let s = page.snapshot().await.unwrap();
        if f(&s) {
            return s;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("timed out: {}", page.snapshot().await.unwrap());
}

fn device(state: &Value, path: &str) -> Value {
    state["devices"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["path"] == path)
        .cloned()
        .unwrap_or(Value::Null)
}

#[tokio::test]
async fn reads_the_adapter_and_devices() {
    let (_rig, _bluez, page) = setup().await;
    let out = run(&*page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let s = out.state;
    assert_eq!(s["available"], true);
    assert_eq!(s["adapter"]["powered"], true);
    assert_eq!(s["adapter"]["name"], "ana-laptop");
    assert_eq!(s["devices"][0]["path"], HEADPHONES, "connected first");
    let h = device(&s, HEADPHONES);
    assert_eq!(h["name"], "Ana's headphones");
    assert_eq!(h["kind"], "Headphones");
    assert_eq!(h["battery"], 80);
    assert_eq!(h["paired"], true);
    assert_eq!(h["connected"], true);
    let k = device(&s, KEYBOARD);
    assert_eq!(
        k["name"], "<b>Keyboard</b>",
        "names are passed on as they are"
    );
    assert_eq!(k["battery"], Value::Null);
    assert!(
        device(&s, NAMELESS).is_null(),
        "a device with no name is left out"
    );
    assert!(s["question"].is_null());
}

#[tokio::test]
async fn pairing_waits_for_an_explicit_confirm() {
    let (_rig, bluez, page) = setup().await;
    let _watcher = open(&page, &bluez).await;
    {
        let st = bluez.state.lock().unwrap();
        let (_, path, cap) = st.agent.clone().unwrap();
        assert_eq!(cap, "KeyboardDisplay");
        assert_eq!(path, AGENT_PATH);
    }
    let out = run(&*page, Op::Pair(KEYBOARD.into())).await;
    assert_eq!(out.error, None, "pairing runs in the background");
    let s = page_until(&page, |s| !s["question"].is_null()).await;
    assert_eq!(device(&s, KEYBOARD)["working"], "pairing");
    let q = &s["question"];
    assert_eq!(q["kind"], "confirm");
    assert_eq!(q["code"], "123456");
    assert_eq!(q["device"], "<b>Keyboard</b>");

    // Nothing happens on its own.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!bluez.state.lock().unwrap().devices[KEYBOARD].paired);
    assert!(bluez.state.lock().unwrap().pair_results.is_empty());

    let id = q["id"].as_u64().unwrap();
    run(
        &*page,
        Op::Answer {
            id,
            yes: true,
            value: String::new(),
        },
    )
    .await;
    until(&bluez, |s| s.devices[KEYBOARD].paired).await;
    let s = page_until(&page, |s| device(s, KEYBOARD)["working"].is_null()).await;
    assert_eq!(device(&s, KEYBOARD)["paired"], true);
    assert!(s["question"].is_null());
    assert_eq!(s["notice"]["text"], "");
}

#[tokio::test]
async fn saying_no_rejects_and_says_why() {
    let (_rig, bluez, page) = setup().await;
    let _watcher = open(&page, &bluez).await;
    run(&*page, Op::Pair(KEYBOARD.into())).await;
    let s = page_until(&page, |s| !s["question"].is_null()).await;
    run(
        &*page,
        Op::Answer {
            id: s["question"]["id"].as_u64().unwrap(),
            yes: false,
            value: String::new(),
        },
    )
    .await;
    until(&bluez, |s| !s.pair_results.is_empty()).await;
    assert_eq!(
        bluez.state.lock().unwrap().pair_results[0],
        "org.bluez.Error.Rejected"
    );
    let s = page_until(&page, |s| s["notice"]["text"] != "").await;
    assert_eq!(
        s["notice"]["text"],
        "Pairing didn't work: the codes didn't match or the device said no."
    );
    assert_eq!(device(&s, KEYBOARD)["paired"], false);
}

#[tokio::test]
async fn a_pin_is_typed_in() {
    let (_rig, bluez, page) = setup().await;
    let _watcher = open(&page, &bluez).await;
    let s = run(&*page, Op::Scan(true)).await.state;
    assert_eq!(s["adapter"]["discovering"], true);
    assert_eq!(device(&s, SPEAKER)["kind"], "Speaker");
    run(&*page, Op::Pair(SPEAKER.into())).await;
    let s = page_until(&page, |s| !s["question"].is_null()).await;
    assert_eq!(s["question"]["kind"], "pin");
    run(
        &*page,
        Op::Answer {
            id: s["question"]["id"].as_u64().unwrap(),
            yes: true,
            value: "0000".into(),
        },
    )
    .await;
    until(&bluez, |s| s.devices[SPEAKER].paired).await;
    assert_eq!(bluez.state.lock().unwrap().devices[SPEAKER].pin, "0000");
}

#[tokio::test]
async fn without_the_page_open_nothing_pairs() {
    let (_rig, bluez, page) = setup().await;
    // No watch: no agent registered.
    run(&*page, Op::Pair(KEYBOARD.into())).await;
    let s = page_until(&page, |s| s["notice"]["text"] != "").await;
    assert!(
        s["notice"]["text"]
            .as_str()
            .unwrap()
            .starts_with("Pairing didn't work")
    );
    assert!(!bluez.state.lock().unwrap().devices[KEYBOARD].paired);
}

#[tokio::test]
async fn power_trust_connect_and_forget() {
    let (_rig, bluez, page) = setup().await;
    let s = run(&*page, Op::SetDiscoverable(true)).await.state;
    assert_eq!(s["adapter"]["discoverable"], true);
    let s = run(&*page, Op::SetTrusted(HEADPHONES.into(), false))
        .await
        .state;
    assert_eq!(device(&s, HEADPHONES)["trusted"], false);
    let s = run(&*page, Op::Disconnect(HEADPHONES.into())).await.state;
    assert_eq!(device(&s, HEADPHONES)["connected"], false);
    run(&*page, Op::Connect(HEADPHONES.into())).await;
    page_until(&page, |s| device(s, HEADPHONES)["connected"] == true).await;
    let s = run(&*page, Op::Forget(HEADPHONES.into())).await.state;
    assert!(device(&s, HEADPHONES).is_null());
    assert!(!bluez.state.lock().unwrap().devices.contains_key(HEADPHONES));
    let out = run(&*page, Op::Forget(HEADPHONES.into())).await;
    assert_eq!(out.error.as_deref(), Some("That device is gone."));

    let s = run(&*page, Op::SetPowered(false)).await.state;
    assert_eq!(s["adapter"]["powered"], false);
    let out = run(&*page, Op::Scan(true)).await;
    assert_eq!(out.error.as_deref(), Some("Bluetooth is off."));
    bluez.state.lock().unwrap().blocked = true;
    let out = run(&*page, Op::SetPowered(true)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Bluetooth is blocked by a switch or key on the computer.")
    );
    assert_eq!(out.state["adapter"]["powered"], false);
}

#[tokio::test]
async fn closing_the_page_drops_the_agent_and_the_search() {
    let (rig, bluez, page) = setup().await;
    let watcher = open(&page, &bluez).await;
    run(&*page, Op::Scan(true)).await;
    let (agent, owner) = {
        let st = bluez.state.lock().unwrap();
        (
            st.agent.clone().unwrap().0,
            st.discovery_owner.clone().unwrap(),
        )
    };
    assert_eq!(agent, owner);
    // A question on show when the page closes is answered "cancelled", never "yes".
    run(&*page, Op::Pair(KEYBOARD.into())).await;
    page_until(&page, |s| !s["question"].is_null()).await;
    watcher.abort();
    let _ = watcher.await;
    drop(page);
    until(&bluez, |s| !s.pair_results.is_empty()).await;
    assert_eq!(
        bluez.state.lock().unwrap().pair_results[0],
        "org.bluez.Error.Canceled"
    );
    assert!(!bluez.state.lock().unwrap().devices[KEYBOARD].paired);
    let other = uq_core::test_bus::connect(&rig.bus).await;
    let dbus = zbus::fdo::DBusProxy::new(&other).await.unwrap();
    for _ in 0..100 {
        let name = zbus::names::BusName::try_from(agent.as_str()).unwrap();
        if !dbus.name_has_owner(name).await.unwrap() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("the agent stayed on the bus after the page closed");
}

#[tokio::test]
async fn without_bluez_the_page_names_the_package() {
    let rig = SysRig::new();
    let page = BluetoothState::with(rig.system());
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["available"], false);
    assert_eq!(out.state["package"], "bluez");
    let out = run(&page, Op::SetPowered(true)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Bluetooth isn't available. Install the bluez package.")
    );
}

#[test]
fn words() {
    assert_eq!(kind_text("input-mouse"), "Mouse");
    assert_eq!(kind_text(""), "Device");
}

/// The adapter comes from the same `GetManagedObjects` answer as the rest of the page (no second
/// read that could find it gone): the first by path, with its properties.
#[test]
fn first_adapter_is_picked_from_one_snapshot() {
    use std::collections::HashMap;
    use zbus::names::OwnedInterfaceName;
    use zbus::zvariant::{OwnedObjectPath, OwnedValue};
    let entry = |iface: &str, name: &str| {
        let mut props = HashMap::new();
        props.insert(
            "Alias".to_string(),
            OwnedValue::try_from(zbus::zvariant::Value::from(name)).unwrap(),
        );
        let mut o = HashMap::new();
        o.insert(OwnedInterfaceName::try_from(iface).unwrap(), props);
        o
    };
    let mut objects: Objects = HashMap::new();
    assert!(first_adapter(&objects).is_none());
    let path = |p: &str| OwnedObjectPath::try_from(p).unwrap();
    objects.insert(path("/org/bluez/hci0/dev_AA"), entry(DEVICE, "phone"));
    assert!(
        first_adapter(&objects).is_none(),
        "a device isn't an adapter"
    );
    objects.insert(path("/org/bluez/hci1"), entry(ADAPTER, "second"));
    objects.insert(path("/org/bluez/hci0"), entry(ADAPTER, "first"));
    let (p, props) = first_adapter(&objects).unwrap();
    assert_eq!(p.as_str(), "/org/bluez/hci0");
    assert_eq!(get::<String>(props, "Alias").as_deref(), Some("first"));
}
