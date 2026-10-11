//! Fabrics: how heavy, how stiff along the grain and across it, how loose
//! on the bias, how they bend. Starting points for the presets in the plan;
//! step 29 calibrates a real fabric.

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

/// The presets, by name.
pub const PRESETS: &[(&str, Fabric)] = &[
    (
        "cottonLawn",
        woven(0.07, 6_000.0, 4_000.0, 120.0, 2e-6, 0.0003),
    ),
    ("poplin", woven(0.12, 9_000.0, 6_000.0, 200.0, 6e-6, 0.0004)),
    (
        "linen",
        woven(0.17, 12_000.0, 9_000.0, 300.0, 1.4e-5, 0.0005),
    ),
    (
        "chambray",
        woven(0.14, 9_000.0, 6_500.0, 220.0, 8e-6, 0.0004),
    ),
    (
        "denim",
        woven(0.40, 25_000.0, 15_000.0, 600.0, 8e-5, 0.0010),
    ),
    (
        "twill",
        woven(0.25, 16_000.0, 10_000.0, 400.0, 3e-5, 0.0007),
    ),
    ("crepe", woven(0.13, 3_500.0, 3_000.0, 80.0, 2.5e-6, 0.0004)),
    ("satin", woven(0.11, 5_000.0, 3_500.0, 90.0, 2e-6, 0.0003)),
    ("jersey", knit(0.18, 400.0, 180.0, 60.0, 2.5e-6, 0.0006)),
    ("ribKnit", knit(0.25, 250.0, 80.0, 40.0, 5e-6, 0.0010)),
    (
        "woolSuiting",
        woven(0.26, 14_000.0, 10_000.0, 350.0, 4e-5, 0.0008),
    ),
    ("fleece", knit(0.28, 900.0, 600.0, 100.0, 3e-5, 0.0025)),
];

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

/// A preset by name.
pub fn preset(name: &str) -> Option<Fabric> {
    PRESETS.iter().find(|(n, _)| *n == name).map(|(_, f)| *f)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wovens_give_on_the_bias_and_presets_are_found() {
        let poplin = preset("poplin").unwrap();
        let along = poplin.stiffness_at(0.0);
        let across = poplin.stiffness_at(std::f32::consts::FRAC_PI_2);
        let bias = poplin.stiffness_at(std::f32::consts::FRAC_PI_4);
        assert!((along - poplin.warp).abs() < 1.0);
        assert!((across - poplin.weft).abs() < 1.0);
        assert!(bias < across / 5.0, "{bias}");
        assert!(preset("tweed").is_none());
        assert_eq!(PRESETS.len(), 12);
    }
}
