//! What is under the pointer.

use kurbo::{Affine, Point};
use serde::{Deserialize, Serialize};
use soom_pattern_core::geom::point_in_polygon;
use soom_pattern_core::{Id, Piece};
use ts_rs::TS;

use crate::view::PieceView;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    tag = "kind",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Hit {
    /// Near a notch.
    Notch { piece: Id, notch: Id },
    /// Near the corner at the end of `edge`.
    Corner { piece: Id, edge: Id },
    /// Near an edge of the outline, `along` mm from its start.
    Edge {
        piece: Id,
        edge: Id,
        along: f64,
        distance: f64,
    },
    /// Inside a piece's cutting line.
    Piece { piece: Id },
}

/// Maps piece coordinates to the work table.
pub fn placement_affine(piece: &Piece) -> Affine {
    let p = piece.placement;
    Affine::translate((p.x, p.y)) * Affine::rotate(p.rotation.to_radians())
}

/// Finds what is under `world` (work-table mm) within `tolerance` mm,
/// trying the topmost piece first. Notches beat corners, corners beat
/// edges, and edges beat the inside of a piece.
pub fn hit_test<'a>(
    pieces: impl DoubleEndedIterator<Item = (&'a Piece, &'a PieceView)>,
    world: Point,
    tolerance: f64,
) -> Option<Hit> {
    for (piece, view) in pieces.rev() {
        let local = placement_affine(piece).inverse() * world;
        let outline = &piece.outline;

        let notch = piece.notches.iter().find(|n| {
            outline.edge_index(&n.edge).is_some_and(|i| {
                let geom = outline.edge_geom(i);
                let along = match n.from {
                    soom_pattern_core::NotchFrom::Start => n.at,
                    soom_pattern_core::NotchFrom::End => geom.length() - n.at,
                };
                geom.point_at(along).distance(local) <= tolerance
            })
        });
        if let Some(n) = notch {
            return Some(Hit::Notch {
                piece: piece.id.clone(),
                notch: n.id.clone(),
            });
        }

        let corner = (0..outline.edges.len())
            .find(|&i| Point::from(outline.edge_end(i)).distance(local) <= tolerance);
        if let Some(i) = corner {
            return Some(Hit::Corner {
                piece: piece.id.clone(),
                edge: outline.edges[i].id.clone(),
            });
        }

        let nearest = (0..outline.edges.len())
            .filter_map(|i| outline.edge_geom(i).nearest(local).map(|n| (i, n)))
            .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance));
        if let Some((i, n)) = nearest
            && n.distance <= tolerance
        {
            return Some(Hit::Edge {
                piece: piece.id.clone(),
                edge: outline.edges[i].id.clone(),
                along: n.along,
                distance: n.distance,
            });
        }

        if point_in_polygon(local, &view.cutting_points()) {
            return Some(Hit::Piece {
                piece: piece.id.clone(),
            });
        }
    }
    None
}
