use serde::{Deserialize, Serialize};

/// Files bigger than this are not read unless the rule raises its own limit.
pub const DEFAULT_MAX_BYTES: u64 = 1024 * 1024;
/// No rule may read more than this, whatever it says.
pub const HARD_MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Format {
    /// `query` is a path such as `build.number`, `$.items[0].name` or `items[*].id`.
    #[default]
    Json,
    /// Same path syntax as JSON.
    Yaml,
    /// Same path syntax as JSON.
    Toml,
    /// `section.key`, or just `key` for lines before the first section.
    Ini,
    /// A column name (from the header row) or a 1-based column number.
    Csv,
    /// Element names separated by `/`, optionally ending in `@attribute`; `//name` finds
    /// `name` at any depth.
    Xml,
    /// A regular expression. The result is capture group 1 if there is one, else the match.
    Regex,
}

/// What to do when a query finds several values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Multiple {
    #[default]
    First,
    Last,
    /// All of them, one after the other, separated by `separator`.
    Join,
}

fn default_max_bytes() -> u64 {
    DEFAULT_MAX_BYTES
}

fn default_separator() -> String {
    ", ".to_string()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// What the person calls it ("Build number").
    pub name: String,
    /// The file to read: a path relative to the project folder, `/` separated. It may use the
    /// patterns `*`, `**` and `?`; the first match in alphabetical order is read.
    pub file: String,
    pub format: Format,
    pub query: String,
    pub multiple: Multiple,
    pub separator: String,
    /// Used when the file or the value is missing. Without one, `{{rule.<id>}}` shows as
    /// unresolved, which the window points out.
    pub fallback: Option<String>,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: u64,
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            name: String::new(),
            file: String::new(),
            format: Format::default(),
            query: String::new(),
            multiple: Multiple::default(),
            separator: default_separator(),
            fallback: None,
            max_bytes: DEFAULT_MAX_BYTES,
        }
    }
}

impl Rule {
    /// The size limit actually applied.
    pub fn byte_limit(&self) -> u64 {
        self.max_bytes.clamp(1, HARD_MAX_BYTES)
    }
}
