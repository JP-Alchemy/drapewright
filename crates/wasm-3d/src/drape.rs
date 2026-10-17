//! The project's garment draped on the avatar: what the page asks for, and
//! how the drape is going. The draping itself is `drapewright-sim`'s.

use drapewright_body::{BodyModel, Pose, Shape};
pub use drapewright_fabric::{Fabric, FabricPreset, Measured};
use std::sync::Arc;

use drapewright_pattern_core::{EdgeKind, Piece};
use drapewright_seams::Seam;
use drapewright_sim::draping::{self, ClothEdge, Stage};
use drapewright_sim::garment::GarmentError;
use drapewright_sim::heatmap;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AvatarSettings;

/// How far apart the cloth's points are, mm: and on the graphics card,
/// which moves four times as many on as fast, finer.
pub(crate) const SPACING: f64 = 20.0;
const CARD_SPACING: f64 = 12.0;

/// A graphics card to drape on: the 3D view's, where it can compute.
pub type Card = (wgpu::Device, wgpu::Queue);

thread_local! {
    static CARD: std::cell::RefCell<Option<Card>> = const { std::cell::RefCell::new(None) };
}

/// Drapes from now on on `card` (none: here).
pub fn use_card(card: Option<Card>) {
    CARD.with(|c| *c.borrow_mut() = card);
}

/// The graphics card drapes are made on, if any.
pub fn card() -> Option<Card> {
    CARD.with(|c| c.borrow().clone())
}

/// What to drape: the project's pieces, sewn by its seams, in a fabric, on
/// the avatar shaped by `settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DrapeRequest {
    pub pieces: Vec<Piece>,
    pub seams: Vec<Seam>,
    /// The fabric, or for one's own fabric the preset it is like.
    pub fabric: FabricPreset,
    /// One's own fabric: what was measured of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub measured: Option<Measured>,
    pub settings: AvatarSettings,
    /// The pose to move into once draped: standing as measured if none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub pose: Option<AvatarPose>,
    /// Worn open: its buttons undone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub open: Option<bool>,
}

/// How the avatar stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum AvatarPose {
    /// As measured: arms out and down, legs straight.
    #[default]
    Standing,
    /// Arms hanging by the sides.
    ArmsDown,
    /// Both arms reaching forward at shoulder height.
    Reaching,
    /// Both arms raised overhead.
    ArmsUp,
    /// Mid-stride.
    Stepping,
    /// Sitting.
    Sitting,
}

impl From<AvatarPose> for Pose {
    fn from(p: AvatarPose) -> Pose {
        match p {
            AvatarPose::Standing => Pose::Standing,
            AvatarPose::ArmsDown => Pose::ArmsDown,
            AvatarPose::Reaching => Pose::Reaching,
            AvatarPose::ArmsUp => Pose::ArmsUp,
            AvatarPose::Stepping => Pose::Stepping,
            AvatarPose::Sitting => Pose::Sitting,
        }
    }
}

/// Where a drape has got to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum DrapeStage {
    /// The seams closing round the body.
    Sewing,
    /// Sewn, and coming to rest.
    Settling,
    /// The body moving into a pose, the garment with it.
    Posing,
    /// Hanging still.
    Settled,
    /// Still moving after the longest a drape is given.
    Stopped,
}

/// How a drape is going.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DrapeProgress {
    pub stage: DrapeStage,
    /// Seconds draped, as the cloth sees it.
    pub time: f64,
    /// The widest gap left in a seam, mm.
    pub widest_gap: f64,
}

/// How big the garment is round the body at one of the figure's levels,
/// measured flat as a sewist measures a finished garment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Girth {
    /// `bust`, `waist` or `hip`.
    pub level: String,
    /// The garment round the body there, mm; none where it doesn't reach.
    pub garment: Option<f64>,
    /// The body round there, mm.
    pub body: f64,
}

/// Why a garment cannot be draped.
#[derive(Debug)]
pub enum DrapeError {
    /// No confirmed seams: nothing is sewn to anything.
    NothingSewn,
    /// The pieces sewn go only half way round the body: a pattern for one
    /// side, its pieces cut once.
    HalfRound,
    Garment(GarmentError),
}

impl From<draping::DrapeError> for DrapeError {
    fn from(e: draping::DrapeError) -> Self {
        match e {
            draping::DrapeError::NothingSewn => DrapeError::NothingSewn,
            draping::DrapeError::HalfRound => DrapeError::HalfRound,
            draping::DrapeError::Garment(e) => DrapeError::Garment(e),
        }
    }
}

impl std::fmt::Display for DrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrapeError::NothingSewn => f.write_str("no seams are sewn yet"),
            DrapeError::HalfRound => f.write_str(
                "the pieces sewn go only half way round the body, from a front to a back. \
                 Under Cutting, set How many to 2 (a left and a right) on the pieces cut as \
                 pairs, then press Find seams again",
            ),
            DrapeError::Garment(e) => e.fmt(f),
        }
    }
}

impl std::error::Error for DrapeError {}

impl From<Stage> for DrapeStage {
    fn from(s: Stage) -> Self {
        match s {
            Stage::Sewing => DrapeStage::Sewing,
            Stage::Settling => DrapeStage::Settling,
            Stage::Posing => DrapeStage::Posing,
            Stage::Settled => DrapeStage::Settled,
            Stage::Stopped => DrapeStage::Stopped,
        }
    }
}

/// A drape in progress on the avatar: `drapewright-sim`'s, in the page's
/// terms.
#[derive(Debug)]
pub struct Draping(draping::Draping);

impl Draping {
    /// Starts draping: the pieces sewn by confirmed seams, but for facings
    /// and linings and pieces between the legs, placed round the avatar and
    /// let go.
    pub fn start(model: &BodyModel, request: &DrapeRequest) -> Result<Draping, DrapeError> {
        Draping::start_on(model, request, None)
    }

    /// Starts draping, moved on on `card` if one is given and it can (else
    /// here, more coarsely).
    pub fn start_on(
        model: &BodyModel,
        request: &DrapeRequest,
        card: Option<&Card>,
    ) -> Result<Draping, DrapeError> {
        let fabric = request
            .measured
            .map_or(request.fabric.fabric(), |m| m.fabric(request.fabric));
        let mut draping = draping::Draping::start(
            model,
            &Shape::from(request.settings.clone()),
            draping::Request {
                pieces: &request.pieces,
                seams: &request.seams,
                fabric,
                open: request.open == Some(true),
                spacing: if card.is_some() {
                    CARD_SPACING
                } else {
                    SPACING
                },
            },
            request.pose.unwrap_or_default().into(),
        )?;
        if let Some((device, queue)) = card {
            match drapewright_sim_gpu::GpuCloth::new(device, queue, &draping.drape().cloth) {
                Ok(gpu) => draping.use_stepper(Box::new(gpu)),
                Err(_) => return Draping::start_on(model, request, None),
            }
        }
        Ok(Draping(draping))
    }

    /// The pieces sewn to nothing, by id: left out.
    pub fn left_out(&self) -> &[String] {
        &self.0.left_out
    }

    /// The facings and linings sewn to nothing, by id: left out, as they
    /// go inside.
    pub fn inside(&self) -> &[String] {
        &self.0.inside
    }

    /// The facings and linings sewn but not placed inside the rest (one
    /// sewn only to another, with nothing to lie on), by id: left out.
    pub fn unplaced(&self) -> &[String] {
        &self.0.unplaced
    }

    /// The facings and linings draped inside the rest, by id.
    pub fn lined(&self) -> &[String] {
        &self.0.lined
    }

    /// How many buttons it has, done up or not.
    pub fn buttons(&self) -> usize {
        self.0.buttons
    }

    /// The garment's size round the body at its bust, waist and hip: as
    /// placed, then as it settles standing.
    pub fn girths(&self) -> Vec<Girth> {
        self.0
            .girths
            .iter()
            .map(|g| Girth {
                level: g.level.into(),
                garment: g.garment,
                body: g.body,
            })
            .collect()
    }

    /// Moves the body into `pose` once the garment has settled (at once if
    /// it has), the garment with it.
    pub fn pose(&mut self, pose: AvatarPose) {
        self.0.pose(pose.into());
    }

    /// The body as it is now (mm), if it has moved from standing as
    /// measured; and its move's number, different each time it moves.
    pub fn body(&self) -> Option<(u32, &[[f32; 3]])> {
        self.0.drape().body()
    }

    /// The seat the figure sits on now, if it sits: its lowest and highest
    /// corners, mm.
    pub fn seat(&self) -> Option<([f32; 3], [f32; 3])> {
        self.0
            .drape()
            .seat()
            .map(|s| (s.low.map(|c| c * 1000.0), s.high.map(|c| c * 1000.0)))
    }

    /// Whether a frame can be taken now: on the graphics card, not until
    /// it has caught up.
    pub fn ready(&mut self) -> bool {
        self.0.ready()
    }

    /// Whether the cloth is moved on on the graphics card.
    pub fn on_card(&self) -> bool {
        self.0.drape().stepped_elsewhere()
    }

    /// Moves the drape on a frame, unless it is done (or the graphics card
    /// is busy).
    pub fn step(&mut self) -> DrapeProgress {
        self.0.step();
        self.progress()
    }

    pub fn progress(&self) -> DrapeProgress {
        let drape = self.0.drape();
        DrapeProgress {
            stage: self.0.stage().into(),
            time: f64::from(drape.time),
            widest_gap: f64::from(drape.cloth.widest_stitch()) * 1000.0,
        }
    }

    /// Where the cloth's points are, mm.
    pub fn points(&self) -> Vec<[f32; 3]> {
        self.0
            .drape()
            .cloth
            .x
            .iter()
            .map(|p| p.map(|c| c * 1000.0))
            .collect()
    }

    /// The cloth's triangles, as indices into its points.
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.0.drape().cloth.triangles
    }
}

/// What the cloth on the avatar is shown as: its fabric, or a heatmap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ClothShows {
    /// In the fabric's colour.
    #[default]
    Fabric,
    /// Where it pulls: none to `PULL_SCALE` N/m.
    Pull,
    /// The room between it and the body: touching to `ROOM_SCALE` mm.
    Room,
}

/// How much paler facings and linings are shown than the fabric, towards
/// white.
const INSIDE_PALER: f32 = 0.45;

pub use drapewright_sim::heatmap::{PULL_SCALE, ROOM_SCALE};

/// The cloth at a point.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClothReading {
    /// How hard it pulls, N/m.
    pub pull: f64,
    /// How far it is stretched beyond its cut size (0.03: 3%).
    pub stretch: f64,
    /// The room between it and the body, mm.
    pub room: f64,
}

/// The cloth as last seen: where its points are (mm), its triangles, and
/// how it is at each point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClothState {
    pub points: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// N/m.
    pull: Vec<f32>,
    stretch: Vec<f32>,
    /// mm.
    room: Vec<f32>,
    /// The points from this one on are inside (facings, linings), if any.
    inside_from: Option<usize>,
    /// The pieces' edges in it.
    edges: Arc<[ClothEdge]>,
    /// Which way each triangle faces out of the garment: 1 its corners'
    /// way round, −1 the other.
    facing: Vec<f32>,
}

/// One of the pieces' edges as seen in the view: where its points are, which
/// of them face the eye, and which way is out of the piece at its middle.
/// A piece cut as a pair, or on the fold, has its edges seen more than
/// once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SeenEdge {
    pub piece: String,
    pub edge: String,
    pub kind: EdgeKind,
    /// Its points on the view, CSS px from its top left, from the edge's
    /// start to its end: x, y, then the next point's.
    pub points: Vec<f32>,
    /// Whether each point faces the eye: in front of it, and the cloth there
    /// turned towards it.
    pub shown: Vec<bool>,
    /// Which point is its middle.
    pub middle: u32,
    /// Out of the piece at its middle point, along the cloth: how far on
    /// the view each mm goes, CSS px.
    pub out: [f32; 2],
    /// Along it from its start, and back along it from its end: how far on
    /// the view each mm goes, CSS px (for moving its corners).
    pub ends: [[f32; 2]; 2],
    /// How squarely the cloth faces the eye at its start and its end: 1
    /// straight on, 0 edge on (at the figure's side).
    pub facing: [f32; 2],
    /// How long it is as draped, mm.
    pub length: f64,
}

impl Draping {
    /// The cloth as it is now.
    pub fn state(&self) -> ClothState {
        let drape = self.0.drape();
        let strain = drape.strain();
        ClothState {
            points: self.points(),
            triangles: self.triangles().to_vec(),
            pull: strain.pull,
            stretch: strain.stretch,
            room: drape.room().iter().map(|r| r * 1000.0).collect(),
            inside_from: (0..drape.cloth.x.len()).find(|&k| drape.cloth.inside(k)),
            edges: self.0.edges.clone(),
            facing: drape.cloth.facing().to_vec(),
        }
    }
}

impl ClothState {
    /// Each point's colour, shown as `shows`, the fabric being `fabric`
    /// (sRGB, 0 to 1).
    pub fn colours(&self, shows: ClothShows, fabric: [f32; 3]) -> Vec<[f32; 3]> {
        match shows {
            // Facings and linings paler: seen inside the garment.
            ClothShows::Fabric => (0..self.points.len())
                .map(|k| {
                    if self.inside_from.is_some_and(|first| k >= first) {
                        fabric.map(|c| c + (1.0 - c) * INSIDE_PALER)
                    } else {
                        fabric
                    }
                })
                .collect(),
            ClothShows::Pull => self.pull.iter().map(|&p| heatmap::pull_colour(p)).collect(),
            ClothShows::Room => self.room.iter().map(|&r| heatmap::room_colour(r)).collect(),
        }
    }

    /// The pieces' edges as seen from `eye`, `project` taking a point (mm)
    /// to the view (CSS px), if it is in front of the eye. Facings and
    /// linings, inside, are left out.
    pub fn seen(
        &self,
        eye: [f32; 3],
        project: impl Fn([f32; 3]) -> Option<[f32; 2]>,
    ) -> Vec<SeenEdge> {
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        let unit = |a: [f32; 3]| {
            let l = dot(a, a).sqrt();
            if l > 1e-9 { a.map(|c| c / l) } else { [0.0; 3] }
        };
        let edges: Vec<&ClothEdge> = self
            .edges
            .iter()
            .filter(|e| !e.inside && e.points.len() >= 2)
            .collect();
        // For each point on an edge: the way the cloth faces there, and the
        // middle of the cloth round it (inside the piece).
        let mut slot = vec![u32::MAX; self.points.len()];
        let mut around: Vec<([f32; 3], [f32; 3], f32)> = Vec::new();
        for e in &edges {
            for &k in &e.points {
                if slot[k as usize] == u32::MAX {
                    slot[k as usize] = around.len() as u32;
                    around.push(([0.0; 3], [0.0; 3], 0.0));
                }
            }
        }
        for (n, t) in self.triangles.iter().enumerate() {
            if t.iter().all(|&k| slot[k as usize] == u32::MAX) {
                continue;
            }
            let [a, b, c] = t.map(|k| self.points[k as usize]);
            let out = self.facing.get(n).copied().unwrap_or(1.0);
            let normal = cross(sub(b, a), sub(c, a)).map(|c| c * out);
            let middle = [0, 1, 2].map(|i| (a[i] + b[i] + c[i]) / 3.0);
            for &k in t {
                if let Some(at) = around.get_mut(slot[k as usize] as usize) {
                    at.0 = [0, 1, 2].map(|i| at.0[i] + normal[i]);
                    at.1 = [0, 1, 2].map(|i| at.1[i] + middle[i]);
                    at.2 += 1.0;
                }
            }
        }
        edges
            .iter()
            .map(|e| {
                let n = e.points.len();
                let at = |i: usize| self.points[e.points[i] as usize];
                let mut points = Vec::with_capacity(2 * n);
                let mut shown = Vec::with_capacity(n);
                for (i, &k) in e.points.iter().enumerate() {
                    let p = at(i);
                    let seen = project(p);
                    let [x, y] = seen.unwrap_or([f32::NAN; 2]);
                    points.extend([x, y]);
                    let facing = dot(around[slot[k as usize] as usize].0, sub(eye, p)) > 0.0;
                    shown.push(seen.is_some() && facing);
                }
                // Out of the piece at its middle: away from the cloth round
                // it, square to the edge, along the cloth.
                let m = n / 2;
                let p = at(m);
                let along = unit(sub(at((m + 1).min(n - 1)), at(m.saturating_sub(1))));
                let (_, middles, count) = around[slot[e.points[m] as usize] as usize];
                let inward = sub(middles.map(|c| c / count.max(1.0)), p);
                let across = sub(inward, along.map(|c| c * dot(inward, along)));
                let way = unit(across).map(|c| -c);
                let out = match (project(p), project([0, 1, 2].map(|i| p[i] + way[i]))) {
                    (Some(a), Some(b)) => [b[0] - a[0], b[1] - a[1]],
                    _ => [0.0; 2],
                };
                let leaving = |from: usize, to: usize| {
                    let p = at(from);
                    let way = unit(sub(at(to), p));
                    match (project(p), project([0, 1, 2].map(|i| p[i] + way[i]))) {
                        (Some(a), Some(b)) => [b[0] - a[0], b[1] - a[1]],
                        _ => [0.0; 2],
                    }
                };
                // Over two of its points, where there are: steadier than one.
                let ends = [
                    leaving(0, 2.min(n - 1)),
                    leaving(n - 1, (n - 1).saturating_sub(2)),
                ];
                let facing = [0, n - 1].map(|i| {
                    let normal = unit(around[slot[e.points[i] as usize] as usize].0);
                    dot(normal, unit(sub(eye, at(i)))).max(0.0)
                });
                let length = (1..n)
                    .map(|i| f64::from(dot(sub(at(i), at(i - 1)), sub(at(i), at(i - 1))).sqrt()))
                    .sum();
                SeenEdge {
                    piece: e.piece.clone(),
                    edge: e.edge.clone(),
                    kind: e.kind,
                    points,
                    shown,
                    middle: m as u32,
                    out,
                    ends,
                    facing,
                    length,
                }
            })
            .collect()
    }

    /// The cloth where the line of sight from `eye` going `way` (a unit
    /// vector) first meets it, if it does.
    pub fn pick(&self, eye: [f32; 3], way: [f32; 3]) -> Option<ClothReading> {
        let sub = |a: [f32; 3], b: [f32; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
        let dot = |a: [f32; 3], b: [f32; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let cross = |a: [f32; 3], b: [f32; 3]| {
            [
                a[1] * b[2] - a[2] * b[1],
                a[2] * b[0] - a[0] * b[2],
                a[0] * b[1] - a[1] * b[0],
            ]
        };
        // The nearest triangle crossed (Möller and Trumbore), and where in
        // it.
        let mut nearest: Option<(f32, [u32; 3], f32, f32)> = None;
        for &t in &self.triangles {
            let [a, b, c] = t.map(|i| self.points[i as usize]);
            let (e1, e2) = (sub(b, a), sub(c, a));
            let p = cross(way, e2);
            let det = dot(e1, p);
            if det.abs() < 1e-9 {
                continue;
            }
            let s = sub(eye, a);
            let u = dot(s, p) / det;
            if !(0.0..=1.0).contains(&u) {
                continue;
            }
            let q = cross(s, e1);
            let v = dot(way, q) / det;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }
            let distance = dot(e2, q) / det;
            if distance > 0.0 && nearest.is_none_or(|n| distance < n.0) {
                nearest = Some((distance, t, u, v));
            }
        }
        let (_, t, u, v) = nearest?;
        let at = |values: &[f32]| {
            let [a, b, c] = t.map(|i| values[i as usize]);
            f64::from((1.0 - u - v) * a + u * b + v * c)
        };
        Some(ClothReading {
            pull: at(&self.pull),
            stretch: at(&self.stretch),
            room: at(&self.room),
        })
    }
}
