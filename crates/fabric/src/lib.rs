//! Fabrics: how heavy, how stiff along the grain and across it, how loose
//! on the bias, how they bend. Twelve presets, and fabrics of one's own:
//! like a preset, with what was measured of them at home (a square
//! weighed, a strip stretched under a weight, and a strip slid out over a
//! table's edge until it droops).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A fabric's properties, in SI units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fabric {
    /// Weight, kg per m² (150 g/m² is 0.15).
    pub density: f32,
    /// Stretch stiffness along the grain (warp) and across it (weft), N/m.
    pub warp: f32,
    pub weft: f32,
    /// Shear stiffness: how firmly it resists stretching on the bias, N/m.
    pub shear: f32,
    /// Bending stiffness, N·m.
    pub bending: f32,
    /// Thickness, m, kept between the cloth and the body.
    pub thickness: f32,
    /// Friction against the body, 0 to 1.
    pub friction: f32,
}

impl Fabric {
    /// Stretch stiffness along a direction `angle` radians from the grain,
    /// N/m: firm along warp and weft, loose on the bias.
    pub fn stiffness_at(&self, angle: f32) -> f32 {
        let (s, c) = angle.sin_cos();
        let (c2, s2) = (c * c, s * s);
        // Compliances add, as springs in series.
        let compliance = c2 * c2 / self.warp + s2 * s2 / self.weft + c2 * s2 / self.shear;
        1.0 / compliance
    }
}

/// The fabrics to drape in, ready made.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS, JsonSchema)]
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

const fn woven(
    density: f32,
    warp: f32,
    weft: f32,
    shear: f32,
    bending: f32,
    thickness: f32,
) -> Fabric {
    Fabric {
        density,
        warp,
        weft,
        shear,
        bending,
        thickness,
        friction: 0.4,
    }
}

const fn knit(
    density: f32,
    warp: f32,
    weft: f32,
    shear: f32,
    bending: f32,
    thickness: f32,
) -> Fabric {
    Fabric {
        density,
        warp,
        weft,
        shear,
        bending,
        thickness,
        friction: 0.5,
    }
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

    /// Its name, as written in files.
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

    /// The preset by its name.
    pub fn named(name: &str) -> Option<FabricPreset> {
        FabricPreset::ALL.into_iter().find(|p| p.name() == name)
    }

    /// What it is like.
    pub fn fabric(self) -> Fabric {
        match self {
            FabricPreset::CottonLawn => woven(0.07, 6_000.0, 4_000.0, 120.0, 2e-6, 0.0003),
            FabricPreset::Poplin => woven(0.12, 9_000.0, 6_000.0, 200.0, 6e-6, 0.0004),
            FabricPreset::Linen => woven(0.17, 12_000.0, 9_000.0, 300.0, 1.4e-5, 0.0005),
            FabricPreset::Chambray => woven(0.14, 9_000.0, 6_500.0, 220.0, 8e-6, 0.0004),
            FabricPreset::Denim => woven(0.40, 25_000.0, 15_000.0, 600.0, 8e-5, 0.0010),
            FabricPreset::Twill => woven(0.25, 16_000.0, 10_000.0, 400.0, 3e-5, 0.0007),
            FabricPreset::Crepe => woven(0.13, 3_500.0, 3_000.0, 80.0, 2.5e-6, 0.0004),
            FabricPreset::Satin => woven(0.11, 5_000.0, 3_500.0, 90.0, 2e-6, 0.0003),
            FabricPreset::Jersey => knit(0.18, 400.0, 180.0, 60.0, 2.5e-6, 0.0006),
            FabricPreset::RibKnit => knit(0.25, 250.0, 80.0, 40.0, 5e-6, 0.0010),
            FabricPreset::WoolSuiting => woven(0.26, 14_000.0, 10_000.0, 350.0, 4e-5, 0.0008),
            FabricPreset::Fleece => knit(0.28, 900.0, 600.0, 100.0, 3e-5, 0.0025),
        }
    }
}

/// What was measured of a fabric at home. What was not is left out, and
/// taken from the preset it is like.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Measured {
    /// Its weight, g/m²: a 10 cm square's, in grams, times 100.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub weight: Option<f64>,
    /// How much longer 10 cm of a strip 5 cm wide, cut along the grain,
    /// grows with 500 g hanging from it, %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub lengthwise: Option<f64>,
    /// The same of a strip cut across the grain, %.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub crosswise: Option<f64>,
    /// How far a strip 2.5 cm wide, cut along the grain, slides out over a
    /// table's edge before its end droops to 45° below it, cm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub overhang: Option<f64>,
}

/// The pull on the stretch test's strip, N/m: 500 g hanging from it, 5 cm
/// wide.
const PULL: f64 = 0.5 * 9.81 / 0.05;
/// Measured to stretch less than this, %, a fabric is taken to stretch
/// this much: a ruler cannot tell less.
const LEAST_STRETCH: f64 = 0.5;
/// A strip's bending length, for each length of it out over the edge as
/// its end droops to 45° (Peirce's cantilever: (cos 22.5° / 8 tan 45°)^⅓).
const BENDING_LENGTH: f64 = 0.4869;
/// What can be measured: least and most weight (g/m²), stretch (%) and
/// overhang (cm).
pub const WEIGHTS: (f64, f64) = (10.0, 2_000.0);
pub const STRETCHES: (f64, f64) = (0.0, 300.0);
pub const OVERHANGS: (f64, f64) = (0.5, 60.0);

impl Measured {
    /// Fabric `like`, with what was measured in place of its own.
    pub fn fabric(&self, like: FabricPreset) -> Fabric {
        let base = like.fabric();
        let density = self.weight.map_or(f64::from(base.density), |w| w / 1000.0);
        let stiffness = |stretch: Option<f64>, preset: f32| {
            stretch.map_or(f64::from(preset), |s| PULL / (s.max(LEAST_STRETCH) / 100.0))
        };
        let (warp, weft) = (
            stiffness(self.lengthwise, base.warp),
            stiffness(self.crosswise, base.weft),
        );
        // As loose on the bias, for how firm it is along and across, as
        // the fabric it is like.
        let shear = f64::from(base.shear)
            * (warp / f64::from(base.warp) * weft / f64::from(base.weft)).sqrt();
        // Its own weight bending it down as it slides out over the edge.
        let bending = self.overhang.map_or(f64::from(base.bending), |cm| {
            density * 9.81 * (cm / 100.0 * BENDING_LENGTH).powi(3)
        });
        Fabric {
            density: density as f32,
            warp: warp as f32,
            weft: weft as f32,
            shear: shear as f32,
            bending: bending as f32,
            ..base
        }
    }

    /// What is wrong with what was measured, if anything.
    pub fn problem(&self) -> Option<String> {
        let out = |v: Option<f64>, (least, most): (f64, f64)| {
            v.is_some_and(|v| !v.is_finite() || v < least || v > most)
        };
        if out(self.weight, WEIGHTS) {
            Some(format!(
                "A fabric weighs between {} and {} g/m² (a 10 cm square between {} and {} g).",
                WEIGHTS.0,
                WEIGHTS.1,
                WEIGHTS.0 / 100.0,
                WEIGHTS.1 / 100.0
            ))
        } else if out(self.lengthwise, STRETCHES) || out(self.crosswise, STRETCHES) {
            Some(format!(
                "A strip stretches between {} and {}% (10 cm grows to at most {} cm).",
                STRETCHES.0,
                STRETCHES.1,
                10.0 + STRETCHES.1 / 10.0
            ))
        } else if out(self.overhang, OVERHANGS) {
            Some(format!(
                "A strip slides out between {} and {} cm before it droops.",
                OVERHANGS.0, OVERHANGS.1
            ))
        } else {
            None
        }
    }
}

/// A fabric of one's own: like a preset, with what was measured of it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OwnFabric {
    pub id: String,
    pub name: String,
    /// The preset it is most like: what was not measured is taken from it.
    pub like: FabricPreset,
    #[serde(default)]
    pub measured: Measured,
}

impl OwnFabric {
    /// What it is like.
    pub fn fabric(&self) -> Fabric {
        self.measured.fabric(self.like)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wovens_give_on_the_bias_and_presets_are_named() {
        let poplin = FabricPreset::Poplin.fabric();
        let along = poplin.stiffness_at(0.0);
        let across = poplin.stiffness_at(std::f32::consts::FRAC_PI_2);
        let bias = poplin.stiffness_at(std::f32::consts::FRAC_PI_4);
        assert!((along - poplin.warp).abs() < 1.0);
        assert!((across - poplin.weft).abs() < 1.0);
        assert!(bias < across / 5.0, "{bias}");
        for p in FabricPreset::ALL {
            assert_eq!(FabricPreset::named(p.name()), Some(p));
        }
        assert!(FabricPreset::named("tweed").is_none());
    }

    #[test]
    fn nothing_measured_is_the_preset() {
        for p in FabricPreset::ALL {
            assert_eq!(Measured::default().fabric(p), p.fabric());
        }
    }

    #[test]
    fn a_knit_measured_at_home() {
        // A 10 cm square weighing 2.2 g; 10 cm of a strip growing to 13 cm
        // lengthwise and 16 cm crosswise under 500 g; a strip sliding out
        // 4 cm before it droops to 45°.
        let measured = Measured {
            weight: Some(220.0),
            lengthwise: Some(30.0),
            crosswise: Some(60.0),
            overhang: Some(4.0),
        };
        let f = measured.fabric(FabricPreset::Jersey);
        assert!((f.density - 0.22).abs() < 1e-6);
        // 98.1 N/m pulling: 30% stretch is 327 N/m, 60% half that.
        assert!((f.warp - 327.0).abs() < 1.0, "{}", f.warp);
        assert!((f.weft - 163.5).abs() < 1.0, "{}", f.weft);
        // Bending length 1.95 cm: 0.22 × 9.81 × 0.0195³ N·m.
        let c = 0.04 * 0.4869;
        assert!((f64::from(f.bending) - 0.22 * 9.81 * c * c * c).abs() < 1e-9);
        // Firmer along than across, as measured, and looser on the bias.
        assert!(f.stiffness_at(std::f32::consts::FRAC_PI_4) < f.weft);
        assert_eq!(measured.problem(), None);
    }

    #[test]
    fn a_woven_that_barely_stretches_is_firm_and_its_preset_bends_as_measured() {
        let measured = Measured {
            lengthwise: Some(0.0),
            ..Default::default()
        };
        let f = measured.fabric(FabricPreset::Linen);
        assert!((f.warp - 19_620.0).abs() < 1.0, "{}", f.warp);
        assert!((f.weft - FabricPreset::Linen.fabric().weft).abs() < 1e-3);
        // A heavier preset bends more for the same overhang: its weight.
        let hang = |like: FabricPreset| {
            Measured {
                overhang: Some(5.0),
                ..Default::default()
            }
            .fabric(like)
            .bending
        };
        assert!(hang(FabricPreset::Denim) > hang(FabricPreset::Poplin));
    }

    #[test]
    fn what_cannot_be_is_refused() {
        let bad = |m: Measured| m.problem().is_some();
        assert!(bad(Measured {
            weight: Some(5.0),
            ..Default::default()
        }));
        assert!(bad(Measured {
            crosswise: Some(-3.0),
            ..Default::default()
        }));
        assert!(bad(Measured {
            overhang: Some(f64::NAN),
            ..Default::default()
        }));
        assert!(!bad(Measured::default()));
    }

    #[test]
    fn written_without_what_was_not_measured() {
        let fabric = OwnFabric {
            id: "f1".into(),
            name: "Blue linen".into(),
            like: FabricPreset::Linen,
            measured: Measured {
                weight: Some(185.0),
                ..Default::default()
            },
        };
        let json = serde_json::to_string(&fabric).unwrap();
        assert_eq!(
            json,
            r#"{"id":"f1","name":"Blue linen","like":"linen","measured":{"weight":185.0}}"#
        );
        assert_eq!(serde_json::from_str::<OwnFabric>(&json).unwrap(), fabric);
    }
}
