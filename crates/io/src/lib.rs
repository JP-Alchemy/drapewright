//! Import and export of pattern file formats.
//!
//! Importers return plain pieces in millimetres together with what they
//! learned about the file: how sure the scale is, and what was found,
//! guessed or left out. Exporters take pieces and return bytes. Nothing in
//! this crate touches a project or the browser.

mod assemble;
pub mod dxf;
pub mod import;
pub mod svg;

pub use import::{
    AllowanceAnswer, AllowanceQuestion, ImportError, ImportNote, ImportResult, ScaleInfo,
    ScaleOption, ScaleReason,
};
