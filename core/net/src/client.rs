use crate::request::{Body, Http, NetError, Request, Response};
use crate::url::check_url;
use std::io::ErrorKind;
use std::time::Duration;

/// Longest answer read (a service answering with a huge page should not fill memory).
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;

/// The real HTTP client: HTTPS (plain HTTP only to this computer), a proxy from the usual
/// environment variables, redirects without forwarding sign-in headers, no cookies.
pub struct UreqHttp {
    agent: ureq::Agent,
}

impl Default for UreqHttp {
    fn default() -> Self {
        Self::new()
    }
}

impl UreqHttp {
    pub fn new() -> Self {
        let builder = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .max_redirects(5)
            .user_agent(format!("CargoFrame/{}", env!("CARGO_PKG_VERSION")))
            .timeout_connect(Some(Duration::from_secs(10)));
        #[cfg(windows)]
        let builder = builder.tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        );
        Self {
            agent: builder.build().into(),
        }
    }
}

fn map_error(e: ureq::Error) -> NetError {
    use ureq::Error as E;
    match e {
        E::Timeout(_) => NetError::Timeout,
        E::HostNotFound | E::ConnectionFailed => NetError::Offline(e.to_string()),
        E::BadUri(m) => NetError::BadUrl(m),
        E::BodyExceedsLimit(_) => NetError::TooLarge {
            limit_mb: MAX_RESPONSE_BYTES / (1024 * 1024),
        },
        E::Io(io) => match io.kind() {
            ErrorKind::TimedOut | ErrorKind::WouldBlock => NetError::Timeout,
            _ => {
                let text = io.to_string();
                let lower = text.to_lowercase();
                if lower.contains("certificate") || lower.contains("tls") {
                    NetError::Tls(text)
                } else {
                    NetError::Offline(text)
                }
            }
        },
        other => {
            let text = other.to_string();
            let lower = text.to_lowercase();
            if lower.contains("certificate") || lower.contains("tls") || lower.contains("handshake")
            {
                NetError::Tls(text)
            } else {
                NetError::Other(text)
            }
        }
    }
}

impl Http for UreqHttp {
    fn send(&self, request: &Request) -> Result<Response, NetError> {
        check_url(&request.url).map_err(|e| NetError::BadUrl(e.to_string()))?;
        let mut builder = ureq::http::Request::builder()
            .method(request.method.as_str())
            .uri(&request.url);
        for (k, v) in &request.headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let timeout = Some(request.timeout);
        let max_redirects = if request.follow_redirects { 5 } else { 0 };
        let response = match &request.body {
            Body::None => {
                let req = builder
                    .body(())
                    .map_err(|e| NetError::Other(format!("invalid request: {e}")))?;
                self.agent
                    .configure_request(req)
                    .timeout_global(timeout)
                    .max_redirects(max_redirects)
                    .max_redirects_will_error(false)
                    .build()
                    .pipe(|r| self.agent.run(r))
            }
            Body::Bytes { content_type, data } => {
                let req = builder
                    .header("Content-Type", content_type.as_str())
                    .body(data.as_slice())
                    .map_err(|e| NetError::Other(format!("invalid request: {e}")))?;
                self.agent
                    .configure_request(req)
                    .timeout_global(timeout)
                    .max_redirects(max_redirects)
                    .max_redirects_will_error(false)
                    .build()
                    .pipe(|r| self.agent.run(r))
            }
        }
        .map_err(map_error)?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
            .collect();
        let mut response = response;
        let body = response
            .body_mut()
            .with_config()
            .limit(MAX_RESPONSE_BYTES)
            .read_to_vec()
            .map_err(map_error)?;
        Ok(Response {
            status,
            headers,
            body,
        })
    }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}
