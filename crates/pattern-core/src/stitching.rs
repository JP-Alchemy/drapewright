//! Each edge's own part of the stitching line. Seams are measured and
//! matched on the stitching line, so where an outline is the cutting line
//! (allowance included) its edges are moved in by their allowance first.

use kurbo::{Point, Vec2};

use crate::allowance::MITRE_LIMIT;
use crate::geom::outward_normal;
use crate::model::{AllowanceMode, Piece};

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
