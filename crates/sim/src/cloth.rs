//! The cloth: points with mass, held together by stretch, bend and stitch
//! constraints, solved by XPBD in small steps (one pass a step), and kept
//! off the body by a collider. Metres, kilograms and seconds.

use std::collections::HashMap;

use crate::fabric::Fabric;

pub type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn length(a: V3) -> f32 {
    dot(a, a).sqrt()
}

fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn axpy(x: &mut V3, a: f32, y: V3) {
    x[0] += a * y[0];
    x[1] += a * y[1];
    x[2] += a * y[2];
}

/// Kept between the cloth and the body, besides the cloth's thickness, m.
const AIR: f32 = 0.003;
/// Cloth inside (a facing, a lining) is kept behind the cloth over it within
/// this, m, by its thickness and this much more.
const LAYER_REACH: f32 = 0.015;
const LAYER_AIR: f32 = 0.001;
/// Cloth this near the body (m) tells which way it faces out.
const LAYER_NEAR: f32 = 0.02;
/// Inside cloth and the cloth near it are found again once any of it has
/// moved this far (m): they are looked for twice as far off as they are
/// kept apart, so as not to be missed meanwhile.
const LAYER_REFRESH: f32 = 0.005;

/// Keeps cloth off a body.
pub trait Collider {
    /// How far `p` is outside the body (negative inside), m, and which way
    /// is out there.
    fn distance(&self, p: V3) -> (f32, V3);
}

/// No body at all.
#[derive(Debug)]
pub struct Nothing;

impl Collider for Nothing {
    fn distance(&self, _: V3) -> (f32, V3) {
        (f32::MAX, [0.0, 1.0, 0.0])
    }
}

/// An edge held near its length.
#[derive(Clone, Copy, Debug)]
struct Edge {
    i: u32,
    j: u32,
    rest: f32,
    /// m/N.
    compliance: f32,
}

/// Two triangles kept from folding where they meet (Bergou's isometric
/// bending): the curvature `Σ k·x` held at zero.
#[derive(Clone, Copy, Debug)]
struct Bend {
    i: [u32; 4],
    k: [f32; 4],
    compliance: f32,
}

/// A cloth, built panel by panel.
#[derive(Debug)]
pub struct ClothBuilder {
    fabric: Fabric,
    x: Vec<V3>,
    rest: Vec<[f32; 2]>,
    mass: Vec<f32>,
    triangles: Vec<[u32; 3]>,
    edges: Vec<Edge>,
    bends: Vec<Bend>,
    stitches: Vec<[u32; 2]>,
    inside_from: Option<u32>,
    on_from: Option<u32>,
    over: Vec<(std::ops::Range<u32>, std::ops::Range<u32>)>,
}

fn cot(a: [f32; 2], b: [f32; 2]) -> f32 {
    let cross = (a[0] * b[1] - a[1] * b[0]).abs().max(1e-12);
    (a[0] * b[0] + a[1] * b[1]) / cross
}

fn area(p: [[f32; 2]; 3]) -> f32 {
    ((p[1][0] - p[0][0]) * (p[2][1] - p[0][1]) - (p[2][0] - p[0][0]) * (p[1][1] - p[0][1])).abs()
        / 2.0
}

impl ClothBuilder {
    pub fn new(fabric: Fabric) -> Self {
        ClothBuilder {
            fabric,
            x: Vec::new(),
            rest: Vec::new(),
            mass: Vec::new(),
            triangles: Vec::new(),
            edges: Vec::new(),
            bends: Vec::new(),
            stitches: Vec::new(),
            inside_from: None,
            on_from: None,
            over: Vec::new(),
        }
    }

    /// Adds a panel: its points laid flat (`rest`, m, the grain along +y),
    /// where each starts in space, and its triangles (indices into
    /// `rest`). Returns the index its points start at.
    pub fn panel(&mut self, rest: &[[f32; 2]], start: &[V3], triangles: &[[u32; 3]]) -> u32 {
        let first = self.x.len() as u32;
        self.x.extend_from_slice(start);
        self.rest.extend_from_slice(rest);
        self.mass.extend(std::iter::repeat_n(0.0, rest.len()));
        // Each edge's triangles: (third point, area).
        let mut sides: HashMap<(u32, u32), Vec<(u32, f32)>> = HashMap::new();
        for t in triangles {
            let a = area(t.map(|i| rest[i as usize]));
            for &i in t {
                self.mass[(first + i) as usize] += a * self.fabric.density / 3.0;
            }
            for k in 0..3 {
                let (i, j, o) = (t[k], t[(k + 1) % 3], t[(k + 2) % 3]);
                sides.entry((i.min(j), i.max(j))).or_default().push((o, a));
            }
            self.triangles.push(t.map(|i| first + i));
        }
        let mut keys: Vec<_> = sides.keys().copied().collect();
        keys.sort_unstable();
        for (i, j) in keys {
            let around = &sides[&(i, j)];
            let (pi, pj) = (rest[i as usize], rest[j as usize]);
            let d = [pj[0] - pi[0], pj[1] - pi[1]];
            let length = d[0].hypot(d[1]);
            // Its strip of cloth: the triangles either side.
            let strip: f32 = around.iter().map(|(_, a)| a).sum();
            let stiffness = self.fabric.stiffness_at(d[0].atan2(d[1])) * strip / (length * length);
            self.edges.push(Edge {
                i: first + i,
                j: first + j,
                rest: length,
                compliance: 1.0 / stiffness.max(1e-6),
            });
            if let [(o0, a0), (o1, a1)] = around[..] {
                let (x0, x1, x2, x3) = (pi, pj, rest[o0 as usize], rest[o1 as usize]);
                let e0 = [x1[0] - x0[0], x1[1] - x0[1]];
                let e1 = [x2[0] - x0[0], x2[1] - x0[1]];
                let e2 = [x3[0] - x0[0], x3[1] - x0[1]];
                let e3 = [x2[0] - x1[0], x2[1] - x1[1]];
                let e4 = [x3[0] - x1[0], x3[1] - x1[1]];
                let m0 = [-e0[0], -e0[1]];
                let (c01, c02) = (cot(e0, e1), cot(e0, e2));
                let (c03, c04) = (cot(m0, e3), cot(m0, e4));
                self.bends.push(Bend {
                    i: [first + i, first + j, first + o0, first + o1],
                    k: [c03 + c04, c01 + c02, -c01 - c03, -c02 - c04],
                    compliance: (a0 + a1) / (3.0 * self.fabric.bending.max(1e-12)),
                });
            }
        }
        first
    }

    /// Sews point `i` to point `j`.
    pub fn stitch(&mut self, i: u32, j: u32) {
        self.stitches.push([i, j]);
    }

    /// The points from `first` on go inside the rest (facings, linings):
    /// they are kept nearer the body than the cloth over them.
    pub fn inside_from(&mut self, first: u32) {
        self.inside_from = Some(first);
    }

    /// Of the points from `inside_from` on, those from `first` on are sewn
    /// onto the cloth they lie on (patch pockets): they are kept outside
    /// it, not behind it.
    pub fn on_from(&mut self, first: u32) {
        self.on_from = Some(first);
    }

    /// The inside points `inside` lie on the points `over` (a lining on
    /// the piece it was traced from), and are kept behind those only (or,
    /// sewn on, in front of them).
    pub fn lies_on(&mut self, inside: std::ops::Range<u32>, over: std::ops::Range<u32>) {
        self.over.push((inside, over));
    }

    pub fn build(mut self) -> Cloth {
        let n = self.x.len();
        // Solved from the top down, a band at a time: a garment hangs from
        // its shoulders and waist, and in one pass a correction then
        // carries all the way down instead of being shared out a little at
        // a time. Within a band, in a scrambled order: always across the
        // same way, a garment's two halves are corrected unevenly at every
        // step, and it creeps round the body for ever.
        let band = |heights: f32| -((heights / BAND).floor() as i64);
        let x = &self.x;
        let height = |i: u32| x[i as usize][1];
        self.edges.sort_by_cached_key(|e| {
            (
                band((height(e.i) + height(e.j)) / 2.0),
                scramble(&[e.i, e.j]),
            )
        });
        self.bends.sort_by_cached_key(|b| {
            (
                band(b.i.iter().map(|&i| height(i)).sum::<f32>() / 4.0),
                scramble(&b.i),
            )
        });
        let inside_from = self.inside_from.map_or(n, |f| f as usize);
        let on_from = self.on_from.map_or(n, |f| (f as usize).max(inside_from));
        // Where inside and outside are sewn together, they are one.
        let mut at_seam = vec![false; n];
        for &[i, j] in &self.stitches {
            if (i as usize >= inside_from) != (j as usize >= inside_from) {
                at_seam[i as usize] = true;
                at_seam[j as usize] = true;
            }
        }
        Cloth {
            over: self.over,
            facing: vec![1.0; self.triangles.len()],
            layer_pairs: Vec::new(),
            layers_found_at: Vec::new(),
            layer_finds: 0,
            inside_from,
            on_from,
            at_seam,
            p: self.x.clone(),
            x: self.x,
            v: vec![[0.0; 3]; n],
            w: self
                .mass
                .iter()
                .map(|&m| if m > 0.0 { 1.0 / m } else { 0.0 })
                .collect(),
            triangles: self.triangles,
            rest: self.rest,
            edges: self.edges,
            bends: self.bends,
            stitches: self.stitches,
            fabric: self.fabric,
            gravity: [0.0, -9.81, 0.0],
            stitch_compliance: 1e-6,
            damping: 3.0,
            max_speed: f32::INFINITY,
            contacts: Vec::new(),
            stepped: 0.0,
        }
    }
}

/// The height of the bands constraints are solved in, m.
const BAND: f32 = 0.01;

/// A number that scrambles the order of points' constraints, the same for
/// the same points every time.
fn scramble(points: &[u32]) -> u64 {
    points.iter().fold(0x9E37_79B9_7F4A_7C15, |h, &i| {
        // SplitMix64's mixing.
        let mut z = h ^ u64::from(i).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    })
}

/// The cloth as a solver elsewhere (on the graphics card) needs it: its
/// points, constraints and fabric, as they are now.
#[derive(Clone, Debug)]
pub struct Parts {
    pub x: Vec<V3>,
    pub v: Vec<V3>,
    /// Inverse masses, 1/kg.
    pub w: Vec<f32>,
    pub triangles: Vec<[u32; 3]>,
    /// Which way each triangle faces out (1 or -1).
    pub facing: Vec<f32>,
    /// Edges: their points, length flat (m) and compliance (m/N).
    pub edges: Vec<(u32, u32, f32, f32)>,
    /// Bends: their four points, weights and compliance.
    pub bends: Vec<([u32; 4], [f32; 4], f32)>,
    pub stitches: Vec<[u32; 2]>,
    /// The points from this one on go inside the rest...
    pub inside_from: usize,
    /// ...but those from this one on are sewn onto it, outside it.
    pub on_from: usize,
    /// Kept this far off the body, m.
    pub gap: f32,
    pub friction: f32,
    /// Inside cloth kept this far behind the cloth over it, within `reach`, m.
    pub apart: f32,
    pub reach: f32,
}

/// How the cloth is pulled, point by point: each point's value the mean of
/// its triangles', by their area, then evened out a little with its
/// neighbours' (one triangle's worth of cloth reads unevenly).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strain {
    /// How far the cloth is stretched beyond its cut size, the most it is
    /// any way (0.03: 3%).
    pub stretch: Vec<f32>,
    /// How hard it pulls, N/m: the fabric's stiffness the way it is most
    /// stretched, times that stretch.
    pub pull: Vec<f32>,
}

/// The cloth as it moves.
#[derive(Debug)]
pub struct Cloth {
    /// Where each point is, m.
    pub x: Vec<V3>,
    /// Where each was before this step.
    p: Vec<V3>,
    v: Vec<V3>,
    /// Inverse masses, 1/kg (0: held still).
    w: Vec<f32>,
    pub triangles: Vec<[u32; 3]>,
    /// Each point laid flat, m, the grain along +y.
    pub rest: Vec<[f32; 2]>,
    edges: Vec<Edge>,
    bends: Vec<Bend>,
    stitches: Vec<[u32; 2]>,
    fabric: Fabric,
    /// m/s².
    pub gravity: V3,
    /// How loosely stitches hold, m/N: loose while the seams close.
    pub stitch_compliance: f32,
    /// Velocity lost each second, as a fraction.
    pub damping: f32,
    /// No point moves faster than this, m/s.
    pub max_speed: f32,
    /// Points touching the body this step, and which way is out.
    contacts: Vec<(u32, V3)>,
    /// How fast the points moved over the last step, on average, m/s.
    stepped: f32,
    /// The points from this one on go inside the rest (facings, linings)...
    inside_from: usize,
    /// ...but those from this one on are sewn onto it, outside it (patch
    /// pockets): kept in front of the cloth they lie on.
    on_from: usize,
    /// Points sewn to the other layer.
    at_seam: Vec<bool>,
    /// Which way each triangle faces out (see `face_out`).
    facing: Vec<f32>,
    /// Inside points and outside ones near them, this step.
    layer_pairs: Vec<(u32, u32)>,
    /// Inside points, and the outside points they lie on.
    over: Vec<(std::ops::Range<u32>, std::ops::Range<u32>)>,
    /// Where the points were when `layer_pairs` were found, and how many
    /// times they have been.
    layers_found_at: Vec<V3>,
    layer_finds: u64,
}

impl Cloth {
    /// The cloth's parts, for a solver elsewhere.
    pub fn parts(&self) -> Parts {
        Parts {
            x: self.x.clone(),
            v: self.v.clone(),
            w: self.w.clone(),
            triangles: self.triangles.clone(),
            facing: self.facing.clone(),
            edges: self
                .edges
                .iter()
                .map(|e| (e.i, e.j, e.rest, e.compliance))
                .collect(),
            bends: self
                .bends
                .iter()
                .map(|b| (b.i, b.k, b.compliance))
                .collect(),
            stitches: self.stitches.clone(),
            inside_from: self.inside_from,
            on_from: self.on_from,
            gap: self.fabric.thickness + AIR,
            friction: self.fabric.friction,
            apart: self.fabric.thickness + LAYER_AIR,
            reach: LAYER_REACH,
        }
    }

    /// Where the points have got to, worked out elsewhere, `elapsed`
    /// seconds after they were last here.
    pub fn moved_to(&mut self, x: Vec<V3>, elapsed: f32) {
        if x.len() != self.x.len() {
            return;
        }
        self.stepped = if elapsed > 0.0 {
            self.x
                .iter()
                .zip(&x)
                .map(|(&a, &b)| length(sub(a, b)))
                .sum::<f32>()
                / (elapsed * x.len().max(1) as f32)
        } else {
            0.0
        };
        self.x = x;
    }

    /// The inside points and the outside ones near them, as they are now:
    /// those kept apart (see `order_layers`); and how many times they have
    /// been found, to tell when they are found afresh.
    pub fn near_layers(&mut self) -> (u64, &[(u32, u32)]) {
        self.refresh_layer_pairs();
        (self.layer_finds, &self.layer_pairs)
    }

    /// Finds the inside points and the outside ones near them again, if
    /// the cloth has moved far enough since they were last found.
    fn refresh_layer_pairs(&mut self) {
        if self.inside_from >= self.x.len() {
            return;
        }
        let moved = self.layers_found_at.len() != self.x.len()
            || self
                .x
                .iter()
                .zip(&self.layers_found_at)
                .any(|(&a, &b)| length(sub(a, b)) > LAYER_REFRESH);
        if moved {
            self.find_layer_pairs();
            self.layers_found_at.clone_from(&self.x);
            self.layer_finds += 1;
        }
    }

    /// Moves the whole cloth by `by` (m), as it is.
    pub fn shift(&mut self, by: V3) {
        for x in &mut self.x {
            *x = [x[0] + by[0], x[1] + by[1], x[2] + by[2]];
        }
    }

    /// Holds a point still.
    pub fn pin(&mut self, i: u32) {
        self.w[i as usize] = 0.0;
    }

    /// The largest gap left in a seam, m.
    pub fn widest_stitch(&self) -> f32 {
        self.stitches
            .iter()
            .map(|&[i, j]| length(sub(self.x[i as usize], self.x[j as usize])))
            .fold(0.0, f32::max)
    }

    /// The fastest-moving point's speed, m/s.
    pub fn fastest(&self) -> f32 {
        self.v.iter().map(|&v| length(v)).fold(0.0, f32::max)
    }

    /// Each edge's length now over its length flat, at its most.
    pub fn most_stretched(&self) -> f32 {
        self.edges
            .iter()
            .map(|e| length(sub(self.x[e.i as usize], self.x[e.j as usize])) / e.rest)
            .fold(0.0, f32::max)
    }

    /// How fast the points moved over the last step as a whole, on average
    /// (m/s): near nothing once the cloth has settled, even where a point
    /// caught between two parts of the body is pushed to and fro within it.
    pub fn step_speed(&self) -> f32 {
        self.stepped
    }

    /// The points' mean speed, m/s: near nothing once the cloth has settled.
    pub fn mean_speed(&self) -> f32 {
        self.v.iter().map(|&v| length(v)).sum::<f32>() / self.v.len().max(1) as f32
    }

    /// Each edge's length now over its length flat, on average.
    pub fn mean_stretch(&self) -> f32 {
        let total: f32 = self
            .edges
            .iter()
            .map(|e| length(sub(self.x[e.i as usize], self.x[e.j as usize])) / e.rest)
            .sum();
        total / self.edges.len().max(1) as f32
    }

    /// A triangle's greatest stretch (1: as cut), and the fabric's
    /// stiffness that way, N/m.
    fn triangle_strain(&self, t: [u32; 3]) -> (f32, f32) {
        let [r0, r1, r2] = t.map(|i| self.rest[i as usize]);
        let [x0, x1, x2] = t.map(|i| self.x[i as usize]);
        // Flat, its edges from its first corner, and the inverse of the
        // matrix they make: the triangle as it was cut.
        let (a, b) = (
            [r1[0] - r0[0], r1[1] - r0[1]],
            [r2[0] - r0[0], r2[1] - r0[1]],
        );
        let det = a[0] * b[1] - b[0] * a[1];
        if det.abs() < 1e-12 {
            return (1.0, 0.0);
        }
        let inv = [[b[1] / det, -b[0] / det], [-a[1] / det, a[0] / det]];
        // How it is shaped now: the deformation from flat (3 by 2), by
        // its columns.
        let (d1, d2) = (sub(x1, x0), sub(x2, x0));
        let column = |j: usize| -> V3 { [0, 1, 2].map(|k| d1[k] * inv[0][j] + d2[k] * inv[1][j]) };
        let (f0, f1) = (column(0), column(1));
        // Its greatest stretch squared, and which way flat (from the u
        // axis), from the deformation's square.
        let (c00, c01, c11) = (dot(f0, f0), dot(f0, f1), dot(f1, f1));
        let mean = (c00 + c11) / 2.0;
        let r = (((c00 - c11) / 2.0).powi(2) + c01 * c01).sqrt();
        let most = (mean + r).max(0.0).sqrt();
        let way = 0.5 * (2.0 * c01).atan2(c00 - c11);
        // From the grain (along v).
        (most, self.fabric.stiffness_at(way.cos().atan2(way.sin())))
    }

    /// How the cloth is pulled, point by point.
    pub fn strain(&self) -> Strain {
        let n = self.x.len();
        let (mut stretch, mut pull, mut weight) = (vec![0.0; n], vec![0.0; n], vec![0.0; n]);
        for &t in &self.triangles {
            let (most, stiffness) = self.triangle_strain(t);
            let beyond = (most - 1.0).max(0.0);
            let a = area(t.map(|i| self.rest[i as usize]));
            for i in t {
                let i = i as usize;
                stretch[i] += a * beyond;
                pull[i] += a * beyond * stiffness;
                weight[i] += a;
            }
        }
        for i in 0..n {
            if weight[i] > 0.0 {
                stretch[i] /= weight[i];
                pull[i] /= weight[i];
            }
        }
        for _ in 0..2 {
            stretch = self.evened(&stretch);
            pull = self.evened(&pull);
        }
        Strain { stretch, pull }
    }

    /// Values at the points, each halfway to its neighbours' mean.
    fn evened(&self, values: &[f32]) -> Vec<f32> {
        let mut sum = vec![0.0f32; values.len()];
        let mut count = vec![0u32; values.len()];
        for t in &self.triangles {
            for k in 0..3 {
                let (i, j) = (t[k] as usize, t[(k + 1) % 3] as usize);
                sum[i] += values[j];
                sum[j] += values[i];
                count[i] += 1;
                count[j] += 1;
            }
        }
        values
            .iter()
            .zip(sum.iter().zip(&count))
            .map(|(&v, (&s, &c))| if c > 0 { (v + s / c as f32) / 2.0 } else { v })
            .collect()
    }

    /// The room between each point and the body, m: none where the cloth
    /// rests on it.
    pub fn room(&self, body: &dyn Collider) -> Vec<f32> {
        let gap = self.fabric.thickness + AIR;
        self.x
            .iter()
            .map(|&p| (body.distance(p).0 - gap).max(0.0))
            .collect()
    }

    /// Pushes any point inside the body, or too near it, out: where the
    /// placing could not see the body's shape.
    pub fn push_out(&mut self, body: &dyn Collider) {
        let gap = self.fabric.thickness + AIR;
        for k in 0..self.x.len() {
            let (d, n) = body.distance(self.x[k]);
            if d < gap {
                axpy(&mut self.x[k], gap - d, n);
                self.p[k] = self.x[k];
            }
        }
    }

    /// Moves on by `dt` seconds, in `substeps` small steps.
    pub fn step(&mut self, dt: f32, substeps: u32, body: &dyn Collider) {
        let h = dt / substeps.max(1) as f32;
        let keep = (-self.damping * h).exp();
        let start = self.x.clone();
        self.refresh_layer_pairs();
        for _ in 0..substeps.max(1) {
            for k in 0..self.x.len() {
                self.p[k] = self.x[k];
                if self.w[k] > 0.0 {
                    let v = &mut self.v[k];
                    axpy(v, h, self.gravity);
                    *v = v.map(|c| c * keep);
                    let v = *v;
                    axpy(&mut self.x[k], h, v);
                }
            }
            self.solve_edges(h);
            self.solve_bends(h);
            self.solve_stitches(h);
            self.collide(body);
            self.order_layers();
            // And again: a seam holds where the body's friction would keep
            // one side of it still while the cloth pulls the other away.
            self.solve_stitches(h);
            for k in 0..self.x.len() {
                let d = sub(self.x[k], self.p[k]);
                self.v[k] = d.map(|c| c / h);
                let speed = length(self.v[k]);
                if speed > self.max_speed {
                    let keep = self.max_speed / speed;
                    self.v[k] = self.v[k].map(|c| c * keep);
                }
            }
            // Resting on the body, a point is not moving towards it or away:
            // being pushed out is not speed.
            for &(k, n) in &self.contacts {
                let v = &mut self.v[k as usize];
                let towards = dot(*v, n);
                axpy(v, -towards, n);
            }
        }
        self.stepped = self
            .x
            .iter()
            .zip(&start)
            .map(|(&a, &b)| length(sub(a, b)))
            .sum::<f32>()
            / (dt * self.x.len().max(1) as f32);
    }

    fn solve_edges(&mut self, h: f32) {
        let x = &mut self.x;
        for e in &self.edges {
            let (i, j) = (e.i as usize, e.j as usize);
            let (wi, wj) = (self.w[i], self.w[j]);
            let d = sub(x[i], x[j]);
            let l = length(d);
            if l < 1e-9 || wi + wj == 0.0 {
                continue;
            }
            let n = d.map(|c| c / l);
            let alpha = e.compliance / (h * h);
            let dl = -(l - e.rest) / (wi + wj + alpha);
            axpy(&mut x[i], wi * dl, n);
            axpy(&mut x[j], -wj * dl, n);
        }
    }

    #[allow(clippy::needless_range_loop)] // Each axis on its own, as the maths does.
    fn solve_bends(&mut self, h: f32) {
        // The curvature `Σ k·x` is linear in the points: held at zero along
        // each axis, its gradient is constant and nothing flips as the
        // cloth goes flat.
        let x = &mut self.x;
        for b in &self.bends {
            let weight: f32 =
                b.i.iter()
                    .zip(&b.k)
                    .map(|(&i, &k)| self.w[i as usize] * k * k)
                    .sum();
            let alpha = b.compliance / (h * h);
            if weight + alpha == 0.0 {
                continue;
            }
            for axis in 0..3 {
                let c: f32 =
                    b.i.iter()
                        .zip(&b.k)
                        .map(|(&i, &k)| k * x[i as usize][axis])
                        .sum();
                let dl = -c / (weight + alpha);
                for (&i, &k) in b.i.iter().zip(&b.k) {
                    x[i as usize][axis] += self.w[i as usize] * k * dl;
                }
            }
        }
    }

    fn solve_stitches(&mut self, h: f32) {
        let alpha = self.stitch_compliance / (h * h);
        let x = &mut self.x;
        for &[i, j] in &self.stitches {
            let (i, j) = (i as usize, j as usize);
            let (wi, wj) = (self.w[i], self.w[j]);
            let d = sub(x[i], x[j]);
            let l = length(d);
            if l < 1e-9 || wi + wj == 0.0 {
                continue;
            }
            let n = d.map(|c| c / l);
            let dl = -l / (wi + wj + alpha);
            axpy(&mut x[i], wi * dl, n);
            axpy(&mut x[j], -wj * dl, n);
        }
    }

    /// Which points go inside the rest (facings, linings).
    pub fn inside(&self, k: usize) -> bool {
        k >= self.inside_from && k < self.on_from
    }

    /// Which points are sewn onto the face of the rest (patch pockets).
    pub fn sewn_on(&self, k: usize) -> bool {
        k >= self.on_from
    }

    /// Which way each triangle faces out, away from the body: 1 the side
    /// its corners run anticlockwise round, -1 the other.
    pub fn facing(&self) -> &[f32] {
        &self.facing
    }

    /// Which way each triangle faces out, away from the body: the side its
    /// corners run anticlockwise round, or (-1) the other. Each panel (each
    /// piece of cloth joined by its triangles) faces one way, as most of it
    /// near the body faces; cloth inside (a facing, a lining) as the cloth
    /// it lies on faces, where it lies on any. It is kept behind that cloth
    /// only where the two face alike (see `order_layers`), and a facing
    /// round the neck, away from the body, could be taken either way.
    pub fn face_out(&mut self, body: &dyn Collider) {
        let n = self.x.len();
        let mut group: Vec<usize> = (0..n).collect();
        fn root(group: &mut [usize], mut k: usize) -> usize {
            while group[k] != k {
                group[k] = group[group[k]];
                k = group[k];
            }
            k
        }
        for t in &self.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2])] {
                let (ra, rb) = (root(&mut group, a as usize), root(&mut group, b as usize));
                group[ra.max(rb)] = ra.min(rb);
            }
        }
        let mut votes = vec![0f32; n];
        let normals: Vec<V3> = self.triangles.iter().map(|t| self.normal(t)).collect();
        for (t, normal) in self.triangles.iter().zip(&normals) {
            let middle =
                [0, 1, 2].map(|k| t.iter().map(|&i| self.x[i as usize][k]).sum::<f32>() / 3.0);
            let (d, out) = body.distance(middle);
            if d < LAYER_NEAR {
                let r = root(&mut group, t[0] as usize);
                votes[r] += dot(*normal, out);
            }
        }
        let mut facing: Vec<f32> = votes
            .iter()
            .map(|&v| if v < 0.0 { -1.0 } else { 1.0 })
            .collect();
        let first = self.inside_from;
        if first < n {
            // Each point's normal by its corners, from its own layer's
            // triangles; the inside cloth starts on the cloth it lies on.
            let mut at = vec![[0f32; 3]; n];
            for (t, normal) in self.triangles.iter().zip(&normals) {
                let inside = t[0] as usize >= first;
                if t.iter().all(|&i| (i as usize >= first) == inside) {
                    for &i in t {
                        axpy(&mut at[i as usize], 1.0, *normal);
                    }
                }
            }
            self.find_layer_pairs();
            let mut alike = vec![0f32; n];
            for &(i, j) in &self.layer_pairs {
                let (ri, rj) = (root(&mut group, i as usize), root(&mut group, j as usize));
                alike[ri] += facing[rj] * dot(at[i as usize], at[j as usize]);
            }
            // An inside panel's points are all inside, and so is its root.
            for r in first..n {
                if alike[r] != 0.0 {
                    facing[r] = if alike[r] < 0.0 { -1.0 } else { 1.0 };
                }
            }
        }
        self.facing = self
            .triangles
            .iter()
            .map(|t| facing[root(&mut group, t[0] as usize)])
            .collect();
    }

    /// A triangle's normal by its corners' order, as long as twice its area.
    fn normal(&self, t: &[u32; 3]) -> V3 {
        let [a, b, c] = t.map(|i| self.x[i as usize]);
        cross(sub(b, a), sub(c, a))
    }

    /// The inside points and the outside ones near them (within
    /// `LAYER_REACH` and as far again as they move in a step), found once
    /// a step for `order_layers`.
    fn find_layer_pairs(&mut self) {
        let first = self.inside_from;
        let n = self.x.len();
        self.layer_pairs.clear();
        if first >= n {
            return;
        }
        let reach = 2.0 * LAYER_REACH;
        let key = |p: V3| p.map(|c| (c / reach).floor() as i32);
        let mut cells: std::collections::HashMap<[i32; 3], Vec<u32>> =
            std::collections::HashMap::new();
        for j in 0..first {
            if !self.at_seam[j] && self.w[j] > 0.0 {
                cells.entry(key(self.x[j])).or_default().push(j as u32);
            }
        }
        for i in first..n {
            if self.w[i] == 0.0 || self.at_seam[i] {
                continue;
            }
            // Only the cloth it lies on, where that is known: where a
            // garment folds (sitting, between the legs), its front and back
            // come together, and a lining behind the front is not to be
            // kept behind the back.
            let lies_on = self
                .over
                .iter()
                .find(|(inside, _)| inside.contains(&(i as u32)))
                .map(|(_, over)| over.clone());
            let [a, b, c] = key(self.x[i]);
            for da in -1..=1 {
                for db in -1..=1 {
                    for dc in -1..=1 {
                        let Some(near) = cells.get(&[a + da, b + db, c + dc]) else {
                            continue;
                        };
                        for &j in near {
                            if lies_on.as_ref().is_none_or(|over| over.contains(&j))
                                && length(sub(self.x[i], self.x[j as usize])) < reach
                            {
                                self.layer_pairs.push((i as u32, j));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Cloth inside (a facing, a lining) kept behind the cloth over it, as
    /// it would rest against it: each inside point and each outside one
    /// near it (within `LAYER_REACH` across the outside cloth) pushed apart
    /// along the outside cloth's outward normal until the inside one is
    /// behind it by its thickness and a little air. Cloth sewn onto the
    /// outside (a patch pocket) is kept in front of it so, the other way.
    /// There is no cloth resting on cloth otherwise.
    ///
    /// Not where the two face opposite ways: there one of them has folded
    /// over (a hem's corner flipping as it swings), and which is behind
    /// which can't be told. Pushed along the fold, the inside cloth would
    /// go through the outside, and once through, each push between them
    /// undoes another, so the cloth never settles. They are left be until
    /// the fold opens.
    fn order_layers(&mut self) {
        let first = self.inside_from;
        if self.layer_pairs.is_empty() {
            return;
        }
        let apart = self.fabric.thickness + LAYER_AIR;
        // Outward normals, point by point, each from its own layer's
        // triangles.
        let mut out = vec![[0f32; 3]; self.x.len()];
        for (t, &facing) in self.triangles.iter().zip(&self.facing) {
            let inside = t[0] as usize >= first;
            if t.iter().any(|&i| (i as usize >= first) != inside) {
                continue;
            }
            let normal = self.normal(t);
            for &i in t {
                axpy(&mut out[i as usize], facing, normal);
            }
        }
        for o in &mut out {
            let l = length(*o);
            if l > 0.0 {
                *o = o.map(|c| c / l);
            }
        }
        for &(i, j) in &self.layer_pairs {
            let (i, j) = (i as usize, j as usize);
            let normal = out[j];
            if dot(out[i], normal) < 0.0 {
                continue;
            }
            // Behind (1) or, sewn on, in front (-1).
            let side = if i >= self.on_from { -1.0 } else { 1.0 };
            let off = sub(self.x[i], self.x[j]);
            let along = dot(off, normal);
            let ahead = side * along;
            if ahead < -apart || ahead > LAYER_REACH {
                continue;
            }
            let across = length(sub(off, normal.map(|v| v * along)));
            if across > LAYER_REACH {
                continue;
            }
            let (wi, wj) = (self.w[i], self.w[j]);
            let short = ahead + apart;
            axpy(&mut self.x[i], -side * short * wi / (wi + wj), normal);
            axpy(&mut self.x[j], side * short * wj / (wi + wj), normal);
            // With friction, as on the body: their slide past each other this
            // step cancelled if small for how hard they press, else cut down.
            let moved = sub(sub(self.x[i], self.p[i]), sub(self.x[j], self.p[j]));
            let slip = sub(moved, normal.map(|v| v * dot(moved, normal)));
            let slide = length(slip);
            if slide > 0.0 {
                let held = (self.fabric.friction * short / slide).min(1.0);
                axpy(&mut self.x[i], -held * wi / (wi + wj), slip);
                axpy(&mut self.x[j], held * wj / (wi + wj), slip);
            }
            // Resting on each other, as on the body: being pushed apart is
            // not speed.
            self.contacts.push((i as u32, normal));
            self.contacts.push((j as u32, normal));
        }
    }

    /// Point `k`, `pressed` m into the body's gap, pushed out along `n`,
    /// with friction (Coulomb, by position): its slide along the body this
    /// step, cancelled if small for how hard it presses on it, else cut
    /// down by that much.
    fn rest_on(&mut self, k: usize, pressed: f32, n: V3, mu: f32) {
        axpy(&mut self.x[k], pressed, n);
        self.contacts.push((k as u32, n));
        let moved = sub(self.x[k], self.p[k]);
        let along = sub(moved, n.map(|c| c * dot(moved, n)));
        let slide = length(along);
        if slide > 0.0 {
            let held = (mu * pressed / slide).min(1.0);
            axpy(&mut self.x[k], -held, along);
        }
    }

    fn collide(&mut self, body: &dyn Collider) {
        // Its thickness and a little air: the body is curved between the
        // cloth's points, and must not show through in the middle of a
        // triangle.
        let gap = self.fabric.thickness + AIR;
        let mu = self.fabric.friction;
        self.contacts.clear();
        for k in 0..self.x.len() {
            if self.w[k] == 0.0 {
                continue;
            }
            let (d, n) = body.distance(self.x[k]);
            if d >= gap {
                continue;
            }
            self.rest_on(k, gap - d, n, mu);
        }
        // The middles of triangles too: the body is curved between points.
        // Their corners then rest on the body as a point would.
        for t in 0..self.triangles.len() {
            let corners = self.triangles[t];
            let [a, b, c] = corners.map(|i| self.x[i as usize]);
            let middle = [0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0);
            let (d, n) = body.distance(middle);
            if d >= gap {
                continue;
            }
            for i in corners {
                if self.w[i as usize] > 0.0 {
                    self.rest_on(i as usize, gap - d, n, mu);
                }
            }
        }
    }
}
