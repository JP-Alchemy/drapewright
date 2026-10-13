//! A garment's own size round the body at the figure's bust, waist and hip,
//! as a sewist measures a finished garment: where the cloth meets each
//! height, measured flat, not as stretched round the body. Sleeves round an
//! arm are not counted.

use crate::garment::Garment;
use crate::place::Figure;

/// How big the garment is round the body at each of the figure's levels:
/// (level, the garment's girth there, mm), none where the garment doesn't
/// reach. `points`: each panel's points, mm, as `place` puts them or the
/// drape has them.
pub fn girths(
    g: &Garment,
    points: &[Vec<[f64; 3]>],
    figure: &Figure,
) -> Vec<(&'static str, Option<f64>)> {
    let trunk: Vec<bool> = (0..g.panels.len())
        .map(|p| !round_an_arm(&points[p], figure))
        .collect();
    figure
        .levels
        .iter()
        .map(|level| (level.name, girth_at(g, points, &trunk, level.y)))
        .collect()
}

/// Whether a panel's points are nearer an arm than the trunk: a sleeve.
pub(crate) fn round_an_arm(points: &[[f64; 3]], figure: &Figure) -> bool {
    if figure.arms.is_empty() || points.is_empty() {
        return false;
    }
    let nearer = points
        .iter()
        .filter(|p| {
            let m = figure.middle(p[1]);
            let to_trunk = (p[0] - m[0]).hypot(p[2] - m[1]);
            let to_arm = figure
                .arms
                .iter()
                .flat_map(|arm| arm.line.iter())
                .map(|q| {
                    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
                })
                .fold(f64::MAX, f64::min);
            to_arm < to_trunk
        })
        .count();
    2 * nearer > points.len()
}

/// The garment's girth at height `y`: each triangle of a trunk panel that
/// crosses it, measured flat between where its edges cross; none where no
/// cloth crosses it.
fn girth_at(g: &Garment, points: &[Vec<[f64; 3]>], trunk: &[bool], y: f64) -> Option<f64> {
    let mut total = 0.0;
    let mut crossed = false;
    for (p, panel) in g.panels.iter().enumerate() {
        if !trunk[p] {
            continue;
        }
        let placed = &points[p];
        let flat = &panel.mesh.points;
        for t in &panel.mesh.triangles {
            let mut ends: Vec<[f64; 2]> = Vec::with_capacity(2);
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (a, b) = (a as usize, b as usize);
                let (ya, yb) = (placed[a][1] - y, placed[b][1] - y);
                // Crossing the height: one end above, the other on or below.
                if (ya > 0.0) != (yb > 0.0) {
                    let s = ya / (ya - yb);
                    ends.push([
                        flat[a][0] + (flat[b][0] - flat[a][0]) * s,
                        flat[a][1] + (flat[b][1] - flat[a][1]) * s,
                    ]);
                }
            }
            if let [e, f] = ends[..] {
                total += (e[0] - f[0]).hypot(e[1] - f[1]);
                crossed = true;
            }
        }
    }
    crossed.then_some(total)
}
