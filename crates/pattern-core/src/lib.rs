//! Geometry kernel and piece model for Drapewright.
//!
//! All lengths are millimetres as `f64`. Coordinates follow SVG: x to the
//! right, y down. Outlines run clockwise as seen on screen. See
//! `docs/file-format.md` for the data model this crate implements.

pub mod allowance;
pub mod build;
pub mod geom;
pub mod model;
pub mod notches;
pub mod ops;
mod point;
pub mod validate;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use geom::{EdgeGeom, EdgeNearest};
pub use model::*;
pub use point::Pt;
pub use validate::{Issue, IssueCode, Severity, validate_piece};

/// Version of this crate, shown in the app's About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
