//! How high on the figure each piece goes as placed, to blend a pattern's
//! sizes by: for a piece round the trunk, the height of every point of it.

use drapewright_pattern_core::blend::Height;
use drapewright_pattern_core::stitching::stitching_edges;
use drapewright_pattern_core::{Id, Piece, Pt};

use crate::garment::{Copy, Garment, Half};
use crate::girth::round_an_arm;
use crate::place::Figure;

/// Where a piece goes on the figure.
#[derive(Clone, Debug, PartialEq)]
pub enum Worn {
    /// Round the trunk, this high.
    Trunk(Height),
    /// Round an arm: a sleeve.
    Arm,
}

/// Each placed piece's place on the figure (`points`: each panel's points,
/// as `place` puts them). Pieces not in the garment are not listed.
pub fn worn(
    g: &Garment,
    points: &[Vec<[f64; 3]>],
    pieces: &[Piece],
    figure: &Figure,
) -> Vec<(Id, Worn)> {
    let mut out: Vec<(Id, Worn)> = Vec::new();
    for (p, panel) in g.panels.iter().enumerate() {
        if panel.copy == Copy::Mirror || out.iter().any(|(id, _)| *id == panel.piece) {
            continue;
        }
        if round_an_arm(&points[p], figure) {
            out.push((panel.piece.clone(), Worn::Arm));
            continue;
        }
        let Some(piece) = pieces.iter().find(|q| q.id == panel.piece) else {
            continue;
        };
        let lines = stitching_edges(piece);
        // Each edge's ends: where they are on the piece, and how high.
        let mut ends: Vec<(f64, f64, f64)> = Vec::new();
        for edge in panel.edges.iter().filter(|e| e.half == Half::A) {
            let Some(line) = piece
                .outline
                .edge_index(&edge.edge)
                .and_then(|e| lines.get(e))
            else {
                continue;
            };
            let pairs = [
                (line.first(), edge.points.first()),
                (line.last(), edge.points.last()),
            ];
            for (at, k) in pairs {
                if let (Some(at), Some(&k)) = (at, k) {
                    ends.push((at.x, at.y, points[p][k as usize][1]));
                }
            }
        }
        if let Some(height) = fitted(&ends) {
            out.push((panel.piece.clone(), Worn::Trunk(height)));
        }
    }
    out
}

/// The height `a x + b y + c` fitted to (x, y, height) by least squares:
/// none unless it fits closely and (a, b) is a unit vector, as for a piece
/// laid flat round the body.
fn fitted(ends: &[(f64, f64, f64)]) -> Option<Height> {
    if ends.len() < 3 {
        return None;
    }
    // Centred, for a well-conditioned 2 × 2 solve.
    let n = ends.len() as f64;
    let (mx, my, mh) = ends.iter().fold((0.0, 0.0, 0.0), |s, e| {
        (s.0 + e.0 / n, s.1 + e.1 / n, s.2 + e.2 / n)
    });
    let (mut xx, mut xy, mut yy, mut xh, mut yh) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(x, y, h) in ends {
        let (x, y, h) = (x - mx, y - my, h - mh);
        xx += x * x;
        xy += x * y;
        yy += y * y;
        xh += x * h;
        yh += y * h;
    }
    let det = xx * yy - xy * xy;
    if det.abs() < 1e-6 {
        return None;
    }
    let a = (xh * yy - yh * xy) / det;
    let b = (yh * xx - xh * xy) / det;
    let c = mh - a * mx - b * my;
    let off = ends
        .iter()
        .map(|&(x, y, h)| (a * x + b * y + c - h).powi(2))
        .sum::<f64>()
        / n;
    ((a.hypot(b) - 1.0).abs() < 0.05 && off.sqrt() < 2.0).then_some(Height {
        up: Pt::new(a, b),
        at: c,
    })
}
