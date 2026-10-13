//! One piece cut in a different size at each height, blended smoothly
//! between them, as a sewist blends between the lines of a multi-size
//! pattern: one size at the bust, another at the waist, and between them
//! a line that moves from the one to the other.
//!
//! A pattern's sizes don't share corners piece for piece (one size's front
//! may be drawn in four edges, the next in seven), so the sizes are matched
//! by shape: each size's sharp corners matched to the nearest of the same
//! turn in the size blended from, and the outline between two corners
//! matched in proportion along its length. The blended piece keeps the
//! edges, notches and darts of the size blended from; the points inside it
//! (its grainline, marks and lines) move with the outline round them.

use kurbo::{ParamCurve, ParamCurveNearest, PathSeg, Point, Rect, Vec2};

use crate::Pt;
use crate::geom::{SAME_POINT, segs_of};
use crate::model::{EdgeKind, NotchFrom, Outline, Piece, Segment};

/// How high on the figure each point of a piece goes as worn, mm: `up · p
/// + at` for a point `p` of the piece.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Height {
    /// A unit vector, in the piece's own coordinates.
    pub up: Pt,
    pub at: f64,
}

impl Height {
    pub fn of(&self, p: Pt) -> f64 {
        self.up.x * p.x + self.up.y * p.y + self.at
    }
}

/// One size of a piece, and how it is worn when it is.
#[derive(Clone, Copy, Debug)]
pub struct Sized<'a> {
    pub piece: &'a Piece,
    pub height: Option<Height>,
}

/// Samples along the outline at most this far apart, mm.
const STEP: f64 = 2.0;
/// Curves are followed this closely, mm.
const FLAT: f64 = 0.05;
/// A blended segment kept as a curve, or a straight line, while it stays
/// this close to the blend, mm; else it is drawn as many short lines.
const KEEP: f64 = 0.25;
/// Short lines this nearly straight are one, mm.
const STRAIGHT: f64 = 0.05;
/// A turn sharper than this is a corner, radians (about 30°) ...
const CORNER: f64 = 0.5;
/// ... measured this far either side of it, mm.
const REACH: f64 = 2.0;
/// Corners further apart than this (or this much of the piece's size
/// across, if more) are not the same corner, mm.
const NEAR: f64 = 40.0;
const NEAR_SHARE: f64 = 0.2;
/// Sizes whose outlines' boxes overlap this much are drawn nested, in one
/// frame, as most multi-size patterns print them.
const NESTED: f64 = 0.6;

/// The piece blended between its sizes: `sizes` smallest first, each the
/// same piece in one size; `reference` the size blended from, whose edges,
/// notches and darts it keeps and whose heights it is blended by;
/// `index(h)` the size to cut at height `h` on the figure, counting along
/// `sizes`, fractional between two. None when the size blended from isn't
/// worn round the body, so has no heights.
pub fn blend(sizes: &[Sized], reference: usize, index: impl Fn(f64) -> f64) -> Option<Piece> {
    let base = sizes.get(reference)?;
    let height = base.height?;
    let piece = base.piece;
    let last = sizes.len().saturating_sub(1) as f64;

    // The size blended from, sampled segment by segment round its outline.
    let (samples, ring) = sample(&piece.outline);
    if ring.points.is_empty() {
        return Some(piece.clone());
    }
    // Each other size in the same frame, matched to it.
    let others: Vec<Option<(Ring, Matched)>> = sizes
        .iter()
        .enumerate()
        .map(|(k, size)| {
            (k != reference).then(|| {
                let other = Ring::of(&size.piece.outline, shift(base, size));
                let matched = Matched::of(&ring, &other);
                (other, matched)
            })
        })
        .collect();
    let blended: Vec<Point> = ring
        .points
        .iter()
        .zip(&ring.at)
        .map(|(&p, &s)| {
            let t = index(height.of(p.into())).clamp(0.0, last);
            let low = t.floor() as usize;
            let high = (low + 1).min(sizes.len() - 1);
            let at = |k: usize| match &others[k] {
                Some((other, matched)) => other.point(matched.along(s)),
                None => p,
            };
            at(low).lerp(at(high), t - low as f64)
        })
        .collect();
    let moved: Vec<Vec2> = blended
        .iter()
        .zip(&ring.points)
        .map(|(b, p)| *b - *p)
        .collect();
    let inside =
        |q: Pt| -> Pt { (Point::from(q) + mean_value(&ring.points, &moved, q.into())).into() };

    let mut out = piece.clone();
    // The outline, segment by segment; and along each edge, how far each
    // sample was and is now.
    let n = out.outline.edges.len();
    let mut segments: Vec<Vec<Segment>> = vec![Vec::new(); n];
    let mut lengths: Vec<(Vec<f64>, Vec<f64>)> = vec![(Vec::new(), Vec::new()); n];
    for run in &samples {
        let at = |i: usize| i % blended.len();
        let points: Vec<Point> = (run.from..=run.to).map(|i| blended[at(i)]).collect();
        let (was, now) = &mut lengths[run.edge];
        let before = now.last().copied().unwrap_or(0.0);
        let walked = running(&points);
        let skip = usize::from(!was.is_empty());
        for (along, walked) in run.along.iter().zip(&walked).skip(skip) {
            was.push(*along);
            now.push(before + walked);
        }
        segments[run.edge].extend(blended_segment(
            &run.seg,
            &points,
            moved[at(run.from)],
            moved[at(run.to)],
        ));
    }
    for (edge, segments) in out.outline.edges.iter_mut().zip(segments) {
        if !segments.is_empty() {
            edge.segments = segments;
        }
    }
    if let Some(first) = samples.first() {
        out.outline.start = blended[first.from].into();
    }
    // Along each edge, a length on the size blended from as it is blended.
    let along = |edge: &str, s: f64| -> Option<(f64, f64)> {
        let e = piece.outline.edge_index(edge)?;
        let (was, now) = lengths.get(e)?;
        Some((stretch(was, now, s), now.last().copied().unwrap_or(0.0)))
    };
    for notch in &mut out.notches {
        let Some(e) = piece.outline.edge_index(&notch.edge) else {
            continue;
        };
        let length = piece.outline.edge_geom(e).length();
        let from_start = match notch.from {
            NotchFrom::Start => notch.at,
            NotchFrom::End => length - notch.at,
        };
        if let Some((s, total)) = along(&notch.edge, from_start) {
            notch.at = match notch.from {
                NotchFrom::Start => s,
                NotchFrom::End => total - s,
            };
        }
    }
    for dart in &mut out.darts {
        if let (Some((from, _)), Some((to, _))) =
            (along(&dart.edge, dart.from), along(&dart.edge, dart.to))
        {
            dart.from = from;
            dart.to = to;
        }
    }
    // Inside: each point moved as the outline round it moved.
    if let Some(g) = &mut out.grainline {
        g.from = inside(g.from);
        g.to = inside(g.to);
    }
    for mark in &mut out.marks {
        mark.at = inside(mark.at);
    }
    for label in &mut out.labels {
        label.at = inside(label.at);
    }
    for line in &mut out.lines {
        line.start = inside(line.start);
        for seg in &mut line.segments {
            seg.map_points(inside);
        }
    }
    Some(out)
}

/// A stretch of the outline drawn as one segment, and its samples.
struct Run {
    edge: usize,
    seg: PathSeg,
    /// Its samples' indices round the ring, first to last (the last may be
    /// the ring's length: its first point, round again).
    from: usize,
    to: usize,
    /// How far along its edge each sample is.
    along: Vec<f64>,
}

/// The outline sampled segment by segment, at most `STEP` apart, and the
/// ring of all its samples.
fn sample(outline: &Outline) -> (Vec<Run>, Ring) {
    let mut runs = Vec::new();
    let mut points: Vec<Point> = Vec::new();
    for (e, edge) in outline.edges.iter().enumerate() {
        let mut along = 0.0;
        for seg in segs_of(outline.edge_start(e), &edge.segments) {
            let mut flat = vec![seg.start()];
            match seg {
                PathSeg::Line(_) => flat.push(seg.end()),
                _ => kurbo::flatten(
                    [kurbo::PathEl::MoveTo(seg.start()), seg.as_path_el()],
                    FLAT,
                    |el| {
                        if let kurbo::PathEl::LineTo(p) = el {
                            flat.push(p);
                        }
                    },
                ),
            }
            let mut dense = vec![flat[0]];
            for w in flat.windows(2) {
                let n = (w[0].distance(w[1]) / STEP).ceil().max(1.0) as usize;
                for i in 1..=n {
                    dense.push(w[0].lerp(w[1], i as f64 / n as f64));
                }
            }
            let from = points.len();
            let mut at = Vec::with_capacity(dense.len());
            for (i, p) in dense.iter().enumerate() {
                if i > 0 {
                    along += dense[i - 1].distance(*p);
                }
                at.push(along);
            }
            points.extend(&dense[..dense.len() - 1]);
            runs.push(Run {
                edge: e,
                seg,
                from,
                to: from + dense.len() - 1,
                along: at,
            });
        }
    }
    (runs, Ring::new(points))
}

/// A closed outline as points round it.
struct Ring {
    points: Vec<Point>,
    /// How far round each point is; and, last, the whole way round.
    at: Vec<f64>,
}

impl Ring {
    fn new(points: Vec<Point>) -> Ring {
        let mut at = Vec::with_capacity(points.len() + 1);
        let mut walked = 0.0;
        for (i, p) in points.iter().enumerate() {
            if i > 0 {
                walked += points[i - 1].distance(*p);
            }
            at.push(walked);
        }
        if let (Some(first), Some(last)) = (points.first(), points.last()) {
            walked += last.distance(*first);
        }
        at.push(walked);
        Ring { points, at }
    }

    /// An outline, moved by `by`.
    fn of(outline: &Outline, by: Vec2) -> Ring {
        let segs = outline.path_segs();
        let mut points: Vec<Point> = crate::geom::flatten_segs(&segs, FLAT)
            .into_iter()
            .map(|p| p + by)
            .collect();
        points.dedup_by(|a, b| a.distance(*b) < SAME_POINT);
        while points.len() > 1 && points[0].distance(points[points.len() - 1]) < SAME_POINT {
            points.pop();
        }
        Ring::new(points)
    }

    fn total(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The point `s` round it (round again past the end).
    fn point(&self, s: f64) -> Point {
        let n = self.points.len();
        if n == 0 {
            return Point::ZERO;
        }
        let total = self.total();
        let s = if total > 0.0 {
            s.rem_euclid(total)
        } else {
            0.0
        };
        let i = self.at[..n].partition_point(|&a| a <= s).saturating_sub(1);
        let (a, b) = (self.points[i], self.points[(i + 1) % n]);
        let l = self.at[i + 1] - self.at[i];
        if l <= 0.0 {
            a
        } else {
            a.lerp(b, (s - self.at[i]) / l)
        }
    }

    /// How far round the point nearest `p` is.
    fn nearest(&self, p: Point) -> f64 {
        let n = self.points.len();
        let mut best = (f64::MAX, 0.0);
        for i in 0..n {
            let (a, b) = (self.points[i], self.points[(i + 1) % n]);
            let line = kurbo::Line::new(a, b);
            let near = line.nearest(p, 1e-9);
            if near.distance_sq < best.0 {
                best = (near.distance_sq, self.at[i] + near.t * a.distance(b));
            }
        }
        best.1
    }

    /// Its corners: how far round, where, and whether it turns right there
    /// (outwards, on an outline running clockwise on screen).
    fn corners(&self) -> Vec<(f64, Point, bool)> {
        let n = self.points.len();
        let mut found: Vec<(f64, Point, bool, f64)> = Vec::new();
        for i in 0..n {
            let (s, p) = (self.at[i], self.points[i]);
            let (a, b) = (self.point(s - REACH), self.point(s + REACH));
            let (u, v) = (p - a, b - p);
            if u.hypot2() < 1e-12 || v.hypot2() < 1e-12 {
                continue;
            }
            let turn = u.cross(v).atan2(u.dot(v));
            if turn.abs() > CORNER {
                found.push((s, p, turn > 0.0, turn.abs()));
            }
        }
        // The sharpest of each cluster.
        let total = self.total();
        let apart = |a: f64, b: f64| {
            let d = (a - b).abs();
            d.min(total - d)
        };
        found
            .iter()
            .filter(|c| {
                !found.iter().any(|o| {
                    apart(o.0, c.0) < 1.5 * REACH
                        && o.3.total_cmp(&c.3).then(c.0.total_cmp(&o.0)).is_gt()
                })
            })
            .map(|c| (c.0, c.1, c.2))
            .collect()
    }
}

/// Where a point round one ring is round another: corners matched, and
/// in proportion between them.
struct Matched {
    /// How far round each matched corner is on the first ring, in order ...
    a: Vec<f64>,
    /// ... and on the second, carried on round past its end to keep going
    /// the same way.
    b: Vec<f64>,
    ta: f64,
    tb: f64,
}

impl Matched {
    fn of(from: &Ring, to: &Ring) -> Matched {
        let (cf, ct) = (from.corners(), to.corners());
        let size = Rect::from_points(from.points[0], from.points[0]);
        let size = from.points.iter().fold(size, |r, &p| r.union_pt(p));
        let near = NEAR.max(NEAR_SHARE * size.width().hypot(size.height()));
        let nearest = |c: &(f64, Point, bool), among: &[(f64, Point, bool)]| -> Option<usize> {
            among
                .iter()
                .enumerate()
                .filter(|(_, o)| o.2 == c.2 && o.1.distance(c.1) < near)
                .min_by(|x, y| x.1.1.distance(c.1).total_cmp(&y.1.1.distance(c.1)))
                .map(|(k, _)| k)
        };
        // Each other's nearest.
        let mut pairs: Vec<(f64, f64)> = cf
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let j = nearest(c, &ct)?;
                (nearest(&ct[j], &cf) == Some(i)).then_some((c.0, ct[j].0))
            })
            .collect();
        pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
        let tb = to.total();
        let mut kept = in_order(&pairs, tb);
        if kept.is_empty() {
            kept.push((0.0, to.nearest(from.point(0.0))));
        }
        let a = kept.iter().map(|k| k.0).collect();
        let mut b: Vec<f64> = Vec::with_capacity(kept.len());
        for (i, k) in kept.iter().enumerate() {
            b.push(if i == 0 {
                k.1
            } else {
                b[i - 1] + (k.1 - kept[i - 1].1).rem_euclid(tb)
            });
        }
        Matched {
            a,
            b,
            ta: from.total(),
            tb,
        }
    }

    /// How far round the second ring the point `s` round the first is.
    fn along(&self, s: f64) -> f64 {
        let (a, b, m) = (&self.a, &self.b, self.a.len());
        let s = s.rem_euclid(self.ta.max(f64::MIN_POSITIVE));
        let (j, s) = match a.partition_point(|&x| x <= s) {
            0 => (m - 1, s + self.ta),
            p => (p - 1, s),
        };
        let (a0, a1, b0, b1) = if j + 1 < m {
            (a[j], a[j + 1], b[j], b[j + 1])
        } else {
            (a[m - 1], a[0] + self.ta, b[m - 1], b[0] + self.tb)
        };
        let f = if a1 > a0 { (s - a0) / (a1 - a0) } else { 0.0 };
        (b0 + f * (b1 - b0)).rem_euclid(self.tb.max(f64::MIN_POSITIVE))
    }
}

/// The most of `pairs` (in order round the first ring) that also run in
/// order round the second, once round.
fn in_order(pairs: &[(f64, f64)], total: f64) -> Vec<(f64, f64)> {
    let m = pairs.len();
    let mut best: Vec<usize> = Vec::new();
    for start in 0..m {
        // Round the second ring from the start's corner.
        let round: Vec<f64> = (0..m)
            .map(|t| (pairs[(start + t) % m].1 - pairs[start].1).rem_euclid(total))
            .collect();
        // Longest run going up from the start, by dynamic programming.
        let mut length = vec![0usize; m];
        let mut prev = vec![usize::MAX; m];
        length[0] = 1;
        for t in 1..m {
            for u in 0..t {
                if length[u] > 0 && round[u] < round[t] && length[u] + 1 > length[t] {
                    length[t] = length[u] + 1;
                    prev[t] = u;
                }
            }
        }
        let Some(end) = (0..m).max_by_key(|&t| (length[t], usize::MAX - t)) else {
            continue;
        };
        if length[end] > best.len() {
            let mut chain = Vec::new();
            let mut t = end;
            while t != usize::MAX {
                chain.push((start + t) % m);
                t = prev[t];
            }
            best = chain;
        }
    }
    let mut kept: Vec<(f64, f64)> = best.into_iter().map(|i| pairs[i]).collect();
    kept.sort_by(|x, y| x.0.total_cmp(&y.0));
    kept
}

/// How far to move size `other` to lie over `base`: not at all when the
/// sizes are drawn nested; else by how they are worn (up and down) and by
/// a fold they share, or their middles (across).
fn shift(base: &Sized, other: &Sized) -> Vec2 {
    let (b, o) = (base.piece.outline.bounds(), other.piece.outline.bounds());
    if overlap(b, o) > NESTED {
        return Vec2::ZERO;
    }
    let up = base.height.map_or(Vec2::new(0.0, -1.0), |h| {
        Vec2::new(h.up.x, h.up.y).normalize()
    });
    let across = Vec2::new(-up.y, up.x);
    let (rb, ro) = (
        Ring::of(&base.piece.outline, Vec2::ZERO),
        Ring::of(&other.piece.outline, Vec2::ZERO),
    );
    let range = |ring: &Ring, d: Vec2| {
        ring.points
            .iter()
            .map(|p| p.to_vec2().dot(d))
            .fold((f64::MAX, f64::MIN), |(l, h), v| (l.min(v), h.max(v)))
    };
    let along_up = match (base.height, other.height) {
        (Some(hb), Some(ho)) => ho.at - hb.at,
        _ => range(&rb, up).1 - range(&ro, up).1,
    };
    let fold = |piece: &Piece| -> Option<f64> {
        let e = piece
            .outline
            .edges
            .iter()
            .position(|e| e.kind == EdgeKind::Fold)?;
        let points = piece.outline.edge_geom(e).flatten(FLAT);
        Some(
            points.iter().map(|p| p.to_vec2().dot(across)).sum::<f64>()
                / points.len().max(1) as f64,
        )
    };
    let side_to_side = match (fold(base.piece), fold(other.piece)) {
        (Some(fb), Some(fo)) => fb - fo,
        _ => {
            let (lb, hb) = range(&rb, across);
            let (lo, ho) = range(&ro, across);
            (lb + hb) / 2.0 - (lo + ho) / 2.0
        }
    };
    up * along_up + across * side_to_side
}

/// How much two boxes overlap: their shared area over their joint area.
fn overlap(a: Rect, b: Rect) -> f64 {
    let shared = a.intersect(b);
    let both = a.area() + b.area() - shared.area();
    if both > 0.0 && shared.width() > 0.0 && shared.height() > 0.0 {
        shared.area() / both
    } else {
        0.0
    }
}

/// How far along a polyline each of its points is.
fn running(points: &[Point]) -> Vec<f64> {
    let mut out = Vec::with_capacity(points.len());
    let mut walked = 0.0;
    for (i, p) in points.iter().enumerate() {
        if i > 0 {
            walked += points[i - 1].distance(*p);
        }
        out.push(walked);
    }
    out
}

/// A length `s` along `was`, as far along `now` (the same samples).
fn stretch(was: &[f64], now: &[f64], s: f64) -> f64 {
    let n = was.len().min(now.len());
    if n == 0 {
        return s;
    }
    let i = was[..n].partition_point(|&w| w <= s);
    if i == 0 {
        return now[0];
    }
    if i >= n {
        return now[n - 1];
    }
    let (w0, w1) = (was[i - 1], was[i]);
    let f = if w1 > w0 { (s - w0) / (w1 - w0) } else { 0.0 };
    now[i - 1] + f * (now[i] - now[i - 1])
}

/// A segment of the size blended from, blended: `points` its samples
/// blended, `start` and `end` how far its ends moved. A line or a curve
/// stays one where it still runs close to the blend (a curve's handles
/// moving with its ends); else it is drawn as short lines.
fn blended_segment(seg: &PathSeg, points: &[Point], start: Vec2, end: Vec2) -> Vec<Segment> {
    let (first, last) = (points[0], points[points.len() - 1]);
    let whole = match seg {
        PathSeg::Cubic(c) => {
            PathSeg::Cubic(kurbo::CubicBez::new(first, c.p1 + start, c.p2 + end, last))
        }
        PathSeg::Quad(q) => {
            let c = q.raise();
            PathSeg::Cubic(kurbo::CubicBez::new(first, c.p1 + start, c.p2 + end, last))
        }
        PathSeg::Line(_) => PathSeg::Line(kurbo::Line::new(first, last)),
    };
    let close = points
        .iter()
        .all(|&p| whole.nearest(p, 1e-6).distance_sq <= KEEP * KEEP);
    if close {
        return vec![Segment::from_path_seg(whole)];
    }
    simplified(points)
        .into_iter()
        .skip(1)
        .map(Segment::line)
        .collect()
}

/// A polyline with the points it doesn't need (within `STRAIGHT` of the
/// line past them) left out.
fn simplified(points: &[Point]) -> Vec<Point> {
    fn keep(points: &[Point], from: usize, to: usize, out: &mut Vec<bool>) {
        if to <= from + 1 {
            return;
        }
        let line = kurbo::Line::new(points[from], points[to]);
        let (far, d) = (from + 1..to)
            .map(|i| (i, line.nearest(points[i], 1e-9).distance_sq))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((from, 0.0));
        if d > STRAIGHT * STRAIGHT {
            out[far] = true;
            keep(points, from, far, out);
            keep(points, far, to, out);
        }
    }
    let n = points.len();
    if n <= 2 {
        return points.to_vec();
    }
    let mut kept = vec![false; n];
    kept[0] = true;
    kept[n - 1] = true;
    keep(points, 0, n - 1, &mut kept);
    points
        .iter()
        .zip(kept)
        .filter_map(|(p, k)| k.then_some(*p))
        .collect()
}

/// How far a point inside a polygon moves when its corners move by
/// `moved`: by mean value coordinates, which follow the outline exactly on
/// it and smoothly inside.
fn mean_value(polygon: &[Point], moved: &[Vec2], q: Point) -> Vec2 {
    let n = polygon.len();
    if n == 0 {
        return Vec2::ZERO;
    }
    let to: Vec<Vec2> = polygon.iter().map(|p| *p - q).collect();
    let r: Vec<f64> = to.iter().map(|v| v.hypot()).collect();
    if let Some(i) = r.iter().position(|&r| r < SAME_POINT) {
        return moved[i];
    }
    // tan(α/2) for the angle each side makes at q.
    let half_tan: Vec<f64> = (0..n)
        .map(|i| {
            let j = (i + 1) % n;
            let (cross, dot) = (to[i].cross(to[j]), to[i].dot(to[j]));
            let denom = r[i] * r[j] + dot;
            if denom.abs() < 1e-12 {
                0.0
            } else {
                cross / denom
            }
        })
        .collect();
    let mut total = 0.0;
    let mut sum = Vec2::ZERO;
    for i in 0..n {
        let w = (half_tan[(i + n - 1) % n] + half_tan[i]) / r[i];
        total += w;
        sum += moved[i] * w;
    }
    if total.abs() < 1e-12 {
        Vec2::ZERO
    } else {
        sum / total
    }
}
