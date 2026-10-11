//! Measuring a figure as a tape measure would. Round measurements are taken
//! around the outside of a slice of the body (a tape bridges hollows, as
//! between the breasts): the bust at the bust point, the underbust at the
//! fold under the breasts, the waist at the narrowest (level with the elbow
//! on a figure with no waist), the hip at the seat's fullest; the high bust
//! under the arms and over the chest above the breasts. The neck is
//! measured around its base; the armhole, where the arm's skin weights give
//! way to the torso's, gives the armpit. Lengths follow the body's back and
//! front. Landmarks are chosen smoothly (weighted towards the best of their
//! candidates), so that measurements change smoothly with the figure, as
//! fitting needs. Millimetres; measurements are named as in a project's
//! measurement sets.

use std::collections::BTreeMap;

use crate::asset::Region;
use crate::shape::{Body, BodyModel};

/// A plane: points `p` with `normal · p = offset`.
#[derive(Clone, Copy, Debug)]
struct Plane {
    normal: [f64; 3],
    offset: f64,
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}

fn unit(a: [f64; 3]) -> [f64; 3] {
    let l = length(a).max(1e-12);
    [a[0] / l, a[1] / l, a[2] / l]
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    length(sub(a, b))
}

/// The convex hull of points in a plane, anticlockwise (monotone chain).
fn hull(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turn = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut lower: Vec<[f64; 2]> = Vec::new();
    for &p in &points {
        while lower.len() >= 2 && turn(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    let mut upper: Vec<[f64; 2]> = Vec::new();
    for &p in points.iter().rev() {
        while upper.len() >= 2 && turn(upper[upper.len() - 2], upper[upper.len() - 1], p) <= 0.0 {
            upper.pop();
        }
        upper.push(p);
    }
    lower.pop();
    upper.pop();
    lower.extend(upper);
    lower
}

fn perimeter(points: &[[f64; 2]]) -> f64 {
    let n = points.len();
    if n < 2 {
        return 0.0;
    }
    (0..n)
        .map(|i| {
            let (a, b) = (points[i], points[(i + 1) % n]);
            (a[0] - b[0]).hypot(a[1] - b[1])
        })
        .sum()
}

/// A choice that moves smoothly as the figure changes, so that measurements
/// do too: of `candidates` (score, point), lower scores better, the points'
/// average weighted towards the best, those within a few `soft` of it
/// counting.
fn softest(candidates: impl IntoIterator<Item = (f64, [f64; 3])>, soft: f64) -> Option<[f64; 3]> {
    let all: Vec<(f64, [f64; 3])> = candidates.into_iter().collect();
    let best = all.iter().map(|c| c.0).fold(f64::MAX, f64::min);
    let mut sum = [0.0; 3];
    let mut total = 0.0;
    for (score, p) in &all {
        let w = (-(score - best) / soft).exp();
        total += w;
        sum.iter_mut().zip(p).for_each(|(a, b)| *a += w * b);
    }
    (total > 0.0).then(|| sum.map(|v| v / total))
}

/// A body ready to measure: its triangles sorted into the regions measured.
struct Measurer<'a> {
    model: &'a BodyModel,
    body: &'a Body,
}

/// Region sets.
const TRUNK: &[Region] = &[Region::Torso, Region::LegLeft, Region::LegRight];
const TORSO: &[Region] = &[Region::Torso];
const ARM: &[Region] = &[Region::ArmLeft, Region::ShoulderLeft];
const LEG: &[Region] = &[Region::LegLeft];

/// Triangles sorted into bands of height, so that a level slice looks only
/// at those near it.
struct Bands {
    bottom: f64,
    bands: Vec<Vec<[u32; 3]>>,
}

/// The bands' height, mm.
const BAND: f64 = 20.0;

impl Bands {
    fn new(m: &Measurer, triangles: &[[u32; 3]]) -> Self {
        let span = |t: &[u32; 3]| {
            let y = t.map(|v| m.point(v)[1]);
            (y[0].min(y[1]).min(y[2]), y[0].max(y[1]).max(y[2]))
        };
        let bottom = triangles.iter().map(|t| span(t).0).fold(f64::MAX, f64::min);
        let mut bands: Vec<Vec<[u32; 3]>> = Vec::new();
        for t in triangles {
            let (lo, hi) = span(t);
            let (a, b) = (
                ((lo - bottom) / BAND) as usize,
                ((hi - bottom) / BAND) as usize,
            );
            if bands.len() <= b {
                bands.resize(b + 1, Vec::new());
            }
            for band in &mut bands[a..=b] {
                band.push(*t);
            }
        }
        Bands { bottom, bands }
    }

    /// The triangles that may cross height `y`.
    fn at(&self, y: f64) -> &[[u32; 3]] {
        let i = (y - self.bottom) / BAND;
        if i < 0.0 {
            return &[];
        }
        self.bands.get(i as usize).map_or(&[], Vec::as_slice)
    }
}

impl<'a> Measurer<'a> {
    fn region(&self, v: u32) -> Option<Region> {
        self.model.region(v as usize)
    }

    fn point(&self, v: u32) -> [f64; 3] {
        self.body.vertices[v as usize]
    }

    /// Triangles all of whose corners are in `regions`.
    fn triangles(&self, regions: &[Region]) -> Vec<[u32; 3]> {
        let outside = !regions.iter().fold(0u16, |m, r| m | 1 << *r as u16);
        self.model
            .asset
            .triangles
            .iter()
            .zip(self.model.corner_regions())
            .filter(|(_, corners)| *corners & outside == 0)
            .map(|(t, _)| *t)
            .collect()
    }

    /// Where `triangles` cross `plane`.
    fn slice(&self, triangles: &[[u32; 3]], plane: Plane) -> Vec<[f64; 3]> {
        let mut out = Vec::new();
        for t in triangles {
            let p = t.map(|v| self.point(v));
            let d = p.map(|q| dot(plane.normal, q) - plane.offset);
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                if (d[i] < 0.0) != (d[j] < 0.0) {
                    let s = d[i] / (d[i] - d[j]);
                    out.push([
                        p[i][0] + s * (p[j][0] - p[i][0]),
                        p[i][1] + s * (p[j][1] - p[i][1]),
                        p[i][2] + s * (p[j][2] - p[i][2]),
                    ]);
                }
            }
        }
        out
    }

    /// Around a level slice at height `y`, as a tape: the perimeter of its
    /// convex hull (seen from above).
    fn around(&self, bands: &Bands, y: f64) -> f64 {
        self.ring(bands.at(y), y)
    }

    /// Around the level slice of `triangles` at height `y`.
    fn ring(&self, triangles: &[[u32; 3]], y: f64) -> f64 {
        let points = self.slice(
            triangles,
            Plane {
                normal: [0.0, 1.0, 0.0],
                offset: y,
            },
        );
        perimeter(&hull(points.iter().map(|p| [p[0], p[2]]).collect()))
    }

    /// Around the slice across `axis` through `at` (a limb), as a tape.
    fn across(&self, triangles: &[[u32; 3]], at: [f64; 3], axis: [f64; 3]) -> f64 {
        let n = unit(axis);
        let points = self.slice(
            triangles,
            Plane {
                normal: n,
                offset: dot(n, at),
            },
        );
        // Measured in the plane: two directions across the axis.
        let any = if n[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u = unit(cross(n, any));
        let w = cross(n, u);
        // Only the slice through this limb, near the axis.
        let near: Vec<[f64; 2]> = points
            .iter()
            .filter(|p| distance(**p, at) < 150.0)
            .map(|p| {
                let r = sub(*p, at);
                [dot(r, u), dot(r, w)]
            })
            .collect();
        perimeter(&hull(near))
    }

    /// The midpoints of edges between two sets of regions: a line around
    /// the body where one gives way to the other.
    fn border(&self, a: &[Region], b: &[Region]) -> Vec<[f64; 3]> {
        let bits = |set: &[Region]| set.iter().fold(0u16, |m, r| m | 1 << *r as u16);
        let (in_a, in_b) = (bits(a), bits(b));
        let mut out = Vec::new();
        for (t, corners) in self
            .model
            .asset
            .triangles
            .iter()
            .zip(self.model.corner_regions())
        {
            if corners & in_a == 0 || corners & in_b == 0 {
                continue;
            }
            for (i, j) in [(0, 1), (1, 2), (2, 0)] {
                let (ri, rj) = (self.region(t[i]), self.region(t[j]));
                let crosses = ri.is_some_and(|r| a.contains(&r))
                    && rj.is_some_and(|r| b.contains(&r))
                    || ri.is_some_and(|r| b.contains(&r)) && rj.is_some_and(|r| a.contains(&r));
                if crosses {
                    let (p, q) = (self.point(t[i]), self.point(t[j]));
                    out.push([
                        (p[0] + q[0]) / 2.0,
                        (p[1] + q[1]) / 2.0,
                        (p[2] + q[2]) / 2.0,
                    ]);
                }
            }
        }
        out
    }

    /// The tightest line around points that lie roughly in a plane (a loop
    /// around the neck): their hull, in the plane they best fit.
    fn around_loop(points: &[[f64; 3]]) -> f64 {
        if points.len() < 3 {
            return 0.0;
        }
        let n = points.len() as f64;
        let c = points.iter().fold([0.0; 3], |s, p| {
            [s[0] + p[0] / n, s[1] + p[1] / n, s[2] + p[2] / n]
        });
        // The plane's normal: the direction the points spread least in.
        let mut cov = [[0.0f64; 3]; 3];
        for p in points {
            let d = sub(*p, c);
            for i in 0..3 {
                for j in 0..3 {
                    cov[i][j] += d[i] * d[j];
                }
            }
        }
        let normal = least_spread(cov);
        let any = if normal[0].abs() < 0.9 {
            [1.0, 0.0, 0.0]
        } else {
            [0.0, 1.0, 0.0]
        };
        let u = unit(cross(normal, any));
        let w = cross(normal, u);
        perimeter(&hull(
            points
                .iter()
                .map(|p| {
                    let d = sub(*p, c);
                    [dot(d, u), dot(d, w)]
                })
                .collect(),
        ))
    }

    /// Half of a tape round the torso, from one armpit to the other across
    /// the front (or the back): in the plane through both armpits that rises
    /// `rise` mm for each mm forward.
    fn half_around(&self, triangles: &[[u32; 3]], armpit: [f64; 3], rise: f64, front: bool) -> f64 {
        let normal = unit([0.0, 1.0, -rise]);
        let forward = unit([0.0, rise, 1.0]);
        let points = self.slice(
            triangles,
            Plane {
                normal,
                offset: dot(normal, armpit),
            },
        );
        // In the plane: across, and forward of the line between the armpits.
        let side: Vec<[f64; 2]> = points
            .iter()
            .map(|p| [p[0], dot(sub(*p, armpit), forward)])
            .filter(|q| (q[1] > 0.0) == front)
            .chain([[armpit[0], 0.0], [-armpit[0], 0.0]])
            .collect();
        perimeter(&hull(side)) - 2.0 * armpit[0].abs()
    }

    /// Over a range of heights, the height whose slice measures most (or
    /// least) around, and that measure.
    fn search(&self, bands: &Bands, from: f64, to: f64, most: bool) -> (f64, f64) {
        let steps = ((to - from).abs() / 4.0).ceil().max(1.0) as usize;
        (0..=steps)
            .map(|k| {
                let y = from + (to - from) * k as f64 / steps as f64;
                (y, self.around(bands, y))
            })
            .filter(|(_, m)| *m > 0.0)
            .max_by(|a, b| {
                if most {
                    a.1.total_cmp(&b.1)
                } else {
                    b.1.total_cmp(&a.1)
                }
            })
            .unwrap_or((from, 0.0))
    }

    /// The outline of the body seen from the side, in the vertical plane
    /// `x`, at the back or the front, between two heights: its length.
    fn profile(&self, triangles: &[[u32; 3]], x: f64, from: f64, to: f64, back: bool) -> f64 {
        let (lo, hi) = (from.min(to), from.max(to));
        let points = self.slice(
            triangles,
            Plane {
                normal: [1.0, 0.0, 0.0],
                offset: x,
            },
        );
        // The outline is known where the mesh's edges cross the plane: take
        // it a little beyond both heights, then cut it at them.
        let reach = 30.0;
        let band: Vec<&[f64; 3]> = points
            .iter()
            .filter(|p| p[1] >= lo - reach && p[1] <= hi + reach)
            .collect();
        if band.is_empty() {
            return 0.0;
        }
        // The front and the back are apart: split them halfway.
        let (zmin, zmax) = band
            .iter()
            .filter(|p| p[1] >= lo && p[1] <= hi)
            .fold((f64::MAX, f64::MIN), |(a, b), p| (a.min(p[2]), b.max(p[2])));
        let split = (zmin + zmax) / 2.0;
        // A tape bridges hollows (under the bust, the small of the back):
        // it follows the outline's outer edge (its hull), not every dip.
        let mut line = outer_edge(
            band.iter()
                .filter(|p| (p[2] < split) == back)
                .map(|p| [p[1], if back { p[2] } else { -p[2] }])
                .collect(),
        );
        line.sort_by(|a, b| a[0].total_cmp(&b[0]));
        line.windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                let length = (a[0] - b[0]).hypot(a[1] - b[1]);
                let rise = b[0] - a[0];
                if rise <= 0.0 {
                    return if a[0] >= lo && a[0] <= hi {
                        length
                    } else {
                        0.0
                    };
                }
                let inside = (b[0].min(hi) - a[0].max(lo)).max(0.0);
                length * inside / rise
            })
            .sum()
    }
}

/// Points `[along, depth]`, kept where a straight edge laid against their
/// smallest-depth side would touch them (the lower hull, in order along).
fn outer_edge(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let turn = |o: [f64; 2], a: [f64; 2], b: [f64; 2]| {
        (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])
    };
    let mut lower: Vec<[f64; 2]> = Vec::new();
    for p in points {
        while lower.len() >= 2 && turn(lower[lower.len() - 2], lower[lower.len() - 1], p) <= 0.0 {
            lower.pop();
        }
        lower.push(p);
    }
    lower
}

/// The direction a symmetric 3×3 matrix spreads least in (power iteration
/// on its shifted inverse would do; with three dimensions, Jacobi is enough).
#[allow(clippy::needless_range_loop)] // Row and column indices read as the maths does.
fn least_spread(mut a: [[f64; 3]; 3]) -> [f64; 3] {
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..50 {
        for (p, q) in [(0, 1), (0, 2), (1, 2)] {
            if a[p][q].abs() < 1e-12 {
                continue;
            }
            let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
            let t = if theta >= 0.0 { 1.0 } else { -1.0 }
                / (theta.abs() + (theta * theta + 1.0).sqrt());
            let c = 1.0 / (t * t + 1.0).sqrt();
            let s = t * c;
            for row in a.iter_mut() {
                let (kp, kq) = (row[p], row[q]);
                row[p] = c * kp - s * kq;
                row[q] = s * kp + c * kq;
            }
            for k in 0..3 {
                let (pk, qk) = (a[p][k], a[q][k]);
                a[p][k] = c * pk - s * qk;
                a[q][k] = s * pk + c * qk;
            }
            for row in v.iter_mut() {
                let (kp, kq) = (row[p], row[q]);
                row[p] = c * kp - s * kq;
                row[q] = s * kp + c * kq;
            }
        }
    }
    let k = (0..3)
        .min_by(|&i, &j| a[i][i].total_cmp(&a[j][j]))
        .unwrap_or(0);
    [v[0][k], v[1][k], v[2][k]]
}

/// Around the torso (`trunk`: with the tops of the legs, for the hip) at
/// height `y`, as a tape: for comparing many figures quickly.
pub fn around_at(model: &BodyModel, body: &Body, y: f64, trunk: bool) -> f64 {
    let m = Measurer { model, body };
    let near: Vec<[u32; 3]> = m
        .triangles(if trunk { TRUNK } else { TORSO })
        .into_iter()
        .filter(|t| {
            let y3 = t.map(|v| m.point(v)[1]);
            y3.iter().any(|&a| a <= y) && y3.iter().any(|&a| a >= y)
        })
        .collect();
    m.ring(&near, y)
}

/// Where measurements are taken, for checking them by eye.
#[derive(Clone, Debug, Default)]
pub struct Landmarks {
    pub points: BTreeMap<&'static str, [f64; 3]>,
    /// Heights of the round measurements' slices.
    pub levels: BTreeMap<&'static str, f64>,
}

/// A figure's measurements, mm, by the names a project's measurement sets
/// use, and where they were taken.
pub fn measure(model: &BodyModel, body: &Body) -> (BTreeMap<&'static str, f64>, Landmarks) {
    let m = Measurer { model, body };
    let mut out = BTreeMap::new();
    let mut marks = Landmarks::default();
    let shown = &body.vertices[..model.asset.shown as usize];
    let height = shown.iter().map(|p| p[1]).fold(0.0, f64::max);
    out.insert("height", height);

    let joint = |name: &str| model.joint(body, name).unwrap_or([0.0, height / 2.0, 0.0]);
    let trunk = Bands::new(&m, &m.triangles(TRUNK));
    let torso_triangles = m.triangles(TORSO);
    let torso = Bands::new(&m, &torso_triangles);

    // The crotch: the lowest point on the middle line between the legs.
    let crotch = softest(
        shown
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                p[0].abs() < 5.0
                    && m.region(*i as u32).is_some_and(|r| TRUNK.contains(&r))
                    && p[1] > 0.3 * height
            })
            .map(|(_, p)| (p[1], *p)),
        3.0,
    )
    .map_or(0.45 * height, |p| p[1]);

    // The armhole (left): where the arm's skin weights give way to the
    // torso's. Its lowest point is the armpit.
    let armhole = m.border(&[Region::Torso], &[Region::ArmLeft, Region::ShoulderLeft]);
    let Some(armpit) = softest(armhole.iter().map(|p| (p[1], *p)), 3.0) else {
        return (out, marks);
    };
    marks.points.insert("armpit", armpit);
    // The shoulder point: the top of the shoulder, straight above its joint.
    let sj = joint("l-shoulder");
    let shoulder = softest(
        shown
            .iter()
            .filter(|p| (p[0] - sj[0]).abs() < 15.0 && (p[2] - sj[2]).abs() < 40.0 && p[1] > sj[1])
            .map(|p| (-p[1], *p)),
        3.0,
    )
    .unwrap_or(sj);
    marks.points.insert("shoulderPoint", shoulder);

    // The neck at its base, where a collar sits: across the neck, lower at
    // the front. Going up from the neck joint, the slices narrow quickly out
    // of the shoulders, then hardly at all: the base is where they first
    // come within 8% of the neck's narrowest.
    let neck_joint = joint("neck");
    let axis = unit(sub(joint("head"), neck_joint));
    let normal = unit([axis[0], axis[1], axis[2] + 0.36]);
    let upper = m.triangles(&[
        Region::Head,
        Region::Neck,
        Region::Torso,
        Region::ShoulderLeft,
        Region::ShoulderRight,
    ]);
    // The rings below lie within reach of the neck joint.
    let near_neck: Vec<[u32; 3]> = upper
        .iter()
        .filter(|t| t.iter().any(|&v| distance(m.point(v), neck_joint) < 250.0))
        .copied()
        .collect();
    let ring_at = |t: f64| -> Vec<[f64; 3]> {
        let at = [
            neck_joint[0] + t * axis[0],
            neck_joint[1] + t * axis[1],
            neck_joint[2] + t * axis[2],
        ];
        m.slice(
            &near_neck,
            Plane {
                normal,
                offset: dot(normal, at),
            },
        )
        .into_iter()
        .filter(|p| distance(*p, at) < 110.0)
        .collect()
    };
    let around: Vec<(f64, f64)> = (0..=25)
        .map(|k| {
            let t = 4.0 * f64::from(k);
            (t, Measurer::around_loop(&ring_at(t)))
        })
        .filter(|(_, p)| *p > 0.0)
        .collect();
    let narrowest = around
        .iter()
        .filter(|(t, _)| *t >= 20.0)
        .map(|(_, p)| *p)
        .fold(f64::MAX, f64::min);
    let (base, neck) = around
        .iter()
        .copied()
        .find(|(_, p)| *p <= 1.08 * narrowest)
        .unwrap_or((0.0, 0.0));
    out.insert("neck", neck);
    let ring = ring_at(base);
    let nape = ring
        .iter()
        .copied()
        .min_by(|a, b| a[2].total_cmp(&b[2]))
        .unwrap_or(neck_joint);
    let neck_side = ring
        .iter()
        .copied()
        .max_by(|a, b| a[0].total_cmp(&b[0]))
        .unwrap_or(nape);
    marks.points.insert("nape", nape);
    marks.points.insert("neckSide", neck_side);

    // Round the trunk. The bust at the bust point, the chest's most forward
    // point below the armpit, on the ribs: above the spine's joint at the
    // bottom of the ribcage, as a fuller figure's stomach below can stand
    // further forward.
    let ribs = joint("spine-2")[1];
    let bust_point = softest(
        shown
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                p[0] > 30.0
                    && p[0] < armpit[0]
                    && p[1] < armpit[1] - 20.0
                    && p[1] > ribs
                    && m.region(*i as u32) == Some(Region::Torso)
            })
            .map(|(_, p)| (-p[2], *p)),
        2.0,
    )
    .unwrap_or([0.0, armpit[1] - 100.0, 0.0]);
    let bust_y = bust_point[1];
    let bust = m.around(&torso, bust_y);
    out.insert("bust", bust);
    marks.levels.insert("bust", bust_y);
    out.insert("bustSpan", 2.0 * bust_point[0]);
    marks.points.insert("bustPoint", bust_point);

    // The high bust as it is measured: under the arms, level across the
    // back, and over the chest above the breasts in front, where the tape
    // rises to pass the middle 3% of the height above the armpits.
    let chest = m
        .slice(
            &torso_triangles,
            Plane {
                normal: [0.0, 1.0, 0.0],
                offset: armpit[1],
            },
        )
        .iter()
        .filter(|p| p[0].abs() < 15.0)
        .map(|p| p[2])
        .fold(f64::MIN, f64::max);
    let rise = if chest > armpit[2] {
        0.03 * height / (chest - armpit[2])
    } else {
        0.0
    };
    out.insert(
        "highBust",
        m.half_around(&torso_triangles, armpit, 0.0, false)
            + m.half_around(&torso_triangles, armpit, rise, true),
    );

    // The hip at the seat's fullest: the buttocks' furthest back point.
    let span = bust_y - crotch;
    let hip_y = softest(
        shown
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                p[0].abs() < 100.0
                    && p[1] > crotch
                    && p[1] < crotch + 0.5 * span
                    && m.region(*i as u32).is_some_and(|r| TRUNK.contains(&r))
            })
            .map(|(_, p)| (p[2], *p)),
        2.0,
    )
    .map_or(crotch + 0.2 * span, |p| p[1]);
    let hip = m.around(&trunk, hip_y);
    out.insert("hip", hip);
    marks.levels.insert("hip", hip_y);

    // Where the elbow would be with the arm hanging: about the waist's
    // height.
    let elbow_y = sj[1] - distance(sj, joint("l-elbow"));

    // Under the bust: the fold, where the breast gives way to the ribs.
    // Going down from the bust point the girth falls, fastest over the
    // breast's lower part: the fold is where that slows to half, whether
    // the breast rounds into the ribs or hangs over a fuller stomach.
    let bottom = elbow_y - 0.03 * height;
    let steps = ((bust_y - bottom) / 4.0).ceil().max(2.0) as usize;
    let ys: Vec<f64> = (0..=steps)
        .map(|k| bust_y + (bottom - bust_y) * k as f64 / steps as f64)
        .collect();
    let girths: Vec<f64> = ys.iter().map(|&y| m.around(&torso, y)).collect();
    // How fast the girth falls, per mm down, between neighbouring levels.
    let falls: Vec<(f64, f64)> = ys
        .windows(2)
        .zip(girths.windows(2))
        .map(|(y, g)| ((y[0] + y[1]) / 2.0, (g[0] - g[1]) / (y[0] - y[1])))
        .collect();
    let steepest = (0..falls.len())
        .max_by(|&a, &b| falls[a].1.total_cmp(&falls[b].1))
        .unwrap_or(0);
    let half = falls.get(steepest).map_or(0.0, |f| f.1 / 2.0);
    let under_y = (steepest + 1..falls.len())
        .find(|&k| half > 0.0 && falls[k].1 < half)
        .map_or(bust_y - 0.03 * height, |k| {
            let ((y0, f0), (y1, f1)) = (falls[k - 1], falls[k]);
            y0 + (y1 - y0) * (f0 - half) / (f0 - f1)
        });
    out.insert("underbust", m.around(&torso, under_y));
    marks.levels.insert("underbust", under_y);

    // The waist: the narrowest between the hip and just below the fold. A
    // figure that only widens from under the bust down has no narrowest
    // below it; there a tape goes where the body bends sideways, level with
    // the elbow. In between, the less the waist goes in, the nearer the
    // elbow it is taken.
    let lo = hip_y + 0.3 * (bust_y - hip_y);
    let hi = (under_y - 10.0).max(lo);
    let steps = ((hi - lo) / 4.0).ceil().max(1.0) as usize;
    let rings: Vec<[f64; 3]> = (0..=steps)
        .map(|k| lo + (hi - lo) * k as f64 / steps as f64)
        .map(|y| [0.0, y, m.around(&torso, y)])
        .filter(|r| r[2] > 0.0)
        .collect();
    let waist_y = softest(rings.iter().map(|r| (r[2], *r)), 3.0).map_or(elbow_y, |n| {
        let below_fold = rings.last().map_or(n[2], |r| r[2]);
        let goes_in = ((below_fold - n[2]) / 20.0).clamp(0.0, 1.0);
        goes_in * n[1] + (1.0 - goes_in) * elbow_y.clamp(lo, hi)
    });
    out.insert("waist", m.around(&torso, waist_y));
    marks.levels.insert("waist", waist_y);
    let high_hip_y = waist_y - 0.4 * (waist_y - hip_y);
    out.insert("highHip", m.around(&trunk, high_hip_y));
    marks.levels.insert("highHip", high_hip_y);
    out.insert("waistToHip", waist_y - hip_y);

    // Lengths down the back and front.
    out.insert(
        "backWaistLength",
        m.profile(&upper, 0.0, nape[1], waist_y, true),
    );
    let to_bust = distance(neck_side, bust_point);
    out.insert("shoulderToBust", to_bust);
    out.insert(
        "frontWaistLength",
        to_bust
            + m.profile(
                &torso_triangles,
                bust_point[0],
                bust_point[1],
                waist_y,
                false,
            ),
    );
    out.insert("shoulderLength", distance(neck_side, shoulder));

    // Across the back and the front at the middle of the armhole, from
    // crease to crease, as a tape lies. With the arms held out, the creases
    // are taken straight above the armpit, just inside it.
    let mid = (armpit[1] + shoulder[1]) / 2.0;
    let upper_body = m.triangles(&[
        Region::Torso,
        Region::ShoulderLeft,
        Region::ShoulderRight,
        Region::ArmLeft,
        Region::ArmRight,
    ]);
    let torso_mid = m.slice(
        &upper_body,
        Plane {
            normal: [0.0, 1.0, 0.0],
            offset: mid,
        },
    );
    let crease_x = armpit[0] - 20.0;
    let arc = |front: bool| -> Option<(f64, [f64; 3])> {
        // The surface straight above the armpit, at the front or back.
        let edge = torso_mid
            .iter()
            .filter(|p| (p[0] - crease_x).abs() < 6.0)
            .map(|p| p[2])
            .fold(None, |best: Option<f64>, z| {
                Some(best.map_or(z, |b| if front { b.max(z) } else { b.min(z) }))
            })?;
        let c = [crease_x, mid, edge];
        // The part of the slice in front of (or behind) the creases, closed
        // by the line between them: the arc is the rest of its hull.
        let mut pts: Vec<[f64; 2]> = torso_mid
            .iter()
            .filter(|p| p[0].abs() <= crease_x && ((p[2] >= edge) == front))
            .map(|p| [p[0], p[2]])
            .collect();
        pts.push([crease_x, edge]);
        pts.push([-crease_x, edge]);
        Some((perimeter(&hull(pts)) - 2.0 * crease_x, c))
    };
    if let Some((across, c)) = arc(true) {
        out.insert("acrossFront", across);
        marks.points.insert("frontCrease", c);
    }
    if let Some((across, c)) = arc(false) {
        out.insert("acrossBack", across);
        marks.points.insert("backCrease", c);
    }
    let shoulder_r = [-shoulder[0], shoulder[1], shoulder[2]];
    let over_back = m.triangles(&[
        Region::Torso,
        Region::ShoulderLeft,
        Region::ShoulderRight,
        Region::Neck,
    ]);
    let plane_normal = unit(cross(
        sub(shoulder_r, shoulder),
        sub([0.0, nape[1] - 60.0, nape[2]], shoulder),
    ));
    let points = m.slice(
        &over_back,
        Plane {
            normal: plane_normal,
            offset: dot(plane_normal, shoulder),
        },
    );
    let behind: f64 = {
        // The slice's back edge from shoulder point to shoulder point, as a
        // tape lies over it: the hull of its back half, seen from behind.
        let mut pts: Vec<[f64; 2]> = points
            .iter()
            .filter(|p| p[2] < (shoulder[2] + nape[2]) / 2.0 + 30.0 && p[0].abs() <= shoulder[0])
            .map(|p| [p[0], p[2]])
            .collect();
        pts.push([shoulder[0], shoulder[2]]);
        pts.push([-shoulder[0], shoulder[2]]);
        let ring = hull(pts);
        perimeter(&ring) - 2.0 * shoulder[0].abs()
    };
    out.insert("shoulderWidth", behind.max(2.0 * shoulder[0]));

    // Arms (the left), from the joints.
    let (el, wr) = (joint("l-elbow"), joint("l-hand"));
    out.insert("armLength", distance(shoulder, el) + distance(el, wr));
    let arm = m.triangles(ARM);
    // The fullest part of the upper arm, below the shoulder's deltoid.
    let upper = (7..=14)
        .map(|k| {
            let t = f64::from(k) / 20.0;
            let at = [
                sj[0] + t * (el[0] - sj[0]),
                sj[1] + t * (el[1] - sj[1]),
                sj[2] + t * (el[2] - sj[2]),
            ];
            m.across(&arm, at, sub(el, sj))
        })
        .fold(0.0, f64::max);
    out.insert("upperArm", upper);
    let near_wrist = [
        el[0] + 0.95 * (wr[0] - el[0]),
        el[1] + 0.95 * (wr[1] - el[1]),
        el[2] + 0.95 * (wr[2] - el[2]),
    ];
    out.insert(
        "wrist",
        m.across(&m.triangles(&[Region::ArmLeft]), near_wrist, sub(wr, el)),
    );

    // Legs (the left).
    let leg = Bands::new(&m, &m.triangles(LEG));
    let (knee, ankle) = (joint("l-knee"), joint("l-ankle"));
    out.insert("inseam", crotch);
    out.insert("outseam", waist_y);
    out.insert("crotchDepth", waist_y - crotch);
    out.insert(
        "thigh",
        m.search(&leg, crotch - 80.0, crotch - 10.0, true).1,
    );
    out.insert("knee", m.around(&leg, knee[1]));
    out.insert(
        "calf",
        m.search(&leg, knee[1] - 250.0, knee[1] - 50.0, true).1,
    );
    out.insert("ankle", m.around(&leg, ankle[1] + 40.0));
    marks.points.insert("crotch", [0.0, crotch, 0.0]);
    (out, marks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tape_goes_round_the_outside() {
        // A square with a dent in one side: the tape bridges the dent.
        let ring = hull(vec![
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [5.0, 8.0],
            [0.0, 10.0],
        ]);
        assert!((perimeter(&ring) - 40.0).abs() < 1e-9);
        // The flattest direction of points on the plane y = 3.
        let mut cov = [[0.0; 3]; 3];
        for p in [
            [1.0, 3.0, 0.0],
            [-1.0, 3.0, 0.0],
            [0.0, 3.0, 2.0],
            [0.0, 3.0, -2.0],
        ] {
            for i in 0..3 {
                for j in 0..3 {
                    cov[i][j] += (p[i] - [0.0, 3.0, 0.0][i]) * (p[j] - [0.0, 3.0, 0.0][j]);
                }
            }
        }
        let n = least_spread(cov);
        assert!((n[1].abs() - 1.0).abs() < 1e-9, "{n:?}");
    }
}
