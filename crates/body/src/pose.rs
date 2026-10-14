//! Poses: the figure moved from the way it stands as measured (arms held
//! out and down, legs straight) into another, to see how a garment moves
//! with it. Each limb bone (an upper arm; a forearm with its hand; a thigh;
//! a shin with its foot) turns about its joint, carrying the bones below
//! it; the trunk stays where it is. How much each bone moves each point of
//! the skin comes from the body's regions, spread over the skin so that it
//! bends smoothly round the joints.

use crate::asset::Region;
use crate::shape::{Body, BodyModel};

/// A pose the figure can be put in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Pose {
    /// As measured: arms out and down, legs straight.
    Standing,
    /// Arms hanging by the sides.
    ArmsDown,
    /// Both arms reaching forward at shoulder height.
    Reaching,
    /// Both arms raised overhead.
    ArmsUp,
    /// Mid-stride: the left leg forward, the right behind.
    Stepping,
    /// Sitting: thighs forward, shins straight down.
    Sitting,
}

impl Pose {
    pub const ALL: [Pose; 6] = [
        Pose::Standing,
        Pose::ArmsDown,
        Pose::Reaching,
        Pose::ArmsUp,
        Pose::Stepping,
        Pose::Sitting,
    ];
}

/// The trunk (with the head), then for the left side and the right in turn
/// an upper arm, a forearm with its hand, a thigh, and a shin with its foot.
const BONES: usize = 9;
const TRUNK: usize = 0;
const fn upper_arm(side: usize) -> usize {
    1 + 4 * side
}
const fn forearm(side: usize) -> usize {
    2 + 4 * side
}
const fn thigh(side: usize) -> usize {
    3 + 4 * side
}
const fn shin(side: usize) -> usize {
    4 + 4 * side
}
/// The left side is towards +x.
const SIDES: [(usize, f64); 2] = [(0, 1.0), (1, -1.0)];

/// Rounds of averaging each point's weights with its neighbours': how far
/// a bend spreads either side of where one region meets the next.
const SPREAD: usize = 24;
/// How far an arm hanging down keeps from the body, mm, leaving room for
/// cloth between: the upper arm, and the forearm and hand (which pass the
/// hips).
const ARM_CLEAR: f64 = 20.0;
const FOREARM_CLEAR: f64 = 30.0;

type V = [f64; 3];

fn sub(a: V, b: V) -> V {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V, b: V) -> V {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn dot(a: V, b: V) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V, b: V) -> V {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: V) -> V {
    let l = dot(a, a).sqrt();
    if l > 1e-12 { a.map(|c| c / l) } else { a }
}

/// A rotation, as a matrix by rows.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rot([V; 3]);

impl Rot {
    const NONE: Rot = Rot([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);

    /// Turning `angle` radians about `axis` (a unit vector), right-handed.
    fn about(axis: V, angle: f64) -> Rot {
        let (s, c) = angle.sin_cos();
        let [x, y, z] = axis;
        let t = 1.0 - c;
        Rot([
            [t * x * x + c, t * x * y - s * z, t * x * z + s * y],
            [t * x * y + s * z, t * y * y + c, t * y * z - s * x],
            [t * x * z - s * y, t * y * z + s * x, t * z * z + c],
        ])
    }

    /// The least turn taking direction `from` to `to`, `share` of the way.
    fn swing(from: V, to: V, share: f64) -> Rot {
        let (a, b) = (unit(from), unit(to));
        let axis = cross(a, b);
        let s = dot(axis, axis).sqrt();
        if s < 1e-9 {
            return Rot::NONE;
        }
        let angle = s.atan2(dot(a, b));
        Rot::about(axis.map(|c| c / s), angle * share)
    }

    fn apply(&self, p: V) -> V {
        [dot(self.0[0], p), dot(self.0[1], p), dot(self.0[2], p)]
    }

    fn then(&self, after: &Rot) -> Rot {
        // `after` applied to each column of `self`.
        let cols = [0, 1, 2].map(|k| after.apply([self.0[0][k], self.0[1][k], self.0[2][k]]));
        Rot([0, 1, 2].map(|r| [cols[0][r], cols[1][r], cols[2][r]]))
    }

    fn inverse(&self) -> Rot {
        let m = self.0;
        Rot([0, 1, 2].map(|r| [m[0][r], m[1][r], m[2][r]]))
    }

    /// As a unit quaternion (w, x, y, z) (Shepperd's method).
    fn quaternion(&self) -> [f64; 4] {
        let m = self.0;
        let trace = m[0][0] + m[1][1] + m[2][2];
        if trace > 0.0 {
            let s = (trace + 1.0).sqrt() * 2.0;
            [
                s / 4.0,
                (m[2][1] - m[1][2]) / s,
                (m[0][2] - m[2][0]) / s,
                (m[1][0] - m[0][1]) / s,
            ]
        } else if m[0][0] > m[1][1] && m[0][0] > m[2][2] {
            let s = (1.0 + m[0][0] - m[1][1] - m[2][2]).sqrt() * 2.0;
            [
                (m[2][1] - m[1][2]) / s,
                s / 4.0,
                (m[0][1] + m[1][0]) / s,
                (m[0][2] + m[2][0]) / s,
            ]
        } else if m[1][1] > m[2][2] {
            let s = (1.0 + m[1][1] - m[0][0] - m[2][2]).sqrt() * 2.0;
            [
                (m[0][2] - m[2][0]) / s,
                (m[0][1] + m[1][0]) / s,
                s / 4.0,
                (m[1][2] + m[2][1]) / s,
            ]
        } else {
            let s = (1.0 + m[2][2] - m[0][0] - m[1][1]).sqrt() * 2.0;
            [
                (m[1][0] - m[0][1]) / s,
                (m[0][2] + m[2][0]) / s,
                (m[1][2] + m[2][1]) / s,
                s / 4.0,
            ]
        }
    }

    fn from_quaternion([w, x, y, z]: [f64; 4]) -> Rot {
        Rot([
            [
                1.0 - 2.0 * (y * y + z * z),
                2.0 * (x * y - w * z),
                2.0 * (x * z + w * y),
            ],
            [
                2.0 * (x * y + w * z),
                1.0 - 2.0 * (x * x + z * z),
                2.0 * (y * z - w * x),
            ],
            [
                2.0 * (x * z - w * y),
                2.0 * (y * z + w * x),
                1.0 - 2.0 * (x * x + y * y),
            ],
        ])
    }

    /// `share` of the way from this turn to `to`, the shorter way round.
    fn towards(&self, to: &Rot, share: f64) -> Rot {
        let a = self.quaternion();
        let mut b = to.quaternion();
        let mut cos: f64 = (0..4).map(|k| a[k] * b[k]).sum();
        if cos < 0.0 {
            b = b.map(|c| -c);
            cos = -cos;
        }
        let (ka, kb) = if cos > 0.9995 {
            (1.0 - share, share)
        } else {
            let angle = cos.acos();
            let s = angle.sin();
            (((1.0 - share) * angle).sin() / s, (share * angle).sin() / s)
        };
        let q = [0, 1, 2, 3].map(|k| ka * a[k] + kb * b[k]);
        let l = q.iter().map(|c| c * c).sum::<f64>().sqrt();
        Rot::from_quaternion(q.map(|c| c / l))
    }
}

/// A bone's move: `p` to `rot·p + shift`.
#[derive(Clone, Copy, Debug)]
struct Move {
    rot: Rot,
    shift: V,
}

impl Move {
    /// Turning by `rot` about `pivot`.
    fn about(rot: Rot, pivot: V) -> Move {
        Move {
            rot,
            shift: sub(pivot, rot.apply(pivot)),
        }
    }

    fn apply(&self, p: V) -> V {
        add(self.rot.apply(p), self.shift)
    }
}

/// How each bone is turned from standing as measured, as the world sees
/// it (a forearm's turn includes its upper arm's); and how far the whole
/// figure is lowered (mm), to stay on the floor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stance {
    turns: [Rot; BONES],
    lowered: f64,
}

impl Stance {
    /// Standing as measured.
    pub const STANDING: Stance = Stance {
        turns: [Rot::NONE; BONES],
        lowered: 0.0,
    };

    /// How far the whole figure is lowered, mm.
    pub fn lowered(&self) -> f64 {
        self.lowered
    }

    /// `share` of the way from this stance to `to` (0 to 1), each bone
    /// turning steadily the shorter way round.
    pub fn towards(&self, to: &Stance, share: f64) -> Stance {
        Stance {
            turns: std::array::from_fn(|b| self.turns[b].towards(&to.turns[b], share)),
            lowered: self.lowered + (to.lowered - self.lowered) * share,
        }
    }
}

/// A limb's three joints: where it meets the trunk, its middle, its end.
#[derive(Clone, Copy, Debug)]
struct Limb {
    top: V,
    middle: V,
    end: V,
}

impl Limb {
    /// Whether `p` is beyond the middle joint, across the plane halfway
    /// between the two bones' directions.
    fn below(&self, p: V) -> bool {
        let n = add(
            unit(sub(self.middle, self.top)),
            unit(sub(self.end, self.middle)),
        );
        dot(sub(p, self.middle), n) > 0.0
    }
}

/// A shaped figure ready to pose: how much each bone moves each point, and
/// where its joints are.
#[derive(Clone, Debug)]
pub struct Rig {
    /// Each point's share in each bone, summing to 1.
    weights: Vec<[f32; BONES]>,
    /// Points `0..shown` are the skin; the rest, joint markers.
    shown: usize,
    /// Arms and legs, left then right.
    arms: [Limb; 2],
    legs: [Limb; 2],
    /// For hanging the arms down: the body's widest but for its arms at
    /// each height (by `BAND` mm, from the floor), each side.
    trunk_width: [Vec<f64>; 2],
}

/// Heights by which the trunk's width is kept, mm.
const BAND: f64 = 20.0;

/// A seat, mm: half as wide, reaching behind the figure and short of its
/// knees by these; and this far under it, room for cloth between.
const SEAT_HALF: f64 = 250.0;
const SEAT_BEHIND: f64 = 60.0;
const SEAT_SHORT: f64 = 80.0;
const SEAT_GAP: f64 = 8.0;

impl Rig {
    /// The rig of `body`, shaped from `model`; none if its joints are
    /// missing.
    pub fn new(model: &BodyModel, body: &Body) -> Option<Rig> {
        let joint = |name: &str| model.joint(body, name);
        let limb = |a: &str, b: &str, c: &str| -> Option<Limb> {
            Some(Limb {
                top: joint(a)?,
                middle: joint(b)?,
                end: joint(c)?,
            })
        };
        let arms = [
            limb("l-shoulder", "l-elbow", "l-hand")?,
            limb("r-shoulder", "r-elbow", "r-hand")?,
        ];
        let legs = [
            limb("l-upper-leg", "l-knee", "l-ankle")?,
            limb("r-upper-leg", "r-knee", "r-ankle")?,
        ];
        let n = body.vertices.len();
        let shown = model.asset.shown as usize;
        let mut weights = vec![[0f32; BONES]; n];
        for (v, w) in weights.iter_mut().enumerate().take(shown) {
            let p = body.vertices[v];
            let arm = |side: usize| {
                if arms[side].below(p) {
                    forearm(side)
                } else {
                    upper_arm(side)
                }
            };
            let leg = |side: usize| {
                if legs[side].below(p) {
                    shin(side)
                } else {
                    thigh(side)
                }
            };
            match model.region(v) {
                Some(Region::ShoulderLeft) => {
                    w[TRUNK] = 0.5;
                    w[upper_arm(0)] = 0.5;
                }
                Some(Region::ShoulderRight) => {
                    w[TRUNK] = 0.5;
                    w[upper_arm(1)] = 0.5;
                }
                Some(Region::ArmLeft) => w[arm(0)] = 1.0,
                Some(Region::ArmRight) => w[arm(1)] = 1.0,
                Some(Region::HandLeft) => w[forearm(0)] = 1.0,
                Some(Region::HandRight) => w[forearm(1)] = 1.0,
                Some(Region::LegLeft) => w[leg(0)] = 1.0,
                Some(Region::LegRight) => w[leg(1)] = 1.0,
                Some(Region::FootLeft) => w[shin(0)] = 1.0,
                Some(Region::FootRight) => w[shin(1)] = 1.0,
                _ => w[TRUNK] = 1.0,
            }
        }
        spread(&mut weights[..shown], &model.skin());
        // The joint markers go with the bone they end (an elbow with its
        // upper arm), the rest with the trunk.
        for j in &model.asset.joints {
            let bone = match j.name.as_str() {
                "l-elbow" => upper_arm(0),
                "r-elbow" => upper_arm(1),
                "l-hand" => forearm(0),
                "r-hand" => forearm(1),
                "l-knee" => thigh(0),
                "r-knee" => thigh(1),
                "l-ankle" => shin(0),
                "r-ankle" => shin(1),
                _ => TRUNK,
            };
            for w in &mut weights[j.first as usize..(j.first as usize + 8).min(n)] {
                *w = [0.0; BONES];
                w[bone] = 1.0;
            }
        }
        let mut trunk_width: [Vec<f64>; 2] = [Vec::new(), Vec::new()];
        for (v, w) in weights.iter().enumerate().take(shown) {
            let arms: f32 = (0..2).map(|s| w[upper_arm(s)] + w[forearm(s)]).sum();
            if arms > 0.01 {
                continue;
            }
            let p = body.vertices[v];
            let side = usize::from(p[0] < 0.0);
            let band = (p[1].max(0.0) / BAND) as usize;
            let widths = &mut trunk_width[side];
            if widths.len() <= band {
                widths.resize(band + 1, 0.0);
            }
            widths[band] = widths[band].max(p[0].abs());
        }
        Some(Rig {
            weights,
            shown,
            arms,
            legs,
            trunk_width,
        })
    }

    /// The figure `body` (this rig's) in `pose`, `share` of the way there
    /// from standing as measured (0 to 1).
    pub fn pose(&self, body: &Body, pose: Pose, share: f64) -> Body {
        let to = self.stance(body, pose);
        self.place(body, &Stance::STANDING.towards(&to, share))
    }

    /// The figure `body` (this rig's) in `stance`.
    pub fn place(&self, body: &Body, stance: &Stance) -> Body {
        let moves = self.moves(stance);
        Body {
            vertices: body
                .vertices
                .iter()
                .zip(&self.weights)
                .map(|(&p, w)| blend(&moves, w, p))
                .collect(),
        }
    }

    /// Each bone's move in `stance`: the upper bones turning about their
    /// joints with the trunk, the lower about theirs, carried along.
    fn moves(&self, stance: &Stance) -> [Move; BONES] {
        let down = [0.0, -stance.lowered, 0.0];
        let mut moves = [Move {
            rot: Rot::NONE,
            shift: down,
        }; BONES];
        for (side, _) in SIDES {
            for (limb, upper, lower) in [
                (&self.arms[side], upper_arm(side), forearm(side)),
                (&self.legs[side], thigh(side), shin(side)),
            ] {
                let mut u = Move::about(stance.turns[upper], limb.top);
                u.shift = add(u.shift, down);
                let rot = stance.turns[lower];
                moves[upper] = u;
                moves[lower] = Move {
                    rot,
                    shift: sub(u.apply(limb.middle), rot.apply(limb.middle)),
                };
            }
        }
        moves
    }

    /// A seat for the figure `body` sitting as `stance`: a box under its
    /// seat and thighs, its front edge short of the knees, as its lowest and
    /// highest corners (mm).
    pub fn seat(&self, body: &Body, stance: &Stance) -> (V, V) {
        let moves = self.moves(stance);
        let knees = [0, 1].map(|s| moves[thigh(s)].apply(self.legs[s].middle));
        let front = knees[0][2].min(knees[1][2]) - SEAT_SHORT;
        let posed = self.place(body, stance);
        // The trunk and thighs over the seat.
        let over: Vec<V> = posed.vertices[..self.shown]
            .iter()
            .zip(&self.weights)
            .filter(|(p, w)| {
                w[TRUNK] + w[thigh(0)] + w[thigh(1)] > 0.9 && p[2] < front && p[0].abs() < SEAT_HALF
            })
            .map(|(&p, _)| p)
            .collect();
        let under = over.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
        let back = over.iter().map(|p| p[2]).fold(f64::MAX, f64::min);
        // Down to the floor.
        (
            [-SEAT_HALF, 0.0, back - SEAT_BEHIND],
            [SEAT_HALF, under - SEAT_GAP, front],
        )
    }

    /// How each bone is turned in `pose`.
    pub fn stance(&self, body: &Body, pose: Pose) -> Stance {
        let mut turns = [Rot::NONE; BONES];
        for (side, sign) in SIDES {
            let mirror = |v: V| [v[0] * sign, v[1], v[2]];
            let arm = &self.arms[side];
            let aims = match pose {
                Pose::ArmsDown => {
                    let tilt = self.hanging(body, side);
                    Some((
                        mirror([tilt.sin(), -tilt.cos(), 0.0]),
                        mirror([tilt.sin(), -tilt.cos(), 0.25]),
                    ))
                }
                Pose::Reaching => Some((mirror([0.12, -0.05, 1.0]), mirror([0.02, 0.0, 1.0]))),
                Pose::ArmsUp => Some((mirror([0.3, 1.0, 0.2]), mirror([0.15, 1.0, 0.1]))),
                _ => None,
            };
            if let Some((upper, fore)) = aims {
                let [u, f] = limb_turns(arm, upper, fore);
                turns[upper_arm(side)] = u;
                turns[forearm(side)] = f;
            }
            // Legs turn forward (towards +z) about the x axis by the angles
            // given, degrees: the thigh's, and the knee's bend back.
            let bends = match pose {
                Pose::Stepping if side == 0 => Some((25.0, 8.0)),
                Pose::Stepping => Some((-12.0, 30.0)),
                Pose::Sitting => Some((88.0, 88.0)),
                _ => None,
            };
            if let Some((hip, knee)) = bends {
                let leg = &self.legs[side];
                let forward =
                    |angle: f64, v: V| Rot::about([1.0, 0.0, 0.0], -angle.to_radians()).apply(v);
                let thigh_aim = forward(hip, sub(leg.middle, leg.top));
                let shin_aim = forward(hip - knee, sub(leg.end, leg.middle));
                let [t, s] = limb_turns(leg, thigh_aim, shin_aim);
                turns[thigh(side)] = t;
                turns[shin(side)] = s;
            }
        }
        // On the floor still, however the legs are bent.
        let turned = Stance {
            turns,
            lowered: 0.0,
        };
        let lowest = |b: &Body| {
            b.vertices[..self.shown]
                .iter()
                .map(|p| p[1])
                .fold(f64::MAX, f64::min)
        };
        Stance {
            turns,
            lowered: lowest(&self.place(body, &turned)) - lowest(body),
        }
    }

    /// How far from straight down an arm hangs (radians): the least that
    /// keeps it clear of the trunk.
    fn hanging(&self, body: &Body, side: usize) -> f64 {
        let sign = if side == 0 { 1.0 } else { -1.0 };
        let widths = &self.trunk_width[side];
        let (upper, fore) = (upper_arm(side), forearm(side));
        let arm = &self.arms[side];
        for degrees in (6..=40).step_by(2) {
            let tilt = f64::from(degrees).to_radians();
            let mut stance = Stance::STANDING;
            [stance.turns[upper], stance.turns[fore]] = limb_turns(
                arm,
                [sign * tilt.sin(), -tilt.cos(), 0.0],
                [sign * tilt.sin(), -tilt.cos(), 0.25],
            );
            let moves = self.moves(&stance);
            let (u, f) = (moves[upper], moves[fore]);
            let clear = body.vertices.iter().zip(&self.weights).all(|(&p, w)| {
                let (moved, clearance) = if w[upper] > 0.95 {
                    (u.apply(p), ARM_CLEAR)
                } else if w[fore] > 0.95 {
                    (f.apply(p), FOREARM_CLEAR)
                } else {
                    return true;
                };
                let band = (moved[1].max(0.0) / BAND) as usize;
                let width = widths.get(band).copied().unwrap_or(0.0);
                width == 0.0 || moved[0] * sign > width + clearance
            });
            if clear {
                return tilt;
            }
        }
        40f64.to_radians()
    }
}

/// The turns of a limb's two bones (as the world sees them) aiming them
/// along `top` and `bottom` (directions).
fn limb_turns(limb: &Limb, top: V, bottom: V) -> [Rot; 2] {
    let upper = Rot::swing(sub(limb.middle, limb.top), top, 1.0);
    // The lower bone's aim as the upper bone, turned, sees it.
    let lower = Rot::swing(
        sub(limb.end, limb.middle),
        upper.inverse().apply(bottom),
        1.0,
    );
    [upper, lower.then(&upper)]
}

/// A point moved by each bone in its share.
fn blend(moves: &[Move; BONES], w: &[f32; BONES], p: V) -> V {
    let mut out = [0.0; 3];
    for (m, &share) in moves.iter().zip(w) {
        if share == 0.0 {
            continue;
        }
        let q = m.apply(p);
        for k in 0..3 {
            out[k] += f64::from(share) * q[k];
        }
    }
    out
}

/// Each point's weights averaged with its neighbours' over the skin,
/// `SPREAD` times.
fn spread(weights: &mut [[f32; BONES]], triangles: &[[u32; 3]]) {
    let n = weights.len();
    let mut neighbours: Vec<Vec<u32>> = vec![Vec::new(); n];
    for t in triangles {
        for (a, b) in [(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            if (a as usize) < n && (b as usize) < n {
                neighbours[a as usize].push(b);
                neighbours[b as usize].push(a);
            }
        }
    }
    for list in &mut neighbours {
        list.sort_unstable();
        list.dedup();
    }
    let mut next = weights.to_vec();
    for _ in 0..SPREAD {
        for (v, out) in next.iter_mut().enumerate() {
            let mut sum = weights[v];
            for &u in &neighbours[v] {
                for (s, w) in sum.iter_mut().zip(&weights[u as usize]) {
                    *s += w;
                }
            }
            let count = (1 + neighbours[v].len()) as f32;
            *out = sum.map(|s| s / count);
        }
        weights.copy_from_slice(&next);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swing_turns_one_direction_onto_another() {
        let r = Rot::swing([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 1.0);
        let p = r.apply([1.0, 0.0, 0.0]);
        assert!((p[1] - 1.0).abs() < 1e-12 && p[0].abs() < 1e-12);
        let half = Rot::swing([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], 0.5).apply([1.0, 0.0, 0.0]);
        assert!((half[0] - half[1]).abs() < 1e-12);
        let back = r.then(&r.inverse()).apply([0.3, 0.2, 0.1]);
        assert!(sub(back, [0.3, 0.2, 0.1]).iter().all(|c| c.abs() < 1e-12));
    }

    #[test]
    fn a_bone_below_another_turns_with_it() {
        let limb = Limb {
            top: [0.0, 0.0, 0.0],
            middle: [0.0, -1.0, 0.0],
            end: [0.0, -2.0, 0.0],
        };
        // The upper bone forward, the lower one up from there.
        let [u, l] = limb_turns(&limb, [0.0, 0.0, 1.0], [0.0, 1.0, 0.0]);
        let u = Move::about(u, limb.top);
        let middle = u.apply(limb.middle);
        assert!(sub(middle, [0.0, 0.0, 1.0]).iter().all(|c| c.abs() < 1e-12));
        let end = add(l.apply(sub(limb.end, limb.middle)), middle);
        assert!(
            sub(end, [0.0, 1.0, 1.0]).iter().all(|c| c.abs() < 1e-12),
            "{end:?}"
        );
    }

    #[test]
    fn turns_are_blended_the_shorter_way_round() {
        let quarter = Rot::about([0.0, 0.0, 1.0], std::f64::consts::FRAC_PI_2);
        let back = Rot::from_quaternion(quarter.quaternion());
        for r in 0..3 {
            assert!(sub(back.0[r], quarter.0[r]).iter().all(|c| c.abs() < 1e-12));
        }
        let half = Rot::NONE.towards(&quarter, 0.5).apply([1.0, 0.0, 0.0]);
        let c = std::f64::consts::FRAC_1_SQRT_2;
        assert!(
            sub(half, [c, c, 0.0]).iter().all(|v| v.abs() < 1e-12),
            "{half:?}"
        );
        let most = Rot::about([1.0, 0.0, 0.0], 3.0);
        let mid = Rot::NONE.towards(&most, 0.5).apply([0.0, 1.0, 0.0]);
        assert!((mid[1] - 1.5f64.cos()).abs() < 1e-9, "{mid:?}");
    }
}
