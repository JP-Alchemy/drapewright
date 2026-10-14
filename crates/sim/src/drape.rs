//! A drape, from flat panels to cloth settled on the body: sewn, the
//! stitches tightening as the seams close round the body while the cloth
//! hangs, then left to settle until it is still. Settled, the body can move
//! into a pose, carrying the cloth with it.

use drapewright_body::{Body, BodyModel, Pose, Region, Rig, Stance};

use crate::cloth::{Cloth, ClothBuilder, V3};
use crate::collide::{Between, BodyField, Seat, Seated, Surface};
use crate::fabric::Fabric;
use crate::garment::Garment;

/// Seconds spent sewing, in three stages, and how loosely the stitches hold
/// in each (m/N), tightening as the seams close. The cloth hangs all the
/// while, so that it settles where it would be worn.
const SEWING: [(f32, f32); 3] = [(0.4, 1e-3), (0.4, 1e-5), (0.4, 1e-8)];
/// Steps a frame: small enough for stiff cloth.
const SUBSTEPS: u32 = 30;
/// Settled: the cloth's mean speed below this, m/s.
const STILL: f32 = 0.004;
/// No point moves faster than this, m/s: the placing leaves the cloth
/// strained where it is stretched round the body, and let go all at once
/// that strain would fling it.
const MAX_SPEED: f32 = 1.0;

/// The body's field: cells this size, m, reaching this far beyond it.
const CELL: f32 = 0.008;
const MARGIN: f32 = 0.06;
/// While the body moves: keyframes no further apart than this (m) where
/// it is under the cloth, so that the body felt between two moves smoothly
/// from one to the next.
const KEY_STEP: f32 = 0.015;
/// The fields felt while the body moves: cells this size (m), exact this
/// far from the body, reaching this far round the cloth.
const MOVING_CELL: f32 = 0.012;
const MOVING_BAND: f32 = 0.024;
const MOVING_MARGIN: f32 = 0.06;
/// How fast the body under the cloth moves at most, m/s: the cloth keeps up.
const BODY_SPEED: f32 = 0.8;
/// The body this near the cloth (m) carries it as it moves.
const NEAR_CLOTH: f32 = 0.05;
/// A body that has moved is still for this long (s) before the cloth on it
/// can be settled.
const STILL_AFTER: f32 = 0.5;

/// A seat starts this far below where it ends up (m) as the figure sits,
/// rising under the thighs as they come up; and sinks as far as it stands.
const SEAT_RISE: f32 = 0.35;

/// Moves of any drape's body, counted: each shown body told apart.
static MOVES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);

/// The figure a drape is on, ready to pose.
#[derive(Debug)]
pub struct Posable {
    pub rig: Rig,
    /// The figure standing as measured, mm.
    pub rest: Body,
    /// The skin felt while the body moves, simpler where the cloth seldom
    /// is (the hands and feet).
    pub moving: Surface,
    /// The whole skin, felt once the body is still.
    pub skin: Vec<[u32; 3]>,
}

impl Posable {
    /// The figure `body` shaped from `model`, ready to pose; none if it
    /// has no joints to pose it by.
    pub fn of(model: &BodyModel, body: &Body) -> Option<Posable> {
        let skin = model.skin();
        let moving = Surface::without(&skin, body.vertices.len(), |t| {
            t.iter().all(|&v| {
                matches!(
                    model.region(v as usize),
                    Some(
                        Region::HandLeft | Region::HandRight | Region::FootLeft | Region::FootRight
                    )
                )
            })
        });
        Some(Posable {
            rig: Rig::new(model, body)?,
            rest: body.clone(),
            moving,
            skin,
        })
    }

    /// The figure in `stance`, m.
    fn points(&self, stance: &Stance) -> Vec<V3> {
        self.rig
            .place(&self.rest, stance)
            .vertices
            .iter()
            .map(|p| p.map(|c| (c / 1000.0) as f32))
            .collect()
    }
}

/// The body moving from one stance to another.
#[derive(Debug)]
struct Motion {
    from: Stance,
    to: Stance,
    /// Keyframes along the way, evenly by share.
    keys: usize,
    /// When it set off, and how long it takes, s.
    start: f32,
    seconds: f32,
    /// The fields at a keyframe and the next, by the first's number.
    fields: Option<(usize, BodyField, BodyField)>,
    /// The seat sat on before, and after.
    seats: (Option<Seat>, Option<Seat>),
}

impl Motion {
    /// How far along it is at `time`: easing in and out.
    fn share(&self, time: f32) -> f32 {
        let t = ((time - self.start) / self.seconds).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// The seat `share` of the way: rising as the figure sits, sinking as
    /// it stands.
    fn seat(&self, share: f32) -> Option<Seat> {
        match self.seats {
            (_, Some(to)) => Some(to.lowered((1.0 - share) * SEAT_RISE)),
            (Some(from), None) => Some(from.lowered(share * SEAT_RISE)),
            (None, None) => None,
        }
    }
}

/// A drape in progress.
#[derive(Debug)]
pub struct Drape {
    pub cloth: Cloth,
    body: BodyField,
    /// Seconds simulated.
    pub time: f32,
    /// Where each panel's points start in the cloth.
    pub firsts: Vec<u32>,
    posable: Option<Posable>,
    /// How the body stands, or is moving to.
    stance: Stance,
    motion: Option<Motion>,
    /// When the body last stopped moving, s.
    still_since: f32,
    /// The body as it is now, mm, if it has moved; and how many times it
    /// has been moved.
    moved: Option<(u32, Vec<[f32; 3]>)>,
    /// The seat the figure sits on, if it sits.
    seat: Option<Seat>,
}

impl Drape {
    /// Starts a drape of `garment`, its panels placed at `starts` (mm), in
    /// `fabric`, on the body `body` (points mm, triangles facing out).
    pub fn new(
        garment: &Garment,
        starts: &[Vec<[f64; 3]>],
        fabric: Fabric,
        body: (&[[f64; 3]], &[[u32; 3]]),
    ) -> Drape {
        let mut builder = ClothBuilder::new(fabric);
        let mut firsts = Vec::new();
        for (panel, start) in garment.panels.iter().zip(starts) {
            // Flat in the fabric's own directions: the grain along +y.
            let (g0, g1) = (panel.grain[0], panel.grain[1]);
            let rest: Vec<[f32; 2]> = panel
                .mesh
                .points
                .iter()
                .map(|p| {
                    let (u, v) = (p[0] * g1 - p[1] * g0, p[0] * g0 + p[1] * g1);
                    [(u / 1000.0) as f32, (v / 1000.0) as f32]
                })
                .collect();
            let start: Vec<V3> = start
                .iter()
                .map(|p| p.map(|c| (c / 1000.0) as f32))
                .collect();
            firsts.push(builder.panel(&rest, &start, &panel.mesh.triangles));
        }
        for [(pa, a), (pb, b)] in &garment.stitches {
            builder.stitch(firsts[*pa] + a, firsts[*pb] + b);
        }
        let mut cloth = builder.build();
        cloth.damping = 6.0;
        cloth.max_speed = MAX_SPEED;
        cloth.stitch_compliance = SEWING[0].1;
        let points: Vec<V3> = body
            .0
            .iter()
            .map(|p| p.map(|c| (c / 1000.0) as f32))
            .collect();
        let field = BodyField::new(&points, body.1, CELL, MARGIN);
        cloth.push_out(&field);
        Drape {
            cloth,
            body: field,
            time: 0.0,
            firsts,
            posable: None,
            stance: Stance::STANDING,
            motion: None,
            still_since: 0.0,
            moved: None,
            seat: None,
        }
    }

    /// The figure the cloth is on, ready to pose (standing as measured, as
    /// it was draped).
    pub fn posable(&mut self, posable: Posable) {
        self.posable = Some(posable);
    }

    /// Moves the body into `pose`, the cloth carried along: from how it
    /// stands now, or as far as it has got towards another pose. Nothing
    /// without a posable figure.
    pub fn pose(&mut self, pose: Pose) {
        let Some(posable) = &self.posable else {
            return;
        };
        let to = posable.rig.stance(&posable.rest, pose);
        let from = match &self.motion {
            Some(m) => m.from.towards(&m.to, f64::from(m.share(self.time))),
            None => self.stance,
        };
        if self.motion.is_none() && to == from {
            return;
        }
        // How far the body under the cloth goes: its keyframes, and how
        // long it takes.
        let near = self.near_cloth(posable);
        let samples = 12;
        let mut path = vec![0f32; near.len()];
        let mut last = posable.points(&from);
        for i in 1..=samples {
            let here = posable.points(&from.towards(&to, f64::from(i) / f64::from(samples)));
            for (length, &v) in path.iter_mut().zip(&near) {
                let (a, b) = (last[v], here[v]);
                *length += [0, 1, 2]
                    .map(|k| (a[k] - b[k]).powi(2))
                    .iter()
                    .sum::<f32>()
                    .sqrt();
            }
            last = here;
        }
        let far = path.iter().fold(0f32, |a, &b| a.max(b));
        let seat = (pose == Pose::Sitting).then(|| {
            let (low, high) = posable.rig.seat(&posable.rest, &to);
            let m = |v: [f64; 3]| v.map(|c| (c / 1000.0) as f32);
            Seat {
                low: m(low),
                high: m(high),
            }
        });
        let from_seat = match &self.motion {
            Some(m) => m.seat(m.share(self.time)),
            None => self.seat,
        };
        self.motion = Some(Motion {
            from,
            to,
            keys: ((far / KEY_STEP).ceil() as usize).max(1),
            start: self.time,
            // At its fastest, half as fast again as on average.
            seconds: (1.5 * far / BODY_SPEED).clamp(0.4, 4.0),
            fields: None,
            seats: (from_seat, seat),
        });
        self.stance = to;
    }

    /// The figure's points (by number) near the cloth.
    fn near_cloth(&self, posable: &Posable) -> Vec<usize> {
        let key = |p: V3| p.map(|c| (c / NEAR_CLOTH).floor() as i32);
        let cells: std::collections::HashSet<[i32; 3]> =
            self.cloth.x.iter().map(|&p| key(p)).collect();
        posable
            .points(&self.stance_now())
            .iter()
            .enumerate()
            .filter(|&(_, &p)| {
                let [i, j, k] = key(p);
                (-1..=1).any(|a| {
                    (-1..=1).any(|b| (-1..=1).any(|c| cells.contains(&[i + a, j + b, k + c])))
                })
            })
            .map(|(v, _)| v)
            .collect()
    }

    /// How the body stands at this moment.
    fn stance_now(&self) -> Stance {
        match &self.motion {
            Some(m) => m.from.towards(&m.to, f64::from(m.share(self.time))),
            None => self.stance,
        }
    }

    /// Whether the body is moving.
    pub fn moving(&self) -> bool {
        self.motion.is_some()
    }

    /// The seat the figure sits on now (m), if any.
    pub fn seat(&self) -> Option<Seat> {
        match &self.motion {
            Some(m) => m.seat(m.share(self.time)),
            None => self.seat,
        }
    }

    /// The body as it is now (mm), if it has moved from standing as
    /// measured; and its move's number, different each time any drape's
    /// body moves.
    pub fn body(&self) -> Option<(u32, &[[f32; 3]])> {
        self.moved.as_ref().map(|(n, points)| (*n, &points[..]))
    }

    /// The field felt at keyframe `key` of `motion`: only round the cloth.
    fn key_field(&self, posable: &Posable, motion: &Motion, key: usize) -> BodyField {
        let share = key as f64 / motion.keys as f64;
        let points = posable.points(&motion.from.towards(&motion.to, share));
        let mut low = [f32::MAX; 3];
        let mut high = [f32::MIN; 3];
        for p in &self.cloth.x {
            for k in 0..3 {
                low[k] = low[k].min(p[k] - MOVING_MARGIN);
                high[k] = high[k].max(p[k] + MOVING_MARGIN);
            }
        }
        BodyField::around(
            &points,
            &posable.moving,
            (low, high),
            MOVING_CELL,
            MOVING_BAND,
        )
    }

    /// Moves the body and the cloth with it on a frame of `dt` seconds.
    fn step_moving(&mut self, dt: f32) {
        let (Some(mut motion), Some(posable)) = (self.motion.take(), self.posable.take()) else {
            return;
        };
        let share = motion.share(self.time);
        let at = share * motion.keys as f32;
        let key = (at.floor() as usize).min(motion.keys - 1);
        motion.fields = match motion.fields.take() {
            Some((k, from, to)) if k == key => Some((k, from, to)),
            Some((k, _, to)) if k + 1 == key => {
                Some((key, to, self.key_field(&posable, &motion, key + 1)))
            }
            _ => Some((
                key,
                self.key_field(&posable, &motion, key),
                self.key_field(&posable, &motion, key + 1),
            )),
        };
        if let Some((_, from, to)) = &motion.fields {
            let between = Between {
                from,
                to,
                share: at - key as f32,
            };
            let seated = Seated {
                body: &between,
                seat: motion.seat(share),
            };
            self.cloth.step(dt, SUBSTEPS, &seated);
        }
        self.time += dt;
        let now = motion.share(self.time);
        let stance = motion.from.towards(&motion.to, f64::from(now));
        // The figure lowered (sitting down), the cloth goes down with it.
        let was = motion.from.towards(&motion.to, f64::from(share));
        let lowered = (stance.lowered() - was.lowered()) / 1000.0;
        self.cloth.shift([0.0, -lowered as f32, 0.0]);
        let shown: Vec<[f32; 3]> = posable
            .points(&stance)
            .iter()
            .map(|p| p.map(|c| c * 1000.0))
            .collect();
        let count = MOVES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        if now >= 1.0 {
            // Still: the whole body felt as it is now.
            let points = posable.points(&motion.to);
            self.body = BodyField::new(&points, &posable.skin, CELL, MARGIN);
            self.still_since = self.time;
            self.seat = motion.seats.1;
        } else {
            self.motion = Some(motion);
        }
        self.moved = Some((count, shown));
        self.posable = Some(posable);
    }

    /// Whether the seams are still being sewn.
    pub fn sewing(&self) -> bool {
        self.time < SEWING.iter().map(|s| s.0).sum::<f32>()
    }

    /// Moves on a frame of `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        if self.motion.is_some() {
            self.step_moving(dt);
            return;
        }
        let mut until = 0.0;
        let mut stage = None;
        for &(seconds, compliance) in &SEWING {
            until += seconds;
            if self.time < until {
                stage = Some(compliance);
                break;
            }
        }
        match stage {
            Some(compliance) => self.cloth.stitch_compliance = compliance,
            None => {
                self.cloth.damping = 3.0;
                self.cloth.stitch_compliance = 1e-8;
            }
        }
        let seated = Seated {
            body: &self.body,
            seat: self.seat,
        };
        self.cloth.step(dt, SUBSTEPS, &seated);
        self.time += dt;
    }

    /// Sewn, and hanging still after a second at least: just sewn, before
    /// gravity has pulled at it, it is still too.
    pub fn settled(&self) -> bool {
        let sewn: f32 = SEWING.iter().map(|s| s.0).sum();
        self.motion.is_none()
            && self.time > sewn + 1.0
            && self.time > self.still_since + STILL_AFTER
            && self.cloth.step_speed() < STILL
    }

    /// How the cloth is pulled, point by point.
    pub fn strain(&self) -> crate::cloth::Strain {
        self.cloth.strain()
    }

    /// The room between each point of the cloth and the body, m.
    pub fn room(&self) -> Vec<f32> {
        self.cloth.room(&self.body)
    }

    /// The body's distance at a point (m): for checking.
    pub fn clearance(&self, p: V3) -> f32 {
        crate::cloth::Collider::distance(&self.body, p).0
    }
}
