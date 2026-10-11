//! Writing the PDF: a cover page with the test squares and a map of the
//! pages, then one page per printed part of the sheet.
//!
//! Geometry is computed in millimetres (f64) and converted to points only
//! as each page is written, relative to that page, so coordinates stay
//! small and exact to well under 0.001 mm.

use std::collections::HashMap;
use std::io::Write as _;

use drapewright_pattern_core::geom::{label_point, segs_of};
use drapewright_pattern_core::notches::notch_marks;
use drapewright_pattern_core::{AllowanceMode, EdgeKind, Id, LineKind, MarkKind};
use drapewright_project::Document;
use drapewright_project::document::Assembly;
use flate2::Compression;
use flate2::write::ZlibEncoder;
use kurbo::{Affine, ParamCurve, PathSeg, Point, Rect, Shape as _, Vec2};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect as PdfRect, Ref, Str, TextStr};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::plan::{PlannedPage, Prepared, PrintError, PrintPlan, prepare, touches};
use super::text;

/// Points per millimetre.
const PT: f64 = 72.0 / 25.4;
/// The sides of the test squares, mm.
const SQUARE_METRIC: f64 = 100.0;
const SQUARE_IMPERIAL: f64 = 101.6;
/// Half the width of the diamonds that match neighbouring pages, mm.
const DIAMOND: f64 = 5.0;
/// Anything this close to a page may show on it, mm.
const REACH: f64 = 6.0;

/// The words on the printed pages, in the user's language. Everything
/// else is drawn from the project.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PdfTexts {
    /// Cover heading, usually the project name.
    pub title: String,
    /// Under the heading: "7 pages of A4, trim and butt".
    pub subtitle: String,
    /// How to print and assemble the pages, a paragraph each.
    pub instructions: Vec<String>,
    /// Labels inside the test squares: "10 cm", "4 in".
    pub square_metric: String,
    pub square_imperial: String,
    pub page_map: String,
    /// Legend for pages left out of the print.
    pub not_printed: String,
    /// Written along fold edges.
    pub place_on_fold: String,
    /// Lines under each piece's name, such as "Cut 2 · Main fabric".
    pub piece_lines: HashMap<Id, Vec<String>>,
}

/// Lines and text on one page, in millimetres from its top-left corner
/// with y down.
struct Canvas {
    content: Content,
    /// Page height, mm.
    height: f64,
}

impl Canvas {
    fn new(height: f64) -> Self {
        Canvas {
            content: Content::new(),
            height,
        }
    }

    fn pt(&self, p: Point) -> (f32, f32) {
        ((p.x * PT) as f32, ((self.height - p.y) * PT) as f32)
    }

    fn move_to(&mut self, p: Point) {
        let (x, y) = self.pt(p);
        self.content.move_to(x, y);
    }

    fn line_to(&mut self, p: Point) {
        let (x, y) = self.pt(p);
        self.content.line_to(x, y);
    }

    fn line(&mut self, a: Point, b: Point) {
        self.move_to(a);
        self.line_to(b);
    }

    fn polyline(&mut self, points: &[Point], closed: bool) {
        let Some((&first, rest)) = points.split_first() else {
            return;
        };
        self.move_to(first);
        for &p in rest {
            self.line_to(p);
        }
        if closed {
            self.content.close_path();
        }
    }

    /// Exact curves: lines and cubic Béziers (quadratics are raised).
    fn path(&mut self, segs: &[PathSeg], closed: bool) {
        let Some(first) = segs.first() else {
            return;
        };
        self.move_to(first.start());
        for seg in segs {
            match *seg {
                PathSeg::Line(l) => self.line_to(l.p1),
                PathSeg::Quad(q) => self.cubic(q.raise().p1, q.raise().p2, q.p2),
                PathSeg::Cubic(c) => self.cubic(c.p1, c.p2, c.p3),
            }
        }
        if closed {
            self.content.close_path();
        }
    }

    fn cubic(&mut self, c1: Point, c2: Point, to: Point) {
        let ((x1, y1), (x2, y2), (x3, y3)) = (self.pt(c1), self.pt(c2), self.pt(to));
        self.content.cubic_to(x1, y1, x2, y2, x3, y3);
    }

    fn rect(&mut self, r: Rect) {
        let (x, y) = self.pt(Point::new(r.x0, r.y1));
        self.content
            .rect(x, y, (r.width() * PT) as f32, (r.height() * PT) as f32);
    }

    fn circle(&mut self, centre: Point, radius: f64) {
        let segs: Vec<PathSeg> = kurbo::Circle::new(centre, radius)
            .path_segments(0.001)
            .collect();
        self.path(&segs, true);
    }

    /// Strokes the current path: width in points, dashes in millimetres
    /// (empty for a solid line), gray from 0 (black) to 1 (white).
    fn stroke(&mut self, width: f64, dash: &[f64], gray: f64) {
        self.content
            .set_line_width(width as f32)
            .set_dash_pattern(dash.iter().map(|d| (d * PT) as f32), 0.0)
            .set_stroke_gray(gray as f32)
            .stroke();
    }

    fn fill(&mut self, gray: f64) {
        self.content.set_fill_gray(gray as f32).fill_nonzero();
    }

    /// Text with its baseline starting at `at`, turned by `angle` (radians,
    /// clockwise on the page).
    fn text(&mut self, s: &str, at: Point, size: f64, bold: bool, gray: f64, angle: f64) {
        let (x, y) = self.pt(at);
        let (sin, cos) = (-angle).sin_cos();
        self.content
            .set_fill_gray(gray as f32)
            .begin_text()
            .set_font(Name(if bold { b"F2" } else { b"F1" }), size as f32)
            .set_text_matrix([cos as f32, sin as f32, -sin as f32, cos as f32, x, y])
            .show(Str(&text::encode(s)))
            .end_text();
    }

    /// Text centred on `centre` (horizontally, and on its x-height).
    fn text_centred(
        &mut self,
        s: &str,
        centre: Point,
        size: f64,
        bold: bool,
        gray: f64,
        angle: f64,
    ) {
        let half = text::width(s, size, bold) / PT / 2.0;
        let lift = size * 0.35 / PT;
        let (dir, down) = (Vec2::from_angle(angle), Vec2::from_angle(angle).turn_90());
        self.text(
            s,
            centre - dir * half + down * lift,
            size,
            bold,
            gray,
            angle,
        );
    }

    fn clip(&mut self, r: Rect) {
        self.rect(r);
        self.content.clip_nonzero().end_path();
    }

    fn finish(self) -> Vec<u8> {
        let raw = self.content.finish();
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(raw.as_slice())
            .expect("compressing into memory");
        z.finish().expect("compressing into memory")
    }
}

/// An arrowhead at `tip`, pointing along `dir` (unit), `size` mm long.
fn arrowhead(c: &mut Canvas, tip: Point, dir: Vec2, size: f64) {
    let back = tip - dir * size;
    let side = dir.turn_90() * (size * 0.4);
    c.move_to(back + side);
    c.line_to(tip);
    c.line_to(back - side);
}

/// Text angle that reads left to right (never upside down) along `dir`.
fn readable(dir: Vec2) -> f64 {
    use std::f64::consts::{FRAC_PI_2, PI};
    let a = dir.atan2();
    if (-FRAC_PI_2..=FRAC_PI_2).contains(&a) {
        a
    } else {
        a + PI
    }
}

/// Draws one piece; `tf` takes piece coordinates to page millimetres.
fn draw_piece(c: &mut Canvas, p: &Prepared, tf: Affine, doc: &Document, texts: &PdfTexts) {
    let piece = p.piece;
    let print = &doc.print;
    let map = |pts: &[Point]| pts.iter().map(|q| tf * *q).collect::<Vec<_>>();
    let outline: Vec<PathSeg> = piece
        .outline
        .path_segs()
        .into_iter()
        .map(|s| tf * s)
        .collect();
    let included = piece.allowance.mode == AllowanceMode::Included;

    // The outline is exact; the line derived from it is a fine polyline.
    if print.stitch_line {
        if included {
            c.polyline(&map(&p.stitching), true);
        } else {
            c.path(&outline, true);
        }
        c.stroke(0.5, &[2.0, 1.5], 0.0);
    }
    if included {
        c.path(&outline, true);
    } else {
        c.polyline(&map(&p.cutting), true);
    }
    c.stroke(0.8, &[], 0.0);

    for mark in notch_marks(piece, &p.cutting, print.notch_style, print.notch_spacing) {
        for [x0, y0, x1, y1] in mark.lines {
            c.line(tf * Point::new(x0, y0), tf * Point::new(x1, y1));
        }
    }
    c.stroke(0.8, &[], 0.0);

    if let Some(g) = &piece.grainline {
        let (a, b) = (tf * Point::from(g.from), tf * Point::from(g.to));
        let len = a.distance(b);
        if len > 1e-6 {
            let dir = (b - a) / len;
            let head = (len / 4.0).min(6.0);
            c.line(a, b);
            arrowhead(c, b, dir, head);
            if !g.one_way {
                arrowhead(c, a, -dir, head);
            }
            c.stroke(0.6, &[], 0.0);
        }
    }

    // "Place on fold" brackets, inside each straight fold edge.
    for i in 0..piece.outline.edges.len() {
        if piece.outline.edges[i].kind != EdgeKind::Fold {
            continue;
        }
        let geom = piece.outline.edge_geom(i);
        let (a, b) = (tf * geom.point_at(0.0), tf * geom.point_at(geom.length()));
        let len = a.distance(b);
        if len < 30.0 {
            continue;
        }
        let along = (b - a) / len;
        let k = tf.as_coeffs();
        let n = geom.normal_at(geom.length() / 2.0);
        // Outward normal on the page (the transform is a rotation and move).
        let out = Vec2::new(k[0] * n.x + k[2] * n.y, k[1] * n.x + k[3] * n.y);
        let inward = -out.normalize();
        let depth = 10.0_f64.min(len / 4.0);
        let (p1, p2) = (a + (b - a) * 0.2, a + (b - a) * 0.8);
        let (q1, q2) = (p1 + inward * depth, p2 + inward * depth);
        c.move_to(p1 + inward * 0.8);
        c.line_to(q1);
        c.line_to(q2);
        c.line_to(p2 + inward * 0.8);
        arrowhead(c, p1 + inward * 0.8, -inward, 3.0);
        arrowhead(c, p2 + inward * 0.8, -inward, 3.0);
        c.stroke(0.6, &[], 0.0);
        let angle = readable(along);
        c.text_centred(
            &texts.place_on_fold,
            q1.midpoint(q2) + inward * 3.5,
            8.0,
            false,
            0.0,
            angle,
        );
    }

    for line in &piece.lines {
        let segs: Vec<PathSeg> = segs_of(line.start, &line.segments)
            .into_iter()
            .map(|s| tf * s)
            .collect();
        let closed = segs
            .first()
            .zip(segs.last())
            .is_some_and(|(f, l)| f.start().distance(l.end()) < 1e-6);
        c.path(&segs, closed);
        let dash: &[f64] = match line.kind {
            LineKind::Fold | LineKind::Placement => &[4.0, 1.5, 0.8, 1.5],
            LineKind::Stitch => &[2.0, 1.5],
            LineKind::Line | LineKind::Cut | LineKind::LengthenShorten => &[],
        };
        c.stroke(0.5, dash, 0.0);
    }

    for mark in &piece.marks {
        let at = tf * Point::from(mark.at);
        let size = mark.size;
        match mark.kind {
            MarkKind::Drill => {
                let r = size.map_or(1.5, |s| s / 2.0);
                c.circle(at, r);
                c.line(at - Vec2::new(r * 1.6, 0.0), at + Vec2::new(r * 1.6, 0.0));
                c.line(at - Vec2::new(0.0, r * 1.6), at + Vec2::new(0.0, r * 1.6));
                c.stroke(0.5, &[], 0.0);
            }
            MarkKind::Dot => {
                c.circle(at, size.map_or(1.5, |s| s / 2.0));
                c.fill(0.0);
            }
            MarkKind::Circle | MarkKind::Button => {
                c.circle(at, size.map_or(3.0, |s| s / 2.0));
                c.stroke(0.5, &[], 0.0);
            }
            MarkKind::Cross => {
                let r = size.map_or(3.0, |s| s / 2.0);
                c.line(at + Vec2::new(-r, -r), at + Vec2::new(r, r));
                c.line(at + Vec2::new(-r, r), at + Vec2::new(r, -r));
                c.stroke(0.5, &[], 0.0);
            }
            MarkKind::Buttonhole => {
                let len = mark.length.unwrap_or(20.0);
                let dir = Vec2::from_angle(mark.angle.to_radians());
                let (a, b) = (at - dir * (len / 2.0), at + dir * (len / 2.0));
                c.line(a, b);
                for end in [a, b] {
                    c.line(end - dir.turn_90() * 2.0, end + dir.turn_90() * 2.0);
                }
                c.stroke(0.5, &[], 0.0);
            }
        }
    }

    for label in &piece.labels {
        let size_pt = label.size * PT;
        c.text(
            &label.text,
            tf * Point::from(label.at),
            size_pt,
            false,
            0.0,
            label.angle.to_radians(),
        );
    }

    // Name and cutting instructions where the piece is widest, so they sit
    // inside it even when it is curved or narrow.
    let (inside, _) = label_point(&p.cutting);
    let centre = tf * inside;
    let lines = texts
        .piece_lines
        .get(&piece.id)
        .map_or(&[][..], Vec::as_slice);
    let height = 6.0 + 4.5 * lines.len() as f64;
    let mut y = centre.y - height / 2.0 + 3.0;
    c.text_centred(&piece.name, Point::new(centre.x, y), 12.0, true, 0.0, 0.0);
    for line in lines {
        y += 4.5;
        c.text_centred(line, Point::new(centre.x, y), 9.0, false, 0.0, 0.0);
    }
}

struct Page {
    width: f64,
    height: f64,
    content: Vec<u8>,
}

/// The sheet's pieces (cutting lines) and the pages they need, for the map.
fn draw_map(c: &mut Canvas, plan: &PrintPlan, prepared: &[Prepared], area: Rect, texts: &PdfTexts) {
    let sheet_w = f64::from(plan.columns - 1) * plan.step_x + plan.printable_width;
    let sheet_h = f64::from(plan.rows - 1) * plan.step_y + plan.printable_height;
    let legend = 8.0;
    let scale = (area.width() / sheet_w).min((area.height() - legend) / sheet_h);
    if scale <= 0.0 {
        return;
    }
    let to_map = Affine::translate(area.origin().to_vec2()) * Affine::scale(scale);
    for p in prepared {
        c.polyline(
            &p.sheet_cutting()
                .iter()
                .map(|q| to_map * *q)
                .collect::<Vec<_>>(),
            true,
        );
    }
    c.stroke(0.4, &[], 0.45);
    for page in &plan.pages {
        let r = plan.page_rect(page.row, page.column);
        let r = Rect::from_points(to_map * r.origin(), to_map * Point::new(r.x1, r.y1));
        c.rect(r);
        if page.printed {
            c.stroke(0.6, &[], 0.0);
            c.text_centred(&page.label, r.center(), 9.0, true, 0.0, 0.0);
        } else {
            c.stroke(0.4, &[1.0, 1.0], 0.5);
        }
    }
    if plan.pages.iter().any(|p| !p.printed) {
        let below = to_map * Point::new(0.0, sheet_h);
        c.text(
            &texts.not_printed,
            below + Vec2::new(0.0, 5.0),
            8.0,
            false,
            0.3,
            0.0,
        );
    }
}

fn cover(plan: &PrintPlan, prepared: &[Prepared], doc: &Document, texts: &PdfTexts) -> Page {
    let (w, h) = (plan.paper_width, plan.paper_height);
    let m = plan.margin.max(10.0);
    let mut c = Canvas::new(h);
    let width = w - 2.0 * m;
    let mut y = m + 7.0;
    c.text(&texts.title, Point::new(m, y), 18.0, true, 0.0, 0.0);
    y += 7.0;
    c.text(&texts.subtitle, Point::new(m, y), 10.0, false, 0.3, 0.0);
    y += 4.0;
    for paragraph in &texts.instructions {
        y += 2.5;
        for line in text::wrap(paragraph, 10.0, width * PT, false) {
            y += 4.6;
            c.text(&line, Point::new(m, y), 10.0, false, 0.0, 0.0);
        }
    }
    y += 8.0;
    let free = Rect::new(m, y, w - m, h - m);

    // Test squares: side by side, else one above the other, else nested.
    let squares = if doc.print.test_square {
        let gap = 10.0;
        let (a, b) = (SQUARE_METRIC, SQUARE_IMPERIAL);
        let (metric, imperial) = if a + gap + b <= free.width() {
            (
                Rect::new(m, y, m + a, y + a),
                Rect::new(m + a + gap, y, m + a + gap + b, y + b),
            )
        } else if a + gap + b <= free.height() {
            (
                Rect::new(m, y, m + a, y + a),
                Rect::new(m, y + a + gap, m + b, y + a + gap + b),
            )
        } else {
            (Rect::new(m, y, m + a, y + a), Rect::new(m, y, m + b, y + b))
        };
        for (r, label) in [
            (metric, &texts.square_metric),
            (imperial, &texts.square_imperial),
        ] {
            c.rect(r);
            c.stroke(0.6, &[], 0.0);
            c.text(
                label,
                Point::new(r.x0 + 3.0, r.y1 - 3.0),
                10.0,
                true,
                0.0,
                0.0,
            );
        }
        metric.union(imperial)
    } else {
        Rect::new(m, y, m, y)
    };

    if doc.print.page_map {
        // Beside the squares or below them, whichever gives the bigger map.
        let right = Rect::new(squares.x1 + 12.0, free.y0, free.x1, free.y1);
        let below = Rect::new(free.x0, squares.y1 + 12.0, free.x1, free.y1);
        let sheet_w = f64::from(plan.columns - 1) * plan.step_x + plan.printable_width;
        let sheet_h = f64::from(plan.rows - 1) * plan.step_y + plan.printable_height;
        let fit = |r: Rect| (r.width() / sheet_w).min((r.height() - 14.0) / sheet_h);
        let area = if !doc.print.test_square || fit(below) >= fit(right) {
            below
        } else {
            right
        };
        c.text(
            &texts.page_map,
            Point::new(area.x0, area.y0 + 4.0),
            11.0,
            true,
            0.0,
            0.0,
        );
        let map_area = Rect::new(area.x0, area.y0 + 8.0, area.x1, area.y1);
        draw_map(&mut c, plan, prepared, map_area, texts);
    }
    Page {
        width: w,
        height: h,
        content: c.finish(),
    }
}

fn tile(
    plan: &PrintPlan,
    prepared: &[Prepared],
    page: &PlannedPage,
    doc: &Document,
    texts: &PdfTexts,
) -> Page {
    let (w, h, m) = (plan.paper_width, plan.paper_height, plan.margin);
    let (pw, ph) = (plan.printable_width, plan.printable_height);
    let mut c = Canvas::new(h);
    let view = plan.page_rect(page.row, page.column);
    let frame = Rect::new(m, m, m + pw, m + ph);
    let to_page = Affine::translate((m - view.x0, m - view.y0));
    let printed = |row: i64, column: i64| {
        plan.pages
            .iter()
            .find(|p| i64::from(p.row) == row && i64::from(p.column) == column)
            .filter(|p| p.printed)
    };

    // The page's label, large and faint, underneath everything.
    c.text_centred(&page.label, frame.center(), 96.0, true, 0.88, 0.0);

    c.content.save_state();
    c.clip(frame);
    for p in prepared {
        if touches(&p.sheet_cutting(), view.inflate(REACH, REACH)) {
            draw_piece(&mut c, p, to_page * p.to_sheet, doc, texts);
        }
    }

    // Marks for joining pages, on the sheet so neighbours print matching
    // halves (trimmed) or the same marks (overlapping).
    for other in &plan.pages {
        let right = other.column + 1 < plan.columns
            && printed(i64::from(other.row), i64::from(other.column) + 1).is_some();
        let down = other.row + 1 < plan.rows
            && printed(i64::from(other.row) + 1, i64::from(other.column)).is_some();
        if !other.printed {
            continue;
        }
        let r = plan.page_rect(other.row, other.column);
        for (has, horizontal) in [(right, true), (down, false)] {
            if !has {
                continue;
            }
            match doc.print.assembly {
                Assembly::Trim => {
                    let centre = if horizontal {
                        Point::new(r.x0 + plan.step_x, r.y0 + ph / 2.0)
                    } else {
                        Point::new(r.x0 + pw / 2.0, r.y0 + plan.step_y)
                    };
                    let diamond = [
                        centre + Vec2::new(-DIAMOND, 0.0),
                        centre + Vec2::new(0.0, -DIAMOND),
                        centre + Vec2::new(DIAMOND, 0.0),
                        centre + Vec2::new(0.0, DIAMOND),
                    ];
                    let on_page: Vec<Point> = diamond.iter().map(|q| to_page * *q).collect();
                    c.polyline(&on_page, true);
                    c.fill(0.55);
                }
                Assembly::Overlap => {
                    // The neighbour's trimmed edge goes on the dashed line;
                    // the circles in the shared strip line up.
                    let (edge_a, edge_b, centres) = if horizontal {
                        let x = r.x0 + plan.step_x;
                        let band = x + doc.print.overlap / 2.0;
                        (
                            Point::new(x, r.y0),
                            Point::new(x, r.y1),
                            [
                                Point::new(band, r.y0 + ph / 4.0),
                                Point::new(band, r.y0 + ph * 0.75),
                            ],
                        )
                    } else {
                        let y = r.y0 + plan.step_y;
                        let band = y + doc.print.overlap / 2.0;
                        (
                            Point::new(r.x0, y),
                            Point::new(r.x1, y),
                            [
                                Point::new(r.x0 + pw / 4.0, band),
                                Point::new(r.x0 + pw * 0.75, band),
                            ],
                        )
                    };
                    c.line(to_page * edge_a, to_page * edge_b);
                    c.stroke(0.5, &[3.0, 2.0], 0.3);
                    let radius = (doc.print.overlap / 3.0).min(4.0);
                    for centre in centres {
                        let q = to_page * centre;
                        c.circle(q, radius);
                        c.line(
                            q - Vec2::new(radius * 1.5, 0.0),
                            q + Vec2::new(radius * 1.5, 0.0),
                        );
                        c.line(
                            q - Vec2::new(0.0, radius * 1.5),
                            q + Vec2::new(0.0, radius * 1.5),
                        );
                    }
                    c.stroke(0.5, &[], 0.0);
                }
            }
        }
    }

    // Neighbours' labels just inside each edge that joins another page.
    let (row, column) = (i64::from(page.row), i64::from(page.column));
    let near = [
        (
            row,
            column - 1,
            Point::new(frame.x0 + 3.0, frame.center().y + 9.0),
            false,
        ),
        (
            row,
            column + 1,
            Point::new(frame.x1 - 3.0, frame.center().y + 9.0),
            true,
        ),
        (
            row - 1,
            column,
            Point::new(frame.center().x + 9.0, frame.y0 + 5.0),
            false,
        ),
        (
            row + 1,
            column,
            Point::new(frame.center().x + 9.0, frame.y1 - 3.0),
            false,
        ),
    ];
    for (r, col, at, from_right) in near {
        if let Some(other) = printed(r, col) {
            let x = if from_right {
                at.x - text::width(&other.label, 9.0, true) / PT
            } else {
                at.x
            };
            c.text(&other.label, Point::new(x, at.y), 9.0, true, 0.35, 0.0);
        }
    }
    c.content.restore_state();

    // The frame: the line to trim along.
    c.rect(frame);
    c.stroke(0.4, &[], 0.0);
    if m >= 6.0 {
        c.text(&page.label, Point::new(m, m - 2.0), 9.0, true, 0.0, 0.0);
        let footer = format!("{} · {}", texts.title, page.label);
        c.text(&footer, Point::new(m, h - m + 4.5), 7.0, false, 0.4, 0.0);
    }
    Page {
        width: w,
        height: h,
        content: c.finish(),
    }
}

/// The project as a PDF of tiled pages that print at true size.
pub fn export_pdf(doc: &Document, texts: &PdfTexts) -> Result<Vec<u8>, PrintError> {
    let (plan, prepared) = prepare(doc)?;
    let mut pages = Vec::new();
    if plan.cover {
        pages.push(cover(&plan, &prepared, doc, texts));
    }
    for page in plan.pages.iter().filter(|p| p.printed) {
        pages.push(tile(&plan, &prepared, page, doc, texts));
    }

    let mut pdf = Pdf::new();
    let mut next = 0;
    let mut alloc = || {
        next += 1;
        Ref::new(next)
    };
    let (catalog, tree, regular, bold, info) = (alloc(), alloc(), alloc(), alloc(), alloc());
    let refs: Vec<(Ref, Ref)> = pages.iter().map(|_| (alloc(), alloc())).collect();

    let mut cat = pdf.catalog(catalog);
    cat.pages(tree);
    let mut prefs = cat.viewer_preferences();
    // Print at 100 %, one-sided: readers that honour this skip "fit to page".
    prefs.pair(Name(b"PrintScaling"), Name(b"None"));
    prefs.pair(Name(b"Duplex"), Name(b"Simplex"));
    prefs.finish();
    cat.finish();
    pdf.pages(tree)
        .kids(refs.iter().map(|r| r.0))
        .count(i32::try_from(pages.len()).unwrap_or(i32::MAX));
    for (page, (page_ref, content_ref)) in pages.iter().zip(&refs) {
        let mut p = pdf.page(*page_ref);
        p.media_box(PdfRect::new(
            0.0,
            0.0,
            (page.width * PT) as f32,
            (page.height * PT) as f32,
        ));
        p.parent(tree);
        p.contents(*content_ref);
        p.resources()
            .fonts()
            .pair(Name(b"F1"), regular)
            .pair(Name(b"F2"), bold);
        p.finish();
        pdf.stream(*content_ref, &page.content)
            .filter(Filter::FlateDecode);
    }
    pdf.type1_font(regular)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.type1_font(bold)
        .base_font(Name(b"Helvetica-Bold"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));
    pdf.document_info(info)
        .title(TextStr(&texts.title))
        .producer(TextStr("Drapewright"));
    Ok(pdf.finish())
}
