//! Measuring a seam: each side's length on the stitching line, and where
//! the notches of the two sides land along it.

use std::borrow::Borrow;

use drapewright_pattern_core::stitching::{
    along_polyline, closed_along, dart_spans, polyline_length, stitching_edges,
};
use drapewright_pattern_core::{AllowanceMode, Id, NotchFrom, NotchKind, Piece};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{Seam, SeamSide};

/// A notch, and how far along the seam it is from where sewing starts, mm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotchAt {
    pub notch: Id,
    pub at: f64,
}

/// Notches that should meet when the seam is sewn: one from each side, or
/// one left without a partner.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotchMeeting {
    pub a: Option<NotchAt>,
    pub b: Option<NotchAt>,
    /// How far apart they land (`b` − `a`), mm, when there are both.
    pub apart: Option<f64>,
}

/// Why a seam can't be measured.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum SeamProblem {
    /// A piece it sews is gone.
    MissingPiece { piece: Id },
    /// An edge it sews is gone.
    MissingEdge { piece: Id, edge: Id },
    /// The part of the edge it sews is not all on the edge.
    OutsideEdge { piece: Id, edge: Id },
}

/// What a seam measures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeamReport {
    pub seam: Id,
    /// Each side's length on the stitching line, mm.
    pub a_length: f64,
    pub b_length: f64,
    /// `b_length` − `a_length`, mm: the ease to spread when positive.
    pub difference: f64,
    pub notches: Vec<NotchMeeting>,
    pub problem: Option<SeamProblem>,
}

/// One side, measured: its length and its notches in sewing order.
struct Measured {
    length: f64,
    notches: Vec<NotchAt>,
}

/// Notches this far apart or closer may be partners when the two sides
/// have different numbers of them, mm.
const PARTNER_REACH: f64 = 30.0;

/// Notches this close are on the same place, mm.
const SAME_PLACE: f64 = 1.5;

/// Measures a seam among `pieces`.
pub fn measure<P: Borrow<Piece>>(seam: &Seam, pieces: &[P]) -> SeamReport {
    let a = measure_side(&seam.a, pieces, false);
    let b = measure_side(&seam.b, pieces, seam.reversed);
    match (a, b) {
        (Ok(a), Ok(b)) => SeamReport {
            seam: seam.id.clone(),
            a_length: a.length,
            b_length: b.length,
            difference: b.length - a.length,
            notches: meet(a.notches, b.notches),
            problem: None,
        },
        (Err(problem), _) | (_, Err(problem)) => SeamReport {
            seam: seam.id.clone(),
            a_length: 0.0,
            b_length: 0.0,
            difference: 0.0,
            notches: Vec::new(),
            problem: Some(problem),
        },
    }
}

/// The length a side sews, mm, if it can be measured.
pub fn side_length<P: Borrow<Piece>>(side: &SeamSide, pieces: &[P]) -> Option<f64> {
    measure_side(side, pieces, false).ok().map(|m| m.length)
}

/// A side's length, and its notches measured from where sewing starts:
/// its start, or its end when sewn the other way.
fn measure_side<P: Borrow<Piece>>(
    side: &SeamSide,
    pieces: &[P],
    from_end: bool,
) -> Result<Measured, SeamProblem> {
    let piece = pieces
        .iter()
        .map(Borrow::borrow)
        .find(|p| p.id == side.piece)
        .ok_or_else(|| SeamProblem::MissingPiece {
            piece: side.piece.clone(),
        })?;
    let index = piece
        .outline
        .edge_index(&side.edge)
        .ok_or_else(|| SeamProblem::MissingEdge {
            piece: side.piece.clone(),
            edge: side.edge.clone(),
        })?;
    let stitching = stitching_edges(piece).swap_remove(index);
    let full = polyline_length(&stitching);
    let (from, to) = (side.from.unwrap_or(0.0), side.to.unwrap_or(full));
    // A tenth of a millimetre either way is the same place.
    if from < -0.1 || to > full + 0.1 || from >= to {
        return Err(SeamProblem::OutsideEdge {
            piece: side.piece.clone(),
            edge: side.edge.clone(),
        });
    }
    let geom = piece.outline.edge_geom(index);
    let outline_length = geom.length();
    // Measured as sewn, its darts closed.
    let darts = dart_spans(piece, index, &stitching);
    let closed = |along: f64| closed_along(&darts, along);
    let mut notches: Vec<(NotchKind, NotchAt)> = piece
        .notches
        .iter()
        .filter(|n| n.edge == side.edge)
        .filter_map(|n| {
            let on_outline = match n.from {
                NotchFrom::Start => n.at,
                NotchFrom::End => outline_length - n.at,
            };
            let on_stitching = match piece.allowance.mode {
                AllowanceMode::Add => on_outline,
                AllowanceMode::Included => along_polyline(&stitching, geom.point_at(on_outline)),
            };
            (from - 0.1..=to + 0.1).contains(&on_stitching).then(|| {
                let at = if from_end {
                    closed(to) - closed(on_stitching)
                } else {
                    closed(on_stitching) - closed(from)
                };
                (
                    n.kind,
                    NotchAt {
                        notch: n.id.clone(),
                        at,
                    },
                )
            })
        })
        .collect();
    notches.sort_by(|a, b| a.1.at.total_cmp(&b.1.at));
    // Notches on the same place (drawn on the cutting line and again on
    // the stitching line) as one.
    notches.dedup_by(|later, earlier| {
        later.0 == earlier.0 && (later.1.at - earlier.1.at).abs() <= SAME_PLACE
    });
    let length = closed(to) - closed(from);
    let part = side.from.is_some() || side.to.is_some();
    let notches = notches
        .into_iter()
        .map(|(_, n)| n)
        // Where part of an edge starts or stops, its notch marks the place
        // the part meets the next: nothing to match.
        .filter(|n| !part || (SAME_PLACE..=length - SAME_PLACE).contains(&n.at))
        .collect();
    Ok(Measured { length, notches })
}

/// The notches of the two sides paired up: in order when there are as many
/// on each side, else each with the nearest free one within reach.
fn meet(a: Vec<NotchAt>, b: Vec<NotchAt>) -> Vec<NotchMeeting> {
    let both = |a: NotchAt, b: NotchAt| NotchMeeting {
        apart: Some(b.at - a.at),
        a: Some(a),
        b: Some(b),
    };
    if a.len() == b.len() {
        return a.into_iter().zip(b).map(|(a, b)| both(a, b)).collect();
    }
    let mut free: Vec<Option<NotchAt>> = b.into_iter().map(Some).collect();
    let mut out = Vec::new();
    for na in a {
        let nearest = free
            .iter()
            .enumerate()
            .filter_map(|(k, nb)| nb.as_ref().map(|nb| (k, (nb.at - na.at).abs())))
            .filter(|(_, d)| *d <= PARTNER_REACH)
            .min_by(|x, y| x.1.total_cmp(&y.1));
        match nearest.and_then(|(k, _)| free[k].take()) {
            Some(nb) => out.push(both(na, nb)),
            None => out.push(NotchMeeting {
                a: Some(na),
                b: None,
                apart: None,
            }),
        }
    }
    out.extend(free.into_iter().flatten().map(|nb| NotchMeeting {
        a: None,
        b: Some(nb),
        apart: None,
    }));
    out.sort_by(|x, y| {
        let at = |m: &NotchMeeting| m.a.as_ref().or(m.b.as_ref()).map_or(0.0, |n| n.at);
        at(x).total_cmp(&at(y))
    });
    out
}
