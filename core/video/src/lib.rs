//! Video, the parts that do not depend on how the screen is captured.
//!
//! A recording starts on the shortcut and keeps only the last few seconds while it runs (the
//! [`buffer`]); when it stops, that is the clip. The person trims it, crops it and marks it up
//! (the [`edit`] model), and the result is written by ffmpeg ([`export`]).
//!
//! Marks are not drawn here. The editor window draws a circle or a text on a transparent picture
//! and says where and when it shows; ffmpeg lays those pictures over the clip. That way what the
//! person saw in the editor is what is exported, with no fonts or drawing code in the core.

pub mod buffer;
pub mod edit;
pub mod export;
pub mod recorder;
pub mod segments;

pub use buffer::{RollingBuffer, Window};
pub use edit::{resolve, Clip, Crop, Edit, Overlay, Problem, Resolved};
pub use export::{ask, locate, locate_in, parse_probe, plan, probe, quiet, run, ExportError, Plan};
pub use segments::{join, JoinError, Ring, Segment};
