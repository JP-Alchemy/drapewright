//! Where facings and linings start: each laid flat on the piece it was
//! traced from (the one it is sewn to, or sewn to by way of another facing),
//! as its stitches say, and started where that piece is on the body. The
//! cloth then keeps them inside it. Pieces sewn onto the garment's face
//! (patch pockets) start so too, laid on the panel they are sewn onto, and
//! are kept outside it.

use drapewright_pattern_core::{Id, Piece};
use drapewright_seams::Seam;

use crate::garment::{Draped, Garment, assemble};

type P2 = [f64; 2];
type V3 = [f64; 3];

/// Points this far apart at least (mm) say how a piece lies on another.
const SPREAD: f64 = 5.0;
/// Fits this close (mm) to each other are as good as each other.
const ALIKE: f64 = 2.0;
/// A point this near another piece (mm), laid flat on it, is on it.
const ON: f64 = 3.0;

/// A move on the flat: turned, perhaps mirrored, then shifted.
#[derive(Clone, Copy, Debug)]
struct Flat {
    m: [[f64; 2]; 2],
    t: P2,
}

impl Flat {
    const NONE: Flat = Flat {
        m: [[1.0, 0.0], [0.0, 1.0]],
        t: [0.0, 0.0],
    };

    fn apply(&self, p: P2) -> P2 {
        [
            self.m[0][0] * p[0] + self.m[0][1] * p[1] + self.t[0],
            self.m[1][0] * p[0] + self.m[1][1] * p[1] + self.t[1],
        ]
    }

    /// This move, then `after`.
    fn then(&self, after: &Flat) -> Flat {
        let a = after.m;
        let s = self.m;
        Flat {
            m: [
                [
                    a[0][0] * s[0][0] + a[0][1] * s[1][0],
                    a[0][0] * s[0][1] + a[0][1] * s[1][1],
                ],
                [
                    a[1][0] * s[0][0] + a[1][1] * s[1][0],
                    a[1][0] * s[0][1] + a[1][1] * s[1][1],
                ],
            ],
            t: after.apply(self.t),
        }
    }
}

fn mean(points: &[P2]) -> P2 {
    let n = points.len().max(1) as f64;
    let s = points
        .iter()
        .fold([0.0, 0.0], |s, p| [s[0] + p[0], s[1] + p[1]]);
    [s[0] / n, s[1] / n]
}

/// The turn (mirrored first, if `mirrored`) and shift best taking `from`
/// onto `to`, and how far off it leaves them (root mean square, mm); none
/// if `from` is too bunched up to tell.
fn fit(from: &[P2], to: &[P2], mirrored: bool) -> Option<(Flat, f64)> {
    let flip = |p: P2| if mirrored { [p[0], -p[1]] } else { p };
    let from: Vec<P2> = from.iter().map(|&p| flip(p)).collect();
    let (ca, cb) = (mean(&from), mean(to));
    let spread = from
        .iter()
        .map(|p| (p[0] - ca[0]).hypot(p[1] - ca[1]))
        .fold(0.0, f64::max);
    if spread < SPREAD {
        return None;
    }
    let (mut cross, mut dot) = (0.0, 0.0);
    for (a, b) in from.iter().zip(to) {
        let (a, b) = ([a[0] - ca[0], a[1] - ca[1]], [b[0] - cb[0], b[1] - cb[1]]);
        cross += a[0] * b[1] - a[1] * b[0];
        dot += a[0] * b[0] + a[1] * b[1];
    }
    let (s, c) = cross.atan2(dot).sin_cos();
    let turn = Flat {
        m: [[c, -s], [s, c]],
        t: [0.0, 0.0],
    };
    let r = turn.apply(ca);
    let mut moved = Flat {
        t: [cb[0] - r[0], cb[1] - r[1]],
        ..turn
    };
    if mirrored {
        moved = Flat {
            m: [[1.0, 0.0], [0.0, -1.0]],
            t: [0.0, 0.0],
        }
        .then(&moved);
    }
    let unflipped: Vec<P2> = from.iter().map(|&p| flip(p)).collect();
    let off = unflipped
        .iter()
        .zip(to)
        .map(|(&a, b)| {
            let m = moved.apply(a);
            (m[0] - b[0]).powi(2) + (m[1] - b[1]).powi(2)
        })
        .sum::<f64>()
        / from.len() as f64;
    Some((moved, off.sqrt()))
}

/// How far `p` is from triangle `t`, and where in it the nearest point is
/// (barycentric, each share 0 to 1).
fn nearest_in(p: P2, t: [P2; 3]) -> (f64, [f64; 3]) {
    let [a, b, c] = t;
    let (v0, v1, v2) = (
        [b[0] - a[0], b[1] - a[1]],
        [c[0] - a[0], c[1] - a[1]],
        [p[0] - a[0], p[1] - a[1]],
    );
    let d00 = v0[0] * v0[0] + v0[1] * v0[1];
    let d01 = v0[0] * v1[0] + v0[1] * v1[1];
    let d11 = v1[0] * v1[0] + v1[1] * v1[1];
    let d20 = v2[0] * v0[0] + v2[1] * v0[1];
    let d21 = v2[0] * v1[0] + v2[1] * v1[1];
    let denom = d00 * d11 - d01 * d01;
    if denom.abs() < 1e-12 {
        return (f64::MAX, [1.0, 0.0, 0.0]);
    }
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;
    if u >= 0.0 && v >= 0.0 && w >= 0.0 {
        return (0.0, [u, v, w]);
    }
    // Outside: the nearest point on its edges.
    let on = |x: P2, y: P2| {
        let d = [y[0] - x[0], y[1] - x[1]];
        let l = d[0] * d[0] + d[1] * d[1];
        let s = if l > 0.0 {
            (((p[0] - x[0]) * d[0] + (p[1] - x[1]) * d[1]) / l).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = [x[0] + s * d[0], x[1] + s * d[1]];
        ((p[0] - q[0]).hypot(p[1] - q[1]), s)
    };
    let (dab, s_ab) = on(a, b);
    let (dbc, s_bc) = on(b, c);
    let (dca, s_ca) = on(c, a);
    if dab <= dbc && dab <= dca {
        (dab, [1.0 - s_ab, s_ab, 0.0])
    } else if dbc <= dca {
        (dbc, [0.0, 1.0 - s_bc, s_bc])
    } else {
        (dca, [s_ca, 0.0, 1.0 - s_ca])
    }
}

/// Where each panel of `g` after the first `starts.len()` (the garment seen
/// from outside, as placed) starts (mm): its facings and linings, and the
/// pieces sewn onto it, where the cloth they lie against is; and the panel
/// each lies on. None for one that can't be told how it lies: sewn to
/// nothing placed, or only at a point.
pub fn place_inside(g: &Garment, starts: &[Vec<V3>]) -> Vec<Option<(Vec<V3>, usize)>> {
    let n = g.panels.len();
    let placed = starts.len();
    // Where each pair of panels is stitched together, flat.
    let mut met: std::collections::HashMap<(usize, usize), Vec<(P2, P2)>> = Default::default();
    for &[(pa, ia), (pb, ib)] in &g.stitches[..g.stitches.len() - g.buttons] {
        if pa == pb {
            continue;
        }
        let (a, b) = (
            g.panels[pa].mesh.points[ia as usize],
            g.panels[pb].mesh.points[ib as usize],
        );
        met.entry((pa, pb)).or_default().push((a, b));
        met.entry((pb, pa)).or_default().push((b, a));
    }
    // Each panel laid on a placed one: which, and how.
    let mut on: Vec<Option<(usize, Flat)>> = (0..n)
        .map(|p| (p < placed).then_some((p, Flat::NONE)))
        .collect();
    // How much of panel `q`, moved by `flat`, lies on panel `base`: a
    // facing or lining lies on what it was traced from, not beside it.
    let covered = |q: usize, flat: &Flat, base: usize| {
        let mesh = &g.panels[base].mesh;
        let tris: Vec<[P2; 3]> = mesh
            .triangles
            .iter()
            .map(|t| t.map(|i| mesh.points[i as usize]))
            .collect();
        let points = &g.panels[q].mesh.points;
        let on = points
            .iter()
            .filter(|&&p| {
                let f = flat.apply(p);
                tris.iter().any(|&t| nearest_in(f, t).0 < ON)
            })
            .count();
        on as f64 / points.len().max(1) as f64
    };
    loop {
        let mut more = false;
        for q in placed..n {
            if on[q].is_some() {
                continue;
            }
            // Laid by each laid panel it is sewn to, turned or mirrored:
            // the way that lays it most on the cloth it ends up over, and
            // of those the closest fit.
            let mut best: Option<(f64, f64, usize, Flat)> = None;
            for (p, laid) in on.iter().enumerate() {
                let (Some((base, onto)), Some(pairs)) = (*laid, met.get(&(q, p))) else {
                    continue;
                };
                let (from, to): (Vec<P2>, Vec<P2>) = pairs.iter().copied().unzip();
                for mirrored in [false, true] {
                    let Some((flat, off)) = fit(&from, &to, mirrored) else {
                        continue;
                    };
                    let flat = flat.then(&onto);
                    let cover = covered(q, &flat, base);
                    let better = best.is_none_or(|(c, o, ..)| {
                        cover > c + 0.05 || (cover > c - 0.05 && off + ALIKE < o)
                    });
                    if better {
                        best = Some((cover, off, base, flat));
                    }
                }
            }
            if let Some((_, _, base, flat)) = best {
                on[q] = Some((base, flat));
                more = true;
            }
        }
        if !more {
            break;
        }
    }
    on.iter()
        .enumerate()
        .skip(placed)
        .map(|(q, slot)| {
            let (base, flat) = (*slot)?;
            let mesh = &g.panels[base].mesh;
            let tris: Vec<[P2; 3]> = mesh
                .triangles
                .iter()
                .map(|t| t.map(|i| mesh.points[i as usize]))
                .collect();
            let at: Vec<V3> = g.panels[q]
                .mesh
                .points
                .iter()
                .map(|&p| {
                    let f = flat.apply(p);
                    let (k, (_, share)) = tris
                        .iter()
                        .map(|&t| nearest_in(f, t))
                        .enumerate()
                        .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
                        .expect("a mesh has triangles");
                    let corners = mesh.triangles[k].map(|i| starts[base][i as usize]);
                    [0, 1, 2].map(|c| {
                        share[0] * corners[0][c]
                            + share[1] * corners[1][c]
                            + share[2] * corners[2][c]
                    })
                })
                .collect();
            Some((at, base))
        })
        .collect()
}

/// A garment with what goes inside it and what is sewn onto it, placed.
#[derive(Clone, Debug)]
pub struct Dressed {
    /// Its outside's panels first, then its facings and linings, then the
    /// pieces sewn onto it.
    pub garment: Garment,
    /// Where each panel starts, mm.
    pub starts: Vec<Vec<V3>>,
    /// Its facings and linings are in it: else (none placed) left out.
    pub lined: bool,
    /// The pieces sewn onto it that couldn't be laid on the panel they are
    /// sewn onto (a pocket whose placement runs over the panel's edge, say):
    /// left out.
    pub unplaced: Vec<Id>,
}

/// The garment `shell` (`draped.sewn`, and the pieces sewn onto them, as
/// `shell` makes it; placed at `starts`) with its facings and linings
/// inside it and the pieces sewn onto it over it, all sewn by `seams`.
/// Facings and linings go in all together or not at all; each piece sewn
/// on that can't be placed is left out alone.
pub fn dressed(
    shell: &Garment,
    starts: &[Vec<V3>],
    draped: &Draped,
    seams: &[Seam],
    spacing: f64,
) -> Dressed {
    let ids = |pieces: &[Piece]| pieces.iter().map(|p| p.id.clone()).collect::<Vec<_>>();
    let plain = Dressed {
        garment: shell.clone(),
        starts: starts.to_vec(),
        lined: draped.lined.is_empty(),
        unplaced: ids(&draped.on),
    };
    // With the facings and linings if there are any, else (or if they
    // can't be placed) without.
    let tries: &[bool] = match (draped.lined.is_empty(), draped.on.is_empty()) {
        (true, true) => &[],
        (true, false) => &[false],
        (false, true) => &[true],
        (false, false) => &[true, false],
    };
    for &with_lining in tries {
        let lined: &[Piece] = if with_lining { &draped.lined } else { &[] };
        let all: Vec<Piece> = draped
            .sewn
            .iter()
            .chain(lined)
            .chain(&draped.on)
            .cloned()
            .collect();
        let Ok(whole) = assemble(&all, seams, spacing) else {
            continue;
        };
        let same = whole.panels.len() >= shell.panels.len()
            && whole.panels.iter().zip(&shell.panels).all(|(a, b)| {
                a.piece == b.piece && a.copy == b.copy && a.mesh.points.len() == b.mesh.points.len()
            });
        if !same {
            continue;
        }
        let placed = place_inside(&whole, starts);
        let n = shell.panels.len();
        let on: Vec<bool> = whole
            .panels
            .iter()
            .map(|panel| draped.on.iter().any(|x| x.id == panel.piece))
            .collect();
        // Facings and linings all placed, or none.
        if (n..whole.panels.len()).any(|p| !on[p] && placed[p - n].is_none()) {
            continue;
        }
        let mut whole = whole;
        let mut at: Vec<Option<Vec<V3>>> = starts.iter().cloned().map(Some).collect();
        let mut unplaced: Vec<Id> = Vec::new();
        for (k, slot) in placed.into_iter().enumerate() {
            let p = n + k;
            match slot {
                Some((points, base)) => {
                    whole.panels[p].over = Some(base);
                    whole.panels[p].sewn_on = on[p];
                    at.push(Some(points));
                }
                None => {
                    at.push(None);
                    if !unplaced.contains(&whole.panels[p].piece) {
                        unplaced.push(whole.panels[p].piece.clone());
                    }
                }
            }
        }
        // A piece is left out whole, not one of its copies.
        let keep: Vec<bool> = whole
            .panels
            .iter()
            .zip(&at)
            .map(|(panel, a)| a.is_some() && !unplaced.contains(&panel.piece))
            .collect();
        let starts: Vec<Vec<V3>> = at
            .into_iter()
            .zip(&keep)
            .filter_map(|(a, &k)| a.filter(|_| k))
            .collect();
        return Dressed {
            garment: whole.keeping(&keep),
            starts,
            lined: with_lining || draped.lined.is_empty(),
            unplaced,
        };
    }
    plain
}

/// The garment `shell` (made of `draped.sewn`, placed at `starts`) with its
/// facings and linings inside it and the pieces sewn onto it over it, as
/// `dressed` makes it; none if nothing more could be placed.
pub fn with_inside(
    shell: &Garment,
    starts: &[Vec<V3>],
    draped: &Draped,
    seams: &[Seam],
    spacing: f64,
) -> Option<(Garment, Vec<Vec<V3>>)> {
    let d = dressed(shell, starts, draped, seams, spacing);
    (d.garment.panels.len() > shell.panels.len()).then_some((d.garment, d.starts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fit_finds_the_turn_and_mirror_that_lays_points_on_others() {
        let from = [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]];
        // Turned a quarter, shifted.
        let to: Vec<P2> = from.iter().map(|p| [3.0 - p[1], 4.0 + p[0]]).collect();
        let (f, off) = fit(&from, &to, false).unwrap();
        assert!(off < 1e-9, "{off}");
        let p = f.apply([2.0, 1.0]);
        assert!(
            (p[0] - 2.0).abs() < 1e-9 && (p[1] - 6.0).abs() < 1e-9,
            "{p:?}"
        );
        // Mirrored: only the mirrored fit lays them exactly.
        let mirror: Vec<P2> = from.iter().map(|p| [p[0], -p[1]]).collect();
        assert!(fit(&from, &mirror, false).unwrap().1 > 1.0);
        let (m, off) = fit(&from, &mirror, true).unwrap();
        assert!(off < 1e-9);
        let p = m.apply([2.0, 1.0]);
        assert!(
            (p[0] - 2.0).abs() < 1e-9 && (p[1] + 1.0).abs() < 1e-9,
            "{p:?}"
        );
        assert!(fit(&[[0.0, 0.0], [1.0, 1.0]], &[[0.0, 0.0], [1.0, 1.0]], false).is_none());
    }

    #[test]
    fn a_point_is_found_in_or_nearest_a_triangle() {
        let t = [[0.0, 0.0], [10.0, 0.0], [0.0, 10.0]];
        let (d, s) = nearest_in([2.0, 3.0], t);
        assert!(d == 0.0 && (s[0] - 0.5).abs() < 1e-9 && (s[2] - 0.3).abs() < 1e-9);
        let (d, s) = nearest_in([5.0, -2.0], t);
        assert!(
            (d - 2.0).abs() < 1e-9 && (s[1] - 0.5).abs() < 1e-9,
            "{d} {s:?}"
        );
    }
}
