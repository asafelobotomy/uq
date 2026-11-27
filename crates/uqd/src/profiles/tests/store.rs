//! Storing profiles and their events.

use super::*;

#[tokio::test]
async fn explain_reports_profile_source() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "profile", "{v}");
    assert_eq!(v["gaps_in"]["detail"]["name"], "Gaming");
    set_look(&mut r, "gaps_in", json!(3)).await;
    let v = r.desktop.explain("look").await.unwrap();
    assert_eq!(v["gaps_in"]["source"], "uq", "{v}");
    assert_eq!(v["blur"]["source"], "profile", "{v}");
}

#[tokio::test]
async fn set_profile_conflict_then_replace() {
    let mut r = rig().await;
    assert_eq!(r.desktop.set_profile(GAMING, false).unwrap(), "gaming");
    let renamed = GAMING.replace("name = \"Gaming\"", "name = \"Games\"");
    assert_eq!(
        r.desktop.set_profile(&renamed, false).unwrap_err(),
        DesktopError::Conflict("A profile called “Gaming” already exists. Replace it?".into())
    );
    assert_eq!(r.desktop.set_profile(&renamed, true).unwrap(), "gaming");
    assert!(
        r.desktop
            .get_profile("gaming")
            .unwrap()
            .contains("name = \"Games\"")
    );
    let list = r.desktop.list_profiles();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "Games");
    assert_eq!(list[0].keys, 2);
    assert!(!list[0].active);
}

#[tokio::test]
async fn set_profile_with_unknown_theme_is_refused() {
    let mut r = rig().await;
    let text = "format = 1\nid = \"gone\"\nname = \"Gone\"\n[appearance]\ntheme = \"nosuch\"\n";
    assert_eq!(
        r.desktop.set_profile(text, false).unwrap_err(),
        DesktopError::UnknownTheme("nosuch".into())
    );
    assert!(r.desktop.list_profiles().is_empty());
    let bad = "format = 1\nid = \"x\"\nname = \"X\"\n[look]\ngaps_in = 9999\n";
    assert!(matches!(
        r.desktop.set_profile(bad, false),
        Err(DesktopError::InvalidSetting(_))
    ));
    assert_eq!(
        r.desktop.get_profile("nope").unwrap_err(),
        DesktopError::InvalidArgument("no profile \"nope\"".into())
    );
}

#[tokio::test]
async fn delete_active_profile_is_refused() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(
        r.desktop.delete_profile("gaming").unwrap_err(),
        DesktopError::InvalidArgument("Turn this profile off before deleting it.".into())
    );
    r.desktop.profile_off().await.unwrap();
    r.desktop.delete_profile("gaming").unwrap();
    assert!(r.desktop.list_profiles().is_empty());
}

#[tokio::test]
async fn fifty_first_profile_is_refused() {
    let mut r = rig().await;
    let text =
        |i: usize| format!("format = 1\nid = \"p{i}\"\nname = \"P{i}\"\n[look]\ngaps_in = 1\n");
    for i in 0..50 {
        r.desktop.set_profile(&text(i), false).unwrap();
    }
    assert_eq!(
        r.desktop.set_profile(&text(50), false).unwrap_err(),
        DesktopError::InvalidArgument("You can keep up to 50 profiles.".into())
    );
    // Replacing one of the fifty is still fine.
    r.desktop.set_profile(&text(3), true).unwrap();
    assert_eq!(r.desktop.list_profiles().len(), 50);
}

#[tokio::test]
async fn profile_changed_event_sent() {
    let mut r = rig().await;
    add(&mut r, GAMING).await;
    assert_eq!(profile_events(&mut r.events), [""]);
    r.desktop.apply_profile("gaming").await.unwrap();
    assert_eq!(profile_events(&mut r.events), ["gaming"]);
    add(&mut r, WORK).await;
    assert_eq!(profile_events(&mut r.events), ["gaming"]);
    r.desktop.profile_off().await.unwrap();
    assert_eq!(profile_events(&mut r.events), [""]);
    r.desktop.delete_profile("work").unwrap();
    assert_eq!(profile_events(&mut r.events), [""]);
    let list = r.desktop.list_profiles();
    assert_eq!(list.len(), 1);
}
