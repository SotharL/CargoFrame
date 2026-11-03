use crate::request::{Body, Download, Http, Method, NetError, Request, Response};
use crate::url::{check_url, redirect_target, same_origin};
use std::io::{ErrorKind, Read, Write};
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

/// Redirects a request may follow before the last answer is returned as it is.
const MAX_REDIRECTS: usize = 5;

/// Headers that carry a sign-in; they follow a redirect only to the same site.
const SIGN_IN_HEADERS: [&str; 3] = ["authorization", "proxy-authorization", "cookie"];

fn is_redirect(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

impl UreqHttp {
    /// One request, one answer: redirects are followed by `send` and `download`, which check
    /// every hop. The body is not read yet.
    fn call(
        &self,
        method: Method,
        url: &str,
        headers: &[(String, String)],
        body: &Body,
        timeout: Duration,
    ) -> Result<ureq::http::Response<ureq::Body>, NetError> {
        let mut builder = ureq::http::Request::builder()
            .method(method.as_str())
            .uri(url);
        for (k, v) in headers {
            builder = builder.header(k.as_str(), v.as_str());
        }
        let timeout = Some(timeout);
        match body {
            Body::None => {
                let req = builder
                    .body(())
                    .map_err(|e| NetError::Other(format!("invalid request: {e}")))?;
                self.agent
                    .configure_request(req)
                    .timeout_global(timeout)
                    .max_redirects(0)
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
                    .max_redirects(0)
                    .max_redirects_will_error(false)
                    .build()
                    .pipe(|r| self.agent.run(r))
            }
        }
        .map_err(map_error)
    }

    fn once(
        &self,
        method: Method,
        url: &str,
        headers: &[(String, String)],
        body: &Body,
        timeout: Duration,
    ) -> Result<Response, NetError> {
        let mut response = self.call(method, url, headers, body, timeout)?;
        let status = response.status().as_u16();
        let headers = header_list(&response);
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

fn header_list(response: &ureq::http::Response<ureq::Body>) -> Vec<(String, String)> {
    response
        .headers()
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("").to_string()))
        .collect()
}

/// The request that follows a redirect answer: checks the target, keeps a sign-in header only
/// for the same site, and turns a redirected POST into a GET as a browser does.
fn next_hop(
    url: &str,
    status: u16,
    location: &str,
    method: &mut Method,
    headers: &mut Vec<(String, String)>,
    body: &mut Body,
) -> Result<String, NetError> {
    let next = redirect_target(url, location)
        .map_err(|e| NetError::BadUrl(format!("refused a redirect: {e}")))?;
    if !same_origin(url, &next) {
        headers.retain(|(k, _)| !SIGN_IN_HEADERS.contains(&k.to_ascii_lowercase().as_str()));
    }
    let to_get = status == 303 || (matches!(status, 301 | 302) && *method == Method::Post);
    if to_get && *method != Method::Get {
        *method = Method::Get;
        *body = Body::None;
    }
    Ok(next)
}

impl Http for UreqHttp {
    /// Sends the request and follows redirects by hand, so every hop is checked: it must be
    /// https (plain http only between this computer's own servers), it may not carry a
    /// user name, and a sign-in header never follows the request to another site. A POST that is
    /// redirected with 301, 302 or 303 becomes a GET without a body, as in a browser.
    fn send(&self, request: &Request) -> Result<Response, NetError> {
        check_url(&request.url).map_err(|e| NetError::BadUrl(e.to_string()))?;
        let mut url = request.url.clone();
        let mut method = request.method;
        let mut headers = request.headers.clone();
        let mut body = request.body.clone();
        let mut hops = 0;
        loop {
            let response = self.once(method, &url, &headers, &body, request.timeout)?;
            if !request.follow_redirects || !is_redirect(response.status) || hops >= MAX_REDIRECTS {
                return Ok(response);
            }
            let Some(location) = response.header("location") else {
                return Ok(response);
            };
            url = next_hop(
                &url,
                response.status,
                location,
                &mut method,
                &mut headers,
                &mut body,
            )?;
            hops += 1;
        }
    }

    /// The same redirect rules as `send`, but the answer body is copied to `out` as it arrives
    /// (never held in memory), up to `limit` bytes, and `progress(done, total)` is told as it goes.
    /// An answer that is not a success is not written: its status and a short text come back.
    fn download(
        &self,
        request: &Request,
        out: &mut dyn Write,
        limit: u64,
        progress: &mut dyn FnMut(u64, Option<u64>),
    ) -> Result<Download, NetError> {
        check_url(&request.url).map_err(|e| NetError::BadUrl(e.to_string()))?;
        let mut url = request.url.clone();
        let mut method = request.method;
        let mut headers = request.headers.clone();
        let mut body = request.body.clone();
        let mut hops = 0;
        loop {
            let mut response = self.call(method, &url, &headers, &body, request.timeout)?;
            let status = response.status().as_u16();
            if is_redirect(status) && hops < MAX_REDIRECTS {
                let location = response
                    .headers()
                    .get("location")
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_string);
                if let Some(location) = location {
                    url = next_hop(
                        &url,
                        status,
                        &location,
                        &mut method,
                        &mut headers,
                        &mut body,
                    )?;
                    hops += 1;
                    continue;
                }
            }
            if !(200..300).contains(&status) {
                let text = response
                    .body_mut()
                    .with_config()
                    .limit(16 * 1024)
                    .read_to_vec()
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
                    .unwrap_or_default();
                return Ok(Download {
                    status,
                    bytes: 0,
                    message: text,
                });
            }
            let total = response
                .headers()
                .get("content-length")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok());
            if total.is_some_and(|t| t > limit) {
                return Err(NetError::Other(format!(
                    "the download is larger than {} MB",
                    limit / (1024 * 1024)
                )));
            }
            let mut reader = response.body_mut().as_reader();
            let mut buf = vec![0u8; 64 * 1024];
            let mut done: u64 = 0;
            loop {
                let n = reader
                    .read(&mut buf)
                    .map_err(|e| map_error(ureq::Error::Io(e)))?;
                if n == 0 {
                    break;
                }
                done += n as u64;
                if done > limit {
                    return Err(NetError::Other(format!(
                        "the download is larger than {} MB",
                        limit / (1024 * 1024)
                    )));
                }
                out.write_all(&buf[..n])
                    .map_err(|e| NetError::Other(format!("cannot write the download: {e}")))?;
                progress(done, total);
            }
            if total.is_some_and(|t| t != done) {
                return Err(NetError::Other(
                    "the connection closed before the download was complete".into(),
                ));
            }
            return Ok(Download {
                status,
                bytes: done,
                message: String::new(),
            });
        }
    }
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}
impl<T> Pipe for T {}
