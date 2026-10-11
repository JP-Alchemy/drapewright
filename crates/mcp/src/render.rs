//! Pictures of pieces for an AI assistant to look at: each piece filled,
//! with its cutting line, its stitching line dashed, folds in blue, hems in
//! orange, notches, grainline, internal lines and marks, numbered, and
//! optionally with each edge's id beside it.

use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, point_in_polygon, segs_of,
};
use drapewright_pattern_core::{EdgeKind, Piece};
use drapewright_project::PieceView;
use drapewright_project::hit::placement_affine;
use kurbo::{Affine, Point, Vec2};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Paint, Path, PathBuilder, Pixmap, Stroke, StrokeDash,
    Transform,
};

use crate::font;

/// Blank border around the drawing, pixels.
const MARGIN: f64 = 28.0;

const FILLS: [(u8, u8, u8); 8] = [
    (255, 236, 179),
    (200, 230, 201),
    (187, 222, 251),
    (248, 187, 208),
    (225, 190, 231),
    (255, 204, 188),
    (178, 235, 242),
    (220, 237, 200),
];
const INK: (u8, u8, u8) = (33, 33, 33);
const STITCH: (u8, u8, u8) = (110, 110, 110);
const FOLD: (u8, u8, u8) = (25, 118, 210);
const HEM: (u8, u8, u8) = (230, 81, 0);
const GRAIN: (u8, u8, u8) = (46, 125, 50);

/// What to write on the picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Labels {
    /// Each piece's number.
    Pieces,
    /// Each piece's number and each edge's id.
    Edges,
    None,
}

#[derive(Debug)]
pub struct Picture {
    pub png: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Millimetres one pixel stands for.
    pub mm_per_px: f64,
}

fn points(flat: &[f64]) -> Vec<Point> {
    flat.chunks_exact(2)
        .map(|c| Point::new(c[0], c[1]))
        .collect()
}

fn paint((r, g, b): (u8, u8, u8)) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(r, g, b, 255);
    paint.anti_alias = true;
    paint
}

fn stroke(width: f32, dash: Option<&[f32]>) -> Stroke {
    Stroke {
        width,
        line_cap: LineCap::Round,
        line_join: LineJoin::Round,
        dash: dash.and_then(|d| StrokeDash::new(d.to_vec(), 0.0)),
        ..Stroke::default()
    }
}

fn polyline(points: impl IntoIterator<Item = (f32, f32)>, close: bool) -> Option<Path> {
    let mut path = PathBuilder::new();
    let mut points = points.into_iter();
    let (x, y) = points.next()?;
    path.move_to(x, y);
    for (x, y) in points {
        path.line_to(x, y);
    }
    if close {
        path.close();
    }
    path.finish()
}

/// Maps work-table millimetres to pixels.
#[derive(Clone, Copy)]
struct Frame {
    origin: Point,
    scale: f64,
}

impl Frame {
    fn px(&self, p: Point) -> (f32, f32) {
        (
            ((p.x - self.origin.x) * self.scale + MARGIN) as f32,
            ((p.y - self.origin.y) * self.scale + MARGIN) as f32,
        )
    }
}

/// Draws `pieces` (with their views) where they lie on the work table,
/// fitted to at most `max_px` pixels on the longer side. Pieces are
/// numbered from 1 in the order given.
pub fn draw(
    pieces: &[(&Piece, &PieceView)],
    labels: Labels,
    max_px: u32,
) -> Result<Picture, String> {
    let placed: Vec<(Affine, Vec<Point>)> = pieces
        .iter()
        .map(|(piece, view)| {
            let a = placement_affine(piece);
            let cut = points(&view.cutting).into_iter().map(|p| a * p).collect();
            (a, cut)
        })
        .collect();
    let all = placed.iter().flat_map(|(_, cut)| cut.iter().copied());
    let (x0, y0, x1, y1) = all.fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    );
    if x0 > x1 {
        return Err("There is nothing to draw.".into());
    }
    let (w, h) = ((x1 - x0).max(1.0), (y1 - y0).max(1.0));
    let usable = f64::from(max_px.max(200)) - 2.0 * MARGIN;
    let scale = usable / w.max(h);
    let width = (w * scale + 2.0 * MARGIN).ceil() as u32;
    let height = (h * scale + 2.0 * MARGIN).ceil() as u32;
    let mut pixmap = Pixmap::new(width, height).ok_or("The picture would be too large.")?;
    pixmap.fill(Color::WHITE);
    let frame = Frame {
        origin: Point::new(x0, y0),
        scale,
    };
    let none = Transform::identity();

    for (i, ((piece, view), (a, cut))) in pieces.iter().zip(&placed).enumerate() {
        let a = *a;
        let px = |p: Point| frame.px(a * p);
        if let Some(path) = polyline(cut.iter().map(|p| frame.px(*p)), true) {
            pixmap.fill_path(
                &path,
                &paint(FILLS[i % FILLS.len()]),
                FillRule::Winding,
                none,
                None,
            );
        }
        // The stitching line, when it is not the outline drawn solid.
        if view.stitching != view.cutting
            && let Some(path) = polyline(points(&view.stitching).into_iter().map(px), true)
        {
            pixmap.stroke_path(
                &path,
                &paint(STITCH),
                &stroke(1.2, Some(&[6.0, 4.0])),
                none,
                None,
            );
        }
        if let Some(path) = polyline(cut.iter().map(|p| frame.px(*p)), true) {
            pixmap.stroke_path(&path, &paint(INK), &stroke(1.8, None), none, None);
        }
        // Folds and hems along the outline.
        for (e, edge) in piece.outline.edges.iter().enumerate() {
            let (colour, dash): (_, Option<&[f32]>) = match edge.kind {
                EdgeKind::Fold => (FOLD, Some(&[10.0, 4.0, 2.0, 4.0])),
                EdgeKind::Hem => (HEM, None),
                EdgeKind::Seam => continue,
            };
            let geom = piece.outline.edge_geom(e);
            let len = geom.length();
            let n = ((len * scale / 3.0).ceil() as usize).clamp(2, 400);
            let along = (0..=n).map(|k| px(geom.point_at(len * k as f64 / n as f64)));
            if let Some(path) = polyline(along, false) {
                pixmap.stroke_path(&path, &paint(colour), &stroke(3.2, dash), none, None);
            }
        }
        for line in &piece.lines {
            let segs = segs_of(line.start, &line.segments);
            if let Some(path) = polyline(flatten_segs(&segs, 0.25).into_iter().map(px), false) {
                pixmap.stroke_path(&path, &paint(STITCH), &stroke(1.0, None), none, None);
            }
        }
        for mark in &view.notches {
            for l in &mark.lines {
                let ends = [px(Point::new(l[0], l[1])), px(Point::new(l[2], l[3]))];
                if let Some(path) = polyline(ends, false) {
                    pixmap.stroke_path(&path, &paint(INK), &stroke(1.6, None), none, None);
                }
            }
        }
        if let Some(g) = &piece.grainline {
            let (from, to) = (px(Point::from(g.from)), px(Point::from(g.to)));
            if let Some(path) = polyline([from, to], false) {
                pixmap.stroke_path(&path, &paint(GRAIN), &stroke(1.8, None), none, None);
            }
            let mut heads = vec![(to, from)];
            if !g.one_way {
                heads.push((from, to));
            }
            for (tip, tail) in heads {
                if let Some(path) = arrowhead(tip, tail, 9.0) {
                    pixmap.stroke_path(&path, &paint(GRAIN), &stroke(1.8, None), none, None);
                }
            }
        }
        for mark in &piece.marks {
            let (cx, cy) = px(Point::from(mark.at));
            let r = (mark.size.unwrap_or(4.0) / 2.0 * scale).max(2.5) as f32;
            if let Some(path) = PathBuilder::from_circle(cx, cy, r) {
                pixmap.stroke_path(&path, &paint(INK), &stroke(1.2, None), none, None);
            }
        }
    }

    if labels != Labels::None {
        for (i, ((piece, view), (a, cut))) in pieces.iter().zip(&placed).enumerate() {
            if labels == Labels::Edges {
                for edge in &view.edges {
                    // Just outside the cutting line, beside the edge's middle.
                    let outside = if view.outline_is_cutting {
                        0.0
                    } else {
                        edge.allowance
                    };
                    let normal = Vec2::new(edge.middle_normal.x, edge.middle_normal.y);
                    let at = Point::from(edge.middle) + normal * (outside + 11.0 / scale);
                    let (x, y) = frame.px(*a * at);
                    let colour = match edge.kind {
                        EdgeKind::Fold => FOLD,
                        EdgeKind::Hem => HEM,
                        EdgeKind::Seam => INK,
                    };
                    label(&mut pixmap, &edge.id, x, y, 11.0, colour);
                }
            }
            let _ = piece;
            // The number goes where the piece is widest, clear of its edges.
            let (at, room) = inner_point(cut);
            let (cx, cy) = frame.px(at);
            let size = ((room * scale * 0.9) as f32).clamp(12.0, 44.0);
            label(&mut pixmap, &(i + 1).to_string(), cx, cy, size, INK);
        }
    }

    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
    Ok(Picture {
        png,
        width,
        height,
        mm_per_px: 1.0 / scale,
    })
}

/// The point inside `polygon` farthest from its edges, and that distance:
/// where a label sits best. A grid search, refined around the best cell.
fn inner_point(polygon: &[Point]) -> (Point, f64) {
    let (x0, y0, x1, y1) = polygon.iter().fold(
        (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
        |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
    );
    let consider = |best: &mut (Point, f64), p: Point| {
        if point_in_polygon(p, polygon) {
            let d = distance_to_polyline(p, polygon, true);
            if d > best.1 {
                *best = (p, d);
            }
        }
    };
    let mut best = (Point::new((x0 + x1) / 2.0, (y0 + y1) / 2.0), 0.0);
    const N: i32 = 24;
    let (cw, ch) = ((x1 - x0) / f64::from(N), (y1 - y0) / f64::from(N));
    for i in 0..=N {
        for j in 0..=N {
            consider(
                &mut best,
                Point::new(x0 + cw * f64::from(i), y0 + ch * f64::from(j)),
            );
        }
    }
    let coarse = best.0;
    for i in -6..=6 {
        for j in -6..=6 {
            consider(
                &mut best,
                Point::new(
                    coarse.x + cw * f64::from(i) / 6.0,
                    coarse.y + ch * f64::from(j) / 6.0,
                ),
            );
        }
    }
    best
}

/// Two short strokes at `tip`, pointing away from `tail`.
fn arrowhead(tip: (f32, f32), tail: (f32, f32), size: f32) -> Option<Path> {
    let (dx, dy) = (tip.0 - tail.0, tip.1 - tail.1);
    let len = dx.hypot(dy);
    if len < 1e-3 {
        return None;
    }
    let (ux, uy) = (dx / len, dy / len);
    let back = (tip.0 - ux * size, tip.1 - uy * size);
    let side = (-uy * size * 0.5, ux * size * 0.5);
    let mut path = PathBuilder::new();
    path.move_to(back.0 + side.0, back.1 + side.1);
    path.line_to(tip.0, tip.1);
    path.line_to(back.0 - side.0, back.1 - side.1);
    path.finish()
}

/// Text with a white halo, so it reads over lines and fills.
fn label(pixmap: &mut Pixmap, text: &str, x: f32, y: f32, height: f32, colour: (u8, u8, u8)) {
    let Some(path) = font::text_centred(text, x, y, height) else {
        return;
    };
    let line = (height / 8.0).max(1.4);
    let none = Transform::identity();
    pixmap.stroke_path(
        &path,
        &paint((255, 255, 255)),
        &stroke(line + 3.0, None),
        none,
        None,
    );
    pixmap.stroke_path(&path, &paint(colour), &stroke(line, None), none, None);
}
