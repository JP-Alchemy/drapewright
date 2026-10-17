//! Moving an edge: dragged out or in on the table, the edge drawn again as
//! a parallel curve, and the edges either side carried on (or cut back) to
//! meet it.

use kurbo::offset::offset_cubic;
use kurbo::{BezPath, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Pt;
use crate::alter::{
    AlterError, Drawn, crossing, cubic, seg_len_zero, stretch, to_segments, with_start,
};
use crate::geom::{EdgeGeom, SAME_POINT, flatten_segs, outward_normal, segs_of, unit_tangent};
use crate::model::{CubicSeg, Dart, EdgeKind, LineSeg, NotchFrom, Segment};

/// How the edges either side of an edge moved out carry on to meet it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum EdgeMove {
    /// On the way they run where they meet it, as a pattern maker
    /// extends a line.
    #[default]
    Extend,
    /// Straight out, square to it: a strip as wide as the move added, with
    /// square ends.
    Extrude,
}

/// How far the edges either side may be carried on past a corner, for
/// each mm the edge moves, and a little more: beyond, they run nearly
/// along it.
const REACH_PER_MM: f64 = 10.0;
const REACH: f64 = 10.0;
/// How closely curves are followed to find where they meet, mm.
const FINE: f64 = 0.01;
/// Curves inside the edge that no longer meet once moved out are carried
/// on to a point at most this many times the move from where they ended;
/// further, they are joined straight across.
const MITRE: f64 = 4.0;

/// The shape with edge `edge` moved out by `by` mm (in where less than
/// 0), the edges either side carried on as `mode` says.
pub(crate) fn move_edge(
    shape: &Drawn,
    edge: &str,
    by: f64,
    mode: EdgeMove,
) -> Result<Drawn, AlterError> {
    let outline = &shape.outline;
    let i = outline.edge_index(edge).ok_or_else(|| AlterError::NoEdge {
        edge: edge.to_owned(),
    })?;
    let n = outline.edges.len();
    if n < 2 {
        return Err(AlterError::AllRound);
    }
    if by.abs() < 1e-9 {
        return Ok(shape.clone());
    }
    let segs = segs_of(outline.edge_start(i), &outline.edges[i].segments);
    let geom = EdgeGeom::new(segs.clone());
    let length = geom.length();
    // Its darts stay where they are, their tips too; the parts between
    // them move.
    let mut darts: Vec<(f64, f64)> = Vec::new();
    let mut on_edge: Vec<&Dart> = shape.darts.iter().filter(|d| d.edge == edge).collect();
    on_edge.sort_by(|a, b| a.from.min(a.to).total_cmp(&b.from.min(b.to)));
    for d in on_edge {
        let (a, b) = (d.from.min(d.to), d.from.max(d.to));
        let after = darts.last().map_or(0.0, |l| l.1);
        if a > after + SAME_POINT && b < length - SAME_POINT && b - a > SAME_POINT {
            darts.push((a, b));
        }
    }
    let mut bounds = vec![0.0];
    for &(a, b) in &darts {
        bounds.extend([a, b]);
    }
    bounds.push(length);
    let mut moved: Vec<Vec<PathSeg>> = bounds
        .chunks(2)
        .map(|w| offset(&stretch(&geom, &segs, w[0], w[1]), by))
        .collect();
    if moved.iter().any(Vec::is_empty) {
        return Err(AlterError::NoMeeting);
    }
    let mut legs: Vec<Vec<PathSeg>> = darts
        .iter()
        .map(|&(a, b)| stretch(&geom, &segs, a, b))
        .collect();
    let reach = REACH + REACH_PER_MM * by.abs();
    let mut runs: Vec<Vec<PathSeg>> = (0..n)
        .map(|k| segs_of(outline.edge_start(k), &outline.edges[k].segments))
        .collect();
    // Moved out square (extruded), an edge either side carries on along
    // the moved edge's normal at that end; otherwise on its own way (and a
    // fold always so, to stay straight).
    let way = |k: usize, own: Vec2, square: Vec2| {
        if by > 0.0 && mode == EdgeMove::Extrude && outline.edges[k].kind != EdgeKind::Fold {
            square
        } else {
            own
        }
    };
    let (prev, next) = ((i + n - 1) % n, (i + 1) % n);
    // The edge before meets the first part.
    let along = way(prev, end_tangent(&runs[prev]), geom.normal_at(0.0));
    let (before, first) =
        meet(&runs[prev], along, &moved[0], reach).ok_or(AlterError::NoMeeting)?;
    runs[prev] = before;
    moved[0] = first;
    // Each dart's legs meet the parts either side, carried on along
    // themselves.
    for (j, leg) in legs.iter_mut().enumerate() {
        let back = reversed(leg);
        let (l, m) = meet(&back, end_tangent(&back), &reversed(&moved[j]), reach)
            .ok_or(AlterError::NoMeeting)?;
        *leg = reversed(&l);
        moved[j] = reversed(&m);
        let (l, m) =
            meet(leg, end_tangent(leg), &moved[j + 1], reach).ok_or(AlterError::NoMeeting)?;
        *leg = l;
        moved[j + 1] = m;
    }
    // The last part meets the edge after (the same edge as before, on a
    // piece of two edges: met at its other end).
    let last = moved.len() - 1;
    let back = reversed(&runs[next]);
    let along = way(next, end_tangent(&back), geom.normal_at(length));
    let (after, end) =
        meet(&back, along, &reversed(&moved[last]), reach).ok_or(AlterError::NoMeeting)?;
    runs[next] = reversed(&after);
    moved[last] = reversed(&end);

    // The edge again, part by part, noting where its darts now open.
    let mut path: Vec<PathSeg> = Vec::new();
    let mut mouths: Vec<(f64, f64)> = Vec::new();
    let length_of = |segs: &[PathSeg]| EdgeGeom::new(segs.to_vec()).length();
    for (j, part) in moved.iter().enumerate() {
        path.extend(part);
        if let Some(leg) = legs.get(j) {
            let at = length_of(&path);
            path.extend(leg);
            mouths.push((at, at + length_of(leg)));
        }
    }
    runs[i] = path;

    let mut altered = shape.clone();
    for k in [prev, i, next] {
        altered.outline.edges[k].segments = to_segments(&runs[k]);
    }
    altered.outline.start = runs[n - 1].last().map_or(outline.start, |s| s.end().into());
    if altered.outline.signed_area() <= 0.0 {
        return Err(AlterError::InsideOut);
    }

    // Notches and darts where they were on their edges' cloth: on the
    // moved parts, moved with them.
    let after: Vec<EdgeGeom> = (0..n).map(|k| altered.outline.edge_geom(k)).collect();
    let in_dart = |s: f64| darts.iter().any(|&(a, b)| s > a && s < b);
    let mapped = |k: usize, s: f64| -> f64 {
        let before = outline.edge_geom(k);
        let mut p = before.point_at(s);
        if k == i && !in_dart(s) {
            p += before.normal_at(s) * by;
        }
        after[k].nearest(p).map_or(s, |near| near.along)
    };
    let changed = [prev, i, next];
    for notch in &mut altered.notches {
        let Some(k) = outline
            .edge_index(&notch.edge)
            .filter(|k| changed.contains(k))
        else {
            continue;
        };
        let (was, now) = (outline.edge_geom(k).length(), after[k].length());
        notch.at = match notch.from {
            NotchFrom::Start => mapped(k, notch.at),
            NotchFrom::End => now - mapped(k, was - notch.at),
        }
        .max(0.0);
    }
    for dart in &mut altered.darts {
        let Some(k) = outline
            .edge_index(&dart.edge)
            .filter(|k| changed.contains(k))
        else {
            continue;
        };
        let span = (dart.from.min(dart.to), dart.from.max(dart.to));
        let kept = darts.iter().position(|d| k == i && *d == span);
        let (from, to) = match kept {
            Some(j) => mouths[j],
            None => {
                let (a, b) = (mapped(k, dart.from), mapped(k, dart.to));
                (a.min(b), a.max(b))
            }
        };
        (dart.from, dart.to) = (from, to);
    }
    Ok(altered)
}

/// The shape with the corner at the end of edge `edge` (its start, where
/// `start`) moved by `by` (mm): the edges either side drawn to it, each of
/// their points moved by `by` times how near the corner it is along them
/// (all of it at the corner, none at their other ends). A straight edge
/// stays straight.
pub(crate) fn move_corner(
    shape: &Drawn,
    edge: &str,
    start: bool,
    by: Vec2,
) -> Result<Drawn, AlterError> {
    let outline = &shape.outline;
    let k = outline.edge_index(edge).ok_or_else(|| AlterError::NoEdge {
        edge: edge.to_owned(),
    })?;
    let n = outline.edges.len();
    if n < 2 {
        return Err(AlterError::AllRound);
    }
    // The edge it ends.
    let i = if start { (k + n - 1) % n } else { k };
    if by.hypot() < 1e-9 {
        return Ok(shape.clone());
    }
    let next = (i + 1) % n;
    // How much of `by` a point of edge `k` moves, by how far along it it is
    // (0 to 1).
    let weight = |k: usize, f: f64| if k == i { f } else { 1.0 - f };
    let shift = |p: Pt, w: f64| Pt::new(p.x + by.x * w, p.y + by.y * w);
    let mut altered = shape.clone();
    for k in [i, next] {
        let geom = outline.edge_geom(k);
        let length = geom.length().max(1e-9);
        let mut walked = 0.0;
        altered.outline.edges[k].segments = outline.edges[k]
            .segments
            .iter()
            .enumerate()
            .map(|(s, seg)| {
                let start = walked;
                walked = geom.along_at(s, 1.0);
                let (w0, w1) = (weight(k, start / length), weight(k, walked / length));
                match *seg {
                    Segment::Line(l) => Segment::Line(LineSeg {
                        to: shift(l.to, w1),
                    }),
                    // Moved in proportion along it: its control points by
                    // the thirds between.
                    Segment::Cubic(c) => Segment::Cubic(CubicSeg {
                        c1: shift(c.c1, w0 + (w1 - w0) / 3.0),
                        c2: shift(c.c2, w0 + 2.0 * (w1 - w0) / 3.0),
                        to: shift(c.to, w1),
                    }),
                }
            })
            .collect();
    }
    if next == 0 {
        altered.outline.start = shift(outline.start, 1.0);
    }
    if altered.outline.signed_area() <= 0.0 {
        return Err(AlterError::InsideOut);
    }
    // Notches and darts where they were on their edges' cloth, moved with
    // it.
    let mapped = |k: usize, s: f64| -> f64 {
        let geom = outline.edge_geom(k);
        let p = geom.point_at(s);
        let w = weight(k, s / geom.length().max(1e-9));
        let p = Point::new(p.x + by.x * w, p.y + by.y * w);
        altered
            .outline
            .edge_geom(k)
            .nearest(p)
            .map_or(s, |near| near.along)
    };
    let mut notches = altered.notches.clone();
    for notch in &mut notches {
        let Some(k) = outline
            .edge_index(&notch.edge)
            .filter(|k| *k == i || *k == next)
        else {
            continue;
        };
        let (was, now) = (
            outline.edge_geom(k).length(),
            altered.outline.edge_geom(k).length(),
        );
        notch.at = match notch.from {
            NotchFrom::Start => mapped(k, notch.at),
            NotchFrom::End => now - mapped(k, was - notch.at),
        }
        .max(0.0);
    }
    let mut darts = altered.darts.clone();
    for dart in &mut darts {
        let Some(k) = outline
            .edge_index(&dart.edge)
            .filter(|k| *k == i || *k == next)
        else {
            continue;
        };
        let (a, b) = (mapped(k, dart.from), mapped(k, dart.to));
        (dart.from, dart.to) = (a.min(b), a.max(b));
    }
    altered.notches = notches;
    altered.darts = darts;
    Ok(altered)
}

/// `segs` the other way round.
fn reversed(segs: &[PathSeg]) -> Vec<PathSeg> {
    segs.iter().rev().map(PathSeg::reverse).collect()
}

fn start_tangent(segs: &[PathSeg]) -> Vec2 {
    segs.iter()
        .map(|s| unit_tangent(s, 0.0))
        .find(|v| *v != Vec2::ZERO)
        .unwrap_or(Vec2::ZERO)
}

fn end_tangent(segs: &[PathSeg]) -> Vec2 {
    segs.iter()
        .rev()
        .map(|s| unit_tangent(s, 1.0))
        .find(|v| *v != Vec2::ZERO)
        .unwrap_or(Vec2::ZERO)
}

/// `seg` ending exactly at `p` (a point it nearly ends at).
fn with_end(seg: PathSeg, p: Point) -> PathSeg {
    match seg {
        PathSeg::Line(l) => PathSeg::Line(Line::new(l.p0, p)),
        PathSeg::Quad(q) => {
            let c = q.raise();
            cubic([c.p0, c.p1, c.p2, p])
        }
        PathSeg::Cubic(c) => cubic([c.p0, c.p1, c.p2, p]),
    }
}

/// A curve offset by `by` mm out (in, where less than 0), each point
/// square to it.
fn offset_seg(seg: &PathSeg, by: f64) -> Vec<PathSeg> {
    if seg_len_zero(seg) {
        return Vec::new();
    }
    let line = |a: Point, b: Point| {
        let out = outward_normal((b - a).normalize()) * by;
        vec![PathSeg::Line(Line::new(a + out, b + out))]
    };
    let c = match *seg {
        PathSeg::Line(l) => return line(l.p0, l.p1),
        PathSeg::Quad(q) => q.raise(),
        PathSeg::Cubic(c) => c,
    };
    // A curve drawn straight (its control points on the line between its
    // ends, in order) is a line.
    let chord = c.p3 - c.p0;
    let span = chord.hypot();
    let straight = span > SAME_POINT
        && [c.p1, c.p2].iter().all(|p| {
            let s = (*p - c.p0).dot(chord) / span;
            ((*p - c.p0).cross(chord) / span).abs() < 1e-6 && (0.0..=span).contains(&s)
        });
    if straight {
        return line(c.p0, c.p3);
    }
    // kurbo offsets to the right of the way the curve runs, as seen on
    // screen: out of a piece is to its left.
    let mut path = BezPath::new();
    offset_cubic(c, -by, FINE, &mut path);
    path.segments().filter(|s| !seg_len_zero(s)).collect()
}

/// Curves offset by `by` mm out, each where the one before ends: those
/// that no longer meet are carried on to meet (or joined straight across,
/// if they would only meet far off), and those that cross are cut where
/// they cross.
fn offset(segs: &[PathSeg], by: f64) -> Vec<PathSeg> {
    let mut out: Vec<PathSeg> = Vec::new();
    for seg in segs {
        let mut parts = offset_seg(seg, by);
        let (Some(last), Some(first)) = (out.last().copied(), parts.first().copied()) else {
            out.extend(parts);
            continue;
        };
        let (end, start) = (last.end(), first.start());
        if end.distance(start) > SAME_POINT {
            if let Some((ta, tb)) = crossing(&last, &first) {
                let cut = last.subsegment(0.0..ta);
                *out.last_mut().expect("there is a last") = cut;
                parts[0] = with_start(first.subsegment(tb..1.0), cut.end());
            } else {
                let (a, b) = (unit_tangent(&last, 1.0), unit_tangent(&first, 0.0));
                let denominator = a.cross(b);
                let corner = (denominator.abs() > 1e-9)
                    .then(|| end + a * ((start - end).cross(b) / denominator))
                    .filter(|m| {
                        (*m - end).dot(a) > 0.0
                            && m.distance(end).max(m.distance(start)) <= MITRE * by.abs()
                    });
                match corner {
                    Some(m) => {
                        match out.last_mut() {
                            Some(PathSeg::Line(l)) => l.p1 = m,
                            _ => out.push(PathSeg::Line(Line::new(end, m))),
                        }
                        match parts.first_mut() {
                            Some(PathSeg::Line(l)) => l.p0 = m,
                            _ => parts.insert(0, PathSeg::Line(Line::new(m, start))),
                        }
                    }
                    None => out.push(PathSeg::Line(Line::new(end, start))),
                }
            }
        }
        out.extend(parts);
    }
    out
}

/// Where `fixed`, curves ending at a corner, carried on past it along
/// `way` (or cut back along itself), meets `moved`, curves starting near
/// the corner, carried back from their start the way they leave it: the
/// two again, `fixed` ending and `moved` starting there. Of the places
/// they meet, the nearest the corner along `fixed` (and its carrying on),
/// within `reach` of it.
fn meet(
    fixed: &[PathSeg],
    way: Vec2,
    moved: &[PathSeg],
    reach: f64,
) -> Option<(Vec<PathSeg>, Vec<PathSeg>)> {
    let corner = fixed.last()?.end();
    let start = moved.first()?.start();
    // `fixed` as points, each with how far along it from the corner it is
    // (back along it, less than 0), and carried on.
    let points = flatten_segs(fixed, FINE);
    let mut line: Vec<(Point, f64)> = Vec::with_capacity(points.len() + 1);
    let mut s = -points.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>();
    for (k, p) in points.iter().enumerate() {
        if k > 0 {
            s += points[k - 1].distance(*p);
        }
        line.push((*p, s));
    }
    if way != Vec2::ZERO {
        line.push((corner + way * reach, reach));
    }
    // `moved` as points, carried back first.
    let back = start_tangent(moved);
    let mut other = vec![start - back * reach];
    other.extend(flatten_segs(moved, FINE));
    let mut best: Option<(f64, Point, bool)> = None;
    for f in line.windows(2) {
        for (k, m) in other.windows(2).enumerate() {
            let Some((t, p)) = cross(f[0].0, f[1].0, m[0], m[1]) else {
                continue;
            };
            let s = f[0].1 + (f[1].1 - f[0].1) * t;
            if s.abs() <= reach && best.is_none_or(|b| s.abs() < b.0.abs()) {
                best = Some((s, p, k == 0));
            }
        }
    }
    let (s, p, before_start) = best?;
    let fixed = if s <= 0.0 {
        cut_end(fixed, p)?
    } else {
        let mut longer = fixed.to_vec();
        match longer.last_mut() {
            // A line carried on along itself stays one line.
            Some(PathSeg::Line(l))
                if (l.p1 - l.p0).normalize().cross(way).abs() < 1e-9
                    && (l.p1 - l.p0).dot(way) > 0.0 =>
            {
                l.p1 = p;
            }
            _ => longer.push(PathSeg::Line(Line::new(corner, p))),
        }
        longer
    };
    let moved = if before_start {
        let mut longer = moved.to_vec();
        if p.distance(start) > SAME_POINT {
            match longer.first_mut() {
                Some(PathSeg::Line(l)) => l.p0 = p,
                _ => longer.insert(0, PathSeg::Line(Line::new(p, start))),
            }
        } else {
            longer[0] = with_start(longer[0], p);
        }
        longer
    } else {
        cut_start(moved, p)?
    };
    Some((fixed, moved))
}

/// Where segments `a0`–`a1` and `b0`–`b1` cross: how far along the first
/// (0 to 1), and the point.
fn cross(a0: Point, a1: Point, b0: Point, b1: Point) -> Option<(f64, Point)> {
    let (r, q) = (a1 - a0, b1 - b0);
    let denominator = r.cross(q);
    if denominator.abs() < 1e-12 {
        return None;
    }
    let t = (b0 - a0).cross(q) / denominator;
    let u = (b0 - a0).cross(r) / denominator;
    // Touching at an end counts.
    let within = |x: f64| (-1e-9..=1.0 + 1e-9).contains(&x);
    (within(t) && within(u)).then(|| (t.clamp(0.0, 1.0), a0 + r * t.clamp(0.0, 1.0)))
}

/// The curve nearest `p`, and where along it.
fn nearest(segs: &[PathSeg], p: Point) -> Option<(usize, f64)> {
    segs.iter()
        .enumerate()
        .map(|(k, s)| (k, s.nearest(p, 1e-9)))
        .min_by(|a, b| a.1.distance_sq.total_cmp(&b.1.distance_sq))
        .map(|(k, n)| (k, n.t))
}

/// `segs` up to `p`, on them, ending exactly there; none if nothing is
/// left.
fn cut_end(segs: &[PathSeg], p: Point) -> Option<Vec<PathSeg>> {
    let (k, t) = nearest(segs, p)?;
    let mut out = segs[..k].to_vec();
    let head = segs[k].subsegment(0.0..t);
    if !seg_len_zero(&head) && head.start().distance(p) > SAME_POINT {
        out.push(with_end(head, p));
    } else if let Some(last) = out.pop() {
        out.push(with_end(last, p));
    }
    (!out.is_empty()).then_some(out)
}

/// `segs` from `p`, on them, starting exactly there; none if nothing is
/// left.
fn cut_start(segs: &[PathSeg], p: Point) -> Option<Vec<PathSeg>> {
    let (k, t) = nearest(segs, p)?;
    let tail = segs[k].subsegment(t..1.0);
    let mut out = Vec::new();
    let rest = &segs[k + 1..];
    if !seg_len_zero(&tail) && tail.end().distance(p) > SAME_POINT {
        out.push(with_start(tail, p));
        out.extend(rest);
    } else if let Some((first, more)) = rest.split_first() {
        out.push(with_start(*first, p));
        out.extend(more);
    }
    (!out.is_empty()).then_some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_curve_is_offset_out_of_the_piece() {
        // Along the top of a piece, left to right: out is up (y down).
        let c = kurbo::CubicBez::new((0.0, 0.0), (50.0, -20.0), (150.0, -20.0), (200.0, 0.0));
        let moved = offset_seg(&PathSeg::Cubic(c), 10.0);
        let start = moved[0].start();
        let normal = outward_normal(unit_tangent(&PathSeg::Cubic(c), 0.0));
        assert!(
            start.distance(Point::ZERO + normal * 10.0) < 1e-6,
            "{start:?}"
        );
        // Every point of it 10 mm from the curve.
        for s in &moved {
            for k in 0..=10 {
                let p = s.eval(f64::from(k) / 10.0);
                let d = PathSeg::Cubic(c).nearest(p, 1e-9).distance_sq.sqrt();
                assert!((d - 10.0).abs() < 0.05, "{d}");
            }
        }
    }

    #[test]
    fn a_curve_drawn_straight_is_offset_as_a_line() {
        let c = kurbo::CubicBez::new((0.0, 0.0), (50.0, 0.0), (150.0, 0.0), (200.0, 0.0));
        let moved = offset_seg(&PathSeg::Cubic(c), 5.0);
        assert_eq!(
            moved,
            [PathSeg::Line(Line::new((0.0, -5.0), (200.0, -5.0)))]
        );
    }

    #[test]
    fn corners_inside_an_edge_meet_again_once_moved() {
        // Up and over a step: out at the outer corner, in at the inner.
        let segs = [
            PathSeg::Line(Line::new((0.0, 0.0), (100.0, 0.0))),
            PathSeg::Line(Line::new((100.0, 0.0), (100.0, -50.0))),
            PathSeg::Line(Line::new((100.0, -50.0), (200.0, -50.0))),
        ];
        let moved = offset(&segs, 10.0);
        let ends: Vec<Point> = moved.iter().map(ParamCurve::end).collect();
        assert_eq!(moved[0].start(), Point::new(0.0, -10.0));
        // In at the first corner: cut where they cross, at (90, -10).
        assert!(ends[0].distance(Point::new(90.0, -10.0)) < 1e-6, "{ends:?}");
        // Out at the second: carried on to meet at (90, -60).
        assert!(
            ends.iter()
                .any(|p| p.distance(Point::new(90.0, -60.0)) < 1e-6),
            "{ends:?}"
        );
        assert_eq!(moved.last().unwrap().end(), Point::new(200.0, -60.0));
    }
}
