//! The top level of the project file (`docs/file-format.md` §3, §6, §7).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
pub use soom_pattern_core::notches::NotchStyle;
use soom_pattern_core::{Id, Piece, Pt};
use ts_rs::TS;

pub const FORMAT: &str = "soom-project";

/// The schema version this build reads and writes.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Meta {
    pub id: String,
    pub name: String,
    /// RFC 3339, UTC.
    pub created: String,
    pub modified: String,
    #[serde(default)]
    pub notes: String,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DisplayUnits {
    Mm,
    #[default]
    Cm,
    In,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Settings {
    #[serde(default)]
    pub display_units: DisplayUnits,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum PaperName {
    #[default]
    A4,
    Letter,
    A3,
    Tabloid,
    A0,
}

impl PaperName {
    /// Width and height in portrait, mm.
    pub fn size(self) -> (f64, f64) {
        match self {
            PaperName::A4 => (210.0, 297.0),
            PaperName::Letter => (215.9, 279.4),
            PaperName::A3 => (297.0, 420.0),
            PaperName::Tabloid => (279.4, 431.8),
            PaperName::A0 => (841.0, 1189.0),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Paper {
    Named(PaperName),
    Custom { width: f64, height: f64 },
}

impl Default for Paper {
    fn default() -> Self {
        Paper::Named(PaperName::A4)
    }
}

impl Paper {
    /// Width and height in portrait, mm.
    pub fn size(self) -> (f64, f64) {
        match self {
            Paper::Named(n) => n.size(),
            Paper::Custom { width, height } => (width.min(height), width.max(height)),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Orientation {
    #[default]
    Portrait,
    Landscape,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Assembly {
    /// Cut away the border on two sides and butt pages together.
    #[default]
    Trim,
    /// Lay pages over each other by the overlap and match the marks.
    Overlap,
}

/// Where a piece goes on the printed sheet, mm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutItem {
    pub piece: Id,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub rotation: f64,
}

fn yes() -> bool {
    true
}
fn ten() -> f64 {
    10.0
}
fn fifteen() -> f64 {
    15.0
}
fn six() -> f64 {
    6.0
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrintSettings {
    #[serde(default)]
    pub paper: Paper,
    #[serde(default)]
    pub orientation: Orientation,
    /// Unprinted border on each page, mm.
    #[serde(default = "ten")]
    pub margin: f64,
    #[serde(default)]
    pub assembly: Assembly,
    #[serde(default = "fifteen")]
    pub overlap: f64,
    #[serde(default = "yes")]
    pub stitch_line: bool,
    #[serde(default)]
    pub notch_style: NotchStyle,
    #[serde(default = "six")]
    pub notch_spacing: f64,
    #[serde(default = "yes")]
    pub test_square: bool,
    #[serde(default = "yes")]
    pub page_map: bool,
    #[serde(default = "yes")]
    pub skip_empty_pages: bool,
    /// Pieces to print; `None` prints all.
    #[serde(default)]
    pub pieces: Option<Vec<Id>>,
    /// Manual arrangement; `None` lays pieces out automatically.
    #[serde(default)]
    pub layout: Option<Vec<LayoutItem>>,
}

impl Default for PrintSettings {
    fn default() -> Self {
        PrintSettings {
            paper: Paper::default(),
            orientation: Orientation::default(),
            margin: 10.0,
            assembly: Assembly::default(),
            overlap: 15.0,
            stitch_line: true,
            notch_style: NotchStyle::default(),
            notch_spacing: 6.0,
            test_square: true,
            page_map: true,
            skip_empty_pages: true,
            pieces: None,
            layout: None,
        }
    }
}

/// Where the user was looking. Not part of undo history.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ViewState {
    pub center: Pt,
    /// Multiple of true size on a calibrated screen.
    pub zoom: f64,
    #[serde(default)]
    pub selection: Vec<Id>,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            center: Pt::new(0.0, 0.0),
            zoom: 0.5,
            selection: Vec::new(),
        }
    }
}

/// Everything that undo and redo step through.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Document {
    pub meta: Meta,
    #[serde(default)]
    pub settings: Settings,
    #[ts(as = "Vec<Piece>")]
    pub pieces: Vec<Arc<Piece>>,
    #[serde(default)]
    pub print: PrintSettings,
    /// Data owned by other programs; preserved untouched.
    #[serde(default)]
    pub extensions: serde_json::Map<String, serde_json::Value>,
}

impl Document {
    pub fn piece(&self, id: &str) -> Option<&Arc<Piece>> {
        self.pieces.iter().find(|p| p.id == id)
    }

    pub fn piece_index(&self, id: &str) -> Option<usize> {
        self.pieces.iter().position(|p| p.id == id)
    }
}

/// The file as stored: `project.json`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProjectFile {
    pub format: String,
    pub schema_version: u32,
    pub generator: String,
    #[serde(flatten)]
    pub document: Document,
    #[serde(default)]
    pub view: ViewState,
}
