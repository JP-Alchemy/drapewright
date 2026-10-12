//! Sewing patterns from PDF files. A PDF measures in points (1/72 inch), so
//! its scale is exact. Each page's drawing is read with
//! `drapewright-pdf-import`, assembled into pieces like any other drawing,
//! and the words printed on the pattern name the pieces, say how many to
//! cut, and mark edges to place on the fold.

use std::collections::HashMap;

use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, label_point, point_in_polygon, polygon_area, segs_of,
    unit_tangent,
};
use drapewright_pattern_core::{
    CutInstruction, Mark, MarkKind, Piece, Pt, ScaleBasis, SourceFormat,
};
use drapewright_pdf_import::{Document, Page, Shape, TextRun};
use kurbo::{
    Affine, BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, PathEl, PathSeg, Point, Rect,
    Vec2,
};

use super::butt::{Edges, Stitch, butt_grid, drawn_frame};
use super::lettering::{
    find_crosses, find_lettering, find_outlined_crosses, find_repeated_decorations,
};
use super::loops::{find_loops, split_touching};
use super::marks::{arrowheads, find_fold_brackets, find_grainlines, find_notches, test_square};
use super::pagelabels;
use super::reading::{FAIR, ReadLine, SURE, read_block};
use super::sizes::find_sizes;
use super::tiles::{Print, arrange};
use crate::assemble::{
    CLOSE_TOLERANCE, RawPath, ascending, assemble, bounds, bridge_crowded, chain, chain_crowded,
    is_straight, mark_folds, meaningful_name, sentence_case, to_segments,
};
use crate::import::{
    AllowanceQuestion, ImportError, ImportNote, ImportResult, ImportSizes, PageLeftOut, PageUse,
    PieceSize, ScaleInfo, ScaleOption, ScaleReason, SizeSwatch, sha256_hex,
};

const MM_PER_POINT: f64 = 25.4 / 72.0;
/// Room left between pages that are not parts of one sheet, mm.
const PAGE_GAP: f64 = 100.0;
/// Segments drawn separately are joined when their ends are this close, mm.
const JOIN: f64 = 0.1;
/// Hairline gaps a plotter driver leaves between strokes, mm.
const BRIDGE: f64 = 0.5;
/// A dashed line stops a dash's gap short of its corner, where a solid
/// stub (half a dash) marks the corner: gaps up to this are bridged, mm.
const DASH_GAP: f64 = 2.0;
/// Such a stub is at most this long, mm.
const DASH_STUB: f64 = 1.5;
/// Line ends this close together meet at one corner, mm.
const CROWD: f64 = 1.0;
/// A line drawn twice on one page is two lines (sizes sharing a stretch)
/// when it is at least this long, mm; shorter, it is drawn over.
const SHARED_LONG: f64 = 10.0;
/// A line drawn in two that overlap or leave a gap, up to this, mm.
const STRAIGHT_ON: f64 = 1.2;
/// Each of them at least this long: no dash of a line drawn dash by dash.
const STRAIGHT_ON_LONG: f64 = 10.0;
/// A line closing an outline drawn in another style is at most this part
/// of the outline's own line.
const CLOSER_SHORTER: f64 = 0.6;
/// The sizes of a nest each end a hair apart at a corner they share: an
/// outline drawn partly in another style is closed across gaps up to
/// this, mm.
const ACROSS: f64 = 0.5;

/// A line of text printed on the pattern, in millimetres.
#[derive(Clone, Debug)]
pub(crate) struct Label {
    pub text: String,
    /// The middle of the line.
    pub at: Point,
    /// Letter size, mm.
    pub size: f64,
    /// Reading direction (unit).
    pub dir: Vec2,
    /// Whether, and how surely, it names its piece.
    pub naming: Naming,
}

/// How surely a label is its piece's name, least first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Naming {
    /// Lettering read from strokes, not set out as a name.
    Never,
    /// Text: any line may be the name, the biggest first.
    Maybe,
    /// Lettering in capitals, read closely.
    Capitals,
    /// Lettering above the piece's cutting.
    AboveCutting,
}

/// Maps a page's user space to millimetres from the top-left corner of the
/// page as it is shown (crop box, rotation and user unit applied), and the
/// page's shown size.
fn page_frame(page: &Page) -> (Affine, f64, f64) {
    let c = page.crop_box;
    let u = page.user_unit * MM_PER_POINT;
    let (w, h) = (c.width() * u, c.height() * u);
    match page.rotate {
        90 => (Affine::new([0.0, u, u, 0.0, -u * c.y0, -u * c.x0]), h, w),
        180 => (Affine::new([-u, 0.0, 0.0, u, u * c.x1, -u * c.y0]), w, h),
        270 => (Affine::new([0.0, -u, -u, 0.0, u * c.y1, u * c.x1]), h, w),
        _ => (Affine::new([u, 0.0, 0.0, -u, -u * c.x0, u * c.y1]), w, h),
    }
}

/// The subpaths of a painted path, in millimetres, each with whether it is
/// closed. Closed ones end exactly where they start.
fn subpaths(path: &BezPath, to_mm: Affine) -> Vec<(Vec<PathSeg>, bool)> {
    let mut out = Vec::new();
    let mut segs: Vec<PathSeg> = Vec::new();
    let mut start = Point::ZERO;
    let mut current = Point::ZERO;
    let mut flush = |segs: &mut Vec<PathSeg>, start: Point, closed: bool| {
        if segs.is_empty() {
            return;
        }
        let end = segs.last().map_or(start, |s| s.end());
        let closed = closed || end.distance(start) <= CLOSE_TOLERANCE;
        if closed && let Some(last) = segs.last_mut() {
            *last = match *last {
                PathSeg::Line(l) => PathSeg::Line(Line::new(l.p0, start)),
                PathSeg::Quad(q) => PathSeg::Cubic(CubicBez::new(q.p0, q.p1, q.p1, start)),
                PathSeg::Cubic(c) => PathSeg::Cubic(CubicBez::new(c.p0, c.p1, c.p2, start)),
            };
        }
        out.push((std::mem::take(segs), closed));
    };
    for el in path.elements() {
        match *el {
            PathEl::MoveTo(p) => {
                flush(&mut segs, start, false);
                start = to_mm * p;
                current = start;
            }
            PathEl::LineTo(p) => {
                let p = to_mm * p;
                if p != current {
                    segs.push(PathSeg::Line(Line::new(current, p)));
                }
                current = p;
            }
            PathEl::QuadTo(c, p) => {
                let (c, p) = (to_mm * c, to_mm * p);
                segs.push(PathSeg::Cubic(kurbo::QuadBez::new(current, c, p).raise()));
                current = p;
            }
            PathEl::CurveTo(c1, c2, p) => {
                let p = to_mm * p;
                segs.push(PathSeg::Cubic(CubicBez::new(
                    current,
                    to_mm * c1,
                    to_mm * c2,
                    p,
                )));
                current = p;
            }
            PathEl::ClosePath => {
                if current.distance(start) > 1e-9 {
                    segs.push(PathSeg::Line(Line::new(current, start)));
                }
                flush(&mut segs, start, true);
                current = start;
            }
        }
    }
    flush(&mut segs, start, false);
    out
}

/// How a shape is drawn, to keep lines of one kind together.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Style {
    pub stroke: Option<[u8; 3]>,
    /// Line width in twentieths of a millimetre.
    pub width: i64,
    pub dashed: bool,
    pub layers: Vec<u32>,
}

fn rgb8(c: [f32; 3]) -> [u8; 3] {
    c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A drawn rectangle along the edges of the page: a frame, not a piece.
fn is_page_frame(segs: &[PathSeg], page: Rect) -> bool {
    let pts = flatten_segs(segs, 0.1);
    let b = bounds(&pts);
    let axis_aligned = segs.iter().all(|s| match s {
        PathSeg::Line(l) => (l.p0.x - l.p1.x).abs() < 0.5 || (l.p0.y - l.p1.y).abs() < 0.5,
        _ => false,
    });
    // On the page: a rectangular piece across several pages of a tiled
    // print is no page's frame.
    let on_page = page.inflate(1.0, 1.0);
    axis_aligned
        && on_page.contains(b.origin())
        && on_page.contains(Point::new(b.x1, b.y1))
        && b.width() >= 0.8 * page.width()
        && b.height() >= 0.8 * page.height()
}

/// Pages put together on one sheet: each page, and where it goes.
type PageGroup = Vec<(usize, Vec2)>;

/// A path read from a page, how it is drawn, and whether any of it shows
/// on that page (tiles draw shapes beyond their edges too).
type PagePath = (RawPath, Style, bool);

/// Paths of one page, in millimetres, ready to assemble.
fn page_paths(
    shape: &Shape,
    to_mm: Affine,
    page: Rect,
    layer_names: &dyn Fn(u32) -> String,
) -> Vec<PagePath> {
    // What of the page the shape can show on: its clip, on the page.
    let visible = shape
        .clip
        .map_or(page, |c| to_mm.transform_rect_bbox(c).intersect(page));
    let shows = visible.width() > 0.0 && visible.height() > 0.0;
    // White fills on white paper are masks and backgrounds.
    if shape.stroke.is_none() && shape.fill.is_some_and(|f| f.iter().all(|v| *v > 0.95)) {
        return Vec::new();
    }
    let style = Style {
        stroke: shape.stroke.map(rgb8),
        width: (shape.line_width * MM_PER_POINT * 20.0).round() as i64,
        dashed: !shape.dash.is_empty(),
        layers: shape.layers.clone(),
    };
    let hint = shape
        .layers
        .iter()
        .map(|l| layer_names(*l).to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    subpaths(&shape.path, to_mm)
        .into_iter()
        .filter(|(segs, _)| !is_page_frame(segs, page))
        .map(|(segs, closed)| {
            let b = bounds(&flatten_segs(&segs, 1.0));
            let seen = shows
                && b.x1 >= visible.x0 - 1.0
                && b.x0 <= visible.x1 + 1.0
                && b.y1 >= visible.y0 - 1.0
                && b.y0 <= visible.y1 + 1.0;
            (
                RawPath {
                    segs,
                    closed,
                    filled: shape.fill.is_some(),
                    hint: hint.clone(),
                    name: None,
                    reference: None,
                    cut: None,
                },
                style.clone(),
                seen,
            )
        })
        .collect()
}

/// Lines of text: runs that continue one another on one baseline, joined.
pub(crate) fn text_lines(runs: &[TextRun], to_mm: Affine) -> Vec<Label> {
    let scale = to_mm.determinant().abs().sqrt();
    let mut lines: Vec<(String, Point, Point, f64)> = Vec::new();
    for run in runs {
        let (start, end, size) = (to_mm * run.start, to_mm * run.end, run.size * scale);
        if let Some((text, _, last_end, last_size)) = lines.last_mut() {
            let along = end - start;
            let unit = if along.hypot() > 1e-9 {
                along / along.hypot()
            } else {
                Vec2::new(1.0, 0.0)
            };
            let gap = (start - *last_end).dot(unit);
            let off = (start - *last_end).cross(unit).abs();
            let same_size = (size - *last_size).abs() <= 0.2 * size.max(*last_size);
            if same_size && off <= 0.3 * size && gap >= -0.5 * size && gap <= 1.5 * size {
                if gap > 0.25 * size && !text.ends_with(' ') && !run.text.starts_with(' ') {
                    text.push(' ');
                }
                text.push_str(&run.text);
                *last_end = end;
                continue;
            }
        }
        lines.push((run.text.clone(), start, end, size));
    }
    lines
        .into_iter()
        .filter_map(|(text, start, end, size)| {
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            if text.is_empty() {
                return None;
            }
            let along = end - start;
            let dir = if along.hypot() > 1e-9 {
                along / along.hypot()
            } else {
                Vec2::new(1.0, 0.0)
            };
            // Halfway along, a third of a letter above the baseline.
            let up = Vec2::new(dir.y, -dir.x);
            Some(Label {
                text,
                at: start.midpoint(end) + up * (size * 0.35),
                size,
                dir,
                naming: Naming::Maybe,
            })
        })
        .collect()
}

/// Words that say what to do with a piece, rather than what it is.
fn is_instruction(text: &str) -> bool {
    let t = text.to_lowercase();
    let starts = [
        "cut", "place", "fold", "grain", "lengthen", "shorten", "size", "sizes", "notch", "seam",
        "stitch", "pleat", "dart", "button", "line", "cf", "cb", "selvage", "selvedge", "bias",
        "x ", "page", "©", "www", "http", "print", "test", "this", "scale", "match", "join",
        "gather", "hem", "inch", "cm", "mm", "height",
    ];
    let words = t.split_whitespace().count();
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    letters < 3
        || words > 6
        || starts.iter().any(|s| t.starts_with(s))
        || t.contains("cut ")
        || t.contains("on fold")
        || t.contains("grainline")
        // The fabric's right side ("FACE!").
        || t.trim_end_matches('!') == "face"
        // Page labels such as "A1", "B12".
        || (t.len() <= 4 && t.chars().next().is_some_and(char::is_alphabetic) && t[1..].chars().all(|c| c.is_ascii_digit()))
}

/// Whether a label is a piece's number set out in big type: "- 4 -", "#12",
/// "No. 3", "- MP -".
fn is_piece_number(text: &str) -> bool {
    let t = text.trim();
    let t = t
        .strip_prefix("No.")
        .or_else(|| t.strip_prefix("Nr."))
        .unwrap_or(t);
    let core: String = t
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '–' | '—' | '#' | '.'))
        .collect();
    (1..=3).contains(&core.chars().count())
        && (core.chars().all(|c| c.is_ascii_digit())
            || core.chars().all(|c| c.is_ascii_uppercase()))
}

fn number_word(w: &str) -> Option<u32> {
    match w {
        "one" | "a" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "six" => Some(6),
        "eight" => Some(8),
        _ => w
            .trim_start_matches('x')
            .parse()
            .ok()
            .filter(|n| (1..=99).contains(n)),
    }
}

/// Cutting instructions printed on a piece: "Cut 2", "CUT 1 ON FOLD",
/// "Cut 1 pair lining", "Cut 2 main fabric, cut 2 interfacing".
pub(crate) fn cuts_from_label(text: &str) -> Vec<CutInstruction> {
    let lower = text.to_lowercase();
    let mut out = Vec::new();
    for part in lower.split("cut").skip(1) {
        let words: Vec<&str> = part
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .collect();
        let Some(mut quantity) = words.iter().take(2).find_map(|w| number_word(w)) else {
            continue;
        };
        let pair = words.iter().any(|w| w.starts_with("pair"));
        if pair {
            quantity *= 2;
        }
        let has = |w: &str| words.iter().any(|x| x.starts_with(w));
        let material = if has("lining") {
            "lining"
        } else if has("interfac") || has("fusible") {
            "interfacing"
        } else if has("contrast") {
            "contrast"
        } else {
            "main"
        };
        out.push(CutInstruction {
            material: material.to_string(),
            quantity,
            mirrored: (pair || has("mirror")).then_some(true),
            on_fold: has("fold").then_some(true),
        });
    }
    out
}

/// Names pieces from the largest words printed inside them, sets their
/// cutting from "Cut 2"-style labels, and marks edges labelled as folds.
pub(crate) fn read_labels(pieces: &mut [Piece], labels: &[Label]) {
    let polys: Vec<Vec<Point>> = pieces
        .iter()
        .map(|p| flatten_segs(&p.outline.path_segs(), 0.5))
        .collect();
    // Each label belongs to the piece it is inside, or else to the nearest
    // piece whose bounds hold it (names centred on a curved piece's box
    // can fall just outside it).
    let owner: Vec<Option<usize>> = labels
        .iter()
        .map(|l| {
            polys
                .iter()
                .position(|poly| point_in_polygon(l.at, poly))
                .or_else(|| {
                    polys
                        .iter()
                        .enumerate()
                        .filter(|(_, poly)| bounds(poly).contains(l.at))
                        .map(|(i, poly)| (i, distance_to_polyline(l.at, poly, true)))
                        .filter(|(_, d)| *d <= 40.0)
                        .min_by(|a, b| a.1.total_cmp(&b.1))
                        .map(|(i, _)| i)
                })
        })
        .collect();
    for (index, piece) in pieces.iter_mut().enumerate() {
        let inside: Vec<&Label> = labels
            .iter()
            .zip(&owner)
            .filter(|(_, o)| **o == Some(index))
            .map(|(l, _)| l)
            .collect();
        let named = |l: &&&Label| l.naming != Naming::Never && !is_instruction(&l.text);
        // A piece numbered in big type ("- 4 -", "#4") is named on the
        // line just under its number.
        let biggest = inside.iter().max_by(|a, b| a.size.total_cmp(&b.size));
        let under_number = biggest.filter(|n| is_piece_number(&n.text)).and_then(|n| {
            let down = Vec2::new(-n.dir.y, n.dir.x);
            inside
                .iter()
                .filter(named)
                .filter(|l| l.size < n.size)
                .filter_map(|l| {
                    let d = l.at - n.at;
                    let (below, across) = (d.dot(down), d.dot(n.dir).abs());
                    (below > 0.0 && below <= 2.5 * n.size && across <= 3.0 * n.size)
                        .then_some((below, l))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .and_then(|(_, l)| meaningful_name(&l.text))
        });
        if piece.name.starts_with("Piece ")
            && let Some(name) = under_number.or_else(|| {
                inside
                    .iter()
                    .filter(named)
                    .max_by(|a, b| a.naming.cmp(&b.naming).then(a.size.total_cmp(&b.size)))
                    .and_then(|l| meaningful_name(&l.text))
            })
        {
            piece.name = sentence_case(&name);
        }
        let cuts: Vec<CutInstruction> = inside
            .iter()
            .flat_map(|l| cuts_from_label(&l.text))
            .collect();
        if !cuts.is_empty() && piece.cut.iter().all(|c| *c == CutInstruction::default()) {
            piece.cut = cuts;
        }
        // "Place on fold" printed along a straight edge, near it; not a fold
        // line, folded in making up (a cut-on facing's, a waistband's).
        for label in labels {
            let t = label.text.to_lowercase();
            let fold_line = t.contains("foldline") || t.contains("fold line");
            if !t.contains("fold") || fold_line || t.split_whitespace().count() > 6 {
                continue;
            }
            let near = (0..piece.outline.edges.len())
                .filter_map(|i| {
                    let pts = flatten_segs(piece.outline.edge_geom(i).segs(), 0.05);
                    if !is_straight(&pts) || pts[0].distance(pts[pts.len() - 1]) < 30.0 {
                        return None;
                    }
                    let along = pts[pts.len() - 1] - pts[0];
                    let parallel = (along / along.hypot()).dot(label.dir).abs() > 0.98;
                    let d = distance_to_polyline(label.at, &pts, false);
                    (parallel && d <= 25.0).then_some((pts[0], pts[pts.len() - 1], d))
                })
                .min_by(|a, b| a.2.total_cmp(&b.2));
            if let Some((a, b, _)) = near {
                mark_folds(piece, a, b);
            }
        }
    }
}

/// One page's paths and words, in millimetres from its top-left corner.
struct PageContent {
    paths: Vec<PagePath>,
    labels: Vec<Label>,
    width: f64,
    height: f64,
    /// The area most of the drawing is clipped to: on a tile, its frame.
    frame: Option<Rect>,
    /// The frame was drawn round the drawing, which stops at it: a tile to
    /// trim there and butt to the next.
    drawn_frame: bool,
}

fn read_page(
    doc: &Document,
    index: usize,
    layer_name: &dyn Fn(u32) -> String,
) -> Result<PageContent, ImportError> {
    let page = doc.page(index).map_err(|e| ImportError::Unreadable {
        format: "PDF",
        detail: e.to_string(),
    })?;
    let (to_mm, width, height) = page_frame(&page);
    let area = Rect::new(0.0, 0.0, width, height);
    let paths = page
        .drawing
        .shapes
        .iter()
        .flat_map(|shape| page_paths(shape, to_mm, area, layer_name))
        .collect();
    // The clip most shapes share.
    let mut clips: Vec<(Rect, usize)> = Vec::new();
    for clip in page.drawing.shapes.iter().filter_map(|s| s.clip) {
        let r = to_mm.transform_rect_bbox(clip).intersect(area);
        match clips.iter_mut().find(|(c, _)| {
            (c.x0 - r.x0).abs() + (c.y0 - r.y0).abs() + (c.x1 - r.x1).abs() + (c.y1 - r.y1).abs()
                < 1.0
        }) {
            Some((_, n)) => *n += 1,
            None => clips.push((r, 1)),
        }
    }
    let mut paths: Vec<PagePath> = paths;
    let mut frame = clips
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(r, n)| *n >= 3 && r.width() > 50.0 && r.height() > 50.0)
        .map(|(r, _)| r);
    // A frame drawn round a drawing that stops at it: the lines of the
    // frame are no part of the pattern.
    let drawn = frame.is_none().then(|| {
        drawn_frame(
            paths.iter().map(|(r, _, _)| r.segs.as_slice()),
            Vec2::new(width, height),
        )
    });
    let drawn_frame = if let Some(Some(f)) = drawn {
        paths.retain(|(r, _, _)| !on_frame(&r.segs, f));
        frame = Some(f);
        true
    } else {
        false
    };
    Ok(PageContent {
        paths,
        labels: text_lines(&page.drawing.texts, to_mm),
        width,
        height,
        frame,
        drawn_frame,
    })
}

/// Takes out the moments where a path doubles back on itself (a stretch
/// under half a millimetre going back the way it came): the middle of the
/// stretch stands for both its ends.
fn despike(segs: &mut Vec<PathSeg>, closed: bool) {
    const SHORT: f64 = 0.5;
    let way = |s: &PathSeg| {
        let d = s.end() - s.start();
        if d.hypot() < 1e-9 {
            Vec2::ZERO
        } else {
            d / d.hypot()
        }
    };
    let mut k = 0;
    while segs.len() > 3 && k < segs.len() {
        let n = segs.len();
        let seg = segs[k];
        let back = (k > 0 || closed).then(|| segs[(k + n - 1) % n]);
        let on = (k + 1 < n || closed).then(|| segs[(k + 1) % n]);
        let doubles_back = matches!(seg, PathSeg::Line(_))
            && seg.start().distance(seg.end()) < SHORT
            && [back, on]
                .iter()
                .flatten()
                .any(|o| matches!(o, PathSeg::Line(_)) && way(o).dot(way(&seg)) < -0.5);
        if !doubles_back {
            k += 1;
            continue;
        }
        let mid = seg.start().midpoint(seg.end());
        let (prev, next) = ((k + n - 1) % n, (k + 1) % n);
        let moved = |s: PathSeg, start: Option<Point>, end: Option<Point>| match s {
            PathSeg::Line(mut l) => {
                l.p0 = start.unwrap_or(l.p0);
                l.p1 = end.unwrap_or(l.p1);
                PathSeg::Line(l)
            }
            other => other,
        };
        if back.is_some() {
            segs[prev] = moved(segs[prev], None, Some(mid));
        }
        if on.is_some() {
            segs[next] = moved(segs[next], Some(mid), None);
        }
        segs.remove(k);
        k = k.saturating_sub(1);
    }
}

/// A block's lines read from strokes, as labels. A pattern CAD's label
/// block (model, size, the piece's name, its cutting) gives the piece its
/// name in the line above "cut …", read fairly well, or else in a line of
/// capitals read closely. Other lettering names nothing: strokes read as
/// letters may be anything.
fn drawn_labels(lines: Vec<ReadLine>) -> Vec<Label> {
    let letters = |t: &str| t.chars().filter(|c| c.is_alphabetic()).count();
    let nameable = |l: &ReadLine| letters(&l.text) >= 3 && !is_instruction(&l.text);
    let above_cut = lines
        .iter()
        .position(|l| l.text.to_lowercase().starts_with("cut"))
        .and_then(|cut| (0..cut).rev().find(|&k| nameable(&lines[k])))
        .filter(|&k| lines[k].fit <= FAIR);
    let capitals = lines
        .iter()
        .position(|l| nameable(l) && !l.text.chars().any(char::is_lowercase) && l.fit <= SURE);
    lines
        .into_iter()
        .enumerate()
        .map(|(k, l)| Label {
            text: l.text,
            at: l.at,
            size: l.size,
            dir: l.dir,
            naming: if Some(k) == above_cut {
                Naming::AboveCutting
            } else if Some(k) == capitals {
                Naming::Capitals
            } else {
                Naming::Never
            },
        })
        .collect()
}

/// A line drawn along a side of `frame`.
fn on_frame(segs: &[PathSeg], frame: Rect) -> bool {
    let pts = flatten_segs(segs, 0.1);
    let near = |v: f64, at: f64| (v - at).abs() <= 0.3;
    [
        pts.iter().all(|p| near(p.x, frame.x0)),
        pts.iter().all(|p| near(p.x, frame.x1)),
        pts.iter().all(|p| near(p.y, frame.y0)),
        pts.iter().all(|p| near(p.y, frame.y1)),
    ]
    .contains(&true)
}

/// Pages whose drawings stop at a frame drawn round them, all one size,
/// placed by the grid under which lines cut off at one page's frame carry
/// on from the next's. The pages left over stay on their own. Also the
/// lines to stitch together across the joins (by page index).
fn butt_pages(pages: &[PageContent], prose: &[bool]) -> Option<(Vec<PageGroup>, Vec<Stitch>)> {
    let framed: Vec<(usize, Rect)> = pages
        .iter()
        .enumerate()
        .filter(|(i, p)| !prose[*i] && p.drawn_frame)
        .filter_map(|(i, p)| p.frame.map(|f| (i, f)))
        .collect();
    // The frame size most pages have.
    let size = |f: &Rect| {
        (
            (f.width() * 2.0).round() as i64,
            (f.height() * 2.0).round() as i64,
        )
    };
    let mut counts: HashMap<(i64, i64), usize> = HashMap::new();
    for (_, f) in &framed {
        *counts.entry(size(f)).or_default() += 1;
    }
    let (common, _) = counts.into_iter().max_by_key(|(k, n)| (*n, *k))?;
    let tiles: Vec<(usize, Rect)> = framed
        .into_iter()
        .filter(|(_, f)| size(f) == common)
        .collect();
    if tiles.len() < 2 {
        return None;
    }
    // Each page's strokes joined into lines first: lettering's are short.
    let edges: Vec<Edges> = tiles
        .iter()
        .map(|(i, f)| {
            let strokes = pages[*i]
                .paths
                .iter()
                .map(|(r, _, _)| r.segs.clone())
                .collect();
            let lines = chain(strokes, JOIN);
            Edges::of(
                lines
                    .iter()
                    .filter(|(_, closed)| !closed)
                    .map(|(s, _)| s.as_slice()),
                *f,
            )
        })
        .collect();
    let edges = Edges::without_furniture(edges);
    let frames: Vec<Rect> = tiles.iter().map(|(_, f)| *f).collect();
    let (at, stitches) = butt_grid(&edges, &frames)?;
    let mut groups = vec![tiles.iter().map(|(i, _)| *i).zip(at).collect::<Vec<_>>()];
    groups.extend(
        (0..pages.len())
            .filter(|i| !tiles.iter().any(|(t, _)| t == i))
            .map(|i| vec![(i, Vec2::ZERO)]),
    );
    let page = |k: usize| tiles[k].0;
    let stitches = stitches
        .into_iter()
        .map(|((a, p), (b, q))| ((page(a), p), (page(b), q)))
        .collect();
    Some((groups, stitches))
}

/// Brings together the two ends of each line cut off at a join of butted
/// tiles: both move to the middle of the hair between them. `placed` takes
/// a page's millimetres to the sheet's.
fn stitch(paths: &mut [PagePath], stitches: &[Stitch], placed: &HashMap<usize, Affine>) {
    for ((a, p), (b, q)) in stitches {
        let (Some(ta), Some(tb)) = (placed.get(a), placed.get(b)) else {
            continue;
        };
        let (p, q) = (*ta * *p, *tb * *q);
        let middle = p.midpoint(q);
        for end in [p, q] {
            for (r, _, _) in paths.iter_mut() {
                let n = r.segs.len();
                if let Some(PathSeg::Line(l)) = r.segs.first_mut()
                    && l.p0.distance(end) <= 0.01
                {
                    l.p0 = middle;
                }
                if let Some(PathSeg::Line(l)) = r.segs.get_mut(n.saturating_sub(1))
                    && l.p1.distance(end) <= 0.01
                {
                    l.p1 = middle;
                }
            }
        }
    }
}

/// Marks for joining tiles, drawn where they meet: lines along the joins,
/// diamonds on them, and circles with crosshairs beside them.
fn remove_join_marks(paths: &mut Vec<(RawPath, Style)>, (xs, ys): &(Vec<f64>, Vec<f64>)) {
    if xs.is_empty() && ys.is_empty() {
        return;
    }
    let on_join = |p: Point, reach: f64| {
        xs.iter().any(|x| (p.x - x).abs() <= reach) || ys.iter().any(|y| (p.y - y).abs() <= reach)
    };
    let along_join = |pts: &[Point]| {
        xs.iter()
            .any(|x| pts.iter().all(|p| (p.x - x).abs() <= 0.5))
            || ys
                .iter()
                .any(|y| pts.iter().all(|p| (p.y - y).abs() <= 0.5))
    };
    let shapes: Vec<(Vec<Point>, Rect)> = paths
        .iter()
        .map(|(r, _)| {
            let pts = flatten_segs(&r.segs, 0.1);
            let b = bounds(&pts);
            (pts, b)
        })
        .collect();
    // Crosshairs: two short lines crossing at their middles.
    let short: Vec<(usize, Point, Vec2)> = paths
        .iter()
        .zip(&shapes)
        .enumerate()
        .filter(|(_, ((r, _), (pts, _)))| {
            !r.closed && pts.len() == 2 && pts[0].distance(pts[1]) <= 15.0
        })
        .map(|(i, (_, (pts, _)))| (i, pts[0].midpoint(pts[1]), (pts[1] - pts[0]).normalize()))
        .collect();
    let mut gone = vec![false; paths.len()];
    let mut centres = Vec::new();
    for (k, (i, mid, dir)) in short.iter().enumerate() {
        for (j, mid2, dir2) in &short[k + 1..] {
            if mid.distance(*mid2) <= 0.5 && dir.dot(*dir2).abs() < 0.1 && on_join(*mid, 25.0) {
                gone[*i] = true;
                gone[*j] = true;
                centres.push(*mid);
            }
        }
    }
    for (i, ((r, _), (pts, b))) in paths.iter().zip(&shapes).enumerate() {
        let small = b.width().hypot(b.height()) <= 15.0;
        if (!r.closed
            && pts.len() >= 2
            && pts[0].distance(pts[pts.len() - 1]) >= 20.0
            && along_join(pts))
            || (r.closed && small && on_join(b.center(), 0.5))
            || (r.closed && small && centres.iter().any(|c| c.distance(b.center()) <= 0.5))
        {
            gone[i] = true;
        }
    }
    let mut i = 0;
    paths.retain(|_| {
        let keep = !gone[i];
        i += 1;
        keep
    });
}

/// Whether two paths are the same drawing, to within `tolerance` mm.
fn same_path(a: &[PathSeg], b: &[PathSeg], tolerance: f64) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(s, t)| {
            s.start().distance(t.start()) <= tolerance && s.end().distance(t.end()) <= tolerance
        })
}

pub fn import_pdf(data: &[u8], file_name: Option<&str>) -> Result<ImportResult, ImportError> {
    import_pdf_pages(data, file_name, None)
}

/// `import_pdf`, using only the `pages` given (numbered from 1), when given.
pub fn import_pdf_pages(
    data: &[u8],
    file_name: Option<&str>,
    pages: Option<&[usize]>,
) -> Result<ImportResult, ImportError> {
    let choice = pages;
    let unreadable = |detail: String| ImportError::Unreadable {
        format: "PDF",
        detail,
    };
    let doc = Document::load(data).map_err(|e| unreadable(e.to_string()))?;
    let layer_name = |id: u32| {
        doc.layers()
            .iter()
            .find(|l| l.id == id)
            .map(|l| l.name.clone())
            .unwrap_or_default()
    };
    let chosen = |i: usize| pages.is_none_or(|p| p.contains(&(i + 1)));
    let numbers: Vec<usize> = (0..doc.page_count()).filter(|&i| chosen(i)).collect();
    let mut pages: Vec<PageContent> = numbers
        .iter()
        .map(|&i| read_page(&doc, i, &layer_name))
        .collect::<Result<_, _>>()?;
    // Words printed over and over on most pages (a watermark: "FREE",
    // "Not for sale") name nothing.
    remove_watermarks(&mut pages);
    // A page of prose (instructions, a warning) is no part of the sheet;
    // left out unless chosen.
    let prose: Vec<bool> = pages
        .iter()
        .map(|p| choice.is_none() && is_prose(&p.labels))
        .collect();
    // Pages labelled for putting together ("1B") go where their labels
    // say; the labels are not the pattern's.
    let labelled = pagelabels::read(
        &pages
            .iter()
            .map(|p| (p.labels.as_slice(), p.width, p.height))
            .collect::<Vec<_>>(),
    );
    let mut grid = vec![None; pages.len()];
    if let Some(l) = &labelled {
        for (i, page) in pages.iter_mut().enumerate() {
            let mut k = 0;
            page.labels.retain(|_| {
                k += 1;
                !l.furniture[i].contains(&(k - 1))
            });
            if !prose[i] {
                grid[i] = l.places[i];
            }
        }
    }

    // Tiles of one sheet go back together; other pages lie side by side.
    // When some pages form a sheet, the pages outside it (a cover,
    // instructions) are left out.
    let prints: Vec<Vec<Print>> = pages
        .iter()
        .zip(&prose)
        .map(|(p, &prose)| {
            p.paths
                .iter()
                .filter(|_| !prose)
                .filter_map(|(r, _, _)| Print::of(&r.segs, r.closed))
                .collect()
        })
        .collect();
    let page_sizes: Vec<Vec2> = pages.iter().map(|p| Vec2::new(p.width, p.height)).collect();
    let mut groups = arrange(&prints, &page_sizes, &grid);
    // Tiles that share no shapes, each cut off at its frame: placed by
    // where their lines are cut off. They have no marks for joining them.
    let mut butted = false;
    let mut stitches = Vec::new();
    if !groups.iter().any(|g| g.len() > 1)
        && let Some((placed, lines)) = butt_pages(&pages, &prose)
    {
        groups = placed;
        stitches = lines;
        butted = true;
    }
    // Where each page went on the sheets.
    let mut placed: HashMap<usize, Affine> = HashMap::new();
    let tiled = groups.iter().any(|g| g.len() > 1);
    let mut notes = Vec::new();
    let mut styled: Vec<PagePath> = Vec::new();
    // The pages each kept path was drawn on.
    let mut drawn_on: Vec<Vec<usize>> = Vec::new();
    // Where each kept path starts, by 2 mm cell, to find copies quickly.
    let mut starts: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    let cell = |p: Point| ((p.x / 2.0).floor() as i64, (p.y / 2.0).floor() as i64);
    let mut labels: Vec<Label> = Vec::new();
    let (mut left_out, mut joined) = (0u32, 0u32);
    // Where tiles meet, across all sheets: (x of vertical joins, y of horizontal ones).
    let mut sheet_joins: (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    // Where each page lies on the sheets.
    let mut page_areas: Vec<Rect> = Vec::new();
    let mut x = 0.0;
    // Each page left out, and why.
    let mut unused: Vec<Option<PageLeftOut>> = vec![None; pages.len()];
    for group in &groups {
        if (tiled && group.len() == 1) || group.iter().all(|(i, _)| prose[*i]) {
            left_out += 1;
            let i = group[0].0;
            unused[i] = Some(if prose[i] {
                PageLeftOut::Instructions
            } else {
                PageLeftOut::NotJoined
            });
            continue;
        }
        if group.len() > 1 {
            joined += group.len() as u32;
        }
        // Footers and page labels sit at the same place on many pages.
        let mut label_places: HashMap<(i64, i64), usize> = HashMap::new();
        for (i, _) in group {
            let mut seen = std::collections::HashSet::new();
            for l in &pages[*i].labels {
                if seen.insert(cell(l.at)) {
                    *label_places.entry(cell(l.at)).or_default() += 1;
                }
            }
        }
        let furniture = |at: Point| {
            group.len() > 1
                && label_places
                    .get(&cell(at))
                    .is_some_and(|&c| c >= 3.max(group.len() / 2))
        };
        let mut joins = (Vec::new(), Vec::new());
        let sheet = group.iter().fold(
            Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN),
            |r, (i, at)| {
                let page = Rect::new(at.x, at.y, at.x + pages[*i].width, at.y + pages[*i].height);
                r.union(page)
            },
        );
        for (i, at) in group {
            let to_sheet = Affine::translate(*at + Vec2::new(x - sheet.x0, -sheet.y0));
            // The ends of the page's dashed lines: a short solid stroke of
            // their colour at one is a stub marking a dashed outline's
            // corner, one for each size.
            let mut dash_ends: Vec<(&Style, Point)> = Vec::new();
            for (r, style, _) in &pages[*i].paths {
                if style.dashed && !r.closed {
                    dash_ends.push((style, r.segs[0].start()));
                    dash_ends.push((style, r.segs[r.segs.len() - 1].end()));
                }
            }
            placed.insert(*i, to_sheet);
            page_areas.push(to_sheet.transform_rect_bbox(Rect::new(
                0.0,
                0.0,
                pages[*i].width,
                pages[*i].height,
            )));
            if group.len() > 1
                && !butted
                && let Some(f) = pages[*i].frame
            {
                let f = to_sheet.transform_rect_bbox(f);
                joins.0.extend([f.x0, f.x1]);
                joins.1.extend([f.y0, f.y1]);
            }
            for (r, style, shows) in &pages[*i].paths {
                let segs: Vec<PathSeg> = r.segs.iter().map(|s| to_sheet * *s).collect();
                // The same shape drawn by several tiles is kept once, and
                // shows if it shows on any of them. A long line drawn twice
                // on one page is two lines: each size of a nest draws its
                // whole outline, the stretches the sizes share too (and,
                // dashed, the stubs at its corners).
                let length = segs.iter().map(|s| s.arclen(0.01)).sum::<f64>();
                let stub = !style.dashed
                    && length <= DASH_STUB
                    && [r.segs[0].start(), r.segs[r.segs.len() - 1].end()]
                        .iter()
                        .any(|&end| {
                            dash_ends.iter().any(|&(dashed, p)| {
                                dashed.stroke == style.stroke
                                    && dashed.width == style.width
                                    && p.distance(end) <= DASH_GAP
                            })
                        });
                let long = length >= SHARED_LONG || stub;
                let (cx, cy) = cell(segs[0].start());
                let copy = (cx - 1..=cx + 1)
                    .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
                    .filter_map(|c| starts.get(&c))
                    .flatten()
                    .copied()
                    .find(|&k| {
                        let (o, os, _) = &styled[k];
                        !(long && drawn_on[k].contains(i))
                            && os == style
                            && o.closed == r.closed
                            && same_path(&o.segs, &segs, 0.2)
                    });
                match copy {
                    Some(k) => {
                        styled[k].2 |= *shows;
                        drawn_on[k].push(*i);
                    }
                    None => {
                        starts.entry((cx, cy)).or_default().push(styled.len());
                        styled.push((RawPath { segs, ..r.clone() }, style.clone(), *shows));
                        drawn_on.push(vec![*i]);
                    }
                }
            }
            for l in pages[*i].labels.iter().filter(|l| !furniture(l.at)) {
                let at = to_sheet * l.at;
                if !labels
                    .iter()
                    .any(|o| o.text == l.text && o.at.distance(at) <= 1.0)
                {
                    labels.push(Label { at, ..l.clone() });
                }
            }
        }
        x += sheet.width() + PAGE_GAP;
        sheet_joins.0.extend(joins.0);
        sheet_joins.1.extend(joins.1);
    }
    if joined > 0 {
        notes.push(ImportNote::PagesJoined { count: joined });
    }
    if left_out > 0 {
        notes.push(ImportNote::PagesLeftOut { count: left_out });
    }

    stitch(&mut styled, &stitches, &placed);

    // What shows nowhere (off every page) is not part of the pattern.
    let mut styled: Vec<(RawPath, Style)> = styled
        .into_iter()
        .filter(|(_, _, shows)| *shows)
        .map(|(r, s, _)| (r, s))
        .collect();
    remove_join_marks(&mut styled, &sheet_joins);

    // Outlines drawn as separate segments: join those drawn alike.
    let (closed, open): (Vec<_>, Vec<_>) = styled.into_iter().partition(|(r, _)| r.closed);
    let mut raw: Vec<(RawPath, Style)> = closed;
    let mut groups: Vec<(Style, RawPath, Vec<Vec<PathSeg>>)> = Vec::new();
    for (r, style) in open {
        match groups.iter_mut().find(|(s, _, _)| *s == style) {
            Some((_, _, paths)) => paths.push(r.segs),
            None => {
                let segs = r.segs.clone();
                groups.push((style, r, vec![segs]));
            }
        }
    }
    // A dashed outline's corners: solid stubs of its colour, a dash's gap
    // from its lines' ends. They go with the dashed lines.
    let ends = |p: &[PathSeg]| [p[0].start(), p[p.len() - 1].end()];
    let length = |p: &[PathSeg]| p.iter().map(|s| s.arclen(0.01)).sum::<f64>();
    for d in 0..groups.len() {
        if !groups[d].0.dashed {
            continue;
        }
        let solid = Style {
            dashed: false,
            ..groups[d].0.clone()
        };
        let Some(s) = groups.iter().position(|(st, _, _)| *st == solid) else {
            continue;
        };
        let dashed_ends: Vec<Point> = groups[d].2.iter().flat_map(|p| ends(p)).collect();
        let near_dash = |p: Point| dashed_ends.iter().any(|e| e.distance(p) <= DASH_GAP);
        let (stubs, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut groups[s].2)
            .into_iter()
            .partition(|p| length(p) <= DASH_STUB && ends(p).into_iter().any(near_dash));
        groups[s].2 = rest;
        let carried = continue_dashes(&groups[d].2, stubs);
        groups[d].2.extend(carried);
    }
    groups.retain(|(_, _, paths)| !paths.is_empty());
    // Where three or more lines end, of any style: the sizes of a nest at
    // a corner they share, and what closes them there. No line goes
    // sharply back there.
    let end_points: Vec<Point> = groups
        .iter()
        .flat_map(|(_, _, paths)| paths.iter().flat_map(|p| ends(p)))
        .collect();
    let end_cell = |p: Point| ((p.x / CROWD).floor() as i64, (p.y / CROWD).floor() as i64);
    let mut line_ends: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (k, &at) in end_points.iter().enumerate() {
        line_ends.entry(end_cell(at)).or_default().push(k);
    }
    let crowded = |at: Point| {
        let (cx, cy) = end_cell(at);
        (cx - 1..=cx + 1)
            .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
            .filter_map(|c| line_ends.get(&c))
            .flatten()
            .filter(|&&k| end_points[k].distance(at) <= CROWD)
            .count()
            >= 3
    };
    // Each style's lines joined end to end: its loops (those traced through
    // a corner they share, apart) and the lines left open.
    let mut joined: Vec<Joined> = groups
        .iter_mut()
        .map(|(style, _, paths)| {
            let reach = if style.dashed { DASH_GAP } else { BRIDGE };
            let chained = chain_crowded(std::mem::take(paths), JOIN, &crowded);
            // A line drawn in two long pieces that run straight on into
            // each other, overlapping a little or a little apart (a panel's
            // side drawn from each end to the middle), is one line; the
            // hair where they overlap goes with the other spikes.
            let straight_on = Some((STRAIGHT_ON, STRAIGHT_ON_LONG));
            let (loops, open): (Vec<_>, Vec<_>) =
                bridge_crowded(chained, reach, &crowded, straight_on)
                    .into_iter()
                    .partition(|(_, closed)| *closed);
            (
                loops
                    .into_iter()
                    .flat_map(|(segs, _)| split_touching(segs, JOIN))
                    .collect(),
                open.into_iter().map(|(segs, _)| segs).collect(),
            )
        })
        .collect();
    close_across_styles(&mut joined);
    for ((style, template, _), (loops, open)) in groups.into_iter().zip(joined) {
        // Lines left open may make loops together, meeting end to middle.
        let looped = find_loops(open, JOIN);
        for (segs, closed) in loops.into_iter().map(|segs| (segs, true)).chain(looped) {
            raw.push((
                RawPath {
                    segs,
                    closed,
                    filled: template.filled,
                    hint: template.hint.clone(),
                    name: None,
                    reference: None,
                    cut: None,
                },
                style.clone(),
            ));
        }
    }

    // Drawings printed through plotter drivers draw everything as strokes:
    // page frames come together only now, and lettering and logos are
    // lines too.
    raw.retain(|(r, _)| !(r.closed && page_areas.iter().any(|p| is_page_frame(&r.segs, *p))));
    let plain: Vec<RawPath> = raw.iter().map(|(r, _)| r.clone()).collect();
    // Arrowheads are left for the grainlines they belong to.
    let heads = arrowheads(&plain);
    let blocks = find_lettering(&plain, &heads);
    let mut lettering: Vec<usize> = blocks.iter().flat_map(|b| b.paths.clone()).collect();
    lettering.sort_unstable();
    let letter_boxes: Vec<Rect> = blocks.iter().map(|b| b.bounds).collect();
    // Lettering drawn as strokes, read: the pieces' names and cutting.
    for block in &blocks {
        let strokes: Vec<Vec<Point>> = block
            .paths
            .iter()
            .map(|&i| flatten_segs(&plain[i].segs, 0.05))
            .collect();
        labels.extend(drawn_labels(read_block(&strokes)));
    }
    let (mut crosses, mut cross_paths) = find_crosses(&plain, &lettering);
    let (outlined, outlined_paths) = find_outlined_crosses(&plain);
    crosses.extend(outlined);
    cross_paths.extend(outlined_paths);
    let (decorations, logos) = find_repeated_decorations(&plain, &cross_paths);
    let drawn_words = letter_boxes.len() + logos;
    if drawn_words > 0 {
        notes.push(ImportNote::LetteringIgnored {
            count: u32::try_from(drawn_words).unwrap_or(u32::MAX),
        });
    }
    let mut gone = vec![false; raw.len()];
    for i in lettering.iter().chain(&decorations).chain(&cross_paths) {
        gone[*i] = true;
    }
    let mut i = 0;
    raw.retain(|_| {
        i += 1;
        !gone[i - 1]
    });

    // For working on the importer: the joined drawing as an SVG, open
    // paths red and closed ones blue.
    if let Some(out) = std::env::var_os("DRAPEWRIGHT_DEBUG_SVG") {
        let mut svg = String::from(
            "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 3000 2500' width='3000mm' height='2500mm'>",
        );
        for (r, _) in &raw {
            let mut d = String::new();
            for (k, seg) in r.segs.iter().enumerate() {
                if k == 0 {
                    d.push_str(&format!("M{:.2},{:.2} ", seg.start().x, seg.start().y));
                }
                d.push_str(&format!("L{:.2},{:.2} ", seg.end().x, seg.end().y));
            }
            let colour = if r.closed { "blue" } else { "red" };
            svg.push_str(&format!(
                "<path d='{d}' fill='none' stroke='{colour}' stroke-width='0.3'/>"
            ));
        }
        svg.push_str("</svg>");
        let _ = std::fs::write(out, svg);
    }

    // Where tiles each draw a hair past their frame, the two halves of a
    // line crossing the join overlap: joined end to end, the line doubles
    // back on itself there for a moment.
    for (r, _) in &mut raw {
        despike(&mut r.segs, r.closed);
    }

    // The test square checks the print; it is not a piece (nor a size).
    let mut square = None;
    raw.retain(|(r, _)| match test_square(r, &labels, &letter_boxes) {
        Some(side) => {
            square.get_or_insert(side);
            false
        }
        None => true,
    });

    // A pattern in several sizes is assembled one size at a time, with what
    // the sizes share.
    let order: Vec<u32> = doc.layers().iter().map(|l| l.id).collect();
    let sizes = find_sizes(&raw, &layer_name, &order, &labels);
    let subsets: Vec<(Option<usize>, Vec<RawPath>)> = match &sizes {
        Some(sizes) => (0..sizes.names.len())
            .map(|k| {
                let paths = raw
                    .iter()
                    .zip(&sizes.of_path)
                    .filter(|(_, of)| of.is_none_or(|s| s == k))
                    .map(|((r, _), _)| r.clone())
                    .collect();
                (Some(k), paths)
            })
            .collect(),
        None => vec![(None, raw.into_iter().map(|(r, _)| r).collect())],
    };

    let scale = ScaleInfo {
        chosen: ScaleOption {
            mm_per_unit: MM_PER_POINT,
            basis: ScaleBasis::DeclaredUnits,
        },
        certain: true,
        reason: ScaleReason::DeclaredUnits,
        alternatives: Vec::new(),
    };
    let sha256 = sha256_hex(data);
    let mut pieces: Vec<Piece> = Vec::new();
    let mut piece_sizes = Vec::new();
    // Notes counting what was left out are the same for every size; keep
    // the largest count of each.
    let mut counted: Vec<ImportNote> = Vec::new();
    for (size, mut raw) in subsets {
        // The pattern's marks.
        let bracket_ends = find_fold_brackets(&mut raw);
        // Before notches: a grainline's head can touch an outline.
        find_grainlines(&mut raw);
        let notches = find_notches(&mut raw, &bracket_ends);
        let (mut found, assembled) =
            assemble(raw, &notches, SourceFormat::Pdf, &scale, file_name, &sha256);
        read_labels(&mut found, &labels);
        name_lines(&mut found, &labels);
        add_crosses(&mut found, &crosses);
        // Lines drawn once for all the sizes reach the largest size's
        // outline: each size's stop at its own.
        if size.is_some() {
            for piece in &mut found {
                clip_inside(piece);
            }
        }
        // Piece ids stay unique across sizes.
        let suffix = size.map(|k| format!("-{k}")).unwrap_or_default();
        for piece in &mut found {
            piece.id.push_str(&suffix);
            if let (Some(k), Some(sizes)) = (size, &sizes) {
                piece_sizes.push(PieceSize {
                    piece: piece.id.clone(),
                    size: sizes.names[k].clone(),
                });
            }
        }
        for mut note in assembled {
            match &mut note {
                ImportNote::SeamLineFound { piece, .. }
                | ImportNote::CuttingLineDiffers { piece, .. }
                | ImportNote::OutlineCrossesItself { piece } => {
                    piece.push_str(&suffix);
                    notes.push(note);
                }
                _ => match counted
                    .iter_mut()
                    .find(|n| std::mem::discriminant(*n) == std::mem::discriminant(&note))
                {
                    Some(existing) if count(existing) < count(&note) => *existing = note,
                    Some(_) => {}
                    None => counted.push(note),
                },
            }
        }
        pieces.extend(found);
    }
    if let Some(side) = square {
        notes.push(ImportNote::TestSquareFound { size: side });
    }
    notes.extend(counted);
    let sizes = sizes
        .map(|s| ImportSizes {
            names: s.names,
            sample: None,
            pieces: piece_sizes,
            swatches: s
                .swatches
                .into_iter()
                .map(|(colour, dashed)| SizeSwatch { colour, dashed })
                .collect(),
        })
        .unwrap_or_default();
    // Every page, and whether it was used.
    let pages = (0..doc.page_count())
        .map(|i| {
            let k = numbers.iter().position(|&n| n == i);
            PageUse {
                number: u32::try_from(i + 1).unwrap_or(u32::MAX),
                label: k.and_then(|k| labelled.as_ref().and_then(|l| l.names[k].clone())),
                left_out: match k {
                    None => Some(PageLeftOut::NotChosen),
                    Some(k) => unused[k],
                },
            }
        })
        .collect();
    Ok(ImportResult {
        allowance: AllowanceQuestion::for_unmeasured(&pieces, &notes, None),
        pieces,
        scale,
        sizes,
        notes,
        sha256,
        pages,
    })
}

/// A style's lines joined: its loops, and the lines left open.
type Joined = (Vec<Vec<PathSeg>>, Vec<Vec<PathSeg>>);
/// A line left open: (style, line).
type LineOf = (usize, usize);

/// Outlines drawn partly in another style: each size of a nest drawn in
/// its own, but for a stretch every size draws alike (a crotch's black
/// seam). A line left open whose two ends are the two ends of a much
/// shorter line of another style is closed by it (never by another size's
/// outline, running from the same corners): the one that fits best, each
/// such line closing one outline unless there are no more of them (a
/// stretch drawn once for all the sizes; the outlines it closes then nest,
/// never lying either side of it).
fn close_across_styles(joined: &mut [Joined]) {
    let ends = |p: &[PathSeg]| (p[0].start(), p[p.len() - 1].end());
    let length = |p: &[PathSeg]| p.iter().map(|s| s.arclen(0.01)).sum::<f64>();
    let cell = |p: Point| ((p.x / ACROSS).floor() as i64, (p.y / ACROSS).floor() as i64);
    // Every open line, numbered across the styles: (style, line).
    let lines: Vec<LineOf> = joined
        .iter()
        .enumerate()
        .flat_map(|(g, (_, open))| (0..open.len()).map(move |k| (g, k)))
        .collect();
    let line = |n: usize| &joined[lines[n].0].1[lines[n].1];
    // Open lines by where they start and where they end.
    let mut by_end: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for n in 0..lines.len() {
        let (a, b) = ends(line(n));
        if a.distance(b) > ACROSS {
            for at in [a, b] {
                let list = by_end.entry(cell(at)).or_default();
                if list.last() != Some(&n) {
                    list.push(n);
                }
            }
        }
    }
    // Every fit, by line and then closing line: (gaps, line, closing line,
    // closing line reversed?).
    let mut fits: Vec<(f64, usize, usize, bool)> = Vec::new();
    for n in 0..lines.len() {
        let p = line(n);
        let (a, b) = ends(p);
        if a.distance(b) <= ACROSS {
            continue;
        }
        let long = length(p);
        let (cx, cy) = cell(b);
        let mut near: Vec<usize> = (cx - 1..=cx + 1)
            .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
            .filter_map(|c| by_end.get(&c))
            .flatten()
            .copied()
            .filter(|&m| lines[m].0 != lines[n].0)
            .collect();
        near.sort_unstable();
        near.dedup();
        for m in near {
            let q = line(m);
            if length(q) > CLOSER_SHORTER * long {
                continue;
            }
            let (qa, qb) = ends(q);
            for (reversed, from, to) in [(false, qa, qb), (true, qb, qa)] {
                if from.distance(b) <= ACROSS && to.distance(a) <= ACROSS {
                    fits.push((from.distance(b) + to.distance(a), n, m, reversed));
                }
            }
        }
    }
    let order = ascending(&fits.iter().map(|f| f.0).collect::<Vec<_>>());
    // The outline a line and its closing line make.
    let outline = |n: usize, m: usize, reversed: bool| {
        let mut segs = line(n).clone();
        if reversed {
            segs.extend(line(m).iter().rev().map(PathSeg::reverse));
        } else {
            segs.extend(line(m).iter().copied());
        }
        segs
    };
    let nest = |a: &[Point], b: &[Point]| {
        let (inside_a, _) = label_point(a);
        let (inside_b, _) = label_point(b);
        point_in_polygon(inside_a, b) || point_in_polygon(inside_b, a)
    };
    // Best fits first, each line closed once, each closing line used once
    // while any is left unused.
    let mut uses = vec![0usize; lines.len()];
    let mut closed: Vec<(usize, usize, bool)> = Vec::new();
    for round in 0..2 {
        for &k in &order {
            let (_, n, m, reversed) = fits[k];
            if closed.iter().any(|c| c.0 == n || c.0 == m)
                || uses[n] > 0
                || (round == 0 && uses[m] > 0)
            {
                continue;
            }
            if round == 1 {
                let mine = flatten_segs(&outline(n, m, reversed), 0.5);
                let apart = closed
                    .iter()
                    .filter(|c| c.1 == m)
                    .any(|c| !nest(&mine, &flatten_segs(&outline(c.0, c.1, c.2), 0.5)));
                if apart {
                    continue;
                }
            }
            uses[m] += 1;
            closed.push((n, m, reversed));
        }
    }
    // The loops, each round some room; the lines that closed them are
    // part of them now.
    let mut gone = vec![false; lines.len()];
    let mut loops: Vec<(usize, Vec<PathSeg>)> = Vec::new();
    for &(n, m, reversed) in &closed {
        let mut segs = outline(n, m, reversed);
        // Hairline gaps where the line meets the line closing it.
        let (start, joint) = (line(n)[0].start(), line(n).len());
        let (end, back) = (segs[joint - 1].end(), segs[joint].start());
        if end.distance(back) > 1e-9 {
            segs.insert(joint, PathSeg::Line(Line::new(end, back)));
        }
        let last = segs[segs.len() - 1].end();
        if last.distance(start) > 1e-9 {
            segs.push(PathSeg::Line(Line::new(last, start)));
        }
        let pts = flatten_segs(&segs, 0.5);
        let around: f64 = segs.iter().map(|s| s.arclen(0.01)).sum();
        if polygon_area(&pts).abs() < 0.5 * around {
            continue;
        }
        loops.push((lines[n].0, segs));
        gone[n] = true;
        gone[m] = true;
    }
    for (g, segs) in loops {
        joined[g].0.push(segs);
    }
    let mut n = 0;
    for (_, open) in joined.iter_mut() {
        open.retain(|_| {
            n += 1;
            !gone[n - 1]
        });
    }
}

/// Dashed lines carried on to their corners by the stubs that mark them:
/// a stub straight on from a line's end, a dash's gap away, carries it on
/// (a stub between two lines carries both). The stubs, and the gaps
/// across to the lines they carry on, to be joined with the lines.
fn continue_dashes(dashed: &[Vec<PathSeg>], stubs: Vec<Vec<PathSeg>>) -> Vec<Vec<PathSeg>> {
    // Each dashed line's ends: (point, heading on).
    let mut ends: Vec<(Point, Vec2)> = Vec::new();
    for p in dashed {
        ends.push((p[0].start(), -unit_tangent(&p[0], 0.0)));
        ends.push((p[p.len() - 1].end(), unit_tangent(&p[p.len() - 1], 1.0)));
    }
    // Every stub end that carries a line's end on: (gap, line end, stub
    // end); stub end 2k is stub k's start, 2k + 1 its end.
    let mut fits: Vec<(f64, usize, usize)> = Vec::new();
    for (e, &(at, on)) in ends.iter().enumerate() {
        for (k, stub) in stubs.iter().enumerate() {
            let (a, b) = (stub[0].start(), stub[stub.len() - 1].end());
            for (near, far, end) in [(a, b, 2 * k), (b, a, 2 * k + 1)] {
                let gap = near - at;
                let along = far - near;
                let straight_on = along.hypot() < 1e-6 || along.dot(on) >= 0.95 * along.hypot();
                if gap.hypot() <= DASH_GAP
                    && gap.dot(on) >= -JOIN
                    && gap.cross(on).abs() <= 0.15
                    && straight_on
                {
                    fits.push((gap.hypot(), e, end));
                }
            }
        }
    }
    // Nearest first (then by line end and stub end, as found).
    let order = ascending(&fits.iter().map(|f| f.0).collect::<Vec<_>>());
    let (mut end_taken, mut stub_end_taken) =
        (vec![false; ends.len()], vec![false; 2 * stubs.len()]);
    let mut gaps: Vec<Vec<PathSeg>> = Vec::new();
    for (gap, e, end) in order.into_iter().map(|k| fits[k]) {
        if end_taken[e] || stub_end_taken[end] {
            continue;
        }
        end_taken[e] = true;
        stub_end_taken[end] = true;
        let stub = &stubs[end / 2];
        let near = if end.is_multiple_of(2) {
            stub[0].start()
        } else {
            stub[stub.len() - 1].end()
        };
        if gap > 1e-9 {
            gaps.push(vec![PathSeg::Line(Line::new(ends[e].0, near))]);
        }
    }
    let mut out = stubs;
    out.extend(gaps);
    out
}

/// Words printed several times over on most pages (a watermark, "FREE",
/// "Not for sale"), taken out: they name nothing.
fn remove_watermarks(pages: &mut [PageContent]) {
    // Each word and its size, as a key: its text hashed (FNV-1a), which
    // keeps to the kinds of map the importer already has.
    let key = |l: &Label| {
        let text = l.text.trim().to_lowercase();
        let hash = text.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
            (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
        });
        (hash as i64, (l.size * 2.0).round() as i64)
    };
    // The pages each word is printed twice or more on.
    let mut on_pages: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (p, page) in pages.iter().enumerate() {
        let mut here: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
        for l in &page.labels {
            here.entry(key(l)).or_default().push(p);
        }
        for (k, times) in here {
            if times.len() >= 2 {
                on_pages.entry(k).or_default().push(p);
            }
        }
    }
    let n = pages.len();
    let marks: Vec<(i64, i64)> = on_pages
        .into_iter()
        .filter(|(_, on)| on.len() >= 3.max(n.div_ceil(2)))
        .map(|(k, _)| k)
        .collect();
    if marks.is_empty() {
        return;
    }
    for page in pages {
        page.labels.retain(|l| !marks.contains(&key(l)));
    }
}

/// Lines of prose this many, of `PROSE_WORDS` words or more, make a page of
/// instructions.
const PROSE_LINES: usize = 5;
const PROSE_WORDS: usize = 4;

/// Whether a page's words are mostly prose to read: instructions, a
/// warning, not a pattern's labels.
fn is_prose(labels: &[Label]) -> bool {
    labels
        .iter()
        .filter(|l| {
            l.text
                .split_whitespace()
                .filter(|w| w.chars().any(char::is_alphabetic))
                .count()
                >= PROSE_WORDS
        })
        .count()
        >= PROSE_LINES
}

/// A piece's lines, and its grainline, cut back to its outline: of a line
/// crossing it, the longest part inside is kept; one wholly outside goes.
fn clip_inside(piece: &mut Piece) {
    let outline = flatten_segs(&piece.outline.path_segs(), 0.05);
    let edges: Vec<Line> = outline
        .iter()
        .zip(outline.iter().cycle().skip(1))
        .map(|(a, b)| Line::new(*a, *b))
        .collect();
    let inside_part = |segs: &[PathSeg]| -> Option<Vec<PathSeg>> {
        // Each segment cut where it crosses the outline, each part in or
        // out by its middle.
        let mut parts: Vec<(PathSeg, bool)> = Vec::new();
        for seg in segs {
            let mut cuts: Vec<f64> = edges
                .iter()
                .flat_map(|e| seg.intersect_line(*e))
                .filter(|x| (0.0..=1.0).contains(&x.line_t))
                .map(|x| x.segment_t)
                .filter(|t| *t > 1e-6 && *t < 1.0 - 1e-6)
                .collect();
            cuts.sort_by(f64::total_cmp);
            cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
            let mut from = 0.0;
            for t in cuts.into_iter().chain([1.0]) {
                let part = seg.subsegment(from..t);
                parts.push((part, point_in_polygon(part.eval(0.5), &outline)));
                from = t;
            }
        }
        if parts.iter().all(|(_, inside)| *inside) {
            return None;
        }
        // The longest run of parts inside.
        let mut best: Option<(f64, usize, usize)> = None;
        let mut k = 0;
        while k < parts.len() {
            if !parts[k].1 {
                k += 1;
                continue;
            }
            let start = k;
            let mut length = 0.0;
            while k < parts.len() && parts[k].1 {
                length += parts[k].0.arclen(0.01);
                k += 1;
            }
            if best.is_none_or(|(l, _, _)| length > l) {
                best = Some((length, start, k));
            }
        }
        Some(match best {
            Some((_, a, b)) => parts[a..b].iter().map(|(p, _)| *p).collect(),
            None => Vec::new(),
        })
    };
    piece.lines.retain_mut(
        |line| match inside_part(&segs_of(line.start, &line.segments)) {
            None => true,
            Some(part) if part.iter().map(|s| s.arclen(0.01)).sum::<f64>() < 1.0 => false,
            Some(part) => {
                line.start = Pt::from(part[0].start());
                line.segments = to_segments(&part);
                true
            }
        },
    );
    if let Some(g) = &mut piece.grainline {
        let seg = PathSeg::Line(Line::new(Point::from(g.from), Point::from(g.to)));
        if let Some(part) = inside_part(&[seg])
            && let (Some(first), Some(last)) = (part.first(), part.last())
            && first.start().distance(last.end()) >= 10.0
        {
            g.from = Pt::from(first.start());
            g.to = Pt::from(last.end());
        }
    }
}

/// The body's levels a line across a piece may be named for: where it
/// goes on the body (a corset's waist line at the waist).
const BODY_LEVELS: [&str; 10] = [
    "waist",
    "natural waist",
    "bust",
    "underbust",
    "under bust",
    "chest",
    "hip",
    "high hip",
    "mid hip",
    "low hip",
];

/// Lines across pieces named by the body level printed along them
/// ("WAIST" just above a line): "Waist".
fn name_lines(pieces: &mut [Piece], labels: &[Label]) {
    let levels: Vec<&Label> = labels
        .iter()
        .filter(|l| BODY_LEVELS.contains(&l.text.trim().to_lowercase().as_str()))
        .collect();
    if levels.is_empty() {
        return;
    }
    for piece in pieces {
        for line in &mut piece.lines {
            if line.label.is_some() {
                continue;
            }
            let pts = flatten_segs(&segs_of(line.start, &line.segments), 0.5);
            let (Some(&a), Some(&b)) = (pts.first(), pts.last()) else {
                continue;
            };
            let along = b - a;
            if along.hypot() < 20.0 {
                continue;
            }
            let unit = along / along.hypot();
            // The nearest label naming a level, printed along it.
            let mut named: Option<(f64, &Label)> = None;
            for &l in &levels {
                if l.dir.dot(unit).abs() < 0.95 {
                    continue;
                }
                let d = distance_to_polyline(l.at, &pts, false);
                if d <= (2.5 * l.size).max(4.0)
                    && (0.0..=along.hypot()).contains(&(l.at - a).dot(unit))
                    && named.is_none_or(|(nearest, _)| d < nearest)
                {
                    named = Some((d, l));
                }
            }
            if let Some((_, l)) = named {
                line.label = Some(sentence_case(l.text.trim()));
            }
        }
    }
}

/// "+" marks on the pieces they sit inside.
fn add_crosses(pieces: &mut [Piece], crosses: &[(Point, f64)]) {
    for (k, (at, size)) in crosses.iter().enumerate() {
        let inside = pieces.iter_mut().find(|p| {
            let poly = flatten_segs(&p.outline.path_segs(), 0.5);
            point_in_polygon(*at, &poly) || distance_to_polyline(*at, &poly, true) <= 20.0
        });
        if let Some(piece) = inside {
            piece.marks.push(Mark {
                id: format!("x{k}"),
                kind: MarkKind::Cross,
                at: Pt::from(*at),
                size: Some(*size),
                length: None,
                angle: 0.0,
                label: None,
            });
        }
    }
}

/// How many things a "left out" note counts.
fn count(note: &ImportNote) -> u32 {
    match note {
        ImportNote::SmallShapesIgnored { count }
        | ImportNote::LooseLinesIgnored { count }
        | ImportNote::LinesOnOutlineIgnored { count }
        | ImportNote::UnsupportedEntities { count }
        | ImportNote::CurvesTraced { count } => *count,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cutting_from_labels() {
        let one = |t: &str| cuts_from_label(t);
        let c = one("CUT 2");
        assert_eq!(
            (c[0].quantity, c[0].mirrored, c[0].on_fold),
            (2, None, None)
        );
        let c = one("Cut 1 on fold");
        assert_eq!((c[0].quantity, c[0].on_fold), (1, Some(true)));
        let c = one("Cut 1 pair from lining");
        assert_eq!(
            (c[0].quantity, c[0].mirrored, c[0].material.as_str()),
            (2, Some(true), "lining")
        );
        let c = one("Cut 2 main fabric, cut 2 interfacing");
        assert_eq!(c.len(), 2);
        assert_eq!(c[1].material, "interfacing");
        assert!(one("Cutting layout").is_empty());
    }

    #[test]
    fn instructions_are_not_names() {
        assert!(is_instruction("CUT 2"));
        assert!(is_instruction("Place on fold"));
        assert!(is_instruction("A1"));
        assert!(is_instruction("Size 12"));
        assert!(!is_instruction("FRONT BODICE"));
        assert!(!is_instruction("Sleeve"));
        assert!(is_instruction("FACE!"));
        assert!(!is_instruction("Face mask"));
        assert!(!is_instruction("Back neck facing"));
    }

    #[test]
    fn drawn_names_above_their_cutting() {
        let line = |text: &str, fit: f64| ReadLine {
            text: text.to_string(),
            at: Point::ZERO,
            size: 3.0,
            dir: Vec2::new(1.0, 0.0),
            fit,
        };
        let naming = |lines: Vec<ReadLine>| -> Vec<Naming> {
            drawn_labels(lines).iter().map(|l| l.naming).collect()
        };
        use Naming::*;
        // The model and size come first; the name is above the cutting,
        // though read in another hand than the font's.
        assert_eq!(
            naming(vec![
                line("ODILE blouse", 0.3),
                line("size 40", 0.3),
                line("SLEEVE", 1.1),
                line("cut 2", 0.3)
            ]),
            [Never, Never, AboveCutting, Never]
        );
        // Without cutting, a line of capitals read closely may be one; not
        // one read far off the font, as strokes that only look like letters
        // are, nor other lettering.
        assert_eq!(
            naming(vec![
                line("LLLLLLLL", 1.2),
                line("BELT LOOPS", 0.4),
                line("seam", 0.3)
            ]),
            [Never, Capitals, Never]
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // Rounded millimetres, exact.
    fn pages_turn_as_shown() {
        let page = |rotate| Page {
            media_box: Rect::new(0.0, 0.0, 720.0, 1440.0),
            crop_box: Rect::new(0.0, 0.0, 720.0, 1440.0),
            rotate,
            user_unit: 1.0,
            drawing: Default::default(),
        };
        // The bottom-left corner of a portrait page, at each rotation.
        let corner = Point::new(0.0, 0.0);
        let (f, w, h) = page_frame(&page(0));
        assert_eq!((w.round(), h.round()), (254.0, 508.0));
        assert!((f * corner).distance(Point::new(0.0, 508.0)) < 1e-9);
        let (f, w, _) = page_frame(&page(90));
        assert_eq!(w.round(), 508.0);
        assert!((f * corner).distance(Point::new(0.0, 0.0)) < 1e-9);
        let (f, _, _) = page_frame(&page(180));
        assert!((f * corner).distance(Point::new(254.0, 0.0)) < 1e-9);
        let (f, _, _) = page_frame(&page(270));
        assert!((f * corner).distance(Point::new(508.0, 254.0)) < 1e-9);
    }
}
