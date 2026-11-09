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
    X(X),
    Bluesky(Bluesky),
    Instagram(Instagram),
    Github(Github),
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
    X,
    Bluesky,
    Instagram,
    Github,
}

impl Kind {
    /// Every kind, in the order the interface lists them.
    pub const ALL: [Kind; 12] = [
        Kind::Jira,
        Kind::Github,
        Kind::Google,
        Kind::Reddit,
        Kind::Claude,
        Kind::Slack,
        Kind::Discord,
        Kind::Teams,
        Kind::X,
        Kind::Bluesky,
        Kind::Instagram,
        Kind::Custom,
    ];
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
            Settings::X(_) => Kind::X,
            Settings::Bluesky(_) => Kind::Bluesky,
            Settings::Instagram(_) => Kind::Instagram,
            Settings::Github(_) => Kind::Github,
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
            Kind::X => "X",
            Kind::Bluesky => "Bluesky",
            Kind::Instagram => "Instagram",
            Kind::Github => "GitHub",
        }
    }

    /// Names the credential store may hold for this kind (and the only ones the window can set).
    pub fn secret_names(self) -> &'static [&'static str] {
        match self {
            Kind::Jira => &["token"],
            Kind::Google => &["client_secret", "oauth"],
            Kind::Reddit => &["oauth"],
            Kind::Claude => &[],
            Kind::Slack => &["token"],
            Kind::Discord => &DISCORD_SECRETS,
            Kind::Teams => &["oauth"],
            Kind::Custom => &["secret"],
            Kind::X => &["oauth"],
            Kind::Bluesky => &["app_password"],
            Kind::Instagram => &["token", "host_key"],
            Kind::Github => &["token"],
        }
    }

    /// Of those, the ones a person types in (the rest are written by the sign-in).
    pub fn typed_secrets(self) -> &'static [&'static str] {
        match self {
            Kind::Jira => &["token"],
            Kind::Google => &["client_secret"],
            Kind::Slack => &["token"],
            Kind::Discord => &DISCORD_SECRETS,
            Kind::Reddit | Kind::Claude | Kind::Teams | Kind::X => &[],
            Kind::Custom => &["secret"],
            Kind::Bluesky => &["app_password"],
            Kind::Instagram => &["token", "host_key"],
            Kind::Github => &["token"],
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

// ----- X -----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct X {
    /// The OAuth 2.0 Client ID of the person's own app in the X developer portal (a "Native App",
    /// so there is no client secret).
    pub client_id: String,
    /// Must match the app's callback address `http://127.0.0.1:<port>/callback`.
    pub redirect_port: u16,
}

impl Default for X {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            redirect_port: 53683,
        }
    }
}

// ----- Bluesky -----

/// Bluesky needs the account's handle and an app password (kept in the credential store): no
/// developer app to register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Bluesky {
    /// `name.bsky.social`, or a custom domain handle.
    pub handle: String,
    /// The account's server. Almost everyone is on `https://bsky.social`.
    pub service: String,
}

impl Default for Bluesky {
    fn default() -> Self {
        Self {
            handle: String::new(),
            service: "https://bsky.social".into(),
        }
    }
}

// ----- GitHub -----

/// Where a share's pictures and files go on GitHub, whose API cannot attach a file to an issue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GithubFiles {
    /// Put in a branch of the repository kept for them, and linked from the issue.
    #[default]
    Branch,
    /// Not sent: the issue has the text only.
    None,
}

/// GitHub (or GitHub Enterprise Server): a token, the repository issues go to, and where pictures
/// go. The token (fine-grained: Issues read and write, and Contents read and write for pictures)
/// is in the credential store.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Github {
    /// `owner/name`. A layout can send to another one with the field `repo`.
    pub repository: String,
    /// `https://api.github.com`, or `https://github.example.com/api/v3` for a company's own.
    pub api_url: String,
    pub files: GithubFiles,
    /// The branch pictures and files are put in.
    pub branch: String,
}

impl Default for Github {
    fn default() -> Self {
        Self {
            repository: String::new(),
            api_url: "https://api.github.com".into(),
            files: GithubFiles::Branch,
            branch: "cargoframe-attachments".into(),
        }
    }
}

/// `owner/name`, as GitHub writes a repository.
pub fn repository_ok(r: &str) -> bool {
    let Some((owner, name)) = r.trim().split_once('/') else {
        return false;
    };
    let part = |p: &str| {
        !p.is_empty()
            && p.len() <= 100
            && p.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    };
    part(owner) && part(name) && name != "." && name != ".."
}

// ----- Instagram -----

/// Instagram takes a picture only from a public web address, never as an upload. So a picture
/// is first put on a web server the person (or their company) owns, then Instagram is told
/// where it is.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Instagram {
    /// Where the picture is sent with an HTTP PUT. `{name}` is replaced by a new file name.
    pub upload_url: String,
    /// The public address of the same file once it is there, also with `{name}`.
    pub public_url: String,
    /// A header that carries the key kept as the secret `host_key` (`AccessKey`, `Authorization`).
    /// Empty: the upload address needs no key.
    pub host_header: String,
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

impl Claude {
    /// Does a chat open in the Claude desktop app (so the app has to be installed)? `Auto` means
    /// the app on Windows and the browser elsewhere.
    pub fn opens_desktop_app(&self, windows: bool) -> bool {
        match self.open_with {
            OpenWith::App => true,
            OpenWith::Browser => false,
            OpenWith::Auto => windows,
        }
    }
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

/// Most channels one connection can hold a webhook for.
pub const MAX_HOOKS: usize = 10;

/// The bot token, then one slot for each webhook link (`hook0` to `hook9`).
const DISCORD_SECRETS: [&str; 1 + MAX_HOOKS] = [
    "token", "hook0", "hook1", "hook2", "hook3", "hook4", "hook5", "hook6", "hook7", "hook8",
    "hook9",
];

/// The name of the secret that holds the webhook link of slot `slot`.
pub fn hook_secret(slot: u8) -> Option<&'static str> {
    DISCORD_SECRETS.get(1 + usize::from(slot)).copied()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiscordMode {
    /// A bot you create and add to your servers: it can list servers, channels and threads.
    /// (The mode of connections saved before webhooks existed.)
    #[default]
    Bot,
    /// A webhook link made in a channel's settings: no bot, one link for each channel.
    Webhook,
}

/// One channel reached by a webhook. The link itself is a secret, in slot `slot`.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Hook {
    pub slot: u8,
    /// What the person calls the channel ("bugs").
    pub name: String,
    /// Optional: the server it belongs to, so the list can be narrowed by server.
    pub server: String,
}

/// With a bot, the token says which servers it is in. With webhooks, each link is one channel.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Discord {
    pub mode: DiscordMode,
    /// Webhook mode: the name shown on each post. Empty keeps the webhook's own name.
    pub shown_as: String,
    /// Webhook mode: the channels.
    pub hooks: Vec<Hook>,
}

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
        Settings::Claude(_) | Settings::Slack(_) => {}
        Settings::Discord(d) => check_discord(d)?,
        Settings::Custom(cu) => check_custom(cu)?,
        Settings::X(x) => {
            if x.client_id.trim().is_empty() {
                return Err("Write the Client ID of your X app".into());
            }
            if x.redirect_port < 1024 {
                return Err("Use a port of 1024 or more for the callback address".into());
            }
        }
        Settings::Bluesky(b) => {
            if !b.handle.contains('.') {
                return Err("Write your Bluesky handle, like jane.bsky.social".into());
            }
            cargoframe_net::check_url(&b.service)
                .map_err(|e| format!("Bluesky server address: {e}"))?;
        }
        Settings::Instagram(i) => check_instagram(i)?,
        Settings::Github(g) => {
            if !repository_ok(&g.repository) {
                return Err("Write the repository as owner/name, like octo-org/app".into());
            }
            cargoframe_net::check_url(&g.api_url)
                .map_err(|e| format!("GitHub API address: {e}"))?;
            if g.api_url.trim().to_lowercase().starts_with("http://") {
                return Err("GitHub API address: it must start with https://".into());
            }
            if g.files == GithubFiles::Branch {
                let b = g.branch.trim();
                if b.is_empty()
                    || b.starts_with('-')
                    || b.contains("..")
                    || b.chars()
                        .any(|c| c.is_whitespace() || "~^:?*[\\".contains(c))
                {
                    return Err(
                        "Write a branch name for the pictures, like cargoframe-attachments".into(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn check_instagram(i: &Instagram) -> Result<(), String> {
    for (what, url) in [
        ("upload address", &i.upload_url),
        ("public address", &i.public_url),
    ] {
        if !url.contains("{name}") {
            return Err(format!(
                "The {what} must contain {{name}}, where the file name goes"
            ));
        }
        cargoframe_net::check_url(&url.replace("{name}", "x.jpg"))
            .map_err(|e| format!("The {what}: {e}"))?;
    }
    let h = i.host_header.trim();
    if h.chars().any(|c| c.is_control() || c == ':' || c == ' ') {
        return Err("The key header is a single word, like AccessKey".into());
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

fn check_discord(d: &Discord) -> Result<(), String> {
    if d.hooks.len() > MAX_HOOKS {
        return Err(format!("A connection holds {MAX_HOOKS} webhooks at most"));
    }
    let mut seen = std::collections::BTreeSet::new();
    for h in &d.hooks {
        if hook_secret(h.slot).is_none() || !seen.insert(h.slot) {
            return Err("Two channels share one webhook slot: remove one and add it again".into());
        }
        if d.mode == DiscordMode::Webhook && h.name.trim().is_empty() {
            return Err("Give each channel a name, so you can tell them apart in the list".into());
        }
    }
    if d.shown_as.chars().count() > 80 {
        return Err("The name shown on posts is 80 characters at most".into());
    }
    Ok(())
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

    #[test]
    fn only_a_chat_opened_in_the_app_needs_it_installed() {
        let claude = |open_with| Claude {
            open_with,
            ..Claude::default()
        };
        assert!(claude(OpenWith::App).opens_desktop_app(false));
        assert!(!claude(OpenWith::Browser).opens_desktop_app(true));
        assert!(claude(OpenWith::Auto).opens_desktop_app(true));
        assert!(!claude(OpenWith::Auto).opens_desktop_app(false));
    }

    #[test]
    fn discord_webhooks_need_names_and_their_own_slots() {
        let hook = |slot, name: &str| Hook {
            slot,
            name: name.into(),
            server: String::new(),
        };
        let discord = |hooks: Vec<Hook>| Connection {
            name: "D".into(),
            enabled: true,
            settings: Settings::Discord(Discord {
                mode: DiscordMode::Webhook,
                shown_as: String::new(),
                hooks,
            }),
        };
        assert!(check(&discord(vec![hook(0, "bugs"), hook(3, "chat")])).is_ok());
        assert!(check(&discord(vec![hook(0, "bugs"), hook(0, "chat")]))
            .unwrap_err()
            .contains("share one"));
        assert!(check(&discord(vec![hook(10, "far")])).is_err());
        assert!(check(&discord(vec![hook(0, " ")]))
            .unwrap_err()
            .contains("name"));
        assert_eq!(hook_secret(0), Some("hook0"));
        assert_eq!(hook_secret(9), Some("hook9"));
        assert_eq!(hook_secret(10), None);
        // The window may set the bot token and every webhook slot, and nothing else.
        assert_eq!(Kind::Discord.typed_secrets().len(), 1 + MAX_HOOKS);
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
