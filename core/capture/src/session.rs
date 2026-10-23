//! Which kind of desktop session this is. Only Linux has a choice to make.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    X11,
    Wayland,
    Other,
}

/// Decide from the environment (`get` reads one variable). Wayland sessions usually also set
/// `DISPLAY` for XWayland programs, so `XDG_SESSION_TYPE` wins.
pub fn detect(get: &dyn Fn(&str) -> Option<String>) -> Session {
    let non_empty = |name: &str| get(name).filter(|v| !v.is_empty());
    match non_empty("XDG_SESSION_TYPE").as_deref() {
        Some("wayland") => return Session::Wayland,
        Some("x11") => return Session::X11,
        _ => {}
    }
    if non_empty("WAYLAND_DISPLAY").is_some() {
        Session::Wayland
    } else if non_empty("DISPLAY").is_some() {
        Session::X11
    } else {
        Session::Other
    }
}

pub fn detect_env() -> Session {
    detect(&|k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> Session {
        let m: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        detect(&|k| m.get(k).cloned())
    }

    #[test]
    fn session_type_wins_over_display() {
        assert_eq!(
            env(&[("XDG_SESSION_TYPE", "wayland"), ("DISPLAY", ":0")]),
            Session::Wayland
        );
        assert_eq!(
            env(&[
                ("XDG_SESSION_TYPE", "x11"),
                ("WAYLAND_DISPLAY", "wayland-0")
            ]),
            Session::X11
        );
    }

    #[test]
    fn falls_back_to_the_display_variables() {
        assert_eq!(env(&[("WAYLAND_DISPLAY", "wayland-0")]), Session::Wayland);
        assert_eq!(env(&[("DISPLAY", ":99")]), Session::X11);
        assert_eq!(env(&[("DISPLAY", "")]), Session::Other);
        assert_eq!(env(&[]), Session::Other);
    }
}
