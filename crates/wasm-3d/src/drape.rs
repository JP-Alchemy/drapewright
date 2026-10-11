//! The project's garment draped on the avatar: what the page asks for, and
//! how the drape is going. The draping itself is `drapewright-sim`'s.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::Piece;
use drapewright_seams::Seam;
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::{Fabric, preset};
use drapewright_sim::garment::{GarmentError, assemble};
use drapewright_sim::place::{Figure, place};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::AvatarSettings;

/// How far apart the cloth's points are, mm.
const SPACING: f64 = 20.0;
/// A frame of the drape, s.
const FRAME: f32 = 1.0 / 60.0;
/// The longest a drape is given to settle, s.
const LONGEST: f32 = 20.0;

/// The fabrics to drape in, until a real one is calibrated (step 29).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum FabricPreset {
    CottonLawn,
    Poplin,
    Linen,
    Chambray,
    Denim,
    Twill,
    Crepe,
    Satin,
    Jersey,
    RibKnit,
    WoolSuiting,
    Fleece,
}

impl FabricPreset {
    pub const ALL: [FabricPreset; 12] = [
        FabricPreset::CottonLawn,
        FabricPreset::Poplin,
        FabricPreset::Linen,
        FabricPreset::Chambray,
        FabricPreset::Denim,
        FabricPreset::Twill,
        FabricPreset::Crepe,
        FabricPreset::Satin,
        FabricPreset::Jersey,
        FabricPreset::RibKnit,
        FabricPreset::WoolSuiting,
        FabricPreset::Fleece,
    ];

    /// Its name among the simulator's presets.
    pub fn name(self) -> &'static str {
        match self {
            FabricPreset::CottonLawn => "cottonLawn",
            FabricPreset::Poplin => "poplin",
            FabricPreset::Linen => "linen",
            FabricPreset::Chambray => "chambray",
            FabricPreset::Denim => "denim",
            FabricPreset::Twill => "twill",
            FabricPreset::Crepe => "crepe",
            FabricPreset::Satin => "satin",
            FabricPreset::Jersey => "jersey",
            FabricPreset::RibKnit => "ribKnit",
            FabricPreset::WoolSuiting => "woolSuiting",
            FabricPreset::Fleece => "fleece",
        }
    }

    pub fn fabric(self) -> Fabric {
        preset(self.name()).expect("every preset is the simulator's")
    }
}

/// What to drape: the project's pieces, sewn by its seams, in a fabric, on
/// the avatar shaped by `settings`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DrapeRequest {
    pub pieces: Vec<Piece>,
    pub seams: Vec<Seam>,
    pub fabric: FabricPreset,
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

/// Why a garment cannot be draped.
#[derive(Debug)]
pub enum DrapeError {
    /// No confirmed seams: nothing is sewn to anything.
    NothingSewn,
    Garment(GarmentError),
}

impl std::fmt::Display for DrapeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DrapeError::NothingSewn => f.write_str("no seams are sewn yet"),
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
    done: Option<DrapeStage>,
}

impl Draping {
    /// Starts draping: the pieces sewn by confirmed seams, placed round the
    /// avatar and let go.
    pub fn start(model: &BodyModel, request: &DrapeRequest) -> Result<Draping, DrapeError> {
        let seams: Vec<Seam> = request
            .seams
            .iter()
            .filter(|s| s.confirmed)
            .cloned()
            .collect();
        let (sewn, left_out): (Vec<&Piece>, Vec<&Piece>) = request
            .pieces
            .iter()
            .partition(|p| seams.iter().any(|s| s.touches(&p.id)));
        if sewn.is_empty() {
            return Err(DrapeError::NothingSewn);
        }
        let sewn: Vec<Piece> = sewn.into_iter().cloned().collect();
        let garment = assemble(&sewn, &seams, SPACING).map_err(DrapeError::Garment)?;
        let body = model.shape(&Shape::from(request.settings.clone()));
        let starts = place(&garment, &Figure::of(model, &body));
        let drape = Drape::new(
            &garment,
            &starts,
            request.fabric.fabric(),
            (&body.vertices, &model.skin()),
        );
        Ok(Draping {
            drape,
            left_out: left_out.iter().map(|p| p.id.clone()).collect(),
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
