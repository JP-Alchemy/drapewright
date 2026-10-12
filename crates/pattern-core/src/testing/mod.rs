//! Random pieces for property-based tests, shared with other crates through
//! the `testing` feature.

use std::f64::consts::TAU;

pub mod garments;

use proptest::prelude::*;

use crate::Pt;
use crate::geom::flatten_segs;
use crate::model::{CornerStyle, Edge, EdgeKind, Outline, Piece, Segment};
use crate::validate::self_intersects;

/// Shape parameters for one vertex of a random outline.
#[derive(Clone, Debug)]
struct Vertex {
    /// Position of the vertex within its angular sector, 0..1.
    angle: f64,
    radius: f64,
    /// Curve the segment leaving this vertex, with these bulges (fractions
    /// of the chord length) at its two control points.
    curve: Option<(f64, f64)>,
    /// Start a new edge at this vertex.
    corner: bool,
}

fn arb_vertex(max_bulge: f64) -> impl Strategy<Value = Vertex> {
    (
        0.15f64..0.85,
        80.0f64..300.0,
        prop::option::weighted(0.6, (-max_bulge..max_bulge, -max_bulge..max_bulge)),
        prop::bool::weighted(0.5),
    )
        .prop_map(|(angle, radius, curve, corner)| Vertex {
            angle,
            radius,
            curve,
            corner,
        })
}

fn build_outline(vertices: &[Vertex]) -> Outline {
    let n = vertices.len();
    // Increasing angle with y down runs clockwise on screen.
    let points: Vec<Pt> = vertices
        .iter()
        .enumerate()
        .map(|(k, v)| {
            let theta = TAU * (k as f64 + v.angle) / n as f64;
            Pt::new(v.radius * theta.cos(), v.radius * theta.sin())
        })
        .collect();
    let mut edges = Vec::new();
    let mut current = Vec::new();
    for k in 0..n {
        let (a, b) = (points[k], points[(k + 1) % n]);
        let seg = match vertices[k].curve {
            None => Segment::line(b),
            Some((h1, h2)) => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                // Outward normal scaled by the chord length.
                let (nx, ny) = (dy, -dx);
                Segment::cubic(
                    [a.x + dx / 3.0 + nx * h1, a.y + dy / 3.0 + ny * h1],
                    [
                        a.x + 2.0 * dx / 3.0 + nx * h2,
                        a.y + 2.0 * dy / 3.0 + ny * h2,
                    ],
                    b,
                )
            }
        };
        current.push(seg);
        if k + 1 == n || vertices[k + 1].corner {
            edges.push(Edge::new(
                format!("e{}", edges.len() + 1),
                std::mem::take(&mut current),
            ));
        }
    }
    Outline {
        start: points[0],
        edges,
    }
}

fn is_simple_and_clockwise(outline: &Outline) -> bool {
    outline.signed_area() > 0.0 && !self_intersects(&flatten_segs(&outline.path_segs(), 0.01))
}

/// A simple outline: a star-shaped polygon whose sides may bulge in or out,
/// grouped into 1–9 edges. Never crosses itself; always clockwise.
pub fn arb_outline() -> impl Strategy<Value = Outline> {
    prop::collection::vec(arb_vertex(0.3), 3..=9)
        .prop_map(|v| build_outline(&v))
        .prop_filter(
            "outline must be simple and clockwise",
            is_simple_and_clockwise,
        )
}

/// Like [`arb_outline`] but with gentle curves only, so the curvature
/// radius stays well above typical seam allowances.
pub fn arb_gentle_outline() -> impl Strategy<Value = Outline> {
    prop::collection::vec(arb_vertex(0.08), 4..=9)
        .prop_map(|v| build_outline(&v))
        .prop_filter(
            "outline must be simple and clockwise",
            is_simple_and_clockwise,
        )
}

fn arb_corner() -> impl Strategy<Value = CornerStyle> {
    prop_oneof![
        4 => Just(CornerStyle::Auto),
        2 => Just(CornerStyle::Mitre),
        1 => Just(CornerStyle::SquareThis),
        1 => Just(CornerStyle::SquareNext),
        1 => Just(CornerStyle::MirrorThis),
        1 => Just(CornerStyle::MirrorNext),
    ]
}

/// Per-edge sewing attributes to lay over an outline.
#[derive(Clone, Debug)]
struct EdgeTraits {
    kind: u8,
    allowance: Option<f64>,
    corner: CornerStyle,
}

fn arb_traits() -> impl Strategy<Value = EdgeTraits> {
    (
        0u8..10,
        prop::option::weighted(0.5, 0.0f64..30.0),
        arb_corner(),
    )
        .prop_map(|(kind, allowance, corner)| EdgeTraits {
            kind,
            allowance,
            corner,
        })
}

fn dress(outline: Outline, seam: f64, traits: Vec<EdgeTraits>) -> Piece {
    let mut piece = Piece::new("p1", "Random piece", outline);
    piece.allowance.seam = seam;
    for (edge, t) in piece.outline.edges.iter_mut().zip(traits) {
        let straight = edge.segments.len() == 1 && matches!(edge.segments[0], Segment::Line(_));
        edge.kind = match t.kind {
            0 | 1 => EdgeKind::Hem,
            2 if straight => EdgeKind::Fold,
            _ => EdgeKind::Seam,
        };
        edge.allowance = t.allowance;
        edge.end_corner = t.corner;
    }
    piece
}

/// A random piece with random edge kinds, allowances and corner styles.
pub fn arb_piece() -> impl Strategy<Value = Piece> {
    (
        arb_outline(),
        0.0f64..25.0,
        prop::collection::vec(arb_traits(), 9),
    )
        .prop_map(|(o, seam, traits)| dress(o, seam, traits))
}

/// Like [`arb_piece`] but with gentle curves (see [`arb_gentle_outline`]).
pub fn arb_gentle_piece() -> impl Strategy<Value = Piece> {
    (
        arb_gentle_outline(),
        0.0f64..25.0,
        prop::collection::vec(arb_traits(), 9),
    )
        .prop_map(|(o, seam, traits)| dress(o, seam, traits))
}
