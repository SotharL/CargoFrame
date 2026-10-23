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
}
