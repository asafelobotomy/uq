//! The Users page (spec §2.6, §3): the accounts on this computer through AccountsService, which
//! asks polkit itself (administrator password) for every change to someone else, and for most
//! changes to an account type. Your own password is changed through `passwd` (users/passwd.rs),
//! which checks your current password and needs no administrator.
//!
//! Rules the page enforces before asking (spec §3): removing a user needs their name typed;
//! you can't remove yourself or the last administrator, or make the last administrator a
//! standard user. Passwords are hashed here (crypt(3)) and never kept.

pub(crate) mod crypt;
pub(crate) mod passwd;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex as StdMutex;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::Mutex;

use super::login_screen::{GREETD_TOML, auto_login_user};
use super::proxies::{AccountsProxy, AccountsUserProxy, fresh, fresh_at};
use super::runner::PageLogic;
use super::sysbus::{self, ACCOUNTS, SystemBus, failure, settle};

/// AccountsService refuses larger pictures.
pub const MAX_AVATAR: u64 = 1024 * 1024;
/// The longest user name `useradd` takes.
pub const MAX_NAME: usize = 32;
pub const MAX_REAL_NAME: usize = 255;

#[derive(Clone)]
pub enum Op {
    Refresh,
    SetRealName {
        uid: u64,
        name: String,
    },
    SetAdmin {
        uid: u64,
        admin: bool,
    },
    /// `current` is needed (and used) only for your own account.
    SetPassword {
        uid: u64,
        current: String,
        password: String,
        confirm: String,
    },
    SetAvatar {
        uid: u64,
        path: PathBuf,
    },
    AddUser {
        name: String,
        real_name: String,
        admin: bool,
        password: String,
        confirm: String,
    },
    /// `typed` must be the user's name.
    RemoveUser {
        uid: u64,
        typed: String,
        remove_files: bool,
    },
}

// Passwords never reach a log through `{:?}`.
impl std::fmt::Debug for Op {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Op::SetPassword { uid, .. } => write!(f, "SetPassword {{ uid: {uid}, .. }}"),
            Op::AddUser { name, .. } => write!(f, "AddUser {{ name: {name:?}, .. }}"),
            Op::Refresh => write!(f, "Refresh"),
            Op::SetRealName { uid, .. } => write!(f, "SetRealName {{ uid: {uid}, .. }}"),
            Op::SetAdmin { uid, admin } => write!(f, "SetAdmin {{ uid: {uid}, admin: {admin} }}"),
            Op::SetAvatar { uid, .. } => write!(f, "SetAvatar {{ uid: {uid}, .. }}"),
            Op::RemoveUser { uid, .. } => write!(f, "RemoveUser {{ uid: {uid}, .. }}"),
        }
    }
}

/// One account as the page shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub uid: u64,
    pub name: String,
    pub real_name: String,
    pub admin: bool,
    pub icon: String,
    pub locked: bool,
    path: zbus::zvariant::OwnedObjectPath,
}

pub struct UsersState {
    system: SystemBus,
    me: u64,
    root: PathBuf,
    /// `passwd` and its arguments (a stub under `/bin/sh` in tests), and how long it may take.
    passwd: Vec<OsString>,
    passwd_timeout: Duration,
    write: Mutex<()>,
    notice: StdMutex<(u64, String)>,
}

/// A user name `useradd` accepts: a lowercase letter or `_`, then lowercase letters, digits,
/// `_` or `-`, at most 32 in all.
pub fn valid_user_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c == '_')
        && name.len() <= MAX_NAME
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
}

/// Why a real name can't be saved, if it can't.
pub fn real_name_problem(name: &str) -> Option<&'static str> {
    if name.chars().count() > MAX_REAL_NAME {
        return Some("That name is too long.");
    }
    if name.chars().any(|c| c.is_control() || c == ':' || c == ',') {
        return Some("A name can't contain a colon, a comma or a line break.");
    }
    None
}

/// Why `password` and `confirm` can't be used, if they can't.
pub fn password_problem(password: &str, confirm: &str) -> Option<&'static str> {
    if password.is_empty() {
        return Some("Type a password.");
    }
    if password != confirm {
        return Some("The two passwords don't match.");
    }
    None
}

/// Why `target` can't be removed by `me`, if it can't.
pub fn remove_problem(users: &[User], me: u64, target: &User) -> Option<&'static str> {
    if target.uid == me {
        return Some("You can't remove your own account.");
    }
    let admins = users.iter().filter(|u| u.admin).count();
    if target.admin && admins <= 1 {
        return Some("This is the only administrator, so it can't be removed.");
    }
    None
}

/// Why `target` can't be made a standard user, if it can't.
pub fn demote_problem(users: &[User], target: &User) -> Option<&'static str> {
    let admins = users.iter().filter(|u| u.admin).count();
    (target.admin && admins <= 1).then_some("This is the only administrator, so it must stay one.")
}

/// Wipe a password string's bytes before it is dropped.
fn wipe(s: &mut String) {
    // SAFETY: zero bytes are valid UTF-8.
    for b in unsafe { s.as_bytes_mut() } {
        // SAFETY: a valid, owned byte.
        unsafe { std::ptr::write_volatile(b, 0) };
    }
    s.clear();
}

impl UsersState {
    pub fn new(_session: zbus::Connection) -> Self {
        Self::with(
            SystemBus::system(),
            u64::from(sysbus::my_uid()),
            PathBuf::from("/"),
        )
    }

    pub fn with(system: SystemBus, me: u64, root: PathBuf) -> Self {
        Self {
            system,
            me,
            root,
            passwd: vec![OsString::from(passwd::PASSWD)],
            passwd_timeout: passwd::TIMEOUT,
            write: Mutex::new(()),
            notice: StdMutex::new((0, String::new())),
        }
    }

    /// Use `cmd` instead of `/usr/bin/passwd` for your own password (tests).
    #[cfg(test)]
    pub fn with_passwd(mut self, cmd: Vec<OsString>, timeout: Duration) -> Self {
        self.passwd = cmd;
        self.passwd_timeout = timeout;
        self
    }

    fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    async fn users(&self) -> zbus::Result<Vec<User>> {
        let conn = self.system.conn().await?;
        let manager: AccountsProxy = fresh(&conn).await?;
        let paths = sysbus::within(manager.list_cached_users()).await?;
        let mut out = Vec::new();
        for path in paths {
            let u: AccountsUserProxy = fresh_at(&conn, path.clone()).await?;
            let read = async {
                if u.system_account().await.unwrap_or(false) {
                    return Ok(None);
                }
                Ok(Some(User {
                    uid: u.uid().await?,
                    name: u.user_name().await?,
                    real_name: u.real_name().await.unwrap_or_default(),
                    admin: u.account_type().await.unwrap_or(0) == 1,
                    icon: u.icon_file().await.unwrap_or_default(),
                    locked: u.locked().await.unwrap_or(false),
                    path: path.clone(),
                }))
            };
            if let Some(user) = sysbus::within(read).await? {
                out.push(user);
            }
        }
        out.sort_by_key(|a| (a.uid != self.me, a.uid));
        Ok(out)
    }

    async fn find(&self, uid: u64) -> zbus::Result<(Vec<User>, User)> {
        let users = self.users().await.map_err(missing_or)?;
        let user = users
            .iter()
            .find(|u| u.uid == uid)
            .cloned()
            .ok_or_else(|| failure("That account isn't there any more."))?;
        Ok((users, user))
    }

    async fn proxy(&self, user: &User) -> zbus::Result<AccountsUserProxy<'static>> {
        let conn = self.system.conn().await?;
        fresh_at(&conn, user.path.clone()).await
    }

    async fn set_password(&self, user: &User, password: &str) -> zbus::Result<bool> {
        let hashed = crypt::hash(password).map_err(failure)?;
        let r = self.proxy(user).await?.set_password(&hashed, "").await;
        let ok = r.is_ok();
        settle(r, &ACCOUNTS)?;
        Ok(ok)
    }

    async fn add_user(
        &self,
        name: &str,
        real_name: &str,
        admin: bool,
        password: &str,
        confirm: &str,
    ) -> zbus::Result<()> {
        let name = name.trim();
        if !valid_user_name(name) {
            return Err(failure(
                "A user name starts with a lowercase letter and has only lowercase letters, digits, - and _ (at most 32).",
            ));
        }
        if let Some(p) =
            real_name_problem(real_name).or_else(|| password_problem(password, confirm))
        {
            return Err(failure(p));
        }
        let users = self.users().await.map_err(missing_or)?;
        if users.iter().any(|u| u.name == name) {
            return Err(failure("There's already an account with that user name."));
        }
        // Hash before the prompt, so a failure here asks for nothing.
        let hashed = crypt::hash(password).map_err(failure)?;
        let conn = self.system.conn().await?;
        let manager: AccountsProxy = fresh(&conn).await?;
        let path = match manager
            .create_user(name, real_name.trim(), i32::from(admin))
            .await
        {
            Ok(p) => p,
            Err(e) => return settle(Err::<(), _>(e), &ACCOUNTS),
        };
        let user: AccountsUserProxy = fresh_at(&conn, path).await?;
        if let Err(e) = user.set_password(&hashed, "").await {
            return Err(failure(format!(
                "The account was made, but its password wasn't set ({}). Set it before logging in.",
                sysbus::error_text(&e)
            )));
        }
        self.notice(&format!("{name} can log in now."));
        Ok(())
    }

    async fn remove_user(&self, uid: u64, typed: &str, remove_files: bool) -> zbus::Result<()> {
        let (users, user) = self.find(uid).await?;
        if let Some(p) = remove_problem(&users, self.me, &user) {
            return Err(failure(p));
        }
        if typed != user.name {
            return Err(failure("Type the user name exactly to remove the account."));
        }
        let conn = self.system.conn().await?;
        let manager: AccountsProxy = fresh(&conn).await?;
        let id = i64::try_from(uid).map_err(|_| failure("That account can't be removed."))?;
        let r = manager.delete_user(id, remove_files).await;
        let ok = r.is_ok();
        settle(r, &ACCOUNTS)?;
        if ok {
            self.notice(&if remove_files {
                format!("{} and their files were removed.", user.name)
            } else {
                format!("{} was removed. Their files were kept.", user.name)
            });
        }
        Ok(())
    }
}

/// A failed read of the accounts as the page shows it: a missing service names its package.
fn missing_or(e: zbus::Error) -> zbus::Error {
    match sysbus::classify(&e) {
        sysbus::Failure::Missing => failure(ACCOUNTS.missing_text()),
        _ => e,
    }
}

fn check_avatar(path: &Path) -> zbus::Result<()> {
    let meta = std::fs::metadata(path)
        .map_err(|e| failure(format!("That picture couldn't be read: {e}")))?;
    if !meta.is_file() {
        return Err(failure("That isn't a picture file."));
    }
    if meta.len() > MAX_AVATAR {
        return Err(failure(
            "That picture is larger than 1 MiB. Pick a smaller one.",
        ));
    }
    Ok(())
}

impl PageLogic for UsersState {
    type Op = Op;

    fn refresh() -> Op {
        Op::Refresh
    }

    fn is_refresh(op: &Op) -> bool {
        matches!(op, Op::Refresh)
    }

    fn write_lock(&self) -> &Mutex<()> {
        &self.write
    }

    async fn apply(&self, op: Op) -> zbus::Result<()> {
        match op {
            Op::Refresh => Ok(()),
            Op::SetRealName { uid, name } => {
                if let Some(p) = real_name_problem(&name) {
                    return Err(failure(p));
                }
                let (_, user) = self.find(uid).await?;
                settle(
                    self.proxy(&user).await?.set_real_name(name.trim()).await,
                    &ACCOUNTS,
                )
            }
            Op::SetAdmin { uid, admin } => {
                let (users, user) = self.find(uid).await?;
                if !admin && let Some(p) = demote_problem(&users, &user) {
                    return Err(failure(p));
                }
                settle(
                    self.proxy(&user)
                        .await?
                        .set_account_type(i32::from(admin))
                        .await,
                    &ACCOUNTS,
                )
            }
            Op::SetPassword {
                uid,
                mut current,
                mut password,
                mut confirm,
            } => {
                let r = async {
                    if let Some(p) = password_problem(&password, &confirm) {
                        return Err(failure(p));
                    }
                    if uid == self.me {
                        // Your own: passwd checks the current password; no administrator.
                        if current.is_empty() {
                            return Err(failure("Type your current password."));
                        }
                        passwd::change_own(&self.passwd, &current, &password, self.passwd_timeout)
                            .await
                            .map_err(failure)?;
                        self.notice("Your password was changed.");
                        return Ok(());
                    }
                    let (_, user) = self.find(uid).await?;
                    if self.set_password(&user, &password).await? {
                        self.notice("The password was changed.");
                    }
                    Ok(())
                }
                .await;
                wipe(&mut current);
                wipe(&mut password);
                wipe(&mut confirm);
                r
            }
            Op::SetAvatar { uid, path } => {
                check_avatar(&path)?;
                let (_, user) = self.find(uid).await?;
                let path = path
                    .to_str()
                    .ok_or_else(|| failure("That file name can't be used."))?;
                settle(
                    self.proxy(&user).await?.set_icon_file(path).await,
                    &ACCOUNTS,
                )
            }
            Op::AddUser {
                name,
                real_name,
                admin,
                mut password,
                mut confirm,
            } => {
                let r = self
                    .add_user(&name, &real_name, admin, &password, &confirm)
                    .await;
                wipe(&mut password);
                wipe(&mut confirm);
                r
            }
            Op::RemoveUser {
                uid,
                typed,
                remove_files,
            } => self.remove_user(uid, &typed, remove_files).await,
        }
    }

    async fn snapshot(&self) -> zbus::Result<Value> {
        let (users, service) = match self.users().await {
            Ok(users) => (users, Value::Null),
            Err(e) => (Vec::new(), sysbus::read_error(&e, &ACCOUNTS)),
        };
        let rows: Vec<Value> = users
            .iter()
            .map(|u| {
                json!({
                    "uid": u.uid,
                    "name": u.name,
                    "real_name": u.real_name,
                    "admin": u.admin,
                    "icon": u.icon,
                    "locked": u.locked,
                    "me": u.uid == self.me,
                    "remove_problem": remove_problem(&users, self.me, u).unwrap_or(""),
                    "demote_problem": demote_problem(&users, u).unwrap_or(""),
                })
            })
            .collect();
        let (seq, text) = self.notice.lock().unwrap().clone();
        Ok(json!({
            "service": service,
            "users": rows,
            "me": self.me,
            "auto_login": {
                "greeter": self.root.join(GREETD_TOML).is_file(),
                "user": auto_login_user(&self.root),
            },
            "limits": {"name": MAX_NAME, "real_name": MAX_REAL_NAME},
            "notice": {"seq": seq, "text": text},
        }))
    }
}

#[cfg(test)]
mod tests;
