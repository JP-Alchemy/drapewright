//! What pattern CAD programs print through plotter drivers: every line,
//! word and logo drawn as strokes, with no text. Lettering (names, cutting
//! instructions, web addresses) is told from pattern lines by its shape:
//! tiny strokes in clusters. Logos repeated on every piece, and "+" marks,
//! are found too.

use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, point_in_polygon, polygon_area,
};
use kurbo::{Point, Rect, Vec2};

use super::marks::sharp_corners;
use crate::assemble::{RawPath, bounds, is_straight};

/// Strokes of lettering fit in a box this big, mm.
const GLYPH: f64 = 9.0;
/// A cluster needs this many strokes to read as words, not marks.
const MIN_STROKES: usize = 4;
/// Fewer strokes are a word of their own this close to other words, mm.
const WORD_GAP: f64 = 10.0;
/// Strokes this close touch, mm.
const TOUCHING: f64 = 0.5;

/// Clusters of tiny strokes: lettering. Returns the indices of their paths
/// and each cluster's bounds. Paths in `keep` are never lettering.
pub(crate) fn find_lettering(raw: &[RawPath], keep: &[usize]) -> (Vec<usize>, Vec<Rect>) {
    let tiny: Vec<(usize, Rect)> = raw
        .iter()
        .enumerate()
        .filter(|(i, _)| !keep.contains(i))
        .filter_map(|(i, r)| {
            let b = bounds(&flatten_segs(&r.segs, 0.1));
            (b.width().hypot(b.height()) <= GLYPH).then_some((i, b))
        })
        .collect();
    // Strokes link when the gap between them is under a letter's height
    // and a half: letters of a word, words of a line, lines of a block.
    let near = |a: &Rect, b: &Rect| {
        let reach = 1.5
            * a.height()
                .max(a.width())
                .max(b.height())
                .max(b.width())
                .max(2.0);
        let dx = (b.x0 - a.x1).max(a.x0 - b.x1).max(0.0);
        let dy = (b.y0 - a.y1).max(a.y0 - b.y1).max(0.0);
        dx.hypot(dy) <= reach
    };
    // Grid of cells, so neighbours are found quickly.
    const CELL: f64 = 10.0;
    let key = |p: Point| ((p.x / CELL).floor() as i64, (p.y / CELL).floor() as i64);
    let mut grid: std::collections::HashMap<(i64, i64), Vec<usize>> = Default::default();
    for (k, (_, b)) in tiny.iter().enumerate() {
        grid.entry(key(b.center())).or_default().push(k);
    }
    let mut cluster = vec![usize::MAX; tiny.len()];
    let mut clusters: Vec<Vec<usize>> = Vec::new();
    for start in 0..tiny.len() {
        if cluster[start] != usize::MAX {
            continue;
        }
        let id = clusters.len();
        cluster[start] = id;
        let mut members = vec![start];
        let mut i = 0;
        while i < members.len() {
            let k = members[i];
            let (cx, cy) = key(tiny[k].1.center());
            for dx in -2..=2 {
                for dy in -2..=2 {
                    for &j in grid.get(&(cx + dx, cy + dy)).map_or(&[][..], Vec::as_slice) {
                        if cluster[j] == usize::MAX && near(&tiny[k].1, &tiny[j].1) {
                            cluster[j] = id;
                            members.push(j);
                        }
                    }
                }
            }
            i += 1;
        }
        clusters.push(members);
    }
    let bounds_of = |members: &[usize]| {
        members
            .iter()
            .map(|&k| tiny[k].1)
            .reduce(|a, b| a.union(b))
            .unwrap_or_default()
    };
    let mut paths = Vec::new();
    let mut boxes = Vec::new();
    for members in clusters.iter().filter(|m| m.len() >= MIN_STROKES) {
        paths.extend(members.iter().map(|&k| tiny[k].0));
        boxes.push(bounds_of(members));
    }
    // A short word a space away from the rest ("to" in "gather to"), unless
    // it is all parallel straight strokes, as double and triple notches are.
    let notch_like = |members: &[usize]| {
        let dirs: Option<Vec<Vec2>> = members
            .iter()
            .map(|&k| {
                let pts = flatten_segs(&raw[tiny[k].0].segs, 0.1);
                let (a, b) = (*pts.first()?, *pts.last()?);
                (is_straight(&pts) && a.distance(b) > 0.0).then(|| (b - a).normalize())
            })
            .collect();
        dirs.is_some_and(|d| d.iter().all(|v| v.dot(d[0]).abs() > 0.95))
    };
    let beside = |b: Rect| {
        boxes.iter().any(|o| {
            let dx = (b.x0 - o.x1).max(o.x0 - b.x1).max(0.0);
            let dy = (b.y0 - o.y1).max(o.y0 - b.y1).max(0.0);
            dx.hypot(dy) <= WORD_GAP
        })
    };
    // A stroke on its own touching nothing (a dash between words): a notch
    // would cross a line.
    let flat: Vec<(Vec<Point>, Rect)> = raw
        .iter()
        .map(|r| {
            let pts = flatten_segs(&r.segs, 0.1);
            let b = bounds(&pts);
            (pts, b)
        })
        .collect();
    let alone = |i: usize, b: Rect| {
        let b = b.inflate(TOUCHING, TOUCHING);
        !flat.iter().enumerate().any(|(j, (other, ob))| {
            j != i
                && ob.x0 <= b.x1
                && b.x0 <= ob.x1
                && ob.y0 <= b.y1
                && b.y0 <= ob.y1
                && flat[i]
                    .0
                    .iter()
                    .any(|p| distance_to_polyline(*p, other, raw[j].closed) <= TOUCHING)
        })
    };
    let mut words = Vec::new();
    for members in clusters.iter().filter(|m| m.len() < MIN_STROKES) {
        let b = bounds_of(members);
        if !beside(b) {
            continue;
        }
        let word = match members.as_slice() {
            [k] => alone(tiny[*k].0, b),
            _ => !notch_like(members),
        };
        if word {
            paths.extend(members.iter().map(|&k| tiny[k].0));
            words.push(b);
        }
    }
    boxes.extend(words);
    paths.sort_unstable();
    (paths, boxes)
}

/// Logos signing each piece: the same intricate closed shape (a letter of
/// a brand name) drawn three or more times inside pieces, at any size,
/// holding no other shape; with the round dots beside them (an "i"'s).
/// Returns the paths, and how many logos they make.
pub(crate) fn find_repeated_decorations(raw: &[RawPath], skip: &[usize]) -> (Vec<usize>, usize) {
    struct Shape {
        index: usize,
        points: Vec<Point>,
        bounds: Rect,
        area: f64,
        /// 4πA/P²: 1 for a circle, small for intricate shapes.
        compactness: f64,
    }
    let shapes: Vec<Shape> = raw
        .iter()
        .enumerate()
        .filter(|(i, r)| r.closed && !skip.contains(i))
        .filter_map(|(i, r)| {
            let points = flatten_segs(&r.segs, 0.1);
            let area = polygon_area(&points).abs();
            let perimeter: f64 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
            (points.len() >= 3 && area > 0.0).then(|| Shape {
                index: i,
                bounds: bounds(&points),
                compactness: 4.0 * std::f64::consts::PI * area / perimeter.powi(2),
                points,
                area,
            })
        })
        .collect();
    let around = |outer: &Shape, inner: &Shape| {
        outer.index != inner.index
            && outer.bounds.contains(inner.bounds.origin())
            && outer
                .bounds
                .contains(Point::new(inner.bounds.x1, inner.bounds.y1))
            && point_in_polygon(inner.points[0], &outer.points)
    };
    // Small intricate shapes inside a piece, holding nothing.
    let candidates: Vec<&Shape> = shapes
        .iter()
        .filter(|s| s.points.len() >= 12 && s.area <= 20_000.0)
        .filter(|s| {
            shapes
                .iter()
                .any(|o| o.area >= 8.0 * s.area && around(o, s))
        })
        .filter(|s| !shapes.iter().any(|o| around(s, o)))
        .collect();
    let alike = |a: &Shape, b: &Shape| {
        let points = a.points.len() as f64 / b.points.len() as f64;
        (a.compactness - b.compactness).abs() <= 0.03 * a.compactness.max(b.compactness)
            && (0.7..=1.4).contains(&points)
    };
    let repeated: Vec<&Shape> = candidates
        .iter()
        .copied()
        .filter(|s| s.compactness < 0.6)
        .filter(|s| candidates.iter().filter(|o| alike(s, o)).count() >= 3)
        .collect();
    // Letters near each other make one logo (spaced capitals too).
    let near = |s: &Shape| {
        let grow = 1.5 * s.bounds.width().max(s.bounds.height());
        s.bounds.inflate(grow, grow)
    };
    let touching = |a: Rect, b: Rect| a.x0 <= b.x1 && b.x0 <= a.x1 && a.y0 <= b.y1 && b.y0 <= a.y1;
    // A logo has letters of more than one shape: one symbol repeated on
    // its own (a button's cross) is a mark.
    let letters: Vec<&Shape> = repeated
        .iter()
        .copied()
        .filter(|s| {
            repeated
                .iter()
                .any(|o| !alike(s, o) && touching(near(s), o.bounds))
        })
        .collect();
    // Round dots within a letter's reach.
    let reach = |s: &Shape| {
        let grow = 0.5 * s.bounds.width().max(s.bounds.height());
        s.bounds.inflate(grow, grow)
    };
    let dots: Vec<&Shape> = shapes
        .iter()
        .filter(|s| s.compactness >= 0.6 && s.area <= 200.0)
        .filter(|s| !shapes.iter().any(|o| around(s, o)))
        .filter(|s| letters.iter().any(|l| reach(l).contains(s.bounds.center())))
        .collect();
    let mut logo = vec![usize::MAX; letters.len()];
    let mut logos = 0;
    for k in 0..letters.len() {
        if logo[k] != usize::MAX {
            continue;
        }
        logo[k] = logos;
        let mut stack = vec![k];
        while let Some(j) = stack.pop() {
            for m in 0..letters.len() {
                if logo[m] == usize::MAX && touching(near(letters[j]), letters[m].bounds) {
                    logo[m] = logos;
                    stack.push(m);
                }
            }
        }
        logos += 1;
    }
    let mut gone: Vec<usize> = letters.iter().chain(&dots).map(|s| s.index).collect();
    // Specks a plotter left among a logo's letters and dots go with it.
    let areas: Vec<Rect> = (0..logos)
        .filter_map(|n| {
            let mine: Vec<&&Shape> = letters
                .iter()
                .zip(&logo)
                .filter(|(_, l)| **l == n)
                .map(|(s, _)| s)
                .collect();
            let theirs = dots
                .iter()
                .filter(|d| mine.iter().any(|l| reach(l).contains(d.bounds.center())));
            mine.iter()
                .map(|s| s.bounds)
                .chain(theirs.map(|d| d.bounds))
                .reduce(|a, b| a.union(b))
                .map(|a| a.inflate(1.0, 1.0))
        })
        .collect();
    for (i, r) in raw.iter().enumerate() {
        let b = bounds(&flatten_segs(&r.segs, 0.1));
        if b.width().hypot(b.height()) <= 2.0
            && areas
                .iter()
                .any(|a| a.contains(b.origin()) && a.contains(Point::new(b.x1, b.y1)))
        {
            gone.push(i);
        }
    }
    gone.sort_unstable();
    gone.dedup();
    (gone, logos)
}

/// "+" marks: two short straight strokes crossing at their middles. Returns
/// each mark's centre and arm length, and the paths that drew them.
pub(crate) fn find_crosses(raw: &[RawPath], skip: &[usize]) -> (Vec<(Point, f64)>, Vec<usize>) {
    let short: Vec<(usize, Point, Vec2, f64)> = raw
        .iter()
        .enumerate()
        .filter(|(i, r)| !r.closed && !skip.contains(i))
        .filter_map(|(i, r)| {
            let pts = flatten_segs(&r.segs, 0.1);
            let (a, b) = (*pts.first()?, *pts.last()?);
            let length = a.distance(b);
            ((2.0..=12.0).contains(&length) && is_straight(&pts))
                .then(|| (i, a.midpoint(b), (b - a) / length, length))
        })
        .collect();
    let mut used = vec![false; short.len()];
    let mut marks = Vec::new();
    let mut paths = Vec::new();
    for k in 0..short.len() {
        if used[k] {
            continue;
        }
        let (i, mid, dir, length) = short[k];
        let partner = (k + 1..short.len()).find(|&j| {
            !used[j]
                && short[j].1.distance(mid) <= 0.6
                && short[j].2.dot(dir).abs() < 0.15
                && (short[j].3 - length).abs() <= 0.3 * length
        });
        if let Some(j) = partner {
            used[k] = true;
            used[j] = true;
            marks.push((mid, (length + short[j].3) / 2.0));
            paths.extend([i, short[j].0]);
        }
    }
    (marks, paths)
}

/// "+" marks drawn round, as one closed outline of twelve square corners.
/// Returns each mark's centre and size, and the paths that drew them.
pub(crate) fn find_outlined_crosses(raw: &[RawPath]) -> (Vec<(Point, f64)>, Vec<usize>) {
    let mut marks = Vec::new();
    let mut paths = Vec::new();
    for (i, r) in raw.iter().enumerate().filter(|(_, r)| r.closed) {
        let pts = flatten_segs(&r.segs, 0.1);
        let b = bounds(&pts);
        let size = (b.width() + b.height()) / 2.0;
        if !(2.0..=30.0).contains(&size) || (b.width() - b.height()).abs() > 0.15 * size {
            continue;
        }
        let corners = sharp_corners(&pts);
        if corners.len() != 12 {
            continue;
        }
        let n = corners.len();
        let square = (0..n).all(|k| {
            let (u, v) = (
                corners[k] - corners[(k + n - 1) % n],
                corners[(k + 1) % n] - corners[k],
            );
            u.normalize().dot(v.normalize()).abs() < 0.2
        });
        let centre = corners.iter().fold(Vec2::ZERO, |s, c| s + c.to_vec2()) / n as f64;
        if square && centre.to_point().distance(b.center()) <= 0.1 * size {
            marks.push((b.center(), size));
            paths.push(i);
        }
    }
    (marks, paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Line, PathSeg};

    fn line(a: (f64, f64), b: (f64, f64)) -> RawPath {
        RawPath {
            segs: vec![PathSeg::Line(Line::new(a, b))],
            closed: false,
            filled: false,
            hint: String::new(),
            name: None,
            reference: None,
            cut: None,
        }
    }

    #[test]
    fn words_of_strokes_and_lone_marks() {
        let mut raw = Vec::new();
        // "LIT" in 5 mm strokes, letters 2 mm apart.
        for (x, strokes) in [(0.0, 2), (5.0, 1), (8.0, 2)] {
            raw.push(line((x, 0.0), (x, 5.0)));
            if strokes == 2 {
                raw.push(line((x, 5.0), (x + 3.0, 5.0)));
            }
        }
        // A notch far away, and a long pattern line.
        raw.push(line((100.0, 0.0), (106.0, 0.0)));
        raw.push(line((0.0, 50.0), (200.0, 50.0)));
        let (paths, boxes) = find_lettering(&raw, &[]);
        assert_eq!(paths, [0, 1, 2, 3, 4]);
        assert_eq!(boxes.len(), 1);
    }

    #[test]
    fn crosses_are_marks() {
        let raw = vec![
            line((10.0, 6.0), (10.0, 14.0)),
            line((6.0, 10.0), (14.0, 10.0)),
            // Two lines meeting at an end are not a cross.
            line((50.0, 50.0), (58.0, 50.0)),
            line((58.0, 50.0), (58.0, 58.0)),
        ];
        let (marks, paths) = find_crosses(&raw, &[]);
        assert_eq!(marks.len(), 1);
        assert!(marks[0].0.distance(Point::new(10.0, 10.0)) < 1e-9);
        assert_eq!(paths, [0, 1]);
    }
}
