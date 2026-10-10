//! Turning the paths of a drawing into pattern pieces, whatever the file
//! format. Every closed shape big enough to be a piece becomes one. A closed
//! shape running parallel inside another is taken as that piece's stitching
//! line (the outer one being its cutting line), and the allowance between
//! them is measured edge by edge. Other shapes and lines inside a piece
//! become its internal lines and marks; a straight line whose name mentions
//! "grain" becomes its grainline. Everything else is counted and reported,
//! never silently dropped.

use std::collections::HashMap;

use kurbo::{ParamCurve, PathSeg, Point, Rect};
use soom_pattern_core::allowance::allowance_lines;
use soom_pattern_core::build::{DEFAULT_CORNER_ANGLE, counter, outline_from_path};
use soom_pattern_core::geom::{
    distance_to_polyline, flatten_segs, point_in_polygon, polygon_area, ray_hit,
};
use soom_pattern_core::ops::normalize_orientation;
use soom_pattern_core::validate::self_intersects;
use soom_pattern_core::{
    EdgeGeom, Grainline, InternalLine, LineKind, Mark, MarkKind, Piece, Pt, Segment, Source,
    SourceFormat, SourceScale,
};

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
}

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
    raw: Vec<RawPath>,
    format: SourceFormat,
    scale: &ScaleInfo,
    file_name: Option<&str>,
    sha256: &str,
) -> (Vec<Piece>, Vec<ImportNote>) {
    let mut notes = Vec::new();
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut small: Vec<usize> = Vec::new();
    let mut open: Vec<usize> = Vec::new();
    for (i, r) in raw.iter().enumerate() {
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
                piece.source = Some(source(outer));
                notes.push(ImportNote::SeamLineFound {
                    piece: piece.id.clone(),
                    allowance: seam,
                });
                let computed = allowance_lines(&piece).cutting;
                let max_mm = hausdorff(&computed, &candidates[c].polyline);
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

    let find_piece = |pt: Point| {
        piece_of
            .values()
            .find(|(_, poly)| point_in_polygon(pt, poly))
            .map(|(pi, _)| *pi)
    };

    let mut loose = 0u32;
    for &i in &open {
        let r = &raw[i];
        let pts = flatten_segs(&r.segs, 0.05);
        let Some(mid) = pts.get(pts.len() / 2).copied() else {
            continue;
        };
        let Some(pi) = find_piece(mid) else {
            loose += 1;
            continue;
        };
        let piece = &mut pieces[pi];
        if r.hint.contains("grain") && piece.grainline.is_none() && is_straight(&pts) {
            piece.grainline = Some(Grainline {
                from: Pt::from(pts[0]),
                to: Pt::from(pts[pts.len() - 1]),
                one_way: false,
            });
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

    let mut ignored = 0u32;
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

    if ignored > 0 {
        notes.push(ImportNote::SmallShapesIgnored { count: ignored });
    }
    if loose > 0 {
        notes.push(ImportNote::LooseLinesIgnored { count: loose });
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

    #[test]
    fn allowances_snap_to_common_values() {
        assert_eq!(snap_allowance(15.08), 15.0);
        assert_eq!(snap_allowance(15.9), 15.875);
        assert_eq!(snap_allowance(11.23), 11.2);
    }
}
