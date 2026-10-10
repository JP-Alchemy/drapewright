//! The piece model. These types are the file format: their serde form is
//! what `docs/file-format.md` documents, field for field.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Pt;

/// Identifier, unique within a project.
pub type Id = String;

/// Default seam allowance when nothing else is known (1.5 cm). Provisional,
/// see `docs/open-questions.md`.
pub const DEFAULT_SEAM_ALLOWANCE: f64 = 15.0;

/// One run of an edge, continuing from where the previous segment ended.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(untagged)]
#[ts(export)]
pub enum Segment {
    Cubic(CubicSeg),
    Line(LineSeg),
}

/// Straight line to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct LineSeg {
    pub to: Pt,
}

/// Cubic Bézier with control points `c1`, `c2`, ending at `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub struct CubicSeg {
    pub c1: Pt,
    pub c2: Pt,
    pub to: Pt,
}

impl Segment {
    pub fn line(to: impl Into<Pt>) -> Self {
        Segment::Line(LineSeg { to: to.into() })
    }

    pub fn cubic(c1: impl Into<Pt>, c2: impl Into<Pt>, to: impl Into<Pt>) -> Self {
        Segment::Cubic(CubicSeg {
            c1: c1.into(),
            c2: c2.into(),
            to: to.into(),
        })
    }

    /// End point of the segment.
    pub fn to(&self) -> Pt {
        match self {
            Segment::Line(l) => l.to,
            Segment::Cubic(c) => c.to,
        }
    }

    /// Applies `f` to every point of the segment.
    pub fn map_points(&mut self, f: impl Fn(Pt) -> Pt) {
        match self {
            Segment::Line(l) => l.to = f(l.to),
            Segment::Cubic(c) => {
                c.c1 = f(c.c1);
                c.c2 = f(c.c2);
                c.to = f(c.to);
            }
        }
    }
}

/// What an edge is, in sewing terms.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EdgeKind {
    /// Sewn to another edge; gets the piece's seam allowance.
    #[default]
    Seam,
    /// Turned up and finished; gets the hem allowance, and the corners next
    /// to it are shaped to fold back.
    Hem,
    /// Placed on the fabric fold; no allowance. Must be straight.
    Fold,
}

/// How the cutting line is shaped where one edge ends and the next begins.
/// See `docs/file-format.md` §4.4.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CornerStyle {
    #[default]
    Auto,
    Mitre,
    SquareThis,
    SquareNext,
    MirrorThis,
    MirrorNext,
}

/// A run of the outline from one corner to the next, e.g. "side seam".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Edge {
    pub id: Id,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: EdgeKind,
    pub segments: Vec<Segment>,
    /// Overrides the piece allowance for this edge, in mm.
    #[serde(default)]
    pub allowance: Option<f64>,
    /// Corner at the end of this edge, where the next edge begins.
    #[serde(default)]
    pub end_corner: CornerStyle,
}

impl Edge {
    pub fn new(id: impl Into<Id>, segments: Vec<Segment>) -> Self {
        Edge {
            id: id.into(),
            name: String::new(),
            kind: EdgeKind::Seam,
            segments,
            allowance: None,
            end_corner: CornerStyle::Auto,
        }
    }

    /// End point of the edge, or `None` if it has no segments.
    pub fn end(&self) -> Option<Pt> {
        self.segments.last().map(Segment::to)
    }
}

/// A closed path made of edges, running clockwise as seen on screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Outline {
    pub start: Pt,
    pub edges: Vec<Edge>,
}

/// Whether the outline is the stitching line or the cutting line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AllowanceMode {
    /// The outline is the stitching line; allowance is added outside it.
    #[default]
    Add,
    /// The outline is the cutting line; the allowance lies inside it.
    Included,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Allowance {
    #[serde(default)]
    pub mode: AllowanceMode,
    /// Default for `seam` edges, mm.
    #[serde(default = "default_seam_allowance")]
    pub seam: f64,
    /// Default for `hem` edges, mm. `None` means the same as `seam`.
    #[serde(default)]
    pub hem: Option<f64>,
}

fn default_seam_allowance() -> f64 {
    DEFAULT_SEAM_ALLOWANCE
}

impl Default for Allowance {
    fn default() -> Self {
        Allowance {
            mode: AllowanceMode::Add,
            seam: DEFAULT_SEAM_ALLOWANCE,
            hem: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CutInstruction {
    /// `main`, `lining`, `interfacing`, `contrast`, or free text.
    #[serde(default = "default_material")]
    pub material: String,
    #[serde(default = "default_quantity")]
    pub quantity: u32,
    /// Cut as mirrored pairs. `None`: yes when `quantity` ≥ 2.
    #[serde(default)]
    pub mirrored: Option<bool>,
    /// Place the fold edge on the fabric fold. `None`: yes when the piece
    /// has a fold edge.
    #[serde(default)]
    pub on_fold: Option<bool>,
}

fn default_material() -> String {
    "main".to_string()
}

fn default_quantity() -> u32 {
    1
}

impl Default for CutInstruction {
    fn default() -> Self {
        CutInstruction {
            material: default_material(),
            quantity: 1,
            mirrored: None,
            on_fold: None,
        }
    }
}

/// The lengthwise grain, from `from` to `to`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Grainline {
    pub from: Pt,
    pub to: Pt,
    /// Napped or one-way fabric: top of the fabric towards `from`.
    #[serde(default)]
    pub one_way: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotchFrom {
    #[default]
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotchKind {
    #[default]
    Single,
    Double,
    Triple,
}

impl NotchKind {
    pub fn count(self) -> usize {
        match self {
            NotchKind::Single => 1,
            NotchKind::Double => 2,
            NotchKind::Triple => 3,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Notch {
    pub id: Id,
    /// The edge the notch sits on.
    pub edge: Id,
    /// Distance along the edge from `from`, mm.
    pub at: f64,
    #[serde(default)]
    pub from: NotchFrom,
    #[serde(default)]
    pub kind: NotchKind,
    /// Degrees away from perpendicular to the edge.
    #[serde(default)]
    pub angle: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MarkKind {
    Drill,
    Dot,
    Circle,
    Cross,
    Button,
    Buttonhole,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Mark {
    pub id: Id,
    pub kind: MarkKind,
    pub at: Pt,
    /// Diameter, mm.
    #[serde(default)]
    pub size: Option<f64>,
    /// Buttonhole length, mm.
    #[serde(default)]
    pub length: Option<f64>,
    #[serde(default)]
    pub angle: f64,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LineKind {
    #[default]
    Line,
    Fold,
    Placement,
    Stitch,
    Cut,
    LengthenShorten,
}

/// A line drawn inside a piece (fold line, placement line, lengthen/shorten…).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct InternalLine {
    pub id: Id,
    #[serde(default)]
    pub kind: LineKind,
    pub start: Pt,
    pub segments: Vec<Segment>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Label {
    pub id: Id,
    pub text: String,
    pub at: Pt,
    /// Letter height, mm.
    #[serde(default = "default_label_size")]
    pub size: f64,
    #[serde(default)]
    pub angle: f64,
}

fn default_label_size() -> f64 {
    5.0
}

/// Position of a piece on the work table: rotate about the piece origin,
/// then translate. Never changes the piece's shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Placement {
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    /// Degrees.
    #[serde(default)]
    pub rotation: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum SourceFormat {
    Svg,
    Dxf,
    Pdf,
    Soom,
    Drafted,
}

/// How the import scale was established.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScaleBasis {
    /// The file states physical units (mm, cm, in, pt…).
    DeclaredUnits,
    /// No physical units; assumed CSS pixels at 96 per inch.
    #[serde(rename = "assumed96dpi")]
    Assumed96Dpi,
    /// No physical units; assumed points at 72 per inch (Illustrator).
    #[serde(rename = "assumed72dpi")]
    Assumed72Dpi,
    /// The user measured a known length.
    Measured,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SourceScale {
    pub mm_per_unit: f64,
    pub basis: ScaleBasis,
    pub confirmed: bool,
}

/// Where a piece came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Source {
    pub format: SourceFormat,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default)]
    pub sha256: Option<String>,
    /// Element id, layer or block inside the source.
    #[serde(default, rename = "ref")]
    #[ts(rename = "ref")]
    pub reference: Option<String>,
    #[serde(default)]
    pub scale: Option<SourceScale>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Piece {
    pub id: Id,
    pub name: String,
    #[serde(default)]
    pub notes: String,
    pub outline: Outline,
    #[serde(default)]
    pub allowance: Allowance,
    #[serde(default = "default_cut")]
    pub cut: Vec<CutInstruction>,
    #[serde(default)]
    pub grainline: Option<Grainline>,
    #[serde(default)]
    pub notches: Vec<Notch>,
    #[serde(default)]
    pub marks: Vec<Mark>,
    #[serde(default)]
    pub lines: Vec<InternalLine>,
    #[serde(default)]
    pub labels: Vec<Label>,
    #[serde(default)]
    pub placement: Placement,
    #[serde(default)]
    pub source: Option<Source>,
}

fn default_cut() -> Vec<CutInstruction> {
    vec![CutInstruction::default()]
}

impl Piece {
    /// A piece with the given outline and every other field at its default.
    pub fn new(id: impl Into<Id>, name: impl Into<String>, outline: Outline) -> Self {
        Piece {
            id: id.into(),
            name: name.into(),
            notes: String::new(),
            outline,
            allowance: Allowance::default(),
            cut: default_cut(),
            grainline: None,
            notches: Vec::new(),
            marks: Vec::new(),
            lines: Vec::new(),
            labels: Vec::new(),
            placement: Placement::default(),
            source: None,
        }
    }

    /// Allowance that applies to edge `index`, in mm.
    pub fn edge_allowance(&self, index: usize) -> f64 {
        let edge = &self.outline.edges[index];
        match edge.kind {
            EdgeKind::Fold => 0.0,
            EdgeKind::Hem => edge
                .allowance
                .unwrap_or_else(|| self.allowance.hem.unwrap_or(self.allowance.seam)),
            EdgeKind::Seam => edge.allowance.unwrap_or(self.allowance.seam),
        }
    }

    pub fn has_fold(&self) -> bool {
        self.outline.edges.iter().any(|e| e.kind == EdgeKind::Fold)
    }
}
