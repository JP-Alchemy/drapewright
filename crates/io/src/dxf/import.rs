//! Pattern pieces from DXF.
//!
//! Garment CAD files in the AAMA/ASTM style keep each piece in a block,
//! with fixed layers: 1 cutting line, 2 corners ("turn points"), 4 notches,
//! 6 mirror line (fold), 7 grainline, 8 internal lines, 13 drill holes,
//! 14 stitching line, and text such as "Piece Name: Front". Other DXF files
//! are read like any drawing: outlines are chained from their lines and
//! arcs, and closed shapes become pieces.
//!
//! Curves stay curves: arcs, circles and polyline bulges become Béziers
//! within 0.001 mm, and splines of degree 3 or less become exact Béziers
//! (one per knot span). Weighted (rational) and higher-degree splines are
//! traced with straight steps within 0.01 mm. A spline stored only as the
//! points it passes through is joined by straight lines and reported.

use drapewright_pattern_core::build::{DEFAULT_CORNER_ANGLE, counter, outline_from_path_at};
use drapewright_pattern_core::geom::{distance_to_polyline, flatten_segs, point_in_polygon};
use drapewright_pattern_core::ops::normalize_orientation;
use drapewright_pattern_core::{
    AllowanceMode, CutInstruction, EdgeKind, Grainline, InternalLine, LineKind, Mark, MarkKind,
    Notch, NotchFrom, NotchKind, Piece, Pt, ScaleBasis, Source, SourceFormat, SourceScale,
};
use kurbo::{
    Affine, Arc, Circle, CubicBez, Line, ParamCurve, PathEl, PathSeg, Point, QuadBez, Shape as _,
    SvgArc, Vec2,
};

use super::reader::{Drawing, Entity, Shape, decode, parse};
use crate::assemble::{
    CLOSE_TOLERANCE, RawPath, assemble, chain, is_straight, meaningful_name, measure_allowances,
    median, to_segments,
};
use crate::import::{
    AllowanceQuestion, ImportError, ImportNote, ImportResult, ImportSizes, PieceSize, ScaleInfo,
    ScaleOption, ScaleReason, sha256_hex,
};

/// Curves become Béziers within this, mm.
const CURVE_TOLERANCE: f64 = 0.01;

/// Turn points and notches this close to the outline count as on it, mm.
const ON_LINE: f64 = 0.5;

fn option(mm_per_unit: f64, basis: ScaleBasis) -> ScaleOption {
    ScaleOption { mm_per_unit, basis }
}

/// Text of every TEXT entity, anywhere in the drawing.
fn all_texts(d: &Drawing) -> impl Iterator<Item = &str> {
    d.entities
        .iter()
        .chain(d.blocks.values().flat_map(|b| &b.entities))
        .filter_map(|e| match &e.shape {
            Shape::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
}

/// Splines that carry only the points they pass through, which are joined
/// by straight lines rather than the curve the drawing program had.
fn traced_splines(d: &Drawing) -> u32 {
    let count = d
        .entities
        .iter()
        .chain(d.blocks.values().flat_map(|b| &b.entities))
        .filter(|e| match &e.shape {
            Shape::Spline {
                degree,
                knots,
                control,
                ..
            } => !has_control(*degree, knots, control),
            _ => false,
        })
        .count();
    u32::try_from(count).unwrap_or(u32::MAX)
}

fn detect_scale(d: &Drawing) -> ScaleInfo {
    let mm = option(1.0, ScaleBasis::AssumedMm);
    let cm = option(10.0, ScaleBasis::AssumedCm);
    let inch = option(25.4, ScaleBasis::AssumedInches);
    let certain = |mm_per_unit: f64| ScaleInfo {
        chosen: option(mm_per_unit, ScaleBasis::DeclaredUnits),
        certain: true,
        reason: ScaleReason::DeclaredUnits,
        alternatives: Vec::new(),
    };
    match d.insunits {
        Some(1) => return certain(25.4),
        Some(2) => return certain(304.8),
        Some(4) => return certain(1.0),
        Some(5) => return certain(10.0),
        Some(6) => return certain(1000.0),
        _ => {}
    }
    let units = all_texts(d).find_map(|t| {
        let lower = t.to_ascii_lowercase();
        lower.strip_prefix("units:").map(|u| u.trim().to_string())
    });
    match units.as_deref() {
        Some("english") => certain(25.4),
        Some("metric") => ScaleInfo {
            chosen: mm,
            certain: false,
            reason: ScaleReason::AamaMetric,
            alternatives: vec![cm, inch],
        },
        _ => {
            let (chosen, alternatives) = if d.measurement == Some(0) {
                (inch, vec![mm, cm])
            } else {
                (mm, vec![cm, inch])
            };
            ScaleInfo {
                chosen,
                certain: false,
                reason: ScaleReason::NoDrawingUnits,
                alternatives,
            }
        }
    }
}

fn is_aama(d: &Drawing) -> bool {
    all_texts(d).any(|t| {
        let lower = t.to_ascii_lowercase();
        lower.starts_with("ansi/aama")
            || lower.starts_with("astm")
            || lower.starts_with("piece name:")
    })
}

/// Arc segments for a polyline bulge from `a` to `b` (drawing units, y up).
fn bulge_arc(a: Point, b: Point, bulge: f64) -> Vec<PathSeg> {
    let chord = a.distance(b);
    let radius = chord * (1.0 + bulge * bulge) / (4.0 * bulge.abs());
    let svg = SvgArc {
        from: a,
        to: b,
        radii: Vec2::new(radius, radius),
        x_rotation: 0.0,
        large_arc: bulge.abs() > 1.0,
        sweep: bulge > 0.0,
    };
    match Arc::from_svg_arc(&svg) {
        Some(arc) => arc_segs(a, &arc),
        None => vec![PathSeg::Line(Line::new(a, b))],
    }
}

fn arc_segs(start: Point, arc: &Arc) -> Vec<PathSeg> {
    let mut from = start;
    arc.append_iter(CURVE_TOLERANCE * 0.1)
        .filter_map(|el| match el {
            PathEl::CurveTo(c1, c2, to) => {
                let seg = PathSeg::Cubic(CubicBez::new(from, c1, c2, to));
                from = to;
                Some(seg)
            }
            _ => None,
        })
        .collect()
}

/// Whether a spline's control points and knots make a B-spline that can
/// be drawn. Splines without them carry only fit points.
fn has_control(degree: usize, knots: &[f64], control: &[Point]) -> bool {
    degree >= 1 && control.len() > degree && knots.len() == control.len() + degree + 1
}

/// A point of a (possibly rational) B-spline by de Boor's algorithm, with
/// its own parameter at each level (`args[r - 1]` at level `r`). With every
/// argument equal this is the point at that parameter; with distinct ones it
/// is the spline's blossom, which gives the exact Bézier control points of a
/// knot span. `k` is the span: `knots[k] <= t < knots[k + 1]`.
fn de_boor(
    degree: usize,
    knots: &[f64],
    control: &[Point],
    weights: &[f64],
    k: usize,
    args: &[f64],
) -> Point {
    let w = |i: usize| weights.get(i).copied().unwrap_or(1.0);
    // Homogeneous coordinates (x·w, y·w, w).
    let mut d: Vec<[f64; 3]> = (0..=degree)
        .map(|j| {
            let i = j + k - degree;
            [control[i].x * w(i), control[i].y * w(i), w(i)]
        })
        .collect();
    for r in 1..=degree {
        let t = args[r - 1];
        for j in (r..=degree).rev() {
            let i = j + k - degree;
            let denom = knots[i + degree + 1 - r] - knots[i];
            let a = if denom.abs() < 1e-12 {
                0.0
            } else {
                (t - knots[i]) / denom
            };
            let (p, q) = (d[j - 1], d[j]);
            d[j] = [0, 1, 2].map(|c| p[c] + (q[c] - p[c]) * a);
        }
    }
    let [x, y, w] = d[degree];
    Point::new(x / w, y / w)
}

/// A B-spline of degree 1 to 3 without weights as exact Bézier segments,
/// one per knot span.
fn spline_beziers(degree: usize, knots: &[f64], control: &[Point]) -> Vec<PathSeg> {
    let mut segs = Vec::new();
    for k in degree..control.len() {
        let (a, b) = (knots[k], knots[k + 1]);
        if b - a <= 1e-12 {
            continue;
        }
        // Control point j of the span is the blossom at (a × (degree − j), b × j).
        let pts: Vec<Point> = (0..=degree)
            .map(|j| {
                let args: Vec<f64> = (0..degree)
                    .map(|r| if r < degree - j { a } else { b })
                    .collect();
                de_boor(degree, knots, control, &[], k, &args)
            })
            .collect();
        segs.push(match degree {
            1 => PathSeg::Line(Line::new(pts[0], pts[1])),
            2 => PathSeg::Quad(QuadBez::new(pts[0], pts[1], pts[2])),
            _ => PathSeg::Cubic(CubicBez::new(pts[0], pts[1], pts[2], pts[3])),
        });
    }
    segs
}

/// Points along any B-spline, close enough that the straight lines between
/// them stray less than `tolerance` (drawing units) from the curve. For the
/// splines that are not exact Béziers: rational ones and degree above 3.
fn spline_samples(
    degree: usize,
    knots: &[f64],
    control: &[Point],
    weights: &[f64],
    tolerance: f64,
) -> Vec<Point> {
    let at = |k: usize, t: f64| de_boor(degree, knots, control, weights, k, &vec![t; degree]);
    let mut out = Vec::new();
    for k in degree..control.len() {
        let (a, b) = (knots[k], knots[k + 1]);
        if b - a <= 1e-12 {
            continue;
        }
        if out.is_empty() {
            out.push(at(k, a));
        }
        // Eight steps across the span, each halved until the curve's
        // midpoint lies within the tolerance of the chord. Popped in order.
        let mut stack: Vec<(f64, f64, u32)> = (0..8)
            .rev()
            .map(|i| {
                let step = (b - a) / 8.0;
                (a + step * f64::from(i), a + step * f64::from(i + 1), 0)
            })
            .collect();
        while let Some((t0, t1, depth)) = stack.pop() {
            let tm = (t0 + t1) / 2.0;
            let (p0, p1) = (at(k, t0), at(k, t1));
            let off = distance_to_polyline(at(k, tm), &[p0, p1], false);
            if off > tolerance && depth < 20 {
                stack.push((tm, t1, depth + 1));
                stack.push((t0, tm, depth + 1));
            } else {
                out.push(p1);
            }
        }
    }
    out
}

fn polyline_segs(points: &[Point], closed: bool) -> Vec<PathSeg> {
    let n = points.len();
    let count = if closed { n } else { n.saturating_sub(1) };
    (0..count)
        .map(|i| (points[i], points[(i + 1) % n]))
        .filter(|(a, b)| a != b)
        .map(|(a, b)| PathSeg::Line(Line::new(a, b)))
        .collect()
}

/// The outline of a shape in millimetres (y down), if it has one, and
/// whether it is closed.
fn shape_segs(shape: &Shape, tf: Affine) -> Option<(Vec<PathSeg>, bool)> {
    let segs: (Vec<PathSeg>, bool) = match shape {
        Shape::Line { a, b } => (vec![PathSeg::Line(Line::new(*a, *b))], false),
        Shape::Polyline { vertices, closed } => {
            let n = vertices.len();
            let count = if *closed { n } else { n.saturating_sub(1) };
            let mut segs = Vec::new();
            for i in 0..count {
                let ((a, bulge), (b, _)) = (vertices[i], vertices[(i + 1) % n]);
                if a == b {
                    continue;
                }
                if bulge.abs() < 1e-9 {
                    segs.push(PathSeg::Line(Line::new(a, b)));
                } else {
                    segs.extend(bulge_arc(a, b, bulge));
                }
            }
            (segs, *closed)
        }
        Shape::Arc {
            center,
            radius,
            start_deg,
            end_deg,
        } => {
            let sweep = (end_deg - start_deg).rem_euclid(360.0);
            let sweep = if sweep == 0.0 { 360.0 } else { sweep };
            let arc = Arc::new(
                *center,
                Vec2::new(*radius, *radius),
                start_deg.to_radians(),
                sweep.to_radians(),
                0.0,
            );
            let start = *center + Vec2::from_angle(start_deg.to_radians()) * *radius;
            (arc_segs(start, &arc), false)
        }
        Shape::Circle { center, radius } => {
            let mut from = Point::ZERO;
            let segs = Circle::new(*center, *radius)
                .path_elements(CURVE_TOLERANCE * 0.1)
                .filter_map(|el| match el {
                    PathEl::MoveTo(p) => {
                        from = p;
                        None
                    }
                    PathEl::CurveTo(c1, c2, to) => {
                        let seg = PathSeg::Cubic(CubicBez::new(from, c1, c2, to));
                        from = to;
                        Some(seg)
                    }
                    _ => None,
                })
                .collect();
            (segs, true)
        }
        Shape::Spline {
            degree,
            knots,
            control,
            weights,
            fit,
            closed,
        } => {
            let (p, w) = (*degree, weights);
            let mut segs = if !has_control(p, knots, control) {
                // Only the points it passes through: joined by straight
                // lines, and reported (see `traced_splines`).
                polyline_segs(fit, false)
            } else if p <= 3 && w.iter().all(|w| (w - 1.0).abs() < 1e-12) {
                spline_beziers(p, knots, control)
            } else {
                // Drawing units per millimetre, for the tolerance.
                let per_mm = 1.0 / tf.determinant().abs().sqrt().max(1e-12);
                polyline_segs(
                    &spline_samples(p, knots, control, w, CURVE_TOLERANCE * per_mm),
                    false,
                )
            };
            if *closed
                && let (Some(first), Some(last)) = (segs.first(), segs.last())
                && first.start().distance(last.end()) > 1e-9
            {
                segs.push(PathSeg::Line(Line::new(last.end(), first.start())));
            }
            (segs, *closed)
        }
        Shape::Point { .. } | Shape::Text { .. } | Shape::Insert { .. } => return None,
    };
    if segs.0.is_empty() {
        return None;
    }
    let mapped: Vec<PathSeg> = segs.0.into_iter().map(|s| tf * s).collect();
    let closed =
        segs.1 || mapped[0].start().distance(mapped[mapped.len() - 1].end()) <= CLOSE_TOLERANCE;
    Some((mapped, closed))
}

/// Entities with the transforms that place them, blocks expanded.
fn flatten(
    d: &Drawing,
    entities: &[Entity],
    tf: Affine,
    depth: u32,
    out: &mut Vec<(Entity, Affine)>,
) {
    for e in entities {
        if let Shape::Insert {
            block,
            at,
            scale,
            rotation_deg,
        } = &e.shape
        {
            if depth >= 8 {
                continue;
            }
            if let Some(b) = d.blocks.get(block) {
                let place = Affine::translate(at.to_vec2())
                    * Affine::rotate(rotation_deg.to_radians())
                    * Affine::scale_non_uniform(scale.0, scale.1)
                    * Affine::translate(-b.base.to_vec2());
                flatten(d, &b.entities, tf * place, depth + 1, out);
            }
        } else {
            out.push((e.clone(), tf));
        }
    }
}

/// "SKIRT FRONT" → "Skirt front"; other text as it is.
fn sentence_case(s: &str) -> String {
    let s = s.trim();
    if s.chars().any(char::is_lowercase) {
        return s.to_string();
    }
    let lower = s.to_lowercase();
    let mut chars = lower.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

pub fn import_dxf(data: &[u8], file_name: Option<&str>) -> Result<ImportResult, ImportError> {
    if data.starts_with(b"AutoCAD Binary DXF") {
        return Err(ImportError::Unreadable {
            format: "DXF",
            detail: "this is a binary DXF; save it as an ASCII DXF and try again".into(),
        });
    }
    let text = decode(data);
    let drawing = parse(&text).map_err(|detail| ImportError::Unreadable {
        format: "DXF",
        detail,
    })?;
    let scale = detect_scale(&drawing);
    let sha256 = sha256_hex(data);
    // Drawing units to millimetres, y flipped to point down.
    let to_mm = Affine::new([
        scale.chosen.mm_per_unit,
        0.0,
        0.0,
        -scale.chosen.mm_per_unit,
        0.0,
        0.0,
    ]);
    let source = |reference: Option<String>| Source {
        format: SourceFormat::Dxf,
        file: file_name.map(str::to_string),
        sha256: Some(sha256.clone()),
        reference,
        scale: Some(SourceScale {
            mm_per_unit: scale.chosen.mm_per_unit,
            basis: scale.chosen.basis,
            confirmed: scale.certain,
        }),
    };

    let aama = is_aama(&drawing);
    let (pieces, mut notes, piece_sizes) = if aama {
        aama_pieces(&drawing, to_mm, &source)
    } else {
        let (pieces, notes) = generic_pieces(&drawing, to_mm, &scale, file_name, &sha256);
        (pieces, notes, Vec::new())
    };
    if drawing.skipped > 0 {
        notes.push(ImportNote::UnsupportedEntities {
            count: drawing.skipped,
        });
    }
    let traced = traced_splines(&drawing);
    if traced > 0 {
        notes.push(ImportNote::CurvesTraced { count: traced });
    }
    // An AAMA boundary (layer 1) is by definition the cutting line.
    let suggested = aama.then_some(AllowanceMode::Included);
    let allowance = AllowanceQuestion::for_unmeasured(&pieces, &notes, suggested);
    Ok(ImportResult {
        pieces,
        scale,
        allowance,
        sizes: sizes_of(&drawing, piece_sizes),
        notes,
        sha256,
    })
}

/// The sizes the pieces come in, in the file's order, and the sample size
/// from the file's header ("Sample Size: 50").
fn sizes_of(d: &Drawing, pieces: Vec<PieceSize>) -> ImportSizes {
    let mut names: Vec<String> = Vec::new();
    for ps in &pieces {
        if !names.contains(&ps.size) {
            names.push(ps.size.clone());
        }
    }
    let header: Vec<String> = d
        .entities
        .iter()
        .filter_map(|e| match &e.shape {
            Shape::Text { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect();
    ImportSizes {
        names,
        sample: annotation(&header, "Sample Size").map(str::to_string),
        pieces,
    }
}

fn generic_pieces(
    d: &Drawing,
    to_mm: Affine,
    scale: &ScaleInfo,
    file_name: Option<&str>,
    sha256: &str,
) -> (Vec<Piece>, Vec<ImportNote>) {
    let mut placed = Vec::new();
    flatten(d, &d.entities, to_mm, 0, &mut placed);

    // Outlines per layer, chained from their lines and arcs.
    let mut layers: Vec<String> = placed.iter().map(|(e, _)| e.layer.clone()).collect();
    layers.sort();
    layers.dedup();
    let mut raw = Vec::new();
    for layer in &layers {
        let mut closed_paths = Vec::new();
        let mut open_paths = Vec::new();
        for (e, tf) in placed.iter().filter(|(e, _)| &e.layer == layer) {
            if let Some((segs, closed)) = shape_segs(&e.shape, *tf) {
                if closed {
                    closed_paths.push(segs)
                } else {
                    open_paths.push(segs)
                }
            }
        }
        let hint = layer.to_ascii_lowercase();
        let name = meaningful_name(layer);
        for segs in closed_paths {
            raw.push(RawPath {
                segs,
                closed: true,
                filled: false,
                hint: hint.clone(),
                name: name.clone(),
                reference: Some(layer.clone()),
            });
        }
        for (segs, closed) in chain(open_paths, CLOSE_TOLERANCE) {
            raw.push(RawPath {
                segs,
                closed,
                filled: false,
                hint: hint.clone(),
                name: name.clone(),
                reference: Some(layer.clone()),
            });
        }
    }
    let (mut pieces, notes) = assemble(raw, SourceFormat::Dxf, scale, file_name, sha256);

    // Names from text inside a piece, when the layer gave none.
    let texts: Vec<(Point, f64, String)> = placed
        .iter()
        .filter_map(|(e, tf)| match &e.shape {
            Shape::Text { at, height, text } => Some((*tf * *at, *height, text.clone())),
            _ => None,
        })
        .collect();
    for piece in &mut pieces {
        if !piece.name.starts_with("Piece ") {
            continue;
        }
        let poly = flatten_segs(&piece.outline.path_segs(), 0.5);
        if let Some(name) = texts
            .iter()
            .filter(|(at, _, _)| point_in_polygon(*at, &poly))
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .and_then(|(_, _, text)| meaningful_name(text))
        {
            piece.name = sentence_case(&name);
        }
    }
    (pieces, notes)
}

/// One AAMA piece: a block's entities in millimetres.
struct AamaParts {
    boundary: Vec<Vec<PathSeg>>,
    sew: Vec<Vec<PathSeg>>,
    turn_points: Vec<Point>,
    notches: Vec<(Point, Option<Point>)>,
    grain: Option<(Point, Point)>,
    mirror: Vec<(Point, Point)>,
    internal: Vec<(Vec<PathSeg>, bool, LineKind)>,
    drills: Vec<Point>,
    texts: Vec<String>,
}

fn aama_parts(placed: &[(Entity, Affine)]) -> AamaParts {
    let mut parts = AamaParts {
        boundary: Vec::new(),
        sew: Vec::new(),
        turn_points: Vec::new(),
        notches: Vec::new(),
        grain: None,
        mirror: Vec::new(),
        internal: Vec::new(),
        drills: Vec::new(),
        texts: Vec::new(),
    };
    let mut boundary_open = Vec::new();
    let mut sew_open = Vec::new();
    for (e, tf) in placed {
        let layer = e.layer.trim();
        if let Shape::Text { text, .. } = &e.shape {
            parts.texts.push(text.clone());
            continue;
        }
        if let Shape::Point { at } = &e.shape {
            let p = *tf * *at;
            match layer {
                "2" => parts.turn_points.push(p),
                "4" => parts.notches.push((p, None)),
                "13" => parts.drills.push(p),
                _ => {}
            }
            continue;
        }
        let Some((segs, closed)) = shape_segs(&e.shape, *tf) else {
            continue;
        };
        let ends = (segs[0].start(), segs[segs.len() - 1].end());
        match layer {
            "1" => {
                if closed {
                    parts.boundary.push(segs)
                } else {
                    boundary_open.push(segs)
                }
            }
            "14" => {
                if closed {
                    parts.sew.push(segs)
                } else {
                    sew_open.push(segs)
                }
            }
            "4" => parts.notches.push((ends.0, Some(ends.1))),
            "7" => parts.grain = Some(ends),
            "6" => parts.mirror.push(ends),
            "8" => parts.internal.push((segs, closed, LineKind::Line)),
            "9" | "10" => parts.internal.push((segs, closed, LineKind::Placement)),
            "11" => parts.internal.push((segs, closed, LineKind::Cut)),
            _ => {}
        }
    }
    for (segs, closed) in chain(boundary_open, CLOSE_TOLERANCE) {
        if closed {
            parts.boundary.push(segs);
        }
    }
    for (segs, closed) in chain(sew_open, CLOSE_TOLERANCE) {
        if closed {
            parts.sew.push(segs);
        }
    }
    parts
}

fn largest(paths: &[Vec<PathSeg>]) -> Option<&Vec<PathSeg>> {
    paths.iter().max_by(|a, b| {
        let area = |p: &Vec<PathSeg>| {
            drapewright_pattern_core::geom::polygon_area(&flatten_segs(p, 0.5)).abs()
        };
        area(a).total_cmp(&area(b))
    })
}

/// The value of a "Key: value" annotation, case-insensitive key.
fn annotation<'a>(texts: &'a [String], key: &str) -> Option<&'a str> {
    texts.iter().find_map(|t| {
        let (k, v) = t.split_once(':')?;
        k.trim().eq_ignore_ascii_case(key).then(|| v.trim())
    })
}

fn aama_pieces(
    d: &Drawing,
    to_mm: Affine,
    source: &dyn Fn(Option<String>) -> Source,
) -> (Vec<Piece>, Vec<ImportNote>, Vec<PieceSize>) {
    let mut ids = counter("");
    let mut pieces = Vec::new();
    let mut notes = Vec::new();
    let mut sizes = Vec::new();

    // Each inserted block is a piece; with no inserts, each block is.
    let inserts: Vec<(String, Affine)> = d
        .entities
        .iter()
        .filter_map(|e| match &e.shape {
            Shape::Insert {
                block,
                at,
                scale,
                rotation_deg,
            } => {
                let base = d.blocks.get(block)?.base;
                let place = Affine::translate(at.to_vec2())
                    * Affine::rotate(rotation_deg.to_radians())
                    * Affine::scale_non_uniform(scale.0, scale.1)
                    * Affine::translate(-base.to_vec2());
                Some((block.clone(), to_mm * place))
            }
            _ => None,
        })
        .collect();
    let blocks: Vec<(String, Affine)> = if inserts.is_empty() {
        let mut names: Vec<&String> = d.blocks.keys().collect();
        names.sort();
        names.into_iter().map(|n| (n.clone(), to_mm)).collect()
    } else {
        inserts
    };

    for (name, tf) in blocks {
        let Some(block) = d.blocks.get(&name) else {
            continue;
        };
        let mut placed = Vec::new();
        flatten(d, &block.entities, tf, 1, &mut placed);
        let parts = aama_parts(&placed);
        let Some(boundary) = largest(&parts.boundary) else {
            continue;
        };
        let piece_name = annotation(&parts.texts, "Piece Name")
            .map(sentence_case)
            .unwrap_or_else(|| sentence_case(&name));

        // The stitching line is the outline when there is one; otherwise the
        // cutting line is, with its allowance included.
        let sew = largest(&parts.sew);
        let outline_path = sew.unwrap_or(boundary);
        let corner_tolerance = if sew.is_some() { 60.0 } else { ON_LINE };
        let corners: Vec<Point> = if sew.is_some() {
            // Turn points sit on the cutting line: use the nearest joints of the
            // stitching line.
            parts
                .turn_points
                .iter()
                .filter_map(|t| {
                    outline_path
                        .iter()
                        .map(ParamCurve::start)
                        .min_by(|a, b| a.distance(*t).total_cmp(&b.distance(*t)))
                })
                .filter(|p| {
                    parts
                        .turn_points
                        .iter()
                        .any(|t| p.distance(*t) <= corner_tolerance)
                })
                .collect()
        } else {
            parts.turn_points.clone()
        };
        let Some(outline) = outline_from_path_at(
            outline_path,
            &corners,
            ON_LINE,
            DEFAULT_CORNER_ANGLE,
            &mut ids,
        ) else {
            continue;
        };
        let mut piece = Piece::new(ids("p"), piece_name, outline);
        normalize_orientation(&mut piece);
        piece.source = Some(source(Some(name.clone())));

        let cut_line = flatten_segs(boundary, 0.05);
        if sew.is_some() {
            if let Some(widths) = measure_allowances(&piece, &cut_line) {
                let seam = median(widths.clone());
                piece.allowance.seam = seam;
                for (edge, w) in piece.outline.edges.iter_mut().zip(&widths) {
                    if (w - seam).abs() > 0.25 {
                        edge.allowance = Some(*w);
                    }
                }
                notes.push(ImportNote::SeamLineFound {
                    piece: piece.id.clone(),
                    allowance: seam,
                });
            }
        } else {
            // Its width is not in the file: the user is asked on import.
            piece.allowance.mode = AllowanceMode::Included;
        }

        // Folds: outline edges lying along a mirror line.
        for &(a, b) in &parts.mirror {
            let along_line = |p: Point| {
                let ab = b - a;
                ((p - a).cross(ab) / ab.hypot().max(1e-9)).abs() <= ON_LINE
            };
            for i in 0..piece.outline.edges.len() {
                let (s, e) = (
                    Point::from(piece.outline.edge_start(i)),
                    Point::from(piece.outline.edge_end(i)),
                );
                let straight = is_straight(&flatten_segs(piece.outline.edge_geom(i).segs(), 0.05));
                if straight && along_line(s) && along_line(e) {
                    piece.outline.edges[i].kind = EdgeKind::Fold;
                }
            }
        }

        // Notches: placed where they meet the outline.
        let reach = piece.allowance.seam.max(
            piece
                .outline
                .edges
                .iter()
                .filter_map(|e| e.allowance)
                .fold(0.0, f64::max),
        ) + 3.0;
        for (a, b) in &parts.notches {
            // A notch line runs in from the cutting line; use its end nearer the outline.
            let nearest = |p: Point| {
                (0..piece.outline.edges.len())
                    .filter_map(|i| piece.outline.edge_geom(i).nearest(p).map(|n| (i, n)))
                    .min_by(|x, y| x.1.distance.total_cmp(&y.1.distance))
            };
            let candidates = [Some(*a), *b].into_iter().flatten().filter_map(nearest);
            let Some((i, n)) = candidates.min_by(|x, y| x.1.distance.total_cmp(&y.1.distance))
            else {
                continue;
            };
            if n.distance > reach {
                continue;
            }
            let len = piece.outline.edge_geom(i).length();
            let from_end = n.along > len / 2.0;
            piece.notches.push(Notch {
                id: ids("n"),
                edge: piece.outline.edges[i].id.clone(),
                at: if from_end { len - n.along } else { n.along },
                from: if from_end {
                    NotchFrom::End
                } else {
                    NotchFrom::Start
                },
                kind: NotchKind::Single,
                angle: 0.0,
            });
        }

        if let Some((a, b)) = parts.grain {
            piece.grainline = Some(Grainline {
                from: Pt::from(a),
                to: Pt::from(b),
                one_way: false,
            });
        }
        for (segs, _closed, kind) in &parts.internal {
            piece.lines.push(InternalLine {
                id: ids("l"),
                kind: *kind,
                start: Pt::from(segs[0].start()),
                segments: to_segments(segs),
                label: None,
            });
        }
        for p in &parts.drills {
            piece.marks.push(Mark {
                id: ids("m"),
                kind: MarkKind::Drill,
                at: Pt::from(*p),
                size: None,
                length: None,
                angle: 0.0,
                label: None,
            });
        }
        if let Some(q) = annotation(&parts.texts, "Quantity")
            .and_then(|q| q.parse::<u32>().ok())
            .filter(|&q| q > 0)
        {
            piece.cut = vec![CutInstruction {
                quantity: q,
                ..CutInstruction::default()
            }];
        }
        if let Some(material) = annotation(&parts.texts, "Material").filter(|m| !m.is_empty()) {
            for c in &mut piece.cut {
                c.material = material.to_string();
            }
        }
        let mut info = Vec::new();
        if let Some(size) = annotation(&parts.texts, "Size").filter(|s| !s.is_empty()) {
            info.push(format!("Size {size}"));
            sizes.push(PieceSize {
                piece: piece.id.clone(),
                size: size.to_string(),
            });
        }
        if let Some(a) = annotation(&parts.texts, "Annotation") {
            info.push(a.to_string());
        }
        piece.notes = info.join(". ");
        pieces.push(piece);
    }
    if pieces.is_empty() {
        notes.push(ImportNote::NoClosedShapes);
    }
    (pieces, notes, sizes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bulges_make_arcs_through_the_right_point() {
        // Bulge 1 is a half circle; positive turns anticlockwise (y up), so
        // from (0,0) to (10,0) it dips below the chord.
        let segs = bulge_arc(Point::new(0.0, 0.0), Point::new(10.0, 0.0), 1.0);
        let mid = segs
            .iter()
            .map(|s| s.eval(0.5))
            .fold(
                Point::new(0.0, f64::MAX),
                |m, p| if p.y < m.y { p } else { m },
            );
        assert!(
            (mid.y + 5.0).abs() < 0.01 && (mid.x - 5.0).abs() < 0.1,
            "{mid:?}"
        );
    }

    /// Largest gap between a spline evaluated directly and its Béziers.
    fn bezier_error(degree: usize, knots: &[f64], control: &[Point]) -> f64 {
        let segs = spline_beziers(degree, knots, control);
        let spans: Vec<usize> = (degree..control.len())
            .filter(|&k| knots[k + 1] - knots[k] > 1e-12)
            .collect();
        assert_eq!(segs.len(), spans.len());
        let mut worst: f64 = 0.0;
        for (seg, &k) in segs.iter().zip(&spans) {
            for i in 0..=20 {
                let s = f64::from(i) / 20.0;
                let t = knots[k] + (knots[k + 1] - knots[k]) * s;
                let direct = de_boor(degree, knots, control, &[], k, &vec![t; degree]);
                worst = worst.max(direct.distance(seg.eval(s)));
            }
        }
        worst
    }

    #[test]
    fn splines_become_exact_beziers() {
        // A clamped cubic with uneven knots: two spans, two cubics.
        let control = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 40.0),
            Point::new(50.0, 45.0),
            Point::new(80.0, -10.0),
            Point::new(120.0, 30.0),
        ];
        let cubic = [0.0, 0.0, 0.0, 0.0, 0.3, 1.0, 1.0, 1.0, 1.0];
        assert!(bezier_error(3, &cubic, &control) < 1e-9);
        let segs = spline_beziers(3, &cubic, &control);
        assert!(segs[0].start().distance(control[0]) < 1e-9);
        assert!(segs[1].end().distance(control[4]) < 1e-9);
        // Quadratic, and an unclamped (uniform) cubic.
        let quadratic = [0.0, 0.0, 0.0, 0.2, 0.7, 1.0, 1.0, 1.0];
        assert!(bezier_error(2, &quadratic, &control) < 1e-9);
        let uniform = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        assert!(bezier_error(3, &uniform, &control) < 1e-9);
    }

    #[test]
    fn weighted_splines_are_sampled_within_tolerance() {
        // A quarter circle of radius 100 as a rational quadratic.
        let control = [
            Point::new(100.0, 0.0),
            Point::new(100.0, 100.0),
            Point::new(0.0, 100.0),
        ];
        let weights = [1.0, std::f64::consts::FRAC_1_SQRT_2, 1.0];
        let knots = [0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
        let tolerance = 0.01;
        let pts = spline_samples(2, &knots, &control, &weights, tolerance);
        assert!(
            pts.iter()
                .all(|p| (p.to_vec2().hypot() - 100.0).abs() < 1e-9)
        );
        assert!(
            pts[0].distance(control[0]) < 1e-9 && pts[pts.len() - 1].distance(control[2]) < 1e-9
        );
        // Every chord stays within the tolerance of the arc.
        for w in pts.windows(2) {
            let mid = w[0].midpoint(w[1]);
            assert!(100.0 - mid.to_vec2().hypot() <= tolerance + 1e-9);
        }
    }

    #[test]
    fn sentence_case_only_changes_shouting() {
        assert_eq!(sentence_case("SKIRT FRONT"), "Skirt front");
        assert_eq!(sentence_case("Back yoke"), "Back yoke");
    }
}
