//! `connections/<id>.json`: how to reach one service. Passwords, tokens and keys are not here;
//! they live in the credential store under `<id>/<name>`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Connection {
    /// What the person calls it ("Work Jira").
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(flatten)]
    pub settings: Settings,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Settings {
    Jira(Jira),
    Google(Google),
    Reddit(Reddit),
    Claude(Claude),
    Slack(Slack),
    Discord(Discord),
    Teams(Teams),
    Custom(Custom),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Jira,
    Google,
    Reddit,
    Claude,
    Slack,
    Discord,
    Teams,
    Custom,
}

impl Settings {
    pub fn kind(&self) -> Kind {
        match self {
            Settings::Jira(_) => Kind::Jira,
            Settings::Google(_) => Kind::Google,
            Settings::Reddit(_) => Kind::Reddit,
            Settings::Claude(_) => Kind::Claude,
            Settings::Slack(_) => Kind::Slack,
            Settings::Discord(_) => Kind::Discord,
            Settings::Teams(_) => Kind::Teams,
            Settings::Custom(_) => Kind::Custom,
        }
    }
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Jira => "Jira",
            Kind::Google => "Google",
            Kind::Reddit => "Reddit",
            Kind::Claude => "Claude",
            Kind::Slack => "Slack",
            Kind::Discord => "Discord",
            Kind::Teams => "Teams",
            Kind::Custom => "Custom API",
        }
    }

    /// Names the credential store may hold for this kind (and the only ones the window can set).
    pub fn secret_names(self) -> &'static [&'static str] {
        match self {
            Kind::Jira => &["token"],
            Kind::Google => &["client_secret", "oauth"],
            Kind::Reddit => &["oauth"],
            Kind::Claude => &[],
            Kind::Slack | Kind::Discord => &["token"],
            Kind::Teams => &["oauth"],
            Kind::Custom => &["secret"],
        }
    }

    /// Of those, the ones a person types in (the rest are written by the sign-in).
    pub fn typed_secrets(self) -> &'static [&'static str] {
        match self {
            Kind::Jira => &["token"],
            Kind::Google => &["client_secret"],
            Kind::Slack | Kind::Discord => &["token"],
            Kind::Reddit | Kind::Claude | Kind::Teams => &[],
            Kind::Custom => &["secret"],
        }
    }
}

// ----- Jira -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JiraAuth {
    /// Jira Cloud: your e-mail address and an API token.
    #[default]
    ApiToken,
    /// Jira Server or Data Center: a personal access token.
    Pat,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Jira {
    /// `https://yourteam.atlassian.net`
    pub site_url: String,
    pub auth: JiraAuth,
    /// Account e-mail, for API-token sign-in.
    pub email: String,
}

// ----- Reddit -----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Reddit {
    /// The id of the "installed app" the person registered at reddit.com/prefs/apps.
    pub client_id: String,
    /// Must match the app's redirect address `http://127.0.0.1:<port>/callback`.
    pub redirect_port: u16,
}

impl Default for Reddit {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            redirect_port: 53682,
        }
    }
}

// ----- Google -----

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Google {
    /// From a "Desktop app" OAuth client in the person's Google Cloud project.
    pub client_id: String,
    /// Also add a row to a spreadsheet (asks for the spreadsheets permission).
    pub sheets: bool,
    /// Upload into any of your Drive folders. Off, CargoFrame can only use folders it created
    /// itself (a narrower permission).
    pub full_drive: bool,
}

// ----- Teams -----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Teams {
    /// The Application (client) ID of an app registration in the person's Microsoft Entra ID.
    pub client_id: String,
    /// `organizations` (any work or school account), or the Directory (tenant) ID or domain.
    pub tenant: String,
}

impl Default for Teams {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            tenant: "organizations".into(),
        }
    }
}

// ----- Claude -----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OpenWith {
    /// The desktop app on Windows, the browser on Linux.
    #[default]
    Auto,
    App,
    Browser,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Claude {
    pub open_with: OpenWith,
    /// Press Enter after pasting, so the message is sent. Off: it waits in the message box.
    pub send_automatically: bool,
    /// Seconds to wait, on top of the usual, before pasting (for a slow computer).
    pub extra_wait_seconds: u32,
}

impl Default for Claude {
    fn default() -> Self {
        Self {
            open_with: OpenWith::default(),
            send_automatically: true,
            extra_wait_seconds: 0,
        }
    }
}

// ----- Slack -----

/// Slack needs nothing but a bot token (kept in the credential store): the token belongs to one
/// workspace, so there is no address or account to set.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Slack {}

// ----- Discord -----

/// Like Slack: only a bot token, which says which servers the bot is in.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Discord {}

// ----- Custom API -----

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomAuth {
    #[default]
    None,
    /// `Authorization: Bearer <secret>`
    Bearer,
    /// A named header carries the secret.
    Header { name: String },
    /// `Authorization: Basic base64(user:<secret>)`
    Basic { user: String },
    /// A query parameter carries the secret.
    Query { param: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Pair {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum SendMethod {
    #[default]
    Post,
    Put,
    Patch,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CustomBody {
    #[default]
    None,
    /// JSON text with `{{field}}` placeholders (values are escaped for JSON).
    Json {
        template: String,
    },
    Form {
        fields: Vec<Pair>,
    },
    /// `image_field` carries the screenshot, `files_field` the zip of ticked files; leave a name
    /// empty to leave that part out.
    Multipart {
        fields: Vec<Pair>,
        image_field: String,
        files_field: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomRequest {
    pub method: SendMethod,
    /// Appended to the base address; may contain `{{field}}` placeholders.
    pub path: String,
    pub headers: Vec<Pair>,
    pub body: CustomBody,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CustomList {
    /// The name a dropdown or info type uses.
    pub id: String,
    pub label: String,
    /// GET path.
    pub path: String,
    /// Where the array is in the answer (`data.items`; empty when the answer is the array).
    pub items_path: String,
    pub value_path: String,
    pub label_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Custom {
    pub base_url: String,
    pub auth: CustomAuth,
    pub send: CustomRequest,
    pub lists: Vec<CustomList>,
    /// A GET used by "Test connection". Empty skips the test.
    pub test_path: String,
    /// Where the answer says what was created (`data.url`), shown as a link.
    pub result_url_path: String,
}

/// Check a connection as it is saved. Returns what to change, in words.
pub fn check(c: &Connection) -> Result<(), String> {
    if c.name.trim().is_empty() {
        return Err("Give the connection a name".into());
    }
    match &c.settings {
        Settings::Jira(j) => {
            cargoframe_net::check_url(&j.site_url).map_err(|e| format!("Jira address: {e}"))?;
            if j.site_url.trim().to_lowercase().starts_with("http://") {
                return Err("Jira address: it must start with https://".into());
            }
            if j.auth == JiraAuth::ApiToken && !j.email.contains('@') {
                return Err("Write the e-mail address of the Jira account".into());
            }
        }
        Settings::Reddit(r) => {
            if r.client_id.trim().is_empty() {
                return Err("Write the client id of your Reddit app".into());
            }
            if r.redirect_port < 1024 {
                return Err("Use a port of 1024 or more for the redirect address".into());
            }
        }
        Settings::Google(g) => {
            if g.client_id.trim().is_empty() {
                return Err("Write the client id of your Google OAuth client".into());
            }
        }
        Settings::Teams(t) => check_teams(t)?,
        Settings::Claude(_) | Settings::Slack(_) | Settings::Discord(_) => {}
        Settings::Custom(cu) => check_custom(cu)?,
    }
    Ok(())
}

/// A GUID, the shape of an application (client) id and of a tenant id.
pub fn is_guid(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    parts.len() == 5
        && [8, 4, 4, 4, 12]
            .iter()
            .zip(&parts)
            .all(|(n, p)| p.len() == *n && p.chars().all(|c| c.is_ascii_hexdigit()))
}

fn check_teams(t: &Teams) -> Result<(), String> {
    if !is_guid(t.client_id.trim()) {
        return Err(
            "Write the Application (client) ID of your Microsoft app: it looks like 3f2a1b4c-…"
                .into(),
        );
    }
    let tenant = t.tenant.trim();
    let word_ok = tenant
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
    if tenant.is_empty()
        || !word_ok
        || tenant.eq_ignore_ascii_case("common")
        || tenant.eq_ignore_ascii_case("consumers")
    {
        return Err(
            "Write \"organizations\", or the Directory (tenant) ID or domain of your organisation. Personal Microsoft accounts cannot use Teams this way".into(),
        );
    }
    Ok(())
}

fn check_custom(cu: &Custom) -> Result<(), String> {
    cargoframe_net::check_url(&cu.base_url).map_err(|e| format!("Base address: {e}"))?;
    let bad_name =
        |n: &str| n.trim().is_empty() || n.chars().any(|c| c.is_control() || c == ':' || c == ' ');
    match &cu.auth {
        CustomAuth::Header { name } if bad_name(name) => {
            return Err("Write the header name that carries the key".into())
        }
        CustomAuth::Query { param } if param.trim().is_empty() => {
            return Err("Write the query parameter that carries the key".into())
        }
        CustomAuth::Basic { user } if user.trim().is_empty() => {
            return Err("Write the user name for basic sign-in".into())
        }
        _ => {}
    }
    for h in &cu.send.headers {
        if bad_name(&h.name) {
            return Err(format!("\"{}\" cannot be used as a header name", h.name));
        }
        if h.name.eq_ignore_ascii_case("authorization") || h.name.eq_ignore_ascii_case("host") {
            return Err(format!(
                "Set \"{}\" with the sign-in option, not as a custom header",
                h.name
            ));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for l in &cu.lists {
        if l.id.trim().is_empty() {
            return Err("Every list needs a name".into());
        }
        if !seen.insert(l.id.as_str()) {
            return Err(format!("The list \"{}\" appears twice", l.id));
        }
        if l.path.trim().is_empty() || l.value_path.trim().is_empty() {
            return Err(format!(
                "The list \"{}\" needs a path and where to find each value",
                l.id
            ));
        }
    }
    if cu.send.path.trim().is_empty() && !matches!(cu.send.body, CustomBody::None) {
        // posting to the base address itself is fine; only warn about nothing
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conn(json: serde_json::Value) -> Connection {
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn each_kind_round_trips_with_a_flat_shape() {
        let j = conn(serde_json::json!({
            "name": "Work",
            "kind": "jira",
            "site_url": "https://a.atlassian.net",
            "email": "me@a.com"
        }));
        assert!(j.enabled);
        assert_eq!(j.settings.kind(), Kind::Jira);
        let back = serde_json::to_value(&j).unwrap();
        assert_eq!(back["kind"], "jira");
        assert_eq!(back["site_url"], "https://a.atlassian.net");
        assert_eq!(back["auth"], "api_token");
        assert_eq!(conn(back), j);

        let c = conn(
            serde_json::json!({"name":"x","kind":"custom","base_url":"https://x.test",
            "auth":{"type":"header","name":"X-Key"},
            "send":{"method":"PUT","path":"/a","body":{"type":"json","template":"{}"}},
            "lists":[{"id":"l","path":"/l","value_path":"id"}]}),
        );
        let Settings::Custom(cu) = &c.settings else {
            panic!()
        };
        assert_eq!(cu.send.method, SendMethod::Put);
        assert_eq!(
            cu.auth,
            CustomAuth::Header {
                name: "X-Key".into()
            }
        );
        assert_eq!(conn(serde_json::to_value(&c).unwrap()), c);
    }

    #[test]
    fn secrets_are_never_part_of_the_file_shape() {
        let c = Connection {
            name: "n".into(),
            enabled: true,
            settings: Settings::Google(Google {
                client_id: "id".into(),
                sheets: true,
                full_drive: false,
            }),
        };
        let text = serde_json::to_string(&c).unwrap();
        assert!(
            !text.contains("secret") && !text.contains("token"),
            "{text}"
        );
        assert_eq!(Kind::Google.secret_names(), &["client_secret", "oauth"]);
        assert_eq!(Kind::Google.typed_secrets(), &["client_secret"]);
    }

    fn jira(site: &str, auth: JiraAuth, email: &str) -> Connection {
        Connection {
            name: "J".into(),
            enabled: true,
            settings: Settings::Jira(Jira {
                site_url: site.into(),
                auth,
                email: email.into(),
            }),
        }
    }

    #[test]
    fn jira_needs_https_and_an_email_for_api_tokens() {
        assert!(check(&jira(
            "https://a.atlassian.net",
            JiraAuth::ApiToken,
            "me@a.com"
        ))
        .is_ok());
        assert!(check(&jira("https://jira.corp", JiraAuth::Pat, "")).is_ok());
        assert!(check(&jira("http://jira.corp", JiraAuth::Pat, "")).is_err());
        assert!(check(&jira("https://a.atlassian.net", JiraAuth::ApiToken, "")).is_err());
        assert!(check(&jira("", JiraAuth::Pat, "")).is_err());
        assert!(check(&jira("https://u:p@a.net", JiraAuth::Pat, "")).is_err());
        let mut c = jira("https://a.net", JiraAuth::Pat, "");
        c.name = " ".into();
        assert!(check(&c).is_err());
    }

    #[test]
    fn reddit_google_and_custom_checks() {
        let reddit = |id: &str, port: u16| Connection {
            name: "R".into(),
            enabled: true,
            settings: Settings::Reddit(Reddit {
                client_id: id.into(),
                redirect_port: port,
            }),
        };
        assert!(check(&reddit("abc", 53682)).is_ok());
        assert!(check(&reddit("", 53682)).is_err());
        assert!(check(&reddit("abc", 80)).is_err());

        let google = |id: &str| Connection {
            name: "G".into(),
            enabled: true,
            settings: Settings::Google(Google {
                client_id: id.into(),
                sheets: false,
                full_drive: false,
            }),
        };
        assert!(check(&google("x.apps.googleusercontent.com")).is_ok());
        assert!(check(&google(" ")).is_err());

        let custom = |f: &dyn Fn(&mut Custom)| {
            let mut cu = Custom {
                base_url: "https://api.test".into(),
                ..Custom::default()
            };
            f(&mut cu);
            Connection {
                name: "C".into(),
                enabled: true,
                settings: Settings::Custom(cu),
            }
        };
        assert!(check(&custom(&|_| {})).is_ok());
        assert!(check(&custom(&|c| c.base_url = "http://api.test".into())).is_err());
        assert!(check(&custom(&|c| c.auth = CustomAuth::Header {
            name: "bad name".into()
        }))
        .is_err());
        assert!(check(&custom(&|c| c.send.headers.push(Pair {
            name: "Authorization".into(),
            value: "x".into()
        })))
        .is_err());
        assert!(check(&custom(&|c| c.lists.push(CustomList {
            id: "a".into(),
            ..CustomList::default()
        })))
        .is_err());
        assert!(check(&custom(&|c| {
            let l = CustomList {
                id: "a".into(),
                path: "/a".into(),
                value_path: "id".into(),
                ..CustomList::default()
            };
            c.lists = vec![l.clone(), l];
        }))
        .is_err());
    }
}
