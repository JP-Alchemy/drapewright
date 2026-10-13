//! Draping patterns on the avatar: the pieces meshed, placed round the
//! body, sewn along their seams, and settled as cloth.

pub mod cloth;
pub mod collide;
pub mod drape;
pub mod fabric;
pub mod garment;
pub mod girth;
pub mod mesh;
pub mod place;

pub use cloth::{Cloth, ClothBuilder, Collider};
pub use fabric::Fabric;
