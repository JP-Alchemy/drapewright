//! Building pieces from raw paths (imports): finding the corners that
//! split an outline into edges, and giving everything fresh ids.

use kurbo::{ParamCurve, ParamCurveArclen, PathSeg};

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
    let segs: Vec<PathSeg> = segs
        .iter()
        .copied()
        .filter(|s| s.arclen(ARCLEN_ACCURACY) > MIN_SEGMENT_LENGTH)
        .collect();
    let n = segs.len();
    if n == 0 {
        return None;
    }
    let threshold = corner_angle.to_radians().cos();
    // Corner before segment k: between the end of k-1 and the start of k.
    let is_corner: Vec<bool> = (0..n)
        .map(|k| {
            let a = unit_tangent(&segs[(k + n - 1) % n], 1.0);
            let b = unit_tangent(&segs[k], 0.0);
            a.dot(b) < threshold
        })
        .collect();
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
    Some(Outline { start, edges })
}

/// Gives the piece and everything in it new ids from `next_id(prefix)`,
/// keeping references (notches to edges) intact.
pub fn reassign_ids(piece: &mut Piece, next_id: &mut dyn FnMut(&str) -> Id) {
    piece.id = next_id("p");
    let mut renamed: Vec<(Id, Id)> = Vec::new();
    for edge in &mut piece.outline.edges {
        let new = next_id("e");
        renamed.push((std::mem::replace(&mut edge.id, new.clone()), new));
    }
    for notch in &mut piece.notches {
        notch.id = next_id("n");
        if let Some((_, new)) = renamed.iter().find(|(old, _)| *old == notch.edge) {
            notch.edge = new.clone();
        }
    }
    for m in &mut piece.marks {
        m.id = next_id("m");
    }
    for l in &mut piece.lines {
        l.id = next_id("l");
    }
    for t in &mut piece.labels {
        t.id = next_id("t");
    }
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
