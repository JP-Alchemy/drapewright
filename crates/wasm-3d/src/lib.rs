//! WebAssembly facade for Drapewright's 3D side: the avatar, the view of
//! it, and the project's garment draped on it. It runs in the sim worker,
//! apart from the 2D engine, and is loaded only when the avatar is first
//! wanted.
//!
//! Like the 2D facade it only converts between JavaScript and Rust types;
//! shaping, measuring and fitting live in `drapewright-body`, draping in
//! `drapewright-sim`. Values cross
//! the boundary as plain JavaScript objects shaped like the TypeScript
//! types generated from the Rust ones (`packages/engine/src/generated`).

pub mod drape;
#[cfg(target_arch = "wasm32")]
mod view;

use std::collections::BTreeMap;

pub use drape::{ClothReading, ClothShows, DrapeProgress, DrapeRequest, DrapeStage, FabricPreset};
use drapewright_body::shape::{OLD, YOUNG};
use drapewright_body::{BodyModel, Shape, fit};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use wasm_bindgen::prelude::*;

/// Called once by the host after the module is instantiated.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// What to fit the avatar to: a person's measurements.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AvatarRequest {
    /// A woman's figure, or a man's.
    pub woman: bool,
    /// Years; the avatar is an adult's, from 25 to 90.
    pub age: f64,
    /// The person's measurements, mm, by name (as in a measurement set).
    pub values: BTreeMap<String, f64>,
}

/// What shapes the avatar: each setting from 0 to 1, age in years, and
/// MakeHuman's measure shapes from −1 to 1.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AvatarSettings {
    pub woman: f64,
    pub age: f64,
    pub muscle: f64,
    pub weight: f64,
    pub height: f64,
    pub proportions: f64,
    pub cup: f64,
    pub firmness: f64,
    pub local: BTreeMap<String, f64>,
}

impl From<AvatarSettings> for Shape {
    fn from(s: AvatarSettings) -> Self {
        Shape {
            woman: s.woman,
            age: s.age,
            muscle: s.muscle,
            weight: s.weight,
            height: s.height,
            proportions: s.proportions,
            cup: s.cup,
            firmness: s.firmness,
            local: s.local,
        }
    }
}

impl From<Shape> for AvatarSettings {
    fn from(s: Shape) -> Self {
        AvatarSettings {
            woman: s.woman,
            age: s.age,
            muscle: s.muscle,
            weight: s.weight,
            height: s.height,
            proportions: s.proportions,
            cup: s.cup,
            firmness: s.firmness,
            local: s.local,
        }
    }
}

/// A fitted avatar, and how close it comes to the person.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AvatarFit {
    pub settings: AvatarSettings,
    /// Every measurement the avatar takes, mm.
    pub measured: BTreeMap<String, f64>,
    /// For each of the person's measurements the avatar takes, how far the
    /// avatar's is from it (the avatar's less the person's), mm.
    pub residuals: BTreeMap<String, f64>,
}

/// Fits the avatar to a person's measurements.
pub fn fit_avatar(model: &BodyModel, request: &AvatarRequest) -> AvatarFit {
    let fitted = fit(
        model,
        &request.values,
        request.woman,
        request.age.clamp(YOUNG, OLD),
    );
    AvatarFit {
        settings: fitted.shape.into(),
        measured: fitted
            .measured
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        residuals: fitted.residuals,
    }
}

/// The avatar's body, loaded and ready to fit.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Avatar {
    model: BodyModel,
}

#[wasm_bindgen]
impl Avatar {
    /// Loads a body file (`assets/body/adult.dwbody`).
    #[wasm_bindgen(constructor)]
    pub fn new(bytes: &[u8]) -> Result<Avatar, JsError> {
        BodyModel::from_bytes(bytes)
            .map(|model| Avatar { model })
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Fits the avatar to an `AvatarRequest`, giving an `AvatarFit`.
    pub fn fit(&self, request: JsValue) -> Result<JsValue, JsError> {
        let request: AvatarRequest =
            serde_wasm_bindgen::from_value(request).map_err(|e| JsError::new(&e.to_string()))?;
        fit_avatar(&self.model, &request)
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .map_err(|e| JsError::new(&e.to_string()))
    }

    /// Starts draping a `DrapeRequest` on the avatar.
    pub fn drape(&self, request: JsValue) -> Result<Draping, JsError> {
        let request: DrapeRequest =
            serde_wasm_bindgen::from_value(request).map_err(|e| JsError::new(&e.to_string()))?;
        drape::Draping::start(&self.model, &request)
            .map(Draping)
            .map_err(|e| JsError::new(&e.to_string()))
    }
}

/// A drape in progress on the avatar, moved on a frame at a time.
#[wasm_bindgen]
#[derive(Debug)]
pub struct Draping(drape::Draping);

#[wasm_bindgen]
impl Draping {
    /// The pieces left out, sewn to nothing, by id.
    #[wasm_bindgen(getter, js_name = leftOut)]
    pub fn left_out(&self) -> Vec<String> {
        self.0.left_out.clone()
    }

    /// The facings and linings left out, as they go inside, by id.
    #[wasm_bindgen(getter)]
    pub fn inside(&self) -> Vec<String> {
        self.0.inside.clone()
    }

    /// Moves the drape on a frame, giving a `DrapeProgress`.
    pub fn step(&mut self) -> Result<JsValue, JsError> {
        self.0
            .step()
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .map_err(|e| JsError::new(&e.to_string()))
    }
}
