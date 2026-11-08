//! Data from files: "if this file is found, read these values (and add it to the files)". A rule
//! names a file in the project folder by its name or part of it, and lists values to read, each
//! found by a name (`GPU: …`, a JSON key), the text around it, or the line that holds it. The
//! file's kind is worked out from its name. Each value is used in text as `{{rule.<key>}}`.
//!
//! * [`model`]: the rule as stored in `rules/<id>.json`.
//! * [`details`]: facts about a file itself (size, modified date, line count, fingerprint).
//! * [`extract`]: apply a rule to the text of a file (JSON, YAML, TOML, INI, CSV, XML, regex, and plain text or log files).
//! * [`run`]: find the file safely inside the project folder, read it with a size limit, and run
//!   every rule, never failing as a whole because one rule did.
//!
//! Rules only ever read. Nothing here can write, run a program, or leave the project folder.

pub mod details;
pub mod extract;
pub mod find;
pub mod model;
pub mod path;
pub mod run;
pub mod text;

pub use extract::{check, extract, RuleError};
pub use model::{
    Find, Format, Multiple, ReadMode, Rule, Several, ValueStep, Which, DEFAULT_MAX_BYTES,
    HARD_MAX_BYTES,
};
pub use run::{peek, run_all, run_rules, FsFiles, Outcome, Peek, ProjectFiles, Report};
