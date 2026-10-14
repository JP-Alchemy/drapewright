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
    // An outline with no corners turns at its rounded ones (a collar's).
    let (segs, corners) = if corners.iter().filter(|&&c| c).count() < 2 {
        at_bends(segs, corners)
    } else {
        (segs, corners)
    };
    Some(group_edges(&segs, &corners, next_id))
}

/// An angle brought within half a turn either way, radians.
fn wrap(angle: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    (angle + PI).rem_euclid(TAU) - PI
}

/// An outline turning this many degrees…
const BEND: f64 = 60.0;
/// …within this far along it turns a corner, rounded though it is, mm.
const BEND_REACH: f64 = 30.0;

/// An outline's rounded corners (a collar's ends): where it turns by
/// `BEND` within `BEND_REACH`, each at the middle of its bend, a curve cut
/// there (the outline the same) to start an edge.
fn at_bends(segs: Vec<PathSeg>, corners: Vec<bool>) -> (Vec<PathSeg>, Vec<bool>) {
    // Samples about a millimetre apart: (way along, heading, segment, t).
    let mut samples: Vec<(f64, f64, usize, f64)> = Vec::new();
    let mut along = 0.0;
    let mut heading: Option<f64> = None;
    let mut turned = 0.0;
    for (k, seg) in segs.iter().enumerate() {
        let length = seg.arclen(ARCLEN_ACCURACY);
        let n = (length.ceil() as usize).max(1);
        for i in 0..n {
            let t = i as f64 / n as f64;
            let d = unit_tangent(seg, t);
            let angle = d.y.atan2(d.x);
            if let Some(h) = heading {
                turned += wrap(angle - h);
            }
            heading = Some(angle);
            samples.push((along + length * t, turned, k, t));
        }
        along += length;
    }
    let total = along;
    let m = samples.len();
    if m < 3 || total < 3.0 * BEND_REACH {
        return (segs, corners);
    }
    // How far the outline turns over the reach centred on each sample, round
    // past its start: once round, it has turned by its last heading's turn
    // back to its first more than at its last sample.
    let back = {
        let d = unit_tangent(&segs[0], 0.0);
        wrap(d.y.atan2(d.x) - heading.unwrap_or(0.0))
    };
    let full = samples[m - 1].1 + back;
    let at = |i: isize| -> (f64, f64) {
        let wraps = i.div_euclid(m as isize) as f64;
        let s = &samples[i.rem_euclid(m as isize) as usize];
        (s.0 + wraps * total, s.1 + wraps * full)
    };
    let turn_round = |i: usize| -> f64 {
        let (s, _) = at(i as isize);
        let (mut lo, mut hi) = (i as isize, i as isize);
        while at(lo - 1).0 >= s - BEND_REACH / 2.0 && (i as isize - lo) < m as isize {
            lo -= 1;
        }
        while at(hi + 1).0 <= s + BEND_REACH / 2.0 && (hi - i as isize) < m as isize {
            hi += 1;
        }
        (at(hi).1 - at(lo).1).abs()
    };
    let turns: Vec<f64> = (0..m).map(turn_round).collect();
    let bend = BEND.to_radians();
    // Each run of samples turning that much: its sharpest.
    let mut bends: Vec<usize> = Vec::new();
    let start = (0..m).find(|&i| turns[i] < bend);
    let Some(start) = start else {
        return (segs, corners);
    };
    let mut best: Option<usize> = None;
    for step in 1..=m {
        let i = (start + step) % m;
        if turns[i] >= bend {
            if best.is_none_or(|b| turns[i] > turns[b]) {
                best = Some(i);
            }
        } else if let Some(b) = best.take() {
            bends.push(b);
        }
    }
    if bends.len() < 2 {
        return (segs, corners);
    }
    // Cut the curves there, and start edges.
    let mut cuts: Vec<Vec<f64>> = vec![Vec::new(); segs.len()];
    for &b in &bends {
        let (_, _, k, t) = samples[b];
        cuts[k].push(t);
    }
    let mut out_segs = Vec::new();
    let mut out_corners = Vec::new();
    for (k, seg) in segs.iter().enumerate() {
        let mut ts = cuts[k].clone();
        ts.sort_by(f64::total_cmp);
        let mut from = 0.0;
        let mut corner = corners[k] || ts.first().is_some_and(|t| *t <= 1e-9);
        for t in ts.into_iter().filter(|t| *t > 1e-9) {
            out_segs.push(seg.subsegment(from..t));
            out_corners.push(corner);
            from = t;
            corner = true;
        }
        out_segs.push(seg.subsegment(from..1.0));
        out_corners.push(corner);
    }
    (out_segs, out_corners)
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

/// Segments shorter than this neither start an edge nor turn a corner (a
/// hair where two strokes were joined, a dash's stub), mm: they go with the
/// edge before them.
const SPECK: f64 = 0.5;

/// For each segment, whether the joint before it turns by more than
/// `corner_angle` degrees.
fn corners_by_angle(segs: &[PathSeg], corner_angle: f64) -> Vec<bool> {
    let n = segs.len();
    let threshold = corner_angle.to_radians().cos();
    let long: Vec<bool> = segs
        .iter()
        .map(|s| s.arclen(ARCLEN_ACCURACY) >= SPECK)
        .collect();
    if !long.contains(&true) {
        return (0..n)
            .map(|k| {
                let a = unit_tangent(&segs[(k + n - 1) % n], 1.0);
                let b = unit_tangent(&segs[k], 0.0);
                a.dot(b) < threshold
            })
            .collect();
    }
    (0..n)
        .map(|k| {
            if !long[k] {
                return false;
            }
            // The way the outline came in: along the last segment before
            // this one that is no speck.
            let mut j = (k + n - 1) % n;
            while !long[j] {
                j = (j + n - 1) % n;
            }
            let a = unit_tangent(&segs[j], 1.0);
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
        // A dart a step made is named after it ("a1.dart"), and is renamed
        // with it.
        if kind == "d"
            && let Some(step) = id.strip_suffix(".dart")
            && let Some(new) = renamed.get(&("a", step.to_owned()))
        {
            *id = format!("{new}.dart");
            return;
        }
        let new = renamed
            .entry((kind, id.clone()))
            .or_insert_with(|| next_id(kind))
            .clone();
        *id = new;
    };
    // The steps first, for the darts they made.
    for a in &mut piece.alterations {
        rename("a", &mut a.id);
    }
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
        for dart in &mut shape.darts {
            rename("d", &mut dart.id);
            rename("e", &mut dart.edge);
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
        if let Some(line) = a.change.own_line() {
            rename("l", line);
        }
        if let Some(dart) = a.change.own_dart() {
            rename("d", dart);
        }
        if let Some(edge) = a.change.own_edge() {
            rename("e", edge);
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
    fn a_hair_where_strokes_meet_starts_no_edge() {
        // A rectangle whose side was drawn in two strokes overlapping by a
        // fraction of a millimetre: a hair jutting off sideways.
        let segs = [
            line((0.0, 0.0), (100.0, 0.0)),
            line((100.0, 0.0), (100.0, 25.0)),
            line((100.0, 25.0), (100.3, 25.1)),
            line((100.3, 25.1), (100.0, 25.2)),
            line((100.0, 25.2), (100.0, 50.0)),
            line((100.0, 50.0), (0.0, 50.0)),
            line((0.0, 50.0), (0.0, 0.0)),
        ];
        let mut ids = counter("");
        let o = outline_from_path(&segs, DEFAULT_CORNER_ANGLE, &mut ids).unwrap();
        assert_eq!(o.edges.len(), 4);
    }

    #[test]
    fn rounded_corners_split_an_outline_with_no_sharp_ones() {
        // A stand collar 500 by 50 mm, every corner rounded (a quarter
        // circle 12 mm round): no corner turns at a point, but its edges
        // are its four sides.
        let (w, h, r) = (500.0, 50.0, 12.0);
        let k = 0.552_284_749_830_793_4 * r;
        let arc = |from: (f64, f64), c1: (f64, f64), c2: (f64, f64), to: (f64, f64)| {
            PathSeg::Cubic(CubicBez::new(from, c1, c2, to))
        };
        let segs = [
            line((r, 0.0), (w - r, 0.0)),
            arc((w - r, 0.0), (w - r + k, 0.0), (w, r - k), (w, r)),
            line((w, r), (w, h - r)),
            arc((w, h - r), (w, h - r + k), (w - r + k, h), (w - r, h)),
            line((w - r, h), (r, h)),
            arc((r, h), (r - k, h), (0.0, h - r + k), (0.0, h - r)),
            line((0.0, h - r), (0.0, r)),
            arc((0.0, r), (0.0, r - k), (r - k, 0.0), (r, 0.0)),
        ];
        let mut ids = counter("");
        let o = outline_from_path(&segs, DEFAULT_CORNER_ANGLE, &mut ids).unwrap();
        assert_eq!(o.edges.len(), 4);
        // Each corner at its rounding's middle: the long sides as long as
        // the side and two halves of a rounding.
        let quarter = std::f64::consts::FRAC_PI_2 * r;
        let mut long: Vec<f64> = (0..4).map(|i| o.edge_geom(i).length()).collect();
        long.sort_by(f64::total_cmp);
        for side in &long[2..] {
            assert!((side - (w - 2.0 * r + quarter)).abs() < 1.0, "{long:?}");
        }
        // The outline is the same: as long round.
        let round: f64 = long.iter().sum();
        assert!((round - (2.0 * (w + h) - 8.0 * r + 4.0 * quarter)).abs() < 0.5);
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
