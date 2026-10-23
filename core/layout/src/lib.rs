//! The layout of a CargoFrame window: blocks on a 12-column grid, and the checks the builder
//! shows as status chips.
//!
//! Each trigger owns one layout. A new layout is empty. The model has no file or UI code; the
//! config crate adds JSON on top and the Svelte builder edits the same shape.

pub mod grid;
pub mod model;
pub mod validate;

pub use model::*;
pub use validate::{
    validate, Code, ConnectionInfo, ConnectionKind, ConnectionState, Issue, Registry, Severity,
    SharingState,
};

/// Version written into every layout file so later releases can migrate old ones.
pub const SCHEMA_VERSION: u32 = 1;

/// Columns in the builder grid.
pub const GRID_COLUMNS: u32 = 12;
