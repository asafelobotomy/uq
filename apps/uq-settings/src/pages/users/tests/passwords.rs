//! Passwords and the prompt.

use super::*;

#[tokio::test]
async fn your_own_password_goes_through_passwd_with_the_current_one() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let (page, dir, old) = stub_passwd(page, "ok");
    let new = pw();
    let out = run(&page, own_password(1000, Default::default(), &new, &new)).await;
    assert_eq!(out.error.as_deref(), Some("Type your current password."));
    assert!(!dir.path().join("argv").exists(), "passwd never ran");

    let out = run(&page, own_password(1000, &old, &new, &new)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "Your password was changed.");
    assert_eq!(
        std::fs::read_to_string(dir.path().join("changed")).unwrap(),
        new
    );
    // AccountsService (and so an administrator prompt) was never involved.
    assert_eq!(fake.user("alice").unwrap().password, "");
    // Never in argv or the environment; prompts were read in the C locale.
    let argv = std::fs::read_to_string(dir.path().join("argv")).unwrap();
    let env = std::fs::read_to_string(dir.path().join("env")).unwrap();
    for secret in [&old, &new] {
        assert!(!argv.contains(secret.as_str()) && !env.contains(secret.as_str()));
    }
    assert!(env.lines().any(|l| l == "LC_ALL=C"), "{env}");
}

#[tokio::test]
async fn your_own_password_failures_read_plainly() {
    let Setup(_rig, _fake, page, _root) = setup(two(), 1000).await;
    let (page, dir, old) = stub_passwd(page, "ok");
    let (new, wrong) = (pw(), pw());
    let out = run(&page, own_password(1000, &wrong, &new, &new)).await;
    assert_eq!(out.error.as_deref(), Some("The current password is wrong."));
    let short = &pw()[..5];
    let out = run(&page, own_password(1000, &old, short, short)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("That password can't be used: The password is shorter than 8 characters")
    );
    let out = run(&page, own_password(1000, &old, &pw(), &pw())).await;
    assert_eq!(out.error.as_deref(), Some("The two passwords don't match."));
    assert!(!dir.path().join("changed").exists());

    let Setup(_rig, _fake, page, _root) = setup(two(), 1000).await;
    let (page, _dir, old) = stub_passwd(page, "hang");
    let started = std::time::Instant::now();
    let out = run(&page, own_password(1000, &old, &new, &new)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("Changing the password took too long, so it was stopped.")
    );
    assert!(started.elapsed() < Duration::from_secs(20));
}

#[tokio::test]
async fn someone_elses_password_still_uses_accountsservice() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let (page, dir, _old) = stub_passwd(page, "ok");
    let new = pw();
    let out = run(&page, own_password(1001, &pw(), &new, &new)).await;
    assert_eq!(out.error, None);
    assert!(fake.user("bob").unwrap().password.starts_with('$'));
    assert!(!dir.path().join("argv").exists(), "passwd never ran");
}

#[tokio::test]
async fn passwords_are_hashed_and_checked() {
    let Setup(_rig, fake, page, _root) = setup(two(), 1000).await;
    let new = pw();
    let out = run(&page, password(1001, &new, &pw())).await;
    assert_eq!(out.error.as_deref(), Some("The two passwords don't match."));
    let out = run(
        &page,
        password(1001, Default::default(), Default::default()),
    )
    .await;
    assert_eq!(out.error.as_deref(), Some("Type a password."));
    assert_eq!(fake.user("bob").unwrap().password, "");

    let out = run(&page, password(1001, &new, &new)).await;
    assert_eq!(out.error, None);
    let stored = fake.user("bob").unwrap().password;
    assert!(stored.starts_with('$'), "a crypt hash");
    assert!(!stored.contains(new.as_str()));
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
    let p = pw();
    for bad in ["Carol", "1carol", "car ol", ""] {
        let out = run(&page, add(bad, &p, &p)).await;
        assert!(
            out.error.unwrap().starts_with("A user name starts"),
            "{bad}"
        );
    }
    let out = run(&page, add("bob", &p, &p)).await;
    assert_eq!(
        out.error.as_deref(),
        Some("There's already an account with that user name.")
    );
    let out = run(&page, add("carol", &p, &pw())).await;
    assert_eq!(out.error.as_deref(), Some("The two passwords don't match."));
    assert!(fake.user("carol").is_none());

    let out = run(&page, add("carol", &p, &p)).await;
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
    let p = pw();
    let out = run(&page, password(1001, &p, &p)).await;
    assert_eq!(out.error, None);
    assert_eq!(out.state["notice"]["text"], "", "no 'changed' notice");
}

#[test]
fn passwords_never_show_in_debug() {
    let p = pw();
    let op = password(1, &p, &p);
    assert!(!format!("{op:?}").contains(p.as_str()));
}
