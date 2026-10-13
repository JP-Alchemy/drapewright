//! The project's garment draped on the avatar: what the page asks for, and
//! how the drape is going. The draping itself is `drapewright-sim`'s.

use drapewright_body::{BodyModel, Shape};
pub use drapewright_fabric::{Fabric, FabricPreset, Measured};
use drapewright_pattern_core::Piece;
use drapewright_seams::Seam;
use drapewright_sim::drape::Drape;
use drapewright_sim::garment::{GarmentError, assemble, draped_pieces};
use drapewright_sim::girth::girths;
use drapewright_sim::place::{Figure, half_round, place};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AvatarSettings;

/// How far apart the cloth's points are, mm.
const SPACING: f64 = 20.0;
/// A frame of the drape, s.
const FRAME: f32 = 1.0 / 60.0;
/// The longest a drape is given to settle, s.
const LONGEST: f32 = 20.0;

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

/// A drape in progress on the avatar.
#[derive(Debug)]
pub struct Draping {
    drape: Drape,
    /// The pieces sewn to nothing, by id: left out.
    pub left_out: Vec<String>,
    /// The facings and linings, by id: left out, as they go inside.
    pub inside: Vec<String>,
    /// The pieces between the legs (a crotch), by id: left out.
    pub between_legs: Vec<String>,
    /// The garment's size round the body at its bust, waist and hip.
    pub girths: Vec<Girth>,
    done: Option<DrapeStage>,
}

impl Draping {
    /// Starts draping: the pieces sewn by confirmed seams, but for facings
    /// and linings and pieces between the legs, placed round the avatar and
    /// let go.
    pub fn start(model: &BodyModel, request: &DrapeRequest) -> Result<Draping, DrapeError> {
        let seams: Vec<Seam> = request
            .seams
            .iter()
            .filter(|s| s.confirmed)
            .cloned()
            .collect();
        let draped = draped_pieces(&request.pieces, &seams);
        let sewn = draped.sewn;
        if sewn.is_empty() {
            return Err(DrapeError::NothingSewn);
        }
        let garment = assemble(&sewn, &seams, SPACING).map_err(DrapeError::Garment)?;
        if half_round(&garment) {
            return Err(DrapeError::HalfRound);
        }
        let body = model.shape(&Shape::from(request.settings.clone()));
        let figure = Figure::of(model, &body);
        let starts = place(&garment, &figure);
        let girths = girths(&garment, &starts, &figure)
            .into_iter()
            .zip(&figure.levels)
            .map(|((level, garment), at)| Girth {
                level: level.into(),
                garment,
                body: at.girth,
            })
            .collect();
        let drape = Drape::new(
            &garment,
            &starts,
            request
                .measured
                .map_or(request.fabric.fabric(), |m| m.fabric(request.fabric)),
            (&body.vertices, &model.skin()),
        );
        Ok(Draping {
            drape,
            left_out: draped.alone,
            inside: draped.inside,
            between_legs: draped.between_legs,
            girths,
            done: None,
        })
    }

    /// Moves the drape on a frame, unless it is done.
    pub fn step(&mut self) -> DrapeProgress {
        if self.done.is_none() {
            self.drape.step(FRAME);
            if self.drape.settled() {
                self.done = Some(DrapeStage::Settled);
            } else if self.drape.time >= LONGEST {
                self.done = Some(DrapeStage::Stopped);
            }
        }
        self.progress()
    }

    pub fn progress(&self) -> DrapeProgress {
        let stage = self.done.unwrap_or(if self.drape.sewing() {
            DrapeStage::Sewing
        } else {
            DrapeStage::Settling
        });
        DrapeProgress {
            stage,
            time: f64::from(self.drape.time),
            widest_gap: f64::from(self.drape.cloth.widest_stitch()) * 1000.0,
        }
    }

    /// Where the cloth's points are, mm.
    pub fn points(&self) -> Vec<[f32; 3]> {
        self.drape
            .cloth
            .x
            .iter()
            .map(|p| p.map(|c| c * 1000.0))
            .collect()
    }

    /// The cloth's triangles, as indices into its points.
    pub fn triangles(&self) -> &[[u32; 3]] {
        &self.drape.cloth.triangles
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

/// Pull shown at its strongest, N/m (as in the app's legend).
pub const PULL_SCALE: f32 = 100.0;
/// Room shown up to, mm (as in the app's legend).
pub const ROOM_SCALE: f32 = 40.0;

const fn rgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    ]
}

/// Pale where it hardly pulls, through yellow and orange to red (as in the
/// app's legend).
const PULL_COLOURS: [(f32, [f32; 3]); 4] = [
    (0.0, rgb(0xe9eef4)),
    (0.25, rgb(0xffe08a)),
    (0.6, rgb(0xff9f43)),
    (1.0, rgb(0xd63031)),
];
/// Deep blue where it touches, paler as it stands away (as in the app's
/// legend).
const ROOM_COLOURS: [(f32, [f32; 3]); 3] = [
    (0.0, rgb(0x1c64d1)),
    (0.35, rgb(0x7cc4fa)),
    (1.0, rgb(0xf1f3f5)),
];

/// The colour `t` (0 to 1) along a scale of colours.
fn along(scale: &[(f32, [f32; 3])], t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    for pair in scale.windows(2) {
        let ((t0, c0), (t1, c1)) = (pair[0], pair[1]);
        if t <= t1 {
            let f = (t - t0) / (t1 - t0);
            return [0, 1, 2].map(|k| c0[k] + (c1[k] - c0[k]) * f);
        }
    }
    scale[scale.len() - 1].1
}

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
}

impl Draping {
    /// The cloth as it is now.
    pub fn state(&self) -> ClothState {
        let strain = self.drape.strain();
        ClothState {
            points: self.points(),
            triangles: self.triangles().to_vec(),
            pull: strain.pull,
            stretch: strain.stretch,
            room: self.drape.room().iter().map(|r| r * 1000.0).collect(),
        }
    }
}

impl ClothState {
    /// Each point's colour, shown as `shows`, the fabric being `fabric`
    /// (sRGB, 0 to 1).
    pub fn colours(&self, shows: ClothShows, fabric: [f32; 3]) -> Vec<[f32; 3]> {
        match shows {
            ClothShows::Fabric => vec![fabric; self.points.len()],
            ClothShows::Pull => self
                .pull
                .iter()
                .map(|&p| along(&PULL_COLOURS, p / PULL_SCALE))
                .collect(),
            ClothShows::Room => self
                .room
                .iter()
                .map(|&r| along(&ROOM_COLOURS, r / ROOM_SCALE))
                .collect(),
        }
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
