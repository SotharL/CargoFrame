//! Why a share failed, in the terms the settings page and the tray badge use.

use std::time::Duration;

/// Failure classes from the plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    /// No network or DNS failure.
    NoNetwork,
    /// The service did not answer in time (default limit 30 s) or answered 408.
    Timeout,
    /// 401 or 403: the sign-in expired or was revoked. The tray offers "Reconnect".
    AuthExpired,
    /// Any other 4xx: the service refused the request and retrying will not help.
    Rejected(u16),
    /// 429: too many requests.
    Throttled(u16),
    /// 5xx: the service is failing.
    Server(u16),
    /// The connection is disabled or not set up.
    ConnectorUnavailable,
    /// The server's certificate is not trusted. Retrying cannot fix it.
    Certificate,
}

impl FailureKind {
    /// Map an HTTP status to a failure. Returns `None` for 1xx, 2xx and 3xx.
    pub fn from_status(status: u16) -> Option<Self> {
        match status {
            401 | 403 => Some(Self::AuthExpired),
            408 => Some(Self::Timeout),
            429 => Some(Self::Throttled(status)),
            400..=499 => Some(Self::Rejected(status)),
            500..=599 => Some(Self::Server(status)),
            _ => None,
        }
    }

    /// Worth another attempt?
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::NoNetwork | Self::Timeout | Self::Throttled(_) | Self::Server(_)
        )
    }

    /// Should the interface offer "Reconnect"?
    pub fn needs_reconnect(&self) -> bool {
        matches!(self, Self::AuthExpired)
    }

    /// Short text for the status chip. The chip always pairs it with an icon.
    pub fn label(&self) -> &'static str {
        match self {
            Self::NoNetwork => "No network",
            Self::Timeout => "Timed out",
            Self::AuthExpired => "Sign-in expired",
            Self::Rejected(_) => "Rejected by the service",
            Self::Throttled(_) => "Too many requests",
            Self::Server(_) => "Service error",
            Self::ConnectorUnavailable => "Connection not set up",
            Self::Certificate => "Certificate not trusted",
        }
    }
}

/// An error from one send attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendError {
    pub kind: FailureKind,
    /// Message for the log. Callers must not put secrets in it; `error.json` in the zip
    /// reuses it.
    pub message: String,
    /// Value of a `Retry-After` header, when the service sent one.
    pub retry_after: Option<Duration>,
}

impl SendError {
    pub fn new(kind: FailureKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            retry_after: None,
        }
    }

    pub fn with_retry_after(mut self, after: Duration) -> Self {
        self.retry_after = Some(after);
        self
    }

    pub fn from_status(status: u16, message: impl Into<String>) -> Option<Self> {
        FailureKind::from_status(status).map(|kind| Self::new(kind, message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(FailureKind::from_status(200), None);
        assert_eq!(FailureKind::from_status(302), None);
        assert_eq!(
            FailureKind::from_status(401),
            Some(FailureKind::AuthExpired)
        );
        assert_eq!(
            FailureKind::from_status(403),
            Some(FailureKind::AuthExpired)
        );
        assert_eq!(FailureKind::from_status(408), Some(FailureKind::Timeout));
        assert_eq!(
            FailureKind::from_status(429),
            Some(FailureKind::Throttled(429))
        );
        assert_eq!(
            FailureKind::from_status(404),
            Some(FailureKind::Rejected(404))
        );
        assert_eq!(
            FailureKind::from_status(422),
            Some(FailureKind::Rejected(422))
        );
        assert_eq!(
            FailureKind::from_status(500),
            Some(FailureKind::Server(500))
        );
        assert_eq!(
            FailureKind::from_status(503),
            Some(FailureKind::Server(503))
        );
    }

    #[test]
    fn retry_rules() {
        assert!(FailureKind::NoNetwork.is_retryable());
        assert!(FailureKind::Timeout.is_retryable());
        assert!(FailureKind::Throttled(429).is_retryable());
        assert!(FailureKind::Server(502).is_retryable());
        assert!(!FailureKind::AuthExpired.is_retryable());
        assert!(!FailureKind::Rejected(400).is_retryable());
        assert!(!FailureKind::ConnectorUnavailable.is_retryable());
    }

    #[test]
    fn an_untrusted_certificate_is_not_retried() {
        assert!(!FailureKind::Certificate.is_retryable());
        assert!(!FailureKind::Certificate.needs_reconnect());
        assert_eq!(FailureKind::Certificate.label(), "Certificate not trusted");
    }

    #[test]
    fn only_expired_sign_in_offers_reconnect() {
        assert!(FailureKind::AuthExpired.needs_reconnect());
        assert!(!FailureKind::Rejected(404).needs_reconnect());
    }
}
