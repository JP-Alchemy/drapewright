//! Seam allowance: deriving the cutting line from the stitching line (mode
//! `add`) or the stitching line from the cutting line (mode `included`).
//!
//! The allowance region is built from simple convex pieces: a rectangle for
//! every flattened segment of every edge, a round wedge wherever the edge
//! kinks outwards, and one polygon per corner shaped by the corner style.
//! Their union (with a robust polygon engine) is the cut piece. Building
//! from pieces instead of offsetting a curve and repairing its loops means
//! tight curves, sharp corners and edges with different allowances need no
//! special cases. The price is a polyline result, kept within
//! [`FLATTEN_TOLERANCE`] of the exact curves.

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use kurbo::{Point, Vec2};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::geom::outward_normal;
use crate::model::{AllowanceMode, CornerStyle, EdgeKind, Id, Piece};

/// Largest distance between a curve and the polyline standing in for it, mm.
pub const FLATTEN_TOLERANCE: f64 = 0.002;

/// A pointed corner may reach at most this many times the larger allowance
/// from the corner before it is cut square across.
pub const MITRE_LIMIT: f64 = 4.0;

/// Below this (sine of the turn angle) a corner counts as straight.
const STRAIGHT: f64 = 1.7e-3; // about 0.1°

type Contour = Vec<[f64; 2]>;

/// Something about the allowance worth telling the user.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    tag = "kind",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AllowanceNote {
    /// The requested corner shape is not possible at this corner (for
    /// example a squared corner at a wide angle); a pointed corner was used.
    CornerFallback { edge: Id },
    /// Allowances from different edges meet and enclose a gap; the gap is
    /// treated as part of the piece.
    EnclosedGap,
    /// The allowance is wider than the piece, so no stitching line remains.
    NothingLeft,
}

/// The derived lines of a piece, as closed polylines running clockwise on
/// screen (the first point is not repeated at the end).
#[derive(Clone, Debug, Default)]
pub struct AllowanceLines {
    pub cutting: Vec<Point>,
    pub stitching: Vec<Point>,
    pub notes: Vec<AllowanceNote>,
}

/// One edge, prepared for building allowance pieces.
struct EdgeInfo {
    id: Id,
    kind: EdgeKind,
    width: f64,
    points: Vec<Point>,
    start_tangent: Vec2,
    end_tangent: Vec2,
    end_corner: CornerStyle,
}

fn edge_infos(piece: &Piece) -> Vec<EdgeInfo> {
    let outline = &piece.outline;
    (0..outline.edges.len())
        .map(|i| {
            let geom = outline.edge_geom(i);
            let edge = &outline.edges[i];
            EdgeInfo {
                id: edge.id.clone(),
                kind: edge.kind,
                width: piece.edge_allowance(i).max(0.0),
                points: geom.flatten(FLATTEN_TOLERANCE),
                start_tangent: geom.start_tangent(),
                end_tangent: geom.end_tangent(),
                end_corner: edge.end_corner,
            }
        })
        .collect()
}

/// Computes the cutting and stitching lines of a piece.
pub fn allowance_lines(piece: &Piece) -> AllowanceLines {
    let edges = edge_infos(piece);
    let outline = outline_polygon(&edges);
    if outline.len() < 3 {
        return AllowanceLines::default();
    }
    match piece.allowance.mode {
        AllowanceMode::Add => grow(&edges, outline),
        AllowanceMode::Included => shrink(&edges, outline),
    }
}

fn outline_polygon(edges: &[EdgeInfo]) -> Vec<Point> {
    let mut pts: Vec<Point> = Vec::new();
    for e in edges {
        for &p in &e.points {
            if pts.last().is_none_or(|q: &Point| q.distance(p) > 1e-9) {
                pts.push(p);
            }
        }
    }
    if pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) <= 1e-9 {
        pts.pop();
    }
    pts
}

fn to_contour(points: &[Point]) -> Contour {
    points.iter().map(|p| [p.x, p.y]).collect()
}

fn signed_area(c: &Contour) -> f64 {
    let n = c.len();
    (0..n)
        .map(|i| {
            let (a, b) = (c[i], c[(i + 1) % n]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        * 0.5
}

/// Pushes a contour, flipped if needed so that it has positive area.
fn push_positive(out: &mut Vec<Contour>, mut c: Contour) {
    c.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-9);
    if c.len() < 3 {
        return;
    }
    let area = signed_area(&c);
    if area.abs() < 1e-12 {
        return;
    }
    if area < 0.0 {
        c.reverse();
    }
    out.push(c);
}

/// Points on a circular arc around `center` of radius `r`, turning from
/// direction `from` to direction `to` the short way, within the flattening
/// tolerance. Both end points included.
fn arc_points(center: Point, r: f64, from: Vec2, to: Vec2) -> Vec<Point> {
    let angle = from.cross(to).atan2(from.dot(to));
    let max_step = if r > FLATTEN_TOLERANCE {
        2.0 * (1.0 - FLATTEN_TOLERANCE / r).acos()
    } else {
        angle.abs()
    };
    let steps = ((angle.abs() / max_step).ceil() as usize).clamp(1, 256);
    let start = from.atan2();
    (0..=steps)
        .map(|k| {
            let a = start + angle * k as f64 / steps as f64;
            center + Vec2::from_angle(a) * r
        })
        .collect()
}

/// Largest gap between a curve and a chord that replaces it inside a fold.
const FOLD_TOLERANCE: f64 = 0.05;

/// Where a curve bends away from the band more tightly than the band is
/// wide, the band folds over itself and lies entirely inside the result:
/// the cutting line bridges across. Its rectangles then only cost time
/// (each crosses all the others), so those points are thinned out. The
/// sliver between each chord and the curve it replaces is returned as a
/// polygon so the region stays covered.
fn thin_folds(points: &[Point], width: f64, side: f64, slivers: &mut Vec<Contour>) -> Vec<Point> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let in_fold = |k: usize| {
        let (d0, d1) = (points[k] - points[k - 1], points[k + 1] - points[k]);
        let (l0, l1) = (d0.hypot(), d1.hypot());
        if l0 < 1e-12 || l1 < 1e-12 {
            return false;
        }
        let turn = (d0 / l0).cross(d1 / l1);
        // Bending away from the band side, with radius below the width.
        side * turn < 0.0 && 0.5 * (l0 + l1) < width * turn.abs()
    };
    let deviation = |a: Point, b: Point, between: &[Point]| {
        let ab = b - a;
        let len = ab.hypot();
        between
            .iter()
            .map(|p| {
                if len < 1e-12 {
                    p.distance(a)
                } else {
                    ((*p - a).cross(ab) / len).abs()
                }
            })
            .fold(0.0, f64::max)
    };
    let mut out = vec![points[0]];
    let mut anchor = 0;
    for k in 1..n - 1 {
        if in_fold(k)
            && deviation(points[anchor], points[k + 1], &points[anchor + 1..=k]) <= FOLD_TOLERANCE
        {
            continue;
        }
        if k - anchor > 1 {
            push_positive(slivers, to_contour(&points[anchor..=k]));
        }
        out.push(points[k]);
        anchor = k;
    }
    if n - 1 - anchor > 1 {
        push_positive(slivers, to_contour(&points[anchor..]));
    }
    out.push(points[n - 1]);
    out
}

/// Rectangles and kink wedges covering everything within `width` of the
/// polyline on one side: `side = 1.0` outside, `-1.0` inside.
/// `start_normal`/`end_normal` are the exact normals at the edge ends; the
/// slivers between them and the first/last segment are covered too.
fn band(
    points: &[Point],
    width: f64,
    side: f64,
    start_normal: Vec2,
    end_normal: Vec2,
    out: &mut Vec<Contour>,
) {
    if width <= 1e-9 || points.len() < 2 {
        return;
    }
    let thinned = thin_folds(points, width, side, out);
    let points = thinned.as_slice();
    let normals: Vec<Option<Vec2>> = points
        .windows(2)
        .map(|w| {
            let d = w[1] - w[0];
            (d.hypot() > 1e-12).then(|| outward_normal(d.normalize()) * side)
        })
        .collect();
    for (w, n) in points.windows(2).zip(&normals) {
        if let Some(n) = n {
            let off = *n * width;
            push_positive(out, to_contour(&[w[0], w[0] + off, w[1] + off, w[1]]));
        }
    }
    // Wedges where consecutive segments turn so that their rectangles open
    // a gap on the band side.
    let mut prev: Option<Vec2> = Some(start_normal * side);
    for (k, n) in normals.iter().enumerate() {
        if let (Some(a), Some(b)) = (prev, n) {
            wedge(points[k], width, a, *b, side, out);
        }
        if n.is_some() {
            prev = *n;
        }
    }
    if let Some(a) = prev {
        wedge(
            points[points.len() - 1],
            width,
            a,
            end_normal * side,
            side,
            out,
        );
    }
}

/// Fills the gap between two offset directions at a vertex with a pointed
/// join, cut square at the mitre limit, so that a sharp turn inside an
/// edge gets the same pointed allowance as a corner. (Between the short
/// segments of a flattened curve the point is within 0.001 mm of a round
/// join.) Turning the other way, the rectangles overlap and nothing is
/// needed.
fn wedge(at: Point, width: f64, from: Vec2, to: Vec2, side: f64, out: &mut Vec<Contour>) {
    let hairpin = from.dot(to) < -1.0 + 1e-9;
    if side * from.cross(to) <= 1e-12 && !hairpin {
        return;
    }
    let (p, q) = (at + from * width, at + to * width);
    let bisector = from + to;
    let pts = if hairpin || bisector.hypot() < 1e-9 {
        // Turning straight back: a square end beyond the vertex, as far out
        // as the width. `ahead` is the direction of travel into the vertex.
        let ahead = from.turn_90() * side;
        vec![at, p, p + ahead * width, q + ahead * width, q]
    } else {
        let bisector = bisector.normalize();
        // Distance from the vertex to the mitre point.
        let reach = width / bisector.dot(from);
        let limit = MITRE_LIMIT * width;
        if reach <= limit {
            vec![at, p, at + bisector * reach, q]
        } else {
            let k = at + bisector * limit;
            let across = bisector.turn_90();
            let along_p = from.turn_90() * -side;
            let along_q = to.turn_90() * side;
            match (
                intersect(p, along_p, k, across),
                intersect(q, along_q, k, across),
            ) {
                (Some((s, _)), Some((u, _))) => vec![at, p, p + along_p * s, q + along_q * u, q],
                _ => vec![at, p, q],
            }
        }
    };
    push_positive(out, to_contour(&pts));
}

/// Intersection of lines `p + s·d` and `q + u·e`, as `(s, u)`.
fn intersect(p: Point, d: Vec2, q: Point, e: Vec2) -> Option<(f64, f64)> {
    let den = d.cross(e);
    if den.abs() < 1e-12 {
        return None;
    }
    let w = q - p;
    Some((w.cross(e) / den, w.cross(d) / den))
}

/// Reflection of direction `v` in an axis with unit direction `axis`.
fn reflect(v: Vec2, axis: Vec2) -> Vec2 {
    axis * (2.0 * v.dot(axis)) - v
}

fn resolve_style(style: CornerStyle, this: EdgeKind, next: EdgeKind) -> CornerStyle {
    match style {
        CornerStyle::Auto if this == EdgeKind::Fold || next == EdgeKind::Fold => CornerStyle::Mitre,
        CornerStyle::Auto if next == EdgeKind::Hem && this != EdgeKind::Hem => {
            CornerStyle::MirrorNext
        }
        CornerStyle::Auto if this == EdgeKind::Hem && next != EdgeKind::Hem => {
            CornerStyle::MirrorThis
        }
        CornerStyle::Auto => CornerStyle::Mitre,
        other => other,
    }
}

/// The geometry of one outward corner.
struct Corner {
    c: Point,
    ta: Vec2,
    tb: Vec2,
    na: Vec2,
    nb: Vec2,
    a: f64,
    b: f64,
    /// Offset of the corner along the incoming edge's normal (start of its
    /// cutting line at the corner).
    a1: Point,
    /// Offset of the corner along the outgoing edge's normal.
    b1: Point,
}

/// What a corner adds to the allowance region and, for folded-back corners
/// that must be narrower than the band, what it cuts away.
struct CornerPieces {
    add: Vec<Point>,
    cut: Option<Vec<Point>>,
}

impl Corner {
    fn new(c: Point, ta: Vec2, tb: Vec2, a: f64, b: f64) -> Self {
        let (na, nb) = (outward_normal(ta), outward_normal(tb));
        Corner {
            c,
            ta,
            tb,
            na,
            nb,
            a,
            b,
            a1: c + na * a,
            b1: c + nb * b,
        }
    }

    fn reach_limit(&self) -> f64 {
        MITRE_LIMIT * self.a.max(self.b)
    }

    /// Pointed corner, cut square across beyond the mitre limit. Falls back
    /// to a round corner when the cutting lines do not meet ahead.
    fn mitre(&self) -> CornerPieces {
        let Corner {
            c, ta, tb, a1, b1, ..
        } = *self;
        match intersect(a1, ta, b1, tb) {
            Some((s, u)) if s >= -1e-9 && u <= 1e-9 => {
                let m = a1 + ta * s;
                let reach = (m - c).hypot();
                let limit = self.reach_limit();
                if reach <= limit {
                    return CornerPieces {
                        add: vec![c, a1, m, b1],
                        cut: None,
                    };
                }
                let bis = (m - c) / reach;
                let k = c + bis * limit;
                let across = bis.turn_90();
                let ma = intersect(a1, ta, k, across).map(|(s, _)| a1 + ta * s);
                let mb = intersect(b1, tb, k, across).map(|(s, _)| b1 + tb * s);
                match (ma, mb) {
                    (Some(ma), Some(mb)) => CornerPieces {
                        add: vec![c, a1, ma, mb, b1],
                        cut: None,
                    },
                    _ => self.round(),
                }
            }
            // Beside an edge with no allowance (a fold), the other's
            // cutting line runs to that edge's line, a hair behind the
            // corner when it turns past square: no rounding past the fold.
            _ if self.a.min(self.b) <= 1e-9 => CornerPieces {
                add: vec![c, a1, b1],
                cut: None,
            },
            _ => self.round(),
        }
    }

    fn round(&self) -> CornerPieces {
        let r = self.a.max(self.b);
        let mut add = vec![self.c];
        add.extend(arc_points(self.c, r, self.na, self.nb));
        CornerPieces { add, cut: None }
    }

    /// Squared end on the incoming edge's allowance. Only meaningful where
    /// the corner is a right angle or sharper.
    fn square_this(&self) -> Option<CornerPieces> {
        let Corner {
            c,
            ta,
            tb,
            na,
            a,
            a1,
            b1,
            ..
        } = *self;
        if ta.dot(tb) > 1e-9 {
            return None;
        }
        let (u, s) = intersect(b1, tb, c, ta)?;
        if s < -1e-9 || u > 1e-9 {
            return None;
        }
        let p = c + ta * s;
        let q = p + na * a;
        Some(CornerPieces {
            add: vec![c, a1, q, p, b1],
            cut: None,
        })
    }

    /// Squared end on the outgoing edge's allowance.
    fn square_next(&self) -> Option<CornerPieces> {
        let Corner {
            c,
            ta,
            tb,
            nb,
            b,
            a1,
            b1,
            ..
        } = *self;
        if ta.dot(tb) > 1e-9 {
            return None;
        }
        let (s, v) = intersect(a1, ta, c, tb)?;
        if v > 1e-9 || s < -1e-9 {
            return None;
        }
        let p = c + tb * v;
        let q = p + nb * b;
        Some(CornerPieces {
            add: vec![c, a1, p, q, b1],
            cut: None,
        })
    }

    /// The outgoing edge's allowance ends in the mirror image of the
    /// incoming edge's cutting line, reflected in the outgoing edge's
    /// stitching line (e.g. side seam into a hem).
    fn mirror_next(&self) -> Option<CornerPieces> {
        let Corner {
            c,
            ta,
            tb,
            nb,
            a1,
            b1,
            ..
        } = *self;
        // Where the incoming cutting line crosses the outgoing stitching line.
        let (s_p, v_p) = intersect(a1, ta, c, tb)?;
        if v_p > 1e-9 {
            return None;
        }
        let p = c + tb * v_p;
        let dir = reflect(ta, tb);
        let (mu_r, _) = intersect(b1, tb, p, dir)?;
        let r = b1 + tb * mu_r;
        let mut add = vec![c];
        if s_p > 1e-9 {
            add.push(a1);
        }
        add.push(p);
        if mu_r <= 1e-9 {
            add.extend([r, b1]);
            Some(CornerPieces { add, cut: None })
        } else {
            // The folded-back end is narrower than the band: trim the band.
            let (_, x_t) = intersect(p, dir, c, nb)?;
            let x = c + nb * x_t;
            add.push(x);
            Some(CornerPieces {
                add,
                cut: Some(vec![x, b1, r]),
            })
        }
    }

    /// The incoming edge's allowance ends in the mirror image of the
    /// outgoing edge's cutting line, reflected in the incoming edge's
    /// stitching line (e.g. hem into a side seam).
    fn mirror_this(&self) -> Option<CornerPieces> {
        let Corner {
            c,
            ta,
            tb,
            na,
            a1,
            b1,
            ..
        } = *self;
        let (u_p, s_p) = intersect(b1, tb, c, ta)?;
        if s_p < -1e-9 {
            return None;
        }
        let p = c + ta * s_p;
        let dir = reflect(tb, ta);
        let (lam_r, _) = intersect(a1, ta, p, dir)?;
        let r = a1 + ta * lam_r;
        let mut tail = vec![p];
        if u_p < -1e-9 {
            tail.push(b1);
        }
        if lam_r >= -1e-9 {
            let mut add = vec![c, a1, r];
            add.extend(tail);
            Some(CornerPieces { add, cut: None })
        } else {
            let (_, x_t) = intersect(p, dir, c, na)?;
            let x = c + na * x_t;
            let mut add = vec![c, x];
            add.extend(tail);
            Some(CornerPieces {
                add,
                cut: Some(vec![r, a1, x]),
            })
        }
    }

    fn pieces(&self, style: CornerStyle) -> (CornerPieces, bool) {
        let shaped = match style {
            CornerStyle::Auto | CornerStyle::Mitre => return (self.mitre(), false),
            CornerStyle::SquareThis => self.square_this(),
            CornerStyle::SquareNext => self.square_next(),
            CornerStyle::MirrorThis => self.mirror_this(),
            CornerStyle::MirrorNext => self.mirror_next(),
        };
        // Reject constructions that run away from the corner (shallow
        // corners, nearly parallel lines): a pointed corner is the safe
        // answer. Trimming is held to the reach of a pointed corner so it
        // can never eat into an edge's allowance away from the corner.
        let within = |pts: &[Point], limit: f64| {
            pts.iter()
                .all(|p| p.is_finite() && (*p - self.c).hypot() <= limit + 1e-6)
        };
        match shaped {
            Some(cp)
                if within(&cp.add, 3.0 * self.reach_limit())
                    && cp
                        .cut
                        .as_deref()
                        .is_none_or(|cut| within(cut, self.reach_limit())) =>
            {
                (cp, false)
            }
            _ => (self.mitre(), true),
        }
    }
}

fn grow(edges: &[EdgeInfo], outline: Vec<Point>) -> AllowanceLines {
    let n = edges.len();
    let mut adds: Vec<Contour> = Vec::with_capacity(4 * outline.len());
    let mut cuts: Vec<Contour> = Vec::new();
    let mut notes = Vec::new();
    push_positive(&mut adds, to_contour(&outline));

    for e in edges {
        band(
            &e.points,
            e.width,
            1.0,
            outward_normal(e.start_tangent),
            outward_normal(e.end_tangent),
            &mut adds,
        );
    }

    for i in 0..n {
        let (ea, eb) = (&edges[i], &edges[(i + 1) % n]);
        if ea.width <= 1e-9 && eb.width <= 1e-9 {
            continue;
        }
        let (ta, tb) = (ea.end_tangent, eb.start_tangent);
        if ta == Vec2::ZERO || tb == Vec2::ZERO || ta.cross(tb) <= STRAIGHT {
            // Inward or straight corners need nothing: the bands overlap.
            continue;
        }
        let Some(&c) = ea.points.last() else { continue };
        let corner = Corner::new(c, ta, tb, ea.width, eb.width);
        let style = resolve_style(ea.end_corner, ea.kind, eb.kind);
        let (pieces, fell_back) = corner.pieces(style);
        if fell_back {
            notes.push(AllowanceNote::CornerFallback {
                edge: ea.id.clone(),
            });
        }
        push_positive(&mut adds, to_contour(&pieces.add));
        if let Some(cut) = pieces.cut {
            push_positive(&mut cuts, to_contour(&cut));
        }
    }

    let mut shapes = adds.simplify_shape(FillRule::NonZero);
    if !cuts.is_empty() {
        shapes = shapes.overlay(&cuts, OverlayRule::Difference, FillRule::NonZero);
    }
    let Some((cutting, holes)) = outer_contour(shapes) else {
        return AllowanceLines {
            cutting: outline.clone(),
            stitching: outline,
            notes,
        };
    };
    if holes {
        notes.push(AllowanceNote::EnclosedGap);
    }
    AllowanceLines {
        cutting,
        stitching: outline,
        notes,
    }
}

fn shrink(edges: &[EdgeInfo], outline: Vec<Point>) -> AllowanceLines {
    let n = edges.len();
    let mut bands: Vec<Contour> = Vec::new();
    for e in edges {
        band(
            &e.points,
            e.width,
            -1.0,
            outward_normal(e.start_tangent),
            outward_normal(e.end_tangent),
            &mut bands,
        );
    }
    // Where the cutting line turns inward, the inner offsets open a gap
    // that must be closed with a pointed corner, as with any stitching line.
    for i in 0..n {
        let (ea, eb) = (&edges[i], &edges[(i + 1) % n]);
        let (ta, tb) = (ea.end_tangent, eb.start_tangent);
        if ta == Vec2::ZERO || tb == Vec2::ZERO || ta.cross(tb) >= -STRAIGHT {
            continue;
        }
        let Some(&c) = ea.points.last() else { continue };
        let (na, nb) = (-outward_normal(ta), -outward_normal(tb));
        let (a1, b1) = (c + na * ea.width, c + nb * eb.width);
        let piece = match intersect(a1, ta, b1, tb) {
            Some((s, u))
                if s >= -1e-9
                    && u <= 1e-9
                    && (a1 + ta * s - c).hypot() <= MITRE_LIMIT * ea.width.max(eb.width) =>
            {
                vec![c, a1, a1 + ta * s, b1]
            }
            _ => {
                let mut pts = vec![c];
                pts.extend(arc_points(c, ea.width.max(eb.width), na, nb));
                pts
            }
        };
        push_positive(&mut bands, to_contour(&piece));
    }
    let subject = vec![to_contour(&outline)];
    let shapes = subject.overlay(&bands, OverlayRule::Difference, FillRule::NonZero);
    match outer_contour(shapes) {
        Some((stitching, _)) => AllowanceLines {
            cutting: outline,
            stitching,
            notes: Vec::new(),
        },
        None => AllowanceLines {
            cutting: outline,
            stitching: Vec::new(),
            notes: vec![AllowanceNote::NothingLeft],
        },
    }
}

/// The outer boundary of the largest shape, and whether that shape has
/// holes.
fn outer_contour(shapes: Vec<Vec<Contour>>) -> Option<(Vec<Point>, bool)> {
    let best = shapes
        .into_iter()
        .filter(|s| !s.is_empty())
        .max_by(|a, b| {
            signed_area(&a[0])
                .abs()
                .total_cmp(&signed_area(&b[0]).abs())
        })?;
    let mut outer = best[0].clone();
    if signed_area(&outer) < 0.0 {
        outer.reverse();
    }
    Some((
        outer.into_iter().map(|[x, y]| Point::new(x, y)).collect(),
        best.len() > 1,
    ))
}
