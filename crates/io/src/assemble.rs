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
use drapewright_pattern_core::build::{DEFAULT_CORNER_ANGLE, counter, outline_from_path};
use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, point_in_polygon, polygon_area, ray_hit,
};
use drapewright_pattern_core::ops::normalize_orientation;
use drapewright_pattern_core::validate::self_intersects;
use drapewright_pattern_core::{
    CutInstruction, EdgeGeom, EdgeKind, Grainline, InternalLine, LineKind, Mark, MarkKind, Notch,
    NotchFrom, NotchKind, Piece, Pt, Segment, Source, SourceFormat, SourceScale,
};
use kurbo::{ParamCurve, PathSeg, Point, Rect};

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
        "path", "rect", "circle", "ellipse", "polygon", "polyline", "line", "g", "use", "layer",
        "svg", "text", "tspan", "group", "xmlid_", "calque", "ebene", "capa",
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

/// If the piece's outline (a stitching line) runs parallel inside `outer`
/// (its cutting line), the allowance of each edge.
///
/// Each edge is measured straight out from the stitching line, as a
/// seamstress would with a ruler. The nearest point of the cutting line
/// would do for even allowances, but not beside a narrower one: near the
/// corner where a 40 mm hem meets a fold, the fold's cutting line is closer
/// than the hem's.
pub(crate) fn measure_allowances(piece: &Piece, outer: &[Point]) -> Option<Vec<f64>> {
    let mut widths = Vec::new();
    for i in 0..piece.outline.edges.len() {
        let geom: EdgeGeom = piece.outline.edge_geom(i);
        let len = geom.length();
        // Stay clear of the corners, where corner shapes change the distance.
        let samples: Vec<f64> = (1..10)
            .map(|k| {
                let along = len * f64::from(k) / 10.0;
                let p = geom.point_at(along);
                ray_hit(p, geom.normal_at(along), outer).map_or_else(
                    || distance_to_polyline(p, outer, true),
                    |hit| hit.distance(p),
                )
            })
            .collect();
        let (lo, hi) = samples
            .iter()
            .fold((f64::MAX, f64::MIN), |(lo, hi), d| (lo.min(*d), hi.max(*d)));
        let mid = median(samples);
        if mid > MAX_DETECTED_ALLOWANCE || hi - lo > 0.5 + 0.05 * mid {
            return None;
        }
        widths.push(snap_allowance(mid));
    }
    Some(widths)
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

/// Marks the piece's straight edges that lie along the line through `a` and
/// `b` as folds.
fn mark_folds(piece: &mut Piece, a: Point, b: Point) {
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

/// Joins open paths that meet end to end (within `tolerance` mm) into
/// longer ones, reversing pieces as needed, and reports which came back to
/// their start. CAD drawings often build an outline from separate lines and
/// arcs.
pub(crate) fn chain(paths: Vec<Vec<PathSeg>>, tolerance: f64) -> Vec<(Vec<PathSeg>, bool)> {
    let close = |a: Point, b: Point| a.distance(b) <= tolerance;
    let mut pool: Vec<Option<Vec<PathSeg>>> = paths
        .into_iter()
        .filter(|p| !p.is_empty())
        .map(Some)
        .collect();
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
            let next = pool.iter().position(|q| {
                q.as_ref().is_some_and(|q| {
                    let (qs, qe) = (q[0].start(), q[q.len() - 1].end());
                    close(end, qs) || close(end, qe) || close(start, qe) || close(start, qs)
                })
            });
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
        let closed = close(path[0].start(), path[path.len() - 1].end());
        out.push((path, closed));
    }
    out
}

/// A closed shape big enough to be a piece.
struct Candidate {
    raw: usize,
    polyline: Vec<Point>,
    area: f64,
    parent: Option<usize>,
}

pub(crate) fn assemble(
    mut raw: Vec<RawPath>,
    notches: &[RawNotch],
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
    };

    for c in (0..candidates.len()).filter(|&c| candidates[c].parent.is_none()) {
        let outer = &raw[candidates[c].raw];
        let children: Vec<usize> = (0..candidates.len())
            .filter(|&k| candidates[k].parent == Some(c))
            .collect();
        let name = outer
            .name
            .clone()
            .unwrap_or_else(|| format!("Piece {}", pieces.len() + 1));

        // Try each direct child, largest first, as this piece's stitching line.
        let mut paired = None;
        let mut by_size = children.clone();
        by_size.sort_by(|&a, &b| candidates[b].area.total_cmp(&candidates[a].area));
        for &k in &by_size {
            let Some(outline) =
                outline_from_path(&raw[candidates[k].raw].segs, DEFAULT_CORNER_ANGLE, &mut ids)
            else {
                continue;
            };
            let mut piece = Piece::new(ids("p"), name.clone(), outline);
            normalize_orientation(&mut piece);
            if let Some(widths) = measure_allowances(&piece, &candidates[c].polyline) {
                paired = Some((k, piece, widths));
                break;
            }
        }

        let piece = match paired {
            Some((k, mut piece, widths)) => {
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
                if let Some(cut) = outer
                    .cut
                    .clone()
                    .or_else(|| raw[candidates[k].raw].cut.clone())
                {
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
                for &other in children.iter().filter(|&&o| o != k) {
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

    let (mut loose, mut copies) = (0u32, 0u32);
    for &i in &open {
        let r = &raw[i];
        let pts = flatten_segs(&r.segs, 0.05);
        let Some(mid) = pts.get(pts.len() / 2).copied() else {
            continue;
        };
        // A copy of part of an outline (FreeSewing draws some seams twice).
        if !r.hint.contains("grain") && !r.hint.contains("cutonfold") && on_outline(&pts) {
            copies += 1;
            continue;
        }
        let Some(pi) = find_piece(mid) else {
            loose += 1;
            continue;
        };
        let piece = &mut pieces[pi];
        if r.hint.contains("cutonfold") {
            // FreeSewing's cut-on-fold bracket: its ends point at the fold.
            mark_folds(piece, pts[0], pts[pts.len() - 1]);
        }
        let grain = (r.hint.contains("grain") && piece.grainline.is_none())
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
            && b.width() <= MAX_MARK_SIZE
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

    // Notches drawn as symbols go on the outline they sit on.
    for n in notches {
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
        let (pieces, notes) = assemble(paths, &[], SourceFormat::Svg, &scale, None, "");
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
