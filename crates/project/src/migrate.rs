//! Reading `project.json` of any version this build understands.

use serde_json::Value;
use thiserror::Error;

use crate::document::{FORMAT, ProjectFile, SCHEMA_VERSION};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LoadError {
    #[error("This file is damaged or is not a Drapewright project ({0}).")]
    NotJson(String),
    #[error("This file is not a Drapewright project.")]
    NotAProject,
    #[error(
        "This project was made with a newer version of Drapewright (format {found}). Please update Drapewright to open it."
    )]
    TooNew { found: u64 },
    #[error("This project could not be read: {0}")]
    Invalid(String),
}

/// The format's name before the app was renamed Drapewright (it was called
/// Soom until October 2026). Those projects are otherwise the same, and are
/// saved under the new name once opened.
const EARLIER_FORMAT: &str = "soom-project";

/// One step: turns a document of version `n` into version `n + 1`.
type Migration = fn(&mut Value) -> Result<(), String>;

/// `MIGRATIONS[i]` upgrades version `i + 1` to `i + 2`.
const MIGRATIONS: &[Migration] = &[v1_to_v2, v2_to_v3];

/// Version 2 adds people's measurements: none yet.
fn v1_to_v2(value: &mut Value) -> Result<(), String> {
    let object = value
        .as_object_mut()
        .ok_or("the project is not an object")?;
    object
        .entry("measurementSets")
        .or_insert_with(|| Value::Array(Vec::new()));
    Ok(())
}

/// Version 3 adds seams: none yet.
fn v2_to_v3(value: &mut Value) -> Result<(), String> {
    let object = value
        .as_object_mut()
        .ok_or("the project is not an object")?;
    object
        .entry("seams")
        .or_insert_with(|| Value::Array(Vec::new()));
    Ok(())
}

pub fn load(json: &str) -> Result<ProjectFile, LoadError> {
    let mut value: Value =
        serde_json::from_str(json).map_err(|e| LoadError::NotJson(e.to_string()))?;
    match value.get("format").and_then(Value::as_str) {
        Some(FORMAT) => {}
        Some(EARLIER_FORMAT) => value["format"] = Value::from(FORMAT),
        _ => return Err(LoadError::NotAProject),
    }
    let version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or(LoadError::NotAProject)?;
    if version == 0 {
        return Err(LoadError::NotAProject);
    }
    if version > u64::from(SCHEMA_VERSION) {
        return Err(LoadError::TooNew { found: version });
    }
    for step in &MIGRATIONS[(version - 1) as usize..] {
        step(&mut value).map_err(LoadError::Invalid)?;
    }
    value["schemaVersion"] = Value::from(SCHEMA_VERSION);
    serde_json::from_value(value).map_err(|e| LoadError::Invalid(e.to_string()))
}
