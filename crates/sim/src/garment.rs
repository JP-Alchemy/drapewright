//! A garment ready to drape: each piece's cut copies laid flat (a pair as
//! its two mirror images, a piece cut on the fold unfolded whole), their
//! edges sampled so that both sides of every seam have matching points,
//! meshed, and the seams as pairs of points to sew together.
//!
//! Flat, panels are in millimetres as drawn but with y up (the pattern's
//! y runs down), so that up the panel is up the body.

use std::collections::HashMap;
use std::fmt;

use drapewright_pattern_core::stitching::stitching_edges;
use drapewright_pattern_core::{EdgeKind, Id, Piece};
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
        let lines: Vec<Vec<P2>> = stitching_edges(piece)
            .into_iter()
            .map(|e| e.into_iter().map(|p| [p.x, -p.y]).collect())
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
            [d[0] / l, d[1] / l]
        });
        let cut = piece
            .cut
            .iter()
            .find(|c| c.material == "main")
            .or(piece.cut.first())
            .cloned()
            .unwrap_or_default();
        let fold = kinds.iter().position(|(_, kind)| *kind == EdgeKind::Fold);
        let on_fold = cut.on_fold.unwrap_or(fold.is_some()) && fold.is_some();
        let pair = cut.quantity >= 2 && cut.mirrored.unwrap_or(true);
        let n = lines.len();
        if on_fold {
            let f = fold.expect("checked");
            let (a, b) = (lines[f][0], lines[f][lines[f].len() - 1]);
            let mut edges = Vec::new();
            let mut order = Vec::new();
            // Half A from the fold's end round to its start, as drawn ...
            for step in 1..n {
                let e = (f + step) % n;
                edges.push((kinds[e].0.clone(), Half::A, kinds[e].1, lines[e].clone()));
                order.push((edges.len() - 1, false));
            }
            // ... then half B, mirrored in the fold, back round to its end.
            for step in (1..n).rev() {
                let e = (f + step) % n;
                let mirrored = lines[e].iter().map(|&p| reflect(p, a, b)).collect();
                edges.push((kinds[e].0.clone(), Half::B, kinds[e].1, mirrored));
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
        for (x, y) in va.into_iter().zip(vb) {
            stitches.push([x, y]);
        }
        let half = |s: &Side| layouts[s.panel].edges[s.edge].1;
        links.push(Link {
            a: (a.panel, layouts[a.panel].edges[a.edge].0.clone(), half(a)),
            b: (b.panel, layouts[b.panel].edges[b.edge].0.clone(), half(b)),
            name: name.clone(),
        });
    }
    Ok(Garment {
        panels,
        stitches,
        links,
    })
}
