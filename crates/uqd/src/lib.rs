//! uqd: the uQ session daemon.
pub mod autostart;
pub mod compositor;
pub mod dbus;
mod files;
pub mod hypr;
mod monitors;
pub mod pick;
pub mod power;
mod profiles;
mod rules;
mod sections;
pub mod service;
mod startup;
pub mod system;
mod welcome;

/// Helpers for tests that write a script and then execute it.
///
/// Writing an executable and exec'ing it while other test threads fork can fail with ETXTBSY
/// ("Text file busy"): a concurrently forked child briefly inherits our write descriptor, and
/// exec refuses a file that is open for writing anywhere. The write fd is closed before we exec,
/// so the condition clears as soon as the other child execs; retry until it does.
#[cfg(test)]
pub(crate) mod test_util {
    use std::io;
    use std::path::Path;

    pub fn write_script(path: &Path, body: &str) {
        std::fs::write(path, body).unwrap();
        std::fs::set_permissions(path, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .unwrap();
    }

    pub fn is_busy(e: &io::Error) -> bool {
        e.kind() == io::ErrorKind::ExecutableFileBusy
    }

    /// Run `f`, retrying for up to 5 s while it fails with ETXTBSY.
    pub fn retry_busy<T>(mut f: impl FnMut() -> io::Result<T>) -> io::Result<T> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            match f() {
                Err(e) if is_busy(&e) && std::time::Instant::now() < deadline => {
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                other => return other,
            }
        }
    }
}
