//! Building pieces from raw paths (imports): finding the corners that
//! split an outline into edges, and giving everything fresh ids.

use kurbo::{ParamCurve, ParamCurveArclen, PathSeg, Point};

use crate::Pt;
use crate::geom::{ARCLEN_ACCURACY, unit_tangent};
use crate::model::{Edge, Id, Outline, Piece, Segment};

/// A joint where the outline turns by more than this many degrees is a
/// corner, and starts a new edge.
pub const DEFAULT_CORNER_ANGLE: f64 = 20.0;

/// Segments shorter than this are dropped when building an outline, mm.
pub const MIN_SEGMENT_LENGTH: f64 = 1e-4;

/// Turns a closed run of curves into an outline, split into edges at the
/// joints that turn by more than `corner_angle` degrees. A smooth closed
/// curve with no corners becomes a single edge. Edges get ids from
/// `next_id("e")`.
pub fn outline_from_path(
    segs: &[PathSeg],
    corner_angle: f64,
    next_id: &mut dyn FnMut(&str) -> Id,
) -> Option<Outline> {
    let segs = usable(segs);
    if segs.is_empty() {
        return None;
    }
    let corners = corners_by_angle(&segs, corner_angle);
    Some(group_edges(&segs, &corners, next_id))
}

/// Like [`outline_from_path`], but the edges break where the file says the
/// corners are: at the joints within `tolerance` of the given points (CAD
/// "turn points"). Falls back to finding corners by angle if none of the
/// points lies on the path.
pub fn outline_from_path_at(
    segs: &[PathSeg],
    corner_points: &[Point],
    tolerance: f64,
    corner_angle: f64,
    next_id: &mut dyn FnMut(&str) -> Id,
) -> Option<Outline> {
    let segs = usable(segs);
    if segs.is_empty() {
        return None;
    }
    let mut corners: Vec<bool> = segs
        .iter()
        .map(|s| {
            corner_points
                .iter()
                .any(|c| c.distance(s.start()) <= tolerance)
        })
        .collect();
    if !corners.contains(&true) {
        corners = corners_by_angle(&segs, corner_angle);
    }
    Some(group_edges(&segs, &corners, next_id))
}

fn usable(segs: &[PathSeg]) -> Vec<PathSeg> {
    segs.iter()
        .copied()
        .filter(|s| s.arclen(ARCLEN_ACCURACY) > MIN_SEGMENT_LENGTH)
        .collect()
}

/// For each segment, whether the joint before it turns by more than
/// `corner_angle` degrees.
fn corners_by_angle(segs: &[PathSeg], corner_angle: f64) -> Vec<bool> {
    let n = segs.len();
    let threshold = corner_angle.to_radians().cos();
    (0..n)
        .map(|k| {
            let a = unit_tangent(&segs[(k + n - 1) % n], 1.0);
            let b = unit_tangent(&segs[k], 0.0);
            a.dot(b) < threshold
        })
        .collect()
}

/// Groups segments into edges, starting a new edge before every segment
/// marked as following a corner. The outline starts at the first corner.
fn group_edges(
    segs: &[PathSeg],
    is_corner: &[bool],
    next_id: &mut dyn FnMut(&str) -> Id,
) -> Outline {
    let n = segs.len();
    let first = is_corner.iter().position(|&c| c).unwrap_or(0);
    let start = Pt::from(segs[first].start());
    let mut edges: Vec<Edge> = Vec::new();
    let mut current: Vec<Segment> = Vec::new();
    for j in 0..n {
        let k = (first + j) % n;
        if j > 0 && is_corner[k] {
            edges.push(Edge::new(next_id("e"), std::mem::take(&mut current)));
        }
        current.push(Segment::from_path_seg(segs[k]));
    }
    edges.push(Edge::new(next_id("e"), current));
    // Close exactly: the last segment ends where the outline starts.
    if let Some(last) = edges.last_mut().and_then(|e| e.segments.last_mut()) {
        match last {
            Segment::Line(l) => l.to = start,
            Segment::Cubic(c) => c.to = start,
        }
    }
    Outline { start, edges }
}

/// Gives the piece and everything in it new ids from `next_id(prefix)`,
/// keeping references (notches to edges, alterations to lines) intact. The
/// piece as drawn, kept with its alterations, has the same things: they
/// get the same new ids.
pub fn reassign_ids(piece: &mut Piece, next_id: &mut dyn FnMut(&str) -> Id) {
    use std::collections::HashMap;
    piece.id = next_id("p");
    // Old id to new, by kind of thing.
    let mut renamed: HashMap<(&str, Id), Id> = HashMap::new();
    let mut rename = |kind: &'static str, id: &mut Id| {
        let new = renamed
            .entry((kind, id.clone()))
            .or_insert_with(|| next_id(kind))
            .clone();
        *id = new;
    };
    let mut shapes: Vec<&mut crate::alter::Drawn> = Vec::new();
    let mut current = crate::alter::Drawn::of(piece);
    let mut drawn = piece.drawn.take();
    shapes.push(&mut current);
    if let Some(d) = drawn.as_mut() {
        shapes.push(d);
    }
    for shape in shapes {
        for edge in &mut shape.outline.edges {
            rename("e", &mut edge.id);
        }
        for notch in &mut shape.notches {
            rename("n", &mut notch.id);
            rename("e", &mut notch.edge);
        }
        for m in &mut shape.marks {
            rename("m", &mut m.id);
        }
        for l in &mut shape.lines {
            rename("l", &mut l.id);
        }
        for t in &mut shape.labels {
            rename("t", &mut t.id);
        }
    }
    for a in &mut piece.alterations {
        rename("a", &mut a.id);
        let crate::alter::Change::Lengthen { line, .. } = &mut a.change;
        if let crate::alter::AlterLine::Own { line } = line {
            rename("l", line);
        }
    }
    current.put(piece);
    piece.drawn = drawn;
}

/// An id generator counting up from 1 per call: `"e1"`, `"e2"`, `"p3"`…
pub fn counter(prefix_scope: &str) -> impl FnMut(&str) -> Id + '_ {
    let mut n = 0u64;
    move |prefix: &str| {
        n += 1;
        format!("{prefix_scope}{prefix}{n}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{CubicBez, Line};

    fn line(a: (f64, f64), b: (f64, f64)) -> PathSeg {
        PathSeg::Line(Line::new(a, b))
    }

    #[test]
    fn splits_at_corners_and_keeps_smooth_joints() {
        // A rectangle whose top is a line followed by a tangent-continuous curve.
        let segs = [
            line((0.0, 0.0), (100.0, 0.0)),
            PathSeg::Cubic(CubicBez::new(
                (100.0, 0.0),
                (150.0, 0.0),
                (200.0, 0.0),
                (200.0, 0.0),
            )),
            line((200.0, 0.0), (200.0, 100.0)),
            line((200.0, 100.0), (0.0, 100.0)),
            line((0.0, 100.0), (0.0, 0.0)),
        ];
        let mut ids = counter("");
        let o = outline_from_path(&segs, DEFAULT_CORNER_ANGLE, &mut ids).unwrap();
        assert_eq!(o.edges.len(), 4);
        assert_eq!(o.edges[0].segments.len(), 2);
        assert!(o.closure_gap() < 1e-12);
    }

    #[test]
    fn a_smooth_closed_curve_is_one_edge() {
        let k = 0.552_284_749_830_793_4 * 50.0;
        let segs = [
            PathSeg::Cubic(CubicBez::new(
                (50.0, 0.0),
                (50.0, k),
                (k, 50.0),
                (0.0, 50.0),
            )),
            PathSeg::Cubic(CubicBez::new(
                (0.0, 50.0),
                (-k, 50.0),
                (-50.0, k),
                (-50.0, 0.0),
            )),
            PathSeg::Cubic(CubicBez::new(
                (-50.0, 0.0),
                (-50.0, -k),
                (-k, -50.0),
                (0.0, -50.0),
            )),
            PathSeg::Cubic(CubicBez::new(
                (0.0, -50.0),
                (k, -50.0),
                (50.0, -k),
                (50.0, 0.0),
            )),
        ];
        let mut ids = counter("");
        let o = outline_from_path(&segs, DEFAULT_CORNER_ANGLE, &mut ids).unwrap();
        assert_eq!(o.edges.len(), 1);
        assert_eq!(o.start, Pt::new(50.0, 0.0));
    }

    #[test]
    fn breaks_at_named_corners() {
        // Points every 25 mm round a 100 × 50 rectangle; only two are named
        // corners, so there are two edges.
        let pts = [
            (0.0, 0.0),
            (50.0, 0.0),
            (100.0, 0.0),
            (100.0, 50.0),
            (50.0, 50.0),
            (0.0, 50.0),
        ];
        let segs: Vec<PathSeg> = (0..pts.len())
            .map(|i| line(pts[i], pts[(i + 1) % pts.len()]))
            .collect();
        let mut ids = counter("");
        let corners = [Point::new(100.0, 0.0), Point::new(0.0, 50.0)];
        let o = outline_from_path_at(&segs, &corners, 0.5, DEFAULT_CORNER_ANGLE, &mut ids).unwrap();
        assert_eq!(o.edges.len(), 2);
        assert_eq!(o.start, Pt::new(100.0, 0.0));
        // No named corner lies on the path: corners by angle instead.
        let mut ids = counter("");
        let o = outline_from_path_at(
            &segs,
            &[Point::new(500.0, 500.0)],
            0.5,
            DEFAULT_CORNER_ANGLE,
            &mut ids,
        )
        .unwrap();
        assert_eq!(o.edges.len(), 4);
    }

    #[test]
    fn starts_at_a_corner() {
        // Starts mid-edge: the outline is rotated to begin at the next corner.
        let segs = [
            line((50.0, 0.0), (100.0, 0.0)),
            line((100.0, 0.0), (100.0, 100.0)),
            line((100.0, 100.0), (0.0, 100.0)),
            line((0.0, 100.0), (0.0, 0.0)),
            line((0.0, 0.0), (50.0, 0.0)),
        ];
        let mut ids = counter("");
        let o = outline_from_path(&segs, DEFAULT_CORNER_ANGLE, &mut ids).unwrap();
        assert_eq!(o.start, Pt::new(100.0, 0.0));
        assert_eq!(o.edges.len(), 4);
        // The top edge is the two halves, merged.
        assert_eq!(o.edges[3].segments.len(), 2);
    }
}
