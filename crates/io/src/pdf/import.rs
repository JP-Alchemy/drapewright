//! Sewing patterns from PDF files. A PDF measures in points (1/72 inch), so
//! its scale is exact. Each page's drawing is read with
//! `drapewright-pdf-import`, assembled into pieces like any other drawing,
//! and the words printed on the pattern name the pieces, say how many to
//! cut, and mark edges to place on the fold.

use std::collections::HashMap;

use drapewright_pattern_core::geom::{distance_to_polyline, flatten_segs, point_in_polygon};
use drapewright_pattern_core::{
    CutInstruction, Mark, MarkKind, Piece, Pt, ScaleBasis, SourceFormat,
};
use drapewright_pdf_import::{Document, Page, Shape, TextRun};
use kurbo::{Affine, BezPath, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, Rect, Vec2};

use super::lettering::{
    find_crosses, find_lettering, find_outlined_crosses, find_repeated_decorations,
};
use super::loops::find_loops;
use super::marks::{arrowheads, find_fold_brackets, find_grainlines, find_notches, test_square};
use super::sizes::find_sizes;
use super::tiles::{Print, arrange};
use crate::assemble::{
    CLOSE_TOLERANCE, RawPath, assemble, bounds, bridge, chain, is_straight, mark_folds,
    meaningful_name, sentence_case,
};
use crate::import::{
    AllowanceQuestion, ImportError, ImportNote, ImportResult, ImportSizes, PieceSize, ScaleInfo,
    ScaleOption, ScaleReason, SizeSwatch, sha256_hex,
};

const MM_PER_POINT: f64 = 25.4 / 72.0;
/// Room left between pages that are not parts of one sheet, mm.
const PAGE_GAP: f64 = 100.0;
/// Segments drawn separately are joined when their ends are this close, mm.
const JOIN: f64 = 0.1;
/// Hairline gaps a plotter driver leaves between strokes, mm.
const BRIDGE: f64 = 0.5;

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
    axis_aligned && b.width() >= 0.8 * page.width() && b.height() >= 0.8 * page.height()
}

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
        "gather", "hem", "inch", "cm", "mm",
    ];
    let words = t.split_whitespace().count();
    let letters = t.chars().filter(|c| c.is_alphabetic()).count();
    letters < 3
        || words > 6
        || starts.iter().any(|s| t.starts_with(s))
        || t.contains("cut ")
        || t.contains("on fold")
        || t.contains("grainline")
        // Page labels such as "A1", "B12".
        || (t.len() <= 4 && t.chars().next().is_some_and(char::is_alphabetic) && t[1..].chars().all(|c| c.is_ascii_digit()))
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
        if piece.name.starts_with("Piece ")
            && let Some(name) = inside
                .iter()
                .filter(|l| !is_instruction(&l.text))
                .max_by(|a, b| a.size.total_cmp(&b.size))
                .and_then(|l| meaningful_name(&l.text))
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
        // "Place on fold" printed along a straight edge, near it.
        for label in labels {
            let t = label.text.to_lowercase();
            if !t.contains("fold") || t.split_whitespace().count() > 6 {
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
    let frame = clips
        .into_iter()
        .max_by_key(|(_, n)| *n)
        .filter(|(r, n)| *n >= 3 && r.width() > 50.0 && r.height() > 50.0)
        .map(|(r, _)| r);
    Ok(PageContent {
        paths,
        labels: text_lines(&page.drawing.texts, to_mm),
        width,
        height,
        frame,
    })
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
    let pages: Vec<PageContent> = (0..doc.page_count())
        .map(|i| read_page(&doc, i, &layer_name))
        .collect::<Result<_, _>>()?;

    // Tiles of one sheet go back together; other pages lie side by side.
    // When some pages form a sheet, the pages outside it (a cover,
    // instructions) are left out.
    let prints: Vec<Vec<Print>> = pages
        .iter()
        .map(|p| {
            p.paths
                .iter()
                .filter_map(|(r, _, _)| Print::of(&r.segs, r.closed))
                .collect()
        })
        .collect();
    let page_sizes: Vec<Vec2> = pages.iter().map(|p| Vec2::new(p.width, p.height)).collect();
    let groups = arrange(&prints, &page_sizes);
    let tiled = groups.iter().any(|g| g.len() > 1);
    let mut notes = Vec::new();
    let mut styled: Vec<PagePath> = Vec::new();
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
    for group in &groups {
        if tiled && group.len() == 1 {
            left_out += 1;
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
            page_areas.push(to_sheet.transform_rect_bbox(Rect::new(
                0.0,
                0.0,
                pages[*i].width,
                pages[*i].height,
            )));
            if group.len() > 1
                && let Some(f) = pages[*i].frame
            {
                let f = to_sheet.transform_rect_bbox(f);
                joins.0.extend([f.x0, f.x1]);
                joins.1.extend([f.y0, f.y1]);
            }
            for (r, style, shows) in &pages[*i].paths {
                let segs: Vec<PathSeg> = r.segs.iter().map(|s| to_sheet * *s).collect();
                // The same shape drawn by several tiles is kept once, and
                // shows if it shows on any of them.
                let (cx, cy) = cell(segs[0].start());
                let copy = (cx - 1..=cx + 1)
                    .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
                    .filter_map(|c| starts.get(&c))
                    .flatten()
                    .copied()
                    .find(|&k| {
                        let (o, os, _) = &styled[k];
                        os == style && o.closed == r.closed && same_path(&o.segs, &segs, 0.2)
                    });
                match copy {
                    Some(k) => styled[k].2 |= *shows,
                    None => {
                        starts.entry((cx, cy)).or_default().push(styled.len());
                        styled.push((RawPath { segs, ..r.clone() }, style.clone(), *shows));
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
    for (style, template, paths) in groups {
        // Lines left open may make loops together, meeting end to middle.
        let (joined, open): (Vec<_>, Vec<_>) = bridge(chain(paths, JOIN), BRIDGE)
            .into_iter()
            .partition(|(_, closed)| *closed);
        let looped = find_loops(open.into_iter().map(|(segs, _)| segs).collect(), JOIN);
        for (segs, closed) in joined.into_iter().chain(looped) {
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
    let (lettering, letter_boxes) = find_lettering(&plain, &heads);
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
        add_crosses(&mut found, &crosses);
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
    Ok(ImportResult {
        allowance: AllowanceQuestion::for_unmeasured(&pieces, &notes, None),
        pieces,
        scale,
        sizes,
        notes,
        sha256,
    })
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
