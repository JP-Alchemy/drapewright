//! Reading pattern pieces from SVG.
//!
//! Every closed shape big enough to be a piece becomes one. A closed shape
//! running parallel inside another is taken as that piece's stitching line
//! (the outer one being its cutting line), and the allowance between them is
//! measured edge by edge. Other shapes and lines inside a piece become its
//! internal lines and marks; a straight line whose id or label mentions
//! "grain" becomes its grainline. Everything else is counted and reported,
//! never silently dropped.

use std::collections::HashMap;

use kurbo::{CubicBez, Line, ParamCurve, PathSeg, Point, QuadBez, Rect};
use soom_pattern_core::allowance::allowance_lines;
use soom_pattern_core::build::{DEFAULT_CORNER_ANGLE, counter, outline_from_path};
use soom_pattern_core::geom::{distance_to_polyline, flatten_segs, point_in_polygon, polygon_area};
use soom_pattern_core::ops::normalize_orientation;
use soom_pattern_core::validate::self_intersects;
use soom_pattern_core::{
    EdgeGeom, Grainline, InternalLine, LineKind, Mark, MarkKind, Piece, Pt, Segment, Source,
    SourceFormat, SourceScale,
};
use usvg::tiny_skia_path::PathSegment;

use super::units::{detect_scale, view_box};
use crate::import::{ImportError, ImportNote, ImportResult, ScaleInfo, round_mm, sha256_hex};

/// Closed shapes smaller than this are marks or decorations, not pieces.
const MIN_PIECE_AREA: f64 = 400.0; // mm²
const MIN_PIECE_SPAN: f64 = 30.0; // mm, bounding-box diagonal

/// An open path whose ends are this close counts as closed, mm.
const CLOSE_TOLERANCE: f64 = 0.05;

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
struct RawPath {
    segs: Vec<PathSeg>,
    closed: bool,
    filled: bool,
    /// Element id, label and class, lower-cased, for keyword hints.
    hint: String,
    name: Option<String>,
    reference: Option<String>,
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

/// Labels and classes from the XML, which usvg does not keep.
#[derive(Default)]
struct ElementInfo {
    label: Option<String>,
    class: Option<String>,
    group_label: Option<String>,
}

fn element_infos(doc: &roxmltree::Document) -> HashMap<String, ElementInfo> {
    const INKSCAPE: &str = "http://www.inkscape.org/namespaces/inkscape";
    let label_of = |n: roxmltree::Node| {
        n.attribute((INKSCAPE, "label"))
            .map(str::to_string)
            .or_else(|| {
                n.children()
                    .find(|c| c.has_tag_name("title"))
                    .and_then(|t| t.text())
                    .map(|t| t.trim().to_string())
            })
    };
    let mut infos = HashMap::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        let Some(id) = node.attribute("id") else {
            continue;
        };
        let group_label = node
            .ancestors()
            .skip(1)
            .filter(|a| a.has_tag_name("g"))
            .find_map(|g| label_of(g).or_else(|| g.attribute("id").map(str::to_string)));
        infos.insert(
            id.to_string(),
            ElementInfo {
                label: label_of(node),
                class: node.attribute("class").map(str::to_string),
                group_label,
            },
        );
    }
    infos
}

/// A readable piece name from an id or label, or `None` for the generated
/// ones editors write ("path1234", "Layer 1", "XMLID_12_").
fn meaningful_name(raw: &str) -> Option<String> {
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

/// Maps usvg's canvas pixels to millimetres in the file's own coordinates.
#[derive(Clone, Copy)]
struct ToMm {
    mm_per_unit: f64,
    /// usvg moves the viewBox origin to (0, 0); this puts it back, in mm.
    origin: (f64, f64),
}

fn collect(
    group: &usvg::Group,
    to_mm: ToMm,
    infos: &HashMap<String, ElementInfo>,
    group_name: Option<&str>,
    out: &mut Vec<RawPath>,
) {
    let ToMm {
        mm_per_unit,
        origin,
    } = to_mm;
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => {
                let own = infos
                    .get(g.id())
                    .and_then(|i| i.label.as_deref())
                    .and_then(meaningful_name)
                    .or_else(|| meaningful_name(g.id()));
                collect(g, to_mm, infos, own.as_deref().or(group_name), out);
            }
            usvg::Node::Path(p) if p.is_visible() => {
                let t = p.abs_transform();
                let (sx, kx, ky, sy, tx, ty) = (
                    f64::from(t.sx),
                    f64::from(t.kx),
                    f64::from(t.ky),
                    f64::from(t.sy),
                    f64::from(t.tx),
                    f64::from(t.ty),
                );
                let map = |q: usvg::tiny_skia_path::Point| {
                    let (x, y) = (f64::from(q.x), f64::from(q.y));
                    Point::new(
                        round_mm(mm_per_unit * (sx * x + kx * y + tx) + origin.0),
                        round_mm(mm_per_unit * (ky * x + sy * y + ty) + origin.1),
                    )
                };
                let info = infos.get(p.id());
                let label = info.and_then(|i| i.label.as_deref());
                let name = label
                    .and_then(meaningful_name)
                    .or_else(|| meaningful_name(p.id()))
                    .or_else(|| {
                        info.and_then(|i| i.group_label.as_deref())
                            .and_then(meaningful_name)
                    })
                    .or_else(|| group_name.map(str::to_string));
                let hint = [Some(p.id()), label, info.and_then(|i| i.class.as_deref())]
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
                    .join(" ")
                    .to_ascii_lowercase();
                let reference = (!p.id().is_empty()).then(|| p.id().to_string());
                let filled = p.fill().is_some();

                let mut segs: Vec<PathSeg> = Vec::new();
                let mut start: Option<Point> = None;
                let mut cur = Point::ZERO;
                let mut flush = |segs: &mut Vec<PathSeg>, start: Point, closed: bool| {
                    if segs.is_empty() {
                        return;
                    }
                    let end = segs.last().map_or(start, |s| s.end());
                    let closed = closed || end.distance(start) <= CLOSE_TOLERANCE;
                    if closed {
                        // Snap the end exactly onto the start.
                        if let Some(last) = segs.last_mut() {
                            *last = match *last {
                                PathSeg::Line(l) => PathSeg::Line(Line::new(l.p0, start)),
                                PathSeg::Quad(q) => PathSeg::Quad(QuadBez::new(q.p0, q.p1, start)),
                                PathSeg::Cubic(c) => {
                                    PathSeg::Cubic(CubicBez::new(c.p0, c.p1, c.p2, start))
                                }
                            };
                        }
                    }
                    out.push(RawPath {
                        segs: std::mem::take(segs),
                        closed,
                        filled,
                        hint: hint.clone(),
                        name: name.clone(),
                        reference: reference.clone(),
                    });
                };
                for seg in p.data().segments() {
                    match seg {
                        PathSegment::MoveTo(q) => {
                            if let Some(s) = start {
                                flush(&mut segs, s, false);
                            }
                            cur = map(q);
                            start = Some(cur);
                        }
                        PathSegment::LineTo(q) => {
                            let q = map(q);
                            if q != cur {
                                segs.push(PathSeg::Line(Line::new(cur, q)));
                            }
                            cur = q;
                        }
                        PathSegment::QuadTo(c, q) => {
                            let q = map(q);
                            segs.push(PathSeg::Cubic(QuadBez::new(cur, map(c), q).raise()));
                            cur = q;
                        }
                        PathSegment::CubicTo(c1, c2, q) => {
                            let q = map(q);
                            segs.push(PathSeg::Cubic(CubicBez::new(cur, map(c1), map(c2), q)));
                            cur = q;
                        }
                        PathSegment::Close => {
                            if let Some(s) = start {
                                if cur != s {
                                    segs.push(PathSeg::Line(Line::new(cur, s)));
                                }
                                flush(&mut segs, s, true);
                                cur = s;
                            }
                        }
                    }
                }
                if let Some(s) = start {
                    flush(&mut segs, s, false);
                }
            }
            _ => {}
        }
    }
}

fn bounds(points: &[Point]) -> Rect {
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

fn snap_allowance(w: f64) -> f64 {
    COMMON_ALLOWANCES
        .iter()
        .copied()
        .find(|c| (c - w).abs() <= 0.2)
        .unwrap_or_else(|| (w * 10.0).round() / 10.0)
}

fn median(mut v: Vec<f64>) -> f64 {
    v.sort_by(f64::total_cmp);
    v[v.len() / 2]
}

/// If `inner` runs parallel inside `outer`, the allowance of each of the
/// piece's edges (measured from the stitching line to the cutting line).
fn measure_allowances(piece: &Piece, outer: &[Point]) -> Option<Vec<f64>> {
    let mut widths = Vec::new();
    for i in 0..piece.outline.edges.len() {
        let geom: EdgeGeom = piece.outline.edge_geom(i);
        let len = geom.length();
        // Stay clear of the corners, where corner shapes change the distance.
        let samples: Vec<f64> = (1..10)
            .map(|k| distance_to_polyline(geom.point_at(len * f64::from(k) / 10.0), outer, true))
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
fn hausdorff(a: &[Point], b: &[Point]) -> f64 {
    let one_way = |from: &[Point], to: &[Point]| {
        sample(from, true, 2.0)
            .iter()
            .map(|p| distance_to_polyline(*p, to, true))
            .fold(0.0, f64::max)
    };
    one_way(a, b).max(one_way(b, a))
}

fn to_segments(segs: &[PathSeg]) -> Vec<Segment> {
    segs.iter().map(|s| Segment::from_path_seg(*s)).collect()
}

fn is_straight(points: &[Point]) -> bool {
    let (Some(&a), Some(&b)) = (points.first(), points.last()) else {
        return false;
    };
    let len = a.distance(b);
    len > 1.0
        && points
            .iter()
            .all(|p| ((*p - a).cross(b - a) / len).abs() < 0.1)
}

pub fn import_svg(data: &[u8], file_name: Option<&str>) -> Result<ImportResult, ImportError> {
    if data.starts_with(&[0x1f, 0x8b]) {
        return Err(ImportError::CompressedSvg);
    }
    let unreadable = |detail: String| ImportError::Unreadable {
        format: "SVG",
        detail,
    };
    let text = std::str::from_utf8(data).map_err(|e| unreadable(e.to_string()))?;
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let doc = roxmltree::Document::parse_with_options(text, options)
        .map_err(|e| unreadable(e.to_string()))?;
    if !doc.root_element().has_tag_name("svg") {
        return Err(unreadable(
            "the file does not start with an <svg> element".into(),
        ));
    }
    let scale = detect_scale(text, &doc);
    let infos = element_infos(&doc);
    let tree = usvg::Tree::from_data(data, &usvg::Options::default())
        .map_err(|e| unreadable(e.to_string()))?;
    let mm_per_unit = scale.chosen.mm_per_unit;
    let origin = view_box(&doc).map_or((0.0, 0.0), |(x, y, w, h)| {
        let size = tree.size();
        (
            x * f64::from(size.width()) / w * mm_per_unit,
            y * f64::from(size.height()) / h * mm_per_unit,
        )
    });
    let mut raw = Vec::new();
    collect(
        tree.root(),
        ToMm {
            mm_per_unit,
            origin,
        },
        &infos,
        None,
        &mut raw,
    );
    let sha256 = sha256_hex(data);
    let (pieces, notes) = assemble(raw, &scale, file_name, &sha256);
    Ok(ImportResult {
        pieces,
        scale,
        notes,
        sha256,
    })
}

/// A closed shape big enough to be a piece.
struct Candidate {
    raw: usize,
    polyline: Vec<Point>,
    area: f64,
    parent: Option<usize>,
}

fn assemble(
    raw: Vec<RawPath>,
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
        format: SourceFormat::Svg,
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
    fn allowances_snap_to_common_values() {
        assert_eq!(snap_allowance(15.08), 15.0);
        assert_eq!(snap_allowance(15.9), 15.875);
        assert_eq!(snap_allowance(11.23), 11.2);
    }
}
