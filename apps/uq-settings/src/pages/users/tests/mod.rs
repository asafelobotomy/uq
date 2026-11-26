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

mod accounts;
mod passwords;
