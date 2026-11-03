//! What is sewn into a garment besides its cloth, to be seen on it: its
//! bones, as raised channels along the cloth, and its lacing, a cord
//! crossing between eyelets from one side of a laced seam to the other.

/// Bones and laces as points of the cloth.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Trims {
    /// Each bone, point by point along it: each a triangle of the cloth and
    /// the weights of its corners.
    pub bones: Vec<Vec<([u32; 3], [f32; 3])>>,
    /// Each laced seam's pairs of points across it, in order along it.
    pub laces: Vec<Vec<[u32; 2]>>,
}

/// What a point of the shapes is part of, to colour it by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trim {
    Bone,
    Lace,
    Eyelet,
}

impl Trim {
    /// Its colour on fabric `fabric` (sRGB, 0 to 1): a bone's channel a
    /// shade darker than the fabric, the lace near black, eyelets steel.
    pub fn colour(self, fabric: [f32; 3]) -> [f32; 3] {
        match self {
            Trim::Bone => fabric.map(|c| c * 0.85),
            Trim::Lace => [0.13, 0.12, 0.12],
            Trim::Eyelet => [0.72, 0.72, 0.74],
        }
    }
}

/// The trims as solid shapes: points, triangles (indices into `points`),
/// and what each point is part of.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shapes {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    pub kinds: Vec<Trim>,
}

/// A bone's channel is this wide, and stands this far off the cloth, mm.
const CHANNEL_WIDTH: f32 = 7.0;
const CHANNEL_OFF: f32 = 1.2;
/// A lace is this thick, and lies this far off the cloth, mm.
const LACE_RADIUS: f32 = 1.3;
const LACE_OFF: f32 = 2.5;
/// An eyelet is this big across, mm.
const EYELET_RADIUS: f32 = 3.5;
/// The lace crosses at eyelets about this far apart, mm.
const EYELETS_APART: f32 = 25.0;

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, s: f32) -> V3 {
    a.map(|c| c * s)
}
fn cross(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn unit(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        scale(a, 1.0 / l)
    } else {
        [0.0; 3]
    }
}

/// Each point's normal, out of the garment: its triangles', by their area,
/// each turned the way `facing` says it faces out.
fn normals(x: &[V3], triangles: &[[u32; 3]], facing: &[f32]) -> Vec<V3> {
    let mut n = vec![[0.0; 3]; x.len()];
    for (k, t) in triangles.iter().enumerate() {
        let [a, b, c] = t.map(|i| x[i as usize]);
        let face = scale(
            cross(sub(b, a), sub(c, a)),
            facing.get(k).copied().unwrap_or(1.0),
        );
        for &i in t {
            n[i as usize] = add(n[i as usize], face);
        }
    }
    n.into_iter().map(unit).collect()
}

impl Trims {
    pub fn is_empty(&self) -> bool {
        self.bones.is_empty() && self.laces.is_empty()
    }

    /// The trims on cloth whose points are `x` (`mm` units to the
    /// millimetre) and triangles `triangles`, each facing out as `facing`
    /// says.
    pub fn shapes(&self, x: &[V3], triangles: &[[u32; 3]], facing: &[f32], mm: f32) -> Shapes {
        let mut out = Shapes::default();
        if self.is_empty() {
            return out;
        }
        let normal = normals(x, triangles, facing);
        let at = |k: u32| x[k as usize];
        let out_at = |k: u32| normal[k as usize];
        // A point of a bone: where it is, and which way is out there.
        let weighed = |(t, w): &([u32; 3], [f32; 3])| -> (V3, V3) {
            let mut p = [0.0; 3];
            let mut n = [0.0; 3];
            for (&k, &wk) in t.iter().zip(w) {
                p = add(p, scale(at(k), wk));
                n = add(n, scale(out_at(k), wk));
            }
            (p, unit(n))
        };
        for bone in &self.bones {
            let chain: Vec<(V3, V3)> = bone
                .iter()
                .filter(|(t, _)| t.iter().all(|&k| (k as usize) < x.len()))
                .map(weighed)
                .collect();
            if chain.len() < 2 {
                continue;
            }
            let first = out.points.len() as u32;
            for (j, &(p, n)) in chain.iter().enumerate() {
                let prev = chain[j.saturating_sub(1)].0;
                let next = chain[(j + 1).min(chain.len() - 1)].0;
                let across = unit(cross(sub(next, prev), n));
                let centre = add(p, scale(n, CHANNEL_OFF * mm));
                for side in [-0.5, 0.5] {
                    out.points
                        .push(add(centre, scale(across, side * CHANNEL_WIDTH * mm)));
                    out.kinds.push(Trim::Bone);
                }
            }
            for j in 0..chain.len() as u32 - 1 {
                let (a, b) = (first + 2 * j, first + 2 * j + 2);
                out.triangles.push([a, b, a + 1]);
                out.triangles.push([a + 1, b, b + 1]);
            }
        }
        for lace in &self.laces {
            let pairs: Vec<[u32; 2]> = lace
                .iter()
                .copied()
                .filter(|p| p.iter().all(|&k| (k as usize) < x.len()))
                .collect();
            // The eyelets: pairs about as far apart as eyelets are.
            let mut eyelets: Vec<[u32; 2]> = Vec::new();
            for p in pairs {
                let far_enough = eyelets.last().is_none_or(|last| {
                    let d = sub(at(p[0]), at(last[0]));
                    dot(d, d).sqrt() >= 0.9 * EYELETS_APART * mm
                });
                if far_enough {
                    eyelets.push(p);
                }
            }
            let lifted = |k: u32, off: f32| add(at(k), scale(out_at(k), off * mm));
            for p in &eyelets {
                for &k in p {
                    disc(&mut out, lifted(k, 0.6), out_at(k), EYELET_RADIUS * mm);
                }
            }
            // Criss-crossed: from each eyelet across to the next one up on
            // the other side.
            for w in eyelets.windows(2) {
                let ([a0, b0], [a1, b1]) = (w[0], w[1]);
                for (from, to) in [(a0, b1), (b0, a1)] {
                    tube(
                        &mut out,
                        lifted(from, LACE_OFF),
                        lifted(to, LACE_OFF),
                        LACE_RADIUS * mm,
                    );
                }
            }
            // And straight across at the ends, where it is tied.
            for p in [eyelets.first(), eyelets.last()].into_iter().flatten() {
                tube(
                    &mut out,
                    lifted(p[0], LACE_OFF),
                    lifted(p[1], LACE_OFF),
                    LACE_RADIUS * mm,
                );
            }
        }
        out
    }
}

/// A cord from `a` to `b`, `r` thick: a tube of six sides.
fn tube(out: &mut Shapes, a: V3, b: V3, r: f32) {
    const SIDES: u32 = 6;
    let d = sub(b, a);
    if dot(d, d) < 1e-12 {
        return;
    }
    let along = unit(d);
    let helper = if along[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = unit(cross(along, helper));
    let v = cross(along, u);
    let first = out.points.len() as u32;
    for end in [a, b] {
        for s in 0..SIDES {
            let angle = s as f32 / SIDES as f32 * std::f32::consts::TAU;
            let off = add(scale(u, angle.cos() * r), scale(v, angle.sin() * r));
            out.points.push(add(end, off));
            out.kinds.push(Trim::Lace);
        }
    }
    for s in 0..SIDES {
        let (i, j) = (first + s, first + (s + 1) % SIDES);
        out.triangles.push([i, j, i + SIDES]);
        out.triangles.push([j, j + SIDES, i + SIDES]);
    }
}

/// An eyelet at `c`, facing `n`: a disc `r` across.
fn disc(out: &mut Shapes, c: V3, n: V3, r: f32) {
    const SIDES: u32 = 8;
    let helper = if n[1].abs() < 0.9 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let u = unit(cross(n, helper));
    let v = cross(n, u);
    let first = out.points.len() as u32;
    out.points.push(c);
    out.kinds.push(Trim::Eyelet);
    for s in 0..SIDES {
        let angle = s as f32 / SIDES as f32 * std::f32::consts::TAU;
        out.points.push(add(
            c,
            add(scale(u, angle.cos() * r), scale(v, angle.sin() * r)),
        ));
        out.kinds.push(Trim::Eyelet);
    }
    for s in 0..SIDES {
        out.triangles
            .push([first, first + 1 + s, first + 1 + (s + 1) % SIDES]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A strip of cloth 100 mm wide along x and 200 mm up, its points every
    /// 20 mm, facing +z.
    fn strip() -> (Vec<V3>, Vec<[u32; 3]>) {
        let mut x = Vec::new();
        for j in 0..=10 {
            for i in 0..=5 {
                x.push([i as f32 * 20.0, j as f32 * 20.0, 0.0]);
            }
        }
        let mut t = Vec::new();
        for j in 0..10u32 {
            for i in 0..5u32 {
                let a = j * 6 + i;
                t.push([a, a + 1, a + 7]);
                t.push([a, a + 7, a + 6]);
            }
        }
        (x, t)
    }

    #[test]
    fn a_bone_stands_off_the_cloth_along_it() {
        let (x, t) = strip();
        // Up the middle of the strip, between its points: half each of two
        // corners of a triangle.
        let trims = Trims {
            bones: vec![
                (0..10)
                    .map(|j| ([j * 6 + 2, j * 6 + 3, j * 6 + 9], [0.5, 0.5, 0.0]))
                    .collect(),
            ],
            laces: Vec::new(),
        };
        let s = trims.shapes(&x, &t, &vec![1.0; t.len()], 1.0);
        // Two points across it at each of its 10.
        assert_eq!(s.points.len(), 20);
        assert_eq!(s.triangles.len(), 18);
        assert!(s.kinds.iter().all(|&k| k == Trim::Bone));
        for p in &s.points {
            assert!((p[2] - CHANNEL_OFF).abs() < 1e-4, "{p:?}");
            assert!((p[0] - 50.0).abs() <= CHANNEL_WIDTH / 2.0 + 1e-4, "{p:?}");
        }
    }

    #[test]
    fn a_lace_crosses_between_eyelets() {
        let (x, t) = strip();
        // The strip's two long sides laced together, every point.
        let pairs: Vec<[u32; 2]> = (0..=10).map(|j| [j * 6, j * 6 + 5]).collect();
        let trims = Trims {
            bones: Vec::new(),
            laces: vec![pairs],
        };
        let s = trims.shapes(&x, &t, &vec![1.0; t.len()], 1.0);
        // Eyelets 40 mm apart (every other point: 20 mm is closer than
        // eyelets are): 6 pairs, 12 eyelets.
        let eyelets = s.kinds.iter().filter(|&&k| k == Trim::Eyelet).count();
        assert_eq!(eyelets, 12 * 9);
        // Five steps, crossed both ways, and straight across at both ends:
        // 12 cords.
        let cords = s.kinds.iter().filter(|&&k| k == Trim::Lace).count();
        assert_eq!(cords, 12 * 12);
        // In front of the cloth.
        assert!(s.points.iter().all(|p| p[2] > 0.0));
    }

    #[test]
    fn nothing_without_trims() {
        let (x, t) = strip();
        assert_eq!(
            Trims::default().shapes(&x, &t, &vec![1.0; t.len()], 1.0),
            Shapes::default()
        );
    }
}
