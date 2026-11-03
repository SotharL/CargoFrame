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
    /// A plain text or log file (`.txt`, `.log`). The query is `tail: 20`, `line: -1`,
    /// `contains: text`, `after: Version:`, `between: [ | ]` or `level: error`; see `text`.
    Text,
    /// Any file, whatever it holds: the query asks for a detail of the file itself, such as
    /// `size`, `modified`, `lines` or `sha256`; see `details`.
    File,
}

/// Which part of a file a rule needs when the file is bigger than its size limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadMode {
    /// All of it, or refuse: the file is data (JSON, CSV…) and half of it is not valid.
    Whole,
    /// The first lines. A log read from the top.
    Head,
    /// The last lines. A log read from the bottom, where the latest entries are.
    Tail,
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
    /// The one at position `nth` (1 is the first, -1 the last, -2 the one before it).
    Nth,
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
    /// Which one to give when `multiple` is `Nth`: 1 is the first, -1 the last.
    pub nth: i64,
    /// For the Regex format: what to give for each match, with `$1`, `$2` or `${name}` standing
    /// for the pattern's groups (`$0` is the whole match), for example `$1.$2`. Empty gives group 1,
    /// or the whole match when the pattern has no groups.
    pub output: String,
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
            nth: 1,
            output: String::new(),
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

    /// What to read of a file bigger than the limit. Only text and log queries can work from
    /// one end of a file; every other format needs the whole of it.
    pub fn read_mode(&self) -> ReadMode {
        match self.format {
            Format::Text => crate::text::parse(&self.query)
                .map(|q| q.read_mode())
                .unwrap_or(ReadMode::Whole),
            // Details of the file itself never need its text.
            Format::File => ReadMode::Whole,
            _ => ReadMode::Whole,
        }
    }
}
