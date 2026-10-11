//! Keeping cloth off the body: the body's signed distance (negative inside)
//! on a grid of cells, worked out once, then read by interpolation. Exact
//! near the surface; further off, inside or outside, as a flood from the
//! grid's sides finds it.

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

/// The point of triangle `t` nearest `p` (Ericson, Real-Time Collision
/// Detection, 5.1.5).
fn nearest_on_triangle(p: V3, t: [V3; 3]) -> V3 {
    let [a, b, c] = t;
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]];
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]];
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        let bc = sub(c, b);
        return [b[0] + w * bc[0], b[1] + w * bc[1], b[2] + w * bc[2]];
    }
    let denom = 1.0 / (va + vb + vc);
    let (v, w) = (vb * denom, vc * denom);
    [
        a[0] + ab[0] * v + ac[0] * w,
        a[1] + ab[1] * v + ac[1] * w,
        a[2] + ab[2] * v + ac[2] * w,
    ]
}

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
                        let near = nearest_on_triangle(q, p);
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
            let steps: [(isize, isize, isize); 6] = [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ];
            for (di, dj, dk) in steps {
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
        BodyField { low, cell, size, d }
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

impl Collider for BodyField {
    fn distance(&self, p: V3) -> (f32, V3) {
        match self.sample(p) {
            Some((d, g)) => {
                let l = dot(g, g).sqrt();
                let n = if l > 1e-6 {
                    g.map(|c| c / l)
                } else {
                    [0.0, 1.0, 0.0]
                };
                (d, n)
            }
            None => (f32::MAX, [0.0, 1.0, 0.0]),
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
}
