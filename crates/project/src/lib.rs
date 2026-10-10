//! The Soom project document: the file format's top level, the commands
//! that change it, unlimited undo, and the derived geometry the app draws.
//! See `docs/file-format.md`.

pub mod command;
pub mod container;
pub mod document;
pub mod engine;
pub mod history;
pub mod hit;
pub mod ids;
pub mod migrate;
pub mod view;

pub use command::{ChangeSet, Command, CommandError};
pub use document::{Document, ProjectFile, SCHEMA_VERSION};
pub use engine::{DispatchOptions, Engine, EngineState};
pub use hit::Hit;
pub use migrate::LoadError;
pub use view::{EdgeView, PieceView};
