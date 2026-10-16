//! Turning the paths of a drawing into pattern pieces, whatever the file
//! format. Every closed shape big enough to be a piece becomes one. A closed
//! shape running parallel inside another is taken as that piece's stitching
//! line (the outer one being its cutting line), and the allowance between
//! them is measured edge by edge. Other shapes and lines inside a piece
//! become its internal lines and marks; a straight line whose name mentions
//! "grain" becomes its grainline. Everything else is counted and reported,
//! never silently dropped.

use std::collections::HashMap;

use drapewright_pattern_core::allowance::allowance_lines;
use drapewright_pattern_core::build::{
    DEFAULT_CORNER_ANGLE, counter, outline_from_path, outline_from_path_at,
};
use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, point_in_polygon, polygon_area, ray_hit, unit_tangent,
};
use drapewright_pattern_core::ops::normalize_orientation;
use drapewright_pattern_core::validate::self_intersects;
use drapewright_pattern_core::{
    CutInstruction, EdgeGeom, EdgeKind, Grainline, Id, InternalLine, LineKind, Mark, MarkKind,
    Notch, NotchFrom, NotchKind, Outline, Piece, Pt, Segment, Source, SourceFormat, SourceScale,
};
use kurbo::{Line, ParamCurve, ParamCurveArclen, ParamCurveNearest, PathSeg, Point, Rect, Vec2};

use crate::import::{ImportNote, ScaleInfo};

/// Closed shapes smaller than this are marks or decorations, not pieces.
pub(crate) const MIN_PIECE_AREA: f64 = 400.0; // mm²
pub(crate) const MIN_PIECE_SPAN: f64 = 30.0; // mm, bounding-box diagonal

/// An open path whose ends are this close counts as closed, mm.
pub(crate) const CLOSE_TOLERANCE: f64 = 0.05;

/// Largest allowance believed when a cutting and stitching line pair up, mm.
const MAX_DETECTED_ALLOWANCE: f64 = 60.0;

/// Small closed shapes up to this size that are round become marks, mm.
const MAX_MARK_SIZE: f64 = 15.0;

/// Common allowances (mm) that measured widths snap to when within 0.2 mm.
const COMMON_ALLOWANCES: [f64; 20] = [
    3.0, 5.0, 6.0, 6.35, 7.0, 9.525, 10.0, 12.0, 12.7, 15.0, 15.875, 20.0, 25.0, 25.4, 30.0, 31.75,
    38.1, 40.0, 50.0, 50.8,
];

/// A path from the file, in millimetres.
#[derive(Clone, Debug)]
pub(crate) struct RawPath {
    pub segs: Vec<PathSeg>,
    pub closed: bool,
    pub filled: bool,
    /// Element id, label, class or layer, lower-cased, for keyword hints.
    pub hint: String,
    pub name: Option<String>,
    pub reference: Option<String>,
    /// How many to cut, when the file says for the piece this path belongs to.
    pub cut: Option<CutInstruction>,
}

/// A notch drawn as a symbol, at its centre, in millimetres.
pub(crate) struct RawNotch {
    pub at: Point,
    pub kind: NotchKind,
}

/// Notches drawn as symbols sit on the stitching line; this far off still counts, mm.
const NOTCH_REACH: f64 = 2.0;
/// A notch cut into the cutting line finds the stitching line this far in, mm.
const CUT_NOTCH_REACH: f64 = 60.0;

/// A line no further than this from an outline along its whole length is
/// drawn over it, mm.
const ON_OUTLINE: f64 = 0.25;

impl RawPath {
    fn polyline(&self) -> Vec<Point> {
        let mut pts = flatten_segs(&self.segs, 0.05);
        if self.closed && pts.len() > 1 {
            pts.pop();
        }
        pts
    }
}

/// A readable piece name from an id or label, or `None` for the generated
/// ones editors write ("path1234", "Layer 1", "XMLID_12_").
pub(crate) fn meaningful_name(raw: &str) -> Option<String> {
    let s = raw.trim();
    if let Some(name) = crate::svg::freesewing::part_name(s) {
        return Some(name);
    }
    if crate::svg::freesewing::is_generated_id(s) {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    let generated = [
        "path",
        "rect",
        "circle",
        "ellipse",
        "polygon",
        "polyline",
        "line",
        "g",
        "use",
        "layer",
        "svg",
        "text",
        "tspan",
        "group",
        "xmlid_",
        "calque",
        "ebene",
        "capa",
        "selection",
        "shape",
        "object",
        "clip",
        "mask",
    ]
    .iter()
    .any(|p| {
        lower.strip_prefix(p).is_some_and(|rest| {
            rest.chars()
                .all(|c| c.is_ascii_digit() || " _-".contains(c))
        })
    });
    if s.is_empty()
        || generated
        || s.chars()
            .all(|c| c.is_ascii_digit() || c == '_' || c == '-')
    {
        return None;
    }
    let spaced: String = s
        .chars()
        .map(|c| if c == '_' || c == '-' { ' ' } else { c })
        .collect();
    // Words naming which line this is, not which piece.
    const LINE_WORDS: [&str; 10] = [
        "cutting line",
        "cut line",
        "cutting",
        "cut",
        "stitching line",
        "stitching",
        "seam line",
        "seam",
        "outline",
        "sa",
    ];
    let mut name = spaced.trim().to_string();
    // "Front (cutting line)"
    if let Some(open) = name.rfind('(')
        && name.ends_with(')')
        && LINE_WORDS.contains(
            &name[open + 1..name.len() - 1]
                .trim()
                .to_ascii_lowercase()
                .as_str(),
        )
    {
        name.truncate(open);
        name = name.trim_end().to_string();
    }
    for word in LINE_WORDS {
        let lower = name.to_ascii_lowercase();
        if let Some(rest) = lower.strip_suffix(word)
            && rest.ends_with(' ')
        {
            name.truncate(rest.trim_end().len());
            break;
        }
    }
    let mut chars = name.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
}

pub(crate) fn bounds(points: &[Point]) -> Rect {
    points
        .iter()
        .fold(Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN), |r, p| {
            Rect::new(r.x0.min(p.x), r.y0.min(p.y), r.x1.max(p.x), r.y1.max(p.y))
        })
}

/// Points along a polyline at most `step` apart.
fn sample(points: &[Point], closed: bool, step: f64) -> Vec<Point> {
    let n = points.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    let mut out = Vec::new();
    for i in 0..count {
        let (a, b) = (points[i], points[(i + 1) % n]);
        let k = ((a.distance(b) / step).ceil() as usize).max(1);
        out.extend((0..k).map(|j| a.lerp(b, j as f64 / k as f64)));
    }
    if !closed && let Some(&last) = points.last() {
        out.push(last);
    }
    out
}

/// Whether most of `inner` lies inside or on `outer`.
fn inside(inner: &[Point], outer: &[Point]) -> bool {
    let probe = sample(inner, true, 5.0);
    let hits = probe
        .iter()
        .filter(|p| point_in_polygon(**p, outer) || distance_to_polyline(**p, outer, true) < 0.05)
        .count();
    hits * 10 >= probe.len() * 9
}

pub(crate) fn snap_allowance(w: f64) -> f64 {
    COMMON_ALLOWANCES
        .iter()
        .copied()
        .find(|c| (c - w).abs() <= 0.2)
        .unwrap_or_else(|| (w * 10.0).round() / 10.0)
}

pub(crate) fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// The indices of `keys`, least key first; equal keys keep their order.
/// Things are put in order by this one sort: each sort of its own adds
/// several kilobytes to the WebAssembly the app downloads.
pub(crate) fn ascending(keys: &[f64]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by(|&a, &b| keys[a].total_cmp(&keys[b]));
    order
}

/// If the piece's outline (a stitching line) runs parallel inside `outer`
/// (its cutting line), the allowance of each edge.
///
/// Each edge is measured straight out from the stitching line, as a
/// seamstress would with a ruler. The nearest point of the cutting line
/// would do for even allowances, but not beside a narrower one: near the
/// corner where a 40 mm hem meets a fold, the fold's cutting line is closer
/// than the hem's.
pub(crate) fn measure_allowances(piece: &Piece, outer: &[Point]) -> Option<Vec<f64>> {
    measure_allowances_avoiding(piece, outer, &[], Agreeing::All)
}

/// How many of an edge's measurements must agree on its allowance.
#[derive(Clone, Copy, PartialEq)]
enum Agreeing {
    /// All but those nearest its corners.
    All,
    /// Most of them: a cutting line shaped at the corners for a deep hem
    /// (turned back on itself so the hem folds up flat), a stray reading.
    Most,
}

/// Of an edge's measurements, this part must agree when most must: a
/// short edge by a deep hem is shaped for nearly half its length.
const MOST: f64 = 0.5;
/// An edge shorter than this, mm, has its allowance from the edge before
/// it when most measurements must agree: it has no room for any.
const TOO_SHORT_TO_MEASURE: f64 = 5.0;

/// Distance from stitching line `inner` at `along` on one of its edges
/// straight out to cutting line `outer`. None where that way slips past a
/// corner of `outer` into the next edge's allowance (beside an inside
/// corner) and runs on alongside the stitching line instead of away from it.
fn allowance_at(geom: &EdgeGeom, along: f64, inner: &[Point], outer: &[Point]) -> Option<f64> {
    let p = geom.point_at(along);
    let normal = geom.normal_at(along);
    match ray_hit(p, normal, outer) {
        Some(hit) => {
            let d = hit.distance(p);
            let halfway = p + normal * (d / 2.0);
            (distance_to_polyline(halfway, inner, true) >= 0.4 * d).then_some(d)
        }
        None => Some(distance_to_polyline(p, outer, true)),
    }
}

/// Like [`measure_allowances`], leaving out what lies within reach of the
/// given points: where one allowance turns into another.
fn measure_allowances_avoiding(
    piece: &Piece,
    outer: &[Point],
    avoid: &[(Point, f64)],
    agreeing: Agreeing,
) -> Option<Vec<f64>> {
    let inner = flatten_segs(&piece.outline.path_segs(), 0.05);
    let mut widths: Vec<f64> = Vec::new();
    for i in 0..piece.outline.edges.len() {
        let geom: EdgeGeom = piece.outline.edge_geom(i);
        let len = geom.length();
        // Stay clear of the corners, where corner shapes change the distance.
        let tenths = if avoid.is_empty() { 10 } else { 40 };
        let samples: Vec<f64> = (1..tenths)
            .map(|k| len * f64::from(k) / f64::from(tenths))
            .filter(|&along| {
                let p = geom.point_at(along);
                avoid.iter().all(|(at, reach)| at.distance(p) > reach + 2.0)
            })
            .filter_map(|along| allowance_at(&geom, along, &inner, outer))
            .collect();
        if samples.len() < 3 {
            if agreeing == Agreeing::Most && len < TOO_SHORT_TO_MEASURE {
                widths.push(f64::NAN);
                continue;
            }
            return None;
        }
        // The samples nearest the corners may be pulled by them on a short
        // edge; the rest must agree.
        let core = if samples.len() >= 5 {
            &samples[1..samples.len() - 1]
        } else {
            &samples[..]
        };
        let (lo, hi) = core
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(*d), hi.max(*d)));
        // Most agreeing: on the middle of the core, the corners' readings
        // left aside.
        let mid = match agreeing {
            Agreeing::All => median(samples.clone()),
            Agreeing::Most => median(core.to_vec()),
        };
        let tolerance = 0.5 + 0.05 * mid;
        let agree = match agreeing {
            Agreeing::All => hi - lo <= tolerance,
            Agreeing::Most => {
                let near = core
                    .iter()
                    .filter(|d| (**d - mid).abs() <= tolerance / 2.0)
                    .count();
                near as f64 >= MOST * core.len() as f64
            }
        };
        if mid > MAX_DETECTED_ALLOWANCE || !agree {
            return None;
        }
        widths.push(snap_allowance(mid));
    }
    // Edges too short to measure: as the edge before them.
    let measured = widths.iter().position(|w| !w.is_nan())?;
    let n = widths.len();
    for k in 1..=n {
        let i = (measured + k) % n;
        if widths[i].is_nan() {
            widths[i] = widths[(i + n - 1) % n];
        }
    }
    Some(widths)
}

/// Where an allowance changes partway along an edge (a cut-on fly or
/// facing that ends before its edge does): the points to break edges at,
/// each with how far the change spreads either side of it, mm.
fn allowance_steps(piece: &Piece, outer: &[Point]) -> Option<Vec<(Point, f64)>> {
    let tolerance = |w: f64| 0.5 + 0.05 * w;
    let inner = flatten_segs(&piece.outline.path_segs(), 0.05);
    let mut steps = Vec::new();
    for i in 0..piece.outline.edges.len() {
        let geom = piece.outline.edge_geom(i);
        let len = geom.length();
        let count = ((len / 2.0) as usize).clamp(10, 400);
        let spacing = 0.8 * len / (count - 1) as f64;
        let along: Vec<f64> = (0..count).map(|k| 0.1 * len + spacing * k as f64).collect();
        let d: Vec<f64> = along
            .iter()
            .map(|&s| allowance_at(&geom, s, &inner, outer).unwrap_or(f64::NAN))
            .collect();
        // Runs of samples that agree: (first, last, width).
        let mut runs: Vec<(usize, usize, f64)> = Vec::new();
        let mut k = 0;
        while k < count {
            let mut j = k;
            while j + 1 < count && (d[j + 1] - d[k]).abs() <= tolerance(d[k]) {
                j += 1;
            }
            runs.push((k, j, median(d[k..=j].to_vec())));
            k = j + 1;
        }
        let long: Vec<(usize, usize, f64)> = runs
            .into_iter()
            .filter(|&(a, b, _)| b - a >= 4 && (b - a) as f64 * spacing >= 10.0)
            .collect();
        let covered: usize = long.iter().map(|(a, b, _)| b - a + 1).sum();
        if long.is_empty()
            || covered * 10 < count * 7
            || long.iter().any(|r| r.2 > MAX_DETECTED_ALLOWANCE)
        {
            return None;
        }
        for pair in long.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if (a.2 - b.2).abs() <= tolerance(a.2.max(b.2)) {
                continue;
            }
            // Where the allowance is halfway from one width to the other.
            let half = (a.2 + b.2) / 2.0;
            let wider_first = a.2 > b.2;
            let (mut lo, mut hi) = (along[a.1], along[b.0]);
            for _ in 0..30 {
                let m = (lo + hi) / 2.0;
                let d = allowance_at(&geom, m, &inner, outer).unwrap_or(half);
                if (d > half) == wider_first {
                    lo = m;
                } else {
                    hi = m;
                }
            }
            let at = (lo + hi) / 2.0;
            let spread = (at - along[a.1]).max(along[b.0] - at);
            steps.push((geom.point_at(at), spread));
        }
    }
    (!steps.is_empty()).then_some(steps)
}

/// The piece's stitching line, with its edges broken at `steps` as well as
/// at its corners.
fn break_edges(
    piece: &Piece,
    steps: &[(Point, f64)],
    ids: &mut dyn FnMut(&str) -> Id,
) -> Option<Outline> {
    let outline = &piece.outline;
    let mut segs = outline.path_segs();
    let mut corners: Vec<Point> = (0..outline.edges.len())
        .map(|i| Point::from(outline.edge_start(i)))
        .collect();
    for &(p, _) in steps {
        let (k, near) = segs
            .iter()
            .enumerate()
            .map(|(k, s)| (k, s.nearest(p, 1e-6)))
            .min_by(|a, b| a.1.distance_sq.total_cmp(&b.1.distance_sq))?;
        let seg = segs[k];
        if near.t > 1e-6 && near.t < 1.0 - 1e-6 {
            segs.splice(
                k..=k,
                [seg.subsegment(0.0..near.t), seg.subsegment(near.t..1.0)],
            );
        }
        corners.push(seg.eval(near.t));
    }
    outline_from_path_at(&segs, &corners, 1e-6, DEFAULT_CORNER_ANGLE, ids)
}

/// Length of a polyline, mm; `closed` counts the way back to the start.
fn polyline_length(points: &[Point], closed: bool) -> f64 {
    let open: f64 = points.windows(2).map(|w| w[0].distance(w[1])).sum();
    match (closed, points.first(), points.last()) {
        (true, Some(a), Some(b)) => open + a.distance(*b),
        _ => open,
    }
}

/// Where on closed path `ring` is nearest `p`: segment and parameter.
fn locate_on(ring: &[PathSeg], p: Point) -> Option<(usize, f64)> {
    ring.iter()
        .enumerate()
        .map(|(k, s)| (k, s.nearest(p, 1e-6)))
        .min_by(|a, b| a.1.distance_sq.total_cmp(&b.1.distance_sq))
        .map(|(k, n)| (k, n.t))
}

/// Closed path `ring` from one place on it forward to another.
fn arc_of(ring: &[PathSeg], from: (usize, f64), to: (usize, f64)) -> Vec<PathSeg> {
    let n = ring.len();
    let keep = |s: PathSeg| (s.start().distance(s.end()) > 1e-9).then_some(s);
    if from.0 == to.0 && from.1 <= to.1 {
        return keep(ring[from.0].subsegment(from.1..to.1))
            .into_iter()
            .collect();
    }
    let mut out: Vec<PathSeg> = keep(ring[from.0].subsegment(from.1..1.0))
        .into_iter()
        .collect();
    let mut k = (from.0 + 1) % n;
    while k != to.0 {
        out.push(ring[k]);
        k = (k + 1) % n;
    }
    out.extend(keep(ring[to.0].subsegment(0.0..to.1)));
    out
}

/// Open path `path`, whose ends lie on closed path `ring`, closed along
/// `ring` the way that encloses more: the closed path, and how long the
/// part along `ring` is.
fn close_along(path: &[PathSeg], ring: &[PathSeg]) -> Option<(Vec<PathSeg>, f64)> {
    let (start, end) = (path.first()?.start(), path.last()?.end());
    let (at_start, at_end) = (locate_on(ring, start)?, locate_on(ring, end)?);
    let forward = arc_of(ring, at_end, at_start);
    let backward: Vec<PathSeg> = arc_of(ring, at_start, at_end)
        .iter()
        .rev()
        .map(PathSeg::reverse)
        .collect();
    let close = |arc: Vec<PathSeg>| {
        let mut segs = path.to_vec();
        if let Some(first) = arc.first()
            && first.start().distance(end) > 1e-9
        {
            segs.push(PathSeg::Line(Line::new(end, first.start())));
        }
        let along: f64 = arc.iter().map(|s| s.arclen(0.01)).sum();
        let last = arc.last().map_or(end, |s| s.end());
        segs.extend(arc);
        if last.distance(start) > 1e-9 {
            segs.push(PathSeg::Line(Line::new(last, start)));
        }
        let area = polygon_area(&flatten_segs(&segs, 0.5)).abs();
        (segs, along, area)
    };
    let (a, b) = (close(forward), close(backward));
    let (segs, along, _) = if a.2 >= b.2 { a } else { b };
    Some((segs, along))
}

/// Largest distance between two closed polylines, in either direction.
pub(crate) fn hausdorff(a: &[Point], b: &[Point]) -> f64 {
    let one_way = |from: &[Point], to: &[Point]| {
        sample(from, true, 2.0)
            .iter()
            .map(|p| distance_to_polyline(*p, to, true))
            .fold(0.0, f64::max)
    };
    one_way(a, b).max(one_way(b, a))
}

pub(crate) fn to_segments(segs: &[PathSeg]) -> Vec<Segment> {
    segs.iter().map(|s| Segment::from_path_seg(*s)).collect()
}

/// A title block holds at least this many drawings…
const KEY_DRAWINGS: usize = 3;
/// …each smaller than this part of it (a nest of sizes is about as big as
/// its largest).
const KEY_PART: f64 = 0.25;

/// A table has at least this many boxes, side by side in two rows and two
/// columns or more.
const TABLE_CELLS: usize = 6;
/// Boxes are side by side when their sides meet this nearly, mm.
const TABLE_MEET: f64 = 0.5;
/// A table's boxes are no longer than this, mm: bigger boxes side by side
/// are pages' frames.
const TABLE_CELL: f64 = 100.0;

/// Tables: small boxes (`cells`, by their bounds) set side by side in rows
/// and columns, each sharing a whole side with the next: a key to the
/// pattern's marks, a size chart. Pieces are never drawn that way. Each
/// table's boxes, by their place in `cells`.
fn tables(cells: &[Rect]) -> Vec<Vec<usize>> {
    let near = |a: f64, b: f64| (a - b).abs() <= TABLE_MEET;
    let small = |a: &Rect| a.width().max(a.height()) <= TABLE_CELL;
    let beside = |a: &Rect, b: &Rect| {
        let across = (near(a.x1, b.x0) || near(a.x0, b.x1)) && near(a.y0, b.y0) && near(a.y1, b.y1);
        let down = (near(a.y1, b.y0) || near(a.y0, b.y1)) && near(a.x0, b.x0) && near(a.x1, b.x1);
        small(a) && small(b) && (across || down)
    };
    let mut group: Vec<usize> = (0..cells.len()).collect();
    fn root(group: &mut [usize], mut i: usize) -> usize {
        while group[i] != i {
            group[i] = group[group[i]];
            i = group[i];
        }
        i
    }
    for a in 0..cells.len() {
        for b in a + 1..cells.len() {
            if beside(&cells[a], &cells[b]) {
                let (ra, rb) = (root(&mut group, a), root(&mut group, b));
                group[ra] = rb;
            }
        }
    }
    let mut by_root: HashMap<usize, Vec<usize>> = HashMap::new();
    for k in 0..cells.len() {
        let r = root(&mut group, k);
        by_root.entry(r).or_default().push(k);
    }
    let lines = |ends: Vec<f64>| {
        let mut ends = ends;
        ends.sort_by(f64::total_cmp);
        ends.dedup_by(|a, b| near(*a, *b));
        ends.len()
    };
    let mut found: Vec<Vec<usize>> = by_root
        .into_values()
        .filter(|table| {
            table.len() >= TABLE_CELLS
                && lines(table.iter().map(|&k| cells[k].y0).collect()) >= 2
                && lines(table.iter().map(|&k| cells[k].x0).collect()) >= 2
        })
        .map(|mut table| {
            table.sort_unstable();
            table
        })
        .collect();
    found.sort();
    found
}

/// Whether a closed path is a box: four straight sides, along the page.
pub(crate) fn is_box(segs: &[PathSeg]) -> bool {
    let mut corners: Vec<Point> = Vec::new();
    for seg in segs {
        let PathSeg::Line(line) = seg else {
            return false;
        };
        if line.p0.distance(line.p1) < 0.1 {
            continue;
        }
        let along = (line.p0.x - line.p1.x).abs() < 0.5 || (line.p0.y - line.p1.y).abs() < 0.5;
        if !along {
            return false;
        }
        if corners.last().is_none_or(|c| c.distance(line.p0) > 0.1) {
            corners.push(line.p0);
        }
    }
    // Sides drawn in several strokes still make four corners.
    let turns = corners
        .iter()
        .zip(corners.iter().cycle().skip(1))
        .zip(corners.iter().cycle().skip(2))
        .filter(|((a, b), c)| (**b - **a).cross(**c - **b).abs() > 1.0)
        .count();
    turns == 4
}

pub(crate) fn is_straight(points: &[Point]) -> bool {
    let (Some(&a), Some(&b)) = (points.first(), points.last()) else {
        return false;
    };
    let len = a.distance(b);
    len > 1.0
        && points
            .iter()
            .all(|p| ((*p - a).cross(b - a) / len).abs() < 0.1)
}

/// A notch where `at` meets the piece's outline, if it is within `reach` mm
/// of it, measured from the nearer end of its edge.
pub(crate) fn notch_at(
    piece: &Piece,
    at: Point,
    reach: f64,
    kind: NotchKind,
    id: String,
) -> Option<Notch> {
    let (i, near) = (0..piece.outline.edges.len())
        .filter_map(|i| piece.outline.edge_geom(i).nearest(at).map(|n| (i, n)))
        .min_by(|a, b| a.1.distance.total_cmp(&b.1.distance))?;
    if near.distance > reach {
        return None;
    }
    let len = piece.outline.edge_geom(i).length();
    let from_end = near.along > len / 2.0;
    Some(Notch {
        id,
        edge: piece.outline.edges[i].id.clone(),
        at: if from_end {
            len - near.along
        } else {
            near.along
        },
        from: if from_end {
            NotchFrom::End
        } else {
            NotchFrom::Start
        },
        kind,
        angle: 0.0,
    })
}

/// Distance from `p` to the infinite line through `a` and `b`.
fn off_line(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    ((p - a).cross(ab) / ab.hypot().max(1e-9)).abs()
}

/// Most a cutting line may be left open between its ends, mm.
const MAX_GAP: f64 = 30.0;

/// How an open cutting line was closed, by the line from its end to its start.
#[derive(Clone, Copy)]
enum Closing {
    /// Along a fold: that line is the fold.
    Fold(Point, Point),
    /// Across a gap left at a corner: not part of the drawing.
    Gap,
}

/// Cutting lines drawn open, as FreeSewing draws them:
/// - **Fold:** a piece cut on the fold is left open along the fold. Both ends
///   lie on the line of one straight edge of its stitching line.
/// - **Gap:** other pieces are left open at the corner where the path
///   starts, with the ends a few mm apart.
///
/// Closed with a straight line, such a path goes around a closed shape (the
/// stitching line). Those paths are closed; for folds, the closing line is
/// returned too.
fn close_open_outlines(raw: &mut [RawPath]) -> Vec<Option<Closing>> {
    let mut closings = vec![None; raw.len()];
    // Only shapes big enough to be pieces can be stitching lines.
    let shapes: Vec<(usize, Vec<Point>, Rect)> = raw
        .iter()
        .enumerate()
        .filter(|(_, r)| r.closed)
        .map(|(i, r)| {
            let poly = r.polyline();
            let b = bounds(&poly);
            (i, poly, b)
        })
        .filter(|(_, poly, b)| {
            polygon_area(poly).abs() >= MIN_PIECE_AREA
                && b.width().hypot(b.height()) >= MIN_PIECE_SPAN
        })
        .collect();
    for i in 0..raw.len() {
        if raw[i].closed || raw[i].segs.len() < 2 {
            continue;
        }
        let (start, end) = (
            raw[i].segs[0].start(),
            raw[i].segs[raw[i].segs.len() - 1].end(),
        );
        if start.distance(end) < CLOSE_TOLERANCE {
            continue;
        }
        let along_edge = |shape: usize| {
            raw[shape].segs.iter().any(|seg| match seg {
                PathSeg::Line(l) => {
                    l.p0.distance(l.p1) > 1.0
                        && off_line(start, l.p0, l.p1) <= 0.5
                        && off_line(end, l.p0, l.p1) <= 0.5
                }
                _ => false,
            })
        };
        let near = start.distance(end) <= MAX_GAP;
        if !near && !shapes.iter().any(|(k, _, _)| along_edge(*k)) {
            continue;
        }
        let mut closed = raw[i].polyline();
        closed.push(end);
        // Within the closed path's bounds first: a cheap test that rules most out.
        let reach = bounds(&closed).inflate(1.0, 1.0);
        let around = |k: usize, poly: &Vec<Point>, b: &Rect| {
            k != i
                && reach.contains(b.origin())
                && reach.contains(Point::new(b.x1, b.y1))
                && inside(poly, &closed)
        };
        let fold = shapes
            .iter()
            .any(|(k, poly, b)| along_edge(*k) && around(*k, poly, b));
        let gap = !fold && near && shapes.iter().any(|(k, poly, b)| around(*k, poly, b));
        if fold || gap {
            raw[i]
                .segs
                .push(PathSeg::Line(kurbo::Line::new(end, start)));
            raw[i].closed = true;
            closings[i] = Some(if fold {
                Closing::Fold(end, start)
            } else {
                Closing::Gap
            });
        }
    }
    closings
}

/// Marks the piece's edges that lie along `hem` (a polyline on its
/// outline) as hems.
pub(crate) fn mark_hems(piece: &mut Piece, hem: &[Point]) {
    const ON_HEM: f64 = 1.0;
    for i in 0..piece.outline.edges.len() {
        let geom = piece.outline.edge_geom(i);
        let length = geom.length();
        let under = [0.1, 0.5, 0.9]
            .iter()
            .all(|&t| distance_to_polyline(geom.point_at(t * length), hem, false) <= ON_HEM);
        if under && piece.outline.edges[i].kind == EdgeKind::Seam {
            piece.outline.edges[i].kind = EdgeKind::Hem;
        }
    }
}

/// Marks the piece's straight edges that lie along the line through `a` and
/// `b` as folds.
pub(crate) fn mark_folds(piece: &mut Piece, a: Point, b: Point) {
    for i in 0..piece.outline.edges.len() {
        let (s, e) = (
            Point::from(piece.outline.edge_start(i)),
            Point::from(piece.outline.edge_end(i)),
        );
        let straight = is_straight(&flatten_segs(piece.outline.edge_geom(i).segs(), 0.05));
        if straight && off_line(s, a, b) <= 0.5 && off_line(e, a, b) <= 0.5 {
            piece.outline.edges[i].kind = EdgeKind::Fold;
            piece.outline.edges[i].allowance = None;
        }
    }
}

/// The straight run of a grainline drawn as a bracket (FreeSewing's "cut on
/// fold and grainline"): its longest straight segment, if that is most of it.
fn grain_segment(segs: &[PathSeg]) -> Option<(Point, Point)> {
    let pts = flatten_segs(segs, 0.05);
    if is_straight(&pts) {
        return Some((pts[0], pts[pts.len() - 1]));
    }
    let total: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    segs.iter()
        .filter_map(|s| match s {
            PathSeg::Line(l) => Some((l.p0, l.p1)),
            _ => None,
        })
        .max_by(|a, b| a.0.distance(a.1).total_cmp(&b.0.distance(b.1)))
        .filter(|(a, b)| a.distance(*b) >= 0.6 * total)
}

/// A path goes on across a line it meets if it turns at most 60° (the
/// cosine).
const JUNCTION_STRAIGHT: f64 = 0.5;
/// Going on this sharply back (the cosine), where several lines meet, is
/// no way on: the sizes of a nest meeting at a corner they share.
const SHARPLY_BACK: f64 = -0.9;
/// Only lines at least this long, mm, are sizes' outlines and the
/// stretches they share: a lettered word's strokes turn sharply and meet
/// everywhere, and join as they always have.
const GUARDED_LONG: f64 = 10.0;

/// A line placed by the point halfway along it has that point at least
/// this far inside its piece, mm: not a tick across the outline.
const LINE_WELL_INSIDE: f64 = 1.0;

/// A line placed by the piece it runs across most runs at least this far
/// across it (and a quarter of its length), mm.
const LINE_MOSTLY_ACROSS: f64 = 15.0;

/// The point halfway along a line (a straight line's middle, not an end).
fn halfway(pts: &[Point]) -> Option<Point> {
    let total: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    let mut left = total / 2.0;
    for w in pts.windows(2) {
        let d = w[0].distance(w[1]);
        if left <= d && d > 0.0 {
            return Some(w[0].lerp(w[1], left / d));
        }
        left -= d;
    }
    pts.first().copied()
}

/// The point `s` mm along `segs`.
fn point_along(segs: &[PathSeg], s: f64) -> Point {
    let mut left = s;
    for seg in segs {
        let length = seg.arclen(0.01);
        if left <= length {
            return seg.eval(seg.inv_arclen(left, 0.01));
        }
        left -= length;
    }
    segs.last().map_or(Point::ZERO, PathSeg::end)
}

/// Whether `away`, leaving where `arriving` ends, goes back along it: a
/// copy of the same line (each size of a nest draws the stretches the
/// sizes share).
fn retraces(arriving: PathSeg, away: &[PathSeg]) -> bool {
    let along: f64 = away.iter().map(|s| s.arclen(0.01)).sum();
    let probe = 1.0_f64.min(0.5 * arriving.arclen(0.01)).min(0.5 * along);
    probe > 0.0 && arriving.nearest(point_along(away, probe), 1e-6).distance_sq <= 0.02 * 0.02
}

/// Joins open paths that meet end to end (within `tolerance` mm) into
/// longer ones, reversing pieces as needed, and reports which came back to
/// their start. CAD drawings often build an outline from separate lines and
/// arcs.
pub(crate) fn chain(paths: Vec<Vec<PathSeg>>, tolerance: f64) -> Vec<(Vec<PathSeg>, bool)> {
    chain_crowded(paths, tolerance, &|_| false)
}

/// `chain`, never going sharply back where `crowded` says many lines meet
/// (the sizes of a nest at a corner they share), even by the one way
/// there: two such sizes are not one outline.
pub(crate) fn chain_crowded(
    paths: Vec<Vec<PathSeg>>,
    tolerance: f64,
    crowded: &dyn Fn(Point) -> bool,
) -> Vec<(Vec<PathSeg>, bool)> {
    let close = |a: Point, b: Point| a.distance(b) <= tolerance;
    let mut pool: Vec<Option<Vec<PathSeg>>> = paths
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(Some)
        .collect();
    // Each path's two ends, by grid cell, so neighbours are found without
    // comparing every pair (PDFs can have tens of thousands of segments).
    let cell = (tolerance * 4.0).max(1e-6);
    let key = |p: Point| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, p) in pool.iter().enumerate() {
        let p = p.as_ref().expect("all present");
        for end in [p[0].start(), p[p.len() - 1].end()] {
            let cell = grid.entry(key(end)).or_default();
            if cell.last() != Some(&i) {
                cell.push(i);
            }
        }
    }
    let near = |grid: &HashMap<(i64, i64), Vec<usize>>, p: Point| {
        let (kx, ky) = key(p);
        let mut found: Vec<usize> = Vec::new();
        for dx in -1..=1 {
            for dy in -1..=1 {
                if let Some(c) = grid.get(&(kx + dx, ky + dy)) {
                    found.extend(c);
                }
            }
        }
        found
    };
    let mut out = Vec::new();
    for i in 0..pool.len() {
        let Some(mut path) = pool[i].take() else {
            continue;
        };
        loop {
            let (start, end) = (path[0].start(), path[path.len() - 1].end());
            if path.len() > 1 && close(start, end) {
                break;
            }
            // Where several lines meet (an outline with a notch cut at one of
            // its corners), go on along the one that turns least, as a pencil
            // tracing the outline would; on a tie, the first in file order.
            // The path's end is extended first, then its start.
            let heading = |seg: &PathSeg, t: f64| unit_tangent(seg, t);
            let ahead = heading(&path[path.len() - 1], 1.0);
            let behind = -heading(&path[0], 0.0);
            let turn = |j: usize, at_end: bool| -> Option<(f64, Vec2)> {
                let q = pool[j].as_ref()?;
                let (qs, qe) = (q[0].start(), q[q.len() - 1].end());
                let (first, last) = (heading(&q[0], 0.0), heading(&q[q.len() - 1], 1.0));
                let (way, along) = if at_end {
                    (end, ahead)
                } else {
                    (start, behind)
                };
                let goes = if at_end {
                    if close(way, qs) {
                        first
                    } else if close(way, qe) {
                        -last
                    } else {
                        return None;
                    }
                } else if close(way, qe) {
                    -last
                } else if close(way, qs) {
                    first
                } else {
                    return None;
                };
                Some((along.dot(goes), goes))
            };
            // The path oriented away from the meeting point at `at_end`.
            let away = |j: usize, at_end: bool| -> Option<Vec<PathSeg>> {
                let q = pool[j].as_ref()?;
                let way = if at_end { end } else { start };
                Some(if close(way, q[0].start()) {
                    q.clone()
                } else {
                    q.iter().rev().map(PathSeg::reverse).collect()
                })
            };
            let arriving = |at_end: bool| {
                if at_end {
                    path[path.len() - 1]
                } else {
                    path[0].reverse()
                }
            };
            // A path meeting a line that runs on through the meeting point
            // both ways, with none of it straight ahead (a notch cut meeting
            // the outline it marks), stops there. So does a long one that
            // could only go back along a copy of itself, or sharply back
            // along another long line, or along anything where many lines
            // meet (a dashed outline's stub at a corner its sizes share).
            let long =
                |segs: &[PathSeg]| segs.iter().map(|s| s.arclen(0.01)).sum::<f64>() >= GUARDED_LONG;
            let guarded = |j: usize, at: Point| {
                long(&path) && (pool[j].as_ref().is_some_and(|q| long(q)) || crowded(at))
            };
            let pick = |at: Point, at_end: bool| {
                let mut ways: Vec<(usize, f64, Vec2)> = near(&grid, at)
                    .into_iter()
                    .filter_map(|j| turn(j, at_end).map(|(straight, goes)| (j, straight, goes)))
                    .filter(|w| {
                        !guarded(w.0, at)
                            || away(w.0, at_end).is_none_or(|a| !retraces(arriving(at_end), &a))
                    })
                    .collect();
                ways.sort_unstable_by_key(|w| w.0);
                ways.dedup_by_key(|w| w.0);
                let best = ways
                    .iter()
                    .copied()
                    .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)))?;
                if guarded(best.0, at) && (ways.len() >= 2 || crowded(at)) && best.1 < SHARPLY_BACK
                {
                    return None;
                }
                let through = ways
                    .iter()
                    .any(|a| ways.iter().any(|b| a.2.dot(b.2) < -0.9));
                (best.1 >= JUNCTION_STRAIGHT || !through).then_some(best.0)
            };
            let next = pick(end, true).or_else(|| pick(start, false));
            let Some(j) = next else { break };
            let q = pool[j].take().expect("found above");
            let (qs, qe) = (q[0].start(), q[q.len() - 1].end());
            let reversed = || q.iter().rev().map(PathSeg::reverse).collect::<Vec<_>>();
            if close(end, qs) {
                path.extend(q.iter().copied());
            } else if close(end, qe) {
                path.extend(reversed());
            } else if close(start, qe) {
                let mut joined = q.clone();
                joined.extend(path);
                path = joined;
            } else {
                let mut joined = reversed();
                joined.extend(path);
                path = joined;
            }
        }
        // A speck of a stroke has its ends together but goes round nothing.
        let length: f64 = path.iter().map(|s| s.arclen(0.01)).sum();
        let closed = close(path[0].start(), path[path.len() - 1].end()) && length > 4.0 * tolerance;
        out.push((path, closed));
    }
    out
}

/// Plotter drivers can leave hairline gaps where one stroke ends and the
/// next begins (round a logo's letters, say). Ends of open paths up to
/// `reach` apart are joined across the gap with a short line, nearest pairs
/// first, each end once; a path whose own ends are that close is closed.
/// Never sharply back where `crowded` says many lines meet; and with
/// `straight_on` (reach, length), also joining ends up to that reach apart
/// of lines at least that long that run straight on into each other.
pub(crate) fn bridge_crowded(
    paths: Vec<(Vec<PathSeg>, bool)>,
    reach: f64,
    crowded: &dyn Fn(Point) -> bool,
    straight_on: Option<(f64, f64)>,
) -> Vec<(Vec<PathSeg>, bool)> {
    let (mut out, open): (Vec<_>, Vec<_>) = paths.into_iter().partition(|(_, closed)| *closed);
    let open: Vec<Vec<PathSeg>> = open.into_iter().map(|(segs, _)| segs).collect();
    // End 2k is path k's start, 2k + 1 its end.
    let point = |e: usize| {
        let p = &open[e / 2];
        if e.is_multiple_of(2) {
            p[0].start()
        } else {
            p[p.len() - 1].end()
        }
    };
    let far = straight_on.map_or(reach, |(r, _)| r.max(reach));
    let cell = far.max(1e-6);
    let key = |p: Point| ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for e in 0..2 * open.len() {
        grid.entry(key(point(e))).or_default().push(e);
    }
    let length = |segs: &[PathSeg]| segs.iter().map(|s| s.arclen(0.01)).sum::<f64>();
    let lengths: Vec<f64> = open.iter().map(|p| length(p)).collect();
    // Each end's path, arriving at it and leaving it.
    let arriving = |e: usize| {
        let p = &open[e / 2];
        if e.is_multiple_of(2) {
            p[0].reverse()
        } else {
            p[p.len() - 1]
        }
    };
    let away = |e: usize| -> Vec<PathSeg> {
        let p = &open[e / 2];
        if e.is_multiple_of(2) {
            p.clone()
        } else {
            p.iter().rev().map(PathSeg::reverse).collect()
        }
    };
    let mut pairs: Vec<(f64, usize, usize)> = Vec::new();
    let mut crowd = vec![0usize; 2 * open.len()];
    for e in 0..2 * open.len() {
        let (kx, ky) = key(point(e));
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &f in grid.get(&(kx + dx, ky + dy)).map_or(&[][..], Vec::as_slice) {
                    let gap = point(e).distance(point(f));
                    if f != e && gap <= reach {
                        crowd[e] += 1;
                    }
                    let guarded = lengths[e / 2] >= GUARDED_LONG && lengths[f / 2] >= GUARDED_LONG;
                    // A path's own ends close it only when it is no mere
                    // speck of a stroke.
                    let fits = if e / 2 == f / 2 {
                        lengths[e / 2] > 10.0 * reach
                    } else {
                        // Nor going back along a copy of itself.
                        !guarded || !retraces(arriving(e), &away(f))
                    };
                    // Farther, only long lines running straight on.
                    let near = gap <= reach
                        || straight_on.is_some_and(|(r, long)| {
                            let into = unit_tangent(&arriving(e), 1.0);
                            let on = unit_tangent(&away(f)[0], 0.0);
                            let across = (point(f) - point(e)).cross(into).abs();
                            gap <= r
                                && lengths[e / 2] >= long
                                && lengths[f / 2] >= long
                                && into.dot(on) >= 0.98
                                && across <= 0.2
                        });
                    if f > e && near && fits {
                        // Of long lines' ends as near, the one going
                        // straight on, as a pencil tracing the line would.
                        let cost = if guarded {
                            let into = unit_tangent(&arriving(e), 1.0);
                            let on = unit_tangent(&away(f)[0], 0.0);
                            gap * (1.0 + (1.0 - into.dot(on)) / 2.0)
                        } else {
                            gap
                        };
                        pairs.push((cost, e, f));
                    }
                }
            }
        }
    }
    // Where several ends meet, none is joined sharply back to another (the
    // sizes of a nest meeting at a corner they share).
    pairs.retain(|&(_, e, f)| {
        let many = crowd[e] >= 2 || crowd[f] >= 2 || crowded(point(e)) || crowded(point(f));
        let long = lengths[e / 2] >= GUARDED_LONG && lengths[f / 2] >= GUARDED_LONG;
        if !many || !long || e / 2 == f / 2 {
            return true;
        }
        let into = unit_tangent(&arriving(e), 1.0);
        let on = unit_tangent(&away(f)[0], 0.0);
        into.dot(on) >= SHARPLY_BACK
    });
    pairs.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut partner: Vec<Option<usize>> = vec![None; 2 * open.len()];
    for (_, e, f) in pairs {
        if partner[e].is_none() && partner[f].is_none() {
            partner[e] = Some(f);
            partner[f] = Some(e);
        }
    }
    // Follow each run of partnered paths from a free end (or anywhere round
    // a loop), joining them in order.
    let mut used = vec![false; open.len()];
    let oriented = |k: usize, from: usize| -> Vec<PathSeg> {
        if from.is_multiple_of(2) {
            open[k].clone()
        } else {
            open[k].iter().rev().map(PathSeg::reverse).collect()
        }
    };
    for k0 in 0..open.len() {
        if used[k0] {
            continue;
        }
        // Back to where the run begins.
        let mut begin = 2 * k0;
        for _ in 0..open.len() {
            match partner[begin] {
                Some(p) if p / 2 != k0 => begin = p ^ 1,
                _ => break,
            }
        }
        let first = begin / 2;
        let mut path = oriented(first, begin);
        used[first] = true;
        let mut exit = begin ^ 1;
        let mut closed = false;
        while let Some(next) = partner[exit] {
            let (from, to) = (point(exit), point(next));
            if next / 2 == first && next == begin {
                if from.distance(to) > 1e-9 {
                    path.push(PathSeg::Line(Line::new(from, to)));
                }
                closed = true;
                break;
            }
            if used[next / 2] {
                break;
            }
            if from.distance(to) > 1e-9 {
                path.push(PathSeg::Line(Line::new(from, to)));
            }
            path.extend(oriented(next / 2, next));
            used[next / 2] = true;
            exit = next ^ 1;
        }
        out.push((path, closed));
    }
    out
}

/// "SKIRT FRONT" → "Skirt front", "BELT LOOPS (5pc)" → "Belt loops (5pc)":
/// words in capitals, perhaps with a note in small letters. Other text
/// ("Back yoke", codes such as "x1-BL-S1M") as it is.
pub(crate) fn sentence_case(s: &str) -> String {
    let s = s.trim();
    let count = |w: &str, f: fn(char) -> bool| w.chars().filter(|&c| f(c)).count();
    let mixed = |w: &str| count(w, char::is_uppercase) > 0 && count(w, char::is_lowercase) > 0;
    if s.split_whitespace().any(mixed)
        || count(s, char::is_uppercase) <= 2 * count(s, char::is_lowercase)
    {
        return s.to_string();
    }
    let lower = s.to_lowercase();
    let mut chars = lower.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// A closed shape big enough to be a piece.
struct Candidate {
    raw: usize,
    polyline: Vec<Point>,
    area: f64,
    parent: Option<usize>,
}

/// `nested`: one size of a pattern whose sizes are drawn nested, its lines
/// drawn once across them all.
pub(crate) fn assemble(
    mut raw: Vec<RawPath>,
    notches: &[RawNotch],
    nested: bool,
    format: SourceFormat,
    scale: &ScaleInfo,
    file_name: Option<&str>,
    sha256: &str,
) -> (Vec<Piece>, Vec<ImportNote>) {
    let closings = close_open_outlines(&mut raw);
    let mut notes = Vec::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut small: Vec<usize> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    // Logos (FreeSewing signs a piece with one) are left out like other
    // decorations. One logo can be many paths with the same hint.
    let mut logos: Vec<&str> = Vec::new();
    for (i, r) in raw.iter().enumerate() {
        if r.hint.split_whitespace().any(|w| w == "logo") {
            if !logos.contains(&r.hint.as_str()) {
                logos.push(&r.hint);
            }
            continue;
        }
        if !r.closed {
            open.push(i);
            continue;
        }
        let polyline = r.polyline();
        let area = polygon_area(&polyline).abs();
        let b = bounds(&polyline);
        if area >= MIN_PIECE_AREA && b.width().hypot(b.height()) >= MIN_PIECE_SPAN {
            candidates.push(Candidate {
                raw: i,
                polyline,
                area,
                parent: None,
            });
        } else {
            small.push(i);
        }
    }

    // Each candidate's parent is the smallest larger candidate around it.
    for i in 0..candidates.len() {
        let parent = (0..candidates.len())
            .filter(|&j| j != i && candidates[j].area > candidates[i].area)
            .filter(|&j| inside(&candidates[i].polyline, &candidates[j].polyline))
            .min_by(|&a, &b| candidates[a].area.total_cmp(&candidates[b].area));
        candidates[i].parent = parent;
    }
    let top = |mut i: usize| {
        while let Some(p) = candidates[i].parent {
            i = p;
        }
        i
    };
    // A box drawn round a picture of the garment, a layout and a key to the
    // lines (a pattern's title block): three or more drawings much smaller
    // than it inside it. It is no piece, nor is anything in it.
    let keys: Vec<usize> = (0..candidates.len())
        .filter(|&c| candidates[c].parent.is_none() && is_box(&raw[candidates[c].raw].segs))
        .filter(|&c| {
            (0..candidates.len())
                .filter(|&k| {
                    k != c && top(k) == c && candidates[k].area < KEY_PART * candidates[c].area
                })
                .count()
                >= KEY_DRAWINGS
        })
        .collect();
    if !keys.is_empty() {
        notes.push(ImportNote::KeysIgnored {
            count: u32::try_from(keys.len()).unwrap_or(u32::MAX),
        });
    }
    // Tables, and everything in their boxes.
    let boxes: Vec<usize> = (0..candidates.len())
        .filter(|&c| candidates[c].parent.is_none() && is_box(&raw[candidates[c].raw].segs))
        .collect();
    let found = tables(
        &boxes
            .iter()
            .map(|&c| bounds(&candidates[c].polyline))
            .collect::<Vec<_>>(),
    );
    if !found.is_empty() {
        notes.push(ImportNote::TablesIgnored {
            count: u32::try_from(found.len()).unwrap_or(u32::MAX),
        });
    }
    let keys: Vec<usize> = keys
        .into_iter()
        .chain(found.iter().flatten().map(|&k| boxes[k]))
        .collect();

    let mut ids = counter("");
    let mut pieces: Vec<Piece> = Vec::new();
    // Top-level candidate index → (piece index, polygon that bounds the cut piece).
    let mut piece_of: HashMap<usize, (usize, Vec<Point>)> = HashMap::new();
    let source = |r: &RawPath| Source {
        format,
        file: file_name.map(str::to_string),
        sha256: Some(sha256.to_string()),
        reference: r.reference.clone(),
        scale: Some(SourceScale {
            mm_per_unit: scale.chosen.mm_per_unit,
            basis: scale.chosen.basis,
            confirmed: scale.certain,
        }),
        words: Vec::new(),
    };

    // Open paths that became stitching lines.
    let mut used_open: Vec<usize> = Vec::new();
    for c in (0..candidates.len()).filter(|&c| candidates[c].parent.is_none() && !keys.contains(&c))
    {
        let outer = &raw[candidates[c].raw];
        let children: Vec<usize> = (0..candidates.len())
            .filter(|&k| candidates[k].parent == Some(c))
            .collect();
        let name = outer
            .name
            .clone()
            .unwrap_or_else(|| format!("Piece {}", pieces.len() + 1));

        // Try each direct child, largest first, as this piece's stitching line.
        // Every measurement along each edge agreeing first, for each child;
        // then, failing that, most of them.
        let mut paired = None;
        let mut by_size = children.clone();
        by_size.sort_by(|&a, &b| candidates[b].area.total_cmp(&candidates[a].area));
        'agreeing: for agreeing in [Agreeing::All, Agreeing::Most] {
            for &k in &by_size {
                let Some(outline) =
                    outline_from_path(&raw[candidates[k].raw].segs, DEFAULT_CORNER_ANGLE, &mut ids)
                else {
                    continue;
                };
                let mut piece = Piece::new(ids("p"), name.clone(), outline);
                normalize_orientation(&mut piece);
                let cutting = &candidates[c].polyline;
                // An allowance that changes partway along an edge: the edge
                // is broken where it changes, each part with its own
                // allowance.
                let mut stepped = || {
                    let steps = allowance_steps(&piece, cutting)?;
                    let outline = break_edges(&piece, &steps, &mut ids)?;
                    let mut piece = Piece::new(ids("p"), name.clone(), outline);
                    normalize_orientation(&mut piece);
                    let widths = measure_allowances_avoiding(&piece, cutting, &steps, agreeing)?;
                    Some((piece, widths))
                };
                let found = match agreeing {
                    Agreeing::All => measure_allowances(&piece, cutting)
                        .map(|widths| (piece.clone(), widths))
                        .or_else(stepped),
                    // Most agreeing, an edge whose allowance changes along
                    // it would pass as one allowance: steps first.
                    Agreeing::Most => stepped().or_else(|| {
                        measure_allowances_avoiding(&piece, cutting, &[], agreeing)
                            .map(|widths| (piece.clone(), widths))
                    }),
                };
                if let Some((piece, widths)) = found {
                    paired = Some((candidates[k].raw, Some(k), piece, widths));
                    break 'agreeing;
                }
            }
        }
        // A stitching line drawn only where there is an allowance: open, its
        // ends on the cutting line, which is the stitching line along the
        // rest (an edge cut without allowance).
        if paired.is_none() {
            let cutting = &candidates[c].polyline;
            let perimeter = polyline_length(cutting, true);
            let mut ends_on: Vec<(f64, usize)> = open
                .iter()
                .filter(|i| !used_open.contains(i))
                .filter_map(|&i| {
                    let pts = raw[i].polyline();
                    let (first, last) = (*pts.first()?, *pts.last()?);
                    let length = polyline_length(&pts, false);
                    let touches = |p: Point| distance_to_polyline(p, cutting, true) <= 0.5;
                    (length >= 0.25 * perimeter
                        && touches(first)
                        && touches(last)
                        && point_in_polygon(pts[pts.len() / 2], cutting))
                    .then_some((length, i))
                })
                .collect();
            ends_on.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
            for (length, i) in ends_on {
                let Some((ring, along_cut)) = close_along(&raw[i].segs, &outer.segs) else {
                    continue;
                };
                // Mostly drawn: a line across the piece (a fold line down a
                // strip) is not a stitching line.
                if along_cut > length {
                    continue;
                }
                let Some(outline) = outline_from_path(&ring, DEFAULT_CORNER_ANGLE, &mut ids) else {
                    continue;
                };
                let mut piece = Piece::new(ids("p"), name.clone(), outline);
                normalize_orientation(&mut piece);
                if let Some(widths) = measure_allowances(&piece, cutting)
                    && widths.iter().any(|w| *w > 0.0)
                {
                    used_open.push(i);
                    paired = Some((i, None, piece, widths));
                    break;
                }
            }
        }

        let piece = match paired {
            Some((stitched, child, mut piece, widths)) => {
                let seam = median(widths.clone());
                piece.allowance.seam = seam;
                for (edge, w) in piece.outline.edges.iter_mut().zip(&widths) {
                    if (w - seam).abs() > 0.25 {
                        edge.allowance = Some(*w);
                    }
                }
                // The edges along a fold the cutting line was left open on.
                if let Some(Closing::Fold(a, b)) = closings[candidates[c].raw] {
                    mark_folds(&mut piece, a, b);
                }
                if let Some(cut) = outer.cut.clone().or_else(|| raw[stitched].cut.clone()) {
                    piece.cut = vec![cut];
                }
                piece.source = Some(source(outer));
                notes.push(ImportNote::SeamLineFound {
                    piece: piece.id.clone(),
                    allowance: seam,
                });
                let computed = allowance_lines(&piece).cutting;
                let max_mm = match closings[candidates[c].raw] {
                    // Only the part the file drew: the bridge across the gap
                    // is ours.
                    Some(Closing::Gap) => {
                        let drawn = flatten_segs(&outer.segs[..outer.segs.len() - 1], 0.05);
                        sample(&drawn, false, 2.0)
                            .iter()
                            .map(|p| distance_to_polyline(*p, &computed, true))
                            .fold(0.0, f64::max)
                    }
                    _ => hausdorff(&computed, &candidates[c].polyline),
                };
                if max_mm > 0.5 {
                    notes.push(ImportNote::CuttingLineDiffers {
                        piece: piece.id.clone(),
                        max_mm: (max_mm * 10.0).round() / 10.0,
                    });
                }
                // Other children are internal shapes.
                for &other in children.iter().filter(|&&o| Some(o) != child) {
                    piece
                        .lines
                        .push(closed_line(&raw[candidates[other].raw], &mut ids));
                }
                piece
            }
            None => {
                let Some(outline) = outline_from_path(&outer.segs, DEFAULT_CORNER_ANGLE, &mut ids)
                else {
                    continue;
                };
                let mut piece = Piece::new(ids("p"), name, outline);
                normalize_orientation(&mut piece);
                piece.source = Some(source(outer));
                if let Some(cut) = outer.cut.clone() {
                    piece.cut = vec![cut];
                }
                for &other in &children {
                    piece
                        .lines
                        .push(closed_line(&raw[candidates[other].raw], &mut ids));
                }
                piece
            }
        };
        if self_intersects(&flatten_segs(&piece.outline.path_segs(), 0.05)) {
            notes.push(ImportNote::OutlineCrossesItself {
                piece: piece.id.clone(),
            });
        }
        piece_of.insert(c, (pieces.len(), candidates[c].polyline.clone()));
        pieces.push(piece);
    }

    // Shapes nested deeper than direct children belong to their top piece.
    for c in 0..candidates.len() {
        if let Some(p) = candidates[c].parent
            && candidates[p].parent.is_some()
            && let Some((pi, _)) = piece_of.get(&top(c))
        {
            let line = closed_line(&raw[candidates[c].raw], &mut ids);
            pieces[*pi].lines.push(line);
        }
    }

    // Pieces can overlap (a layout drawn across them): a point belongs to the
    // smallest piece around it, the same one every time.
    let mut regions: Vec<(usize, &Vec<Point>, f64)> = piece_of
        .values()
        .map(|(pi, poly)| (*pi, poly, polygon_area(poly).abs()))
        .collect();
    regions.sort_by(|a, b| a.2.total_cmp(&b.2).then(a.0.cmp(&b.0)));
    let find_piece = |pt: Point| {
        regions
            .iter()
            .find(|(_, poly, _)| point_in_polygon(pt, poly))
            .map(|(pi, _, _)| *pi)
    };
    // The piece a line is on: the one round its middle point, or else the
    // one well round the point halfway along it (a straight line's middle
    // point is its end, which can lie past the piece: a line drawn once
    // across a nest of sizes, out to the largest), or else, in a nest, the
    // one most of it is across (such a line, on the smallest sizes).
    let piece_of_line = |pts: &[Point]| {
        let mid = pts.get(pts.len() / 2).copied()?;
        find_piece(mid)
            .or_else(|| {
                let half = halfway(pts)?;
                regions
                    .iter()
                    .find(|(_, poly, _)| {
                        point_in_polygon(half, poly)
                            && distance_to_polyline(half, poly, true) >= LINE_WELL_INSIDE
                    })
                    .map(|(pi, _, _)| *pi)
            })
            .or_else(|| {
                if !nested {
                    return None;
                }
                let along = sample(pts, false, 1.0);
                let across = |poly: &Vec<Point>| {
                    along.iter().filter(|&&p| point_in_polygon(p, poly)).count()
                };
                let (pi, most) = regions
                    .iter()
                    .map(|(pi, poly, _)| (*pi, across(poly)))
                    .max_by_key(|&(pi, n)| (n, std::cmp::Reverse(pi)))?;
                (most as f64 >= LINE_MOSTLY_ACROSS.max(0.25 * along.len() as f64)).then_some(pi)
            })
    };

    // Each piece's outline, and the cutting line the file drew around it.
    let outlines: Vec<(Vec<Point>, Rect)> = pieces
        .iter()
        .map(|p| flatten_segs(&p.outline.path_segs(), 0.05))
        .chain(regions.iter().map(|(_, poly, _)| (*poly).clone()))
        .map(|poly| {
            let b = bounds(&poly).inflate(ON_OUTLINE, ON_OUTLINE);
            (poly, b)
        })
        .collect();
    let on_outline = |pts: &[Point]| {
        let b = bounds(pts);
        outlines.iter().any(|(poly, around)| {
            around.contains(b.origin())
                && around.contains(Point::new(b.x1, b.y1))
                && sample(pts, false, 2.0)
                    .iter()
                    .all(|p| distance_to_polyline(*p, poly, true) <= ON_OUTLINE)
        })
    };

    // A piece's grainline is the longest line marked as one: shorter arrows
    // show which way to press a dart or a pleat.
    let mut longest_grain: HashMap<usize, (f64, usize)> = HashMap::new();
    for &i in open.iter().filter(|i| !used_open.contains(i)) {
        let r = &raw[i];
        if !r.hint.contains("grain") {
            continue;
        }
        let pts = flatten_segs(&r.segs, 0.05);
        if let Some((from, to)) = grain_segment(&r.segs)
            && let Some(pi) = piece_of_line(&pts)
        {
            let length = from.distance(to);
            let best = longest_grain.entry(pi).or_insert((length, i));
            if length > best.0 {
                *best = (length, i);
            }
        }
    }
    let (mut loose, mut copies) = (0u32, 0u32);
    for &i in open.iter().filter(|i| !used_open.contains(i)) {
        let r = &raw[i];
        let pts = flatten_segs(&r.segs, 0.05);
        if pts.is_empty() {
            continue;
        }
        // A hem drawn along a piece's outline (as Drapewright writes a
        // FreeSewing design it drafts): the edges under it are hems.
        if r.hint.split_whitespace().any(|w| w == "hem")
            && let Some(pi) = outlines[..pieces.len()].iter().position(|(poly, around)| {
                let b = bounds(&pts);
                around.contains(b.origin())
                    && around.contains(Point::new(b.x1, b.y1))
                    && sample(&pts, false, 2.0)
                        .iter()
                        .all(|p| distance_to_polyline(*p, poly, true) <= ON_OUTLINE)
            })
        {
            mark_hems(&mut pieces[pi], &pts);
            continue;
        }
        // A copy of part of an outline (FreeSewing draws some seams twice).
        if !r.hint.contains("grain") && !r.hint.contains("cutonfold") && on_outline(&pts) {
            copies += 1;
            continue;
        }
        let Some(pi) = piece_of_line(&pts) else {
            loose += 1;
            continue;
        };
        let piece = &mut pieces[pi];
        if r.hint.contains("cutonfold") {
            // FreeSewing's cut-on-fold bracket: its ends point at the fold.
            mark_folds(piece, pts[0], pts[pts.len() - 1]);
        }
        let grain = (r.hint.contains("grain")
            && piece.grainline.is_none()
            && longest_grain.get(&pi).is_none_or(|&(_, best)| best == i))
        .then(|| grain_segment(&r.segs))
        .flatten();
        if let Some((from, to)) = grain {
            piece.grainline = Some(Grainline {
                from: Pt::from(from),
                to: Pt::from(to),
                one_way: false,
            });
        } else if r.hint.contains("cutonfold") {
            // An arrow pointing at the fold, not a line on the piece.
        } else {
            piece.lines.push(InternalLine {
                id: ids("l"),
                kind: LineKind::Line,
                start: Pt::from(r.segs[0].start()),
                segments: to_segments(&r.segs),
                label: None,
            });
        }
    }

    let mut ignored = u32::try_from(logos.len()).unwrap_or(u32::MAX);
    for &i in &small {
        let r = &raw[i];
        if r.hint.contains("grain") {
            // Arrowheads of a grainline.
            continue;
        }
        let pts = r.polyline();
        let b = bounds(&pts);
        let round = (b.width() - b.height()).abs() <= 0.1 * b.width().max(b.height())
            && (0.5..=MAX_MARK_SIZE).contains(&b.width())
            && (polygon_area(&pts).abs() - std::f64::consts::PI * b.width() * b.height() / 4.0)
                .abs()
                <= 0.1 * b.width() * b.height();
        match (round, find_piece(b.center())) {
            (true, Some(pi)) => {
                let piece = &mut pieces[pi];
                piece.marks.push(Mark {
                    id: ids("m"),
                    kind: if r.filled {
                        MarkKind::Dot
                    } else {
                        MarkKind::Circle
                    },
                    at: Pt::from(b.center()),
                    size: Some((b.width() + b.height()) / 2.0),
                    length: None,
                    angle: 0.0,
                    label: None,
                });
            }
            _ => ignored += 1,
        }
    }

    // Notches drawn as symbols go on the outline they sit on. One cut into
    // a piece's cutting line goes on its outline straight in from there
    // (on the stitching line, when the piece has one).
    for n in notches {
        let cut_into = regions
            .iter()
            .find(|(_, poly, _)| distance_to_polyline(n.at, poly, true) <= 0.5)
            .map(|(pi, _, _)| *pi);
        if let Some(pi) = cut_into
            && let Some(notch) = notch_at(&pieces[pi], n.at, CUT_NOTCH_REACH, n.kind, ids("n"))
        {
            pieces[pi].notches.push(notch);
            continue;
        }
        let nearest = |p: &Piece| {
            (0..p.outline.edges.len())
                .filter_map(|i| p.outline.edge_geom(i).nearest(n.at))
                .map(|near| near.distance)
                .min_by(f64::total_cmp)
        };
        let best = pieces
            .iter()
            .enumerate()
            .filter_map(|(pi, p)| nearest(p).map(|d| (pi, d)))
            .filter(|(_, d)| *d <= NOTCH_REACH)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        let placed = best.and_then(|(pi, _)| {
            notch_at(&pieces[pi], n.at, NOTCH_REACH, n.kind, ids("n")).map(|notch| (pi, notch))
        });
        match placed {
            Some((pi, notch)) => pieces[pi].notches.push(notch),
            None => ignored += 1,
        }
    }

    if ignored > 0 {
        notes.push(ImportNote::SmallShapesIgnored { count: ignored });
    }
    if loose > 0 {
        notes.push(ImportNote::LooseLinesIgnored { count: loose });
    }
    if copies > 0 {
        notes.push(ImportNote::LinesOnOutlineIgnored { count: copies });
    }
    if pieces.is_empty() && !raw.is_empty() {
        notes.push(ImportNote::NoClosedShapes);
    }
    (pieces, notes)
}

fn closed_line(r: &RawPath, ids: &mut impl FnMut(&str) -> String) -> InternalLine {
    let start = r.segs.first().map_or(Point::ZERO, ParamCurve::start);
    InternalLine {
        id: ids("l"),
        kind: LineKind::Line,
        start: Pt::from(start),
        segments: to_segments(&r.segs),
        label: None,
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // Snapped allowances are exact constants.
mod tests {
    use super::*;

    #[test]
    fn names_from_ids_and_labels() {
        assert_eq!(meaningful_name("front-bodice"), Some("Front bodice".into()));
        assert_eq!(meaningful_name("path1234"), None);
        assert_eq!(meaningful_name("Layer 1"), None);
        assert_eq!(meaningful_name("XMLID_12_"), None);
        assert_eq!(meaningful_name("Selection34"), None);
        assert_eq!(meaningful_name("back_yoke"), Some("Back yoke".into()));
        assert_eq!(meaningful_name("facing-cut"), Some("Facing".into()));
        assert_eq!(meaningful_name("Front seam line"), Some("Front".into()));
        assert_eq!(meaningful_name("Seam"), Some("Seam".into()));
        assert_eq!(
            meaningful_name("Skirt back (cutting line)"),
            Some("Skirt back".into())
        );
    }

    #[test]
    fn chains_lines_into_a_closed_outline() {
        use kurbo::Line;
        let seg = |a: (f64, f64), b: (f64, f64)| vec![PathSeg::Line(Line::new(a, b))];
        // A square drawn as four lines, one of them backwards, plus a stray.
        let paths = vec![
            seg((0.0, 0.0), (10.0, 0.0)),
            seg((10.0, 10.0), (10.0, 0.0)),
            seg((10.0, 10.0), (0.0, 10.0)),
            seg((0.0, 10.0), (0.0, 0.0)),
            seg((50.0, 50.0), (60.0, 60.0)),
        ];
        let chained = chain(paths, 0.01);
        assert_eq!(chained.len(), 2);
        assert!(chained[0].1);
        assert_eq!(chained[0].0.len(), 4);
        assert!(!chained[1].1);
    }

    fn raw(segs: Vec<PathSeg>, closed: bool) -> RawPath {
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

    fn lines(points: &[(f64, f64)]) -> Vec<PathSeg> {
        points
            .windows(2)
            .map(|w| PathSeg::Line(kurbo::Line::new(w[0], w[1])))
            .collect()
    }

    #[test]
    fn cutting_lines_left_open_are_closed() {
        // A stitching line 100 × 200 with its fold at x = 0.
        let stitch = || {
            raw(
                lines(&[
                    (0.0, 0.0),
                    (100.0, 0.0),
                    (100.0, 200.0),
                    (0.0, 200.0),
                    (0.0, 0.0),
                ]),
                true,
            )
        };
        // Open along the fold: both ends on the line x = 0, beyond the
        // stitching line's corners (the hem allowance is deeper).
        let fold = raw(
            lines(&[(0.0, -10.0), (110.0, -10.0), (110.0, 230.0), (0.0, 230.0)]),
            false,
        );
        let mut paths = vec![stitch(), fold];
        let closings = close_open_outlines(&mut paths);
        assert!(paths[1].closed);
        assert!(
            matches!(closings[1], Some(Closing::Fold(a, b)) if a.x.abs() < 1e-9 && b.x.abs() < 1e-9)
        );

        // Open at a corner: the ends 14 mm apart.
        let gap = raw(
            lines(&[
                (-10.0, 0.0),
                (-10.0, -10.0),
                (110.0, -10.0),
                (110.0, 210.0),
                (-10.0, 210.0),
                (-10.0, 14.0),
            ]),
            false,
        );
        let mut paths = vec![stitch(), gap];
        let closings = close_open_outlines(&mut paths);
        assert!(paths[1].closed);
        assert!(matches!(closings[1], Some(Closing::Gap)));

        // An open line that goes around nothing stays open.
        let stray = raw(lines(&[(300.0, 0.0), (400.0, 0.0), (400.0, 100.0)]), false);
        let mut paths = vec![stitch(), stray];
        assert!(close_open_outlines(&mut paths)[1].is_none());
        assert!(!paths[1].closed);
    }

    #[test]
    fn copies_of_outlines_are_left_out_and_shared_lines_go_to_the_smaller_piece() {
        let scale = ScaleInfo {
            chosen: crate::import::ScaleOption {
                mm_per_unit: 1.0,
                basis: drapewright_pattern_core::ScaleBasis::DeclaredUnits,
            },
            certain: true,
            reason: crate::import::ScaleReason::DeclaredUnits,
            alternatives: Vec::new(),
        };
        let square = |x0: f64, y0: f64, x1: f64, y1: f64| {
            raw(
                lines(&[(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)]),
                true,
            )
        };
        let paths = vec![
            // Two pieces that overlap where 150 < x < 200.
            square(0.0, 0.0, 200.0, 200.0),
            square(150.0, 50.0, 300.0, 150.0),
            // Drawn over the big piece's left edge: a copy.
            raw(lines(&[(0.0, 20.0), (0.0, 180.0)]), false),
            // Where they overlap.
            raw(lines(&[(160.0, 90.0), (190.0, 110.0)]), false),
            // Inside the big piece only.
            raw(lines(&[(20.0, 20.0), (80.0, 80.0)]), false),
        ];
        let (pieces, notes) = assemble(paths, &[], false, SourceFormat::Svg, &scale, None, "");
        assert_eq!(pieces.len(), 2);
        assert_eq!((pieces[0].lines.len(), pieces[1].lines.len()), (1, 1));
        assert!(notes.contains(&ImportNote::LinesOnOutlineIgnored { count: 1 }));
    }

    #[test]
    fn allowances_snap_to_common_values() {
        assert_eq!(snap_allowance(15.08), 15.0);
        assert_eq!(snap_allowance(15.9), 15.875);
        assert_eq!(snap_allowance(11.23), 11.2);
    }
}
