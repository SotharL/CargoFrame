//! sway and Hyprland: shortcuts set live over the compositor's IPC socket. They last while the
//! compositor runs and are set again each time CargoFrame starts; the config files are not
//! touched. The keys bound last time are unbound before the new ones are bound.

use crate::Binding;
use cargoframe_capture::wayland::{hypr_request, sway_request, SWAY_RUN_COMMAND};
use cargoframe_config::shortcut;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;

/// What was bound, so the next change can unbind it.
pub type Bound = Vec<String>;

/// sway's answer to a command: `[{"success": true}]` or an error per command.
fn sway_ok(answer: &Value) -> Result<(), String> {
    match answer.as_array().and_then(|a| a.first()) {
        Some(first) if first.get("success").and_then(Value::as_bool) == Some(true) => Ok(()),
        Some(first) => Err(first
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("sway refused it")
            .to_string()),
        None => Err("sway gave no answer".into()),
    }
}

/// The sway command that binds one shortcut.
pub fn sway_bind(keys: &str, command: &str) -> String {
    format!("bindsym --no-warn {keys} exec {command}")
}

/// Bind `wanted` (each with its command line) in sway, after unbinding `previous`.
pub fn sway_apply(
    socket: &Path,
    previous: &[String],
    wanted: &[(Binding, String)],
) -> (Bound, BTreeMap<String, String>) {
    let run = |cmd: &str| -> Result<(), String> {
        sway_request(socket, SWAY_RUN_COMMAND, cmd)
            .map_err(|e| format!("sway did not answer: {e}"))
            .and_then(|a| sway_ok(&a))
    };
    for keys in previous {
        if let Err(e) = run(&format!("unbindsym {keys}")) {
            tracing::debug!(%keys, "sway did not unbind: {e}");
        }
    }
    let mut bound = Vec::new();
    let mut errors = BTreeMap::new();
    for (b, command) in wanted {
        let keys = shortcut::for_sway(&b.shortcut);
        match run(&sway_bind(&keys, command)) {
            Ok(()) => bound.push(keys),
            Err(e) => {
                errors.insert(
                    b.id.clone(),
                    format!("sway did not take {}: {e}", b.shortcut),
                );
            }
        }
    }
    (bound, errors)
}

/// The Hyprland requests that bind and unbind one shortcut.
pub fn hypr_bind(keys: &str, command: &str) -> String {
    format!("keyword bind {keys}, exec, {command}")
}

pub fn hypr_unbind(keys: &str) -> String {
    format!("keyword unbind {keys}")
}

/// Bind `wanted` in Hyprland, after unbinding `previous`.
pub fn hypr_apply(
    socket: &Path,
    previous: &[String],
    wanted: &[(Binding, String)],
) -> (Bound, BTreeMap<String, String>) {
    let run = |cmd: &str| -> Result<(), String> {
        let answer =
            hypr_request(socket, cmd).map_err(|e| format!("Hyprland did not answer: {e}"))?;
        let text = String::from_utf8_lossy(&answer);
        if text.trim() == "ok" {
            Ok(())
        } else {
            Err(text.trim().to_string())
        }
    };
    for keys in previous {
        if let Err(e) = run(&hypr_unbind(keys)) {
            tracing::debug!(%keys, "Hyprland did not unbind: {e}");
        }
    }
    let mut bound = Vec::new();
    let mut errors = BTreeMap::new();
    for (b, command) in wanted {
        let keys = shortcut::for_hyprland(&b.shortcut);
        match run(&hypr_bind(&keys, command)) {
            Ok(()) => bound.push(keys),
            Err(e) => {
                errors.insert(
                    b.id.clone(),
                    format!("Hyprland did not take {}: {e}", b.shortcut),
                );
            }
        }
    }
    (bound, errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::{Arc, Mutex};

    fn binding(id: &str, shortcut: &str) -> (Binding, String) {
        (
            Binding {
                id: id.into(),
                name: id.into(),
                shortcut: shortcut.into(),
            },
            format!("/opt/cargoframe open --trigger {id}"),
        )
    }

    /// A fake sway that records each command and refuses the ones with "F12".
    fn fake_sway(dir: &Path, n: usize) -> (std::path::PathBuf, Arc<Mutex<Vec<String>>>) {
        let path = dir.join("sway.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for _ in 0..n {
                let (mut s, _) = listener.accept().unwrap();
                let mut head = [0u8; 14];
                s.read_exact(&mut head).unwrap();
                let len = u32::from_ne_bytes([head[6], head[7], head[8], head[9]]) as usize;
                let mut body = vec![0u8; len];
                s.read_exact(&mut body).unwrap();
                let cmd = String::from_utf8(body).unwrap();
                let answer = if cmd.contains("F12") {
                    r#"[{"success":false,"error":"Invalid key"}]"#.to_string()
                } else {
                    r#"[{"success":true}]"#.to_string()
                };
                log.lock().unwrap().push(cmd);
                let mut msg = b"i3-ipc".to_vec();
                msg.extend_from_slice(&(answer.len() as u32).to_ne_bytes());
                msg.extend_from_slice(&head[10..14]);
                msg.extend_from_slice(answer.as_bytes());
                s.write_all(&msg).unwrap();
            }
        });
        (path, seen)
    }

    #[test]
    fn sway_unbinds_the_old_keys_then_binds_the_new_ones() {
        let dir = tempfile::tempdir().unwrap();
        let (sock, seen) = fake_sway(dir.path(), 3);
        let (bound, errors) = sway_apply(
            &sock,
            &["Ctrl+Shift+F8".to_string()],
            &[binding("bug", "Ctrl+Shift+F9"), binding("x", "F12")],
        );
        assert_eq!(bound, vec!["Ctrl+Shift+F9".to_string()]);
        assert!(errors["x"].contains("Invalid key"), "{errors:?}");
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0], "unbindsym Ctrl+Shift+F8");
        assert_eq!(
            seen[1],
            "bindsym --no-warn Ctrl+Shift+F9 exec /opt/cargoframe open --trigger bug"
        );
    }

    #[test]
    fn a_missing_sway_says_so_for_each_trigger() {
        let (bound, errors) = sway_apply(
            Path::new("/nonexistent/sway.sock"),
            &[],
            &[binding("bug", "Ctrl+Shift+F9")],
        );
        assert!(bound.is_empty());
        assert!(errors["bug"].contains("sway did not answer"), "{errors:?}");
    }

    #[test]
    fn hyprland_gets_bind_and_unbind_keywords() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(".socket.sock");
        let listener = UnixListener::bind(&path).unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        std::thread::spawn(move || {
            for _ in 0..2 {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = [0u8; 512];
                let n = s.read(&mut buf).unwrap();
                log.lock()
                    .unwrap()
                    .push(String::from_utf8_lossy(&buf[..n]).to_string());
                s.write_all(b"ok").unwrap();
            }
        });
        let (bound, errors) = hypr_apply(
            &path,
            &["CTRL, F8".to_string()],
            &[binding("bug", "Ctrl+Shift+F9")],
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(bound, vec!["CTRL SHIFT, F9".to_string()]);
        let seen = seen.lock().unwrap();
        assert_eq!(seen[0], "keyword unbind CTRL, F8");
        assert_eq!(
            seen[1],
            "keyword bind CTRL SHIFT, F9, exec, /opt/cargoframe open --trigger bug"
        );
    }
}
