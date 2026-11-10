//! Shared helpers for the private-bus integration tests. Never touches the user's session bus.
#![allow(dead_code)]

use serde_json::{Value, json};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
pub use uq_core::test_bus::Bus;
use uq_core::test_bus::private_bus_with_services;

/// A private bus with one activatable service, `org.test.Ghost`, whose start leaves a `started`
/// file in the bus directory: tests use it to prove the bridge never auto-starts providers.
pub fn private_bus() -> Bus {
    let body = "[D-BUS Service]\nName=org.test.Ghost\nExec=/usr/bin/touch @DIR@/started\n";
    // The directory is only known once the bus exists, so the service file is written after;
    // dbus-daemon reads servicedir entries at activation time.
    let bus = private_bus_with_services(&[]);
    let body = body.replace("@DIR@", &bus.dir().display().to_string());
    std::fs::write(bus.dir().join("s").join("ghost.service"), body).unwrap();
    bus
}

pub struct Bridge {
    pub child: Child,
    pub stdin: Option<ChildStdin>,
    pub out: BufReader<ChildStdout>,
    /// Events that arrived while waiting for a reply; signals and replies race.
    pub events: std::collections::VecDeque<Value>,
}

impl Bridge {
    pub fn start(bus: &Bus) -> Bridge {
        Bridge::start_with_env(bus, &[])
    }

    pub fn start_with_env(bus: &Bus, env: &[(&str, &std::ffi::OsStr)]) -> Bridge {
        let mut child = Command::new(env!("CARGO_BIN_EXE_uqctl"))
            .arg("bridge")
            .envs(env.iter().copied())
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            // The private bus has no logind: the bridge never touches the real system bus.
            .env("DBUS_SYSTEM_BUS_ADDRESS", &bus.address)
            // No Wayland display: the bridge never reaches the user's compositor.
            .env_remove("WAYLAND_DISPLAY")
            .env_remove("WAYLAND_SOCKET")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Its expected "logind unavailable" and "lock-notify off" lines would clutter test output.
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .unwrap();
        let stdin = child.stdin.take();
        let out = BufReader::new(child.stdout.take().unwrap());
        Bridge {
            child,
            stdin,
            out,
            events: Default::default(),
        }
    }

    pub async fn next(&mut self) -> Value {
        let mut l = String::new();
        let n = tokio::time::timeout(Duration::from_secs(10), self.out.read_line(&mut l))
            .await
            .expect("bridge output within 10 s")
            .unwrap();
        assert!(n > 0, "bridge closed stdout");
        let v: Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v["v"], 1);
        v
    }

    pub async fn send(&mut self, id: u64, call: &str, args: Value) {
        let line = format!("{}\n", json!({"v":1,"id":id,"call":call,"args":args}));
        self.stdin
            .as_mut()
            .unwrap()
            .write_all(line.as_bytes())
            .await
            .unwrap();
    }

    /// Send a call and return its reply, skipping any events that arrive first.
    pub async fn call(&mut self, id: u64, call: &str, args: Value) -> Value {
        self.send(id, call, args).await;
        self.reply(id).await
    }

    /// Wait for the reply with this id, queuing any events that arrive first.
    pub async fn reply(&mut self, id: u64) -> Value {
        loop {
            let v = self.next().await;
            if v["id"] == id {
                return v;
            }
            self.events.push_back(v);
        }
    }

    /// Skip until an event with this name arrives.
    pub async fn event(&mut self, name: &str) -> Value {
        loop {
            let v = match self.events.pop_front() {
                Some(v) => v,
                None => self.next().await,
            };
            if v["event"] == name {
                return v["data"].clone();
            }
        }
    }
}
