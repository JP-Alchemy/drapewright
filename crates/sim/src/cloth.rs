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

fn axpy(x: &mut V3, a: f32, y: V3) {
    x[0] += a * y[0];
    x[1] += a * y[1];
    x[2] += a * y[2];
}

/// Kept between the cloth and the body, besides the cloth's thickness, m.
const AIR: f32 = 0.003;

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
        Cloth {
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
}

impl Cloth {
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
