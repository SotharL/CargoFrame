use crate::multipart::Multipart;
use crate::url::form_encode;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl Method {
    pub fn as_str(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum Body {
    #[default]
    None,
    Bytes {
        content_type: String,
        data: Vec<u8>,
    },
}

impl Body {
    pub fn json(value: &serde_json::Value) -> Self {
        Body::Bytes {
            content_type: "application/json".into(),
            data: serde_json::to_vec(value).unwrap_or_default(),
        }
    }

    pub fn form(fields: &[(&str, &str)]) -> Self {
        Body::Bytes {
            content_type: "application/x-www-form-urlencoded".into(),
            data: form_encode(fields).into_bytes(),
        }
    }

    pub fn multipart(m: Multipart) -> Self {
        let (content_type, data) = m.finish();
        Body::Bytes { content_type, data }
    }

    pub fn text(content_type: &str, text: impl Into<String>) -> Self {
        Body::Bytes {
            content_type: content_type.into(),
            data: text.into().into_bytes(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Request {
    pub method: Method,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Body,
    pub timeout: Duration,
    /// Follow redirects (default). Turn off when a header other than `Authorization` carries a
    /// secret, so it cannot be forwarded to another site.
    pub follow_redirects: bool,
}

impl Request {
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: Body::None,
            timeout: Duration::from_secs(30),
            follow_redirects: true,
        }
    }

    pub fn get(url: impl Into<String>) -> Self {
        Self::new(Method::Get, url)
    }

    pub fn post(url: impl Into<String>) -> Self {
        Self::new(Method::Post, url)
    }

    pub fn header(mut self, name: &str, value: impl Into<String>) -> Self {
        self.headers.push((name.to_string(), value.into()));
        self
    }

    pub fn body(mut self, body: Body) -> Self {
        self.body = body;
        self
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.timeout = t;
        self
    }

    /// Return a redirect answer as it is instead of following it.
    pub fn no_redirects(mut self) -> Self {
        self.follow_redirects = false;
        self
    }
}

#[derive(Debug, Clone)]
pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }

    pub fn json(&self) -> Option<serde_json::Value> {
        serde_json::from_slice(&self.body).ok()
    }

    /// `Retry-After` in seconds (the date form is not used by the services we talk to).
    pub fn retry_after(&self) -> Option<Duration> {
        self.header("retry-after")?
            .trim()
            .parse::<u64>()
            .ok()
            .map(Duration::from_secs)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NetError {
    /// DNS failure, refused or unreachable.
    #[error("cannot reach the server: {0}")]
    Offline(String),
    #[error("the server did not answer in time")]
    Timeout,
    #[error("secure connection failed: {0}")]
    Tls(String),
    #[error("{0}")]
    BadUrl(String),
    #[error("the answer is larger than {limit_mb} MB")]
    TooLarge { limit_mb: u64 },
    #[error("{0}")]
    Other(String),
}

/// How a download ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    pub status: u16,
    /// Bytes written to the destination (0 when the answer was not a success).
    pub bytes: u64,
    /// What the server said when it refused (a short text), empty on success.
    pub message: String,
}

impl Download {
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

/// Something that can send a request. The connectors only depend on this.
pub trait Http: Send + Sync {
    fn send(&self, request: &Request) -> Result<Response, NetError>;

    /// Fetch a large answer straight into `out`, reporting `progress(done, total)`. At most
    /// `limit` bytes are accepted. Clients that cannot do this say so.
    fn download(
        &self,
        _request: &Request,
        _out: &mut dyn std::io::Write,
        _limit: u64,
        _progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Download, NetError> {
        Err(NetError::Other("this client cannot download files".into()))
    }
}
