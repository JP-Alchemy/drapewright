//! Marks printed on PDF patterns, found by their shape: grainlines (a long
//! straight line with arrowheads), notches (short cuts across the cutting
//! line, or small triangles on it), "place on fold" brackets, and the test
//! square printed to check the scale.

use drapewright_pattern_core::NotchKind;
use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, point_in_polygon, polygon_area,
};
use kurbo::{ParamCurve, PathSeg, Point, Rect, Vec2};

use super::import::Label;
use crate::assemble::{MIN_PIECE_AREA, RawNotch, RawPath, bounds, is_straight};

/// Grainlines are at least this long, mm; arrowheads at most this big.
const MIN_GRAINLINE: f64 = 30.0;
const MAX_ARROWHEAD: f64 = 15.0;
/// Notch cuts are between these lengths, mm.
const NOTCH_LENGTHS: (f64, f64) = (1.5, 15.0);
/// Marks this close to a line are on it, mm.
const TOUCH: f64 = 0.5;
/// Notches closer than this on one line make one double or triple notch, mm.
const NOTCH_GROUP: f64 = 9.0;
/// Test square sides: 1–4 inches, and 5 or 10 cm.
const TEST_SQUARES: [f64; 6] = [25.4, 50.0, 50.8, 76.2, 100.0, 101.6];

/// The closed outlines big enough to be pieces, flattened, with bounds.
fn outlines(raw: &[RawPath]) -> Vec<(Vec<Point>, Rect)> {
    raw.iter()
        .filter(|r| r.closed)
        .map(|r| flatten_segs(&r.segs, 0.1))
        .filter(|pts| polygon_area(pts).abs() >= MIN_PIECE_AREA)
        .map(|pts| {
            let b = bounds(&pts);
            (pts, b)
        })
        .collect()
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1
}

/// The point of `poly` nearest to `p`, and the direction of the line there.
fn nearest_on(poly: &[Point], p: Point) -> Option<(Point, Vec2)> {
    let n = poly.len();
    (0..n)
        .filter_map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let ab = b - a;
            let len2 = ab.hypot2();
            if len2 < 1e-12 {
                return None;
            }
            let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
            let q = a + ab * t;
            Some((q, ab / len2.sqrt(), q.distance(p)))
        })
        .min_by(|x, y| x.2.total_cmp(&y.2))
        .map(|(q, dir, _)| (q, dir))
}

/// Arrowhead-sized paths: (index, bounds, points, closed).
fn small_heads(raw: &[RawPath]) -> Vec<(usize, Rect, Vec<Point>, bool)> {
    raw.iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let pts = flatten_segs(&r.segs, 0.1);
            let b = bounds(&pts);
            let length: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            (b.width().hypot(b.height()) <= MAX_ARROWHEAD && length <= 3.0 * MAX_ARROWHEAD)
                .then_some((i, b, pts, r.closed))
        })
        .collect()
}

/// The arrowheads pointing at `end`: small shapes containing it or with a
/// corner on it, or a "V" whose point is it.
fn heads_at(heads: &[(usize, Rect, Vec<Point>, bool)], end: Point) -> Vec<usize> {
    heads
        .iter()
        .filter(|(_, b, pts, closed)| {
            b.inflate(TOUCH, TOUCH).contains(end)
                && ((*closed
                    && (point_in_polygon(end, pts)
                        || pts.iter().any(|p| p.distance(end) <= TOUCH)))
                    || (!*closed
                        && pts
                            .iter()
                            .skip(1)
                            .take(pts.len().saturating_sub(2))
                            .any(|p| p.distance(end) <= TOUCH)))
        })
        .map(|(i, _, _, _)| *i)
        .collect()
}

fn remove(raw: &mut Vec<RawPath>, gone: &[usize]) {
    let mut i = 0;
    raw.retain(|_| {
        let keep = !gone.contains(&i);
        i += 1;
        keep
    });
}

/// Straight lines with an arrowhead at either end get the hint "grain";
/// their arrowheads are removed.
pub(crate) fn find_grainlines(raw: &mut Vec<RawPath>) {
    let heads = small_heads(raw);
    let mut gone = Vec::new();
    for r in raw.iter_mut().filter(|r| !r.closed) {
        let pts = flatten_segs(&r.segs, 0.1);
        if pts.len() < 2 || !is_straight(&pts) {
            continue;
        }
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        if a.distance(b) < MIN_GRAINLINE {
            continue;
        }
        let found: Vec<usize> = [a, b].iter().flat_map(|e| heads_at(&heads, *e)).collect();
        if !found.is_empty() {
            r.hint.push_str(" grain");
            gone.extend(found);
        }
    }
    remove(raw, &gone);
}

/// "Place on fold" brackets: a ⊐ of three straight lines whose ends point
/// at an outline. They get the hint "cutonfold", with their ends moved onto
/// the outline so the fold edge is found. Returns where their ends are.
pub(crate) fn find_fold_brackets(raw: &mut Vec<RawPath>) -> Vec<Point> {
    let lines = outlines(raw);
    let heads = small_heads(raw);
    let mut ends = Vec::new();
    let mut gone = Vec::new();
    for r in raw.iter_mut().filter(|r| !r.closed) {
        let corners: Vec<Point> = std::iter::once(r.segs[0].start())
            .chain(r.segs.iter().map(|s| s.end()))
            .collect();
        if corners.len() != 4 || !r.segs.iter().all(|s| matches!(s, PathSeg::Line(_))) {
            continue;
        }
        let legs = (corners[1] - corners[0], corners[3] - corners[2]);
        let across = corners[2] - corners[1];
        let unit = |v: Vec2| v / v.hypot().max(1e-9);
        let square = unit(legs.0).dot(unit(across)).abs() < 0.1
            && unit(legs.1).dot(unit(across)).abs() < 0.1;
        let opposite = unit(legs.0).dot(unit(legs.1)) < -0.9;
        if !square || !opposite || across.hypot() < 30.0 || legs.0.hypot() > 40.0 {
            continue;
        }
        // Both ends close to one outline.
        let target = lines.iter().find(|(poly, b)| {
            b.inflate(5.0, 5.0).contains(corners[0])
                && distance_to_polyline(corners[0], poly, true) <= 5.0
                && distance_to_polyline(corners[3], poly, true) <= 5.0
        });
        let Some((poly, _)) = target else { continue };
        let (Some((a, _)), Some((b, _))) =
            (nearest_on(poly, corners[0]), nearest_on(poly, corners[3]))
        else {
            continue;
        };
        if let Some(PathSeg::Line(first)) = r.segs.first_mut() {
            first.p0 = a;
        }
        if let Some(PathSeg::Line(last)) = r.segs.last_mut() {
            last.p1 = b;
        }
        r.hint.push_str(" cutonfold");
        // The bracket's arrowheads go with it.
        gone.extend(heads_at(&heads, corners[0]));
        gone.extend(heads_at(&heads, corners[3]));
        ends.extend([a, b]);
    }
    remove(raw, &gone);
    ends
}

/// Short cuts across an outline, and small triangles on one, become
/// notches; the paths that drew them are removed. Marks near `avoid` (the
/// ends of fold brackets, whose arrowheads touch the outline) are left.
pub(crate) fn find_notches(raw: &mut Vec<RawPath>, avoid: &[Point]) -> Vec<RawNotch> {
    let lines = outlines(raw);
    // (point on the outline, direction of the cut, which outline)
    let mut found: Vec<(Point, Vec2, usize)> = Vec::new();
    let mut used = vec![false; raw.len()];
    for (i, r) in raw.iter().enumerate() {
        let pts = flatten_segs(&r.segs, 0.1);
        if pts.len() < 2 {
            continue;
        }
        let b = bounds(&pts);
        if avoid.iter().any(|p| b.inflate(3.0, 3.0).contains(*p)) {
            continue;
        }
        if r.closed {
            // A small triangle with a side or a corner on an outline.
            let corners: Vec<Point> = r.segs.iter().map(|s| s.start()).collect();
            if corners.len() != 3 || b.width().hypot(b.height()) > 12.0 {
                continue;
            }
            for (k, (poly, pb)) in lines.iter().enumerate() {
                if !overlaps(pb.inflate(1.0, 1.0), b) {
                    continue;
                }
                let on: Vec<Point> = corners
                    .iter()
                    .copied()
                    .filter(|c| distance_to_polyline(*c, poly, true) <= TOUCH)
                    .collect();
                let at = match on.as_slice() {
                    [a, b] => a.midpoint(*b),
                    [a] => *a,
                    _ => continue,
                };
                if let Some((q, tangent)) = nearest_on(poly, at) {
                    found.push((q, Vec2::new(-tangent.y, tangent.x), k));
                    used[i] = true;
                    break;
                }
            }
            continue;
        }
        let (a, z) = (pts[0], pts[pts.len() - 1]);
        let length = a.distance(z);
        if !is_straight(&pts) || length < NOTCH_LENGTHS.0 || length > NOTCH_LENGTHS.1 {
            continue;
        }
        let dir = (z - a) / length;
        for (k, (poly, pb)) in lines.iter().enumerate() {
            if !pb.inflate(length, length).contains(a) {
                continue;
            }
            let (da, dz) = (
                distance_to_polyline(a, poly, true),
                distance_to_polyline(z, poly, true),
            );
            let crosses =
                point_in_polygon(a, poly) != point_in_polygon(z, poly) && da.max(dz) <= length;
            if da.min(dz) > TOUCH && !crosses {
                continue;
            }
            let end = if da <= dz { a } else { z };
            let Some((q, tangent)) = nearest_on(poly, end) else {
                continue;
            };
            if dir.dot(tangent).abs() > 0.5 {
                continue; // along the line, not across it
            }
            found.push((q, dir, k));
            used[i] = true;
            break;
        }
    }
    // Cuts close together on one line are one double or triple notch.
    let mut taken = vec![false; found.len()];
    let mut notches = Vec::new();
    for i in 0..found.len() {
        if taken[i] {
            continue;
        }
        let group: Vec<usize> = (i..found.len())
            .filter(|&j| {
                !taken[j]
                    && found[j].2 == found[i].2
                    && found[j].0.distance(found[i].0) <= NOTCH_GROUP
                    && found[j].1.cross(found[i].1).abs() < 0.3
            })
            .collect();
        for &j in &group {
            taken[j] = true;
        }
        let centre = group
            .iter()
            .fold(Vec2::ZERO, |s, &j| s + found[j].0.to_vec2())
            / group.len() as f64;
        let at = nearest_on(&lines[found[i].2].0, centre.to_point()).map_or(found[i].0, |(q, _)| q);
        notches.push(RawNotch {
            at,
            kind: match group.len() {
                1 => NotchKind::Single,
                2 => NotchKind::Double,
                _ => NotchKind::Triple,
            },
        });
    }
    let mut i = 0;
    raw.retain(|_| {
        let keep = !used[i];
        i += 1;
        keep
    });
    notches
}

/// The side of a test square (a square of 1–4 in or 5–10 cm with its
/// size printed inside), if `r` is one, mm.
pub(crate) fn test_square(r: &RawPath, labels: &[Label]) -> Option<f64> {
    if !r.closed || !r.segs.iter().all(|s| matches!(s, PathSeg::Line(_))) || r.segs.len() != 4 {
        return None;
    }
    let pts = flatten_segs(&r.segs, 0.05);
    let b = bounds(&pts);
    let side = (b.width() + b.height()) / 2.0;
    let square = (b.width() - b.height()).abs() <= 0.5
        && (polygon_area(&pts).abs() - b.area()).abs() <= 0.02 * b.area();
    let standard = TEST_SQUARES.iter().find(|s| (side - **s).abs() <= 0.4)?;
    let says_size = labels.iter().any(|l| {
        let t = l.text.to_lowercase();
        b.contains(l.at)
            && (t.contains("cm")
                || t.contains("in")
                || t.contains('"')
                || t.contains("test")
                || t.contains("square")
                || t.contains("mm"))
    });
    (square && says_size).then_some(*standard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Line;

    fn raw(points: &[(f64, f64)], closed: bool) -> RawPath {
        let mut segs: Vec<PathSeg> = points
            .windows(2)
            .map(|w| PathSeg::Line(Line::new(w[0], w[1])))
            .collect();
        if closed {
            segs.push(PathSeg::Line(Line::new(*points.last().unwrap(), points[0])));
        }
        RawPath {
            segs,
            closed,
            filled: false,
            hint: String::new(),
            name: None,
            reference: None,
            cut: None,
        }
    }

    #[test]
    fn notches_from_cuts_and_triangles() {
        let piece = raw(
            &[(0.0, 0.0), (200.0, 0.0), (200.0, 300.0), (0.0, 300.0)],
            true,
        );
        let mut paths = vec![
            piece,
            // A cut across the right side, and two close together on the left.
            raw(&[(197.0, 100.0), (203.0, 100.0)], false),
            raw(&[(-3.0, 150.0), (3.0, 150.0)], false),
            raw(&[(-3.0, 156.0), (3.0, 156.0)], false),
            // A triangle on the bottom.
            raw(&[(95.0, 300.0), (105.0, 300.0), (100.0, 306.0)], true),
            // A line along the outline is not a notch.
            raw(&[(50.0, 0.0), (60.0, 0.0)], false),
        ];
        let notches = find_notches(&mut paths, &[]);
        assert_eq!(paths.len(), 2, "the piece and the line along it remain");
        let kinds: Vec<NotchKind> = notches.iter().map(|n| n.kind).collect();
        assert_eq!(
            kinds,
            [NotchKind::Single, NotchKind::Double, NotchKind::Single]
        );
        assert!(notches[0].at.distance(Point::new(200.0, 100.0)) < 1e-9);
        assert!(notches[1].at.distance(Point::new(0.0, 153.0)) < 1e-9);
        assert!(notches[2].at.distance(Point::new(100.0, 300.0)) < 1e-9);
    }

    #[test]
    fn grainlines_by_their_arrowheads() {
        let mut paths = vec![
            raw(&[(50.0, 20.0), (50.0, 200.0)], false),
            // A "V" at the bottom end.
            raw(&[(47.0, 194.0), (50.0, 200.0), (53.0, 194.0)], false),
            // A plain line, no heads.
            raw(&[(80.0, 20.0), (80.0, 200.0)], false),
        ];
        find_grainlines(&mut paths);
        // The arrowhead goes with its grainline.
        assert_eq!(paths.len(), 2);
        assert!(paths[0].hint.contains("grain"));
        assert!(!paths[1].hint.contains("grain"));
    }
}
