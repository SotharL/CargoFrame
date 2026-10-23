//! Data rules: "look in this file, read this value". A rule names a file in the project folder,
//! a format and a query; running it gives one text value that text blocks, prompts and info
//! types use as `{{rule.<id>}}`.
//!
//! * [`model`]: the rule as stored in `rules/<id>.json`.
//! * [`extract`]: apply a rule to the text of a file (JSON, YAML, TOML, INI, CSV, XML, regex).
//! * [`run`]: find the file safely inside the project folder, read it with a size limit, and run
//!   every rule, never failing as a whole because one rule did.
//!
//! Rules only ever read. Nothing here can write, run a program, or leave the project folder.

pub mod extract;
pub mod model;
pub mod path;
pub mod run;

pub use extract::{check, extract, RuleError};
pub use model::{Format, Multiple, Rule, DEFAULT_MAX_BYTES, HARD_MAX_BYTES};
pub use run::{run_all, FsFiles, Outcome, ProjectFiles};
