//! Fabrics: the presets, and fabrics of one's own measured at home (the
//! `drapewright-fabric` crate), by name for the tests and examples.

pub use drapewright_fabric::{Fabric, FabricPreset, Measured, OwnFabric};

/// A preset by name.
pub fn preset(name: &str) -> Option<Fabric> {
    FabricPreset::named(name).map(FabricPreset::fabric)
}
