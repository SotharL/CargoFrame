//! A stand-in for a desktop's StatusNotifierWatcher, so a test session counts as having a tray.
//! Without one CargoFrame opens Settings at launch (there would be no icon to find it by); with
//! one it stays tray-only, which is what the idle memory and CPU checks need to measure.
//!
//! Prints `ready` once the name is owned, then waits to be killed.

#[cfg(target_os = "linux")]
fn main() -> zbus::Result<()> {
    use zbus::{blocking::connection, interface};

    struct Watcher;

    #[interface(name = "org.kde.StatusNotifierWatcher")]
    impl Watcher {
        fn register_status_notifier_item(&self, _service: &str) {}
        fn register_status_notifier_host(&self, _service: &str) {}
        #[zbus(property)]
        fn is_status_notifier_host_registered(&self) -> bool {
            true
        }
        #[zbus(property)]
        fn protocol_version(&self) -> i32 {
            0
        }
        #[zbus(property)]
        fn registered_status_notifier_items(&self) -> Vec<String> {
            Vec::new()
        }
    }

    let _connection = connection::Builder::session()?
        .name("org.kde.StatusNotifierWatcher")?
        .serve_at("/StatusNotifierWatcher", Watcher)?
        .build()?;
    println!("ready");
    loop {
        std::thread::park();
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}
