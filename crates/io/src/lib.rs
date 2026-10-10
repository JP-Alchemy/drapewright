//! Import and export of pattern file formats.
//!
//! Importers return plain pieces in millimetres together with what they
//! learned about the file: how sure the scale is, and what was found,
//! guessed or left out. Exporters take pieces and return bytes. Nothing in
//! this crate touches a project or the browser.

pub mod import;
pub mod svg;

pub use import::{ImportError, ImportNote, ImportResult, ScaleInfo, ScaleOption, ScaleReason};
