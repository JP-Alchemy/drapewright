//! Each edge's own part of the stitching line. Seams are measured and
//! matched on the stitching line, so where an outline is the cutting line
//! (allowance included) its edges are moved in by their allowance first.

use kurbo::{Point, Vec2};

use crate::allowance::MITRE_LIMIT;
use crate::geom::{flatten_segs, outward_normal, segs_of};
use crate::model::{AllowanceMode, InternalLine, Piece};

/// How closely edges are followed, mm.
const TOLERANCE: f64 = 0.01;

/// The stitching line of each edge, in outline order and direction. For an
/// outline that is the stitching line (allowance added) that is each edge
/// itself. For one that is the cutting line (allowance included) it is each
/// edge moved in by its allowance, trimmed or extended to meet its
/// neighbours, as the stitching line runs.
pub fn stitching_edges(piece: &Piece) -> Vec<Vec<Point>> {
    let outline = &piece.outline;
    let edges: Vec<Vec<Point>> = (0..outline.edges.len())
        .map(|i| outline.edge_geom(i).flatten(TOLERANCE))
        .collect();
    if piece.allowance.mode == AllowanceMode::Add {
        return edges;
    }
    let widths: Vec<f64> = (0..edges.len())
        .map(|i| piece.edge_allowance(i).max(0.0))
        .collect();
    let mut moved: Vec<Vec<Point>> = edges
        .iter()
        .zip(&widths)
        .map(|(points, &w)| offset_inward(points, w))
        .collect();
    let n = moved.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if i == j {
            break;
        }
        let limit = MITRE_LIMIT * widths[i].max(widths[j]);
        let corner = edges[i].last().copied().unwrap_or_default();
        let (a, b) = (moved[i].clone(), moved[j].clone());
        let (a, b) = meet(a, b, corner, limit);
        moved[i] = a;
        moved[j] = b;
    }
    moved
}

/// Where an edge's darts are on its stitching line (`stitching`, as
/// `stitching_edges` gives it): from and to, mm along it, in order.
pub fn dart_spans(piece: &Piece, index: usize, stitching: &[Point]) -> Vec<(f64, f64)> {
    let edge = &piece.outline.edges[index];
    let geom = piece.outline.edge_geom(index);
    let at = |along: f64| match piece.allowance.mode {
        crate::AllowanceMode::Add => along,
        crate::AllowanceMode::Included => along_polyline(stitching, geom.point_at(along)),
    };
    let mut spans: Vec<(f64, f64)> = piece
        .darts
        .iter()
        .filter(|d| d.edge == edge.id)
        .map(|d| {
            let (a, b) = (at(d.from), at(d.to));
            (a.min(b), a.max(b))
        })
        .collect();
    spans.sort_by(|x, y| x.0.total_cmp(&y.0));
    spans
}

/// How far along a stitching line `along` is with its darts (`spans`)
/// closed, as it is sewn, mm. In a dart, it is where the dart closes.
pub fn closed_along(spans: &[(f64, f64)], along: f64) -> f64 {
    let folded: f64 = spans
        .iter()
        .map(|&(from, to)| (along.min(to) - from).max(0.0))
        .sum();
    along - folded
}

/// Where along a stitching line a place `closed` along it as sewn is (its
/// darts, `spans`, closed): the inverse of `closed_along`. Where a dart
/// closes, the side before it.
pub fn opened_along(spans: &[(f64, f64)], closed: f64) -> f64 {
    let mut along = closed;
    for &(from, to) in spans {
        if along > from {
            along += to - from;
        }
    }
    along
}

/// A stitching line (`points`) as sewn, its darts (`spans`) closed: the
/// line beyond each dart moved back to meet the line before it. It is as
/// long as `closed_along` measures.
pub fn closed_polyline(points: &[Point], spans: &[(f64, f64)]) -> Vec<Point> {
    if spans.is_empty() || points.len() < 2 {
        return points.to_vec();
    }
    // How far along the line each of its points is.
    let mut walked = vec![0.0];
    for w in points.windows(2) {
        walked.push(walked[walked.len() - 1] + w[0].distance(w[1]));
    }
    let at = |s: f64| {
        let i = walked
            .partition_point(|&w| w < s)
            .clamp(1, points.len() - 1);
        let len = walked[i] - walked[i - 1];
        let t = if len > 0.0 {
            (s - walked[i - 1]) / len
        } else {
            1.0
        };
        points[i - 1].lerp(points[i], t.clamp(0.0, 1.0))
    };
    // The line's points outside its darts, and where each dart starts (its
    // other side meets it there).
    let mut kept: Vec<f64> = walked
        .iter()
        .copied()
        .filter(|&s| !spans.iter().any(|&(from, to)| from < s && s <= to))
        .chain(spans.iter().map(|&(from, _)| from))
        .collect();
    kept.sort_by(f64::total_cmp);
    kept.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    kept.into_iter()
        .map(|s| {
            let back: Vec2 = spans
                .iter()
                .filter(|&&(_, to)| to < s)
                .map(|&(from, to)| at(from) - at(to))
                .fold(Vec2::ZERO, |sum, v| sum + v);
            at(s) + back
        })
        .collect()
}

/// A line drawn inside a piece, as points along it from its start. Lines
/// are where they are drawn whether the outline is the cutting line or the
/// stitching line.
pub fn line_points(line: &InternalLine) -> Vec<Point> {
    flatten_segs(&segs_of(line.start, &line.segments), TOLERANCE)
}

/// Whether a line inside a piece is closed: it ends where it starts (a
/// pocket's placement drawn all round).
pub fn line_closed(line: &InternalLine) -> bool {
    line.segments
        .last()
        .is_some_and(|s| Point::from(s.to()).distance(Point::from(line.start)) < 0.1)
}

/// The length of a polyline, mm.
pub fn polyline_length(points: &[Point]) -> f64 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// How far along `points` the point nearest to `p` is, mm.
pub fn along_polyline(points: &[Point], p: Point) -> f64 {
    let (mut best, mut best_along, mut walked) = (f64::MAX, 0.0, 0.0);
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let ab = b - a;
        let len = ab.hypot();
        let t = if len > 0.0 {
            ((p - a).dot(ab) / (len * len)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let d = p.distance(a + ab * t);
        if d < best {
            best = d;
            best_along = walked + t * len;
        }
        walked += len;
    }
    best_along
}

/// Each point moved `width` inwards (outlines run clockwise on screen, so
/// inwards is against the outward normal), keeping the corners of the
/// flattened curve mitred.
fn offset_inward(points: &[Point], width: f64) -> Vec<Point> {
    if width == 0.0 || points.len() < 2 {
        return points.to_vec();
    }
    let dir = |a: Point, b: Point| {
        let d = b - a;
        let len = d.hypot();
        if len > 0.0 { d / len } else { Vec2::ZERO }
    };
    let n = points.len();
    (0..n)
        .map(|k| {
            let before = if k > 0 {
                dir(points[k - 1], points[k])
            } else {
                Vec2::ZERO
            };
            let after = if k + 1 < n {
                dir(points[k], points[k + 1])
            } else {
                Vec2::ZERO
            };
            let normals: Vec<Vec2> = [before, after]
                .into_iter()
                .filter(|d| *d != Vec2::ZERO)
                .map(|d| -outward_normal(d))
                .collect();
            let sum = normals.iter().fold(Vec2::ZERO, |s, v| s + *v);
            if sum.hypot() < 1e-12 {
                return points[k];
            }
            let bisector = sum / sum.hypot();
            // Along a kink the offset must reach further to stay `width` from
            // both segments; never more than the mitre limit.
            let reach = width / bisector.dot(normals[0]).max(1.0 / MITRE_LIMIT);
            points[k] + bisector * reach
        })
        .collect()
}

/// Two consecutive moved edges made to meet: cut back where they cross, or
/// carried on along their ends until they meet (within `limit` of the
/// outline's `corner`).
fn meet(
    mut a: Vec<Point>,
    mut b: Vec<Point>,
    corner: Point,
    limit: f64,
) -> (Vec<Point>, Vec<Point>) {
    if a.len() < 2 || b.len() < 2 {
        return (a, b);
    }
    // Only the parts near the corner can cross.
    let near = |p: Point| p.distance(corner) <= limit + 1.0;
    let last_a = (0..a.len() - 1)
        .rev()
        .take_while(|&k| near(a[k]) || near(a[k + 1]))
        .last();
    let first_b = (0..b.len() - 1)
        .take_while(|&k| near(b[k]) || near(b[k + 1]))
        .last();
    if let (Some(from_a), Some(to_b)) = (last_a, first_b) {
        // The crossing furthest along `a` and earliest along `b`.
        for ka in (from_a..a.len() - 1).rev() {
            for kb in 0..=to_b {
                if let Some(x) = crossing(a[ka], a[ka + 1], b[kb], b[kb + 1]) {
                    a.truncate(ka + 1);
                    a.push(x);
                    b.drain(..=kb);
                    b.insert(0, x);
                    return (a, b);
                }
            }
        }
    }
    // No crossing: a corner where the stitching line opens up. Carry both on
    // in a straight line until they meet.
    let (end_a, before_a) = (a[a.len() - 1], a[a.len() - 2]);
    let (start_b, after_b) = (b[0], b[1]);
    let (da, db) = (end_a - before_a, after_b - start_b);
    let denom = da.cross(db);
    if denom.abs() > 1e-12 {
        let s = (start_b - end_a).cross(db) / denom;
        let u = (start_b - end_a).cross(da) / denom;
        let x = end_a + da * s;
        if s >= 0.0 && u <= 0.0 && x.distance(corner) <= limit {
            a.push(x);
            b.insert(0, x);
        }
    }
    (a, b)
}

/// Where segments `p0`–`p1` and `q0`–`q1` cross, if they do.
fn crossing(p0: Point, p1: Point, q0: Point, q1: Point) -> Option<Point> {
    let (r, s) = (p1 - p0, q1 - q0);
    let denom = r.cross(s);
    if denom.abs() < 1e-12 {
        return None;
    }
    let t = (q0 - p0).cross(s) / denom;
    let u = (q0 - p0).cross(r) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| p0 + r * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Allowance, Edge, Outline, Segment};
    use crate::point::Pt;

    fn rectangle(mode: AllowanceMode) -> Piece {
        let mut piece = Piece::new(
            "p",
            "Box",
            Outline {
                start: Pt::new(0.0, 0.0),
                edges: vec![
                    Edge::new("top", vec![Segment::line([100.0, 0.0])]),
                    Edge::new("right", vec![Segment::line([100.0, 50.0])]),
                    Edge::new("bottom", vec![Segment::line([0.0, 50.0])]),
                    Edge::new("left", vec![Segment::line([0.0, 0.0])]),
                ],
            },
        );
        piece.allowance = Allowance {
            mode,
            seam: 10.0,
            hem: None,
        };
        piece
    }

    #[test]
    fn added_allowance_leaves_the_edges_as_they_are() {
        let lengths: Vec<f64> = stitching_edges(&rectangle(AllowanceMode::Add))
            .iter()
            .map(|e| polyline_length(e))
            .collect();
        assert_eq!(lengths, [100.0, 50.0, 100.0, 50.0]);
    }

    #[test]
    fn included_allowance_measures_inside_it() {
        let edges = stitching_edges(&rectangle(AllowanceMode::Included));
        let lengths: Vec<f64> = edges.iter().map(|e| polyline_length(e)).collect();
        for (got, want) in lengths.iter().zip([80.0, 30.0, 80.0, 30.0]) {
            assert!((got - want).abs() < 1e-6, "{lengths:?}");
        }
        assert!(edges[0][0].distance(Point::new(10.0, 10.0)) < 1e-6);
        // A point on the cutting line is found along the stitching line.
        assert!((along_polyline(&edges[0], Point::new(30.0, 0.0)) - 20.0).abs() < 1e-6);
    }

    #[test]
    fn darts_close_along_a_line_and_open_again() {
        // Two darts, 20 and 10 mm between their legs' ends, along a line
        // 200 mm long.
        let spans = [(50.0, 70.0), (120.0, 130.0)];
        let near = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(near(closed_along(&spans, 40.0), 40.0));
        // In a dart: where it closes.
        assert!(near(closed_along(&spans, 60.0), 50.0));
        assert!(near(closed_along(&spans, 200.0), 170.0));
        // And back again; where a dart closes, the side before it.
        for along in [0.0, 40.0, 50.0, 100.0, 120.0, 150.0, 200.0] {
            let back = opened_along(&spans, closed_along(&spans, along));
            assert!(near(back, along), "{along} came back {back}");
        }
        assert!(near(opened_along(&spans, closed_along(&spans, 70.0)), 50.0));
    }

    #[test]
    fn a_line_with_a_dart_closed_runs_past_it() {
        // Down 100, in 50 to a tip and back out (its mouth 20 across),
        // down 100 more.
        let points = [
            Point::new(0.0, 0.0),
            Point::new(0.0, 100.0),
            Point::new(-50.0, 110.0),
            Point::new(0.0, 120.0),
            Point::new(0.0, 220.0),
        ];
        let leg = 50f64.hypot(10.0);
        let spans = [(100.0, 100.0 + 2.0 * leg)];
        let closed = closed_polyline(&points, &spans);
        assert_eq!(closed.len(), 3, "{closed:?}");
        assert!(
            closed[2].distance(Point::new(0.0, 200.0)) < 1e-9,
            "{closed:?}"
        );
        let total = polyline_length(&points);
        assert!((polyline_length(&closed) - closed_along(&spans, total)).abs() < 1e-9);
        assert_eq!(closed_polyline(&points, &[]), points);
    }

    #[test]
    fn an_inward_corner_opens_and_is_closed() {
        // An L shape: the corner at (50, 50) turns inwards.
        let mut piece = Piece::new(
            "p",
            "L",
            Outline {
                start: Pt::new(0.0, 0.0),
                edges: vec![
                    Edge::new("a", vec![Segment::line([100.0, 0.0])]),
                    Edge::new("b", vec![Segment::line([100.0, 50.0])]),
                    Edge::new("c", vec![Segment::line([50.0, 50.0])]),
                    Edge::new("d", vec![Segment::line([50.0, 100.0])]),
                    Edge::new("e", vec![Segment::line([0.0, 100.0])]),
                    Edge::new("f", vec![Segment::line([0.0, 0.0])]),
                ],
            },
        );
        piece.allowance = rectangle(AllowanceMode::Included).allowance;
        let lengths: Vec<f64> = stitching_edges(&piece)
            .iter()
            .map(|e| polyline_length(e))
            .collect();
        // The inner corner's stitching lines run past it to meet.
        for (got, want) in lengths.iter().zip([80.0, 30.0, 50.0, 50.0, 30.0, 80.0]) {
            assert!((got - want).abs() < 1e-6, "{lengths:?}");
        }
    }
}
