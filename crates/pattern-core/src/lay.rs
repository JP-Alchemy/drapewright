//! A pattern laid out on fabric to cut, and how much fabric it takes: each
//! piece's cutting line turned so that its grain runs along the fabric
//! (the length of it, parallel to the selvages), the biggest first, each
//! slid in as far along as it fits. On fabric folded in half lengthwise, a
//! piece cut on the fold has its fold edge on the fabric's fold, and a
//! pair is cut at once through both layers.
//!
//! The fabric is worked out in rows across it, each a few millimetres
//! deep: where each piece is in each row, and the spans of each row
//! already taken. A piece fits where none of its rows overlaps a span
//! taken (with a little room between pieces).

use kurbo::{Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Pt;
use crate::allowance::allowance_lines;
use crate::geom::polygon_area;
use crate::model::{EdgeKind, Id, Piece};

/// What to lay out, on what fabric.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayRequest {
    pub pieces: Vec<Piece>,
    /// The pieces cut from this fabric: `main`, `lining` ...
    pub material: String,
    /// The fabric's width, selvage to selvage, mm.
    pub width: f64,
    /// Folded in half lengthwise, selvages together.
    pub folded: bool,
    /// Every piece the same way up (a nap, a pile, a print one way up).
    pub one_way: bool,
}

/// The pieces laid out on the fabric.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Layout {
    /// The fabric's width, mm, and how much of its length it takes.
    pub width: f64,
    pub length: f64,
    pub folded: bool,
    /// How much of the fabric the pieces take, 0 to 1.
    pub used: f64,
    pub placed: Vec<Placed>,
    pub notes: Vec<LayNote>,
}

/// One piece cut: where its cutting line is on the fabric (x along it, y
/// across from the fold, or from a selvage), mm.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Placed {
    pub piece: Id,
    pub name: String,
    /// Its cutting line; a piece cut on the fold of open fabric is laid
    /// out whole, as both halves.
    pub rings: Vec<Vec<Pt>>,
    /// Along its grain, from one end to the other.
    pub grain: [Pt; 2],
    /// Laid face down: the other of a pair.
    pub mirrored: bool,
    /// Cut through both layers of folded fabric (a pair, or a piece on the
    /// fold), or one.
    pub layers: u32,
    pub on_fold: bool,
}

/// What a layout says of its pieces.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum LayNote {
    /// Wider than the fabric (a layer of it, folded): left out.
    TooWide { piece: Id },
    /// Cut through one layer of folded fabric (the top one): one alone, or
    /// the last of an odd number.
    OneLayer { piece: Id },
    /// No grainline: laid along the fabric as it is drawn, up and down.
    NoGrain { piece: Id },
}

/// The fabric is worked out in rows this deep, mm.
const ROW: f64 = 2.5;
/// Room left between pieces, mm.
const GAP: f64 = 2.0;
/// Closer than this is touching, mm.
const EPSILON: f64 = 1e-6;

/// A shape to lay down: its rings, from its top left corner, and its
/// extent along each row from the top.
struct Shape {
    rings: Vec<Vec<Point>>,
    grain: [Point; 2],
    rows: Vec<Option<(f64, f64)>>,
    length: f64,
}

/// A piece to lay out once.
struct Lay<'a> {
    piece: &'a Piece,
    /// Each way it may be laid.
    shapes: Vec<Shape>,
    mirrored: bool,
    layers: u32,
    on_fold: bool,
    area: f64,
}

/// Lays the pieces cut from `request.material` out on its fabric.
pub fn lay(request: &LayRequest) -> Layout {
    let across = if request.folded {
        request.width / 2.0
    } else {
        request.width
    };
    let mut notes = Vec::new();
    let mut copies: Vec<Lay> = Vec::new();
    for piece in &request.pieces {
        let Some(cut) = piece.cut.iter().find(|c| c.material == request.material) else {
            continue;
        };
        let cutting: Vec<Point> = allowance_lines(piece).cutting;
        if cutting.len() < 3 || cut.quantity == 0 {
            continue;
        }
        let fold = piece
            .outline
            .edges
            .iter()
            .position(|e| e.kind == EdgeKind::Fold)
            .map(|i| {
                (
                    Point::from(piece.outline.edge_start(i)),
                    Point::from(piece.outline.edge_end(i)),
                )
            });
        let on_fold = cut.on_fold.unwrap_or(fold.is_some()) && fold.is_some();
        let mirrored = cut.mirrored.unwrap_or(cut.quantity >= 2);
        let grain = match piece.grainline {
            Some(g) => (Point::from(g.from), Point::from(g.to)),
            None => {
                notes.push(LayNote::NoGrain {
                    piece: piece.id.clone(),
                });
                let b = piece.outline.bounds();
                (
                    Point::new(b.center().x, b.y0),
                    Point::new(b.center().x, b.y1),
                )
            }
        };
        let area = polygon_area(&cutting).abs();
        let q = cut.quantity;
        // How many times to lay it, which face down, through how many
        // layers.
        let mut lays: Vec<(bool, u32)> = Vec::new();
        if request.folded {
            if on_fold {
                lays.extend((0..q).map(|_| (false, 2)));
            } else if mirrored {
                lays.extend((0..q / 2).map(|_| (false, 2)));
                if q % 2 == 1 {
                    lays.push((false, 1));
                }
            } else {
                lays.extend((0..q).map(|_| (false, 1)));
            }
            if lays.iter().any(|l| l.1 == 1) {
                notes.push(LayNote::OneLayer {
                    piece: piece.id.clone(),
                });
            }
        } else {
            lays.extend((0..q).map(|k| (mirrored && k % 2 == 1, 1)));
        }
        for (face_down, layers) in lays {
            let rings: Vec<Vec<Point>> = match (on_fold, request.folded, fold) {
                // Open fabric: the piece whole, both halves.
                (true, false, Some((a, b))) => vec![
                    cutting.clone(),
                    cutting.iter().map(|&p| reflect(p, a, b)).collect(),
                ],
                _ => vec![cutting.clone()],
            };
            let mut shapes = Vec::new();
            // Face down, the grain is turned along the fabric as flipped.
            let flipped = |v: Vec2| if face_down { Vec2::new(-v.x, v.y) } else { v };
            let along = flipped(grain.1 - grain.0);
            let turn = -along.y.atan2(along.x);
            if on_fold && request.folded {
                // The fold along the fabric's fold, the piece below it.
                let (a, b) = fold.expect("on the fold");
                let d = flipped(b - a);
                for t in [-d.y.atan2(d.x), -d.y.atan2(d.x) + std::f64::consts::PI] {
                    let shape = shape_of(&rings, [grain.0, grain.1], t, face_down);
                    let fold_y = rotate(a, t, face_down).y;
                    let top = rings_top(&rings, t, face_down);
                    if (fold_y - top).abs() < 1.0 {
                        shapes.push(shape);
                    }
                }
            } else {
                shapes.push(shape_of(&rings, [grain.0, grain.1], turn, face_down));
                if !request.one_way {
                    shapes.push(shape_of(
                        &rings,
                        [grain.0, grain.1],
                        turn + std::f64::consts::PI,
                        face_down,
                    ));
                }
            }
            copies.push(Lay {
                piece,
                shapes,
                mirrored: face_down,
                layers,
                on_fold,
                area: area * rings.len() as f64,
            });
        }
    }
    // The biggest first.
    copies.sort_by(|a, b| b.area.total_cmp(&a.area));

    let rows_across = (across / ROW).floor() as usize;
    let mut taken: Vec<Vec<(f64, f64)>> = vec![Vec::new(); rows_across];
    let mut placed = Vec::new();
    let mut length: f64 = 0.0;
    let mut used = 0.0;
    for copy in &copies {
        // Each way it may be laid, at each place across: the one furthest
        // back along the fabric, nearest its fold or selvage.
        let mut best: Option<(f64, usize, usize)> = None;
        for (k, shape) in copy.shapes.iter().enumerate() {
            if shape.rows.len() > rows_across {
                continue;
            }
            let tops: Vec<usize> = if copy.on_fold && rows_across > 0 {
                vec![0]
            } else {
                (0..=rows_across - shape.rows.len()).collect()
            };
            for top in tops {
                // Only to go further back than the best so far (or as far,
                // nearer the fold or selvage).
                let limit = best.map_or(
                    f64::MAX,
                    |(bx, _, bt)| if top < bt { bx + 1e-9 } else { bx - 1e-9 },
                );
                if let Some(x) = slid(shape, &taken[top..top + shape.rows.len()], limit) {
                    best = Some((x, k, top));
                }
            }
        }
        let Some((x, k, top)) = best else {
            if !notes.iter().any(|n| {
                *n == LayNote::TooWide {
                    piece: copy.piece.id.clone(),
                }
            }) {
                notes.push(LayNote::TooWide {
                    piece: copy.piece.id.clone(),
                });
            }
            continue;
        };
        let shape = &copy.shapes[k];
        for (r, row) in shape.rows.iter().enumerate() {
            if let Some((a, b)) = row {
                take(&mut taken[top + r], (x + a, x + b));
            }
        }
        let y = top as f64 * ROW;
        let at = |p: Point| Pt::new(p.x + x, p.y + y);
        length = length.max(x + shape.length);
        used += copy.area * f64::from(copy.layers);
        placed.push(Placed {
            piece: copy.piece.id.clone(),
            name: copy.piece.name.clone(),
            rings: shape
                .rings
                .iter()
                .map(|ring| ring.iter().map(|&p| at(p)).collect())
                .collect(),
            grain: [at(shape.grain[0]), at(shape.grain[1])],
            mirrored: copy.mirrored,
            layers: copy.layers,
            on_fold: copy.on_fold,
        });
    }
    let fabric = request.width * length;
    Layout {
        width: request.width,
        length,
        folded: request.folded,
        used: if fabric > 0.0 { used / fabric } else { 0.0 },
        placed,
        notes,
    }
}

/// `p` turned by `t` radians (y down: clockwise on screen), face down
/// first if `mirrored` (flipped left to right).
fn rotate(p: Point, t: f64, mirrored: bool) -> Point {
    let x = if mirrored { -p.x } else { p.x };
    let (s, c) = t.sin_cos();
    Point::new(x * c - p.y * s, x * s + p.y * c)
}

/// The top of `rings` turned.
fn rings_top(rings: &[Vec<Point>], t: f64, mirrored: bool) -> f64 {
    rings
        .iter()
        .flatten()
        .map(|&p| rotate(p, t, mirrored).y)
        .fold(f64::MAX, f64::min)
}

/// `rings` turned and moved to start at the top left, with their extent
/// in each row.
fn shape_of(rings: &[Vec<Point>], grain: [Point; 2], t: f64, mirrored: bool) -> Shape {
    let turned: Vec<Vec<Point>> = rings
        .iter()
        .map(|ring| ring.iter().map(|&p| rotate(p, t, mirrored)).collect())
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in turned.iter().flatten() {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    let shift = Vec2::new(-x0, -y0);
    let rings: Vec<Vec<Point>> = turned
        .into_iter()
        .map(|ring| ring.into_iter().map(|p| p + shift).collect())
        .collect();
    let height = y1 - y0;
    let rows = (0..(height / ROW).ceil().max(1.0) as usize)
        .map(|r| {
            let (top, bottom) = (r as f64 * ROW, (r + 1) as f64 * ROW);
            rings
                .iter()
                .filter_map(|ring| across_band(ring, top, bottom))
                .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
        })
        .collect();
    Shape {
        grain: grain.map(|p| rotate(p, t, mirrored) + shift),
        rings,
        rows,
        length: x1 - x0,
    }
}

/// How far across a ring reaches between heights `top` and `bottom`: the
/// ring clipped to that band.
fn across_band(ring: &[Point], top: f64, bottom: f64) -> Option<(f64, f64)> {
    let clip = |points: Vec<Point>, inside: &dyn Fn(Point) -> bool, at: f64| -> Vec<Point> {
        let n = points.len();
        let mut out = Vec::with_capacity(n + 4);
        for i in 0..n {
            let (a, b) = (points[i], points[(i + 1) % n]);
            let (ia, ib) = (inside(a), inside(b));
            if ia {
                out.push(a);
            }
            if ia != ib && (b.y - a.y).abs() > 1e-12 {
                let s = (at - a.y) / (b.y - a.y);
                out.push(Point::new(a.x + (b.x - a.x) * s, at));
            }
        }
        out
    };
    let below_top = clip(ring.to_vec(), &|p| p.y >= top, top);
    let band = clip(below_top, &|p| p.y <= bottom, bottom);
    band.iter()
        .map(|p| (p.x, p.x))
        .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
}

/// How far along the fabric a shape slides before it fits, its rows on
/// `rows` (each row's spans taken, in order); none if not before `limit`.
fn slid(shape: &Shape, rows: &[Vec<(f64, f64)>], limit: f64) -> Option<f64> {
    let n = shape.rows.len();
    let mut x: f64 = 0.0;
    // Round the rows until as many in a row fit as it has.
    let (mut r, mut fitting) = (0, 0);
    while fitting < n {
        if let Some((a, b)) = shape.rows[r] {
            let (lo, hi) = (x + a - GAP, x + b + GAP);
            let spans = &rows[r];
            // The first span ending past `lo` (by more than rounding).
            let k = spans.partition_point(|s| s.1 <= lo + EPSILON);
            match spans.get(k) {
                Some(&(start, end)) if start < hi - EPSILON => {
                    x = (end - a + GAP).max(x + EPSILON);
                    if x >= limit {
                        return None;
                    }
                    fitting = 0;
                }
                _ => fitting += 1,
            }
        } else {
            fitting += 1;
        }
        r = (r + 1) % n;
    }
    Some(x)
}

/// A span taken in a row, merged with those it meets.
fn take(row: &mut Vec<(f64, f64)>, span: (f64, f64)) {
    let k = row.partition_point(|s| s.1 < span.0);
    let mut merged = span;
    let mut end = k;
    while end < row.len() && row[end].0 <= merged.1 {
        merged = (merged.0.min(row[end].0), merged.1.max(row[end].1));
        end += 1;
    }
    row.splice(k..end, [merged]);
}

fn reflect(p: Point, a: Point, b: Point) -> Point {
    let d = b - a;
    let l = d.hypot2();
    if l == 0.0 {
        return p;
    }
    let t = (p - a).dot(d) / l;
    let f = a + d * t;
    Point::new(2.0 * f.x - p.x, 2.0 * f.y - p.y)
}
