//! Seams as kept in a project (`docs/file-format.md` §10.4).

use drapewright_pattern_core::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One side of a seam: an edge of a piece, or part of it; or part of a line
/// drawn inside a piece, where another piece is sewn onto its face (a
/// patch pocket onto its placement).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeamSide {
    pub piece: Id,
    /// The edge sewn; empty where `line` is sewn instead.
    #[serde(default)]
    pub edge: Id,
    /// A line inside the piece (one of its `lines`) sewn instead of an
    /// edge. Added in schema version 8.
    #[serde(default)]
    pub line: Option<Id>,
    /// Sewn from this far along the edge's stitching line (or along the
    /// line, as drawn), mm; from its start when absent.
    #[serde(default)]
    pub from: Option<f64>,
    /// Sewn up to this far along, mm; to its end when absent.
    #[serde(default)]
    pub to: Option<f64>,
    /// The piece's mirror copy, its other half: a back cut as a pair is
    /// sewn to itself at the centre back.
    #[serde(default)]
    pub mirror: bool,
}

impl SeamSide {
    pub fn whole(piece: impl Into<Id>, edge: impl Into<Id>) -> Self {
        SeamSide {
            piece: piece.into(),
            edge: edge.into(),
            line: None,
            from: None,
            to: None,
            mirror: false,
        }
    }

    /// Part of a line inside a piece, from and to mm along it.
    pub fn on_line(piece: impl Into<Id>, line: impl Into<Id>, from: f64, to: f64) -> Self {
        SeamSide {
            piece: piece.into(),
            edge: Id::new(),
            line: Some(line.into()),
            from: Some(from),
            to: Some(to),
            mirror: false,
        }
    }

    /// Whether it sews a line inside its piece rather than an edge.
    pub fn is_line(&self) -> bool {
        self.line.is_some()
    }

    /// Whether it sews this edge of this piece.
    pub fn sews_edge(&self, piece: &str, edge: &str) -> bool {
        self.line.is_none() && self.piece == piece && self.edge == edge
    }
}

/// What happens to a difference in length between the sides.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Ease {
    /// Spread evenly along the seam.
    #[default]
    Even,
}

fn yes() -> bool {
    true
}

/// Two edges (or parts of edges) sewn together, or an edge sewn onto a
/// line inside another piece.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Seam {
    pub id: Id,
    /// "Side seam"; empty for none.
    #[serde(default)]
    pub name: String,
    pub a: SeamSide,
    pub b: SeamSide,
    /// `a`'s start is sewn to `b`'s end, not its start.
    #[serde(default)]
    pub reversed: bool,
    #[serde(default)]
    pub ease: Ease,
    /// Confirmed by hand. Suggestions are not, until then.
    #[serde(default = "yes")]
    pub confirmed: bool,
}

impl Seam {
    /// Whether the seam uses this piece.
    pub fn touches(&self, piece: &str) -> bool {
        self.a.piece == piece || self.b.piece == piece
    }
}
