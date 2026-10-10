//! Geometry kernel and piece model for Soom.
//!
//! All lengths are millimetres as `f64`. Coordinates follow SVG: x to the
//! right, y down. See `docs/file-format.md` for the data model this crate
//! implements.

/// Version of this crate, shown in the app's About panel.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
