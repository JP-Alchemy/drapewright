//! Edits to a piece's outline that keep everything attached to it (notches,
//! corner styles) where the user put it.

use kurbo::{Affine, ParamCurve};
use thiserror::Error;

use crate::Pt;
use crate::model::{CornerStyle, Edge, Id, NotchFrom, Piece, Segment};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum OpError {
    #[error("This piece has no edge with id “{0}”.")]
    UnknownEdge(Id),
    #[error("That point is too close to a corner to split the edge there.")]
    TooCloseToCorner,
    #[error("A piece needs at least two edges to join one with the next.")]
    NothingToJoin,
}

/// Closest a split may be to either end of an edge, mm.
pub const MIN_SPLIT_DISTANCE: f64 = 0.01;

fn swap_corner_side(style: CornerStyle) -> CornerStyle {
    match style {
        CornerStyle::SquareThis => CornerStyle::SquareNext,
        CornerStyle::SquareNext => CornerStyle::SquareThis,
        CornerStyle::MirrorThis => CornerStyle::MirrorNext,
        CornerStyle::MirrorNext => CornerStyle::MirrorThis,
        other => other,
    }
}

fn reverse_segments(start: Pt, segments: &[Segment]) -> Vec<Segment> {
    // Points visited by the edge: start, then each segment's end.
    let mut points = Vec::with_capacity(segments.len() + 1);
    points.push(start);
    points.extend(segments.iter().map(Segment::to));
    segments
        .iter()
        .enumerate()
        .rev()
        .map(|(j, s)| match *s {
            Segment::Line(_) => Segment::line(points[j]),
            Segment::Cubic(c) => Segment::cubic(c.c2, c.c1, points[j]),
        })
        .collect()
}

/// Reverses the direction of the outline. Geometry, notch positions and
/// corner shapes are unchanged; only the order in which they are listed
/// changes.
pub fn reverse(piece: &mut Piece) {
    let outline = &mut piece.outline;
    let n = outline.edges.len();
    if n == 0 {
        return;
    }
    let starts: Vec<Pt> = (0..n).map(|i| outline.edge_start(i)).collect();
    let old = std::mem::take(&mut outline.edges);
    outline.edges = (0..n)
        .map(|k| {
            let src = &old[n - 1 - k];
            // The corner at the end of the reversed edge is the corner that
            // used to sit at its start, i.e. the end corner of the edge before.
            let corner_owner = &old[(2 * n - 2 - k) % n];
            Edge {
                segments: reverse_segments(starts[n - 1 - k], &src.segments),
                end_corner: swap_corner_side(corner_owner.end_corner),
                ..src.clone()
            }
        })
        .collect();
    for notch in &mut piece.notches {
        notch.from = match notch.from {
            NotchFrom::Start => NotchFrom::End,
            NotchFrom::End => NotchFrom::Start,
        };
    }
}

/// Makes the outline run clockwise on screen (positive area with y down).
/// Returns whether it had to be reversed.
pub fn normalize_orientation(piece: &mut Piece) -> bool {
    if piece.outline.signed_area() < 0.0 {
        reverse(piece);
        true
    } else {
        false
    }
}

fn edge_index(piece: &Piece, edge_id: &str) -> Result<usize, OpError> {
    piece
        .outline
        .edge_index(edge_id)
        .ok_or_else(|| OpError::UnknownEdge(edge_id.to_string()))
}

/// Splits an edge into two at distance `along` from its start. The first
/// part keeps the edge's id; the second part gets `new_id`. Notches stay
/// exactly where they were.
pub fn split_edge(piece: &mut Piece, edge_id: &str, along: f64, new_id: Id) -> Result<(), OpError> {
    let i = edge_index(piece, edge_id)?;
    let geom = piece.outline.edge_geom(i);
    let len = geom.length();
    if !(MIN_SPLIT_DISTANCE..=len - MIN_SPLIT_DISTANCE).contains(&along) {
        return Err(OpError::TooCloseToCorner);
    }
    let (k, t) = geom.locate(along);
    let edge = &piece.outline.edges[i];
    let mut first: Vec<Segment> = edge.segments[..k].to_vec();
    let mut second: Vec<Segment> = Vec::new();
    if t <= 1e-9 {
        second.extend_from_slice(&edge.segments[k..]);
    } else if t >= 1.0 - 1e-9 {
        first.push(edge.segments[k]);
        second.extend_from_slice(&edge.segments[k + 1..]);
    } else {
        let seg = geom.segs()[k];
        first.push(Segment::from_path_seg(seg.subsegment(0.0..t)));
        second.push(Segment::from_path_seg(seg.subsegment(t..1.0)));
        second.extend_from_slice(&edge.segments[k + 1..]);
    }
    if first.is_empty() || second.is_empty() {
        return Err(OpError::TooCloseToCorner);
    }

    let new_edge = Edge {
        id: new_id.clone(),
        segments: second,
        ..edge.clone()
    };
    let edge = &mut piece.outline.edges[i];
    edge.segments = first;
    edge.end_corner = CornerStyle::Auto;
    piece.outline.edges.insert(i + 1, new_edge);

    for notch in piece.notches.iter_mut().filter(|n| n.edge == edge_id) {
        let from_start = match notch.from {
            NotchFrom::Start => notch.at,
            NotchFrom::End => len - notch.at,
        };
        if from_start <= along {
            // Stays on the first part. Its far corner is gone, so measure
            // from the start.
            notch.from = NotchFrom::Start;
            notch.at = from_start;
        } else {
            notch.edge = new_id.clone();
            if notch.from == NotchFrom::Start {
                notch.at = from_start - along;
            }
        }
    }
    Ok(())
}

/// Joins an edge with the edge after it, removing the corner between them.
/// The joined edge keeps the first edge's id, name, kind and allowance.
pub fn join_with_next(piece: &mut Piece, edge_id: &str) -> Result<(), OpError> {
    let n = piece.outline.edges.len();
    if n < 2 {
        return Err(OpError::NothingToJoin);
    }
    let i = edge_index(piece, edge_id)?;
    let j = (i + 1) % n;
    let len_i = piece.outline.edge_geom(i).length();
    let len_j = piece.outline.edge_geom(j).length();
    let start_i = piece.outline.edge_start(i);
    let next = piece.outline.edges[j].clone();

    for notch in &mut piece.notches {
        if notch.edge == next.id {
            notch.edge = edge_id.to_string();
            if notch.from == NotchFrom::Start {
                notch.at += len_i;
            }
        } else if notch.edge == edge_id && notch.from == NotchFrom::End {
            notch.at += len_j;
        }
    }

    let edge = &mut piece.outline.edges[i];
    edge.segments.extend_from_slice(&next.segments);
    edge.end_corner = next.end_corner;
    if edge.name.is_empty() {
        edge.name = next.name;
    }
    piece.outline.edges.remove(j);
    if j == 0 {
        // The joined edge runs through the old start point: it now begins
        // the outline.
        piece.outline.edges.rotate_right(1);
        piece.outline.start = start_i;
    }
    Ok(())
}

/// Applies an affine map to every point of the piece's geometry (outline,
/// grainline, marks, internal lines and labels). Distances stored along
/// edges (notches) and allowances are not changed. If the map mirrors, the
/// outline is reversed so it keeps running clockwise.
pub fn transform(piece: &mut Piece, affine: Affine) {
    let f = |p: Pt| Pt::from(affine * kurbo::Point::from(p));
    let outline = &mut piece.outline;
    outline.start = f(outline.start);
    for edge in &mut outline.edges {
        for s in &mut edge.segments {
            s.map_points(f);
        }
    }
    if let Some(g) = &mut piece.grainline {
        g.from = f(g.from);
        g.to = f(g.to);
    }
    for m in &mut piece.marks {
        m.at = f(m.at);
    }
    for l in &mut piece.lines {
        l.start = f(l.start);
        for s in &mut l.segments {
            s.map_points(f);
        }
    }
    for l in &mut piece.labels {
        l.at = f(l.at);
    }
    if affine.determinant() < 0.0 {
        reverse(piece);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Notch, NotchKind, Outline};

    fn rect_piece() -> Piece {
        let outline = Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([200.0, 0.0])]),
                Edge::new("right", vec![Segment::line([200.0, 100.0])]),
                Edge::new("bottom", vec![Segment::line([0.0, 100.0])]),
                Edge::new("left", vec![Segment::line([0.0, 0.0])]),
            ],
        };
        let mut piece = Piece::new("p1", "Rectangle", outline);
        piece.notches.push(Notch {
            id: "n1".into(),
            edge: "top".into(),
            at: 150.0,
            from: NotchFrom::Start,
            kind: NotchKind::Single,
            angle: 0.0,
        });
        piece
    }

    #[test]
    fn split_moves_notches_to_the_part_they_lie_on() {
        let mut p = rect_piece();
        split_edge(&mut p, "top", 120.0, "top2".into()).unwrap();
        assert_eq!(p.outline.edges.len(), 5);
        assert_eq!(p.outline.edges[1].id, "top2");
        assert_eq!(p.notches[0].edge, "top2");
        assert!((p.notches[0].at - 30.0).abs() < 1e-9);
    }

    #[test]
    fn split_refuses_points_at_corners() {
        let mut p = rect_piece();
        assert_eq!(
            split_edge(&mut p, "top", 0.0, "x".into()),
            Err(OpError::TooCloseToCorner)
        );
        assert_eq!(
            split_edge(&mut p, "top", 200.0, "x".into()),
            Err(OpError::TooCloseToCorner)
        );
        assert_eq!(
            split_edge(&mut p, "nope", 10.0, "x".into()),
            Err(OpError::UnknownEdge("nope".into()))
        );
    }

    #[test]
    fn join_across_the_outline_start_keeps_the_outline_closed() {
        let mut p = rect_piece();
        join_with_next(&mut p, "left").unwrap();
        assert_eq!(p.outline.edges.len(), 3);
        assert_eq!(p.outline.edges[0].id, "left");
        assert_eq!(p.outline.start, Pt::new(0.0, 100.0));
        assert!(p.outline.closure_gap() < 1e-12);
        // The notch that was 150 mm along "top" is now 100 + 150 mm along "left".
        assert_eq!(p.notches[0].edge, "left");
        assert!((p.notches[0].at - 250.0).abs() < 1e-9);
    }

    #[test]
    fn reverse_keeps_corner_styles_at_the_same_corner() {
        let mut p = rect_piece();
        // Corner between "right" and "bottom".
        p.outline.edges[1].end_corner = CornerStyle::MirrorNext;
        reverse(&mut p);
        // Reversed order: left, bottom, right, top. The same corner now sits
        // between "bottom" and "right", and "bottom" is the edge before it.
        let ids: Vec<_> = p.outline.edges.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(ids, ["left", "bottom", "right", "top"]);
        assert_eq!(p.outline.edges[1].end_corner, CornerStyle::MirrorThis);
        assert!(p.outline.signed_area() < 0.0);
        assert_eq!(p.notches[0].from, NotchFrom::End);
    }

    #[test]
    fn mirroring_transform_keeps_the_outline_clockwise() {
        let mut p = rect_piece();
        transform(&mut p, Affine::FLIP_X);
        assert!(p.outline.signed_area() > 0.0);
    }
}
