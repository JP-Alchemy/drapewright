//! A garment's own size round the body at the figure's bust, waist and hip,
//! as a sewist measures a finished garment: where the cloth meets each
//! height, measured flat, not as stretched round the body. Sleeves round an
//! arm are not counted.

use std::collections::{HashMap, HashSet};

use crate::garment::Garment;
use crate::place::{Figure, Level};

/// How big the garment is round the body at each of the figure's levels:
/// (level, the garment's girth there, mm), none where it doesn't go round
/// the body. `points`: each panel's points, mm, as `place` puts them or the
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
        .map(|level| (level.name, girth_at(g, points, &trunk, level)))
        .collect()
}

/// Whether a panel's points are nearer an arm than the trunk: a sleeve.
/// Nearer their surfaces, not their middles: a side panel of a garment
/// worn under the arm is nearer the arm's middle than the trunk's.
pub(crate) fn round_an_arm(points: &[[f64; 3]], figure: &Figure) -> bool {
    if figure.arms.is_empty() || points.is_empty() {
        return false;
    }
    let nearer = points
        .iter()
        .filter(|p| {
            let s = figure.section(p[1]);
            let (dx, dz) = (p[0] - s.middle[0], p[2] - s.middle[1]);
            let d = dx.hypot(dz);
            // The trunk's reach that way round, an ellipse.
            let [a, b] = s.half;
            let reach = if d > 0.0 {
                a * b / ((b * dx / d).powi(2) + (a * dz / d).powi(2)).sqrt()
            } else {
                0.0
            };
            let to_trunk = d - reach;
            let to_arm = figure
                .arms
                .iter()
                .flat_map(|arm| arm.line.iter().zip(&arm.radius))
                .map(|(q, r)| {
                    ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
                        - r
                })
                .fold(f64::MAX, f64::min);
            to_arm < to_trunk
        })
        .count();
    2 * nearer > points.len()
}

/// Cloth across a height that ends at more edges not sewn to anything than
/// this (a front buttoned shut, or a slit, is one opening: two ends) ...
const OPEN_ENDS: usize = 2;
/// ... and is less than this much of the body round there doesn't go
/// round the body: a bunnysuit cut up for the legs, at the hips. (An
/// armhole dipping below the bust is an opening too, but the cloth there
/// is about as big as the body.)
const MOSTLY: f64 = 0.75;

/// The garment's girth at a level: each triangle of a trunk panel that
/// crosses its height, measured flat between where its edges cross; none
/// where the cloth doesn't go round the body there.
fn girth_at(g: &Garment, points: &[Vec<[f64; 3]>], trunk: &[bool], level: &Level) -> Option<f64> {
    let y = level.y;
    let sewn: HashSet<(usize, u32)> = g.stitches.iter().flatten().copied().collect();
    let mut total = 0.0;
    let mut crossed = false;
    let mut open = 0;
    for (p, panel) in g.panels.iter().enumerate() {
        if !trunk[p] {
            continue;
        }
        let placed = &points[p];
        let flat = &panel.mesh.points;
        // The panel's outline: the sides of only one triangle.
        let mut sides: HashMap<(u32, u32), u32> = HashMap::new();
        for t in &panel.mesh.triangles {
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                *sides.entry((a.min(b), a.max(b))).or_default() += 1;
            }
        }
        for t in &panel.mesh.triangles {
            let mut ends: Vec<[f64; 2]> = Vec::with_capacity(2);
            for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
                let (i, j) = (a as usize, b as usize);
                let (ya, yb) = (placed[i][1] - y, placed[j][1] - y);
                // Crossing the height: one end above, the other on or below.
                if (ya > 0.0) != (yb > 0.0) {
                    let s = ya / (ya - yb);
                    ends.push([
                        flat[i][0] + (flat[j][0] - flat[i][0]) * s,
                        flat[i][1] + (flat[j][1] - flat[i][1]) * s,
                    ]);
                    // Across the outline where it isn't sewn: an end.
                    let outline = sides.get(&(a.min(b), a.max(b))) == Some(&1);
                    if outline && !(sewn.contains(&(p, a)) && sewn.contains(&(p, b))) {
                        open += 1;
                    }
                }
            }
            if let [e, f] = ends[..] {
                total += (e[0] - f[0]).hypot(e[1] - f[1]);
                crossed = true;
            }
        }
    }
    let round = open <= OPEN_ENDS || total >= MOSTLY * level.girth;
    (crossed && round).then_some(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::place::{Arm, Section};

    /// A trunk 300 mm wide and 200 deep, an arm 90 mm thick hanging close
    /// beside it, 230 mm out from its middle.
    fn figure() -> Figure {
        let heights: Vec<f64> = (0..=25).map(|k| 900.0 + 20.0 * f64::from(k)).collect();
        Figure {
            waist: 1050.0,
            shoulders: 1400.0,
            sections: heights
                .iter()
                .map(|&y| Section {
                    y,
                    middle: [0.0, 0.0],
                    half: [150.0, 100.0],
                })
                .collect(),
            crotch: 850.0,
            legs: Vec::new(),
            arms: vec![Arm {
                line: heights.iter().rev().map(|&y| [230.0, y, 0.0]).collect(),
                up: vec![[1.0, 0.0, 0.0]; heights.len()],
                side: vec![[0.0, 0.0, 1.0]; heights.len()],
                radius: vec![45.0; heights.len()],
            }],
            levels: Vec::new(),
        }
    }

    #[test]
    fn a_side_panel_under_the_arm_is_not_a_sleeve() {
        // Just off the trunk's side: nearer the arm's middle than the
        // trunk's, but nearer the trunk's surface.
        let panel: Vec<[f64; 3]> = (0..20)
            .map(|k| {
                [
                    165.0,
                    1000.0 + 15.0 * f64::from(k),
                    -30.0 + 3.0 * f64::from(k),
                ]
            })
            .collect();
        assert!(!round_an_arm(&panel, &figure()));
        // Round the arm itself: a sleeve.
        let sleeve: Vec<[f64; 3]> = (0..24)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 24.0;
                [230.0 + 50.0 * a.cos(), 1200.0, 50.0 * a.sin()]
            })
            .collect();
        assert!(round_an_arm(&sleeve, &figure()));
    }
}
