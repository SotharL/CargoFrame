//! Shared helpers for the connector tests.

use crate::ctx::{Browser, Ctx};
use crate::payload::Progress;
use crate::secrets::{MemorySecrets, SecretStore};
use cargoframe_deliver::{Deliver, DeliverError, Delivered, Delivery};
use cargoframe_net::UreqHttp;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub struct NoBrowser;

impl Browser for NoBrowser {
    fn open(&self, _url: &str) -> Result<(), String> {
        Err("no browser in this test".into())
    }
}

/// A desktop that remembers what it was asked to deliver.
#[derive(Default)]
pub struct FakeDeliver {
    pub deliveries: Mutex<Vec<Delivery>>,
    /// Make the next delivery fail with this.
    pub fail_with: Mutex<Option<DeliverError>>,
}

impl Deliver for FakeDeliver {
    fn deliver(
        &self,
        d: &Delivery,
        open: &dyn Fn(&str) -> Result<(), String>,
        _cancelled: &dyn Fn() -> bool,
    ) -> Result<Delivered, DeliverError> {
        self.deliveries.lock().unwrap().push(d.clone());
        // Like the real one, the link is opened first.
        open(&d.open_url).map_err(DeliverError::Open)?;
        if let Some(e) = self.fail_with.lock().unwrap().take() {
            return Err(e);
        }
        Ok(Delivered {
            window: "Claude".into(),
            files: d.files.len(),
            skipped_files: 0,
            text: !d.text.trim().is_empty(),
            submitted: d.submit,
        })
    }
}

/// Plays a person who signs in: opens the address and "returns" to the redirect address.
pub struct ApproveBrowser {
    pub opened: Mutex<Vec<String>>,
}

impl ApproveBrowser {
    pub fn new() -> Self {
        Self {
            opened: Mutex::new(Vec::new()),
        }
    }
}

impl Browser for ApproveBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        self.opened.lock().unwrap().push(url.to_string());
        let pairs = crate::oauth::query_pairs(url.split_once('?').map(|(_, q)| q).unwrap_or(""));
        let get = |k: &str| pairs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
        let (redirect, state) = (get("redirect_uri").unwrap(), get("state").unwrap());
        std::thread::spawn(move || {
            use cargoframe_net::{Http, Request};
            let _ = UreqHttp::new().send(&Request::get(format!(
                "{redirect}?code=THE_CODE&state={state}"
            )));
        });
        Ok(())
    }
}

pub struct World {
    pub http: UreqHttp,
    pub secrets: MemorySecrets,
    pub progress: Progress,
    pub clock: Arc<Mutex<u64>>,
    pub timeout: Duration,
}

impl World {
    pub fn new() -> Self {
        Self {
            http: UreqHttp::new(),
            secrets: MemorySecrets::new(),
            progress: Progress::new(),
            clock: Arc::new(Mutex::new(1_000_000)),
            timeout: Duration::from_secs(5),
        }
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    pub fn secret(self, connection: &str, name: &str, value: &str) -> Self {
        self.secrets
            .set(&format!("{connection}/{name}"), value)
            .unwrap();
        self
    }

    pub fn with_browser<R>(
        &self,
        connection: &str,
        browser: &dyn Browser,
        f: impl FnOnce(&Ctx) -> R,
    ) -> R {
        self.with_all(connection, browser, &FakeDeliver::default(), f)
    }

    pub fn with_all<R>(
        &self,
        connection: &str,
        browser: &dyn Browser,
        deliver: &dyn Deliver,
        f: impl FnOnce(&Ctx) -> R,
    ) -> R {
        let clock = self.clock.clone();
        let now = move || *clock.lock().unwrap();
        let cancelled = || false;
        let ctx = Ctx {
            connection_id: connection,
            http: &self.http,
            secrets: &self.secrets,
            browser,
            deliver,
            progress: &self.progress,
            timeout: self.timeout,
            now: &now,
            cancelled: &cancelled,
        };
        f(&ctx)
    }

    pub fn with<R>(&self, connection: &str, f: impl FnOnce(&Ctx) -> R) -> R {
        self.with_browser(connection, &NoBrowser, f)
    }
}
