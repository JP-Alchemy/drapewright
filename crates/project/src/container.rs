//! The `.soom` file: a zip archive with `project.json` and, under
//! `sources/`, the files the pieces were imported from
//! (docs/file-format.md §1).
//!
//! Packing is deterministic: entries are written in a fixed order with a
//! fixed date, so the same project always gives the same bytes.

use std::io::{Cursor, Read, Write};

use thiserror::Error;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

/// The project document inside the archive.
pub const PROJECT_JSON: &str = "project.json";
/// Folder of original imported files: `sources/<sha256>-<file name>`.
pub const SOURCES: &str = "sources/";

/// Most a `.soom` may unpack to, all files together. A larger archive is
/// refused rather than allowed to fill the device.
const MAX_UNPACKED: u64 = 1 << 30;
/// Most files a `.soom` may hold.
const MAX_FILES: usize = 10_000;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ContainerError {
    #[error("This is not a Soom project file (.soom).")]
    NotSoom,
    #[error("This project file has no project in it.")]
    NoProject,
    #[error("This project file is damaged: {0}")]
    Damaged(String),
    #[error("This project file is too large to open.")]
    TooLarge,
}

/// A file kept beside the project, such as an original imported pattern.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sidecar {
    /// Path inside the archive, e.g. `sources/3fa9…-skirt.dxf`.
    pub path: String,
    pub data: Vec<u8>,
}

/// A file name that is safe as one entry under `sources/`: no folders, no
/// "..", nothing hidden.
pub fn safe_source_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.contains(['/', '\\', '\0'])
}

/// Packs `project.json` and its sidecars into a `.soom` file. Sidecars
/// outside `sources/`, or with unsafe names, are refused.
pub fn pack(project_json: &str, sidecars: &[Sidecar]) -> Result<Vec<u8>, ContainerError> {
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(DateTime::DEFAULT);
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let damaged = |e: &dyn std::fmt::Display| ContainerError::Damaged(e.to_string());
    zip.start_file(PROJECT_JSON, options)
        .map_err(|e| damaged(&e))?;
    zip.write_all(project_json.as_bytes())
        .map_err(|e| damaged(&e))?;
    let mut sorted: Vec<&Sidecar> = sidecars.iter().collect();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    sorted.dedup_by(|a, b| a.path == b.path);
    for sidecar in sorted {
        let name = sidecar
            .path
            .strip_prefix(SOURCES)
            .filter(|n| safe_source_name(n))
            .ok_or_else(|| {
                ContainerError::Damaged(format!("bad sidecar path {:?}", sidecar.path))
            })?;
        zip.start_file(format!("{SOURCES}{name}"), options)
            .map_err(|e| damaged(&e))?;
        zip.write_all(&sidecar.data).map_err(|e| damaged(&e))?;
    }
    let cursor = zip.finish().map_err(|e| damaged(&e))?;
    Ok(cursor.into_inner())
}

/// What a `.soom` file holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unpacked {
    /// The `project.json` text, to be opened (and migrated) by the engine.
    pub project_json: String,
    /// Files under `sources/`. Anything else in the archive is ignored, as
    /// the format asks of readers.
    pub sidecars: Vec<Sidecar>,
}

/// Reads a `.soom` file.
pub fn unpack(bytes: &[u8]) -> Result<Unpacked, ContainerError> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|_| ContainerError::NotSoom)?;
    if zip.len() > MAX_FILES {
        return Err(ContainerError::TooLarge);
    }
    let mut budget = MAX_UNPACKED;
    let mut project_json = None;
    let mut sidecars = Vec::new();
    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|e| ContainerError::Damaged(e.to_string()))?;
        if file.is_dir() {
            continue;
        }
        let name = file
            .name()
            .map_err(|e| ContainerError::Damaged(e.to_string()))?
            .to_string();
        let wanted =
            name == PROJECT_JSON || name.strip_prefix(SOURCES).is_some_and(safe_source_name);
        if !wanted {
            continue;
        }
        // Read at most what is left of the budget, whatever the entry claims.
        let mut data = Vec::new();
        let read = file
            .by_ref()
            .take(budget + 1)
            .read_to_end(&mut data)
            .map_err(|e| ContainerError::Damaged(e.to_string()))?;
        if read as u64 > budget {
            return Err(ContainerError::TooLarge);
        }
        budget -= read as u64;
        if name == PROJECT_JSON {
            let text = String::from_utf8(data)
                .map_err(|_| ContainerError::Damaged("project.json is not UTF-8 text".into()))?;
            project_json = Some(text);
        } else {
            sidecars.push(Sidecar { path: name, data });
        }
    }
    Ok(Unpacked {
        project_json: project_json.ok_or(ContainerError::NoProject)?,
        sidecars,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sidecar(path: &str, data: &[u8]) -> Sidecar {
        Sidecar {
            path: path.into(),
            data: data.to_vec(),
        }
    }

    #[test]
    fn packs_and_unpacks_the_project_and_its_sources() {
        let json = r#"{"format":"soom-project","schemaVersion":1}"#;
        let sources = [
            sidecar("sources/b2-skirt.dxf", b"0\nSECTION\n"),
            sidecar("sources/a1-front.svg", b"<svg/>"),
        ];
        let bytes = pack(json, &sources).unwrap();
        let back = unpack(&bytes).unwrap();
        assert_eq!(back.project_json, json);
        // Sorted by path.
        assert_eq!(back.sidecars, [sources[1].clone(), sources[0].clone()]);
        // The same project always gives the same bytes.
        assert_eq!(pack(json, &sources).unwrap(), bytes);
    }

    #[test]
    fn refuses_what_is_not_a_project() {
        assert_eq!(unpack(b"not a zip"), Err(ContainerError::NotSoom));
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        zip.start_file("readme.txt", SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"hello").unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        assert_eq!(unpack(&bytes), Err(ContainerError::NoProject));
    }

    #[test]
    fn ignores_unknown_files_and_unsafe_names() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in [
            ("project.json", &b"{}"[..]),
            ("thumbnails/cover.png", b"png"),
            ("sources/../../escape.txt", b"x"),
            ("sources/ok.svg", b"<svg/>"),
        ] {
            zip.start_file(name, SimpleFileOptions::default()).unwrap();
            zip.write_all(data).unwrap();
        }
        let bytes = zip.finish().unwrap().into_inner();
        let back = unpack(&bytes).unwrap();
        assert_eq!(back.sidecars, [sidecar("sources/ok.svg", b"<svg/>")]);
        assert!(pack("{}", &[sidecar("sources/../x", b"")]).is_err());
        assert!(pack("{}", &[sidecar("elsewhere/x", b"")]).is_err());
    }
}
