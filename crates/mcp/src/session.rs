//! The project an assistant is working on, and the files it reads and
//! writes. One project is open at a time, as in the app.

use std::path::{Path, PathBuf};

use drapewright_pattern_core::Issue;
use drapewright_project::container::{self, Sidecar};
use drapewright_project::{ChangeSet, Command, Engine};

use crate::ToolError;

/// What the server needs from the computer it runs on. Tests pass fixed
/// values, so their results don't depend on the clock.
#[derive(Clone, Copy, Debug)]
pub struct Host {
    /// The time now, RFC 3339 in UTC.
    pub now: fn() -> String,
    /// A new, unique project id.
    pub new_id: fn() -> String,
}

/// Written into saved projects as the program that made them.
pub fn generator() -> String {
    format!("Drapewright MCP {}", drapewright_pattern_core::VERSION)
}

#[derive(Debug)]
pub struct OpenProject {
    pub engine: Engine,
    /// Where it was opened from or last saved to.
    pub path: Option<PathBuf>,
    /// Files the pieces were imported from, kept in the project file.
    pub sources: Vec<Sidecar>,
    saved_revision: u64,
}

impl OpenProject {
    pub fn unsaved(&self) -> bool {
        self.engine.state().revision != self.saved_revision
    }

    /// Keeps a copy of an imported file, as the app does.
    pub fn keep_source(&mut self, sha256: &str, file_name: &str, data: &[u8]) {
        let path = source_path(sha256, file_name);
        if !self.sources.iter().any(|s| s.path == path) {
            self.sources.push(Sidecar {
                path,
                data: data.to_vec(),
            });
        }
    }
}

/// `sources/<sha256>-<name>`, made safe as one file name exactly as the app
/// does (`sourceName` in `web/src/storage/projects.ts`).
pub fn source_path(sha256: &str, file_name: &str) -> String {
    let clean: String = file_name
        .chars()
        .map(|c| {
            if c < ' ' || c == '/' || c == '\\' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let clean: String = clean.trim_start_matches('.').chars().take(120).collect();
    let clean = if clean.is_empty() {
        "file".into()
    } else {
        clean
    };
    format!("{}{sha256}-{clean}", container::SOURCES)
}

/// `~/…` from the home folder; anything else as given.
pub fn expand(path: &str) -> PathBuf {
    match (path.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => Path::new(&home).join(rest),
        _ => PathBuf::from(path),
    }
}

fn read(path: &Path) -> Result<Vec<u8>, ToolError> {
    std::fs::read(path)
        .map_err(|e| ToolError::new(format!("Could not read {}: {e}", path.display())))
}

/// Writes through a temporary file, so a failed save never leaves half a
/// file in place of a good one.
pub fn write(path: &Path, data: &[u8], overwrite: bool) -> Result<(), ToolError> {
    if path.exists() && !overwrite {
        return Err(ToolError::new(format!(
            "{} already exists. Pass overwrite: true to replace it.",
            path.display()
        )));
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".partial");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, data)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            ToolError::new(format!("Could not write {}: {e}", path.display()))
        })
}

#[derive(Debug)]
pub struct Session {
    pub host: Host,
    open: Option<OpenProject>,
}

impl Session {
    pub fn new(host: Host) -> Self {
        Session { host, open: None }
    }

    pub fn project(&self) -> Result<&OpenProject, ToolError> {
        self.open.as_ref().ok_or_else(no_project)
    }

    pub fn project_mut(&mut self) -> Result<&mut OpenProject, ToolError> {
        self.open.as_mut().ok_or_else(no_project)
    }

    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    fn check_unsaved(&self, discard: bool) -> Result<(), ToolError> {
        match &self.open {
            Some(p) if p.unsaved() && !discard => Err(ToolError::new(
                "The open project has unsaved changes. Save them with save_project, or pass discard_unsaved: true to drop them.",
            )),
            _ => Ok(()),
        }
    }

    /// Starts a new, empty project.
    pub fn create(&mut self, name: &str, discard: bool) -> Result<(), ToolError> {
        self.check_unsaved(discard)?;
        let engine = Engine::new(
            &(self.host.new_id)(),
            name,
            &(self.host.now)(),
            &generator(),
        );
        let saved_revision = engine.state().revision;
        self.open = Some(OpenProject {
            engine,
            path: None,
            sources: Vec::new(),
            saved_revision,
        });
        Ok(())
    }

    /// Opens a `.drapewright` file (or a bare `project.json`). Returns
    /// problems found in its pieces, which don't stop it opening.
    pub fn open(&mut self, path: &Path, discard: bool) -> Result<Vec<Issue>, ToolError> {
        self.check_unsaved(discard)?;
        let bytes = read(path)?;
        let (json, sources) = if bytes.starts_with(b"PK") {
            let unpacked = container::unpack(&bytes).map_err(|e| ToolError::new(e.to_string()))?;
            (unpacked.project_json, unpacked.sidecars)
        } else {
            let json = String::from_utf8(bytes).map_err(|_| {
                ToolError::new("This is not a Drapewright project file (.drapewright).")
            })?;
            (json, Vec::new())
        };
        let (engine, issues) =
            Engine::open(&json, &generator()).map_err(|e| ToolError::new(e.to_string()))?;
        let saved_revision = engine.state().revision;
        self.open = Some(OpenProject {
            engine,
            path: Some(path.to_path_buf()),
            sources,
            saved_revision,
        });
        Ok(issues)
    }

    /// Saves to `path`, or where the project was opened from or last saved.
    pub fn save(&mut self, path: Option<&Path>, overwrite: bool) -> Result<PathBuf, ToolError> {
        let now = (self.host.now)();
        let project = self.project_mut()?;
        let target = match (path, &project.path) {
            (Some(p), _) => with_extension(p),
            (None, Some(p)) => p.clone(),
            (None, None) => {
                return Err(ToolError::new(
                    "This project has not been saved yet. Give a path ending in .drapewright.",
                ));
            }
        };
        let same = project.path.as_deref() == Some(target.as_path());
        let json = project.engine.to_json(&now);
        let bytes =
            container::pack(&json, &project.sources).map_err(|e| ToolError::new(e.to_string()))?;
        write(&target, &bytes, overwrite || same)?;
        project.path = Some(target.clone());
        project.saved_revision = project.engine.state().revision;
        Ok(target)
    }

    /// Applies the commands as one undo step; if any is rejected, nothing
    /// changes.
    pub fn dispatch(&mut self, commands: Vec<Command>) -> Result<ChangeSet, ToolError> {
        let project = self.project_mut()?;
        project
            .engine
            .dispatch_all(commands)
            .map_err(|e| ToolError::new(e.to_string()))
    }
}

fn no_project() -> ToolError {
    ToolError::new(
        "No project is open. Use open_project or new_project first (import_pattern also starts one).",
    )
}

/// The path with `.drapewright` added unless it already ends in it.
fn with_extension(path: &Path) -> PathBuf {
    let is_project = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("drapewright"));
    if is_project {
        path.to_path_buf()
    } else {
        let mut p = path.as_os_str().to_owned();
        p.push(".drapewright");
        PathBuf::from(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_names_match_the_app() {
        assert_eq!(source_path("ab", "skirt.dxf"), "sources/ab-skirt.dxf");
        assert_eq!(source_path("ab", "a/b\\c.svg"), "sources/ab-a-b-c.svg");
        assert_eq!(source_path("ab", "..hidden.svg"), "sources/ab-hidden.svg");
        assert_eq!(source_path("ab", "..."), "sources/ab-file");
        assert!(container::safe_source_name(
            &source_path("ab", "x.svg")["sources/".len()..]
        ));
    }

    #[test]
    fn project_files_get_their_extension() {
        assert_eq!(
            with_extension(Path::new("/t/dress")),
            Path::new("/t/dress.drapewright")
        );
        assert_eq!(
            with_extension(Path::new("/t/dress.Drapewright")),
            Path::new("/t/dress.Drapewright")
        );
    }
}
