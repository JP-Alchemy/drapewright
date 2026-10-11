//! Shaping the avatar. A figure's settings (woman's or man's, age, muscle,
//! weight, height, proportions, cup size, firmness) weigh MakeHuman's macro
//! shapes as Anny does: each shape's weight is the product of how much of
//! each of its variations the settings ask for. Local shapes (MakeHuman's
//! "measure" shapes among them) then change single measurements.

use std::collections::BTreeMap;

use crate::asset::{AssetError, BodyAsset, LOCAL_UNIT, Region};

/// The phenotype features, in the order the body file numbers them.
pub const GENDER: u8 = 0;
pub const AGE: u8 = 1;
pub const MUSCLE: u8 = 2;
pub const WEIGHT: u8 = 3;
pub const HEIGHT: u8 = 4;
pub const PROPORTIONS: u8 = 5;
pub const CUP: u8 = 6;
pub const FIRMNESS: u8 = 7;

/// Each feature's variations, in the order the body file numbers them.
pub const VARIATIONS: [&[&str]; 8] = [
    &["male", "female"],
    &["young", "old"],
    &["minmuscle", "averagemuscle", "maxmuscle"],
    &["minweight", "averageweight", "maxweight"],
    &["minheight", "maxheight"],
    &["idealproportions", "uncommonproportions"],
    &["mincup", "averagecup", "maxcup"],
    &["minfirmness", "averagefirmness", "maxfirmness"],
];

/// MakeHuman's "young" and "old" are these ages, in years.
pub const YOUNG: f64 = 25.0;
pub const OLD: f64 = 90.0;

/// What shapes a figure. Each setting runs from 0 to 1 unless said.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// 1 for a woman's figure, 0 for a man's.
    pub woman: f64,
    /// Years, from 25 to 90.
    pub age: f64,
    pub muscle: f64,
    pub weight: f64,
    pub height: f64,
    pub proportions: f64,
    pub cup: f64,
    pub firmness: f64,
    /// Local shapes by name, each from −1 to 1.
    pub local: BTreeMap<String, f64>,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            woman: 1.0,
            age: 35.0,
            muscle: 0.5,
            weight: 0.5,
            height: 0.5,
            proportions: 0.5,
            cup: 0.5,
            firmness: 0.5,
            local: BTreeMap::new(),
        }
    }
}

/// How much of each anchor `value` is, between its two nearest anchors.
fn interpolate(value: f64, anchors: &[f64]) -> Vec<f64> {
    let v = value.clamp(anchors[0], anchors[anchors.len() - 1]);
    let mut out = vec![0.0; anchors.len()];
    for i in 0..anchors.len() - 1 {
        let (a, b) = (anchors[i], anchors[i + 1]);
        if v <= b || i == anchors.len() - 2 {
            let t = (v - a) / (b - a);
            out[i] = 1.0 - t;
            out[i + 1] = t;
            break;
        }
    }
    out
}

impl Shape {
    /// How much of each variation of each feature the settings ask for.
    pub fn variation_weights(&self) -> [Vec<f64>; 8] {
        let two = [0.0, 1.0];
        let three = [0.0, 0.5, 1.0];
        [
            interpolate(self.woman, &two),
            interpolate((self.age - YOUNG) / (OLD - YOUNG), &two),
            interpolate(self.muscle, &three),
            interpolate(self.weight, &three),
            interpolate(self.height, &two),
            interpolate(self.proportions, &two),
            interpolate(self.cup, &three),
            interpolate(self.firmness, &three),
        ]
    }
}

/// The body model, ready to shape figures.
#[derive(Debug)]
pub struct BodyModel {
    pub asset: BodyAsset,
    /// The principal components, in mm.
    components: Vec<Vec<f32>>,
    /// For each triangle, the regions of its corners, one bit each.
    corners: Vec<u16>,
}

/// A shaped figure, standing on the floor (y = 0), mm.
#[derive(Clone, Debug)]
pub struct Body {
    pub vertices: Vec<[f64; 3]>,
}

impl BodyModel {
    pub fn new(asset: BodyAsset) -> Self {
        let components = asset
            .components
            .iter()
            .map(|c| c.values.iter().map(|&v| f32::from(v) * c.scale).collect())
            .collect();
        // A corner in no region (none should be) takes a bit of its own.
        let bit = |v: u32| match Region::from_u8(asset.regions[v as usize]) {
            Some(r) => 1u16 << r as u16,
            None => 1 << 15,
        };
        let corners = asset
            .triangles
            .iter()
            .map(|t| bit(t[0]) | bit(t[1]) | bit(t[2]))
            .collect();
        BodyModel {
            asset,
            components,
            corners,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetError> {
        Ok(BodyModel::new(BodyAsset::from_bytes(bytes)?))
    }

    /// The local shapes' names.
    pub fn local_names(&self) -> impl Iterator<Item = &str> {
        self.asset.locals.iter().map(|l| l.name.as_str())
    }

    pub fn region(&self, vertex: usize) -> Option<Region> {
        Region::from_u8(self.asset.regions[vertex])
    }

    /// The triangles drawn: the skin, without the joint markers.
    pub fn skin(&self) -> Vec<[u32; 3]> {
        let shown = self.asset.shown;
        self.asset
            .triangles
            .iter()
            .filter(|t| t.iter().all(|&v| v < shown))
            .copied()
            .collect()
    }

    /// For each triangle, the regions of its corners, one bit each.
    pub(crate) fn corner_regions(&self) -> &[u16] {
        &self.corners
    }

    /// A figure shaped by `shape`.
    pub fn shape(&self, shape: &Shape) -> Body {
        let weights = shape.variation_weights();
        // The mean figure, and each component as far as this figure differs
        // from it.
        let mut mix: Vec<f64> = self.asset.mean_mix.iter().map(|m| -f64::from(*m)).collect();
        for m in &self.asset.macros {
            let w: f64 = m
                .factors
                .iter()
                .map(|&(f, v)| weights[f as usize][v as usize])
                .product();
            if w == 0.0 {
                continue;
            }
            for (c, coef) in mix.iter_mut().zip(&m.coefficients) {
                *c += w * f64::from(*coef);
            }
        }
        let mut vertices: Vec<[f64; 3]> = self
            .asset
            .base
            .iter()
            .zip(self.asset.mean.chunks_exact(3))
            .map(|(p, m)| {
                [
                    f64::from(p[0] + m[0]),
                    f64::from(p[1] + m[1]),
                    f64::from(p[2] + m[2]),
                ]
            })
            .collect();
        for (c, values) in mix.iter().zip(&self.components) {
            if *c == 0.0 {
                continue;
            }
            for (p, d) in vertices.iter_mut().zip(values.chunks_exact(3)) {
                p[0] += c * f64::from(d[0]);
                p[1] += c * f64::from(d[1]);
                p[2] += c * f64::from(d[2]);
            }
        }
        for local in &self.asset.locals {
            let value = shape
                .local
                .get(&local.name)
                .copied()
                .unwrap_or(0.0)
                .clamp(-1.0, 1.0);
            let (amount, offsets) = if value > 0.0 {
                (value, &local.increase)
            } else {
                (-value, &local.decrease)
            };
            if amount == 0.0 {
                continue;
            }
            let unit = amount * f64::from(LOCAL_UNIT);
            for (&v, o) in offsets.vertices.iter().zip(&offsets.offsets) {
                let p = &mut vertices[v as usize];
                p[0] += unit * f64::from(o[0]);
                p[1] += unit * f64::from(o[1]);
                p[2] += unit * f64::from(o[2]);
            }
        }
        // Standing on the floor.
        let floor = vertices[..self.asset.shown as usize]
            .iter()
            .map(|p| p[1])
            .fold(f64::MAX, f64::min);
        for p in &mut vertices {
            p[1] -= floor;
        }
        Body { vertices }
    }

    /// Where a joint is in a shaped figure: the middle of its marker.
    pub fn joint(&self, body: &Body, name: &str) -> Option<[f64; 3]> {
        let j = self.asset.joints.iter().find(|j| j.name == name)?;
        let first = j.first as usize;
        let mut sum = [0.0; 3];
        for p in &body.vertices[first..first + 8] {
            sum[0] += p[0];
            sum[1] += p[1];
            sum[2] += p[2];
        }
        Some([sum[0] / 8.0, sum[1] / 8.0, sum[2] / 8.0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_share_their_weight_between_neighbouring_variations() {
        assert_eq!(interpolate(0.25, &[0.0, 0.5, 1.0]), [0.5, 0.5, 0.0]);
        assert_eq!(interpolate(1.0, &[0.0, 0.5, 1.0]), [0.0, 0.0, 1.0]);
        assert_eq!(interpolate(-3.0, &[0.0, 1.0]), [1.0, 0.0]);
        let w = Shape {
            age: 57.5,
            ..Shape::default()
        }
        .variation_weights();
        assert_eq!(w[AGE as usize], [0.5, 0.5]);
        assert_eq!(w[GENDER as usize], [0.0, 1.0]);
    }
}
