//! A wake-up at a wall-clock time that survives suspend and notices clock changes.
//!
//! tokio's timers run on `CLOCK_MONOTONIC`, which stops while the machine sleeps: a sleep of
//! eight hours to the next dark/light boundary would end eight hours of *awake* time later. A
//! `CLOCK_REALTIME` timerfd armed with an absolute time expires as soon as the machine is awake
//! past that time, and with `TFD_TIMER_CANCEL_ON_SET` a read fails with `ECANCELED` when the
//! clock is set (NTP step, manual change), so the caller can work out the deadline again.

use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;

/// Why `WallTimer::wait` returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Wake {
    /// The time it was armed for has come.
    Due,
    /// The wall clock was set; the deadline should be worked out again.
    ClockChanged,
}

pub struct WallTimer {
    fd: AsyncFd<OwnedFd>,
}

/// Where a timer with nothing to wait for is armed: far enough to never matter, so a clock
/// change is still reported.
const FAR: Duration = Duration::from_secs(365 * 24 * 3600);

impl WallTimer {
    pub fn new() -> std::io::Result<WallTimer> {
        // SAFETY: plain syscall; the result is checked before it is used as a descriptor.
        let raw = unsafe {
            libc::timerfd_create(
                libc::CLOCK_REALTIME,
                libc::TFD_NONBLOCK | libc::TFD_CLOEXEC,
            )
        };
        if raw < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: `raw` is a fresh descriptor that nothing else owns.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        // SAFETY: an OwnedFd is open and always returns the same descriptor until dropped,
        // which happens only with the AsyncFd.
        let fd = unsafe { AsyncFd::register_with_interest(fd, Interest::READABLE) }
            .map_err(std::io::Error::from)?;
        Ok(WallTimer { fd })
    }

    fn arm(&self, at: SystemTime) -> std::io::Result<()> {
        let since = at.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
        let spec = libc::itimerspec {
            it_interval: libc::timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: libc::timespec {
                tv_sec: since.as_secs() as libc::time_t,
                // An all-zero value would disarm the timer: 1 ns past the epoch instead.
                tv_nsec: if since.is_zero() {
                    1
                } else {
                    libc::c_long::from(since.subsec_nanos())
                },
            },
        };
        // SAFETY: valid descriptor and a fully initialised itimerspec; no old value wanted.
        let r = unsafe {
            libc::timerfd_settime(
                self.fd.as_raw_fd(),
                libc::TFD_TIMER_ABSTIME | libc::TFD_TIMER_CANCEL_ON_SET,
                &spec,
                std::ptr::null_mut(),
            )
        };
        if r < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    }

    /// Wait until the wall clock reaches `at` (`None`: only for a clock change). A time already
    /// past returns `Due` at once.
    pub async fn wait(&mut self, at: Option<SystemTime>) -> std::io::Result<Wake> {
        self.arm(at.unwrap_or_else(|| SystemTime::now() + FAR))?;
        loop {
            let mut guard = self.fd.readable().await?;
            let mut n = [0u8; 8];
            // SAFETY: reads at most 8 bytes into `n`.
            let r = unsafe {
                libc::read(
                    guard.get_inner().as_raw_fd(),
                    n.as_mut_ptr().cast(),
                    n.len(),
                )
            };
            if r == 8 {
                return Ok(Wake::Due);
            }
            let e = std::io::Error::last_os_error();
            match e.raw_os_error() {
                Some(libc::EAGAIN) => guard.clear_ready(),
                Some(libc::EINTR) => {}
                Some(libc::ECANCELED) => return Ok(Wake::ClockChanged),
                _ => return Err(e),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fires_at_the_wall_clock_time() {
        let mut t = WallTimer::new().unwrap();
        let start = std::time::Instant::now();
        let at = SystemTime::now() + Duration::from_millis(150);
        assert_eq!(t.wait(Some(at)).await.unwrap(), Wake::Due);
        assert!(SystemTime::now() >= at);
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[tokio::test]
    async fn a_past_time_is_due_at_once() {
        let mut t = WallTimer::new().unwrap();
        let past = SystemTime::now() - Duration::from_secs(60);
        let w = tokio::time::timeout(Duration::from_secs(1), t.wait(Some(past))).await;
        assert_eq!(w.unwrap().unwrap(), Wake::Due);
        // Re-armed for later, it waits again.
        let later = SystemTime::now() + Duration::from_secs(30);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), t.wait(Some(later)))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn nothing_to_wait_for_waits() {
        let mut t = WallTimer::new().unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(200), t.wait(None))
                .await
                .is_err()
        );
    }
}
