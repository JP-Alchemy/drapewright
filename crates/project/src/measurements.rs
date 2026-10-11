//! Body measurements (`docs/file-format.md` §10.2): the vocabulary of what
//! can be measured, and the sets of a person's measurements a project keeps.
//! Values are millimetres. The names and how-to-measure text shown to people
//! live with the app's other words (`web/src/strings`).

use std::collections::BTreeMap;

use drapewright_pattern_core::Id;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Whose figure: which measurements are asked for, and (later) the
/// avatar's shape.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Figure {
    #[default]
    Woman,
    Man,
}

/// Taken around the body, or along it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MeasureKind {
    Around,
    Along,
}

/// Where a measurement belongs in a form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MeasurementGroup {
    Height,
    Around,
    Torso,
    Arms,
    Legs,
}

/// The measurements Drapewright knows, in the order a form asks for them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum MeasurementKey {
    Height,
    Bust,
    HighBust,
    Underbust,
    Waist,
    HighHip,
    Hip,
    Neck,
    ShoulderWidth,
    ShoulderLength,
    AcrossFront,
    AcrossBack,
    BackWaistLength,
    FrontWaistLength,
    BustSpan,
    ShoulderToBust,
    WaistToHip,
    ArmLength,
    UpperArm,
    Wrist,
    Inseam,
    Outseam,
    CrotchDepth,
    Thigh,
    Knee,
    Calf,
    Ankle,
}

/// What Drapewright knows about a measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeasurementInfo {
    pub key: MeasurementKey,
    pub kind: MeasureKind,
    pub group: MeasurementGroup,
    /// Taken for women's figures only (the bust's shape).
    pub women_only: bool,
    /// The usual range for adults, mm. A value outside it is probably in
    /// the wrong units, and the form says so.
    pub usual: (f64, f64),
    /// FreeSewing's name for the same measurement, where it has one.
    pub freesewing: Option<String>,
}

impl MeasurementKey {
    pub const ALL: [MeasurementKey; 27] = [
        MeasurementKey::Height,
        MeasurementKey::Bust,
        MeasurementKey::HighBust,
        MeasurementKey::Underbust,
        MeasurementKey::Waist,
        MeasurementKey::HighHip,
        MeasurementKey::Hip,
        MeasurementKey::Neck,
        MeasurementKey::ShoulderWidth,
        MeasurementKey::ShoulderLength,
        MeasurementKey::AcrossFront,
        MeasurementKey::AcrossBack,
        MeasurementKey::BackWaistLength,
        MeasurementKey::FrontWaistLength,
        MeasurementKey::BustSpan,
        MeasurementKey::ShoulderToBust,
        MeasurementKey::WaistToHip,
        MeasurementKey::ArmLength,
        MeasurementKey::UpperArm,
        MeasurementKey::Wrist,
        MeasurementKey::Inseam,
        MeasurementKey::Outseam,
        MeasurementKey::CrotchDepth,
        MeasurementKey::Thigh,
        MeasurementKey::Knee,
        MeasurementKey::Calf,
        MeasurementKey::Ankle,
    ];

    /// The key as stored in a project ("highBust").
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_string))
            .expect("keys are strings")
    }

    pub fn from_name(name: &str) -> Option<Self> {
        serde_json::from_value(serde_json::Value::from(name)).ok()
    }

    pub fn info(self) -> MeasurementInfo {
        use MeasureKind::{Along, Around};
        use MeasurementGroup as G;
        use MeasurementKey as K;
        let (kind, group, women_only, usual, freesewing) = match self {
            K::Height => (Along, G::Height, false, (1300.0, 2200.0), None),
            K::Bust => (Around, G::Around, false, (650.0, 1600.0), Some("chest")),
            K::HighBust => (Around, G::Around, true, (600.0, 1500.0), Some("highBust")),
            K::Underbust => (Around, G::Around, true, (550.0, 1400.0), Some("underbust")),
            K::Waist => (Around, G::Around, false, (500.0, 1500.0), Some("waist")),
            K::HighHip => (Around, G::Around, false, (650.0, 1600.0), Some("hips")),
            K::Hip => (Around, G::Around, false, (700.0, 1700.0), Some("seat")),
            K::Neck => (Around, G::Around, false, (280.0, 550.0), Some("neck")),
            K::ShoulderWidth => (
                Along,
                G::Torso,
                false,
                (300.0, 550.0),
                Some("shoulderToShoulder"),
            ),
            K::ShoulderLength => (Along, G::Torso, false, (90.0, 190.0), None),
            K::AcrossFront => (Along, G::Torso, false, (250.0, 500.0), None),
            K::AcrossBack => (Along, G::Torso, false, (280.0, 520.0), None),
            K::BackWaistLength => (Along, G::Torso, false, (300.0, 550.0), None),
            K::FrontWaistLength => (
                Along,
                G::Torso,
                false,
                (330.0, 650.0),
                Some("hpsToWaistFront"),
            ),
            K::BustSpan => (Along, G::Torso, true, (120.0, 300.0), Some("bustSpan")),
            K::ShoulderToBust => (Along, G::Torso, true, (180.0, 380.0), Some("hpsToBust")),
            K::WaistToHip => (Along, G::Torso, false, (120.0, 300.0), Some("waistToSeat")),
            K::ArmLength => (
                Along,
                G::Arms,
                false,
                (450.0, 750.0),
                Some("shoulderToWrist"),
            ),
            K::UpperArm => (Around, G::Arms, false, (200.0, 550.0), Some("biceps")),
            K::Wrist => (Around, G::Arms, false, (120.0, 230.0), Some("wrist")),
            K::Inseam => (Along, G::Legs, false, (600.0, 950.0), Some("inseam")),
            K::Outseam => (Along, G::Legs, false, (850.0, 1250.0), Some("waistToFloor")),
            K::CrotchDepth => (Along, G::Legs, false, (200.0, 380.0), Some("crotchDepth")),
            K::Thigh => (Around, G::Legs, false, (400.0, 900.0), Some("upperLeg")),
            K::Knee => (Around, G::Legs, false, (280.0, 550.0), Some("knee")),
            K::Calf => (Around, G::Legs, false, (250.0, 550.0), None),
            K::Ankle => (Around, G::Legs, false, (180.0, 330.0), Some("ankle")),
        };
        MeasurementInfo {
            key: self,
            kind,
            group,
            women_only,
            usual,
            freesewing: freesewing.map(str::to_string),
        }
    }
}

/// The whole vocabulary, in form order.
pub fn vocabulary() -> Vec<MeasurementInfo> {
    MeasurementKey::ALL.iter().map(|k| k.info()).collect()
}

/// Custom measurements' keys start with this.
pub const CUSTOM_PREFIX: &str = "x-";

/// A key a measurement set may hold: one of the vocabulary's, or custom.
pub fn is_valid_key(key: &str) -> bool {
    MeasurementKey::from_name(key).is_some()
        || key
            .strip_prefix(CUSTOM_PREFIX)
            .is_some_and(|rest| !rest.is_empty())
}

fn one() -> u32 {
    1
}

/// One person's measurements, mm, with the earlier ones kept.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeasurementSet {
    pub id: Id,
    /// Whose they are ("Me", a client's name).
    pub name: String,
    #[serde(default)]
    pub figure: Figure,
    /// Counts up with each new measuring.
    #[serde(default = "one")]
    pub revision: u32,
    /// The day they were taken (`YYYY-MM-DD`).
    #[serde(default)]
    pub taken_on: Option<String>,
    /// Their age in years, for the avatar's shape.
    #[serde(default)]
    pub age: Option<u32>,
    /// Measurement key → mm. Keys are the vocabulary's, or custom ones
    /// starting with `x-`.
    #[serde(default)]
    pub values: BTreeMap<String, f64>,
    /// Earlier revisions, newest first.
    #[serde(default)]
    pub previous: Vec<EarlierMeasurements>,
}

/// A revision kept from before the person was measured again.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EarlierMeasurements {
    pub revision: u32,
    #[serde(default)]
    pub taken_on: Option<String>,
    pub values: BTreeMap<String, f64>,
}

impl MeasurementSet {
    /// The value of a vocabulary measurement, mm.
    pub fn get(&self, key: MeasurementKey) -> Option<f64> {
        self.values.get(&key.name()).copied()
    }

    /// Why the set can't be kept as it is, in plain words.
    pub fn problem(&self) -> Option<String> {
        if self.name.trim().is_empty() {
            return Some("Give these measurements a name.".into());
        }
        if let Some(date) = &self.taken_on
            && !is_date(date)
        {
            return Some(format!("“{date}” is not a date (use YYYY-MM-DD)."));
        }
        if self.age.is_some_and(|age| !(1..=120).contains(&age)) {
            return Some("An age is in years, from 1 to 120.".into());
        }
        for (key, value) in &self.values {
            if !is_valid_key(key) {
                return Some(format!("“{key}” is not a measurement Drapewright knows."));
            }
            if !value.is_finite() || *value <= 0.0 {
                return Some("Measurements must be more than zero.".into());
            }
        }
        None
    }
}

/// `YYYY-MM-DD`, with a month and day that can exist.
fn is_date(s: &str) -> bool {
    let parts: Vec<&str> = s.split('-').collect();
    let number = |p: &str, len: usize| {
        (p.len() == len && p.bytes().all(|b| b.is_ascii_digit()))
            .then(|| p.parse::<u32>().ok())
            .flatten()
    };
    match parts.as_slice() {
        [y, m, d] => {
            number(y, 4).is_some()
                && number(m, 2).is_some_and(|m| (1..=12).contains(&m))
                && number(d, 2).is_some_and(|d| (1..=31).contains(&d))
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_round_trips_and_has_a_sensible_range() {
        let names: Vec<String> = MeasurementKey::ALL.iter().map(|k| k.name()).collect();
        for (key, name) in MeasurementKey::ALL.iter().zip(&names) {
            assert_eq!(MeasurementKey::from_name(name), Some(*key));
            let (lo, hi) = key.info().usual;
            assert!(0.0 < lo && lo < hi, "{name}");
        }
        // No key twice, and the form order is the declaration order.
        let unique: std::collections::HashSet<&String> = names.iter().collect();
        assert_eq!(unique.len(), names.len());
        assert_eq!(names[0], "height");
        assert_eq!(names[2], "highBust");
    }

    #[test]
    fn keys_and_values_are_checked() {
        let mut set = MeasurementSet {
            id: "m1".into(),
            name: "Me".into(),
            figure: Figure::Woman,
            revision: 1,
            taken_on: Some("2026-10-11".into()),
            age: Some(40),
            values: BTreeMap::from([("bust".into(), 960.0), ("x-elbow".into(), 260.0)]),
            previous: Vec::new(),
        };
        assert_eq!(set.problem(), None);
        assert_eq!(set.get(MeasurementKey::Bust), Some(960.0));
        set.values.insert("bustt".into(), 900.0);
        assert!(set.problem().unwrap().contains("bustt"));
        set.values.remove("bustt");
        set.values.insert("waist".into(), -1.0);
        assert!(set.problem().is_some());
        set.values.remove("waist");
        set.taken_on = Some("2026-13-01".into());
        assert!(set.problem().is_some());
        set.taken_on = None;
        set.age = Some(0);
        assert!(set.problem().unwrap().contains("years"));
        set.age = None;
        assert_eq!(set.problem(), None);
        assert!(!is_valid_key("x-"));
    }
}
