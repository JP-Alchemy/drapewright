//! Writing pieces as SVG at true size (millimetres), readable by Inkscape,
//! Illustrator and browsers, and by Drapewright's own importer.

use std::fmt::Write as _;

use drapewright_pattern_core::allowance::allowance_lines;
use drapewright_pattern_core::{Piece, Segment};
use kurbo::{Affine, Point, Vec2};

#[derive(Clone, Debug)]
pub struct SvgExportOptions {
    pub cutting_line: bool,
    pub stitching_line: bool,
    pub grainline: bool,
    pub internal_lines: bool,
    pub names: bool,
    /// Place pieces where they sit on the work table (otherwise at their
    /// own coordinates).
    pub use_placement: bool,
}

impl Default for SvgExportOptions {
    fn default() -> Self {
        SvgExportOptions {
            cutting_line: true,
            stitching_line: true,
            grainline: true,
            internal_lines: true,
            names: true,
            use_placement: true,
        }
    }
}

/// A number with at most four decimals and no trailing zeros.
fn num(v: f64) -> String {
    let s = format!("{:.4}", if v.abs() < 5e-5 { 0.0 } else { v });
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn pt(d: &mut String, cmd: char, p: Point) {
    let _ = write!(d, "{cmd}{} {}", num(p.x), num(p.y));
}

fn segments_d(start: Point, segments: &[Segment], affine: Affine, close: bool) -> String {
    let mut d = String::new();
    pt(&mut d, 'M', affine * start);
    for s in segments {
        match s {
            Segment::Line(l) => pt(&mut d, 'L', affine * Point::from(l.to)),
            Segment::Cubic(c) => {
                let (a, b, e) = (
                    affine * Point::from(c.c1),
                    affine * Point::from(c.c2),
                    affine * Point::from(c.to),
                );
                let _ = write!(
                    d,
                    "C{} {} {} {} {} {}",
                    num(a.x),
                    num(a.y),
                    num(b.x),
                    num(b.y),
                    num(e.x),
                    num(e.y)
                );
            }
        }
    }
    if close {
        d.push('Z');
    }
    d
}

fn polygon_d(points: &[Point], affine: Affine) -> String {
    let mut d = String::new();
    for (i, p) in points.iter().enumerate() {
        pt(&mut d, if i == 0 { 'M' } else { 'L' }, affine * *p);
    }
    d.push('Z');
    d
}

fn placement_affine(piece: &Piece) -> Affine {
    let p = piece.placement;
    Affine::translate((p.x, p.y)) * Affine::rotate(p.rotation.to_radians())
}

/// Writes the pieces as one SVG document sized in millimetres.
pub fn export_svg(pieces: &[Piece], options: &SvgExportOptions) -> String {
    struct Prepared<'a> {
        piece: &'a Piece,
        affine: Affine,
        cutting: Vec<Point>,
        stitching: Vec<Point>,
    }
    let prepared: Vec<Prepared> = pieces
        .iter()
        .map(|piece| {
            let lines = allowance_lines(piece);
            let affine = if options.use_placement {
                placement_affine(piece)
            } else {
                Affine::IDENTITY
            };
            Prepared {
                piece,
                affine,
                cutting: lines.cutting,
                stitching: lines.stitching,
            }
        })
        .collect();

    let margin = 10.0;
    let (mut min, mut max) = (
        Point::new(f64::MAX, f64::MAX),
        Point::new(f64::MIN, f64::MIN),
    );
    for p in &prepared {
        for q in &p.cutting {
            let q = p.affine * *q;
            min = Point::new(min.x.min(q.x), min.y.min(q.y));
            max = Point::new(max.x.max(q.x), max.y.max(q.y));
        }
    }
    if prepared.is_empty() {
        (min, max) = (Point::ZERO, Point::new(100.0, 100.0));
    }
    let (x, y) = (min.x - margin, min.y - margin);
    let (w, h) = (max.x - min.x + 2.0 * margin, max.y - min.y + 2.0 * margin);

    let mut svg = String::new();
    let _ = writeln!(svg, r#"<?xml version="1.0" encoding="UTF-8"?>"#);
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="{}mm" height="{}mm" viewBox="{} {} {} {}">"#,
        num(w),
        num(h),
        num(x),
        num(y),
        num(w),
        num(h)
    );
    let _ = writeln!(
        svg,
        "<!-- Made with Drapewright. Sizes are in millimetres; print at 100%. -->"
    );
    for p in &prepared {
        let id = escape(&p.piece.id);
        let name = escape(&p.piece.name);
        let _ = writeln!(
            svg,
            r#"<g id="{id}" inkscape:label="{name}" inkscape:groupmode="layer">"#
        );
        if options.cutting_line {
            let _ = writeln!(
                svg,
                r##"<path id="{id}-cutting-line" inkscape:label="{name} (cutting line)" d="{}" fill="none" stroke="#000" stroke-width="0.5"/>"##,
                polygon_d(&p.cutting, p.affine)
            );
        }
        if options.stitching_line {
            let o = &p.piece.outline;
            let d = if p.piece.allowance.mode == drapewright_pattern_core::AllowanceMode::Add {
                // The outline is the stitching line: keep its exact curves.
                let segs: Vec<Segment> = o
                    .edges
                    .iter()
                    .flat_map(|e| e.segments.iter().copied())
                    .collect();
                segments_d(Point::from(o.start), &segs, p.affine, true)
            } else {
                polygon_d(&p.stitching, p.affine)
            };
            let _ = writeln!(
                svg,
                r##"<path id="{id}-stitching-line" inkscape:label="{name}" d="{d}" fill="none" stroke="#000" stroke-width="0.3" stroke-dasharray="3 2"/>"##
            );
        }
        if options.internal_lines {
            for line in &p.piece.lines {
                let _ = writeln!(
                    svg,
                    r##"<path id="{}" d="{}" fill="none" stroke="#000" stroke-width="0.3"/>"##,
                    escape(&format!("{}-{}", p.piece.id, line.id)),
                    segments_d(Point::from(line.start), &line.segments, p.affine, false)
                );
            }
        }
        if options.grainline
            && let Some(g) = p.piece.grainline
        {
            let (a, b) = (p.affine * Point::from(g.from), p.affine * Point::from(g.to));
            let mut d = String::new();
            pt(&mut d, 'M', a);
            pt(&mut d, 'L', b);
            let _ = writeln!(
                svg,
                r##"<path id="{id}-grainline" d="{d}" fill="none" stroke="#000" stroke-width="0.4"/>"##
            );
            let dir = (b - a).normalize();
            let head = |tip: Point, back: Vec2| {
                let side = back.turn_90() * 2.0;
                polygon_d(
                    &[tip, tip + back * 6.0 + side, tip + back * 6.0 - side],
                    Affine::IDENTITY,
                )
            };
            let mut arrows = head(b, -dir);
            if !g.one_way {
                arrows.push_str(&head(a, dir));
            }
            let _ = writeln!(
                svg,
                r##"<path id="{id}-grainline-arrows" d="{arrows}" fill="#000"/>"##
            );
        }
        if options.names {
            let b = p
                .cutting
                .iter()
                .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |r, q| {
                    let q = p.affine * *q;
                    (r.0.min(q.x), r.1.min(q.y), r.2.max(q.x), r.3.max(q.y))
                });
            let c = Point::new((b.0 + b.2) / 2.0, (b.1 + b.3) / 2.0);
            let _ = writeln!(
                svg,
                r##"<text x="{}" y="{}" font-family="Helvetica, Arial, sans-serif" font-size="8" text-anchor="middle">{name}</text>"##,
                num(c.x),
                num(c.y)
            );
        }
        let _ = writeln!(svg, "</g>");
    }
    let _ = writeln!(svg, "</svg>");
    svg
}

#[cfg(test)]
mod tests {
    use super::num;

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(3.0), "3");
        assert_eq!(num(-0.00001), "0");
        assert_eq!(num(1.23456), "1.2346");
    }
}
