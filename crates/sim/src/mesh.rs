//! Meshing a panel: its boundary exactly as given, so that the points of
//! a seam's two sides match, and any lines inside it as given too (where a
//! pocket is sewn onto it), and inside an even lattice of points, joined by
//! a constrained Delaunay triangulation. The triangles come out near
//! equilateral and much the same size, as cloth simulation likes.

use std::fmt;

use spade::{
    AngleLimit, ConstrainedDelaunayTriangulation, Point2, RefinementParameters, Triangulation,
};

/// A panel's triangles.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PanelMesh {
    /// Points, mm, in the panel's plane.
    pub points: Vec<[f64; 2]>,
    /// Triangles, counter-clockwise.
    pub triangles: Vec<[u32; 3]>,
    /// For each boundary chain given, its points' indices, first to last.
    pub chains: Vec<Vec<u32>>,
    /// For each chain given inside the panel, likewise.
    pub inner: Vec<Vec<u32>>,
}

/// Why a panel could not be meshed.
#[derive(Clone, Debug, PartialEq)]
pub enum MeshError {
    /// The chains do not join end to start, all the way round.
    NotClosed,
    /// The boundary runs clockwise, or encloses nothing.
    Clockwise,
    /// The boundary touches or crosses itself.
    Tangled,
    /// A line inside the panel runs out of it, or too near its edge.
    LineOutside,
}

impl fmt::Display for MeshError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            MeshError::NotClosed => "the panel's outline is not closed",
            MeshError::Clockwise => "the panel's outline runs the wrong way round",
            MeshError::Tangled => "the panel's outline crosses itself",
            MeshError::LineOutside => "a line on the panel runs over its edge",
        })
    }
}

impl std::error::Error for MeshError {}

/// Twice the signed area of a closed loop (counter-clockwise positive).
fn twice_area(loop_: &[[f64; 2]]) -> f64 {
    (0..loop_.len())
        .map(|i| {
            let (a, b) = (loop_[i], loop_[(i + 1) % loop_.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum()
}

/// Whether `p` is inside the closed loop (crossing number).
fn inside(loop_: &[[f64; 2]], p: [f64; 2]) -> bool {
    let mut odd = false;
    for i in 0..loop_.len() {
        let (a, b) = (loop_[i], loop_[(i + 1) % loop_.len()]);
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if x > p[0] {
                odd = !odd;
            }
        }
    }
    odd
}

/// Distance from `p` to the segment `a`–`b`.
fn to_segment(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l = dx * dx + dy * dy;
    let t = if l > 0.0 {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p[0] - a[0] - t * dx).hypot(p[1] - a[1] - t * dy)
}

/// Meshes the panel bounded by `chains`, which run counter-clockwise, each
/// ending where the next begins (the last where the first begins). Inside,
/// points lie `spacing` mm apart.
pub fn mesh(chains: &[Vec<[f64; 2]>], spacing: f64) -> Result<PanelMesh, MeshError> {
    mesh_inside(chains, &[], spacing)
}

/// How near a panel's edge a line inside it may come, for `spacing`: as
/// near as the lattice comes.
pub fn line_clearance(spacing: f64) -> f64 {
    0.45 * spacing
}

/// Whether a chain of points lies inside the boundary `chains`, clear of it
/// (see `line_clearance`): a line that can be meshed inside the panel.
pub fn inside_panel(chains: &[Vec<[f64; 2]>], line: &[[f64; 2]], spacing: f64) -> bool {
    let boundary: Vec<[f64; 2]> = chains
        .iter()
        .flat_map(|c| c[..c.len().saturating_sub(1)].iter().copied())
        .collect();
    let keep = line_clearance(spacing);
    let n = boundary.len();
    let clear = |p: [f64; 2]| {
        inside(&boundary, p)
            && (0..n).all(|i| to_segment(p, boundary[i], boundary[(i + 1) % n]) >= keep)
    };
    // Each point, and each segment's middle.
    line.iter().all(|&p| clear(p))
        && line
            .windows(2)
            .all(|w| clear([(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0]))
}

/// Meshes the panel bounded by `chains` (as `mesh` does) with the chains
/// `inner` inside it kept as they are: their points are points of the mesh,
/// joined in order by triangles' sides. Chains may share their ends.
pub fn mesh_inside(
    chains: &[Vec<[f64; 2]>],
    inner: &[Vec<[f64; 2]>],
    spacing: f64,
) -> Result<PanelMesh, MeshError> {
    let close = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-6;
    if chains.is_empty() || chains.iter().any(|c| c.len() < 2) {
        return Err(MeshError::NotClosed);
    }
    for (k, chain) in chains.iter().enumerate() {
        let next = &chains[(k + 1) % chains.len()];
        if !close(chain[chain.len() - 1], next[0]) {
            return Err(MeshError::NotClosed);
        }
    }
    // The boundary once round: each chain without its last point, which
    // is the next chain's first.
    let mut boundary: Vec<[f64; 2]> = Vec::new();
    let mut starts = Vec::new();
    for chain in chains {
        starts.push(boundary.len() as u32);
        boundary.extend_from_slice(&chain[..chain.len() - 1]);
    }
    let n = boundary.len() as u32;
    if twice_area(&boundary) <= 0.0 {
        return Err(MeshError::Clockwise);
    }
    let chain_indices: Vec<Vec<u32>> = chains
        .iter()
        .zip(&starts)
        .map(|(chain, &start)| (0..chain.len() as u32).map(|k| (start + k) % n).collect())
        .collect();

    let mut cdt = ConstrainedDelaunayTriangulation::<Point2<f64>>::new();
    // Each of spade's vertices, by its index, as ours.
    let mut ours: Vec<u32> = Vec::new();
    let mut points: Vec<[f64; 2]> = Vec::new();
    let mut add = |cdt: &mut ConstrainedDelaunayTriangulation<Point2<f64>>,
                   p: [f64; 2]|
     -> Result<(spade::handles::FixedVertexHandle, u32), MeshError> {
        let handle = cdt
            .insert(Point2::new(p[0], p[1]))
            .map_err(|_| MeshError::Tangled)?;
        if handle.index() < ours.len() {
            // Landed on a point already there.
            return Err(MeshError::Tangled);
        }
        let index = points.len() as u32;
        ours.push(index);
        points.push(p);
        Ok((handle, index))
    };
    let handles: Vec<_> = boundary
        .iter()
        .map(|&p| add(&mut cdt, p).map(|(h, _)| h))
        .collect::<Result<_, _>>()?;
    for i in 0..handles.len() {
        let (a, b) = (handles[i], handles[(i + 1) % handles.len()]);
        if cdt.can_add_constraint(a, b) {
            cdt.add_constraint(a, b);
        } else {
            return Err(MeshError::Tangled);
        }
    }

    // The lines inside, each point once (chains may share their ends).
    if inner.iter().any(|c| !inside_panel(chains, c, spacing)) {
        return Err(MeshError::LineOutside);
    }
    let mut inner_indices: Vec<Vec<u32>> = Vec::new();
    let mut inner_points: Vec<([f64; 2], spade::handles::FixedVertexHandle, u32)> = Vec::new();
    for chain in inner {
        let mut indices = Vec::new();
        let mut chain_handles = Vec::new();
        for &p in chain {
            let known = inner_points
                .iter()
                .find(|(q, ..)| (q[0] - p[0]).hypot(q[1] - p[1]) < 1e-9);
            let (handle, index) = match known {
                Some(&(_, h, i)) => (h, i),
                None => {
                    let (h, index) = add(&mut cdt, p).map_err(|_| MeshError::LineOutside)?;
                    inner_points.push((p, h, index));
                    (h, index)
                }
            };
            indices.push(index);
            chain_handles.push(handle);
        }
        for w in chain_handles.windows(2) {
            if w[0] == w[1] {
                continue;
            }
            if cdt.can_add_constraint(w[0], w[1]) {
                cdt.add_constraint(w[0], w[1]);
            } else {
                return Err(MeshError::LineOutside);
            }
        }
        inner_indices.push(indices);
    }
    let near_line = |p: [f64; 2], keep: f64| {
        inner
            .iter()
            .any(|c| c.windows(2).any(|w| to_segment(p, w[0], w[1]) < keep))
    };

    // Inside: rows of points, each row half a step along from the last,
    // each even about the panel's middle (so a piece cut on the fold is
    // meshed alike either side of it).
    let (mut low, mut high) = ([f64::MAX; 2], [f64::MIN; 2]);
    for p in &boundary {
        for k in 0..2 {
            low[k] = low[k].min(p[k]);
            high[k] = high[k].max(p[k]);
        }
    }
    let row = spacing * 3f64.sqrt() / 2.0;
    let keep = line_clearance(spacing);
    let middle = (low[0] + high[0]) / 2.0;
    let mut v = low[1] + row / 2.0;
    let mut odd = false;
    while v < high[1] {
        let half = if odd { 0.0 } else { spacing / 2.0 };
        let mut u = middle + half - spacing * ((middle + half - low[0]) / spacing).floor();
        while u < high[0] {
            let p = [u, v];
            let clear = (0..boundary.len())
                .all(|i| to_segment(p, boundary[i], boundary[(i + 1) % boundary.len()]) >= keep)
                && !near_line(p, keep);
            if clear && inside(&boundary, p) {
                add(&mut cdt, p)?;
            }
            u += spacing;
        }
        v += row;
        odd = !odd;
    }

    // Where the lattice meets the boundary, a few points more to keep the
    // triangles from going thin; the boundary itself is never split.
    let lattice = points.len();
    cdt.refine(
        RefinementParameters::new()
            .keep_constraint_edges()
            .exclude_outer_faces(true)
            .with_angle_limit(AngleLimit::from_deg(28.0))
            .with_max_additional_vertices(lattice / 4 + 16),
    );
    for v in cdt.vertices().skip(ours.len()) {
        let p = v.position();
        ours.push(points.len() as u32);
        points.push([p.x, p.y]);
    }

    // Inside the boundary, and not flat: three points along a straight edge
    // can make a triangle of no area lying on it.
    let triangles: Vec<[u32; 3]> = cdt
        .inner_faces()
        .filter(|f| {
            let c = f.center();
            inside(&boundary, [c.x, c.y])
        })
        .map(|f| f.vertices().map(|v| ours[v.fix().index()]))
        .filter(|t| {
            let [a, b, c] = t.map(|i| points[i as usize]);
            (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]) > 1e-6 * spacing * spacing
        })
        .collect();
    Ok(PanelMesh {
        points,
        triangles,
        chains: chain_indices,
        inner: inner_indices,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A straight side from `a` to `b` in `n` equal steps.
    fn side(a: [f64; 2], b: [f64; 2], n: usize) -> Vec<[f64; 2]> {
        (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
            })
            .collect()
    }

    fn area(m: &PanelMesh) -> f64 {
        m.triangles
            .iter()
            .map(|t| twice_area(&t.map(|i| m.points[i as usize])) / 2.0)
            .sum()
    }

    fn smallest_angle(m: &PanelMesh) -> f64 {
        let mut least = f64::MAX;
        for t in &m.triangles {
            let p = t.map(|i| m.points[i as usize]);
            for k in 0..3 {
                let (a, b, c) = (p[k], p[(k + 1) % 3], p[(k + 2) % 3]);
                let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - a[0], c[1] - a[1]]);
                let cos = (u[0] * v[0] + u[1] * v[1]) / (u[0].hypot(u[1]) * v[0].hypot(v[1]));
                least = least.min(cos.clamp(-1.0, 1.0).acos().to_degrees());
            }
        }
        least
    }

    #[test]
    fn a_rectangle_is_filled_evenly_and_keeps_its_boundary() {
        let chains = vec![
            side([0.0, 0.0], [400.0, 0.0], 20),
            side([400.0, 0.0], [400.0, 600.0], 30),
            side([400.0, 600.0], [0.0, 600.0], 20),
            side([0.0, 600.0], [0.0, 0.0], 30),
        ];
        let m = mesh(&chains, 20.0).unwrap();
        assert!((area(&m) - 240_000.0).abs() < 1e-6, "{}", area(&m));
        assert!(
            m.triangles
                .iter()
                .all(|t| twice_area(&t.map(|i| m.points[i as usize])) > 0.0)
        );
        assert!(smallest_angle(&m) > 22.0, "{}", smallest_angle(&m));
        // The chains are the points given, in order.
        for (chain, indices) in chains.iter().zip(&m.chains) {
            let got: Vec<[f64; 2]> = indices.iter().map(|&i| m.points[i as usize]).collect();
            assert_eq!(&got, chain);
        }
        // About one point for each 20 mm × 17 mm of cloth.
        assert!((600..800).contains(&m.points.len()), "{}", m.points.len());
    }

    #[test]
    fn an_l_shape_is_meshed_inside_only() {
        let chains = vec![
            side([0.0, 0.0], [300.0, 0.0], 15),
            side([300.0, 0.0], [300.0, 100.0], 5),
            side([300.0, 100.0], [100.0, 100.0], 10),
            side([100.0, 100.0], [100.0, 300.0], 10),
            side([100.0, 300.0], [0.0, 300.0], 5),
            side([0.0, 300.0], [0.0, 0.0], 15),
        ];
        let m = mesh(&chains, 20.0).unwrap();
        assert!((area(&m) - 50_000.0).abs() < 1e-6, "{}", area(&m));
        assert!(smallest_angle(&m) > 20.0, "{}", smallest_angle(&m));
    }

    #[test]
    fn broken_or_backwards_outlines_are_refused() {
        let open = vec![
            side([0.0, 0.0], [100.0, 0.0], 5),
            side([100.0, 0.0], [0.0, 50.0], 5),
        ];
        assert_eq!(mesh(&open, 20.0), Err(MeshError::NotClosed));
        let clockwise = vec![
            side([0.0, 0.0], [0.0, 100.0], 5),
            side([0.0, 100.0], [100.0, 100.0], 5),
            side([100.0, 100.0], [0.0, 0.0], 7),
        ];
        assert_eq!(mesh(&clockwise, 20.0), Err(MeshError::Clockwise));
    }
}
