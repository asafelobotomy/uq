//! `Power`: the closed set of session power actions (spec §2.8).
//!
//! The shell sends one of five action names; each maps to a fixed argv run directly, never
//! through a shell. Anything else is refused.

use serde_json::Value;
use uqd::compositor::CompositorError;
use uqd::hypr::HyprSocket;

/// What an action runs: a fixed argv, or Hyprland's own exit through its socket.
#[derive(Debug, PartialEq, Eq)]
pub enum PowerCmd {
    Argv(Vec<String>),
    HyprExit,
}

const HYPR_EXIT: &str = "return hl.dispatch(hl.dsp.exit())";

fn argv(parts: &[&str]) -> PowerCmd {
    PowerCmd::Argv(parts.iter().map(|s| (*s).to_string()).collect())
}

/// Map an action to its command. `has_bin` answers whether a program is on `PATH`.
fn argv_with(action: &str, has_bin: impl Fn(&str) -> bool) -> Result<PowerCmd, String> {
    match action {
        "lock" => Ok(argv(&["loginctl", "lock-session"])),
        "suspend" => Ok(argv(&["systemctl", "suspend"])),
        "reboot" => Ok(argv(&["systemctl", "reboot"])),
        "poweroff" => Ok(argv(&["systemctl", "poweroff"])),
        "logout" if has_bin("hyprshutdown") => Ok(argv(&["hyprshutdown"])),
        "logout" => Ok(PowerCmd::HyprExit),
        _ => Err("unknown power action".into()),
    }
}

fn on_path(name: &str) -> bool {
    std::env::var_os("PATH")
        .is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(name).is_file()))
}

pub fn argv_for(action: &str) -> Result<PowerCmd, String> {
    argv_with(action, on_path)
}

/// The bridge's `Power` call.
pub async fn power(args: &[Value]) -> Result<Value, String> {
    let [Value::String(action)] = args else {
        return Err("Power takes one string argument (the action)".into());
    };
    match argv_for(action)? {
        PowerCmd::Argv(argv) => {
            let (prog, rest) = argv.split_first().ok_or("empty power command")?;
            let status = tokio::process::Command::new(prog)
                .args(rest)
                .stdin(std::process::Stdio::null())
                .status()
                .await
                .map_err(|e| format!("{prog}: {e}"))?;
            if status.success() {
                Ok(Value::Null)
            } else {
                Err(format!("{prog} failed: {status}"))
            }
        }
        PowerCmd::HyprExit => match HyprSocket::from_env().eval(HYPR_EXIT).await {
            Ok(()) => Ok(Value::Null),
            Err(CompositorError::NotRunning) => Err("Hyprland is not running".into()),
            Err(CompositorError::Rejected(msg)) => Err(msg),
            Err(e) => Err(e.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(a: &[&str]) -> PowerCmd {
        argv(a)
    }

    #[test]
    fn argv_table() {
        let none = |_: &str| false;
        assert_eq!(
            argv_with("lock", none).unwrap(),
            v(&["loginctl", "lock-session"])
        );
        assert_eq!(
            argv_with("suspend", none).unwrap(),
            v(&["systemctl", "suspend"])
        );
        assert_eq!(
            argv_with("reboot", none).unwrap(),
            v(&["systemctl", "reboot"])
        );
        assert_eq!(
            argv_with("poweroff", none).unwrap(),
            v(&["systemctl", "poweroff"])
        );
    }

    #[test]
    fn unknown_rejected() {
        for bad in ["halt", "lock; rm", "", "LOCK", "lock "] {
            assert_eq!(
                argv_with(bad, |_| true),
                Err("unknown power action".to_string()),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn logout_prefers_hyprshutdown() {
        assert_eq!(
            argv_with("logout", |n| n == "hyprshutdown").unwrap(),
            v(&["hyprshutdown"])
        );
        assert_eq!(argv_with("logout", |_| false).unwrap(), PowerCmd::HyprExit);
    }

    #[tokio::test]
    async fn bridge_power_rejects_unknown_and_bad_args() {
        assert_eq!(
            power(&[Value::String("halt".into())]).await,
            Err("unknown power action".to_string())
        );
        assert!(power(&[]).await.is_err());
    }
}
