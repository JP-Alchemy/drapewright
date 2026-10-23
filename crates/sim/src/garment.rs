//! A garment ready to drape: each piece's cut copies laid flat (a pair as
//! its two mirror images, a piece cut on the fold unfolded whole), their
//! edges sampled so that both sides of every seam have matching points,
//! meshed, and the seams as pairs of points to sew together.
//!
//! Flat, panels are in millimetres as drawn but with y up (the pattern's
//! y runs down), so that up the panel is up the body.

use std::collections::HashMap;
use std::fmt;

use drapewright_pattern_core::geom::{flatten_segs, segs_of};
use drapewright_pattern_core::stitching::{
    closed_along, dart_spans, line_points, opened_along, stitching_edges,
};
use drapewright_pattern_core::upright::{quarter_turned, upright_turns};
use drapewright_pattern_core::{EdgeKind, Id, LineKind, MarkKind, Piece};
use drapewright_seams::Seam;

use crate::mesh::{MeshError, PanelMesh, inside_panel, line_clearance, mesh_inside};

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
    /// How high its line named for the waist is, flat: where it goes on the
    /// body (a corset is put on by its waist, not hung from it).
    pub waist: Option<f64>,
    /// A facing or a lining: it goes inside the rest.
    pub inside: bool,
    /// Inside: the panel it lies on, if known.
    pub over: Option<usize>,
    /// Sewn into the garment's seams, joined to it only so (a cape sewn
    /// into the princess seams): it hangs outside the rest.
    pub outside: bool,
    /// Sewn onto the face of the panel it lies on (`over`), a patch pocket:
    /// it lies outside that panel.
    pub sewn_on: bool,
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
    /// Pairs of (panel, point) sewn together, the buttons done up last.
    pub stitches: Vec<[(usize, u32); 2]>,
    pub links: Vec<Link>,
    /// How many of the stitches are buttons done up.
    pub buttons: usize,
}

impl Garment {
    /// The garment of its first `n` panels only, as sewn among themselves.
    pub fn first(&self, n: usize) -> Garment {
        let keep: Vec<bool> = (0..self.panels.len()).map(|p| p < n).collect();
        self.keeping(&keep)
    }

    /// The garment without the panels `keep` says no to, and their
    /// stitches.
    pub fn keeping(&self, keep: &[bool]) -> Garment {
        let mut index = vec![None; self.panels.len()];
        let mut panels = Vec::new();
        for (p, panel) in self.panels.iter().enumerate() {
            if keep.get(p).copied().unwrap_or(false) {
                index[p] = Some(panels.len());
                panels.push(panel.clone());
            }
        }
        let moved = |(p, i): (usize, u32)| index[p].map(|q| (q, i));
        let stitch = |s: &[(usize, u32); 2]| Some([moved(s[0])?, moved(s[1])?]);
        let (sewn, done_up) = self.stitches.split_at(self.stitches.len() - self.buttons);
        let mut stitches: Vec<_> = sewn.iter().filter_map(stitch).collect();
        let buttons: Vec<_> = done_up.iter().filter_map(stitch).collect();
        let count = buttons.len();
        stitches.extend(buttons);
        for panel in &mut panels {
            panel.over = panel.over.and_then(|o| index[o]);
        }
        let links = self
            .links
            .iter()
            .filter_map(|l| {
                Some(Link {
                    a: (index[l.a.0]?, l.a.1.clone(), l.a.2),
                    b: (index[l.b.0]?, l.b.1.clone(), l.b.2),
                    name: l.name.clone(),
                })
            })
            .collect();
        Garment {
            panels,
            stitches,
            links,
            buttons: count,
        }
    }

    /// The garment with its buttons undone: worn open.
    pub fn unbuttoned(&self) -> Garment {
        let mut open = self.clone();
        open.stitches.truncate(self.stitches.len() - self.buttons);
        open.buttons = 0;
        open
    }
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
    /// How high its line named for the waist is.
    waist: Option<f64>,
    /// Its line named for the centre front, where it is on the panel.
    centre: Option<Vec<P2>>,
    /// The lines drawn inside it that a piece may be sewn onto (a pocket's
    /// placement), each from its start, as `edges` are.
    lines: Vec<(Id, Half, Vec<P2>)>,
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
        let quarters = upright_turns(piece);
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
        // Where its line named for the waist is, on average.
        let waist = piece
            .lines
            .iter()
            .find(|l| {
                l.label
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains("waist"))
            })
            .map(|l| {
                let pts = flatten_segs(&segs_of(l.start, &l.segments), 0.5);
                pts.iter()
                    .map(|p| quarter_turned([p.x, -p.y], quarters)[1])
                    .sum::<f64>()
                    / pts.len().max(1) as f64
            });
        // Its centre front line, if drawn.
        let centre: Option<Vec<P2>> = piece
            .lines
            .iter()
            .find(|l| {
                l.label
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase() == "centre front")
            })
            .map(|l| {
                flatten_segs(&segs_of(l.start, &l.segments), 0.5)
                    .iter()
                    .map(|p| quarter_turned([p.x, -p.y], quarters))
                    .collect()
            });
        // Lines a piece may be sewn onto, stood upright as the edges are.
        let inner: Vec<(Id, Vec<P2>)> = piece
            .lines
            .iter()
            .filter(|l| {
                matches!(
                    l.kind,
                    LineKind::Line | LineKind::Placement | LineKind::Stitch
                )
            })
            .map(|l| {
                let points = line_points(l)
                    .iter()
                    .map(|p| quarter_turned([p.x, -p.y], quarters))
                    .collect();
                (l.id.clone(), points)
            })
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
                waist,
                centre: None,
                // On each half: as drawn, then mirrored in the fold.
                lines: inner
                    .iter()
                    .map(|(id, l)| (id.clone(), Half::A, l.clone()))
                    .chain(inner.iter().map(|(id, l)| {
                        let mirrored = l.iter().map(|&p| reflect(p, a, b)).collect();
                        (id.clone(), Half::B, mirrored)
                    }))
                    .collect(),
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
                waist,
                centre: centre
                    .as_ref()
                    .map(|c| c.iter().map(|m| [flip * m[0], m[1]]).collect()),
                lines: inner
                    .iter()
                    .map(|(id, l)| {
                        let points = l.iter().map(|p| [flip * p[0], p[1]]).collect();
                        (id.clone(), half, points)
                    })
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
fn mesh_either_way(
    chains: &[Vec<P2>],
    inner: &[Vec<P2>],
    spacing: f64,
) -> Result<PanelMesh, MeshError> {
    let round: Vec<P2> = chains
        .iter()
        .flat_map(|c| c[..c.len() - 1].iter().copied())
        .collect();
    if twice_area(&round) > 0.0 {
        return mesh_inside(chains, inner, spacing);
    }
    let backwards: Vec<Vec<P2>> = chains
        .iter()
        .rev()
        .map(|c| c.iter().rev().copied().collect())
        .collect();
    let mut m = mesh_inside(&backwards, inner, spacing)?;
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
/// differently, and it is meshed whole. The lines inside it (`inner`, half A's
/// first) are mirrored too.
fn mirrored_mesh(
    p: usize,
    layouts: &[Layout],
    (sampled, inner): (&[Vec<Vec<P2>>], &[Chains]),
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
            // The lines inside: each on half B half A's mirrored, none on
            // the fold.
            let (on_a, on_b): (Vec<_>, Vec<_>) = inner[p].iter().partition(|(h, _)| *h == Half::A);
            let on_a: Vec<Vec<P2>> = on_a.into_iter().map(|(_, c)| c.clone()).collect();
            let fold_off = |q: P2| {
                let d = [b[0] - a[0], b[1] - a[1]];
                ((q[0] - a[0]) * d[1] - (q[1] - a[1]) * d[0]).abs() / d[0].hypot(d[1]).max(1e-9)
            };
            if on_a.len() != on_b.len()
                || !on_a
                    .iter()
                    .zip(&on_b)
                    .all(|(x, (_, y))| mirrors(y, x, a, b))
                || on_a
                    .iter()
                    .flatten()
                    .any(|&q| fold_off(q) < line_clearance(spacing))
            {
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
            let m = mesh_either_way(&chains, &on_a, spacing)?;
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
            let mirrored: Vec<Vec<u32>> = m
                .inner
                .iter()
                .map(|c| c.iter().map(|&i| index[i as usize]).collect())
                .collect();
            let mut lines = m.inner.clone();
            lines.extend(mirrored);
            Ok(Some(PanelMesh {
                points,
                triangles: all,
                chains,
                inner: lines,
            }))
        }
        (Copy::Mirror, _) if p > 0 => {
            let original = p - 1;
            let (a, b) = ([0.0, 0.0], [0.0, 1.0]);
            if layouts[original].copy != Copy::Original
                || layouts[original].piece != layout.piece
                || !(0..layout.edges.len())
                    .all(|e| mirrors(&sampled[p][e], &sampled[original][e], a, b))
                || inner[p].len() != inner[original].len()
                || !inner[p]
                    .iter()
                    .zip(&inner[original])
                    .all(|((_, x), (_, y))| mirrors(x, y, a, b))
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
                inner: m.inner.clone(),
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
/// Two stretches of an edge this near each other's ends, mm, are one
/// stretch sewn in layers.
const LAYERED: f64 = 0.5;

/// Whether two seams sew the same stretch of an edge: layers sewn as one.
fn shares(x: &(Side, Side, bool, String), y: &(Side, Side, bool, String)) -> bool {
    let span = |s: &Side| (s.from.min(s.to), s.from.max(s.to));
    [&x.0, &x.1].iter().any(|s| {
        [&y.0, &y.1].iter().any(|t| {
            let ((a, b), (c, d)) = (span(s), span(t));
            s.panel == t.panel
                && s.edge == t.edge
                && (a - c).abs() <= LAYERED
                && (b - d).abs() <= LAYERED
        })
    })
}

/// Which panels are sewn into the garment's seams (a cape, its front edge
/// sewn into a princess seam): those sewn along part of an edge that
/// another seam sews more of, and the panels sewn to them but not to the
/// garment (the cape's back): not the garment itself (the most panels
/// sewn together otherwise), facings and linings apart.
fn outside_layers(
    layouts: &[Layout],
    pieces: &[Piece],
    seams: &[(Side, Side, bool, String)],
) -> Vec<bool> {
    let n = layouts.len();
    let lined: Vec<bool> = layouts.iter().map(|l| inside(&pieces[l.piece])).collect();
    let span = |s: &Side| (s.from.min(s.to), s.from.max(s.to));
    // Sewn in: the other side of a seam whose side on an edge lies within
    // another's there, shorter.
    let mut sewn_in = vec![false; n];
    for (k, (a, b, ..)) in seams.iter().enumerate() {
        for (x, other) in [(a, b), (b, a)] {
            let (lo, hi) = span(x);
            let within = seams.iter().enumerate().any(|(j, (c, d, ..))| {
                j != k
                    && [c, d].iter().any(|y| {
                        let (ylo, yhi) = span(y);
                        y.panel == x.panel
                            && y.edge == x.edge
                            && ylo <= lo + LAYERED
                            && yhi >= hi - LAYERED
                            && yhi - ylo > hi - lo + LAYERED
                    })
            });
            if within && !lined[other.panel] {
                sewn_in[other.panel] = true;
            }
        }
    }
    // Groups of panels sewn together, those sewn in apart from the rest.
    let mut group: Vec<usize> = (0..n).collect();
    let find = |group: &mut Vec<usize>, mut p: usize| {
        while group[p] != p {
            group[p] = group[group[p]];
            p = group[p];
        }
        p
    };
    for (a, b, ..) in seams {
        if sewn_in[a.panel] == sewn_in[b.panel] && !lined[a.panel] && !lined[b.panel] {
            let (x, y) = (find(&mut group, a.panel), find(&mut group, b.panel));
            group[x] = y;
        }
    }
    let of: Vec<usize> = (0..n).map(|p| find(&mut group, p)).collect();
    let mut sizes = vec![0usize; n];
    for p in (0..n).filter(|&p| !lined[p]) {
        sizes[of[p]] += 1;
    }
    let Some(garment) = (0..n).max_by_key(|&g| (sizes[g], std::cmp::Reverse(g))) else {
        return vec![false; n];
    };
    (0..n)
        .map(|p| !lined[p] && of[p] != garment && (0..n).any(|q| sewn_in[q] && of[q] == of[p]))
        .collect()
}

/// Seams sewing a piece into another seam, between its two sides (a cape's
/// edge into a princess seam, a collar between a neckline and its facing):
/// where one seam's side runs over part of another's on the same edge, the
/// longer is split where the shorter begins and ends, at the same place on
/// its other side as sewn. The stretch they share is then sewn once, all
/// its layers to the same points.
fn layered(mut seams: Vec<(Side, Side, bool, String)>) -> Vec<(Side, Side, bool, String)> {
    let span = |s: &Side| (s.from.min(s.to), s.from.max(s.to));
    // Each split makes two stretches' ends meet; there are only so many.
    for _ in 0..4 * seams.len() * seams.len() + 1 {
        let mut cut = None;
        'find: for (k, (a, b, ..)) in seams.iter().enumerate() {
            for (second, x) in [(false, a), (true, b)] {
                let (lo, hi) = span(x);
                for (j, (c, d, ..)) in seams.iter().enumerate() {
                    for y in [c, d]
                        .into_iter()
                        .filter(|y| j != k && y.panel == x.panel && y.edge == x.edge)
                    {
                        let (ylo, yhi) = span(y);
                        if ylo >= hi - LAYERED || yhi <= lo + LAYERED {
                            continue;
                        }
                        if let Some(v) = [ylo, yhi]
                            .into_iter()
                            .find(|v| *v > lo + LAYERED && *v < hi - LAYERED)
                        {
                            cut = Some((k, second, v));
                            break 'find;
                        }
                    }
                }
            }
        }
        let Some((k, second, v)) = cut else {
            break;
        };
        let (a, b, reversed, name) = seams.remove(k);
        let (x, y) = if second { (b, a) } else { (a, b) };
        // As far along `y`, as sewn: its start sewn to `x`'s start, or to
        // its end where the seam is reversed.
        let f = (v - x.from) / (x.to - x.from);
        let g = if reversed { 1.0 - f } else { f };
        let w = y.from + g * (y.to - y.from);
        let part = |s: &Side, from: f64, to: f64| Side {
            from,
            to,
            ..s.clone()
        };
        let (x1, x2) = (part(&x, x.from, v), part(&x, v, x.to));
        let (y1, y2) = if reversed {
            (part(&y, w, y.to), part(&y, y.from, w))
        } else {
            (part(&y, y.from, w), part(&y, w, y.to))
        };
        for (xp, yp) in [(x2, y2), (x1, y1)] {
            let (a, b) = if second { (yp, xp) } else { (xp, yp) };
            seams.insert(k, (a, b, reversed, name.clone()));
        }
    }
    seams
}

/// Where a stretch of a line sewn onto is in its panel's chains: (panel,
/// chain, its first point in the chain, 1 if recorded backwards).
type LineRun = (usize, usize, usize, usize);

/// Chains of points along lines inside a panel, and the half each is on.
type Chains = Vec<(Half, Vec<P2>)>;

/// The lines sewn onto (`onto`'s line sides, each in `steps` steps),
/// sampled: each panel's chains of points (each one or more stretches end
/// to end, a closed line's last joined to its first round its start), and
/// where each seam's stretch is in them. A stretch over one already sampled
/// is left out.
fn sample_lines(
    layouts: &[Layout],
    onto: &[(Side, Side, bool)],
    steps_of: &[usize],
) -> (Vec<Chains>, HashMap<usize, LineRun>) {
    let mut inner: Vec<Chains> = vec![Vec::new(); layouts.len()];
    let mut runs: HashMap<usize, LineRun> = HashMap::new();
    let same = |a: P2, b: P2| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
    for (p, layout) in layouts.iter().enumerate() {
        for (k, (_, half, points)) in layout.lines.iter().enumerate() {
            let mut asked: Vec<(f64, f64, usize, usize)> = onto
                .iter()
                .enumerate()
                .filter(|(_, (_, l, _))| l.panel == p && l.edge == k)
                .map(|(j, (_, l, _))| (l.from, l.to, steps_of[j], j))
                .collect();
            if asked.is_empty() {
                continue;
            }
            asked.sort_by(|x, y| x.0.min(x.1).total_cmp(&y.0.min(y.1)));
            let first = inner[p].len();
            let mut at = f64::NEG_INFINITY;
            for (from, to, n, j) in asked {
                let (lo, hi) = (from.min(to), from.max(to));
                if lo < at - 1e-6 {
                    continue;
                }
                let way = usize::from(from > to);
                let stretch = steps(points, lo, hi, n);
                if (lo - at).abs() <= 1e-6 {
                    let c = inner[p].len() - 1;
                    let chain = &mut inner[p][c].1;
                    let start = chain.len() - 1;
                    chain.extend(stretch.into_iter().skip(1));
                    runs.insert(j, (p, c, start, way));
                } else {
                    inner[p].push((*half, stretch));
                    runs.insert(j, (p, inner[p].len() - 1, 0, way));
                }
                at = hi;
            }
            // Round a closed line, past its start: its last chain, ending
            // where it ends, goes on into its first, starting where it
            // starts.
            let last = inner[p].len() - 1;
            let closed = points.len() > 2 && same(points[0], points[points.len() - 1]);
            if closed
                && last > first
                && same(inner[p][first].1[0], points[0])
                && same(*inner[p][last].1.last().expect("points"), points[0])
            {
                let head = inner[p].remove(first).1;
                let last = last - 1;
                let offset = inner[p][last].1.len() - 1;
                inner[p][last].1.extend(head.into_iter().skip(1));
                for r in runs.values_mut().filter(|r| r.0 == p) {
                    if r.1 == first {
                        r.1 = last;
                        r.2 += offset;
                    } else if r.1 > first {
                        r.1 -= 1;
                    }
                }
            }
        }
    }
    (inner, runs)
}

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
/// lies against what it faces, between it and the body.
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

/// Whether a piece goes between the legs (a bodysuit's crotch, a gusset).
pub fn between_legs(piece: &Piece) -> bool {
    let name = piece.name.to_lowercase();
    name.contains("crotch") || name.contains("gusset")
}

/// What a drape is made of, and what is left out of it (by id).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Draped {
    /// The pieces sewn by confirmed seams, less those left out below: the
    /// garment as seen from outside.
    pub sewn: Vec<Piece>,
    /// Facings and linings sewn by confirmed seams: inside it.
    pub lined: Vec<Piece>,
    /// Pieces sewn onto lines inside the garment's own (patch pockets onto
    /// their placements), and by nothing else: over them.
    pub on: Vec<Piece>,
    /// Sewn to nothing.
    pub alone: Vec<Id>,
    /// Facings and linings sewn to nothing: left out, as they go inside.
    pub inside: Vec<Id>,
    /// Sewn onto a piece that isn't draped, or is a facing or a lining (a
    /// pocket inside a coat, which would face the body): left out.
    pub off: Vec<Id>,
}

/// What a drape is made of: the pieces sewn by confirmed seams, facings
/// and linings apart, and pieces sewn onto others apart; and those left
/// out.
pub fn draped_pieces(pieces: &[Piece], seams: &[Seam]) -> Draped {
    let mut out = Draped::default();
    let confirmed: Vec<&Seam> = seams.iter().filter(|s| s.confirmed).collect();
    // Each piece's pieces it is sewn onto, if it is sewn onto lines alone.
    let onto = |piece: &Piece| -> Option<Vec<&str>> {
        let mine: Vec<&&Seam> = confirmed.iter().filter(|s| s.touches(&piece.id)).collect();
        let hosts: Option<Vec<&str>> = mine
            .iter()
            .map(|s| {
                let (me, other) = if s.a.piece == piece.id && !s.a.is_line() {
                    (&s.a, &s.b)
                } else {
                    (&s.b, &s.a)
                };
                (me.piece == piece.id
                    && !me.is_line()
                    && other.is_line()
                    && other.piece != piece.id)
                    .then_some(other.piece.as_str())
            })
            .collect();
        hosts.filter(|h| !h.is_empty())
    };
    let mut sewn_on: Vec<(&Piece, Vec<&str>)> = Vec::new();
    for piece in pieces {
        let sewn = confirmed.iter().any(|s| s.touches(&piece.id));
        if !inside(piece)
            && let Some(hosts) = onto(piece)
        {
            sewn_on.push((piece, hosts));
            continue;
        }
        match (inside(piece), sewn) {
            (true, true) => out.lined.push(piece.clone()),
            (true, false) => out.inside.push(piece.id.clone()),
            (false, true) => out.sewn.push(piece.clone()),
            (false, false) => out.alone.push(piece.id.clone()),
        }
    }
    // On the garment's own pieces only.
    for (piece, hosts) in sewn_on {
        if hosts.iter().all(|h| out.sewn.iter().any(|p| p.id == *h)) {
            out.on.push(piece.clone());
        } else {
            out.off.push(piece.id.clone());
        }
    }
    out
}

/// The garment as seen from outside, to place round the body and measure:
/// its own pieces (`draped.sewn`) meshed as the whole garment meshes them,
/// with the lines pieces are sewn onto, but without those pieces.
pub fn shell(draped: &Draped, seams: &[Seam], spacing: f64) -> Result<Garment, GarmentError> {
    if draped.on.is_empty() {
        return assemble(&draped.sewn, seams, spacing);
    }
    let all: Vec<Piece> = draped.sewn.iter().chain(&draped.on).cloned().collect();
    let whole = assemble(&all, seams, spacing)?;
    let own = whole
        .panels
        .iter()
        .take_while(|p| draped.sewn.iter().any(|s| s.id == p.piece))
        .count();
    Ok(whole.first(own))
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
    // And which of its lines is a given line's half.
    let find_line = |piece: &str, line: &str, half: Half| -> Option<(usize, usize)> {
        layouts.iter().enumerate().find_map(|(i, l)| {
            (pieces[l.piece].id == piece)
                .then(|| {
                    l.lines
                        .iter()
                        .position(|x| x.0 == line && x.1 == half)
                        .map(|k| (i, k))
                })
                .flatten()
        })
    };
    // The seams on panels: each seam on both sides of the body where its
    // pieces are in pairs, once where it names a mirror copy itself.
    let mut sewn: Vec<(Side, Side, bool, String)> = Vec::new();
    // Edges sewn onto lines inside panels (a pocket onto its placement):
    // the edge's side, the line's (its `edge` the line's index), and
    // whether sewn the other way round.
    let mut onto: Vec<(Side, Side, bool)> = Vec::new();
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
            let on_line = |s: &drapewright_seams::SeamSide, half: Half| -> Option<Side> {
                let half = if explicit {
                    if s.mirror { Half::B } else { Half::A }
                } else {
                    half
                };
                let (panel, line) = find_line(&s.piece, s.line.as_ref()?, half)?;
                let total = length(&layouts[panel].lines[line].2);
                Some(Side {
                    panel,
                    edge: line,
                    from: s.from.unwrap_or(0.0).clamp(0.0, total),
                    to: s.to.unwrap_or(total).clamp(0.0, total),
                })
            };
            let pair = match (seam.a.is_line(), seam.b.is_line()) {
                (false, true) => side(&seam.a, ha).zip(on_line(&seam.b, hb)),
                (true, false) => side(&seam.b, hb).zip(on_line(&seam.a, ha)),
                _ => None,
            };
            if let Some((edge, line)) = pair {
                onto.push((edge, line, seam.reversed));
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
    // Pieces sewn into the garment's seams hang outside it.
    let outside = outside_layers(&layouts, pieces, &split);
    // A piece sewn into a seam, between its two sides (a cape's edge into
    // a princess seam): its layers are sewn as one.
    let mut sewn = layered(split);
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
    // Each seam's steps, as many on both sides; layers sewn as one, as
    // many as each other.
    let mut steps_of: Vec<usize> = sewn
        .iter()
        .map(|(a, b, _, _)| {
            ((a.to - a.from).abs().max((b.to - b.from).abs()) / spacing)
                .round()
                .max(1.0) as usize
        })
        .collect();
    loop {
        let mut changed = false;
        for k in 0..sewn.len() {
            for j in 0..sewn.len() {
                if j != k && shares(&sewn[k], &sewn[j]) && steps_of[j] > steps_of[k] {
                    steps_of[k] = steps_of[j];
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    // Each edge's stretches sewn to others.
    let mut wanted: HashMap<(usize, usize), Vec<Stretch>> = HashMap::new();
    for (k, (a, b, _, _)) in sewn.iter().enumerate() {
        let n = steps_of[k];
        wanted
            .entry((a.panel, a.edge))
            .or_default()
            .push((a.from, a.to, n, k, false));
        wanted
            .entry((b.panel, b.edge))
            .or_default()
            .push((b.from, b.to, n, k, true));
    }
    // Each edge sewn onto a line, in as many steps as its stretch of the
    // line takes: the line's points are then the same whatever is sewn
    // onto it, or whether it is there at all.
    let base = sewn.len();
    let onto_steps: Vec<usize> = onto
        .iter()
        .map(|(_, l, _)| ((l.to - l.from).abs() / spacing).round().max(1.0) as usize)
        .collect();
    for (j, (e, _, _)) in onto.iter().enumerate() {
        wanted.entry((e.panel, e.edge)).or_default().push((
            e.from,
            e.to,
            onto_steps[j],
            base + j,
            false,
        ));
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
            // The stretches sampled: (low end, high end, first point).
            let mut done: Vec<(f64, f64, usize)> = Vec::new();
            let free = |points: &mut Vec<P2>, from: f64, to: f64| {
                if to - from > 1e-6 {
                    let n = ((to - from) / spacing).round().max(1.0) as usize;
                    points.extend(steps(line, from, to, n).into_iter().skip(1));
                }
            };
            for (from, to, n, k, second) in asked {
                let (lo, hi) = (from.min(to), from.max(to));
                // Recorded from its `from` to its `to`.
                let way = if from <= to { 0 } else { 1 };
                // Another layer of a stretch sewn already: its points.
                if let Some(&(_, _, start)) = done
                    .iter()
                    .find(|(l, h, _)| (l - lo).abs() <= LAYERED && (h - hi).abs() <= LAYERED)
                {
                    runs.insert((k, second), (p, e, start, way));
                    continue;
                }
                if lo < at - 1e-6 {
                    // Overlaps a stretch already sewn: left out.
                    continue;
                }
                free(&mut points, at, lo);
                let start = points.len() - 1;
                points.extend(steps(line, lo, hi, n).into_iter().skip(1));
                runs.insert((k, second), (p, e, start, way));
                done.push((lo, hi, start));
                at = hi;
            }
            free(&mut points, at, total);
            edges.push(points);
        }
        sampled.push(edges);
    }
    // The lines sewn onto, sampled: chains of points, each one or more
    // stretches end to end, and where each seam's stretch is in them.
    let (inner, line_runs) = sample_lines(&layouts, &onto, &onto_steps);
    // Each panel's boundary, round it.
    let boundaries: Vec<Vec<Vec<P2>>> = layouts
        .iter()
        .enumerate()
        .map(|(p, layout)| {
            layout
                .order
                .iter()
                .map(|&(e, back)| {
                    let mut c = sampled[p][e].clone();
                    if back {
                        c.reverse();
                    }
                    c
                })
                .collect()
        })
        .collect();
    // Only the chains inside their panels, clear of its edge: a pocket
    // whose placement runs over the edge is not sewn there. Each chain's
    // place among those kept.
    let mut kept: Vec<Chains> = Vec::new();
    let mut kept_as: Vec<Vec<Option<usize>>> = Vec::new();
    for (p, chains) in inner.into_iter().enumerate() {
        let mut here = Vec::new();
        let mut places = Vec::new();
        for (half, c) in chains {
            if inside_panel(&boundaries[p], &c, spacing) {
                places.push(Some(here.len()));
                here.push((half, c));
            } else {
                places.push(None);
            }
        }
        kept.push(here);
        kept_as.push(places);
    }
    // Meshed, each edge's points found in the mesh.
    let mut panels = Vec::new();
    for (p, layout) in layouts.iter().enumerate() {
        let chains = &boundaries[p];
        let failed = |error| GarmentError::Mesh {
            piece: pieces[layout.piece].name.clone(),
            error,
        };
        let m = match mirrored_mesh(p, &layouts, (&sampled, &kept), &panels, spacing)
            .map_err(failed)?
        {
            Some(m) => m,
            None => {
                let lines: Vec<Vec<P2>> = kept[p].iter().map(|(_, c)| c.clone()).collect();
                mesh_inside(chains, &lines, spacing).map_err(failed)?
            }
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
            waist: layout.waist,
            inside: inside(&pieces[layout.piece]),
            over: None,
            outside: outside[p],
            sewn_on: false,
        });
    }
    // Stitches: the two runs of each seam, point for point.
    let mut stitches = Vec::new();
    let mut links = Vec::new();
    for (k, (a, b, reversed, name)) in sewn.iter().enumerate() {
        let (Some(&ra), Some(&rb)) = (runs.get(&(k, false)), runs.get(&(k, true))) else {
            continue;
        };
        let n = steps_of[k];
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
    // Edges sewn onto lines, point for point.
    for (j, (e, _, reversed)) in onto.iter().enumerate() {
        let (Some(&(p, edge, start, backwards)), Some(&(q, chain, at, way))) =
            (runs.get(&(base + j, false)), line_runs.get(&j))
        else {
            continue;
        };
        let Some(chain) = kept_as[q][chain] else {
            continue;
        };
        debug_assert_eq!((p, edge), (e.panel, e.edge));
        let n = onto_steps[j];
        let points = &panels[p].edges[edge].points;
        let mut va: Vec<(usize, u32)> = (start..=start + n).map(|i| (p, points[i])).collect();
        if backwards == 1 {
            va.reverse();
        }
        let line = &panels[q].mesh.inner[chain];
        let mut vb: Vec<(usize, u32)> = (at..=at + n).map(|i| (q, line[i])).collect();
        if way == 1 {
            vb.reverse();
        }
        if *reversed {
            va.reverse();
        }
        stitches.extend(va.into_iter().zip(vb).map(|(x, y)| [x, y]));
    }
    let buttons = buttons(&layouts, pieces, &panels, &sewn[..seams_sewn]);
    let done_up = buttons.len();
    stitches.extend(buttons);
    Ok(Garment {
        panels,
        stitches,
        links,
        buttons: done_up,
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
            let piece = &pieces[layout.piece];
            if layout.copy == Copy::Unfolded
                || !piece.name.to_lowercase().contains("front")
                || inside(piece)
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
    // Fronts with no buttons marked that cross past the centre front line
    // drawn on them (a coat done up with snaps, which patterns leave to
    // the instructions): held together on that line from the top of the
    // opening down two thirds of it, every `FASTENED` mm.
    let unmarked = |p: usize| {
        openings[p]
            .as_ref()
            .is_some_and(|(_, b, h)| b.is_empty() && h.is_empty())
    };
    // The centre line, as far as it goes on, at height y.
    let centre_at = |p: usize, y: f64| -> Option<P2> {
        let c = layouts[p].centre.as_ref()?;
        let (a, b) = (c[0], c[c.len() - 1]);
        ((b[1] - a[1]).abs() > 1.0).then(|| {
            let t = (y - a[1]) / (b[1] - a[1]);
            [a[0] + t * (b[0] - a[0]), y]
        })
    };
    for p in (0..layouts.len()).filter(|&p| unmarked(p)) {
        let Some(q) = (p + 1..layouts.len()).find(|&q| {
            unmarked(q)
                && stem(&pieces[layouts[q].piece].name) == stem(&pieces[layouts[p].piece].name)
        }) else {
            continue;
        };
        let (Some(edge), Some(centre)) = (
            openings[p].as_ref().map(|o| o.0),
            layouts[p].centre.as_ref(),
        ) else {
            continue;
        };
        // Along the opening, near it: the front crosses past its centre.
        let line = &layouts[p].edges[edge].3;
        let (a, b) = (line[0], line[line.len() - 1]);
        let (c0, c1) = (centre[0], centre[centre.len() - 1]);
        let way = |u: P2, v: P2| {
            let d = [v[0] - u[0], v[1] - u[1]];
            let l = d[0].hypot(d[1]).max(1e-9);
            [d[0] / l, d[1] / l]
        };
        let (e, c) = (way(a, b), way(c0, c1));
        let off = ((c0[0] - a[0]) * e[1] - (c0[1] - a[1]) * e[0]).abs();
        if (e[0] * c[0] + e[1] * c[1]).abs() < 0.98 || off > BUTTON_STAND {
            continue;
        }
        let (bottom, top) = (a[1].min(b[1]), a[1].max(b[1]));
        let mut y = top - FIRST_FASTENING;
        while y >= top - FASTENED_SHARE * (top - bottom) {
            if let (Some(x), Some(z)) = (centre_at(p, y), centre_at(q, y)) {
                out.push([(p, nearest(p, x)), (q, nearest(q, z))]);
            }
            y -= FASTENED;
        }
    }
    out
}

/// Fronts done up at their centre front line: the first fastening this far
/// below the top of the opening, the rest this far apart, down this share
/// of it, mm.
const FIRST_FASTENING: f64 = 15.0;
const FASTENED: f64 = 100.0;
const FASTENED_SHARE: f64 = 2.0 / 3.0;
