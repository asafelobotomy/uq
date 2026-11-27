//! What a startup app can wait for (Settings › Startup, per app): a network connection, or mains
//! power. Both are read from files under a root (`/` for real, a folder in tests).

use std::path::Path;
use std::time::Duration;
use uq_core::autostart::When;
use uq_core::autostart::extra::NETWORK_WAIT_SECS;

/// Shown when an app that starts only on mains power wasn't started.
pub const ON_BATTERY: &str = "Not started: the computer was on battery";
/// Shown while an app waits for a connection.
pub const WAITING_NETWORK: &str = "Waiting for the network";

const POLL: Duration = Duration::from_secs(1);

/// Whether a default route exists: a line of `/proc/net/route` whose destination is 0 on an
/// interface other than `lo`, or `::/0` in `/proc/net/ipv6_route`.
pub fn online(root: &Path) -> bool {
    let v4 = std::fs::read_to_string(root.join("proc/net/route")).unwrap_or_default();
    let v4 = v4.lines().skip(1).any(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        f.len() > 2 && f[0] != "lo" && f[1] == "00000000"
    });
    let v6 = std::fs::read_to_string(root.join("proc/net/ipv6_route")).unwrap_or_default();
    let v6 = v6.lines().any(|l| {
        let f: Vec<&str> = l.split_whitespace().collect();
        f.len() > 9 && f[0] == "00000000000000000000000000000000" && f[1] == "00" && f[9] != "lo"
    });
    v4 || v6
}

/// Whether the computer runs on mains power: a mains supply says it is online, or, with none
/// listed (most desktops), no battery says it is discharging.
pub fn on_mains(root: &Path) -> bool {
    let dir = root.join("sys/class/power_supply");
    let Ok(rd) = std::fs::read_dir(&dir) else {
        return true;
    };
    let read = |p: &Path, f: &str| {
        std::fs::read_to_string(p.join(f))
            .unwrap_or_default()
            .trim()
            .to_string()
    };
    let mut mains = None;
    let mut discharging = false;
    for e in rd.flatten() {
        let p = e.path();
        match read(&p, "type").as_str() {
            "Mains" => {
                let on = read(&p, "online") == "1";
                mains = Some(mains.unwrap_or(false) || on);
            }
            "Battery" => discharging |= read(&p, "status") == "Discharging",
            _ => {}
        }
    }
    mains.unwrap_or(!discharging)
}

/// What to do about `when` now: `Err(reason)` when the app isn't to start this login. A wait
/// for the network calls `waiting(true)` while it waits, gives up after
/// [`NETWORK_WAIT_SECS`] and starts the app anyway.
pub async fn settle(root: &Path, when: When, waiting: impl Fn(bool)) -> Result<(), &'static str> {
    match when {
        When::Power if !on_mains(root) => Err(ON_BATTERY),
        When::Power => Ok(()),
        When::Network => {
            if online(root) {
                return Ok(());
            }
            waiting(true);
            let until = tokio::time::Instant::now() + Duration::from_secs(NETWORK_WAIT_SECS);
            while tokio::time::Instant::now() < until && !online(root) {
                tokio::time::sleep(POLL).await;
            }
            waiting(false);
            Ok(())
        }
    }
}

/// Wait for `id`'s condition, showing "waiting for the network" while it waits. False when it
/// isn't met: the app is marked held and not started this login.
pub(super) async fn met(ctx: &super::Ctx, id: &str, when: When) -> bool {
    let (runner, emit, key) = (ctx.runner.clone(), ctx.emit.clone(), id.to_string());
    let waiting = move |on: bool| {
        let mut r = super::lock(&runner);
        if on {
            r.holding.insert(key.clone(), WAITING_NETWORK);
        } else {
            r.holding.remove(&key);
        }
        drop(r);
        emit();
    };
    let Err(why) = settle(&ctx.probe_root, when, waiting).await else {
        return true;
    };
    tracing::info!("startup app {id} not started: {why}");
    {
        let mut r = super::lock(&ctx.runner);
        r.statuses.insert(id.to_string(), super::Status::Held(why));
        r.record.pending.remove(id);
        r.record.save(&ctx.path);
    }
    (ctx.emit)();
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn supply(root: &Path, name: &str, kind: &str, extra: &[(&str, &str)]) {
        let d = root.join("sys/class/power_supply").join(name);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("type"), format!("{kind}\n")).unwrap();
        for (f, v) in extra {
            std::fs::write(d.join(f), format!("{v}\n")).unwrap();
        }
    }

    #[test]
    fn mains_power_is_read_from_the_supplies() {
        let d = tempfile::tempdir().unwrap();
        assert!(on_mains(d.path()), "no supplies at all: a desktop");
        supply(d.path(), "BAT0", "Battery", &[("status", "Discharging")]);
        assert!(!on_mains(d.path()));
        supply(d.path(), "AC", "Mains", &[("online", "1")]);
        assert!(on_mains(d.path()));
        supply(d.path(), "AC", "Mains", &[("online", "0")]);
        assert!(!on_mains(d.path()));
    }

    #[test]
    fn a_default_route_means_online() {
        let d = tempfile::tempdir().unwrap();
        let net = d.path().join("proc/net");
        std::fs::create_dir_all(&net).unwrap();
        let head = "Iface\tDestination\tGateway\tFlags\n";
        std::fs::write(net.join("route"), format!("{head}lo\t00000000\t0\t1\n")).unwrap();
        assert!(!online(d.path()));
        std::fs::write(
            net.join("route"),
            format!("{head}wlan0\t0001A8C0\t0\t1\nwlan0\t00000000\t0101A8C0\t3\n"),
        )
        .unwrap();
        assert!(online(d.path()));
    }

    #[tokio::test(start_paused = true)]
    async fn the_network_wait_gives_up_and_starts() {
        let d = tempfile::tempdir().unwrap();
        let seen = std::sync::Mutex::new(Vec::new());
        let r = settle(d.path(), When::Network, |w| seen.lock().unwrap().push(w)).await;
        assert_eq!(r, Ok(()));
        assert_eq!(*seen.lock().unwrap(), [true, false]);
        supply(d.path(), "BAT0", "Battery", &[("status", "Discharging")]);
        assert_eq!(settle(d.path(), When::Power, |_| {}).await, Err(ON_BATTERY));
    }
}
