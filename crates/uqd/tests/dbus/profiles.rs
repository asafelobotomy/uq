//! Profiles over D-Bus.

use super::*;

pub(crate) fn report(json: &str) -> serde_json::Value {
    serde_json::from_str(json).unwrap()
}

#[tokio::test]
pub(crate) async fn profile_crud_over_dbus() {
    let s = served().await;
    let id: String = s.proxy.call("SetProfile", &(GAMING, false)).await.unwrap();
    assert_eq!(id, "gaming");
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    let list = report(&list);
    assert_eq!(list[0]["id"], "gaming");
    assert_eq!(list[0]["keys"], 2);
    assert_eq!(list[0]["active"], false);

    let text: String = s.proxy.call("GetProfile", &("gaming",)).await.unwrap();
    let again = uq_core::profile::Profile::parse(&text).unwrap();
    assert_eq!(again, uq_core::profile::Profile::parse(GAMING).unwrap());

    let err = s
        .proxy
        .call::<_, _, String>("SetProfile", &(GAMING, false))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.Conflict");
    let renamed = GAMING.replace("Gaming", "Games");
    let id: String = s
        .proxy
        .call("SetProfile", &(renamed.as_str(), true))
        .await
        .unwrap();
    assert_eq!(id, "gaming");
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    assert_eq!(report(&list)[0]["name"], "Games");

    let err = s
        .proxy
        .call::<_, _, String>("GetProfile", &("nope",))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidArgument");
    let err = s
        .proxy
        .call::<_, _, String>("SetProfile", &("format = 1\nid = \"../x\"\n", false))
        .await
        .unwrap_err();
    assert_eq!(error_name(&err), "org.uq.Desktop1.Error.InvalidSetting");

    let () = s.proxy.call("DeleteProfile", &("gaming",)).await.unwrap();
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    assert_eq!(report(&list), serde_json::json!([]));
}

#[tokio::test]
pub(crate) async fn apply_and_off_over_dbus_emit_profile_changed() {
    let s = served().await;
    let _: String = s.proxy.call("SetProfile", &(GAMING, false)).await.unwrap();
    let mut changed = s.proxy.receive_signal("ProfileChanged").await.unwrap();
    let next_id = |msg: zbus::Message| msg.body().deserialize::<String>().unwrap();
    let wait = std::time::Duration::from_secs(5);

    let json: String = s.proxy.call("ApplyProfile", &("gaming",)).await.unwrap();
    assert_eq!(report(&json)["active"], "gaming");
    assert_eq!(s.gaps().await, 0);
    let msg = tokio::time::timeout(wait, changed.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_id(msg), "gaming");
    let list: String = s.proxy.call("ListProfiles", &()).await.unwrap();
    assert_eq!(report(&list)[0]["active"], true);

    let json: String = s.proxy.call("ProfileOff", &()).await.unwrap();
    assert_eq!(report(&json)["active"], "");
    let msg = tokio::time::timeout(wait, changed.next())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(next_id(msg), "");
    assert_ne!(s.gaps().await, 0);
}
