use super::*;
use crate::pages::fake_system::{FakeAccounts, FakeUser, SysRig};
use crate::pages::runner::run;

struct Setup(SysRig, FakeAccounts, UsersState, tempfile::TempDir);

async fn setup(users: Vec<FakeUser>, me: u64) -> Setup {
    let mut rig = SysRig::new();
    let fake = FakeAccounts::start(&mut rig, users).await;
    let root = tempfile::tempdir().unwrap();
    let page = UsersState::with(rig.system(), me, root.path().to_path_buf());
    Setup(rig, fake, page, root)
}

fn two() -> Vec<FakeUser> {
    let mut sys = FakeUser::new(970, "colord", "", false);
    sys.system = true;
    vec![
        FakeUser::new(1000, "alice", "Alice A", true),
        FakeUser::new(1001, "bob", "Bob B", false),
        sys,
    ]
}

fn password(uid: u64, p: &str, c: &str) -> Op {
    Op::SetPassword {
        uid,
        password: p.into(),
        confirm: c.into(),
    }
}

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
async fn passwords_are_hashed_and_checked() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let out = run(&page, password(1001, "s3cret!", "s3cret?")).await;
    assert_eq!(out.error.as_deref(), Some("The two passwords don't match."));
    let out = run(&page, password(1001, "", "")).await;
    assert_eq!(out.error.as_deref(), Some("Type a password."));
    assert_eq!(fake.user("bob").unwrap().password, "");

    let out = run(&page, password(1001, "s3cret!", "s3cret!")).await;
    assert_eq!(out.error, None);
    let stored = fake.user("bob").unwrap().password;
    assert!(stored.starts_with('$'), "a crypt hash: {stored}");
    assert!(!stored.contains("s3cret"));
    assert_eq!(out.state["notice"]["text"], "The password was changed.");
}

#[tokio::test]
async fn add_user_checks_then_creates_with_a_password() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let add = |name: &str, p: &str, c: &str| Op::AddUser {
        name: name.into(),
        real_name: "Carol C".into(),
        admin: false,
        password: p.into(),
        confirm: c.into(),
    };
    for bad in ["Carol", "1carol", "car ol", ""] {
        let out = run(&page, add(bad, "pw", "pw")).await;
        assert!(
            out.error.unwrap().starts_with("A user name starts"),
            "{bad}"
        );
    }
    let out = run(&page, add("bob", "pw", "pw")).await;
    assert_eq!(
        out.error.as_deref(),
        Some("There's already an account with that user name.")
    );
    let out = run(&page, add("carol", "pw", "px")).await;
    assert_eq!(out.error.as_deref(), Some("The two passwords don't match."));
    assert!(fake.user("carol").is_none());

    let out = run(&page, add("carol", "pw", "pw")).await;
    assert_eq!(out.error, None);
    let carol = fake.user("carol").unwrap();
    assert_eq!(carol.real_name, "Carol C");
    assert_eq!(carol.account_type, 0);
    assert!(carol.password.starts_with('$'));
    assert_eq!(out.state["users"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn a_cancelled_prompt_changes_nothing_quietly() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    fake.deny.set(true);
    let out = run(
        &page,
        Op::SetAdmin {
            uid: 1001,
            admin: true,
        },
    )
    .await;
    assert_eq!(out.error, None, "quiet");
    assert_eq!(out.state["users"][1]["admin"], false, "the old value");
    let out = run(&page, password(1001, "pw", "pw")).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "", "no 'changed' notice");
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

#[test]
fn passwords_never_show_in_debug() {
    let op = password(1, "hunter2", "hunter2");
    assert!(!format!("{op:?}").contains("hunter2"));
}
