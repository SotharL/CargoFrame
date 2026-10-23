//! The operating system's credential store: Windows Credential Manager, or the Secret Service
//! (GNOME Keyring, KDE Wallet) on Linux. This is the only place passwords, tokens and keys are
//! kept; they are never written to a config file, a log or a zip.

use cargoframe_connectors::{SecretError, SecretStore};

const SERVICE: &str = "CargoFrame";

pub struct KeyringStore;

fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(SERVICE, key).map_err(map)
}

fn map(e: keyring::Error) -> SecretError {
    match e {
        // The service itself is missing or locked out: say so, so the window can explain it.
        keyring::Error::NoStorageAccess(inner) => SecretError::Unavailable(inner.to_string()),
        keyring::Error::PlatformFailure(inner) => SecretError::Unavailable(inner.to_string()),
        keyring::Error::TooLong(what, max) => {
            SecretError::Failed(format!("the {what} is too long (at most {max})"))
        }
        other => SecretError::Failed(other.to_string()),
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        match entry(key)?.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map(e)),
        }
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        entry(key)?.set_password(value).map_err(map)
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map(e)),
        }
    }
}
