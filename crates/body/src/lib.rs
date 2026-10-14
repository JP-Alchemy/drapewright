//! The avatar: a body shaped from MakeHuman's data (CC0, as prepared in
//! Anny, Apache-2.0), measured as a tape measure would, and fitted to a
//! person's measurements. Millimetres, y up, z to the front, the figure's
//! left towards +x.

pub mod asset;
pub mod fit;
pub mod measure;
pub mod pose;
pub mod shape;

pub use asset::{AssetError, BodyAsset, Region};
pub use fit::{Fit, fit};
pub use measure::{Landmarks, measure};
pub use pose::{Pose, Rig, Stance};
pub use shape::{Body, BodyModel, Shape};
