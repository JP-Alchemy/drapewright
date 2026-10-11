//! Import and export of pattern file formats.
//!
//! Importers return plain pieces in millimetres together with what they
//! learned about the file: how sure the scale is, and what was found,
//! guessed or left out. Exporters take pieces, or for printing the whole
//! document, and return bytes. Nothing in this crate touches the browser.

mod assemble;
pub mod dxf;
pub mod import;
pub mod pdf;
pub mod svg;

pub use import::{
    AllowanceAnswer, AllowanceQuestion, ImportError, ImportNote, ImportResult, ImportSizes,
    PieceSize, ScaleInfo, ScaleOption, ScaleReason, SizeSwatch,
};
