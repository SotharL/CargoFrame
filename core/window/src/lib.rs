//! What the CargoFrame window shows for one capture: the built-in variables, the project's
//! files, the state of every block, and the checks behind the status chips. Pure data in,
//! one serializable [`view::SessionView`] out, so the web view only has to draw it.

pub mod files;
pub mod vars;
pub mod view;
