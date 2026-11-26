//! Listing, adding, removing and auto-login.

use super::*;

#[tokio::test]
async fn lists_people_not_system_accounts_with_me_first() {
    let Setup(_rig, _fake, page, _root) = setup(two(), 1001).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    let users = out.state["users"].as_array().unwrap();
    let names: Vec<_> = users.iter().map(|u| u["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["bob", "alice"]);
    assert_eq!(users[0]["me"], true);
    assert_eq!(
        users[0]["remove_problem"],
        "You can't remove your own account."
    );
    assert_eq!(
        users[1]["remove_problem"],
        "This is the only administrator, so it can't be removed."
    );
    assert_eq!(
        users[1]["demote_problem"],
        "This is the only administrator, so it must stay one."
    );
    assert!(out.state["service"].is_null());
}

#[tokio::test]
async fn real_name_type_and_avatar_round_trip() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let out = run(
        &page,
        Op::SetRealName {
            uid: 1001,
            name: " Robert ".into(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(fake.user("bob").unwrap().real_name, "Robert");
    let out = run(
        &page,
        Op::SetAdmin {
            uid: 1001,
            admin: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["users"][1]["admin"], true);
    // With two administrators, either can become a standard user.
    let out = run(
        &page,
        Op::SetAdmin {
            uid: 1000,
            admin: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["users"][0]["admin"], false);

    let dir = tempfile::tempdir().unwrap();
    let pic = dir.path().join("me.png");
    std::fs::write(&pic, b"png").unwrap();
    let out = run(
        &page,
        Op::SetAvatar {
            uid: 1000,
            path: pic.clone(),
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["users"][0]["icon"], pic.display().to_string());

    // A fresh page reads the same.
    let again = UsersState::with(_rig.system(), 1000, page.root.clone());
    let out = run(&again, Op::Refresh).await;
    assert_eq!(out.state["users"][1]["real_name"], "Robert");
    assert_eq!(out.state["users"][1]["admin"], true);
}

#[tokio::test]
async fn the_last_administrator_stays() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1001).await;
    let out = run(
        &page,
        Op::SetAdmin {
            uid: 1000,
            admin: false,
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("This is the only administrator, so it must stay one.")
    );
    assert_eq!(fake.user("alice").unwrap().account_type, 1);
    let out = run(
        &page,
        Op::RemoveUser {
            uid: 1000,
            typed: "alice".into(),
            remove_files: false,
        },
    )
    .await;
    assert!(out.error.unwrap().contains("only administrator"));
    assert!(fake.user("alice").is_some());
}

#[tokio::test]
async fn removing_needs_the_name_and_never_yourself() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let remove = |uid, typed: &str, files| Op::RemoveUser {
        uid,
        typed: typed.into(),
        remove_files: files,
    };
    let out = run(&page, remove(1000, "alice", false)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("You can't remove your own account.")
    );
    let out = run(&page, remove(1001, "Bob", true)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Type the user name exactly to remove the account.")
    );
    assert!(fake.user("bob").is_some());
    let out = run(&page, remove(1001, "bob", true)).await;
    assert_eq!(out.error, None);
    assert!(fake.user("bob").is_none());
    assert_eq!(*fake.deleted.lock().unwrap(), [(1001, true)]);
    assert_eq!(
        out.state["notice"]["text"],
        "bob and their files were removed."
    );
}

#[tokio::test]
async fn removing_the_auto_login_user_turns_auto_login_off() {
    let mut rig = SysRig::new();
    let fake = FakeAccounts::start(&mut rig, two()).await;
    let helper = FakeHelper::start(&mut rig, &[("bob", 1001)]).await;
    let greetd = "etc/uq/greetd.toml";
    std::fs::write(
        helper.path(greetd),
        format!(
            "{}\n[initial_session]\ncommand = \"uq-session\"\nuser = \"bob\"\n",
            helper.read(greetd).unwrap()
        ),
    )
    .unwrap();
    let page = UsersState::with(rig.system(), 1000, helper.root.path().to_path_buf());
    let out = run(
        &page,
        Op::RemoveUser {
            uid: 1001,
            typed: "bob".into(),
            remove_files: false,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert!(fake.user("bob").is_none());
    assert!(!helper.read(greetd).unwrap().contains("initial_session"));
    assert_eq!(
        out.state["notice"]["text"],
        "bob was removed. Their files were kept. Auto-login is off."
    );
}

#[tokio::test]
async fn a_refused_auto_login_change_is_said_after_a_removal() {
    let mut rig = SysRig::new();
    let _fake = FakeAccounts::start(&mut rig, two()).await;
    let helper = FakeHelper::start(&mut rig, &[("bob", 1001)]).await;
    let greetd = "etc/uq/greetd.toml";
    let with_bob = format!(
        "{}\n[initial_session]\ncommand = \"uq-session\"\nuser = \"bob\"\n",
        helper.read(greetd).unwrap()
    );
    std::fs::write(helper.path(greetd), &with_bob).unwrap();
    helper.deny.set(true);
    let page = UsersState::with(rig.system(), 1000, helper.root.path().to_path_buf());
    let out = run(
        &page,
        Op::RemoveUser {
            uid: 1001,
            typed: "bob".into(),
            remove_files: true,
        },
    )
    .await;
    assert_eq!(out.error, None);
    assert!(
        out.state["notice"]["text"]
            .as_str()
            .unwrap()
            .ends_with("Auto-login still names them: turn it off on the Login screen page.")
    );
    assert_eq!(helper.read(greetd).unwrap(), with_bob);
}

#[tokio::test]
async fn missing_service_names_the_package() {
    let rig = SysRig::new();
    let page = UsersState::with(rig.system(), 1000, PathBuf::from("/nonexistent"));
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["service"]["package"], "accountsservice");
    assert_eq!(out.state["users"], serde_json::json!([]));
    let out = run(
        &page,
        Op::SetAdmin {
            uid: 1,
            admin: true,
        },
    )
    .await;
    assert_eq!(
        out.error.as_deref(),
        Some("The accounts service isn't available. Install the accountsservice package.")
    );
}

#[tokio::test]
async fn auto_login_status_comes_from_the_greeter_config() {
    let Setup(_rig, _fake, page, _root) = setup(two(), 1000).await;
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["auto_login"]["greeter"], false);
    std::fs::create_dir_all(page.root.join("etc/uq")).unwrap();
    std::fs::write(
        page.root.join(GREETD_TOML),
        "[initial_session]\ncommand = \"/usr/bin/uq-session\"\nuser = \"alice\"\n",
    )
    .unwrap();
    let out = run(&page, Op::Refresh).await;
    assert_eq!(out.state["auto_login"]["greeter"], true);
    assert_eq!(out.state["auto_login"]["user"], "alice");
}

#[test]
fn name_rules() {
    assert!(valid_user_name("alice") && valid_user_name("_svc") && valid_user_name("a-b_1"));
    assert!(!valid_user_name("Alice") && !valid_user_name("-a") && !valid_user_name(""));
    assert!(!valid_user_name(&"a".repeat(33)));
    assert!(real_name_problem("Ann Lee").is_none());
    assert!(real_name_problem("a:b").is_some() && real_name_problem("a\nb").is_some());
}
