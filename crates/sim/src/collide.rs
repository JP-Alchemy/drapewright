//! Keeping cloth off the body: the body's signed distance (negative inside)
//! on a grid of cells, worked out once, then read by interpolation. Exact
//! near the surface; further off, inside or outside, as a flood from the
//! grid's sides finds it. A body that moves is felt as it goes between two
//! such fields, each worked out only round the cloth.

use std::collections::VecDeque;

use crate::cloth::{Collider, V3};

/// The body's distance field. Metres.
#[derive(Clone, Debug)]
pub struct BodyField {
    low: V3,
    cell: f32,
    size: [usize; 3],
    /// Distance at each grid point, x fastest.
    d: Vec<f32>,
    /// Each field made has a number of its own.
    id: u64,
}

/// Fields made, counted.
static FIELDS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn next_id() -> u64 {
    FIELDS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// A field's grid, as a solver elsewhere reads it.
#[derive(Clone, Copy, Debug)]
pub struct Grid<'a> {
    /// Its number: another field has another.
    pub id: u64,
    /// Its first grid point, m.
    pub low: V3,
    /// Between grid points, m.
    pub cell: f32,
    pub size: [usize; 3],
    /// Distance at each grid point (m), x fastest.
    pub values: &'a [f32],
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Which part of a triangle is nearest a point: a corner, an edge (from
/// corner k to the next), or inside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Feature {
    Corner(usize),
    Edge(usize),
    Face,
}

/// The point of triangle `t` nearest `p`, and where on the triangle it is
/// (Ericson, Real-Time Collision Detection, 5.1.5).
fn nearest_on_triangle(p: V3, t: [V3; 3]) -> (V3, Feature) {
    let [a, b, c] = t;
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, Feature::Corner(0));
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (b, Feature::Corner(1));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (
            [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]],
            Feature::Edge(0),
        );
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (c, Feature::Corner(2));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (
            [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]],
            Feature::Edge(2),
        );
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let bc = sub(c, b);
        return (
            [b[0] + w * bc[0], b[1] + w * bc[1], b[2] + w * bc[2]],
            Feature::Edge(1),
        );
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    (
        [
            a[0] + ab[0] * v + ac[0] * w,
            a[1] + ab[1] * v + ac[1] * w,
            a[2] + ab[2] * v + ac[2] * w,
        ],
        Feature::Face,
    )
}

fn unit(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-12 { a.map(|c| c / l) } else { a }
}

/// A closed surface's triangles, with each one's edges numbered, so that
/// which side of it a point is can be told from the nearest part of it.
#[derive(Clone, Debug)]
pub struct Surface {
    pub triangles: Vec<[u32; 3]>,
    /// Each triangle's edges, from corner k to the next, by number.
    edges: Vec<[u32; 3]>,
    edge_count: usize,
    /// Holes closed: each one's rim, its middle a point of its own after
    /// the surface's own (from `first_middle`).
    rims: Vec<Vec<u32>>,
    first_middle: u32,
}

impl Surface {
    pub fn new(triangles: Vec<[u32; 3]>) -> Surface {
        Surface::closed(triangles, Vec::new(), 0)
    }

    /// The closed surface `triangles` over `points` points without those
    /// `left_out`, each hole that leaves closed by a fan from the middle
    /// of its rim: a simpler surface where detail doesn't matter.
    pub fn without(
        triangles: &[[u32; 3]],
        points: usize,
        left_out: impl Fn(&[u32; 3]) -> bool,
    ) -> Surface {
        let kept: Vec<[u32; 3]> = triangles.iter().filter(|t| !left_out(t)).copied().collect();
        let sides: std::collections::HashSet<(u32, u32)> = kept
            .iter()
            .flat_map(|t| [0, 1, 2].map(|k| (t[k], t[(k + 1) % 3])))
            .collect();
        let rim_sides: Vec<(u32, u32)> = sides
            .iter()
            .filter(|&&(a, b)| !sides.contains(&(b, a)))
            .copied()
            .collect();
        // Rims: the sides left open, joined where they meet.
        let mut group: std::collections::HashMap<u32, u32> = std::collections::HashMap::new();
        fn root(group: &mut std::collections::HashMap<u32, u32>, v: u32) -> u32 {
            let mut r = v;
            while let Some(&up) = group.get(&r) {
                if up == r {
                    break;
                }
                r = up;
            }
            group.insert(v, r);
            r
        }
        for &(a, b) in &rim_sides {
            group.entry(a).or_insert(a);
            group.entry(b).or_insert(b);
            let (ra, rb) = (root(&mut group, a), root(&mut group, b));
            if ra != rb {
                group.insert(ra.max(rb), ra.min(rb));
            }
        }
        let mut roots: Vec<u32> = Vec::new();
        let mut rims: Vec<Vec<u32>> = Vec::new();
        let mut closing = kept;
        let first_middle = points as u32;
        let mut sorted = rim_sides;
        sorted.sort_unstable();
        for (a, b) in sorted {
            let r = root(&mut group, a);
            let rim = match roots.iter().position(|&x| x == r) {
                Some(i) => i,
                None => {
                    roots.push(r);
                    rims.push(Vec::new());
                    rims.len() - 1
                }
            };
            if !rims[rim].contains(&a) {
                rims[rim].push(a);
            }
            closing.push([b, a, first_middle + rim as u32]);
        }
        Surface::closed(closing, rims, first_middle)
    }

    fn closed(triangles: Vec<[u32; 3]>, rims: Vec<Vec<u32>>, first_middle: u32) -> Surface {
        let mut numbers = std::collections::HashMap::new();
        let edges = triangles
            .iter()
            .map(|t| {
                [0, 1, 2].map(|k| {
                    let (a, b) = (t[k], t[(k + 1) % 3]);
                    let next = numbers.len() as u32;
                    *numbers.entry((a.min(b), a.max(b))).or_insert(next)
                })
            })
            .collect();
        Surface {
            triangles,
            edges,
            edge_count: numbers.len(),
            rims,
            first_middle,
        }
    }

    /// `points` with the holes' middles after them, as the surface's
    /// triangles number them.
    fn with_middles<'a>(&self, points: &'a [V3]) -> std::borrow::Cow<'a, [V3]> {
        if self.rims.is_empty() {
            return std::borrow::Cow::Borrowed(points);
        }
        let mut all = points[..self.first_middle as usize].to_vec();
        for rim in &self.rims {
            let mut sum = [0.0; 3];
            for &v in rim {
                let p = points[v as usize];
                sum = [sum[0] + p[0], sum[1] + p[1], sum[2] + p[2]];
            }
            all.push(sum.map(|c| c / rim.len() as f32));
        }
        std::borrow::Cow::Owned(all)
    }

    /// The surface's normals over `points` for telling sides: each
    /// triangle's, each edge's (its two triangles' together) and each
    /// corner's (its triangles' by their angles there): pseudo-normals, as
    /// Bærentzen and Aanæs, Signed distance computation using the angle
    /// weighted pseudonormal (2005).
    fn normals(&self, points: &[V3]) -> Normals {
        let mut face = Vec::with_capacity(self.triangles.len());
        let mut edge = vec![[0.0; 3]; self.edge_count];
        let mut corner = vec![[0.0; 3]; points.len()];
        for (t, edges) in self.triangles.iter().zip(&self.edges) {
            let p = t.map(|i| points[i as usize]);
            let n = unit(cross(sub(p[1], p[0]), sub(p[2], p[0])));
            face.push(n);
            for k in 0..3 {
                let e = &mut edge[edges[k] as usize];
                *e = [e[0] + n[0], e[1] + n[1], e[2] + n[2]];
                let (u, v) = (
                    unit(sub(p[(k + 1) % 3], p[k])),
                    unit(sub(p[(k + 2) % 3], p[k])),
                );
                let angle = dot(u, v).clamp(-1.0, 1.0).acos();
                let c = &mut corner[t[k] as usize];
                *c = [0, 1, 2].map(|i| c[i] + angle * n[i]);
            }
        }
        Normals { face, edge, corner }
    }
}

struct Normals {
    face: Vec<V3>,
    edge: Vec<V3>,
    corner: Vec<V3>,
}

/// The steps from a grid point to its six neighbours.
const NEIGHBOURS: [(isize, isize, isize); 6] = [
    (1, 0, 0),
    (-1, 0, 0),
    (0, 1, 0),
    (0, -1, 0),
    (0, 0, 1),
    (0, 0, -1),
];

impl BodyField {
    /// The field of the closed surface `triangles` over `points` (m, facing
    /// outward), on cells `cell` m across, reaching `margin` m beyond it.
    pub fn new(points: &[V3], triangles: &[[u32; 3]], cell: f32, margin: f32) -> BodyField {
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for t in triangles {
            for &i in t {
                let p = points[i as usize];
                for k in 0..3 {
                    low[k] = low[k].min(p[k]);
                    high[k] = high[k].max(p[k]);
                }
            }
        }
        // Cells laid evenly about the body's middle, so that a body alike on
        // both sides is felt alike on both.
        let half = [0, 1, 2].map(|k| ((high[k] - low[k]) / 2.0 + margin) / cell);
        let half = half.map(|h| h.ceil() as usize);
        let low = [0, 1, 2].map(|k| (low[k] + high[k]) / 2.0 - half[k] as f32 * cell);
        let size = half.map(|h| 2 * h + 1);
        let index = |i: usize, j: usize, k: usize| i + size[0] * (j + size[1] * k);
        let at = |i: usize, j: usize, k: usize| {
            [
                low[0] + i as f32 * cell,
                low[1] + j as f32 * cell,
                low[2] + k as f32 * cell,
            ]
        };
        // Near the surface: the distance to the nearest triangle, and which
        // side of it the point is.
        let band = 3.0 * cell;
        let mut d = vec![f32::MAX; size[0] * size[1] * size[2]];
        let mut side = vec![0i8; d.len()];
        for t in triangles {
            let p = t.map(|i| points[i as usize]);
            let normal = cross(sub(p[1], p[0]), sub(p[2], p[0]));
            let range = |k: usize| {
                let lo = p.iter().map(|q| q[k]).fold(f32::MAX, f32::min) - band;
                let hi = p.iter().map(|q| q[k]).fold(f32::MIN, f32::max) + band;
                let a = ((lo - low[k]) / cell).floor().max(0.0) as usize;
                let b = (((hi - low[k]) / cell).ceil() as usize).min(size[k] - 1);
                a..=b
            };
            for k in range(2) {
                for j in range(1) {
                    for i in range(0) {
                        let q = at(i, j, k);
                        let (near, _) = nearest_on_triangle(q, p);
                        let off = sub(q, near);
                        let dist = dot(off, off).sqrt();
                        let n = index(i, j, k);
                        if dist < d[n] {
                            d[n] = dist;
                            side[n] = if dot(off, normal) >= 0.0 { 1 } else { -1 };
                        }
                    }
                }
            }
        }
        // Further off: outside if reached from the grid's sides without
        // passing through the surface.
        let mut outside = vec![false; d.len()];
        let mut queue = VecDeque::new();
        let blocked = |n: usize, d: &[f32]| d[n] < 0.75 * cell;
        for k in 0..size[2] {
            for j in 0..size[1] {
                for i in 0..size[0] {
                    let edge = i == 0
                        || j == 0
                        || k == 0
                        || i == size[0] - 1
                        || j == size[1] - 1
                        || k == size[2] - 1;
                    let n = index(i, j, k);
                    if edge && !blocked(n, &d) {
                        outside[n] = true;
                        queue.push_back((i, j, k));
                    }
                }
            }
        }
        while let Some((i, j, k)) = queue.pop_front() {
            for (di, dj, dk) in NEIGHBOURS {
                let (a, b, c) = (i as isize + di, j as isize + dj, k as isize + dk);
                if a < 0 || b < 0 || c < 0 {
                    continue;
                }
                let (a, b, c) = (a as usize, b as usize, c as usize);
                if a >= size[0] || b >= size[1] || c >= size[2] {
                    continue;
                }
                let n = index(a, b, c);
                if !outside[n] && !blocked(n, &d) {
                    outside[n] = true;
                    queue.push_back((a, b, c));
                }
            }
        }
        for n in 0..d.len() {
            let dist = d[n].min(band);
            d[n] = if blocked(n, &d) {
                // At the surface: the nearest triangle says which side.
                dist * f32::from(side[n])
            } else if outside[n] {
                dist
            } else {
                -dist
            };
        }
        BodyField {
            low,
            cell,
            size,
            d,
            id: next_id(),
        }
    }

    /// The field of `surface` over `points` (m) in the box from `low` to
    /// `high` only, on cells `cell` m across: exact to `band` m from the
    /// surface, and further off only which side of it, as the nearest part
    /// of the surface says. Quicker than `new` for a body that keeps
    /// moving, where only the cloth's surroundings matter.
    pub fn around(
        points: &[V3],
        surface: &Surface,
        (low, high): (V3, V3),
        cell: f32,
        band: f32,
    ) -> BodyField {
        let size = [0, 1, 2].map(|k| (((high[k] - low[k]) / cell).ceil() as usize).max(1) + 1);
        let index = |i: usize, j: usize, k: usize| i + size[0] * (j + size[1] * k);
        let points = surface.with_middles(points);
        let points = &points[..];
        let normals = surface.normals(points);
        let mut d = vec![f32::MAX; size[0] * size[1] * size[2]];
        let mut side = vec![0i8; d.len()];
        for (t, tri) in surface.triangles.iter().enumerate() {
            let p = tri.map(|i| points[i as usize]);
            let mut range = [0..=0, 0..=0, 0..=0];
            let mut inside = true;
            for (k, r) in range.iter_mut().enumerate() {
                let lo = p.iter().map(|q| q[k]).fold(f32::MAX, f32::min) - band;
                let hi = p.iter().map(|q| q[k]).fold(f32::MIN, f32::max) + band;
                if hi < low[k] || lo > high[k] {
                    inside = false;
                    break;
                }
                let a = ((lo - low[k]) / cell).floor().max(0.0) as usize;
                let b = (((hi - low[k]) / cell).ceil().max(0.0) as usize).min(size[k] - 1);
                *r = a..=b;
            }
            if !inside {
                continue;
            }
            let [ri, rj, rk] = range;
            let face = normals.face[t];
            for k in rk {
                for j in rj.clone() {
                    for i in ri.clone() {
                        let q = [
                            low[0] + i as f32 * cell,
                            low[1] + j as f32 * cell,
                            low[2] + k as f32 * cell,
                        ];
                        let n = index(i, j, k);
                        // No nearer than the triangle's plane.
                        if dot(sub(q, p[0]), face).abs() >= d[n].min(band) {
                            continue;
                        }
                        let (near, feature) = nearest_on_triangle(q, p);
                        let off = sub(q, near);
                        let dist = dot(off, off).sqrt();
                        if dist < d[n] {
                            d[n] = dist;
                            let normal = match feature {
                                Feature::Corner(c) => normals.corner[tri[c] as usize],
                                Feature::Edge(e) => normals.edge[surface.edges[t][e] as usize],
                                Feature::Face => normals.face[t],
                            };
                            side[n] = if dot(off, normal) >= 0.0 { 1 } else { -1 };
                        }
                    }
                }
            }
        }
        // Further off: the side of the nearest point near the surface, found
        // by spreading out from there; the surface between keeps inside from
        // outside. (A triangle's nearest point beyond the band may not be
        // the surface's.)
        let mut queue = VecDeque::new();
        for k in 0..size[2] {
            for j in 0..size[1] {
                for i in 0..size[0] {
                    let n = index(i, j, k);
                    if d[n] < band {
                        queue.push_back((i, j, k));
                    } else {
                        side[n] = 0;
                    }
                }
            }
        }
        while let Some((i, j, k)) = queue.pop_front() {
            let s = side[index(i, j, k)];
            for (di, dj, dk) in NEIGHBOURS {
                let (a, b, c) = (i as isize + di, j as isize + dj, k as isize + dk);
                if a < 0 || b < 0 || c < 0 {
                    continue;
                }
                let (a, b, c) = (a as usize, b as usize, c as usize);
                if a >= size[0] || b >= size[1] || c >= size[2] {
                    continue;
                }
                let n = index(a, b, c);
                if d[n] >= band && side[n] == 0 {
                    side[n] = s;
                    queue.push_back((a, b, c));
                }
            }
        }
        for (v, s) in d.iter_mut().zip(&side) {
            // Nowhere near the surface at all: outside.
            let s = if *s == 0 { 1.0 } else { f32::from(*s) };
            *v = v.min(band) * s;
        }
        BodyField {
            low,
            cell,
            size,
            d,
            id: next_id(),
        }
    }

    /// The field's grid, for a solver elsewhere.
    pub fn grid(&self) -> Grid<'_> {
        Grid {
            id: self.id,
            low: self.low,
            cell: self.cell,
            size: self.size,
            values: &self.d,
        }
    }

    fn value(&self, i: usize, j: usize, k: usize) -> f32 {
        self.d[i + self.size[0] * (j + self.size[1] * k)]
    }

    /// The distance at `p` by trilinear interpolation, and its gradient.
    fn sample(&self, p: V3) -> Option<(f32, V3)> {
        let g = [0, 1, 2].map(|k| (p[k] - self.low[k]) / self.cell);
        if (0..3).any(|k| g[k] < 0.0 || g[k] >= (self.size[k] - 1) as f32) {
            return None;
        }
        let c = g.map(|v| v.floor() as usize);
        let f = [0, 1, 2].map(|k| g[k] - c[k] as f32);
        let v = |a: usize, b: usize, e: usize| self.value(c[0] + a, c[1] + b, c[2] + e);
        let (v000, v100, v010, v110) = (v(0, 0, 0), v(1, 0, 0), v(0, 1, 0), v(1, 1, 0));
        let (v001, v101, v011, v111) = (v(0, 0, 1), v(1, 0, 1), v(0, 1, 1), v(1, 1, 1));
        let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
        let x00 = lerp(v000, v100, f[0]);
        let x10 = lerp(v010, v110, f[0]);
        let x01 = lerp(v001, v101, f[0]);
        let x11 = lerp(v011, v111, f[0]);
        let y0 = lerp(x00, x10, f[1]);
        let y1 = lerp(x01, x11, f[1]);
        let d = lerp(y0, y1, f[2]);
        let gx = lerp(
            lerp(v100 - v000, v110 - v010, f[1]),
            lerp(v101 - v001, v111 - v011, f[1]),
            f[2],
        );
        let gy = lerp(x10 - x00, x11 - x01, f[2]);
        let gz = y1 - y0;
        Some((d, [gx / self.cell, gy / self.cell, gz / self.cell]))
    }
}

/// A distance and the way out there, from a gradient.
fn outwards(d: f32, g: V3) -> (f32, V3) {
    let l = dot(g, g).sqrt();
    let n = if l > 1e-6 {
        g.map(|c| c / l)
    } else {
        [0.0, 1.0, 0.0]
    };
    (d, n)
}

impl Collider for BodyField {
    fn distance(&self, p: V3) -> (f32, V3) {
        match self.sample(p) {
            Some((d, g)) => outwards(d, g),
            None => (f32::MAX, [0.0, 1.0, 0.0]),
        }
    }
}

/// A seat: a box, its lowest and highest corners (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Seat {
    pub low: V3,
    pub high: V3,
}

impl Seat {
    /// The box's corners, and its faces as triangles facing out (as a
    /// figure's skin is).
    pub fn mesh(&self) -> ([V3; 8], [[u32; 3]; 12]) {
        let (l, h) = (self.low, self.high);
        let corners = std::array::from_fn(|i| {
            [
                if i & 1 == 0 { l[0] } else { h[0] },
                if i & 2 == 0 { l[1] } else { h[1] },
                if i & 4 == 0 { l[2] } else { h[2] },
            ]
        });
        let faces = [
            [0, 2, 3],
            [0, 3, 1],
            [4, 5, 7],
            [4, 7, 6],
            [0, 1, 5],
            [0, 5, 4],
            [2, 6, 7],
            [2, 7, 3],
            [0, 4, 6],
            [0, 6, 2],
            [1, 3, 7],
            [1, 7, 5],
        ];
        (corners, faces)
    }

    /// The seat `by` m lower.
    pub fn lowered(self, by: f32) -> Seat {
        Seat {
            low: [self.low[0], self.low[1] - by, self.low[2]],
            high: [self.high[0], self.high[1] - by, self.high[2]],
        }
    }
}

impl Collider for Seat {
    fn distance(&self, p: V3) -> (f32, V3) {
        let middle = [0, 1, 2].map(|k| (self.low[k] + self.high[k]) / 2.0);
        let half = [0, 1, 2].map(|k| (self.high[k] - self.low[k]) / 2.0);
        let off = sub(p, middle);
        let q = [0, 1, 2].map(|k| off[k].abs() - half[k]);
        let out = q.map(|c| c.max(0.0));
        let outside = dot(out, out).sqrt();
        if outside > 0.0 {
            let n = [0, 1, 2].map(|k| out[k] * off[k].signum());
            return (outside, unit(n));
        }
        // Inside: out through the nearest face.
        let k = (0..3).max_by(|&a, &b| q[a].total_cmp(&q[b])).unwrap_or(1);
        let mut n = [0.0; 3];
        n[k] = if off[k] < 0.0 { -1.0 } else { 1.0 };
        (q[k], n)
    }
}

/// A body and the seat it sits on, if any: whichever is nearer.
pub struct Seated<'a> {
    pub body: &'a dyn Collider,
    pub seat: Option<Seat>,
}

impl std::fmt::Debug for Seated<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seated")
            .field("seat", &self.seat)
            .finish_non_exhaustive()
    }
}

impl Collider for Seated<'_> {
    fn distance(&self, p: V3) -> (f32, V3) {
        let on_body = self.body.distance(p);
        match self.seat {
            Some(seat) => {
                let on_seat = seat.distance(p);
                if on_seat.0 < on_body.0 {
                    on_seat
                } else {
                    on_body
                }
            }
            None => on_body,
        }
    }
}

/// A body on its way from one field to the next, `share` of the way: the
/// two fields' distances blended. Close enough together, the surface
/// between moves smoothly from the one to the other.
#[derive(Clone, Copy, Debug)]
pub struct Between<'a> {
    pub from: &'a BodyField,
    pub to: &'a BodyField,
    pub share: f32,
}

impl Collider for Between<'_> {
    fn distance(&self, p: V3) -> (f32, V3) {
        let s = self.share;
        match (self.from.sample(p), self.to.sample(p)) {
            (Some((da, ga)), Some((db, gb))) => outwards(
                da + (db - da) * s,
                [0, 1, 2].map(|k| ga[k] + (gb[k] - ga[k]) * s),
            ),
            (Some((d, g)), None) | (None, Some((d, g))) => outwards(d, g),
            (None, None) => (f32::MAX, [0.0, 1.0, 0.0]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cube from -0.1 to 0.1 m, its faces facing out.
    fn cube() -> (Vec<V3>, Vec<[u32; 3]>) {
        let s = 0.1;
        let points = vec![
            [-s, -s, -s],
            [s, -s, -s],
            [s, s, -s],
            [-s, s, -s],
            [-s, -s, s],
            [s, -s, s],
            [s, s, s],
            [-s, s, s],
        ];
        let faces = vec![
            [0, 2, 1],
            [0, 3, 2],
            [4, 5, 6],
            [4, 6, 7],
            [0, 1, 5],
            [0, 5, 4],
            [3, 6, 2],
            [3, 7, 6],
            [0, 4, 7],
            [0, 7, 3],
            [1, 2, 6],
            [1, 6, 5],
        ];
        (points, faces)
    }

    #[test]
    fn a_cube_is_inside_out_and_its_faces_point_away() {
        let (points, faces) = cube();
        let field = BodyField::new(&points, &faces, 0.01, 0.05);
        let (d, _) = field.distance([0.0, 0.0, 0.0]);
        assert!(d < -0.02, "the middle is inside: {d}");
        let (d, n) = field.distance([0.0, 0.13, 0.0]);
        assert!((d - 0.03).abs() < 0.005, "{d}");
        assert!(n[1] > 0.9, "{n:?}");
        let (d, n) = field.distance([0.105, 0.0, 0.0]);
        assert!((d - 0.005).abs() < 0.003, "{d}");
        assert!(n[0] > 0.9, "{n:?}");
        assert!(field.distance([1.0, 1.0, 1.0]).0 > 1e30, "far off, nothing");
    }

    #[test]
    fn a_seat_is_felt_from_above_and_inside() {
        let seat = Seat {
            low: [-0.2, 0.0, -0.2],
            high: [0.2, 0.5, 0.2],
        };
        let (d, n) = seat.distance([0.0, 0.53, 0.05]);
        assert!((d - 0.03).abs() < 1e-6 && n[1] > 0.999, "{d} {n:?}");
        let (d, n) = seat.distance([0.1, 0.49, 0.0]);
        assert!((d + 0.01).abs() < 1e-6 && n[1] > 0.999, "{d} {n:?}");
        let (d, n) = seat.distance([0.23, 0.54, 0.0]);
        assert!(
            (d - 0.05).abs() < 1e-6 && n[0] > 0.5 && n[1] > 0.7,
            "{d} {n:?}"
        );
        assert!((seat.lowered(0.1).distance([0.0, 0.5, 0.0]).0 - 0.1).abs() < 1e-6);
    }

    #[test]
    fn a_surface_with_a_part_left_out_is_closed_over_the_hole() {
        let (points, faces) = cube();
        // The top's two triangles left out: closed by a fan of four.
        let surface = Surface::without(&faces, points.len(), |t| {
            t.iter().all(|&v| points[v as usize][1] > 0.0)
        });
        assert_eq!(surface.rims.len(), 1);
        assert_eq!(surface.triangles.len(), 14);
        let all = surface.with_middles(&points);
        assert!(sub(all[8], [0.0, 0.1, 0.0]).iter().all(|c| c.abs() < 1e-6));
        let field = BodyField::around(&points, &surface, ([-0.2; 3], [0.2; 3]), 0.01, 0.03);
        assert!(field.distance([0.0, 0.0, 0.0]).0 < -0.02);
        let (d, n) = field.distance([0.0, 0.13, 0.0]);
        assert!((d - 0.03).abs() < 0.005 && n[1] > 0.9, "{d} {n:?}");
    }

    #[test]
    fn round_part_of_a_cube_its_sides_are_told_by_its_nearest_part() {
        let (points, faces) = cube();
        // Only the top half, cutting through the cube.
        let field = BodyField::around(
            &points,
            &Surface::new(faces),
            ([-0.2, 0.0, -0.2], [0.2, 0.2, 0.2]),
            0.01,
            0.03,
        );
        let (d, _) = field.distance([0.0, 0.03, 0.0]);
        assert!(d < -0.02, "deep inside: {d}");
        let (d, n) = field.distance([0.0, 0.13, 0.0]);
        assert!((d - 0.03).abs() < 0.005, "{d}");
        assert!(n[1] > 0.9, "{n:?}");
        // Off a corner and an edge: outside.
        assert!(field.distance([0.11, 0.11, 0.11]).0 > 0.01);
        assert!(field.distance([0.11, 0.11, 0.0]).0 > 0.008);
        assert!(
            field.distance([0.0, 0.0, 0.17]).0 > 0.02,
            "far off the side"
        );
        // Between it and the same cube 3 cm up, half way: 1.5 cm up.
        let up: Vec<V3> = points.iter().map(|p| [p[0], p[1] + 0.03, p[2]]).collect();
        let moved = BodyField::around(
            &up,
            &Surface::new(cube().1),
            ([-0.2, 0.0, -0.2], [0.2, 0.2, 0.2]),
            0.01,
            0.03,
        );
        let between = Between {
            from: &field,
            to: &moved,
            share: 0.5,
        };
        let (d, n) = between.distance([0.0, 0.135, 0.0]);
        assert!((d - 0.02).abs() < 0.004, "{d}");
        assert!(n[1] > 0.9, "{n:?}");
    }
}
