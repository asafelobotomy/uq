//! A private dbus-daemon for tests.

use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};

pub struct Bus {
    child: Child,
    pub address: String,
    _dir: tempfile::TempDir,
}

impl Drop for Bus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A private dbus-daemon; the socket path stays under 108 bytes.
pub fn private_bus() -> Bus {
    let dir = tempfile::Builder::new()
        .prefix("uqs")
        .tempdir_in("/tmp")
        .unwrap();
    let conf = dir.path().join("c.conf");
    std::fs::write(
        &conf,
        format!(
            "<busconfig><type>session</type><listen>unix:path={}</listen>\
             <policy context=\"default\"><allow send_destination=\"*\" eavesdrop=\"true\"/>\
             <allow eavesdrop=\"true\"/><allow own=\"*\"/></policy></busconfig>",
            dir.path().join("b").display()
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
        _dir: dir,
    }
}

pub async fn connect(bus: &Bus) -> zbus::Connection {
    zbus::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .await
        .unwrap()
}
