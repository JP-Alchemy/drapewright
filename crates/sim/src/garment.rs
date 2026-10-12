//! A garment ready to drape: each piece's cut copies laid flat (a pair as
//! its two mirror images, a piece cut on the fold unfolded whole), their
//! edges sampled so that both sides of every seam have matching points,
//! meshed, and the seams as pairs of points to sew together.
//!
//! Flat, panels are in millimetres as drawn but with y up (the pattern's
//! y runs down), so that up the panel is up the body.

use std::collections::HashMap;
use std::fmt;

use drapewright_pattern_core::stitching::{
    closed_along, dart_spans, opened_along, stitching_edges,
};
use drapewright_pattern_core::{EdgeKind, Id, MarkKind, Piece};
use drapewright_seams::Seam;

use crate::mesh::{MeshError, PanelMesh, mesh};

/// Which copy of its piece a panel is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Copy {
    /// As drawn.
    Original,
    /// Its mirror image: the other of a pair.
    Mirror,
    /// Cut on the fold: both halves, whole.
    Unfolded,
}

/// Which half of a panel an edge is on: an unfolded piece has each edge
/// twice, the second mirrored; a mirror copy's edges are all on half B.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Half {
    A,
    B,
}

/// One of a panel's edges.
#[derive(Clone, Debug, PartialEq)]
pub struct PanelEdge {
    pub edge: Id,
    pub half: Half,
    pub kind: EdgeKind,
    /// Its points along the stitching line, from the edge's start to its
    /// end (indices into the panel's mesh).
    pub points: Vec<u32>,
}

/// A piece of cloth to drape.
#[derive(Clone, Debug, PartialEq)]
pub struct Panel {
    pub piece: Id,
    pub name: String,
    pub copy: Copy,
    /// Points flat (mm, y up) and their triangles.
    pub mesh: PanelMesh,
    /// Along the grain, flat (a unit vector).
    pub grain: [f64; 2],
    pub edges: Vec<PanelEdge>,
}

impl Panel {
    pub fn edge(&self, id: &str, half: Half) -> Option<&PanelEdge> {
        self.edges.iter().find(|e| e.edge == id && e.half == half)
    }
}

/// Where a seam joins two panels: for placing them.
#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    /// Panel, edge, half.
    pub a: (usize, Id, Half),
    pub b: (usize, Id, Half),
    pub name: String,
}

/// A garment's panels and the points sewn together.
#[derive(Clone, Debug, PartialEq)]
pub struct Garment {
    pub panels: Vec<Panel>,
    /// Pairs of (panel, point) sewn together.
    pub stitches: Vec<[(usize, u32); 2]>,
    pub links: Vec<Link>,
}

/// Why a garment could not be made.
#[derive(Clone, Debug, PartialEq)]
pub enum GarmentError {
    /// A piece's outline could not be meshed.
    Mesh { piece: String, error: MeshError },
    /// Nothing to drape.
    Empty,
}

impl fmt::Display for GarmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GarmentError::Mesh { piece, error } => write!(f, "{piece}: {error}"),
            GarmentError::Empty => f.write_str("there are no pieces to drape"),
        }
    }
}

impl std::error::Error for GarmentError {}

type P2 = [f64; 2];

fn length(points: &[P2]) -> f64 {
    points
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum()
}

/// The point `s` mm along a polyline.
fn at_length(points: &[P2], s: f64) -> P2 {
    let mut walked = 0.0;
    for w in points.windows(2) {
        let l = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if walked + l >= s && l > 0.0 {
            let t = ((s - walked) / l).clamp(0.0, 1.0);
            return [
                w[0][0] + t * (w[1][0] - w[0][0]),
                w[0][1] + t * (w[1][1] - w[0][1]),
            ];
        }
        walked += l;
    }
    *points.last().unwrap_or(&[0.0, 0.0])
}

/// `n` equal steps along a polyline from `from` mm to `to` mm: n + 1 points.
fn steps(points: &[P2], from: f64, to: f64, n: usize) -> Vec<P2> {
    (0..=n)
        .map(|k| at_length(points, from + (to - from) * k as f64 / n as f64))
        .collect()
}

/// A panel before meshing: its edges' stitching lines (y up, each from its
/// start to its end) and the order they run round it.
struct Layout {
    piece: usize,
    copy: Copy,
    edges: Vec<(Id, Half, EdgeKind, Vec<P2>)>,
    /// Round the panel: (edge, run backwards).
    order: Vec<(usize, bool)>,
    grain: P2,
    /// Unfolded: the fold, from its start to its end.
    fold: Option<(P2, P2)>,
    /// Each edge's darts: where they are along its stitching line.
    darts: Vec<Vec<(f64, f64)>>,
    /// The piece's marks (buttons, say) where they are on the panel.
    marks: Vec<(MarkKind, P2)>,
}

fn reflect(p: P2, a: P2, b: P2) -> P2 {
    let d = [b[0] - a[0], b[1] - a[1]];
    let l = d[0] * d[0] + d[1] * d[1];
    if l == 0.0 {
        return p;
    }
    let t = ((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l;
    let f = [a[0] + t * d[0], a[1] + t * d[1]];
    [2.0 * f[0] - p[0], 2.0 * f[1] - p[1]]
}

fn twice_area(loop_: &[P2]) -> f64 {
    (0..loop_.len())
        .map(|i| {
            let (a, b) = (loop_[i], loop_[(i + 1) % loop_.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

/// The order round a panel, made counter-clockwise.
fn counter_clockwise(
    edges: &[(Id, Half, EdgeKind, Vec<P2>)],
    order: Vec<(usize, bool)>,
) -> Vec<(usize, bool)> {
    let loop_: Vec<P2> = order
        .iter()
        .flat_map(|&(e, back)| {
            let p = &edges[e].3;
            let mut v: Vec<P2> = p[..p.len() - 1].to_vec();
            if back {
                v = p[1..].iter().rev().copied().collect();
            }
            v
        })
        .collect();
    if twice_area(&loop_) >= 0.0 {
        order
    } else {
        order
            .into_iter()
            .rev()
            .map(|(e, back)| (e, !back))
            .collect()
    }
}

/// `p` turned a quarter `quarters` times, counter-clockwise (y up).
fn quarter_turned(p: P2, quarters: u8) -> P2 {
    match quarters % 4 {
        0 => p,
        1 => [-p[1], p[0]],
        2 => [-p[0], -p[1]],
        _ => [p[1], -p[0]],
    }
}

/// A hem's allowance is at least this much more than a seam's.
const HEM_DEEPER: f64 = 1.5;

/// How many quarter turns, counter-clockwise (y up), stand a piece upright
/// as worn: its grain up and down the body and its hems at the bottom.
/// Pattern CAD lays long pieces on their side to fit the sheet. A piece is
/// taken as drawn unless its grain or its hems say otherwise; one lying on
/// its side whose hems don't say which way is up is stood with its grain's
/// arrow (at `to`) pointing down, as CAD draws it. `lines` are its edges'
/// stitching lines, y up.
fn upright(piece: &Piece, lines: &[Vec<P2>]) -> u8 {
    let grain = piece.grainline.as_ref().map(|g| {
        let d = [g.to.x - g.from.x, -(g.to.y - g.from.y)];
        let l = d[0].hypot(d[1]).max(1e-9);
        [d[0] / l, d[1] / l]
    });
    // Hems: marked as such, or given a deeper allowance than the seams.
    let seam = piece.allowance.seam.max(1.0);
    let hems: Vec<P2> = (0..lines.len())
        .filter(|&e| {
            piece.outline.edges[e].kind == EdgeKind::Hem
                || (piece.outline.edges[e].kind == EdgeKind::Seam
                    && piece.edge_allowance(e) >= HEM_DEEPER * seam
                    && piece.edge_allowance(e) >= 15.0)
        })
        .map(|e| at_length(&lines[e], length(&lines[e]) / 2.0))
        .collect();
    let all: Vec<P2> = lines.iter().flatten().copied().collect();
    let score = |quarters: u8| {
        let mut score = if quarters == 0 { 1.0 } else { 0.0 };
        if let Some(g) = grain {
            let g = quarter_turned(g, quarters);
            if g[1].abs() > std::f64::consts::FRAC_1_SQRT_2 {
                score += 2.0;
            }
            if g[1] < -0.5 {
                score += 0.75;
            }
        }
        let ys: Vec<f64> = all
            .iter()
            .map(|&p| quarter_turned(p, quarters)[1])
            .collect();
        let (low, high) = ys
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), &y| (l.min(y), h.max(y)));
        if high > low {
            for &hem in &hems {
                let height = (quarter_turned(hem, quarters)[1] - low) / (high - low);
                score += 2.0 * (1.0 - 2.0 * height);
            }
        }
        score
    };
    (0..4u8)
        .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)))
        .unwrap_or(0)
}

fn layouts(pieces: &[Piece]) -> Vec<Layout> {
    let mut out = Vec::new();
    for (k, piece) in pieces.iter().enumerate() {
        let stitching = stitching_edges(piece);
        let spans: Vec<Vec<(f64, f64)>> = (0..stitching.len())
            .map(|e| dart_spans(piece, e, &stitching[e]))
            .collect();
        let lines: Vec<Vec<P2>> = stitching
            .into_iter()
            .map(|e| e.into_iter().map(|p| [p.x, -p.y]).collect())
            .collect();
        // Stood upright, so that up the panel is up the body.
        let quarters = upright(piece, &lines);
        let lines: Vec<Vec<P2>> = lines
            .into_iter()
            .map(|e| e.into_iter().map(|p| quarter_turned(p, quarters)).collect())
            .collect();
        let kinds: Vec<(Id, EdgeKind)> = piece
            .outline
            .edges
            .iter()
            .map(|e| (e.id.clone(), e.kind))
            .collect();
        let grain = piece.grainline.as_ref().map_or([0.0, 1.0], |g| {
            let d = [g.to.x - g.from.x, -(g.to.y - g.from.y)];
            let l = d[0].hypot(d[1]).max(1e-9);
            quarter_turned([d[0] / l, d[1] / l], quarters)
        });
        let cut = piece
            .cut
            .iter()
            .find(|c| c.material == "main")
            .or(piece.cut.first())
            .cloned()
            .unwrap_or_default();
        let marks: Vec<(MarkKind, P2)> = piece
            .marks
            .iter()
            .map(|m| (m.kind, quarter_turned([m.at.x, -m.at.y], quarters)))
            .collect();
        let fold = kinds.iter().position(|(_, kind)| *kind == EdgeKind::Fold);
        let on_fold = cut.on_fold.unwrap_or(fold.is_some()) && fold.is_some();
        let pair = cut.quantity >= 2 && cut.mirrored.unwrap_or(true);
        let n = lines.len();
        if on_fold {
            let f = fold.expect("checked");
            let (a, b) = (lines[f][0], lines[f][lines[f].len() - 1]);
            let mut edges = Vec::new();
            let mut order = Vec::new();
            let mut darts = Vec::new();
            // Half A from the fold's end round to its start, as drawn ...
            for step in 1..n {
                let e = (f + step) % n;
                edges.push((kinds[e].0.clone(), Half::A, kinds[e].1, lines[e].clone()));
                darts.push(spans[e].clone());
                order.push((edges.len() - 1, false));
            }
            // ... then half B, mirrored in the fold, back round to its end.
            for step in (1..n).rev() {
                let e = (f + step) % n;
                let mirrored = lines[e].iter().map(|&p| reflect(p, a, b)).collect();
                edges.push((kinds[e].0.clone(), Half::B, kinds[e].1, mirrored));
                darts.push(spans[e].clone());
                order.push((edges.len() - 1, true));
            }
            let order = counter_clockwise(&edges, order);
            out.push(Layout {
                piece: k,
                copy: Copy::Unfolded,
                edges,
                order,
                grain,
                fold: Some((a, b)),
                darts,
                marks: marks.clone(),
            });
            continue;
        }
        let mut copies = vec![Copy::Original];
        if pair {
            copies.push(Copy::Mirror);
        }
        for copy in copies {
            let (half, flip) = match copy {
                Copy::Mirror => (Half::B, -1.0),
                _ => (Half::A, 1.0),
            };
            let edges: Vec<_> = (0..n)
                .map(|e| {
                    let points = lines[e].iter().map(|p| [flip * p[0], p[1]]).collect();
                    (kinds[e].0.clone(), half, kinds[e].1, points)
                })
                .collect();
            let order = counter_clockwise(&edges, (0..n).map(|e| (e, false)).collect());
            out.push(Layout {
                piece: k,
                copy,
                edges,
                order,
                grain: [flip * grain[0], grain[1]],
                fold: None,
                darts: spans.clone(),
                marks: marks
                    .iter()
                    .map(|&(k, m)| (k, [flip * m[0], m[1]]))
                    .collect(),
            });
        }
    }
    out
}

/// Whether runs of points `x` and `y` are each other's mirror image in the
/// line from `a` to `b`.
fn mirrors(x: &[P2], y: &[P2], a: P2, b: P2) -> bool {
    x.len() == y.len()
        && x.iter().zip(y).all(|(&p, &q)| {
            let r = reflect(q, a, b);
            (p[0] - r[0]).hypot(p[1] - r[1]) < 1e-6
        })
}

/// Meshes chains running round either way, giving the chains' points in
/// the way they were given.
fn mesh_either_way(chains: &[Vec<P2>], spacing: f64) -> Result<PanelMesh, MeshError> {
    let round: Vec<P2> = chains
        .iter()
        .flat_map(|c| c[..c.len() - 1].iter().copied())
        .collect();
    if twice_area(&round) > 0.0 {
        return mesh(chains, spacing);
    }
    let backwards: Vec<Vec<P2>> = chains
        .iter()
        .rev()
        .map(|c| c.iter().rev().copied().collect())
        .collect();
    let mut m = mesh(&backwards, spacing)?;
    m.chains = m
        .chains
        .into_iter()
        .rev()
        .map(|c| c.into_iter().rev().collect())
        .collect();
    Ok(m)
}

/// Mesh `m` reflected in the line from `a` to `b`, its triangles wound
/// counter-clockwise still; the points in `keep` (on the line) not copied.
/// Also gives each of `m`'s points' index in the reflection.
fn reflected(m: &PanelMesh, a: P2, b: P2, keep: &[u32]) -> (Vec<P2>, Vec<[u32; 3]>, Vec<u32>) {
    let mut points = Vec::new();
    let mut index = vec![0u32; m.points.len()];
    for (i, &p) in m.points.iter().enumerate() {
        if keep.contains(&(i as u32)) {
            index[i] = i as u32;
        } else {
            index[i] = (m.points.len() + points.len()) as u32;
            points.push(reflect(p, a, b));
        }
    }
    let triangles = m
        .triangles
        .iter()
        .map(|t| {
            [
                index[t[0] as usize],
                index[t[2] as usize],
                index[t[1] as usize],
            ]
        })
        .collect();
    (points, triangles, index)
}

/// A panel meshed as the mirror image of what it mirrors, so that its two
/// sides (a piece unfolded, or a pair) are meshed alike: alike
/// triangulations meet a body alike on either side, and a garment cut
/// alike on both sides hangs alike. `None` where its sides are sewn
/// differently, and it is meshed whole.
fn mirrored_mesh(
    p: usize,
    layouts: &[Layout],
    sampled: &[Vec<Vec<P2>>],
    panels: &[Panel],
    spacing: f64,
) -> Result<Option<PanelMesh>, MeshError> {
    let layout = &layouts[p];
    let in_order = |edge_points: &dyn Fn(usize) -> Vec<u32>| -> Vec<Vec<u32>> {
        layout
            .order
            .iter()
            .map(|&(e, back)| {
                let mut c = edge_points(e);
                if back {
                    c.reverse();
                }
                c
            })
            .collect()
    };
    match (layout.copy, layout.fold) {
        (Copy::Unfolded, Some((a, b))) => {
            // Half B's edges mirror half A's: edge `half + j` is edge
            // `half - 1 - j` reflected in the fold.
            let half = layout.edges.len() / 2;
            if !(0..half).all(|j| mirrors(&sampled[p][half + j], &sampled[p][half - 1 - j], a, b)) {
                return Ok(None);
            }
            // Half A, from the fold's end round to its start, then along
            // the fold.
            let along = (a[0] - b[0]).hypot(a[1] - b[1]);
            let mut chains: Vec<Vec<P2>> = (0..half).map(|e| sampled[p][e].clone()).collect();
            chains.push(steps(
                &[a, b],
                0.0,
                along,
                (along / spacing).round().max(1.0) as usize,
            ));
            let m = mesh_either_way(&chains, spacing)?;
            let fold = m.chains[half].clone();
            let (more, triangles, index) = reflected(&m, a, b, &fold);
            let edge_points = |e: usize| -> Vec<u32> {
                if e < half {
                    m.chains[e].clone()
                } else {
                    m.chains[2 * half - 1 - e]
                        .iter()
                        .map(|&i| index[i as usize])
                        .collect()
                }
            };
            let chains = in_order(&edge_points);
            let mut points = m.points.clone();
            points.extend(more);
            let mut all = m.triangles.clone();
            all.extend(triangles);
            Ok(Some(PanelMesh {
                points,
                triangles: all,
                chains,
            }))
        }
        (Copy::Mirror, _) if p > 0 => {
            let original = p - 1;
            let (a, b) = ([0.0, 0.0], [0.0, 1.0]);
            if layouts[original].copy != Copy::Original
                || layouts[original].piece != layout.piece
                || !(0..layout.edges.len())
                    .all(|e| mirrors(&sampled[p][e], &sampled[original][e], a, b))
            {
                return Ok(None);
            }
            // The same points along each edge, from its start to its end.
            let m = &panels[original].mesh;
            let (points, triangles, _) = reflected(m, a, b, &[]);
            let base = m.points.len() as u32;
            let triangles = triangles.into_iter().map(|t| t.map(|i| i - base)).collect();
            let chains = in_order(&|e: usize| panels[original].edges[e].points.clone());
            Ok(Some(PanelMesh {
                points,
                triangles,
                chains,
            }))
        }
        _ => Ok(None),
    }
}

/// Where a dart's tip is along a stitching line, between its mouth's ends
/// `from` and `to`: the furthest point from its mouth (a corner of the
/// line, or where it is curved, a point along it).
fn tip_along(line: &[P2], from: f64, to: f64) -> f64 {
    let (a, b) = (at_length(line, from), at_length(line, to));
    let mouth = [b[0] - a[0], b[1] - a[1]];
    let width = mouth[0].hypot(mouth[1]).max(1e-9);
    let off = |p: P2| ((p[0] - a[0]) * mouth[1] - (p[1] - a[1]) * mouth[0]).abs() / width;
    let mut walked = 0.0;
    let mut corners = Vec::new();
    for w in line.windows(2) {
        walked += (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if from < walked && walked < to {
            corners.push((walked, w[1]));
        }
    }
    let along = (0..=64).map(|k| {
        let u = from + (to - from) * f64::from(k) / 64.0;
        (u, at_length(line, u))
    });
    corners
        .into_iter()
        .chain(along)
        .max_by(|s, t| off(s.1).total_cmp(&off(t.1)))
        .map_or((from + to) / 2.0, |(u, _)| u)
}

/// Side `x` with a dart of its edge in it taken out, and side `y` it is
/// sewn to split where that dart closes, as sewn: the parts before and
/// after the dart (along `x`) that are left, each pair sewn as `x` and `y`
/// were. None without such a dart.
fn past_dart(layouts: &[Layout], x: &Side, y: &Side, reversed: bool) -> Option<Vec<(Side, Side)>> {
    const SHORT: f64 = 1e-6;
    let spans = &layouts[x.panel].darts[x.edge];
    let (lo, hi) = (x.from.min(x.to), x.from.max(x.to));
    let &(d0, d1) = spans
        .iter()
        .find(|&&(d0, d1)| lo < d1 - SHORT && d0 + SHORT < hi)?;
    let closed = |v: f64| closed_along(spans, v);
    let sewn = (closed(x.to) - closed(x.from)).abs();
    if sewn < SHORT {
        // All dart: nothing of it is sewn.
        return Some(Vec::new());
    }
    // The parts of `x`, its own way, and how far along it as sewn the dart
    // closes.
    let (first, second, f) = if x.from <= x.to {
        (
            (x.from, d0.max(x.from)),
            (d1.min(x.to), x.to),
            (closed(d0) - closed(x.from)) / sewn,
        )
    } else {
        (
            (x.from, d1.min(x.from)),
            (d0.max(x.to), x.to),
            (closed(x.from) - closed(d1)) / sewn,
        )
    };
    // The same place along `y`, as sewn.
    let y_spans = &layouts[y.panel].darts[y.edge];
    let y_closed = |v: f64| closed_along(y_spans, v);
    let g = if reversed { 1.0 - f } else { f };
    let split = opened_along(
        y_spans,
        y_closed(y.from) + g * (y_closed(y.to) - y_closed(y.from)),
    );
    let side = |s: &Side, (from, to): (f64, f64)| Side {
        from,
        to,
        ..s.clone()
    };
    let (y_first, y_second) = if reversed {
        ((split, y.to), (y.from, split))
    } else {
        ((y.from, split), (split, y.to))
    };
    let long = |(from, to): (f64, f64)| (to - from).abs() > SHORT;
    Some(
        [(first, y_first), (second, y_second)]
            .into_iter()
            .filter(|&(xs, _)| long(xs))
            .map(|(xs, ys)| (side(x, xs), side(y, ys)))
            .collect(),
    )
}

/// A stretch of an edge to sew: from and to (mm along it), in how many
/// steps, for which seam, and whether it is that seam's second side.
type Stretch = (f64, f64, usize, usize, bool);

/// One side of a seam, on a panel.
#[derive(Clone, Debug)]
struct Side {
    panel: usize,
    edge: usize,
    from: f64,
    to: f64,
}

/// A facing or a lining: turned inside when the garment is made up, it
/// lies against what it faces. Draped, with no cloth of its own to rest
/// on, it would hang loose from its seams instead.
pub fn inside(piece: &Piece) -> bool {
    let name = piece.name.to_lowercase();
    let lining = |material: &str| {
        let m = material.to_lowercase();
        m.contains("lining") || m.contains("interfacing")
    };
    name.contains("facing")
        || name.contains("lining")
        || (!piece.cut.is_empty() && piece.cut.iter().all(|c| lining(&c.material)))
}

/// What a drape is made of: the pieces sewn by confirmed seams, less
/// facings and linings; and, left out, the pieces sewn to nothing and the
/// facings and linings (by id).
pub fn draped_pieces(pieces: &[Piece], seams: &[Seam]) -> (Vec<Piece>, Vec<Id>, Vec<Id>) {
    let (mut sewn, mut alone, mut within) = (Vec::new(), Vec::new(), Vec::new());
    for piece in pieces {
        if inside(piece) {
            within.push(piece.id.clone());
        } else if seams.iter().any(|s| s.confirmed && s.touches(&piece.id)) {
            sewn.push(piece.clone());
        } else {
            alone.push(piece.id.clone());
        }
    }
    (sewn, alone, within)
}

/// Lays out the pieces, sews them by `seams` (confirmed ones only), and
/// meshes them with points about `spacing` mm apart.
pub fn assemble(pieces: &[Piece], seams: &[Seam], spacing: f64) -> Result<Garment, GarmentError> {
    let layouts = layouts(pieces);
    if layouts.is_empty() {
        return Err(GarmentError::Empty);
    }
    // Each piece's panels, and which edge of each is a given edge's half.
    let find = |piece: &str, edge: &str, half: Half| -> Option<(usize, usize)> {
        layouts.iter().enumerate().find_map(|(i, l)| {
            (pieces[l.piece].id == piece)
                .then(|| {
                    l.edges
                        .iter()
                        .position(|e| e.0 == edge && e.1 == half)
                        .map(|e| (i, e))
                })
                .flatten()
        })
    };
    // The seams on panels: each seam on both sides of the body where its
    // pieces are in pairs, once where it names a mirror copy itself.
    let mut sewn: Vec<(Side, Side, bool, String)> = Vec::new();
    for seam in seams.iter().filter(|s| s.confirmed) {
        let explicit = seam.a.mirror || seam.b.mirror;
        let halves: &[(Half, Half)] = if explicit {
            &[(Half::A, Half::A)]
        } else {
            &[(Half::A, Half::A), (Half::B, Half::B)]
        };
        for &(ha, hb) in halves {
            let side = |s: &drapewright_seams::SeamSide, half: Half| -> Option<Side> {
                let half = if explicit {
                    if s.mirror { Half::B } else { Half::A }
                } else {
                    half
                };
                let (panel, edge) = find(&s.piece, &s.edge, half)?;
                let line = &layouts[panel].edges[edge].3;
                let total = length(line);
                Some(Side {
                    panel,
                    edge,
                    from: s.from.unwrap_or(0.0).clamp(0.0, total),
                    to: s.to.unwrap_or(total).clamp(0.0, total),
                })
            };
            if let (Some(a), Some(b)) = (side(&seam.a, ha), side(&seam.b, hb)) {
                sewn.push((a, b, seam.reversed, seam.name.clone()));
            }
        }
    }
    // A seam runs past a dart in its edge, the dart closed: each side split
    // where the dart is, at the same place on the other side as sewn.
    let mut split = Vec::new();
    for seam in sewn {
        let mut parts = vec![seam];
        while let Some((a, b, reversed, name)) = parts.pop() {
            let around = past_dart(&layouts, &a, &b, reversed).or_else(|| {
                past_dart(&layouts, &b, &a, reversed)
                    .map(|v| v.into_iter().map(|(y, x)| (x, y)).collect())
            });
            match around {
                // In order: the first part next.
                Some(around) => parts.extend(
                    around
                        .into_iter()
                        .rev()
                        .map(|(x, y)| (x, y, reversed, name.clone())),
                ),
                None => split.push((a, b, reversed, name)),
            }
        }
    }
    let mut sewn = split;
    // Seams, then darts: only seams join panels.
    let seams_sewn = sewn.len();
    // Darts sewn closed, leg to leg, from the mouth to the tip.
    for (p, layout) in layouts.iter().enumerate() {
        for (e, spans) in layout.darts.iter().enumerate() {
            let line = &layout.edges[e].3;
            for &(from, to) in spans {
                let tip = tip_along(line, from, to);
                let leg = |end: f64| Side {
                    panel: p,
                    edge: e,
                    from: end,
                    to: tip,
                };
                sewn.push((leg(from), leg(to), false, String::new()));
            }
        }
    }
    // Each edge's stretches sewn to others, with as many steps on both sides.
    let mut wanted: HashMap<(usize, usize), Vec<Stretch>> = HashMap::new();
    for (k, (a, b, _, _)) in sewn.iter().enumerate() {
        let n = ((a.to - a.from).abs().max((b.to - b.from).abs()) / spacing)
            .round()
            .max(1.0) as usize;
        wanted
            .entry((a.panel, a.edge))
            .or_default()
            .push((a.from, a.to, n, k, false));
        wanted
            .entry((b.panel, b.edge))
            .or_default()
            .push((b.from, b.to, n, k, true));
    }
    // Sampled edges: their points, and where each seam side's run is.
    let mut sampled: Vec<Vec<Vec<P2>>> = Vec::new();
    let mut runs: HashMap<(usize, bool), (usize, usize, usize, usize)> = HashMap::new();
    for (p, layout) in layouts.iter().enumerate() {
        let mut edges = Vec::new();
        for (e, (_, _, _, line)) in layout.edges.iter().enumerate() {
            let total = length(line);
            let mut asked = wanted.remove(&(p, e)).unwrap_or_default();
            asked.sort_by(|x, y| x.0.min(x.1).total_cmp(&y.0.min(y.1)));
            let mut points: Vec<P2> = vec![line[0]];
            let mut at = 0.0;
            let free = |points: &mut Vec<P2>, from: f64, to: f64| {
                if to - from > 1e-6 {
                    let n = ((to - from) / spacing).round().max(1.0) as usize;
                    points.extend(steps(line, from, to, n).into_iter().skip(1));
                }
            };
            for (from, to, n, k, second) in asked {
                let (lo, hi) = (from.min(to), from.max(to));
                if lo < at - 1e-6 {
                    // Overlaps a stretch already sewn: left out.
                    continue;
                }
                free(&mut points, at, lo);
                let start = points.len() - 1;
                points.extend(steps(line, lo, hi, n).into_iter().skip(1));
                // Recorded from its `from` to its `to`.
                runs.insert((k, second), (p, e, start, if from <= to { 0 } else { 1 }));
                at = hi;
            }
            free(&mut points, at, total);
            edges.push(points);
        }
        sampled.push(edges);
    }
    // Meshed, each edge's points found in the mesh.
    let mut panels = Vec::new();
    for (p, layout) in layouts.iter().enumerate() {
        let chains: Vec<Vec<P2>> = layout
            .order
            .iter()
            .map(|&(e, back)| {
                let mut c = sampled[p][e].clone();
                if back {
                    c.reverse();
                }
                c
            })
            .collect();
        let failed = |error| GarmentError::Mesh {
            piece: pieces[layout.piece].name.clone(),
            error,
        };
        let m = match mirrored_mesh(p, &layouts, &sampled, &panels, spacing).map_err(failed)? {
            Some(m) => m,
            None => mesh(&chains, spacing).map_err(failed)?,
        };
        let mut edges: Vec<PanelEdge> = Vec::new();
        for (chain, &(e, back)) in m.chains.iter().zip(&layout.order) {
            let mut points = chain.clone();
            if back {
                points.reverse();
            }
            let (id, half, kind, _) = &layout.edges[e];
            edges.push(PanelEdge {
                edge: id.clone(),
                half: *half,
                kind: *kind,
                points,
            });
        }
        // As laid out, edge by edge (the mesh's chains run round the panel).
        let mut by_layout: Vec<Option<PanelEdge>> = vec![None; layout.edges.len()];
        for (edge, &(e, _)) in edges.into_iter().zip(&layout.order) {
            by_layout[e] = Some(edge);
        }
        panels.push(Panel {
            piece: pieces[layout.piece].id.clone(),
            name: pieces[layout.piece].name.clone(),
            copy: layout.copy,
            mesh: m,
            grain: layout.grain,
            edges: by_layout.into_iter().flatten().collect(),
        });
    }
    // Stitches: the two runs of each seam, point for point.
    let mut stitches = Vec::new();
    let mut links = Vec::new();
    for (k, (a, b, reversed, name)) in sewn.iter().enumerate() {
        let (Some(&ra), Some(&rb)) = (runs.get(&(k, false)), runs.get(&(k, true))) else {
            continue;
        };
        let n = ((a.to - a.from).abs().max((b.to - b.from).abs()) / spacing)
            .round()
            .max(1.0) as usize;
        let run = |r: (usize, usize, usize, usize)| -> Vec<(usize, u32)> {
            let (p, e, start, backwards) = r;
            let points = &panels[p].edges[e].points;
            let mut v: Vec<(usize, u32)> = (start..=start + n).map(|i| (p, points[i])).collect();
            if backwards == 1 {
                v.reverse();
            }
            v
        };
        let (mut va, vb) = (run(ra), run(rb));
        if *reversed {
            va.reverse();
        }
        // A dart's legs meet at its tip: the same point.
        for (x, y) in va.into_iter().zip(vb).filter(|(x, y)| x != y) {
            stitches.push([x, y]);
        }
        if k >= seams_sewn {
            continue;
        }
        let half = |s: &Side| layouts[s.panel].edges[s.edge].1;
        let link = Link {
            a: (a.panel, layouts[a.panel].edges[a.edge].0.clone(), half(a)),
            b: (b.panel, layouts[b.panel].edges[b.edge].0.clone(), half(b)),
            name: name.clone(),
        };
        // A seam in parts either side of a dart: one link.
        if !links.contains(&link) {
            links.push(link);
        }
    }
    stitches.extend(buttons(&layouts, pieces, &panels, &sewn[..seams_sewn]));
    Ok(Garment {
        panels,
        stitches,
        links,
    })
}

/// Buttons are marked this near the edge they button over, mm.
const BUTTON_STAND: f64 = 40.0;

/// A piece's name without "left" or "right".
fn stem(name: &str) -> String {
    name.to_lowercase()
        .split_whitespace()
        .filter(|w| *w != "left" && *w != "right")
        .collect::<Vec<_>>()
        .join(" ")
}

/// A garment buttoned down the front, as worn: the buttons marked along
/// one front's open edge (an edge sewn to nothing), each held to a
/// buttonhole marked along the other front's, or else (buttoned through
/// loops) to that front's open edge at the same height. Pairs of (panel,
/// point) held together, as stitches are.
fn buttons(
    layouts: &[Layout],
    pieces: &[Piece],
    panels: &[Panel],
    sewn: &[(Side, Side, bool, String)],
) -> Vec<[(usize, u32); 2]> {
    let open = |p: usize, e: usize| {
        !sewn
            .iter()
            .any(|(a, b, ..)| (a.panel == p && a.edge == e) || (b.panel == p && b.edge == e))
    };
    // Each front's open edge running up and down it (the longest), and
    // the marks near it: (edge, buttons, buttonholes).
    type Opening = (usize, Vec<P2>, Vec<P2>);
    let openings: Vec<Option<Opening>> = layouts
        .iter()
        .enumerate()
        .map(|(p, layout)| {
            if layout.copy == Copy::Unfolded
                || !pieces[layout.piece].name.to_lowercase().contains("front")
            {
                return None;
            }
            let edge = (0..layout.edges.len())
                .filter(|&e| open(p, e))
                .filter(|&e| {
                    let line = &layout.edges[e].3;
                    let (a, b) = (line[0], line[line.len() - 1]);
                    (b[1] - a[1]).abs() > (b[0] - a[0]).abs()
                })
                .max_by(|&x, &y| {
                    length(&layout.edges[x].3).total_cmp(&length(&layout.edges[y].3))
                })?;
            let line = &layout.edges[edge].3;
            let near = |m: P2| {
                line.windows(2).any(|w| {
                    let (a, b) = (w[0], w[1]);
                    let d = [b[0] - a[0], b[1] - a[1]];
                    let l = d[0] * d[0] + d[1] * d[1];
                    let t = if l > 0.0 {
                        (((m[0] - a[0]) * d[0] + (m[1] - a[1]) * d[1]) / l).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let f = [a[0] + t * d[0], a[1] + t * d[1]];
                    (m[0] - f[0]).hypot(m[1] - f[1]) <= BUTTON_STAND
                })
            };
            let marked = |kinds: &[MarkKind]| -> Vec<P2> {
                layout
                    .marks
                    .iter()
                    .filter(|(k, m)| kinds.contains(k) && near(*m))
                    .map(|&(_, m)| m)
                    .collect()
            };
            Some((
                edge,
                marked(&[MarkKind::Button, MarkKind::Cross]),
                marked(&[MarkKind::Buttonhole]),
            ))
        })
        .collect();
    let low = |p: usize| {
        panels[p]
            .mesh
            .points
            .iter()
            .map(|q| q[1])
            .fold(f64::MAX, f64::min)
    };
    let nearest = |p: usize, m: P2| -> u32 {
        let points = &panels[p].mesh.points;
        (0..points.len())
            .min_by(|&a, &b| {
                let d = |i: usize| (points[i][0] - m[0]).hypot(points[i][1] - m[1]);
                d(a).total_cmp(&d(b))
            })
            .map_or(0, |i| i as u32)
    };
    let mut out = Vec::new();
    for p in 0..layouts.len() {
        let Some((_, buttons, _)) = &openings[p] else {
            continue;
        };
        if buttons.is_empty() {
            continue;
        }
        // The other front: as named but for its side, or the other of a pair.
        let Some(q) = (0..layouts.len()).find(|&q| {
            q != p
                && openings[q].is_some()
                && stem(&pieces[layouts[q].piece].name) == stem(&pieces[layouts[p].piece].name)
                && !openings[q]
                    .as_ref()
                    .is_some_and(|o| !o.1.is_empty() && q < p)
        }) else {
            continue;
        };
        let (edge, theirs, holes) = openings[q].as_ref().expect("found");
        let held: Vec<P2> = if holes.is_empty() {
            theirs.clone()
        } else {
            holes.clone()
        };
        let (base_p, base_q) = (low(p), low(q));
        for &b in buttons {
            let height = b[1] - base_p;
            let to = if held.is_empty() {
                // Through a loop at the edge: the edge's point that high.
                let points = &panels[q].edges[*edge].points;
                points
                    .iter()
                    .copied()
                    .min_by(|&i, &j| {
                        let off =
                            |k: u32| (panels[q].mesh.points[k as usize][1] - base_q - height).abs();
                        off(i).total_cmp(&off(j))
                    })
                    .expect("an edge has points")
            } else {
                let m = held
                    .iter()
                    .copied()
                    .min_by(|x, y| {
                        ((x[1] - base_q) - height)
                            .abs()
                            .total_cmp(&((y[1] - base_q) - height).abs())
                    })
                    .expect("marks");
                nearest(q, m)
            };
            out.push([(p, nearest(p, b)), (q, to)]);
        }
    }
    out
}
