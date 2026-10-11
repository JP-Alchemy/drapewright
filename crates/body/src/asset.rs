//! The body file: everything needed to shape the avatar, written once by
//! the `build_body` example from MakeHuman's data and read by the app.
//!
//! After an 8-byte signature the file is deflate-compressed, little-endian
//! data: the base mesh (mm, y up, z to the front), which body region each
//! vertex is in, the joint markers, the macro shapes as principal
//! components with each shape's recipe, and the local shapes, sparse.

use std::io::{Read, Write};

use flate2::Compression;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use thiserror::Error;

const SIGNATURE: &[u8; 8] = b"DWBODY\x001";

/// Local shapes' offsets are stored in hundredths of a millimetre.
pub const LOCAL_UNIT: f32 = 0.01;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("This is not a Drapewright body file.")]
    NotABody,
    #[error("The body file is damaged ({0}).")]
    Damaged(String),
}

/// Where a vertex is on the body (from the bone that moves it most).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Region {
    Head = 0,
    Neck = 1,
    Torso = 2,
    /// The deltoid: between the torso and the arm.
    ShoulderLeft = 3,
    ShoulderRight = 4,
    ArmLeft = 5,
    ArmRight = 6,
    HandLeft = 7,
    HandRight = 8,
    LegLeft = 9,
    LegRight = 10,
    FootLeft = 11,
    FootRight = 12,
    /// Not drawn: joint markers.
    Joint = 13,
}

impl Region {
    pub fn from_u8(v: u8) -> Option<Self> {
        use Region::*;
        Some(match v {
            0 => Head,
            1 => Neck,
            2 => Torso,
            3 => ShoulderLeft,
            4 => ShoulderRight,
            5 => ArmLeft,
            6 => ArmRight,
            7 => HandLeft,
            8 => HandRight,
            9 => LegLeft,
            10 => LegRight,
            11 => FootLeft,
            12 => FootRight,
            13 => Joint,
            _ => return None,
        })
    }
}

/// A joint marker: 8 vertices from `first`, moved by the shapes like the
/// body around them; their middle is the joint.
#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub name: String,
    pub first: u32,
}

/// One principal component of the macro shapes: offsets for every vertex
/// (x, y, z in turn), stored as `values × scale` mm.
#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub scale: f32,
    pub values: Vec<i16>,
}

/// A macro shape: which phenotype variations weigh it (feature, variation),
/// and its make-up in principal components.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroShape {
    pub factors: Vec<(u8, u8)>,
    pub coefficients: Vec<f32>,
}

/// Sparse offsets: vertices and their offsets in `LOCAL_UNIT`s.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sparse {
    pub vertices: Vec<u32>,
    pub offsets: Vec<[i16; 3]>,
}

/// A local shape, such as MakeHuman's "measure-waist-circ": applied with a
/// value from −1 (`decrease`) to 1 (`increase`).
#[derive(Clone, Debug, PartialEq)]
pub struct LocalShape {
    pub name: String,
    pub increase: Sparse,
    pub decrease: Sparse,
}

/// The whole body file.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyAsset {
    /// Where the data comes from and its licences.
    pub notice: String,
    /// Base mesh, mm.
    pub base: Vec<[f32; 3]>,
    pub triangles: Vec<[u32; 3]>,
    /// Vertices `0..shown` are drawn; the rest are joint markers.
    pub shown: u32,
    pub regions: Vec<u8>,
    pub joints: Vec<Joint>,
    /// The mean figure's offsets from the base mesh, mm (x, y, z in turn),
    /// and its part in each component.
    pub mean: Vec<f32>,
    pub mean_mix: Vec<f32>,
    pub components: Vec<Component>,
    pub macros: Vec<MacroShape>,
    pub locals: Vec<LocalShape>,
}

struct Writer(Vec<u8>);

impl Writer {
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn len(&mut self, n: usize) {
        self.u32(u32::try_from(n).expect("fits in a u32"));
    }
    fn text(&mut self, s: &str) {
        self.len(s.len());
        self.0.extend_from_slice(s.as_bytes());
    }
    fn sparse(&mut self, s: &Sparse) {
        self.len(s.vertices.len());
        for (v, o) in s.vertices.iter().zip(&s.offsets) {
            self.u32(*v);
            o.iter().for_each(|x| self.i16(*x));
        }
    }
}

struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], AssetError> {
        let end = self.at.checked_add(n).filter(|e| *e <= self.data.len());
        let end = end.ok_or_else(|| AssetError::Damaged("it ends early".into()))?;
        let bytes = &self.data[self.at..end];
        self.at = end;
        Ok(bytes)
    }
    fn u32(&mut self) -> Result<u32, AssetError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }
    fn f32(&mut self) -> Result<f32, AssetError> {
        Ok(f32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }
    fn i16(&mut self) -> Result<i16, AssetError> {
        Ok(i16::from_le_bytes(
            self.take(2)?.try_into().expect("2 bytes"),
        ))
    }
    /// A count, sanity-checked against what is left to read.
    fn len(&mut self, min_item: usize) -> Result<usize, AssetError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min_item.max(1)) > self.data.len() - self.at {
            return Err(AssetError::Damaged("a count is too large".into()));
        }
        Ok(n)
    }
    fn text(&mut self) -> Result<String, AssetError> {
        let n = self.len(1)?;
        String::from_utf8(self.take(n)?.to_vec())
            .map_err(|_| AssetError::Damaged("text is not UTF-8".into()))
    }
    fn sparse(&mut self) -> Result<Sparse, AssetError> {
        let n = self.len(10)?;
        let mut s = Sparse::default();
        for _ in 0..n {
            s.vertices.push(self.u32()?);
            s.offsets.push([self.i16()?, self.i16()?, self.i16()?]);
        }
        Ok(s)
    }
}

impl BodyAsset {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut w = Writer(Vec::new());
        w.text(&self.notice);
        w.len(self.base.len());
        self.base.iter().flatten().for_each(|v| w.f32(*v));
        w.len(self.triangles.len());
        self.triangles.iter().flatten().for_each(|v| w.u32(*v));
        w.u32(self.shown);
        w.len(self.regions.len());
        w.0.extend_from_slice(&self.regions);
        w.len(self.joints.len());
        for j in &self.joints {
            w.text(&j.name);
            w.u32(j.first);
        }
        w.len(self.mean.len());
        self.mean.iter().for_each(|v| w.f32(*v));
        w.len(self.mean_mix.len());
        self.mean_mix.iter().for_each(|v| w.f32(*v));
        w.len(self.components.len());
        for c in &self.components {
            w.f32(c.scale);
            w.len(c.values.len());
            // Low bytes, then high bytes: smooth values compress far better.
            let bytes: Vec<[u8; 2]> = c.values.iter().map(|v| v.to_le_bytes()).collect();
            w.0.extend(bytes.iter().map(|b| b[0]));
            w.0.extend(bytes.iter().map(|b| b[1]));
        }
        w.len(self.macros.len());
        for m in &self.macros {
            w.len(m.factors.len());
            for (f, v) in &m.factors {
                w.0.push(*f);
                w.0.push(*v);
            }
            w.len(m.coefficients.len());
            m.coefficients.iter().for_each(|c| w.f32(*c));
        }
        w.len(self.locals.len());
        for l in &self.locals {
            w.text(&l.name);
            w.sparse(&l.increase);
            w.sparse(&l.decrease);
        }
        let mut out = SIGNATURE.to_vec();
        let mut z = DeflateEncoder::new(&mut out, Compression::best());
        z.write_all(&w.0).expect("writing to memory");
        z.finish().expect("writing to memory");
        out
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, AssetError> {
        let body = bytes.strip_prefix(SIGNATURE).ok_or(AssetError::NotABody)?;
        let mut data = Vec::new();
        DeflateDecoder::new(body)
            .read_to_end(&mut data)
            .map_err(|e| AssetError::Damaged(e.to_string()))?;
        let mut r = Reader { data: &data, at: 0 };
        let notice = r.text()?;
        let n = r.len(12)?;
        let mut base = Vec::with_capacity(n);
        for _ in 0..n {
            base.push([r.f32()?, r.f32()?, r.f32()?]);
        }
        let t = r.len(12)?;
        let mut triangles = Vec::with_capacity(t);
        for _ in 0..t {
            let tri = [r.u32()?, r.u32()?, r.u32()?];
            if tri.iter().any(|&v| v as usize >= n) {
                return Err(AssetError::Damaged(
                    "a triangle's corner is not a vertex".into(),
                ));
            }
            triangles.push(tri);
        }
        let shown = r.u32()?;
        let k = r.len(1)?;
        let regions = r.take(k)?.to_vec();
        let joints = (0..r.len(8)?)
            .map(|_| {
                Ok(Joint {
                    name: r.text()?,
                    first: r.u32()?,
                })
            })
            .collect::<Result<Vec<_>, AssetError>>()?;
        let m = r.len(4)?;
        let mean = (0..m).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
        let m = r.len(4)?;
        let mean_mix = (0..m).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
        let components = (0..r.len(8)?)
            .map(|_| {
                let scale = r.f32()?;
                let m = r.len(2)?;
                let low = r.take(m)?.to_vec();
                let high = r.take(m)?;
                let values = low
                    .iter()
                    .zip(high)
                    .map(|(&l, &h)| i16::from_le_bytes([l, h]))
                    .collect();
                Ok(Component { scale, values })
            })
            .collect::<Result<Vec<_>, AssetError>>()?;
        let macros = (0..r.len(8)?)
            .map(|_| {
                let f = r.len(2)?;
                let factors = (0..f)
                    .map(|_| {
                        let b = r.take(2)?;
                        Ok((b[0], b[1]))
                    })
                    .collect::<Result<Vec<_>, AssetError>>()?;
                let c = r.len(4)?;
                let coefficients = (0..c).map(|_| r.f32()).collect::<Result<Vec<_>, _>>()?;
                Ok(MacroShape {
                    factors,
                    coefficients,
                })
            })
            .collect::<Result<Vec<_>, AssetError>>()?;
        let locals = (0..r.len(12)?)
            .map(|_| {
                Ok(LocalShape {
                    name: r.text()?,
                    increase: r.sparse()?,
                    decrease: r.sparse()?,
                })
            })
            .collect::<Result<Vec<_>, AssetError>>()?;
        let asset = BodyAsset {
            notice,
            base,
            triangles,
            shown,
            regions,
            joints,
            mean,
            mean_mix,
            components,
            macros,
            locals,
        };
        asset.check()?;
        Ok(asset)
    }

    /// The parts agree with each other.
    fn check(&self) -> Result<(), AssetError> {
        let n = self.base.len();
        let bad = |what: &str| Err(AssetError::Damaged(what.into()));
        if self.regions.len() != n || self.shown as usize > n {
            return bad("regions do not match the vertices");
        }
        if self.mean.len() != 3 * n || self.mean_mix.len() != self.components.len() {
            return bad("the mean figure does not match the vertices");
        }
        if self.components.iter().any(|c| c.values.len() != 3 * n) {
            return bad("a component does not match the vertices");
        }
        let k = self.components.len();
        if self.macros.iter().any(|m| m.coefficients.len() != k) {
            return bad("a macro shape does not match the components");
        }
        let in_mesh = |s: &Sparse| s.vertices.iter().all(|&v| (v as usize) < n);
        if self
            .locals
            .iter()
            .any(|l| !in_mesh(&l.increase) || !in_mesh(&l.decrease))
        {
            return bad("a local shape moves vertices that are not there");
        }
        if self.joints.iter().any(|j| j.first as usize + 8 > n) {
            return bad("a joint marker is not in the mesh");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_body_file_reads_back_as_written() {
        let asset = BodyAsset {
            notice: "test".into(),
            base: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            triangles: vec![[0, 1, 2]],
            shown: 3,
            regions: vec![2, 2, 2],
            joints: Vec::new(),
            mean: vec![0.0; 9],
            mean_mix: vec![1.5],
            components: vec![Component {
                scale: 0.5,
                values: vec![1, 2, 3, 4, 5, 6, 7, 8, 9],
            }],
            macros: vec![MacroShape {
                factors: vec![(0, 1), (3, 2)],
                coefficients: vec![2.5],
            }],
            locals: vec![LocalShape {
                name: "measure-waist-circ".into(),
                increase: Sparse {
                    vertices: vec![1],
                    offsets: vec![[100, -100, 0]],
                },
                decrease: Sparse::default(),
            }],
        };
        let bytes = asset.to_bytes();
        assert_eq!(BodyAsset::from_bytes(&bytes).unwrap(), asset);
        assert!(matches!(
            BodyAsset::from_bytes(b"nope"),
            Err(AssetError::NotABody)
        ));
        let mut cut = bytes.clone();
        cut.truncate(cut.len() - 5);
        assert!(BodyAsset::from_bytes(&cut).is_err());
    }
}
