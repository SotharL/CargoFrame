//! Context helpers that run when a trigger fires.
//!
//! Everything here is pure logic over strings: no file or network access, so it is cheap to
//! test and safe to call on any thread.
//!
//! * [`climb`]: from the focused program's path to the project folder.
//! * [`template`]: `{{rule.name}}` substitution used by text blocks, prompts and info types.
//! * [`glob`]: pattern matching and the read-only allow-list for data rules and file pickers.

pub mod climb;
pub mod glob;
pub mod template;
