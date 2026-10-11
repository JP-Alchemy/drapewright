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
/// Arrowheads drawn round (a triangle outline) can be this big, mm.
const MAX_OUTLINED_HEAD: f64 = 30.0;
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

/// A path that may be an arrowhead.
struct Head {
    index: usize,
    bounds: Rect,
    points: Vec<Point>,
    /// A closed head's corners (a triangle's three).
    corners: Vec<Point>,
    closed: bool,
}

/// Where closed shape `pts` turns by more than 35°. Sides shorter than
/// a quarter of a millimetre (a plotter's closing stroke) don't count.
pub(crate) fn sharp_corners(pts: &[Point]) -> Vec<Point> {
    let mut p: Vec<Point> = Vec::new();
    for &q in pts {
        if p.last().is_none_or(|l: &Point| l.distance(q) >= 0.25) {
            p.push(q);
        }
    }
    while p.len() > 1 && p[0].distance(p[p.len() - 1]) < 0.25 {
        p.pop();
    }
    let n = p.len();
    if n < 3 {
        return Vec::new();
    }
    let threshold = 35f64.to_radians().cos();
    (0..n)
        .filter(|&i| {
            let (u, v) = (p[i] - p[(i + n - 1) % n], p[(i + 1) % n] - p[i]);
            u.normalize().dot(v.normalize()) < threshold
        })
        .map(|i| p[i])
        .collect()
}

/// Paths that may be arrowheads: small open ones (a "V"), and triangles,
/// filled or drawn round.
fn small_heads(raw: &[RawPath]) -> Vec<Head> {
    raw.iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let pts = flatten_segs(&r.segs, 0.1);
            let b = bounds(&pts);
            let size = b.width().hypot(b.height());
            if !r.closed {
                let length: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
                return (size <= MAX_ARROWHEAD && length <= 3.0 * MAX_ARROWHEAD).then_some(Head {
                    index: i,
                    bounds: b,
                    points: pts,
                    corners: Vec::new(),
                    closed: false,
                });
            }
            // Round shapes (a dart's point, a button) are not arrowheads.
            let corners = sharp_corners(&pts);
            (size <= MAX_OUTLINED_HEAD && (3..=4).contains(&corners.len())).then_some(Head {
                index: i,
                bounds: b,
                points: pts,
                corners,
                closed: true,
            })
        })
        .collect()
}

/// The arrowheads at the `end` of a line arriving along `dir` (unit): a
/// triangle around or against the end, astride the line, with its point on
/// the line ahead; or a "V" whose point is the end.
fn heads_at(heads: &[Head], end: Point, dir: Vec2) -> Vec<usize> {
    heads
        .iter()
        .filter(|h| h.bounds.inflate(TOUCH, TOUCH).contains(end))
        .filter(|h| {
            if !h.closed {
                return h
                    .points
                    .iter()
                    .skip(1)
                    .take(h.points.len().saturating_sub(2))
                    .any(|p| p.distance(end) <= TOUCH);
            }
            let touches = point_in_polygon(end, &h.points)
                || distance_to_polyline(end, &h.points, true) <= TOUCH;
            let ahead = |c: &Point| (*c - end).dot(dir);
            let Some(tip) = h
                .corners
                .iter()
                .copied()
                .max_by(|a, b| ahead(a).total_cmp(&ahead(b)))
            else {
                return false;
            };
            let size = h.bounds.width().hypot(h.bounds.height());
            let side = |c: &Point| (*c - end).cross(dir);
            touches
                && ahead(&tip) >= -TOUCH
                && side(&tip).abs() <= TOUCH.max(0.15 * size)
                && h.corners.iter().any(|c| side(c) > TOUCH)
                && h.corners.iter().any(|c| side(c) < -TOUCH)
        })
        .map(|h| h.index)
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
    let mut gone = Vec::new();
    for (i, found) in arrows(raw) {
        raw[i].hint.push_str(" grain");
        gone.extend(found);
    }
    remove(raw, &gone);
}

/// Straight lines long enough to be grainlines with arrowheads at either
/// end: each line, and its heads.
fn arrows(raw: &[RawPath]) -> Vec<(usize, Vec<usize>)> {
    let heads = small_heads(raw);
    let mut out = Vec::new();
    for (i, r) in raw.iter().enumerate().filter(|(_, r)| !r.closed) {
        let pts = flatten_segs(&r.segs, 0.1);
        if pts.len() < 2 || !is_straight(&pts) {
            continue;
        }
        let (a, b) = (pts[0], pts[pts.len() - 1]);
        if a.distance(b) < MIN_GRAINLINE {
            continue;
        }
        let along = (b - a).normalize();
        let mut found = heads_at(&heads, a, -along);
        found.extend(heads_at(&heads, b, along));
        if !found.is_empty() {
            out.push((i, found));
        }
    }
    out
}

/// The arrowheads of lines that may be grainlines: not to be taken for
/// lettering, though plotter output often writes "grainline" beside them.
pub(crate) fn arrowheads(raw: &[RawPath]) -> Vec<usize> {
    let mut heads: Vec<usize> = arrows(raw).into_iter().flat_map(|(_, h)| h).collect();
    heads.sort_unstable();
    heads.dedup();
    heads
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
        gone.extend(heads_at(&heads, corners[0], unit(corners[0] - corners[1])));
        gone.extend(heads_at(&heads, corners[3], unit(corners[3] - corners[2])));
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
/// size printed inside, as text or drawn as lettering), if `r` is one, mm.
pub(crate) fn test_square(r: &RawPath, labels: &[Label], lettering: &[Rect]) -> Option<f64> {
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
    // Its size printed inside, as text or drawn as lettering.
    let lettered = lettering.iter().any(|l| b.contains(l.center()));
    (square && (says_size || lettered)).then_some(*standard)
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
