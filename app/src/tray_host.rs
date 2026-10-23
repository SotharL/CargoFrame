//! Is there anything on this desktop that can show a tray icon?
//!
//! Windows always has a notification area. On Linux the tray is a StatusNotifierItem host:
//! KDE provides one, GNOME needs the AppIndicator extension. Without one the icon is
//! invisible, so the app opens Settings at launch instead.

#[cfg(target_os = "linux")]
pub fn available() -> bool {
    use zbus::blocking::{fdo::DBusProxy, Connection};
    use zbus::names::BusName;

    // If the check itself cannot run, assume a tray exists rather than nag.
    let Ok(conn) = Connection::session() else { return true };
    let Ok(bus) = DBusProxy::new(&conn) else { return true };
    let Ok(name) = BusName::try_from("org.kde.StatusNotifierWatcher") else { return true };
    bus.name_has_owner(name).unwrap_or(true)
}

#[cfg(not(target_os = "linux"))]
pub fn available() -> bool {
    true
}
