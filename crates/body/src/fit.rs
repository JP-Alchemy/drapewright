//! Fitting a figure to a person's measurements. First the overall settings
//! (height, weight, muscle, cup size) to the height, bust, waist and hip,
//! starting from the best of a coarse grid; then those with MakeHuman's
//! measure shapes to every measurement given. What cannot be matched is
//! reported, measurement by measurement.

use std::collections::BTreeMap;

use crate::measure::{around_at, measure};
use crate::shape::{BodyModel, Shape};

/// The local shapes that change each measurement (the two arm lengths, and
/// the two leg lengths, share the measurement between them).
const LOCALS: &[(&str, &[&str])] = &[
    ("bust", &["measure-bust-circ-incr"]),
    ("underbust", &["measure-underbust-circ-incr"]),
    ("waist", &["measure-waist-circ-incr"]),
    ("hip", &["measure-hips-circ-incr"]),
    ("neck", &["measure-neck-circ-incr"]),
    ("shoulderWidth", &["measure-shoulder-dist-incr"]),
    ("acrossFront", &["measure-frontchest-dist-incr"]),
    ("backWaistLength", &["measure-napetowaist-dist-incr"]),
    ("waistToHip", &["measure-waisttohip-dist-incr"]),
    ("upperArm", &["measure-upperarm-circ-incr"]),
    (
        "armLength",
        &[
            "measure-upperarm-length-incr",
            "measure-lowerarm-length-incr",
        ],
    ),
    ("wrist", &["measure-wrist-circ-incr"]),
    ("thigh", &["measure-thigh-circ-incr"]),
    ("knee", &["measure-knee-circ-incr"]),
    ("calf", &["measure-calf-circ-incr"]),
    ("ankle", &["measure-ankle-circ-incr"]),
    (
        "inseam",
        &[
            "measure-upperleg-height-incr",
            "measure-lowerleg-height-incr",
        ],
    ),
    ("bustSpan", &["breast-dist-incr"]),
    ("shoulderToBust", &["breast-trans-up"]),
];

/// How closely a measurement is worth matching, mm: lengths more closely
/// than round measurements.
fn tolerance(name: &str) -> f64 {
    match name {
        "height" => 5.0,
        "bust" | "underbust" | "waist" | "hip" | "highHip" | "highBust" | "neck" => 8.0,
        _ => 5.0,
    }
}

/// Measurements the figure takes.
fn tolerance_known(name: &str) -> bool {
    matches!(
        name,
        "height"
            | "bust"
            | "highBust"
            | "underbust"
            | "waist"
            | "highHip"
            | "hip"
            | "neck"
            | "shoulderWidth"
            | "shoulderLength"
            | "acrossFront"
            | "acrossBack"
            | "backWaistLength"
            | "frontWaistLength"
            | "bustSpan"
            | "shoulderToBust"
            | "waistToHip"
            | "armLength"
            | "upperArm"
            | "wrist"
            | "inseam"
            | "outseam"
            | "crotchDepth"
            | "thigh"
            | "knee"
            | "calf"
            | "ankle"
    )
}

/// A fitted figure: its settings, its measurements, and how far each of
/// the person's measurements is from the figure's (figure − person), mm.
#[derive(Clone, Debug)]
pub struct Fit {
    pub shape: Shape,
    pub measured: BTreeMap<&'static str, f64>,
    pub residuals: BTreeMap<String, f64>,
}

/// The figure's measurements.
fn measured(model: &BodyModel, shape: &Shape) -> BTreeMap<&'static str, f64> {
    measure(model, &model.shape(shape)).0
}

/// Weight and muscle shape a figure together, and unevenly: at full weight
/// little muscle makes a much rounder figure than average muscle does, and
/// a step from the middle settings can stall where they change the
/// measurements little. So the search starts from the best of a coarse grid
/// of the two, figures compared quickly (round the trunk at the levels of
/// the starting figure) and in proportion to height. The bust is left to
/// the cup size on a woman's figure.
fn start_near(model: &BodyModel, shape: &mut Shape, targets: &BTreeMap<String, f64>, woman: bool) {
    let round: Vec<(&str, f64)> = ["bust", "waist", "hip"]
        .iter()
        .filter(|&&k| !(woman && k == "bust"))
        .filter_map(|&k| targets.get(k).map(|&t| (k, t)))
        .collect();
    if round.is_empty() {
        return;
    }
    // Where the starting figure is measured round, as shares of its height.
    let (taken, marks) = measure(model, &model.shape(shape));
    let height = taken.get("height").copied().unwrap_or(1.0).max(1.0);
    let score = |s: &Shape| -> f64 {
        let body = model.shape(s);
        let tall = body.vertices[..model.asset.shown as usize]
            .iter()
            .map(|p| p[1])
            .fold(1.0, f64::max);
        let scale = targets.get("height").map_or(1.0, |h| h / tall);
        round
            .iter()
            .map(|&(k, t)| {
                // The hip takes in the tops of the legs.
                let trunk = k == "hip";
                let share = marks.levels.get(k).map_or(0.5, |y| y / height);
                let r = (around_at(model, &body, share * tall, trunk) * scale - t) / tolerance(k);
                r * r
            })
            .sum()
    };
    let grid = [0.0, 0.25, 0.5, 0.75, 1.0];
    let best = grid
        .iter()
        .flat_map(|&weight| grid.iter().map(move |&muscle| (weight, muscle)))
        .map(|(weight, muscle)| {
            let s = Shape {
                weight,
                muscle,
                ..shape.clone()
            };
            (score(&s), weight, muscle)
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    if let Some((_, weight, muscle)) = best {
        shape.weight = weight;
        shape.muscle = muscle;
    }
}

/// Something a fit changes: an overall setting (0 to 1), or measure shapes
/// moved together (−1 to 1).
#[derive(Clone, Copy)]
enum Var {
    Height,
    Weight,
    Muscle,
    Cup,
    Local(&'static [&'static str]),
}

impl Var {
    fn get(self, s: &Shape) -> f64 {
        match self {
            Var::Height => s.height,
            Var::Weight => s.weight,
            Var::Muscle => s.muscle,
            Var::Cup => s.cup,
            Var::Local(names) => s.local.get(names[0]).copied().unwrap_or(0.0),
        }
    }

    fn put(self, s: &mut Shape, v: f64) {
        let (lo, hi) = self.range();
        let v = v.clamp(lo, hi);
        match self {
            Var::Height => s.height = v,
            Var::Weight => s.weight = v,
            Var::Muscle => s.muscle = v,
            Var::Cup => s.cup = v,
            Var::Local(names) => {
                for name in names {
                    s.local.insert((*name).to_string(), v);
                }
            }
        }
    }

    fn range(self) -> (f64, f64) {
        match self {
            Var::Local(_) => (-1.0, 1.0),
            _ => (0.0, 1.0),
        }
    }

    /// How far to move it to see how it moves the measurements.
    fn probe(self) -> f64 {
        match self {
            Var::Local(_) => 0.25,
            _ => 0.05,
        }
    }
}

/// Beyond about this many tolerances a measurement counts less and less, so
/// that one the figure cannot reach does not pull the others away.
const FAR: f64 = 4.0;

/// Moves `vars` so that the figure's measurements come closer to `goals`
/// (Levenberg–Marquardt on a Cauchy loss): how each moves each measurement
/// is worked out by moving each a little, and a damped step taken only
/// if the figure comes closer; if not, the step is damped harder and tried
/// again.
fn refine(
    model: &BodyModel,
    mut shape: Shape,
    vars: &[Var],
    goals: &[(&str, f64)],
    mut lambda: f64,
    rounds: usize,
) -> Shape {
    if vars.is_empty() || goals.is_empty() {
        return shape;
    }
    let residuals = |now: &BTreeMap<&'static str, f64>| -> Vec<f64> {
        goals
            .iter()
            .map(|(k, t)| (now.get(k).copied().unwrap_or(*t) - t) / tolerance(k))
            .collect()
    };
    let cost = |r: &[f64]| r.iter().map(|x| (x / FAR).powi(2).ln_1p()).sum::<f64>();
    let mut now = measured(model, &shape);
    let mut r = residuals(&now);
    for _ in 0..rounds {
        if r.iter().all(|x| x.abs() < 0.25) {
            break;
        }
        let columns: Vec<Vec<f64>> = vars
            .iter()
            .map(|&var| {
                let v = var.get(&shape);
                let up = if v + var.probe() <= var.range().1 {
                    var.probe()
                } else {
                    -var.probe()
                };
                let mut moved = shape.clone();
                var.put(&mut moved, v + up);
                let after = measured(model, &moved);
                goals
                    .iter()
                    .map(|(k, _)| {
                        (after.get(k).copied().unwrap_or(0.0) - now.get(k).copied().unwrap_or(0.0))
                            / up
                            / tolerance(k)
                    })
                    .collect()
            })
            .collect();
        let bounds: Vec<(f64, f64)> = vars
            .iter()
            .map(|&var| {
                let v = var.get(&shape);
                let (lo, hi) = var.range();
                (v - hi, v - lo)
            })
            .collect();
        // Far measurements weigh less (the loss's weights, as least squares).
        let weight: Vec<f64> = r
            .iter()
            .map(|x| (1.0 + (x / FAR).powi(2)).recip().sqrt())
            .collect();
        let columns: Vec<Vec<f64>> = columns
            .iter()
            .map(|c| c.iter().zip(&weight).map(|(a, w)| a * w).collect())
            .collect();
        let weighted: Vec<f64> = r.iter().zip(&weight).map(|(a, w)| a * w).collect();
        let mut closer = false;
        for _ in 0..6 {
            let delta = solve_bounded(&columns, &weighted, lambda, &bounds);
            let mut tried = shape.clone();
            for (&var, d) in vars.iter().zip(&delta) {
                var.put(&mut tried, var.get(&shape) - d);
            }
            let then = measured(model, &tried);
            let rt = residuals(&then);
            if cost(&rt) < cost(&r) {
                // Hardly closer: as close as it comes.
                closer = cost(&rt) < 0.999 * cost(&r);
                (shape, now, r) = (tried, then, rt);
                lambda = (lambda / 3.0).max(1e-4);
                break;
            }
            lambda *= 4.0;
        }
        if !closer {
            break;
        }
    }
    shape
}

/// Fits a woman's (`woman`) or a man's figure of `age` to `targets`
/// (measurement name → mm). Measurements the figure has no way to take are
/// left out of the residuals.
pub fn fit(model: &BodyModel, targets: &BTreeMap<String, f64>, woman: bool, age: f64) -> Fit {
    let mut shape = Shape {
        woman: if woman { 1.0 } else { 0.0 },
        age,
        ..Shape::default()
    };
    start_near(model, &mut shape, targets, woman);
    let goals: Vec<(&str, f64)> = targets
        .iter()
        .filter(|(k, _)| tolerance_known(k))
        .map(|(k, t)| (k.as_str(), *t))
        .collect();

    // The overall settings to the main measurements first.
    let mut vars = vec![Var::Height, Var::Weight, Var::Muscle];
    if woman {
        vars.push(Var::Cup);
    }
    let main: Vec<(&str, f64)> = goals
        .iter()
        .copied()
        .filter(|(k, _)| ["height", "bust", "waist", "hip"].contains(k))
        .collect();
    shape = refine(model, shape, &vars, &main, 0.05, 8);

    // Then with the measure shapes, against every measurement given.
    vars.extend(
        LOCALS
            .iter()
            .filter(|(k, shapes)| {
                targets.contains_key(*k)
                    && shapes.iter().all(|s| model.local_names().any(|n| n == *s))
            })
            .map(|(_, shapes)| Var::Local(shapes)),
    );
    shape = refine(model, shape, &vars, &goals, 1.0, 10);

    let measured = measured(model, &shape);
    let residuals = targets
        .iter()
        .filter_map(|(k, t)| measured.get(k.as_str()).map(|m| (k.clone(), m - t)))
        .collect();
    Fit {
        shape,
        measured,
        residuals,
    }
}

/// The damped step `x` (subtracted from the settings) for residuals `r`,
/// each part kept within its `bounds` (lowest, highest): solved, then
/// solved again for the parts not held at a bound.
fn solve_bounded(
    columns: &[Vec<f64>],
    residual: &[f64],
    lambda: f64,
    bounds: &[(f64, f64)],
) -> Vec<f64> {
    let n = columns.len();
    let mut fixed: Vec<Option<f64>> = vec![None; n];
    for _ in 0..n.max(1) {
        // The residual left once the fixed parts are taken.
        let mut r = residual.to_vec();
        for (c, f) in columns.iter().zip(&fixed) {
            if let Some(x) = f {
                r.iter_mut().zip(c).for_each(|(ri, ci)| *ri -= ci * x);
            }
        }
        let free: Vec<usize> = (0..n).filter(|&i| fixed[i].is_none()).collect();
        let cols: Vec<Vec<f64>> = free.iter().map(|&i| columns[i].clone()).collect();
        let x = solve_damped(&cols, &r, lambda);
        let mut changed = false;
        for (&i, xi) in free.iter().zip(&x) {
            let (lo, hi) = bounds[i];
            if *xi < lo || *xi > hi {
                fixed[i] = Some(xi.clamp(lo, hi));
                changed = true;
            }
        }
        if !changed {
            let mut out: Vec<f64> = fixed.iter().map(|f| f.unwrap_or(0.0)).collect();
            for (&i, xi) in free.iter().zip(&x) {
                out[i] = *xi;
            }
            return out;
        }
    }
    fixed.iter().map(|f| f.unwrap_or(0.0)).collect()
}

/// The step `x` minimising |J x − r|² + λ|x|², for J given by columns.
#[allow(clippy::needless_range_loop)] // Row and column indices read as the maths does.
fn solve_damped(columns: &[Vec<f64>], residual: &[f64], lambda: f64) -> Vec<f64> {
    let n = columns.len();
    // Normal equations: (JᵀJ + λI) x = Jᵀr.
    let mut a = vec![vec![0.0f64; n + 1]; n];
    for i in 0..n {
        for j in 0..n {
            a[i][j] = columns[i]
                .iter()
                .zip(&columns[j])
                .map(|(x, y)| x * y)
                .sum::<f64>();
        }
        a[i][i] += lambda;
        a[i][n] = columns[i]
            .iter()
            .zip(residual)
            .map(|(x, y)| x * y)
            .sum::<f64>();
    }
    // Gaussian elimination with partial pivoting.
    for c in 0..n {
        let pivot = (c..n)
            .max_by(|&x, &y| a[x][c].abs().total_cmp(&a[y][c].abs()))
            .unwrap_or(c);
        a.swap(c, pivot);
        if a[c][c].abs() < 1e-12 {
            continue;
        }
        for r in 0..n {
            if r != c {
                let f = a[r][c] / a[c][c];
                for k in c..=n {
                    a[r][k] -= f * a[c][k];
                }
            }
        }
    }
    (0..n)
        .map(|i| {
            if a[i][i].abs() < 1e-12 {
                0.0
            } else {
                a[i][n] / a[i][i]
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damped_steps_solve_small_systems() {
        // x + y = 3, x − y = 1 → x = 2, y = 1 (barely damped).
        let x = solve_damped(&[vec![1.0, 1.0], vec![1.0, -1.0]], &[3.0, 1.0], 1e-9);
        assert!(
            (x[0] - 2.0).abs() < 1e-6 && (x[1] - 1.0).abs() < 1e-6,
            "{x:?}"
        );
    }
}
