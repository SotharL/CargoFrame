//! Turning what went wrong into the failure classes the retry and fallback logic understands.

use cargoframe_net::{NetError, Response};
use cargoframe_safe::{FailureKind, SendError};

/// A network-level failure.
pub fn from_net(e: &NetError) -> SendError {
    let kind = match e {
        NetError::Offline(_) | NetError::Other(_) => FailureKind::NoNetwork,
        NetError::Timeout => FailureKind::Timeout,
        NetError::Tls(_) => FailureKind::Certificate,
        NetError::BadUrl(_) | NetError::TooLarge { .. } => FailureKind::Rejected(0),
    };
    SendError::new(kind, e.to_string())
}

/// The first useful sentence of an error body: JSON the common services use, or plain text.
/// Cut short so a web page of an error cannot flood the log.
pub fn service_message(resp: &Response) -> String {
    let text = resp.text();
    let from_json = resp.json().and_then(|v| message_in(&v));
    let msg = from_json.unwrap_or_else(|| {
        let t = text.trim();
        if t.starts_with('<') {
            // an HTML error page says nothing useful
            String::new()
        } else {
            t.to_string()
        }
    });
    let one_line: String = msg.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 300 {
        format!("{}…", one_line.chars().take(300).collect::<String>())
    } else {
        one_line
    }
}

fn message_in(v: &serde_json::Value) -> Option<String> {
    use serde_json::Value;
    // Jira: {"errorMessages": ["..."], "errors": {"field": "..."}}
    if let Some(list) = v.get("errorMessages").and_then(Value::as_array) {
        let mut parts: Vec<String> = list
            .iter()
            .filter_map(|m| m.as_str().map(String::from))
            .collect();
        if let Some(map) = v.get("errors").and_then(Value::as_object) {
            parts.extend(
                map.iter()
                    .filter_map(|(k, m)| m.as_str().map(|m| format!("{k}: {m}"))),
            );
        }
        if !parts.is_empty() {
            return Some(parts.join("; "));
        }
    }
    if let Some(map) = v.get("errors").and_then(Value::as_object) {
        let parts: Vec<String> = map
            .iter()
            .filter_map(|(k, m)| m.as_str().map(|m| format!("{k}: {m}")))
            .collect();
        if !parts.is_empty() {
            return Some(parts.join("; "));
        }
    }
    // Google: {"error": {"message": "..."}}; OAuth: {"error": "...", "error_description": "..."}
    if let Some(e) = v.get("error") {
        if let Some(m) = e.get("message").and_then(Value::as_str) {
            return Some(m.to_string());
        }
        if let Some(code) = e.as_str() {
            return Some(match v.get("error_description").and_then(Value::as_str) {
                Some(d) => format!("{code}: {d}"),
                None => code.to_string(),
            });
        }
    }
    // Reddit: {"json": {"errors": [["RATELIMIT", "you are doing that too much", "ratelimit"]]}}
    if let Some(errs) = v.pointer("/json/errors").and_then(Value::as_array) {
        let parts: Vec<String> = errs
            .iter()
            .filter_map(|e| {
                let a = e.as_array()?;
                Some(
                    a.iter()
                        .take(2)
                        .filter_map(Value::as_str)
                        .collect::<Vec<_>>()
                        .join(": "),
                )
            })
            .collect();
        if !parts.is_empty() {
            return Some(parts.join("; "));
        }
    }
    v.get("message").and_then(Value::as_str).map(String::from)
}

/// A non-success answer. `what` says what was being done ("create the issue").
pub fn from_response(resp: &Response, what: &str) -> SendError {
    let kind = FailureKind::from_status(resp.status).unwrap_or(FailureKind::Rejected(resp.status));
    let mut detail = service_message(resp);
    if (300..400).contains(&resp.status) {
        if let Some(to) = resp.header("location") {
            detail = format!("the server redirected to {to}; use that address instead");
        }
    }
    let message = if detail.is_empty() {
        format!("Could not {what} (HTTP {})", resp.status)
    } else {
        format!("Could not {what} (HTTP {}): {detail}", resp.status)
    };
    let mut err = SendError::new(kind, message);
    if let Some(after) = resp.retry_after() {
        err = err.with_retry_after(after);
    }
    err
}

/// Something the connector can see is wrong before sending: not retried, and the share goes
/// to the fallback so nothing is lost.
pub fn invalid(message: impl Into<String>) -> SendError {
    SendError::new(FailureKind::Rejected(0), message)
}

pub fn not_set_up(message: impl Into<String>) -> SendError {
    SendError::new(FailureKind::ConnectorUnavailable, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn resp(status: u16, body: &str) -> Response {
        Response {
            status,
            headers: vec![],
            body: body.as_bytes().to_vec(),
        }
    }

    #[test]
    fn jira_google_oauth_and_reddit_messages_are_found() {
        let m = |s: &str| service_message(&resp(400, s));
        assert_eq!(
            m(r#"{"errorMessages":["Bad"],"errors":{"summary":"Required"}}"#),
            "Bad; summary: Required"
        );
        assert_eq!(m(r#"{"errors":{"project":"Unknown"}}"#), "project: Unknown");
        assert_eq!(
            m(r#"{"error":{"code":403,"message":"No access"}}"#),
            "No access"
        );
        assert_eq!(
            m(r#"{"error":"invalid_grant","error_description":"Bad code"}"#),
            "invalid_grant: Bad code"
        );
        assert_eq!(
            m(r#"{"json":{"errors":[["RATELIMIT","slow down","ratelimit"]]}}"#),
            "RATELIMIT: slow down"
        );
        assert_eq!(m(r#"{"message":"plain"}"#), "plain");
        assert_eq!(m("  just   text "), "just text");
    }

    #[test]
    fn html_pages_and_long_messages_do_not_leak_through() {
        assert_eq!(service_message(&resp(500, "<html>boom</html>")), "");
        let long = "x".repeat(1000);
        let m = service_message(&resp(500, &long));
        assert!(m.chars().count() <= 301 && m.ends_with('…'));
    }

    #[test]
    fn failures_get_the_right_class_and_keep_retry_after() {
        let mut r = resp(429, r#"{"message":"slow"}"#);
        r.headers.push(("Retry-After".into(), "5".into()));
        let e = from_response(&r, "create the issue");
        assert_eq!(e.kind, FailureKind::Throttled(429));
        assert_eq!(e.retry_after, Some(std::time::Duration::from_secs(5)));
        assert_eq!(e.message, "Could not create the issue (HTTP 429): slow");

        assert_eq!(
            from_response(&resp(401, ""), "x").kind,
            FailureKind::AuthExpired
        );
        assert_eq!(
            from_response(&resp(503, ""), "x").kind,
            FailureKind::Server(503)
        );
        assert_eq!(
            from_response(&resp(404, ""), "x").message,
            "Could not x (HTTP 404)"
        );
        let mut moved = resp(302, "");
        moved
            .headers
            .push(("Location".into(), "https://new.example/api".into()));
        assert!(from_response(&moved, "x")
            .message
            .contains("use that address instead"));
    }

    #[test]
    fn network_errors_map_to_failure_kinds() {
        assert_eq!(from_net(&NetError::Timeout).kind, FailureKind::Timeout);
        assert_eq!(
            from_net(&NetError::Offline("x".into())).kind,
            FailureKind::NoNetwork
        );
        assert_eq!(
            from_net(&NetError::Tls("x".into())).kind,
            FailureKind::Certificate
        );
        assert!(!from_net(&NetError::BadUrl("x".into())).kind.is_retryable());
    }
}
