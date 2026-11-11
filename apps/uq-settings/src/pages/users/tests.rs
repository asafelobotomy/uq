use super::*;
use crate::pages::fake_system::{FakeAccounts, FakeHelper, FakeUser, SysRig};
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
        current: String::new(),
        password: p.into(),
        confirm: c.into(),
    }
}

fn own_password(uid: u64, current: &str, p: &str, c: &str) -> Op {
    Op::SetPassword {
        uid,
        current: current.into(),
        password: p.into(),
        confirm: c.into(),
    }
}

/// A stand-in for `/usr/bin/passwd`, run by `/bin/sh` (never exec'd itself, so no ETXTBSY):
/// it asks like shadow's passwd does (prompts on stderr, one line read per prompt), accepts
/// the current password in `$1/current`, refuses "short" as pam_pwquality would, and records
/// the new password, its own arguments and its environment in `$1`. Mode `$2`: `hang` never
/// asks anything.
const STUB: &str = r#"
dir=$1
printf '%s\n' "$*" > "$dir/argv"
env > "$dir/env"
[ "$2" = hang ] && exec sleep 30
echo "Changing password for alice."
printf 'Current password: ' >&2
read -r cur
if [ "$cur" != "$(cat "$dir/current")" ]; then
    echo 'passwd: Authentication token manipulation error' >&2
    echo 'passwd: password unchanged' >&2
    exit 10
fi
tries=0
while :; do
    printf 'New password: ' >&2
    read -r new
    if [ "$new" = short ]; then
        echo 'BAD PASSWORD: The password is shorter than 8 characters' >&2
        tries=$((tries + 1))
        [ "$tries" -ge 3 ] && exit 10
        continue
    fi
    printf 'Retype new password: ' >&2
    read -r again
    [ "$new" = "$again" ] || { echo 'Sorry, passwords do not match.' >&2; exit 10; }
    break
done
printf '%s' "$new" > "$dir/changed"
echo 'passwd: password updated successfully'
"#;

fn stub_passwd(page: UsersState, mode: &str) -> (UsersState, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("stub.sh"), STUB).unwrap();
    std::fs::write(dir.path().join("current"), "old pw").unwrap();
    let cmd = vec![
        OsString::from("/bin/sh"),
        dir.path().join("stub.sh").into_os_string(),
        dir.path().as_os_str().to_owned(),
        OsString::from(mode),
    ];
    (page.with_passwd(cmd, Duration::from_secs(2)), dir)
}

#[tokio::test]
async fn your_own_password_goes_through_passwd_with_the_current_one() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let (page, dir) = stub_passwd(page, "ok");
    let out = run(&page, own_password(1000, "", "new pw 1", "new pw 1")).await;
    assert_eq!(out.error.as_deref(), Some("Type your current password."));
    assert!(!dir.path().join("argv").exists(), "passwd never ran");

    let out = run(&page, own_password(1000, "old pw", "new pw 1", "new pw 1")).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Your password was changed.");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("changed")).unwrap(),
        "new pw 1"
    );
    // AccountsService (and so an administrator prompt) was never involved.
    assert_eq!(fake.user("alice").unwrap().password, "");
    // Never in argv or the environment; prompts were read in the C locale.
    let argv = std::fs::read_to_string(dir.path().join("argv")).unwrap();
    let env = std::fs::read_to_string(dir.path().join("env")).unwrap();
    for secret in ["old pw", "new pw 1"] {
        assert!(!argv.contains(secret) && !env.contains(secret), "{secret}");
    }
    assert!(env.lines().any(|l| l == "LC_ALL=C"), "{env}");
}

#[tokio::test]
async fn your_own_password_failures_read_plainly() {
    let Setup(_rig, _fake, page, _root) = setup(two(), 1000).await;
    let (page, dir) = stub_passwd(page, "ok");
    let out = run(&page, own_password(1000, "wrong", "new pw 1", "new pw 1")).await;
    assert_eq!(out.error.as_deref(), Some("The current password is wrong."));
    let out = run(&page, own_password(1000, "old pw", "short", "short")).await;
    assert_eq!(
        out.error.as_deref(),
        Some("That password can't be used: The password is shorter than 8 characters")
    );
    let out = run(&page, own_password(1000, "old pw", "a", "b")).await;
    assert_eq!(out.error.as_deref(), Some("The two passwords don't match."));
    assert!(!dir.path().join("changed").exists());

    let Setup(_rig, _fake, page, _root) = setup(two(), 1000).await;
    let (page, _dir) = stub_passwd(page, "hang");
    let started = std::time::Instant::now();
    let out = run(&page, own_password(1000, "old pw", "new pw 1", "new pw 1")).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Changing the password took too long, so it was stopped.")
    );
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[tokio::test]
async fn someone_elses_password_still_uses_accountsservice() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let (page, dir) = stub_passwd(page, "ok");
    let out = run(&page, own_password(1001, "ignored", "s3cret!", "s3cret!")).await;
    assert_eq!(out.error, None);
    assert!(fake.user("bob").unwrap().password.starts_with('$'));
    assert!(!dir.path().join("argv").exists(), "passwd never ran");
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
