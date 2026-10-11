//! Seams as kept in a project (`docs/file-format.md` §10.4).

use drapewright_pattern_core::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// One side of a seam: an edge of a piece, or part of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeamSide {
    pub piece: Id,
    pub edge: Id,
    /// Sewn from this far along the edge's stitching line, mm; from its
    /// start when absent.
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
            from: None,
            to: None,
            mirror: false,
        }
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

/// Two edges (or parts of edges) sewn together.
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
