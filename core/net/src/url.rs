//! Just enough URL handling: encoding, query strings, and a check that a URL is safe to call.

/// Encode one query or form component (RFC 3986 unreserved characters stay).
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `a=1&b=x%20y` for a form body (spaces as `%20`, which every server accepts).
pub fn form_encode(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{}={}", encode_component(k), encode_component(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `?a=1&b=2`, or an empty string when there are no parameters.
pub fn query_string(fields: &[(&str, &str)]) -> String {
    if fields.is_empty() {
        String::new()
    } else {
        format!("?{}", form_encode(fields))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UrlError {
    #[error("The address is empty")]
    Empty,
    #[error("The address must start with https://")]
    NotHttps,
    #[error("The address has no host name")]
    NoHost,
    #[error("Do not put a user name or password in the address; use the sign-in fields")]
    Credentials,
    #[error("The address contains spaces or control characters")]
    BadCharacters,
}

fn is_loopback(host: &str) -> bool {
    let h = host.trim_start_matches('[').trim_end_matches(']');
    h.eq_ignore_ascii_case("localhost") || h == "127.0.0.1" || h == "::1"
}

/// Accept `https://host/...`. Plain `http://` is accepted only for this computer (localhost),
/// which is how a local test server is reached. Returns the host name.
pub fn check_url(url: &str) -> Result<String, UrlError> {
    let url = url.trim();
    if url.is_empty() {
        return Err(UrlError::Empty);
    }
    if url.chars().any(|c| c.is_control() || c == ' ') {
        return Err(UrlError::BadCharacters);
    }
    let (secure, rest) = if let Some(r) = strip_prefix_ci(url, "https://") {
        (true, r)
    } else if let Some(r) = strip_prefix_ci(url, "http://") {
        (false, r)
    } else {
        return Err(UrlError::NotHttps);
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    if authority.contains('@') {
        return Err(UrlError::Credentials);
    }
    let host = if let Some(stripped) = authority.strip_prefix('[') {
        // [::1]:8080
        stripped.split(']').next().unwrap_or("")
    } else {
        authority.split(':').next().unwrap_or("")
    };
    if host.is_empty() {
        return Err(UrlError::NoHost);
    }
    if !secure && !is_loopback(host) {
        return Err(UrlError::NotHttps);
    }
    Ok(host.to_string())
}

/// `scheme://authority` of a URL in lower case, with the default port dropped, so two spellings
/// of the same site compare equal. `None` for anything that is not an http(s) URL.
pub fn origin(url: &str) -> Option<String> {
    let url = url.trim();
    let (scheme, rest) = if let Some(r) = strip_prefix_ci(url, "https://") {
        ("https", r)
    } else {
        ("http", strip_prefix_ci(url, "http://")?)
    };
    let authority = rest
        .split(['/', '?', '#'])
        .next()
        .unwrap_or("")
        .to_lowercase();
    let default_port = if scheme == "https" { ":443" } else { ":80" };
    let authority = authority
        .strip_suffix(default_port)
        .unwrap_or(&authority)
        .to_string();
    Some(format!("{scheme}://{authority}"))
}

pub fn same_origin(a: &str, b: &str) -> bool {
    match (origin(a), origin(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RedirectError {
    #[error("the redirect address is not usable: {0}")]
    Unusable(UrlError),
    #[error("the service tried to move the request from https to plain http")]
    Downgrade,
}

/// Where a redirect from `current` to `location` leads, if it is safe to follow. The address is
/// resolved the way a browser would, must pass [`check_url`], and may not leave https for http.
pub fn redirect_target(current: &str, location: &str) -> Result<String, RedirectError> {
    let location = location.trim();
    let current = current.trim();
    let base_origin = origin(current).ok_or(RedirectError::Unusable(UrlError::NotHttps))?;
    let after_origin = &current[strip_origin_len(current)..];
    let path_part = after_origin.split(['?', '#']).next().unwrap_or("");

    let resolved = if location.starts_with("//") {
        let scheme = base_origin.split("://").next().unwrap_or("https");
        format!("{scheme}:{location}")
    } else if has_scheme(location) {
        location.to_string()
    } else if location.starts_with('/') {
        format!("{base_origin}{location}")
    } else if location.starts_with('?') {
        format!("{base_origin}{path_part}{location}")
    } else {
        let dir = match path_part.rfind('/') {
            Some(i) => &path_part[..=i],
            None => "/",
        };
        format!("{base_origin}{dir}{location}")
    };
    check_url(&resolved).map_err(RedirectError::Unusable)?;
    if base_origin.starts_with("https://") && !resolved.to_lowercase().starts_with("https://") {
        return Err(RedirectError::Downgrade);
    }
    Ok(resolved)
}

/// Length of `scheme://authority` at the start of `url` (0 when it has none).
fn strip_origin_len(url: &str) -> usize {
    let Some(i) = url.find("://") else { return 0 };
    let rest = &url[i + 3..];
    i + 3 + rest.split(['/', '?', '#']).next().map_or(0, str::len)
}

fn has_scheme(s: &str) -> bool {
    let Some(i) = s.find(':') else { return false };
    let scheme = &s[..i];
    !scheme.is_empty()
        && scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    if s.len() >= prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
        Some(&s[prefix.len()..])
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_reserved_and_non_ascii() {
        assert_eq!(encode_component("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
        assert_eq!(encode_component("AZaz09-_.~"), "AZaz09-_.~");
        assert_eq!(form_encode(&[("q", "x y"), ("n", "1")]), "q=x%20y&n=1");
        assert_eq!(query_string(&[]), "");
        assert_eq!(query_string(&[("a", "1")]), "?a=1");
    }

    #[test]
    fn https_is_required_except_for_this_computer() {
        assert_eq!(
            check_url("https://acme.atlassian.net/x").unwrap(),
            "acme.atlassian.net"
        );
        assert_eq!(check_url("HTTPS://Example.com").unwrap(), "Example.com");
        assert_eq!(check_url("http://127.0.0.1:8080/a").unwrap(), "127.0.0.1");
        assert_eq!(check_url("http://localhost/a").unwrap(), "localhost");
        assert_eq!(check_url("http://[::1]:9/a").unwrap(), "::1");
        assert_eq!(check_url("http://example.com"), Err(UrlError::NotHttps));
        assert_eq!(check_url("ftp://example.com"), Err(UrlError::NotHttps));
        assert_eq!(check_url("example.com"), Err(UrlError::NotHttps));
    }

    #[test]
    fn rejects_credentials_empty_hosts_and_odd_characters() {
        assert_eq!(check_url(""), Err(UrlError::Empty));
        assert_eq!(check_url("https://"), Err(UrlError::NoHost));
        assert_eq!(check_url("https:///path"), Err(UrlError::NoHost));
        assert_eq!(
            check_url("https://user:pw@example.com"),
            Err(UrlError::Credentials)
        );
        assert_eq!(
            check_url("https://example.com@evil.com/"),
            Err(UrlError::Credentials)
        );
        assert_eq!(
            check_url("https://exa mple.com"),
            Err(UrlError::BadCharacters)
        );
        assert_eq!(
            check_url("https://example.com/\r\nX: y"),
            Err(UrlError::BadCharacters)
        );
        // an @ after the path is fine
        assert!(check_url("https://example.com/a@b").is_ok());
    }

    #[test]
    fn origins_ignore_case_and_default_ports() {
        assert!(same_origin(
            "https://Example.com/a",
            "https://example.com:443/b"
        ));
        assert!(same_origin("http://127.0.0.1:80/x", "http://127.0.0.1/y"));
        assert!(!same_origin("https://example.com/", "http://example.com/"));
        assert!(!same_origin(
            "https://example.com/",
            "https://example.com:8443/"
        ));
        assert!(!same_origin(
            "https://a.example.com/",
            "https://example.com/"
        ));
        assert!(!same_origin("ftp://example.com/", "ftp://example.com/"));
    }

    #[test]
    fn redirects_are_resolved_like_a_browser_would() {
        let cur = "https://api.example.com/v1/items/42?x=1";
        let go = |l: &str| redirect_target(cur, l).unwrap();
        assert_eq!(
            go("https://other.example.org/z"),
            "https://other.example.org/z"
        );
        assert_eq!(go("//cdn.example.org/z"), "https://cdn.example.org/z");
        assert_eq!(go("/login"), "https://api.example.com/login");
        assert_eq!(go("43"), "https://api.example.com/v1/items/43");
        assert_eq!(go("?page=2"), "https://api.example.com/v1/items/42?page=2");
    }

    #[test]
    fn a_redirect_may_not_leave_https_or_carry_credentials() {
        let cur = "https://api.example.com/start";
        assert_eq!(
            redirect_target(cur, "http://api.example.com/plain"),
            Err(RedirectError::Unusable(UrlError::NotHttps))
        );
        // even this computer: a service must not point an https call at a local port
        assert_eq!(
            redirect_target(cur, "http://127.0.0.1:9/x"),
            Err(RedirectError::Downgrade)
        );
        assert_eq!(
            redirect_target(cur, "//127.0.0.1:9/x").map(|_| ()),
            Ok(()) // scheme-relative keeps https
        );
        assert!(matches!(
            redirect_target(cur, "ftp://example.com/f"),
            Err(RedirectError::Unusable(UrlError::NotHttps))
        ));
        assert!(matches!(
            redirect_target(cur, "https://user:pw@example.com/"),
            Err(RedirectError::Unusable(UrlError::Credentials))
        ));
        assert!(matches!(
            redirect_target(cur, "https://exa mple.com/"),
            Err(RedirectError::Unusable(UrlError::BadCharacters))
        ));
        assert!(matches!(
            redirect_target(cur, "javascript:alert(1)"),
            Err(RedirectError::Unusable(UrlError::NotHttps))
        ));
    }

    #[test]
    fn a_local_test_server_may_redirect_to_another_local_one() {
        assert_eq!(
            redirect_target("http://127.0.0.1:1/a", "http://127.0.0.1:2/b").unwrap(),
            "http://127.0.0.1:2/b"
        );
    }
}
