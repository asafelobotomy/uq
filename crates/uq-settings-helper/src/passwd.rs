//! The user lookup auto-login needs, behind a trait so tests don't depend on the machine's
//! accounts.

use std::ffi::{CStr, CString};
use std::io;

/// Look up a login name's uid.
pub trait Passwd: Send + Sync {
    /// `Ok(None)` when no such user exists.
    fn uid_of(&self, name: &str) -> io::Result<Option<u32>>;
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
}
