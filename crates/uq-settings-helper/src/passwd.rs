//! The user lookups auto-login needs (the uid, and whether the account is locked or expired),
//! behind a trait so tests don't depend on the machine's accounts.

use std::ffi::{CStr, CString};
use std::io;

/// Look up a login name's uid.
pub trait Passwd: Send + Sync {
    /// `Ok(None)` when no such user exists.
    fn uid_of(&self, name: &str) -> io::Result<Option<u32>>;

    /// The account's shadow entry, or `Ok(None)` when it has none here (an account from a
    /// directory service, say), in which case nothing more is known about it.
    fn shadow(&self, name: &str) -> io::Result<Option<Shadow>> {
        let _ = name;
        Ok(None)
    }
}

/// What `/etc/shadow` says about an account, in days since 1970-01-01; `-1` is unset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shadow {
    /// The password field starts with `!` (`passwd -l`, `usermod -L`).
    pub locked: bool,
    pub last_change: i64,
    pub max: i64,
    pub inactive: i64,
    pub expire: i64,
}

impl Shadow {
    /// Why this account can't log in on day `today`, if it can't: locked, past its expiry
    /// date, or its password expired so long ago that the account went inactive.
    pub fn refusal(&self, today: i64) -> Option<&'static str> {
        if self.locked {
            return Some("the account is locked");
        }
        if self.expire >= 0 && today >= self.expire {
            return Some("the account has expired");
        }
        if self.last_change > 0
            && self.max >= 0
            && self.inactive >= 0
            && today >= self.last_change + self.max + self.inactive
        {
            return Some("the account is inactive because its password expired");
        }
        None
    }
}

/// Today, in days since 1970-01-01 (as shadow counts).
pub fn today() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| (d.as_secs() / 86_400) as i64)
}

/// The system's NSS database, through `getpwnam_r(3)`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPasswd;

impl Passwd for SystemPasswd {
    fn uid_of(&self, name: &str) -> io::Result<Option<u32>> {
        let cname = CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "name contains NUL"))?;
        let mut buf = vec![0u8; 1024];
        loop {
            let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
            let mut result: *mut libc::passwd = std::ptr::null_mut();
            // SAFETY: every pointer is valid for the call, and `buf` outlives every use of
            // `pwd`'s string fields (we read only `pw_uid` and `pw_name`, before `buf` changes).
            let rc = unsafe {
                libc::getpwnam_r(
                    cname.as_ptr(),
                    &mut pwd,
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    &mut result,
                )
            };
            if rc == libc::ERANGE && buf.len() < (1 << 20) {
                buf.resize(buf.len() * 2, 0);
                continue;
            }
            if rc != 0 {
                return Err(io::Error::from_raw_os_error(rc));
            }
            if result.is_null() {
                return Ok(None);
            }
            // Belt and braces: the entry must be for exactly the name asked about.
            let got = unsafe { CStr::from_ptr(pwd.pw_name) };
            if got.to_bytes() != name.as_bytes() {
                return Ok(None);
            }
            return Ok(Some(pwd.pw_uid));
        }
    }

    fn shadow(&self, name: &str) -> io::Result<Option<Shadow>> {
        let cname = CString::new(name)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "name contains NUL"))?;
        let mut buf = vec![0u8; 1024];
        loop {
            let mut sp: libc::spwd = unsafe { std::mem::zeroed() };
            let mut result: *mut libc::spwd = std::ptr::null_mut();
            // SAFETY: every pointer is valid for the call, and `buf` outlives every use of
            // `sp`'s string fields (read below, before `buf` changes).
            let rc = unsafe {
                libc::getspnam_r(
                    cname.as_ptr(),
                    &mut sp,
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    &mut result,
                )
            };
            if rc == libc::ERANGE && buf.len() < (1 << 20) {
                buf.resize(buf.len() * 2, 0);
                continue;
            }
            // No shadow file or no entry in it: nothing more is known.
            if rc == libc::ENOENT || (rc == 0 && result.is_null()) {
                return Ok(None);
            }
            if rc != 0 {
                return Err(io::Error::from_raw_os_error(rc));
            }
            let got = unsafe { CStr::from_ptr(sp.sp_namp) };
            if got.to_bytes() != name.as_bytes() {
                return Ok(None);
            }
            let locked = !sp.sp_pwdp.is_null()
                && unsafe { CStr::from_ptr(sp.sp_pwdp) }.to_bytes().first() == Some(&b'!');
            // `c_long` is `i64` here but narrower on 32-bit targets.
            #[allow(clippy::useless_conversion)]
            return Ok(Some(Shadow {
                locked,
                last_change: i64::from(sp.sp_lstchg),
                max: i64::from(sp.sp_max),
                inactive: i64::from(sp.sp_inact),
                expire: i64::from(sp.sp_expire),
            }));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_uid_0_and_a_made_up_name_is_missing() {
        assert_eq!(SystemPasswd.uid_of("root").unwrap(), Some(0));
        assert_eq!(SystemPasswd.uid_of("uq-no-such-user-xyz").unwrap(), None);
        assert!(SystemPasswd.uid_of("a\0b").is_err());
    }

    #[test]
    fn a_made_up_name_has_no_shadow_entry() {
        // Unprivileged, the shadow file can't be read; either way there is no entry.
        assert!(matches!(
            SystemPasswd.shadow("uq-no-such-user-xyz"),
            Ok(None) | Err(_)
        ));
    }

    #[test]
    fn locked_expired_and_inactive_accounts_are_refused() {
        let ok = Shadow {
            locked: false,
            last_change: 20_000,
            max: 90,
            inactive: 7,
            expire: -1,
        };
        assert_eq!(ok.refusal(20_050), None);
        assert_eq!(
            Shadow { locked: true, ..ok }.refusal(20_050),
            Some("the account is locked")
        );
        assert_eq!(
            Shadow {
                expire: 20_050,
                ..ok
            }
            .refusal(20_050),
            Some("the account has expired")
        );
        assert_eq!(
            Shadow {
                expire: 20_051,
                ..ok
            }
            .refusal(20_050),
            None
        );
        // Password expired at 20 090, the account a week later.
        assert_eq!(ok.refusal(20_096), None);
        assert!(ok.refusal(20_097).unwrap().contains("inactive"));
        // No maximum age, or "change at next login" (0): never inactive.
        assert_eq!(Shadow { max: -1, ..ok }.refusal(30_000), None);
        assert_eq!(
            Shadow {
                last_change: 0,
                ..ok
            }
            .refusal(30_000),
            None
        );
    }
}
