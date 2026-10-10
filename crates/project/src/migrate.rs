//! Reading `project.json` of any version this build understands.

use serde_json::Value;
use thiserror::Error;

use crate::document::{FORMAT, ProjectFile, SCHEMA_VERSION};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum LoadError {
    #[error("This file is damaged or is not a Soom project ({0}).")]
    NotJson(String),
    #[error("This file is not a Soom project.")]
    NotSoom,
    #[error(
        "This project was made with a newer version of Soom (format {found}). Please update Soom to open it."
    )]
    TooNew { found: u64 },
    #[error("This project could not be read: {0}")]
    Invalid(String),
}

/// One step: turns a document of version `n` into version `n + 1`.
type Migration = fn(&mut Value) -> Result<(), String>;

/// `MIGRATIONS[i]` upgrades version `i + 1` to `i + 2`. Version 1 is the
/// first, so there are none yet.
const MIGRATIONS: &[Migration] = &[];

pub fn load(json: &str) -> Result<ProjectFile, LoadError> {
    let mut value: Value =
        serde_json::from_str(json).map_err(|e| LoadError::NotJson(e.to_string()))?;
    if value.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err(LoadError::NotSoom);
    }
    let version = value
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .ok_or(LoadError::NotSoom)?;
    if version == 0 {
        return Err(LoadError::NotSoom);
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
