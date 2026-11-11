//! Hashing a typed password for AccountsService, which takes a crypt(3) hash, never the text.
//! libxcrypt picks the system's preferred method (yescrypt on Arch) and reads its own random
//! salt from the kernel. The buffers that held the text are wiped before they are freed.

use std::ffi::{CStr, CString, c_char, c_int, c_ulong, c_void};

#[link(name = "crypt")]
unsafe extern "C" {
    fn crypt_gensalt_ra(
        prefix: *const c_char,
        count: c_ulong,
        rbytes: *const c_char,
        nrbytes: c_int,
    ) -> *mut c_char;
    fn crypt_ra(
        phrase: *const c_char,
        setting: *const c_char,
        data: *mut *mut c_void,
        size: *mut c_int,
    ) -> *mut c_char;
    fn free(ptr: *mut c_void);
}

/// Overwrite `len` bytes at `ptr`, in a way the optimiser keeps.
unsafe fn wipe(ptr: *mut u8, len: usize) {
    for i in 0..len {
        // SAFETY: the caller owns `len` bytes at `ptr`.
        unsafe { std::ptr::write_volatile(ptr.add(i), 0) };
    }
}

/// The crypt(3) hash of `password` with a fresh salt.
pub fn hash(password: &str) -> Result<String, String> {
    if password.contains('\0') {
        return Err("A password can't contain a NUL character.".into());
    }
    let phrase = CString::new(password).map_err(|_| "bad password".to_string())?;
    // SAFETY: a null prefix asks for the default method; null rbytes asks libxcrypt to read
    // its own random bytes. The result is malloc'd and freed below.
    let setting = unsafe { crypt_gensalt_ra(std::ptr::null(), 0, std::ptr::null(), 0) };
    if setting.is_null() {
        wipe_cstring(phrase);
        return Err("The system couldn't make a password salt.".into());
    }
    let mut data: *mut c_void = std::ptr::null_mut();
    let mut size: c_int = 0;
    // SAFETY: valid NUL-terminated strings; crypt_ra allocates `data` (size in `size`) and
    // returns a pointer into it.
    let out = unsafe { crypt_ra(phrase.as_ptr(), setting, &mut data, &mut size) };
    let result = if out.is_null() {
        Err("The system couldn't hash the password.".to_string())
    } else {
        // SAFETY: `out` points to a NUL-terminated string inside `data`.
        let s = unsafe { CStr::from_ptr(out) }
            .to_string_lossy()
            .into_owned();
        // A failed hash starts with '*' (libxcrypt's failure token).
        if s.starts_with('*') || s.is_empty() {
            Err("The system couldn't hash the password.".to_string())
        } else {
            Ok(s)
        }
    };
    // SAFETY: `data` holds `size` bytes from crypt_ra (or is null); `setting` was malloc'd.
    unsafe {
        if !data.is_null() {
            wipe(data.cast(), usize::try_from(size).unwrap_or(0));
            free(data);
        }
        free(setting.cast());
    }
    wipe_cstring(phrase);
    result
}

fn wipe_cstring(s: CString) {
    let mut bytes = s.into_bytes();
    let len = bytes.len();
    // SAFETY: we own the buffer.
    unsafe { wipe(bytes.as_mut_ptr(), len) };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_are_salted_crypt_strings() {
        let a = hash("correct horse").unwrap();
        let b = hash("correct horse").unwrap();
        assert!(a.starts_with('$'), "{a}");
        assert_ne!(a, b, "a fresh salt each time");
        assert!(!a.contains("correct horse"));
    }

    #[test]
    fn nul_is_refused() {
        assert!(hash("a\0b").is_err());
    }
}
