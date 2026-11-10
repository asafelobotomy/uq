//! A private dbus-daemon for tests, shared by every crate (feature `test-bus`).
//!
//! Never touches the user's session bus. The socket lives under a short `/tmp/uqb*` directory
//! because unix socket paths must stay under 108 bytes.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};

pub struct Bus {
    child: Child,
    pub address: String,
    dir: tempfile::TempDir,
}

impl Bus {
    /// The directory holding the bus socket, its config and any service files.
    pub fn dir(&self) -> &Path {
        self.dir.path()
    }
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A private bus with a permissive policy and no activatable services.
pub fn private_bus() -> Bus {
    private_bus_with_services(&[])
}

/// A private bus whose `servicedir` holds the given `(file name, body)` D-Bus service files.
pub fn private_bus_with_services(services: &[(&str, &str)]) -> Bus {
    let dir = tempfile::Builder::new()
        .prefix("uqb")
        .tempdir_in("/tmp")
        .unwrap();
    let services_dir = dir.path().join("s");
    std::fs::create_dir(&services_dir).unwrap();
    for (file, body) in services {
        std::fs::write(services_dir.join(file), body).unwrap();
    }
    let conf = dir.path().join("c.conf");
    std::fs::write(
        &conf,
        format!(
            "<busconfig><type>session</type><listen>unix:path={}</listen>\
             <servicedir>{}</servicedir>\
             <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
             <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>",
            dir.path().join("b").display(),
            services_dir.display()
        ),
    )
    .unwrap();
    let mut child = Command::new("dbus-daemon")
        .args(["--nofork", "--print-address"])
        .arg(format!("--config-file={}", conf.display()))
        .stdout(Stdio::piped())
        .spawn()
        .expect("dbus-daemon");
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let address = line.trim().to_string();
    assert!(address.len() < 108, "{address}");
    Bus {
        child,
        address,
        dir,
    }
}

pub async fn connect(bus: &Bus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bus_starts_and_two_clients_see_each_other() {
        let bus = private_bus();
        let a = connect(&bus).await;
        a.request_name("org.test.A").await.unwrap();
        let b = connect(&bus).await;
        let dbus = zbus::fdo::DBusProxy::new(&b).await.unwrap();
        let has = dbus
            .name_has_owner("org.test.A".try_into().unwrap())
            .await
            .unwrap();
        assert!(has);
        assert!(bus.dir().exists());
    }
}
