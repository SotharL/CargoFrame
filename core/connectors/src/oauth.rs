//! OAuth 2 sign-in for desktop apps: the browser opens the service's page, the service sends the
//! person back to a one-time server on this computer (`http://127.0.0.1:<port>/callback`), and
//! the code is swapped for tokens. Used by Reddit and Google.
//!
//! * `state` is checked, so a callback nobody asked for is refused.
//! * PKCE (S256) is used where the service supports it.
//! * Tokens are kept in the credential store as one JSON value; the access token is renewed
//!   with the refresh token when it is about to expire.

use crate::ctx::Ctx;
use crate::error::{from_net, from_response, invalid, not_set_up};
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use cargoframe_net::{encode_component, Body, Request};
use cargoframe_safe::{FailureKind, SendError};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

const SECRET_NAME: &str = "oauth";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientAuth {
    /// `Authorization: Basic base64(client_id:client_secret)` (Reddit; the secret is empty).
    Basic,
    /// `client_id` and `client_secret` in the form body (Google).
    Body,
}

#[derive(Debug, Clone)]
pub struct OAuthClient {
    pub auth_url: String,
    pub token_url: String,
    pub revoke_url: Option<String>,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    pub extra_auth_params: Vec<(String, String)>,
    pub pkce: bool,
    pub client_auth: ClientAuth,
    /// The port the service's app registration allows (Reddit); `None` picks a free one.
    pub redirect_port: Option<u16>,
    /// How long to wait for the person to finish signing in.
    pub wait: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Seconds since 1970.
    pub expires_at: u64,
}

fn random_hex(bytes: usize) -> String {
    let mut buf = vec![0u8; bytes];
    if getrandom::fill(&mut buf).is_err() {
        // Without randomness a sign-in must not proceed: the state would be guessable.
        return String::new();
    }
    buf.iter().map(|b| format!("{b:02x}")).collect()
}

/// PKCE: a random verifier and its S256 challenge.
pub fn pkce_pair() -> Option<(String, String)> {
    let mut raw = [0u8; 32];
    getrandom::fill(&mut raw).ok()?;
    let verifier = URL_SAFE_NO_PAD.encode(raw);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    Some((verifier, challenge))
}

fn request_with_client(
    client: &OAuthClient,
    url: &str,
    mut fields: Vec<(&str, String)>,
    ctx: &Ctx,
) -> Request {
    let mut req = Request::post(url).timeout(ctx.timeout);
    match client.client_auth {
        ClientAuth::Basic => {
            let secret = client.client_secret.as_deref().unwrap_or("");
            let creds = STANDARD.encode(format!("{}:{}", client.client_id, secret));
            req = req.header("Authorization", format!("Basic {creds}"));
        }
        ClientAuth::Body => {
            fields.push(("client_id", client.client_id.clone()));
            if let Some(s) = &client.client_secret {
                if !s.is_empty() {
                    fields.push(("client_secret", s.clone()));
                }
            }
        }
    }
    let pairs: Vec<(&str, &str)> = fields.iter().map(|(k, v)| (*k, v.as_str())).collect();
    req.header("Accept", "application/json")
        .body(Body::form(&pairs))
}

fn parse_tokens(
    resp: &cargoframe_net::Response,
    now: u64,
    previous_refresh: Option<&str>,
) -> Result<Tokens, SendError> {
    let json = resp
        .json()
        .ok_or_else(|| invalid("The service answered the sign-in with something unreadable"))?;
    if let Some(err) = json.get("error").and_then(|e| e.as_str()) {
        let detail = json
            .get("error_description")
            .and_then(|d| d.as_str())
            .map(|d| format!(": {d}"))
            .unwrap_or_default();
        // A refused code or an expired refresh token means: sign in again.
        let kind = if matches!(
            err,
            "invalid_grant" | "invalid_client" | "unauthorized_client"
        ) {
            FailureKind::AuthExpired
        } else {
            FailureKind::Rejected(resp.status)
        };
        return Err(SendError::new(
            kind,
            format!("Sign-in refused ({err}{detail})"),
        ));
    }
    let access = json
        .get("access_token")
        .and_then(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .ok_or_else(|| invalid("The service did not return an access token"))?;
    let refresh = json
        .get("refresh_token")
        .and_then(|t| t.as_str())
        .map(String::from)
        .or_else(|| previous_refresh.map(String::from));
    let lifetime = json
        .get("expires_in")
        .and_then(|e| e.as_u64())
        .unwrap_or(3600);
    Ok(Tokens {
        access_token: access.to_string(),
        refresh_token: refresh,
        expires_at: now + lifetime,
    })
}

fn store_tokens(ctx: &Ctx, tokens: &Tokens) -> Result<(), SendError> {
    let json = serde_json::to_string(tokens).map_err(|e| invalid(e.to_string()))?;
    ctx.set_secret(SECRET_NAME, &json)
}

pub fn load_tokens(ctx: &Ctx) -> Result<Option<Tokens>, SendError> {
    Ok(ctx
        .secret(SECRET_NAME)?
        .and_then(|j| serde_json::from_str(&j).ok()))
}

/// Is there a refresh token (so a share can work without asking again)?
pub fn is_signed_in(ctx: &Ctx) -> bool {
    matches!(load_tokens(ctx), Ok(Some(t)) if t.refresh_token.is_some() || t.expires_at > (ctx.now)())
}

/// Forget the tokens, and ask the service to revoke them when it offers that (best effort).
pub fn sign_out(client: &OAuthClient, ctx: &Ctx) {
    if let (Some(url), Ok(Some(tokens))) = (&client.revoke_url, load_tokens(ctx)) {
        let token = tokens
            .refresh_token
            .clone()
            .unwrap_or(tokens.access_token.clone());
        let req = request_with_client(
            client,
            url,
            vec![
                ("token", token),
                ("token_type_hint", "refresh_token".into()),
            ],
            ctx,
        );
        let _ = ctx.http.send(&req);
    }
    let _ = ctx
        .secrets
        .delete(&crate::secrets::key(ctx.connection_id, SECRET_NAME));
}

/// The authorization address the browser is sent to.
pub fn authorization_url(
    client: &OAuthClient,
    redirect_uri: &str,
    state: &str,
    challenge: Option<&str>,
) -> String {
    let mut params: Vec<(String, String)> = vec![
        ("response_type".into(), "code".into()),
        ("client_id".into(), client.client_id.clone()),
        ("redirect_uri".into(), redirect_uri.into()),
        ("state".into(), state.into()),
    ];
    if !client.scopes.is_empty() {
        params.push(("scope".into(), client.scopes.join(" ")));
    }
    if let Some(c) = challenge {
        params.push(("code_challenge".into(), c.into()));
        params.push(("code_challenge_method".into(), "S256".into()));
    }
    params.extend(client.extra_auth_params.iter().cloned());
    let query = params
        .iter()
        .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
        .collect::<Vec<_>>()
        .join("&");
    let sep = if client.auth_url.contains('?') {
        '&'
    } else {
        '?'
    };
    format!("{}{sep}{query}", client.auth_url)
}

const DONE_PAGE: &str = "<!doctype html><meta charset=utf-8><title>CargoFrame</title>\
<body style=\"font-family:system-ui,sans-serif;max-width:32rem;margin:4rem auto;padding:0 1rem\">\
<h1>You can close this tab</h1><p>CargoFrame is now signed in. Go back to the CargoFrame window.</p></body>";

const FAILED_PAGE: &str = "<!doctype html><meta charset=utf-8><title>CargoFrame</title>\
<body style=\"font-family:system-ui,sans-serif;max-width:32rem;margin:4rem auto;padding:0 1rem\">\
<h1>Sign-in did not finish</h1><p>Go back to the CargoFrame window for details.</p></body>";

fn respond(stream: &mut TcpStream, status: &str, body: &str) {
    let msg = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(msg.as_bytes());
    let _ = stream.flush();
}

pub(crate) fn query_pairs(q: &str) -> Vec<(String, String)> {
    q.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let (k, v) = p.split_once('=').unwrap_or((p, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 3 <= b.len() => {
                match std::str::from_utf8(&b[i + 1..i + 3])
                    .ok()
                    .and_then(|h| u8::from_str_radix(h, 16).ok())
                {
                    Some(v) => {
                        out.push(v);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Wait for the service to send the person back. Returns the authorization code.
fn wait_for_code(
    listener: &TcpListener,
    state: &str,
    wait: Duration,
    cancelled: &dyn Fn() -> bool,
) -> Result<String, SendError> {
    listener
        .set_nonblocking(true)
        .map_err(|e| invalid(format!("Cannot wait for the sign-in: {e}")))?;
    let started = Instant::now();
    loop {
        if cancelled() {
            return Err(invalid("Sign-in cancelled"));
        }
        if started.elapsed() > wait {
            return Err(SendError::new(
                FailureKind::Timeout,
                "Sign-in timed out: the browser page was not completed in time",
            ));
        }
        let mut stream = match listener.accept() {
            Ok((s, _)) => s,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(40));
                continue;
            }
            Err(e) => return Err(invalid(format!("Cannot wait for the sign-in: {e}"))),
        };
        let _ = stream.set_nonblocking(false);
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 8192];
        let mut len = 0;
        // read until the end of the request line is in
        while len < buf.len() && !buf[..len].windows(2).any(|w| w == b"\r\n") {
            match stream.read(&mut buf[len..]) {
                Ok(0) | Err(_) => break,
                Ok(n) => len += n,
            }
        }
        let head = String::from_utf8_lossy(&buf[..len]).into_owned();
        let line = head.lines().next().unwrap_or("");
        let mut parts = line.split(' ');
        let (method, target) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
        let (path, query) = target.split_once('?').unwrap_or((target, ""));
        if method != "GET" || path != "/callback" {
            // a favicon request, a probe: not the callback
            respond(&mut stream, "404 Not Found", "Not found");
            continue;
        }
        let pairs = query_pairs(query);
        let get = |k: &str| pairs.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        if get("state") != Some(state) {
            respond(&mut stream, "400 Bad Request", FAILED_PAGE);
            return Err(invalid(
                "The sign-in answer could not be verified (wrong state); try again",
            ));
        }
        if let Some(err) = get("error") {
            respond(&mut stream, "200 OK", FAILED_PAGE);
            return Err(invalid(format!("The service refused the sign-in ({err})")));
        }
        match get("code") {
            Some(code) if !code.is_empty() => {
                respond(&mut stream, "200 OK", DONE_PAGE);
                return Ok(code.to_string());
            }
            _ => {
                respond(&mut stream, "400 Bad Request", FAILED_PAGE);
                return Err(invalid("The sign-in answer had no code"));
            }
        }
    }
}

/// Run the whole sign-in and keep the tokens. Returns them for the caller's first call.
pub fn authorize(client: &OAuthClient, ctx: &Ctx) -> Result<Tokens, SendError> {
    let port = client.redirect_port.unwrap_or(0);
    let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| {
        invalid(if port == 0 {
            format!("Cannot listen for the sign-in on this computer: {e}")
        } else {
            format!(
                "Port {port} is in use, so the sign-in answer cannot be received. Close what uses it and try again ({e})"
            )
        })
    })?;
    let port = listener
        .local_addr()
        .map_err(|e| invalid(e.to_string()))?
        .port();
    let redirect_uri = format!("http://127.0.0.1:{port}/callback");
    let state = random_hex(16);
    if state.is_empty() {
        return Err(invalid(
            "This computer could not provide secure random numbers",
        ));
    }
    let pkce = if client.pkce { pkce_pair() } else { None };
    if client.pkce && pkce.is_none() {
        return Err(invalid(
            "This computer could not provide secure random numbers",
        ));
    }
    let url = authorization_url(
        client,
        &redirect_uri,
        &state,
        pkce.as_ref().map(|(_, c)| c.as_str()),
    );
    ctx.browser
        .open(&url)
        .map_err(|e| invalid(format!("Could not open the browser: {e}")))?;

    let code = wait_for_code(&listener, &state, client.wait, ctx.cancelled)?;
    drop(listener);

    let mut fields = vec![
        ("grant_type", "authorization_code".to_string()),
        ("code", code),
        ("redirect_uri", redirect_uri),
    ];
    if let Some((verifier, _)) = &pkce {
        fields.push(("code_verifier", verifier.clone()));
    }
    let req = request_with_client(client, &client.token_url, fields, ctx);
    let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
    let tokens = if resp.is_success() {
        parse_tokens(&resp, (ctx.now)(), None)?
    } else {
        // Services put the reason in the body (`invalid_grant`); show it.
        match parse_tokens(&resp, (ctx.now)(), None) {
            Err(e) => return Err(e),
            Ok(_) => return Err(from_response(&resp, "finish the sign-in")),
        }
    };
    store_tokens(ctx, &tokens)?;
    Ok(tokens)
}

/// A valid access token, renewed first if it is about to expire.
pub fn access_token(client: &OAuthClient, ctx: &Ctx) -> Result<String, SendError> {
    let tokens = load_tokens(ctx)?
        .ok_or_else(|| not_set_up("Not signed in yet: open Connections and press Sign in"))?;
    let now = (ctx.now)();
    if tokens.expires_at > now + 60 {
        return Ok(tokens.access_token);
    }
    let Some(refresh) = tokens.refresh_token.clone() else {
        return Err(SendError::new(
            FailureKind::AuthExpired,
            "The sign-in has expired: press Sign in again",
        ));
    };
    let req = request_with_client(
        client,
        &client.token_url,
        vec![
            ("grant_type", "refresh_token".to_string()),
            ("refresh_token", refresh.clone()),
        ],
        ctx,
    );
    let resp = ctx.http.send(&req).map_err(|e| from_net(&e))?;
    let fresh = match parse_tokens(&resp, now, Some(&refresh)) {
        Ok(t) => t,
        Err(e) => {
            return Err(if resp.is_success() {
                e
            } else if matches!(e.kind, FailureKind::Rejected(_)) {
                from_response(&resp, "renew the sign-in")
            } else {
                e
            })
        }
    };
    store_tokens(ctx, &fresh)?;
    Ok(fresh.access_token)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ctx::Browser;
    use crate::payload::Progress;
    use crate::secrets::{MemorySecrets, SecretStore};
    use cargoframe_net::mock::{MockResponse, MockServer};
    use cargoframe_net::UreqHttp;
    use std::sync::{Arc, Mutex};

    /// Plays the person: reads the sign-in address and "returns" to the redirect address.
    struct FakeBrowser {
        opened: Mutex<Vec<String>>,
        /// What to send back instead of the right state or a code.
        mode: Mode,
    }

    #[derive(Clone, Copy)]
    enum Mode {
        Approve,
        WrongState,
        Deny,
        Nothing,
    }

    impl Browser for FakeBrowser {
        fn open(&self, url: &str) -> Result<(), String> {
            self.opened.lock().unwrap().push(url.to_string());
            let q = url.split_once('?').map(|(_, q)| q).unwrap_or("");
            let pairs = query_pairs(q);
            let get = |k: &str| pairs.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());
            let redirect = get("redirect_uri").unwrap();
            let state = get("state").unwrap();
            let mode = self.mode;
            std::thread::spawn(move || {
                let target = match mode {
                    Mode::Approve => format!("{redirect}?code=CODE%201&state={state}"),
                    Mode::WrongState => format!("{redirect}?code=CODE&state=evil"),
                    Mode::Deny => format!("{redirect}?error=access_denied&state={state}"),
                    Mode::Nothing => return,
                };
                // a stray request first: it must be ignored
                let base = redirect.trim_end_matches("/callback").to_string();
                let _ = UreqHttp::new().send(&Request::get(format!("{base}/favicon.ico")));
                let _ = UreqHttp::new().send(&Request::get(target));
            });
            Ok(())
        }
    }
    use cargoframe_net::Http;

    fn client(server: &MockServer) -> OAuthClient {
        OAuthClient {
            auth_url: "https://auth.example/authorize".into(),
            token_url: server.url("/token"),
            revoke_url: Some(server.url("/revoke")),
            client_id: "cid".into(),
            client_secret: Some("sec".into()),
            scopes: vec!["a".into(), "b c".into()],
            extra_auth_params: vec![("duration".into(), "permanent".into())],
            pkce: true,
            client_auth: ClientAuth::Body,
            redirect_port: None,
            wait: Duration::from_secs(5),
        }
    }

    struct World {
        http: UreqHttp,
        secrets: MemorySecrets,
        progress: Progress,
        clock: Arc<Mutex<u64>>,
    }

    impl World {
        fn new() -> Self {
            Self {
                http: UreqHttp::new(),
                secrets: MemorySecrets::new(),
                progress: Progress::new(),
                clock: Arc::new(Mutex::new(1_000_000)),
            }
        }

        fn with<R>(&self, browser: &dyn Browser, f: impl FnOnce(&Ctx) -> R) -> R {
            let clock = self.clock.clone();
            let now = move || *clock.lock().unwrap();
            let cancelled = || false;
            let ctx = Ctx {
                connection_id: "svc",
                http: &self.http,
                secrets: &self.secrets,
                browser,
                clipboard: &crate::testkit::FakeClipboard::default(),
                progress: &self.progress,
                timeout: Duration::from_secs(5),
                now: &now,
                cancelled: &cancelled,
            };
            f(&ctx)
        }
    }

    fn token_server() -> MockServer {
        MockServer::start(|req| match req.path() {
            "/token" if req.form_value("grant_type").as_deref() == Some("authorization_code") => {
                MockResponse::json(
                    200,
                    serde_json::json!({"access_token":"AT1","refresh_token":"RT1","expires_in":3600}),
                )
            }
            "/token" => MockResponse::json(
                200,
                serde_json::json!({"access_token":"AT2","expires_in":3600}),
            ),
            "/revoke" => MockResponse::ok(),
            _ => MockResponse::new(404),
        })
    }

    #[test]
    fn pkce_challenge_matches_its_verifier() {
        let (v, c) = pkce_pair().unwrap();
        assert!(v.len() >= 43 && v.len() <= 128);
        assert_eq!(c, URL_SAFE_NO_PAD.encode(Sha256::digest(v.as_bytes())));
        assert_ne!(pkce_pair().unwrap().0, v);
    }

    #[test]
    fn the_authorization_address_carries_everything_the_service_needs() {
        let server = token_server();
        let url = authorization_url(
            &client(&server),
            "http://127.0.0.1:1/callback",
            "ST",
            Some("CH"),
        );
        assert!(url.starts_with("https://auth.example/authorize?response_type=code&client_id=cid"));
        assert!(url.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A1%2Fcallback"));
        assert!(url.contains("scope=a%20b%20c"));
        assert!(url.contains("state=ST"));
        assert!(url.contains("code_challenge=CH&code_challenge_method=S256"));
        assert!(url.ends_with("duration=permanent"));
    }

    #[test]
    fn a_full_sign_in_stores_tokens_and_sends_the_verifier() {
        let server = token_server();
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Approve,
        };
        let tokens = w
            .with(&browser, |ctx| authorize(&client(&server), ctx))
            .unwrap();
        assert_eq!(tokens.access_token, "AT1");
        assert_eq!(tokens.refresh_token.as_deref(), Some("RT1"));
        assert_eq!(tokens.expires_at, 1_000_000 + 3600);
        assert!(w.secrets.keys().contains(&"svc/oauth".to_string()));

        let req = &server.requests_to("/token")[0];
        assert_eq!(req.form_value("code").as_deref(), Some("CODE 1"));
        assert_eq!(req.form_value("client_id").as_deref(), Some("cid"));
        assert_eq!(req.form_value("client_secret").as_deref(), Some("sec"));
        assert!(req
            .form_value("redirect_uri")
            .unwrap()
            .ends_with("/callback"));
        // the verifier answers the challenge that was sent in the browser address
        let opened = browser.opened.lock().unwrap()[0].clone();
        let challenge = query_pairs(opened.split_once('?').unwrap().1)
            .into_iter()
            .find(|(k, _)| k == "code_challenge")
            .unwrap()
            .1;
        let verifier = req.form_value("code_verifier").unwrap();
        assert_eq!(
            challenge,
            URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
        );
        assert!(w.with(&browser, is_signed_in));
    }

    #[test]
    fn basic_client_auth_puts_the_credentials_in_the_header() {
        let server = token_server();
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Approve,
        };
        let mut c = client(&server);
        c.client_auth = ClientAuth::Basic;
        c.client_secret = None;
        c.pkce = false;
        w.with(&browser, |ctx| authorize(&c, ctx)).unwrap();
        let req = &server.requests_to("/token")[0];
        assert_eq!(
            req.header("authorization"),
            Some(format!("Basic {}", STANDARD.encode("cid:")).as_str())
        );
        assert_eq!(req.form_value("client_id"), None);
        assert_eq!(req.form_value("code_verifier"), None);
    }

    #[test]
    fn a_wrong_state_denial_and_silence_all_fail_cleanly() {
        let server = token_server();
        let w = World::new();
        for (mode, want) in [
            (Mode::WrongState, "could not be verified"),
            (Mode::Deny, "access_denied"),
        ] {
            let browser = FakeBrowser {
                opened: Mutex::new(vec![]),
                mode,
            };
            let err = w
                .with(&browser, |ctx| authorize(&client(&server), ctx))
                .unwrap_err();
            assert!(err.message.contains(want), "{}", err.message);
        }
        assert!(server.requests_to("/token").is_empty());

        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Nothing,
        };
        let mut c = client(&server);
        c.wait = Duration::from_millis(300);
        let err = w.with(&browser, |ctx| authorize(&c, ctx)).unwrap_err();
        assert_eq!(err.kind, FailureKind::Timeout);
        assert!(w.secrets.keys().is_empty());
    }

    #[test]
    fn cancelling_stops_the_wait() {
        let server = token_server();
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Nothing,
        };
        let now = || 0u64;
        let cancelled = || true;
        let ctx = Ctx {
            connection_id: "svc",
            http: &w.http,
            secrets: &w.secrets,
            browser: &browser,
            clipboard: &crate::testkit::FakeClipboard::default(),
            progress: &w.progress,
            timeout: Duration::from_secs(1),
            now: &now,
            cancelled: &cancelled,
        };
        let started = Instant::now();
        let err = authorize(&client(&server), &ctx).unwrap_err();
        assert!(err.message.contains("cancelled"));
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn a_refused_code_exchange_reports_the_reason() {
        let server = MockServer::start(|_| {
            MockResponse::json(
                400,
                serde_json::json!({"error":"invalid_grant","error_description":"Bad code"}),
            )
        });
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Approve,
        };
        let err = w
            .with(&browser, |ctx| authorize(&client(&server), ctx))
            .unwrap_err();
        assert_eq!(err.kind, FailureKind::AuthExpired);
        assert!(err.message.contains("Bad code"), "{}", err.message);
        assert!(w.secrets.keys().is_empty());
    }

    #[test]
    fn the_access_token_is_reused_then_renewed_with_the_refresh_token() {
        let server = token_server();
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Approve,
        };
        w.with(&browser, |ctx| authorize(&client(&server), ctx))
            .unwrap();

        // fresh: no extra request
        assert_eq!(
            w.with(&browser, |ctx| access_token(&client(&server), ctx))
                .unwrap(),
            "AT1"
        );
        assert_eq!(server.requests_to("/token").len(), 1);

        // within a minute of expiry: renew, keeping the refresh token
        *w.clock.lock().unwrap() += 3600 - 30;
        assert_eq!(
            w.with(&browser, |ctx| access_token(&client(&server), ctx))
                .unwrap(),
            "AT2"
        );
        let renew = &server.requests_to("/token")[1];
        assert_eq!(
            renew.form_value("grant_type").as_deref(),
            Some("refresh_token")
        );
        assert_eq!(renew.form_value("refresh_token").as_deref(), Some("RT1"));
        let stored = w.with(&browser, load_tokens).unwrap().unwrap();
        assert_eq!(stored.refresh_token.as_deref(), Some("RT1"));
        assert_eq!(stored.access_token, "AT2");
    }

    #[test]
    fn a_revoked_refresh_token_means_sign_in_again() {
        let server = MockServer::start(|_| {
            MockResponse::json(400, serde_json::json!({"error":"invalid_grant"}))
        });
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Nothing,
        };
        w.secrets
            .set(
                "svc/oauth",
                &serde_json::to_string(&Tokens {
                    access_token: "old".into(),
                    refresh_token: Some("RT".into()),
                    expires_at: 1,
                })
                .unwrap(),
            )
            .unwrap();
        let err = w
            .with(&browser, |ctx| access_token(&client(&server), ctx))
            .unwrap_err();
        assert_eq!(err.kind, FailureKind::AuthExpired);
    }

    #[test]
    fn not_signed_in_is_said_plainly() {
        let server = token_server();
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Nothing,
        };
        let err = w
            .with(&browser, |ctx| access_token(&client(&server), ctx))
            .unwrap_err();
        assert_eq!(err.kind, FailureKind::ConnectorUnavailable);
        assert!(!w.with(&browser, is_signed_in));
    }

    #[test]
    fn signing_out_revokes_and_forgets() {
        let server = token_server();
        let w = World::new();
        let browser = FakeBrowser {
            opened: Mutex::new(vec![]),
            mode: Mode::Approve,
        };
        w.with(&browser, |ctx| authorize(&client(&server), ctx))
            .unwrap();
        w.with(&browser, |ctx| sign_out(&client(&server), ctx));
        assert!(w.secrets.keys().is_empty());
        let revoke = &server.requests_to("/revoke")[0];
        assert_eq!(revoke.form_value("token").as_deref(), Some("RT1"));
    }
}
