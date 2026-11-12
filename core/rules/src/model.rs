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

/// When several files match a rule's file name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Several {
    /// The one changed last (the latest log).
    #[default]
    Newest,
    /// All of them: values are joined, and every one is added to the files.
    All,
}

/// Which of the places a value is found in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Which {
    #[default]
    First,
    Last,
    /// Every one, joined (lines one per line, other values with ", ").
    All,
}

/// How a value is found in a file. The file's kind is worked out from its name, so nobody has to
/// say whether it is JSON or a log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Find {
    /// The value written after a name: `GPU: RTX 4070`, `version = 3.1`, or the key `version` of
    /// a JSON, YAML, TOML, INI or XML file (`build.number` reaches inside).
    ValueOf { name: String },
    /// The text between two pieces of text on a line. No `end`: up to the end of the line.
    Between { start: String, end: Option<String> },
    /// The whole line that holds a piece of text.
    LineWith { text: String },
    /// A rule made before 0.3.0: its own format and query (the fields of [`Rule`]).
    Advanced,
    /// Made from other values, not read from a file: the first choice that fits gives the value,
    /// such as `{{rule.stream}}_{{rule.change}}_{{rule.shelf}}_{{rule.config}}` when a shelf is
    /// found, else `{{rule.stream}}_{{rule.change}}_{{rule.config}}`. See [`crate::combine`].
    Combine { choices: Vec<Choice> },
}

/// One way to make a combined value. With no conditions it fits when every value its text names
/// is found; with conditions, when all of them hold (and the text is not empty).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Choice {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub when: Vec<Cond>,
    /// May use any `{{variable}}`: values from files, the program, the project, the capture.
    pub text: String,
}

/// "{{rule.shelf}} is found": `value` is a variable's name (`rule.shelf`, braces allowed).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cond {
    pub value: String,
    pub test: CondTest,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CondTest {
    /// Has a value.
    #[default]
    Found,
    /// Has no value.
    Missing,
    /// Is this text (case does not matter).
    Is,
    IsNot,
    /// Has this text in it (case does not matter).
    Contains,
}

impl Find {
    pub fn is_combine(&self) -> bool {
        matches!(self, Find::Combine { .. })
    }
}

/// One value a rule reads, used in text as `{{rule.<key>}}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValueStep {
    /// The variable's name: `{{rule.<key>}}`. Letters, digits, `_` and `-`.
    pub key: String,
    /// What the person calls it ("GPU").
    pub name: String,
    pub find: Find,
    #[serde(default)]
    pub which: Which,
    /// Used when the file or the value is missing.
    #[serde(default)]
    pub fallback: Option<String>,
}

fn is_default<T: Default + PartialEq>(v: &T) -> bool {
    *v == T::default()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Rule {
    /// What the person calls it ("Machine config").
    pub name: String,
    /// The file to look for: its name or part of it (`system_info`, `.log`), found anywhere in the
    /// project folder; or a path with patterns (`logs/*.log`, `**/app.toml`).
    pub file: String,
    /// When several files match.
    #[serde(skip_serializing_if = "is_default")]
    pub several: Several,
    /// Tick the file (or files) in the window's Files blocks.
    #[serde(skip_serializing_if = "is_default")]
    pub add_to_files: bool,
    /// The values read from the file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub values: Vec<ValueStep>,
    /// How the lines of the file write their time (`[YYYY-MM-DD HH:mm:ss]`), so a log viewer that
    /// shows it searches by time with it, with nothing for the person to set. Empty: the viewer
    /// finds the time by itself.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub log_time: String,
    // ----- before 0.3.0 a rule read one value with a format and a query (`Find::Advanced`) -----
    pub format: Format,
    #[serde(skip_serializing_if = "String::is_empty")]
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
            several: Several::default(),
            add_to_files: false,
            values: Vec::new(),
            log_time: String::new(),
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
    /// The values this rule reads. A rule from before 0.3.0 reads one, named by the rule's id.
    pub fn steps(&self, id: &str) -> Vec<ValueStep> {
        if !self.values.is_empty() || self.query.trim().is_empty() {
            return self.values.clone();
        }
        vec![ValueStep {
            key: id.to_string(),
            name: self.name.clone(),
            find: Find::Advanced,
            which: Which::First,
            fallback: self.fallback.clone(),
        }]
    }

    /// A rule that reads no file: every value it has is made from other values.
    pub fn only_combines(&self) -> bool {
        !self.values.is_empty() && self.values.iter().all(|v| v.find.is_combine())
    }

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
