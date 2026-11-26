//! Talking to AccountsService: listing, adding, changing and removing users.

use super::*;

pub(crate) fn check_avatar(path: &Path) -> zbus::Result<()> {
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

    pub(crate) fn notice(&self, text: &str) {
        let mut n = self.notice.lock().unwrap();
        *n = (n.0 + 1, text.to_string());
    }

    pub(crate) async fn users(&self) -> zbus::Result<Vec<User>> {
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

    pub(crate) async fn find(&self, uid: u64) -> zbus::Result<(Vec<User>, User)> {
        let users = self.users().await.map_err(missing_or)?;
        let user = users
            .iter()
            .find(|u| u.uid == uid)
            .cloned()
            .ok_or_else(|| failure("That account isn't there any more."))?;
        Ok((users, user))
    }

    pub(crate) async fn proxy(&self, user: &User) -> zbus::Result<AccountsUserProxy<'static>> {
        let conn = self.system.conn().await?;
        fresh_at(&conn, user.path.clone()).await
    }

    pub(crate) async fn set_password(&self, user: &User, password: &str) -> zbus::Result<bool> {
        let hashed = crypt::hash(password).map_err(failure)?;
        let r = self.proxy(user).await?.set_password(&hashed, "").await;
        let ok = r.is_ok();
        settle(r, &ACCOUNTS)?;
        Ok(ok)
    }

    pub(crate) async fn add_user(
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

    pub(crate) async fn remove_user(
        &self,
        uid: u64,
        typed: &str,
        remove_files: bool,
    ) -> zbus::Result<()> {
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
            let mut text = if remove_files {
                format!("{} and their files were removed.", user.name)
            } else {
                format!("{} was removed. Their files were kept.", user.name)
            };
            // The login screen must not go on trying to log in an account that's gone.
            if auto_login_user(&self.root) == user.name {
                let cleared = async { HelperProxy::new(&conn).await?.set_auto_login("").await };
                text.push_str(match cleared.await {
                    Ok(()) => " Auto-login is off.",
                    Err(_) => " Auto-login still names them: turn it off on the Login screen page.",
                });
            }
            self.notice(&text);
        }
        Ok(())
    }
}
