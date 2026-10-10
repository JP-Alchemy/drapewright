//! WebAssembly facade for Soom's 2D engine.
//!
//! This crate only converts between JavaScript and Rust types. All logic
//! lives in the platform-independent crates it wraps, so a native shell can
//! use them without going through here. Values cross the boundary as plain
//! JavaScript objects shaped like the TypeScript types generated from the
//! Rust ones (`packages/engine/src/generated`).

use serde::Serialize;
use serde::de::DeserializeOwned;
use soom_io::svg::{SvgExportOptions, export_svg};
use soom_io::{AllowanceAnswer, ImportResult, ScaleOption};
use soom_pattern_core::{Issue, Piece, Pt};
use soom_project::document::ViewState;
use soom_project::{Command, CommandError, DispatchOptions, Engine};
use wasm_bindgen::prelude::*;

/// Called once by the host after the module is instantiated.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Engine version, shown in the app's About panel.
#[wasm_bindgen(js_name = engineVersion)]
pub fn engine_version() -> String {
    soom_pattern_core::VERSION.to_string()
}

fn generator() -> String {
    format!("Soom {}", soom_pattern_core::VERSION)
}

fn to_js<T: Serialize + ?Sized>(value: &T) -> Result<JsValue, JsError> {
    value
        .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
        .map_err(|e| JsError::new(&e.to_string()))
}

fn from_js<T: DeserializeOwned>(value: JsValue) -> Result<T, JsError> {
    serde_wasm_bindgen::from_value(value).map_err(|e| JsError::new(&e.to_string()))
}

/// The outcome of a command: `{ status: "ok", changes }` or
/// `{ status: "rejected", error }` with a plain-language `error.message`.
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "camelCase")]
enum DispatchResult {
    Ok { changes: soom_project::ChangeSet },
    Rejected { error: RejectedError },
}

#[derive(Serialize)]
struct RejectedError {
    #[serde(flatten)]
    error: CommandError,
    message: String,
}

/// An open project.
#[wasm_bindgen]
#[derive(Debug)]
pub struct EngineHandle {
    engine: Engine,
    load_issues: Vec<Issue>,
}

#[wasm_bindgen]
impl EngineHandle {
    /// A new, empty project.
    pub fn create(project_id: &str, name: &str, now: &str) -> EngineHandle {
        EngineHandle {
            engine: Engine::new(project_id, name, now, &generator()),
            load_issues: Vec::new(),
        }
    }

    /// Opens `project.json` text. Throws with a plain-language message if
    /// the file cannot be opened at all.
    pub fn open(json: &str) -> Result<EngineHandle, JsError> {
        let (engine, load_issues) =
            Engine::open(json, &generator()).map_err(|e| JsError::new(&e.to_string()))?;
        Ok(EngineHandle {
            engine,
            load_issues,
        })
    }

    /// Problems found in the pieces when the project was opened.
    #[wasm_bindgen(js_name = loadIssues)]
    pub fn load_issues(&self) -> Result<JsValue, JsError> {
        to_js(&self.load_issues)
    }

    pub fn dispatch(&mut self, command: JsValue, options: JsValue) -> Result<JsValue, JsError> {
        let command: Command = from_js(command)?;
        let options: DispatchOptions = if options.is_undefined() || options.is_null() {
            Default::default()
        } else {
            from_js(options)?
        };
        let result = match self.engine.dispatch(command, options) {
            Ok(changes) => DispatchResult::Ok { changes },
            Err(error) => DispatchResult::Rejected {
                error: RejectedError {
                    message: error.to_string(),
                    error,
                },
            },
        };
        to_js(&result)
    }

    /// Returns the change set, or `null` when there is nothing to undo.
    pub fn undo(&mut self) -> Result<JsValue, JsError> {
        to_js(&self.engine.undo())
    }

    pub fn redo(&mut self) -> Result<JsValue, JsError> {
        to_js(&self.engine.redo())
    }

    pub fn state(&self) -> Result<JsValue, JsError> {
        to_js(&self.engine.state())
    }

    pub fn document(&self) -> Result<JsValue, JsError> {
        to_js(self.engine.document())
    }

    pub fn piece(&self, id: &str) -> Result<JsValue, JsError> {
        to_js(&self.engine.document().piece(id).map(|p| p.as_ref()))
    }

    #[wasm_bindgen(js_name = pieceView)]
    pub fn piece_view(&mut self, id: &str) -> Result<JsValue, JsError> {
        to_js(&self.engine.piece_view(id).as_deref())
    }

    /// What is under the point `(x, y)` on the work table (mm), within
    /// `tolerance` mm, or `null`.
    #[wasm_bindgen(js_name = hitTest)]
    pub fn hit_test(&mut self, x: f64, y: f64, tolerance: f64) -> Result<JsValue, JsError> {
        to_js(&self.engine.hit_test(Pt::new(x, y), tolerance))
    }

    /// Distance along an edge (mm from its start) of the point nearest to
    /// `(x, y)` on the work table, or `null`.
    #[wasm_bindgen(js_name = nearestOnEdge)]
    pub fn nearest_on_edge(&self, piece: &str, edge: &str, x: f64, y: f64) -> Option<f64> {
        self.engine
            .nearest_on_edge(piece, edge, Pt::new(x, y))
            .map(|n| n.along)
    }

    #[wasm_bindgen(js_name = viewState)]
    pub fn view_state(&self) -> Result<JsValue, JsError> {
        to_js(self.engine.view_state())
    }

    #[wasm_bindgen(js_name = setViewState)]
    pub fn set_view_state(&mut self, view: JsValue) -> Result<(), JsError> {
        let view: ViewState = from_js(view)?;
        self.engine.set_view_state(view);
        Ok(())
    }

    /// The project as `project.json` text, stamped as modified at `now`.
    #[wasm_bindgen(js_name = toJson)]
    pub fn to_json(&self, now: &str) -> String {
        self.engine.to_json(now)
    }
}

/// The cutting and stitching lines a piece would have, without changing any
/// project: used to preview choices such as corner shapes.
#[wasm_bindgen(js_name = previewAllowance)]
pub fn preview_allowance(piece: JsValue) -> Result<JsValue, JsError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Preview {
        cutting: Vec<f64>,
        stitching: Vec<f64>,
        notes: Vec<soom_pattern_core::allowance::AllowanceNote>,
    }
    let piece: Piece = from_js(piece)?;
    let lines = soom_pattern_core::allowance::allowance_lines(&piece);
    let flat = |pts: &[kurbo::Point]| pts.iter().flat_map(|p| [p.x, p.y]).collect();
    to_js(&Preview {
        cutting: flat(&lines.cutting),
        stitching: flat(&lines.stitching),
        notes: lines.notes,
    })
}

/// Reads pattern pieces from an SVG file.
#[wasm_bindgen(js_name = importSvg)]
pub fn import_svg(data: &[u8], file_name: Option<String>) -> Result<JsValue, JsError> {
    let result = soom_io::svg::import_svg(data, file_name.as_deref())
        .map_err(|e| JsError::new(&e.to_string()))?;
    to_js(&result)
}

/// Reads pattern pieces from an ASCII DXF file (CAD drawings and AAMA/ASTM
/// garment pattern files).
#[wasm_bindgen(js_name = importDxf)]
pub fn import_dxf(data: &[u8], file_name: Option<String>) -> Result<JsValue, JsError> {
    let result = soom_io::dxf::import_dxf(data, file_name.as_deref())
        .map_err(|e| JsError::new(&e.to_string()))?;
    to_js(&result)
}

/// Settles an import with the user's answers: the scale she confirmed and,
/// when the file did not say, whether its outlines include seam allowance
/// (`allowance` is null when nothing was asked).
#[wasm_bindgen(js_name = finishImport)]
pub fn finish_import(
    result: JsValue,
    scale: JsValue,
    allowance: JsValue,
) -> Result<JsValue, JsError> {
    let mut result: ImportResult = from_js(result)?;
    let scale: ScaleOption = from_js(scale)?;
    let allowance: Option<AllowanceAnswer> = from_js(allowance)?;
    result.rescale(&scale, true);
    if let Some(answer) = allowance {
        result.answer_allowance(answer);
    }
    to_js(&result)
}

/// Writes pieces as a true-size SVG document.
#[wasm_bindgen(js_name = exportSvg)]
pub fn export_svg_js(pieces: JsValue) -> Result<String, JsError> {
    let pieces: Vec<Piece> = from_js(pieces)?;
    Ok(export_svg(&pieces, &SvgExportOptions::default()))
}
