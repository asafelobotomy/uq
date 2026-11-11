//! A fake AccountsService: the manager object and one object per user, over one shared list.

use std::sync::{Arc, Mutex};

use zbus::zvariant::OwnedObjectPath;

use super::{Deny, Shared, SysRig};

#[derive(Debug, Clone, PartialEq)]
pub struct FakeUser {
    pub uid: u64,
    pub name: String,
    pub real_name: String,
    pub account_type: i32,
    pub icon: String,
    pub locked: bool,
    pub system: bool,
    /// The crypted password last set.
    pub password: String,
}

impl FakeUser {
    pub fn new(uid: u64, name: &str, real_name: &str, admin: bool) -> Self {
        Self {
            uid,
            name: name.into(),
            real_name: real_name.into(),
            account_type: i32::from(admin),
            icon: String::new(),
            locked: false,
            system: false,
            password: String::new(),
        }
    }
}

#[derive(Debug, zbus::DBusError)]
#[zbus(prefix = "org.freedesktop.Accounts.Error")]
pub enum AccountsError {
    #[zbus(error)]
    ZBus(zbus::Error),
    PermissionDenied(String),
    UserExists(String),
    UserDoesNotExist(String),
    Failed(String),
}

fn user_path(uid: u64) -> String {
    format!("/org/freedesktop/Accounts/User{uid}")
}

struct ManagerIface {
    users: Shared<Vec<FakeUser>>,
    deny: Deny,
    deleted: Shared<Vec<(u64, bool)>>,
}

#[zbus::interface(name = "org.freedesktop.Accounts")]
impl ManagerIface {
    fn list_cached_users(&self) -> Vec<OwnedObjectPath> {
        self.users
            .lock()
            .unwrap()
            .iter()
            .filter(|u| !u.system)
            .map(|u| OwnedObjectPath::try_from(user_path(u.uid)).unwrap())
            .collect()
    }

    async fn create_user(
        &self,
        name: String,
        fullname: String,
        account_type: i32,
        #[zbus(object_server)] server: &zbus::ObjectServer,
    ) -> Result<OwnedObjectPath, AccountsError> {
        if self.deny.on() {
            return Err(AccountsError::PermissionDenied("Not authorized".into()));
        }
        let uid = {
            let mut users = self.users.lock().unwrap();
            if users.iter().any(|u| u.name == name) {
                return Err(AccountsError::UserExists(format!(
                    "A user with name '{name}' already exists"
                )));
            }
            let uid = users.iter().map(|u| u.uid).max().unwrap_or(999).max(999) + 1;
            users.push(FakeUser::new(uid, &name, &fullname, account_type == 1));
            uid
        };
        server
            .at(
                user_path(uid),
                UserIface {
                    uid,
                    users: self.users.clone(),
                    deny: self.deny.clone(),
                },
            )
            .await
            .map_err(AccountsError::ZBus)?;
        Ok(OwnedObjectPath::try_from(user_path(uid)).unwrap())
    }

    fn delete_user(&self, id: i64, remove_files: bool) -> Result<(), AccountsError> {
        if self.deny.on() {
            return Err(AccountsError::PermissionDenied("Not authorized".into()));
        }
        let mut users = self.users.lock().unwrap();
        let before = users.len();
        users.retain(|u| u.uid as i64 != id);
        if users.len() == before {
            return Err(AccountsError::UserDoesNotExist("No such user".into()));
        }
        self.deleted.lock().unwrap().push((id as u64, remove_files));
        Ok(())
    }
}

struct UserIface {
    uid: u64,
    users: Shared<Vec<FakeUser>>,
    deny: Deny,
}

impl UserIface {
    fn get<T>(&self, f: impl FnOnce(&FakeUser) -> T, default: T) -> T {
        self.users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.uid == self.uid)
            .map_or(default, f)
    }

    fn edit(&self, f: impl FnOnce(&mut FakeUser)) -> Result<(), AccountsError> {
        if self.deny.on() {
            return Err(AccountsError::PermissionDenied("Not authorized".into()));
        }
        let mut users = self.users.lock().unwrap();
        let u = users
            .iter_mut()
            .find(|u| u.uid == self.uid)
            .ok_or_else(|| AccountsError::UserDoesNotExist("No such user".into()))?;
        f(u);
        Ok(())
    }
}

#[zbus::interface(name = "org.freedesktop.Accounts.User")]
impl UserIface {
    #[zbus(property)]
    fn uid(&self) -> u64 {
        self.uid
    }
    #[zbus(property)]
    fn user_name(&self) -> String {
        self.get(|u| u.name.clone(), String::new())
    }
    #[zbus(property)]
    fn real_name(&self) -> String {
        self.get(|u| u.real_name.clone(), String::new())
    }
    #[zbus(property)]
    fn account_type(&self) -> i32 {
        self.get(|u| u.account_type, 0)
    }
    #[zbus(property)]
    fn icon_file(&self) -> String {
        self.get(|u| u.icon.clone(), String::new())
    }
    #[zbus(property)]
    fn locked(&self) -> bool {
        self.get(|u| u.locked, false)
    }
    #[zbus(property)]
    fn system_account(&self) -> bool {
        self.get(|u| u.system, false)
    }
    fn set_real_name(&self, name: String) -> Result<(), AccountsError> {
        self.edit(|u| u.real_name = name)
    }
    fn set_account_type(&self, t: i32) -> Result<(), AccountsError> {
        self.edit(|u| u.account_type = t)
    }
    fn set_password(&self, password: String, _hint: String) -> Result<(), AccountsError> {
        self.edit(|u| u.password = password)
    }
    fn set_icon_file(&self, file: String) -> Result<(), AccountsError> {
        self.edit(|u| u.icon = file)
    }
}

pub struct FakeAccounts {
    pub users: Shared<Vec<FakeUser>>,
    pub deny: Deny,
    /// `(uid, remove_files)` of each `DeleteUser`.
    pub deleted: Shared<Vec<(u64, bool)>>,
}

impl FakeAccounts {
    pub async fn start(rig: &mut SysRig, users: Vec<FakeUser>) -> Self {
        let uids: Vec<u64> = users.iter().map(|u| u.uid).collect();
        let users = Arc::new(Mutex::new(users));
        let deny = Deny::default();
        let deleted = Arc::new(Mutex::new(Vec::new()));
        let c = rig.own("org.freedesktop.Accounts").await;
        let os = c.object_server();
        os.at(
            "/org/freedesktop/Accounts",
            ManagerIface {
                users: users.clone(),
                deny: deny.clone(),
                deleted: deleted.clone(),
            },
        )
        .await
        .unwrap();
        for uid in uids {
            os.at(
                user_path(uid),
                UserIface {
                    uid,
                    users: users.clone(),
                    deny: deny.clone(),
                },
            )
            .await
            .unwrap();
        }
        Self {
            users,
            deny,
            deleted,
        }
    }

    pub fn user(&self, name: &str) -> Option<FakeUser> {
        self.users
            .lock()
            .unwrap()
            .iter()
            .find(|u| u.name == name)
            .cloned()
    }
}
