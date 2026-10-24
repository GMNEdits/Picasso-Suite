//! Picasso Suite core: the project model, editing operations, undo history and
//! project files. No I/O beyond reading and writing project files.

pub mod edit;
pub mod history;
pub mod io;
pub mod model;
pub mod time;

pub use edit::{Edge, EditError};
pub use history::History;
pub use model::*;
pub use time::Rational;
