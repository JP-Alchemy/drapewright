//! The tools an assistant can call. Every change goes through the same
//! commands as the app, as one undo step, so it can be undone and the
//! project stays valid.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use drapewright_io::pdf::{PdfTexts, PrintPlan, export_pdf, plan_print};
use drapewright_io::svg::{SvgExportOptions, export_svg};
use drapewright_io::{AllowanceAnswer, ImportResult};
use drapewright_pattern_core::build::{DEFAULT_CORNER_ANGLE, counter, outline_from_path};
use drapewright_pattern_core::ops::normalize_orientation;
use drapewright_pattern_core::{
    Allowance, AllowanceMode, CornerStyle, CutInstruction, EdgeKind, Grainline, Id, NotchFrom,
    NotchKind, Piece, Pt, ScaleBasis, Source, SourceFormat,
};
use drapewright_project::document::{Assembly, Document, Orientation, Paper, PaperName};
use drapewright_project::hit::placement_affine;
use drapewright_project::{Command, PieceView};
use kurbo::{BezPath, ParamCurve, Point, Rect};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::render::{self, Labels};
use crate::session::{OpenProject, Session, expand, write};
use crate::{ToolError, text};

/// Room left between imported pieces and those already on the table, mm
/// (`IMPORT_GAP` in the app).
const IMPORT_GAP: f64 = 50.0;

/// What a tool returns: text, and sometimes a picture.
#[derive(Debug, Default)]
pub struct Output {
    pub content: Vec<Value>,
}

impl Output {
    pub fn text(text: impl Into<String>) -> Self {
        Output {
            content: vec![json!({ "type": "text", "text": text.into() })],
        }
    }

    pub fn json(value: &impl Serialize) -> Self {
        Output::text(serde_json::to_string_pretty(value).unwrap_or_default())
    }

    fn image(mut self, png: &[u8]) -> Self {
        self.content.push(json!({
            "type": "image",
            "data": crate::base64(png),
            "mimeType": "image/png",
        }));
        self
    }
}

type Run = Box<dyn Fn(&mut Session, Value) -> Result<Output, ToolError> + Send + Sync>;

/// How a tool affects things, for the hints clients show.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Only reads.
    Reads,
    /// Changes the open project; undo takes it back.
    Edits,
    /// Writes files, or removes pieces.
    Writes,
}

pub struct Tool {
    pub name: &'static str,
    title: &'static str,
    description: &'static str,
    effect: Effect,
    input_schema: Value,
    run: Run,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool").field("name", &self.name).finish()
    }
}

impl Tool {
    pub fn describe(&self) -> Value {
        json!({
            "name": self.name,
            "title": self.title,
            "description": self.description,
            "inputSchema": self.input_schema,
            "annotations": {
                "title": self.title,
                "readOnlyHint": self.effect == Effect::Reads,
                "destructiveHint": self.effect == Effect::Writes,
                "idempotentHint": self.effect == Effect::Reads,
                "openWorldHint": false,
            },
        })
    }

    pub fn call(&self, session: &mut Session, args: Value) -> Result<Output, ToolError> {
        (self.run)(session, args)
    }
}

/// JSON Schema for a tool's arguments, with everything inline.
fn schema<A: JsonSchema>() -> Value {
    let generator = schemars::generate::SchemaSettings::draft2020_12()
        .with(|s| {
            s.inline_subschemas = true;
            s.meta_schema = None;
        })
        .into_generator();
    let mut value = generator.into_root_schema_for::<A>().to_value();
    if let Value::Object(map) = &mut value {
        map.remove("title");
        map.remove("description");
        map.entry("properties").or_insert_with(|| json!({}));
    }
    value
}

fn tool<A: DeserializeOwned + JsonSchema + 'static>(
    name: &'static str,
    title: &'static str,
    effect: Effect,
    description: &'static str,
    run: fn(&mut Session, A) -> Result<Output, ToolError>,
) -> Tool {
    Tool {
        name,
        title,
        description,
        effect,
        input_schema: schema::<A>(),
        run: Box::new(move |session, args| {
            let args: A = serde_json::from_value(args)
                .map_err(|e| ToolError::new(format!("The arguments don't fit this tool: {e}")))?;
            run(session, args)
        }),
    }
}

pub fn all() -> Vec<Tool> {
    use Effect::*;
    vec![
        tool(
            "project_summary",
            "Project summary",
            Reads,
            "The open project: its name, file, whether it has unsaved changes, and every piece with its id, size, seam allowance, cutting, edges, notches and any problems. Start here to learn the piece ids other tools take.",
            project_summary,
        ),
        tool(
            "new_project",
            "New project",
            Edits,
            "Starts a new, empty project (it is not saved until save_project).",
            new_project,
        ),
        tool(
            "open_project",
            "Open project",
            Edits,
            "Opens a Drapewright project file (.drapewright), such as a backup saved from the app.",
            open_project,
        ),
        tool(
            "save_project",
            "Save project",
            Writes,
            "Saves the project as a .drapewright file, with the pattern files its pieces came from. The user opens it in the Drapewright app with Projects → Restore a backup, or Open pattern.",
            save_project,
        ),
        tool(
            "inspect_pattern_file",
            "Inspect pattern file",
            Reads,
            "Reads an SVG, DXF or PDF sewing pattern without adding it: the pieces found, the sizes the file holds, how sure its scale is (with what each reading would make the pieces measure), whether seam allowance must be asked about, and what was left out. Use it before import_pattern to know which questions to ask the user.",
            inspect_pattern_file,
        ),
        tool(
            "import_pattern",
            "Import pattern",
            Edits,
            "Adds the pieces of an SVG, DXF or PDF pattern to the open project (starting a project if none is open), beside any pieces already there. When the file holds several sizes, doesn't say how big it is, or has pieces drawn with one outline, the matching argument is required: ask the user, never guess. The original file is kept in the project.",
            import_pattern,
        ),
        tool(
            "get_piece",
            "Get piece",
            Reads,
            "Everything about one piece: size, area, seam allowance, cutting, grainline, each edge (id, name, kind, length, allowance, corner), notches, marks, internal lines, position on the table, where it came from, and problems. Coordinates are mm in the piece's own frame, y down.",
            get_piece,
        ),
        tool(
            "render_preview",
            "Preview",
            Reads,
            "A picture of pieces where they lie on the work table: filled, cutting line solid, stitching line dashed, folds blue dash-dot, hems orange, notches, grainlines (green arrows), internal lines and marks. Pieces are numbered as listed in the reply; with labels \"edges\" every edge's id is written beside it. Use it to see shapes, check work and find edge ids.",
            render_preview,
        ),
        tool(
            "update_piece",
            "Update piece",
            Edits,
            "Renames a piece, changes its seam allowance, or sets how to cut it. Give only what changes.",
            update_piece,
        ),
        tool(
            "update_edge",
            "Update edge",
            Edits,
            "Changes one edge of a piece: its kind (seam, hem or fold), its name (e.g. \"centre front\"), its own seam allowance, or the corner shape where it ends. Give only what changes.",
            update_edge,
        ),
        tool(
            "split_edge",
            "Split edge",
            Edits,
            "Adds a corner on an edge, at_mm along it from its start, making two edges (for example to give part of an edge a different allowance).",
            split_edge,
        ),
        tool(
            "join_edges",
            "Join edges",
            Edits,
            "Removes the corner at the end of an edge, joining it with the edge after it.",
            join_edges,
        ),
        tool(
            "set_grainline",
            "Set grainline",
            Edits,
            "Sets or removes a piece's grainline: along its length (\"vertical\") or width (\"horizontal\") through its middle, or between two points.",
            set_grainline,
        ),
        tool(
            "add_notch",
            "Add notch",
            Edits,
            "Adds a notch on an edge, at_mm from its start (or end), single, double (back pieces) or triple.",
            add_notch,
        ),
        tool(
            "delete_notch",
            "Delete notch",
            Edits,
            "Removes a notch.",
            delete_notch,
        ),
        tool(
            "create_piece",
            "Create piece",
            Edits,
            "Draws a new piece: a rectangle (waistband, ties, pocket…) or any closed outline as SVG path data in millimetres. It is placed beside the other pieces.",
            create_piece,
        ),
        tool(
            "delete_pieces",
            "Delete pieces",
            Writes,
            "Removes pieces from the project (undo brings them back).",
            delete_pieces,
        ),
        tool(
            "move_pieces",
            "Move pieces",
            Edits,
            "Moves pieces on the work table by dx_mm, dy_mm (y down).",
            move_pieces,
        ),
        tool(
            "rotate_piece",
            "Turn piece",
            Edits,
            "Turns a piece on the work table about its centre, clockwise in degrees.",
            rotate_piece,
        ),
        tool(
            "arrange_pieces",
            "Arrange pieces",
            Edits,
            "Lays pieces out tidily on the work table in rows no wider than width_mm, tallest first, without turning them. Useful after imports, or to see how they fit a fabric width.",
            arrange_pieces,
        ),
        tool("undo", "Undo", Edits, "Undoes the last change.", undo),
        tool(
            "redo",
            "Redo",
            Edits,
            "Redoes the change undo took back.",
            redo,
        ),
        tool(
            "print_settings",
            "Print settings",
            Edits,
            "Shows how the project prints (paper, pages, grid, which pieces) and changes any settings given: paper, orientation, how pages join, margins, the stitching line, the test square, the page map, and which pieces print.",
            print_settings,
        ),
        tool(
            "export_pdf",
            "Export PDF",
            Writes,
            "Writes the PDF of tiled pages that prints at true size, with the project's print settings (see print_settings).",
            export_pdf_tool,
        ),
        tool(
            "export_svg",
            "Export SVG",
            Writes,
            "Writes pieces as a true-size SVG file (millimetres), for other programs.",
            export_svg_tool,
        ),
    ]
}

// ---------------------------------------------------------------- helpers

fn r1(v: f64) -> f64 {
    text::round1(v)
}

fn pt(p: Pt) -> [f64; 2] {
    [r1(p.x), r1(p.y)]
}

fn unknown_piece(id: &str) -> ToolError {
    ToolError::new(format!(
        "There is no piece \"{id}\". project_summary lists the pieces and their ids."
    ))
}

fn piece_and_view(
    project: &mut OpenProject,
    id: &str,
) -> Result<(Arc<Piece>, Arc<PieceView>), ToolError> {
    let piece = project
        .engine
        .document()
        .piece(id)
        .cloned()
        .ok_or_else(|| unknown_piece(id))?;
    let view = project
        .engine
        .piece_view(id)
        .ok_or_else(|| unknown_piece(id))?;
    Ok((piece, view))
}

/// Where a piece's cutting line lies on the work table.
fn table_bounds(piece: &Piece, view: &PieceView) -> Rect {
    let a = placement_affine(piece);
    view.cutting_points()
        .into_iter()
        .map(|p| a * p)
        .fold(Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN), |r, p| {
            Rect::new(r.x0.min(p.x), r.y0.min(p.y), r.x1.max(p.x), r.y1.max(p.y))
        })
}

fn has_fold(piece: &Piece) -> bool {
    piece.outline.edges.iter().any(|e| e.kind == EdgeKind::Fold)
}

fn check_length(what: &str, mm: f64, max: f64) -> Result<(), ToolError> {
    if !mm.is_finite() || mm < 0.0 || mm > max {
        return Err(ToolError::new(format!(
            "{what} must be between 0 and {max} mm."
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------- project

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NoArgs {}

#[derive(Serialize)]
struct Summary {
    project: String,
    file: Option<String>,
    unsaved_changes: bool,
    pieces: Vec<PieceLine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    undo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    redo: Option<String>,
}

#[derive(Serialize)]
struct PieceLine {
    id: Id,
    name: String,
    size: String,
    seam_allowance: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    cut: Vec<String>,
    edges: usize,
    #[serde(skip_serializing_if = "is_zero")]
    folds: usize,
    #[serde(skip_serializing_if = "is_zero")]
    hems: usize,
    notches: usize,
    grainline: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    problems: Vec<String>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

fn summary(session: &mut Session) -> Result<Summary, ToolError> {
    let project = session.project_mut()?;
    let ids: Vec<Id> = project
        .engine
        .document()
        .pieces
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let mut pieces = Vec::new();
    for id in ids {
        let (piece, view) = piece_and_view(project, &id)?;
        let [x0, y0, x1, y1] = view.outline_bounds;
        let count = |k: EdgeKind| piece.outline.edges.iter().filter(|e| e.kind == k).count();
        pieces.push(PieceLine {
            id: piece.id.clone(),
            name: piece.name.clone(),
            size: text::size(x1 - x0, y1 - y0),
            seam_allowance: text::allowance(&piece),
            cut: piece
                .cut
                .iter()
                .map(|c| text::cut(c, has_fold(&piece)))
                .collect(),
            edges: piece.outline.edges.len(),
            folds: count(EdgeKind::Fold),
            hems: count(EdgeKind::Hem),
            notches: piece.notches.len(),
            grainline: piece.grainline.is_some(),
            problems: view.issues.iter().map(|i| i.message.clone()).collect(),
        });
    }
    let state = project.engine.state();
    Ok(Summary {
        project: project.engine.document().meta.name.clone(),
        file: project.path.as_ref().map(|p| p.display().to_string()),
        unsaved_changes: project.unsaved(),
        pieces,
        undo: state.undo_label,
        redo: state.redo_label,
    })
}

fn project_summary(session: &mut Session, _: NoArgs) -> Result<Output, ToolError> {
    Ok(Output::json(&summary(session)?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NewProject {
    /// The project's name, e.g. "Linen summer dress".
    name: Option<String>,
    /// Drop unsaved changes to the project open now.
    #[serde(default)]
    discard_unsaved: bool,
}

fn new_project(session: &mut Session, a: NewProject) -> Result<Output, ToolError> {
    let name = a.name.unwrap_or_else(|| "Untitled pattern".into());
    session.create(&name, a.discard_unsaved)?;
    Ok(Output::json(&summary(session)?))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct OpenArgs {
    /// Path of the .drapewright file. `~/` means the home folder.
    path: String,
    /// Drop unsaved changes to the project open now.
    #[serde(default)]
    discard_unsaved: bool,
}

fn open_project(session: &mut Session, a: OpenArgs) -> Result<Output, ToolError> {
    let issues = session.open(&expand(&a.path), a.discard_unsaved)?;
    let mut out = serde_json::to_value(summary(session)?).unwrap_or_default();
    if !issues.is_empty() {
        out["problems_found_on_opening"] =
            json!(issues.iter().map(|i| &i.message).collect::<Vec<_>>());
    }
    Ok(Output::json(&out))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SaveArgs {
    /// Where to save, ending in .drapewright (added if missing). Leave out to
    /// save where the project was opened from or last saved.
    path: Option<String>,
    /// Replace a different file already at that path.
    #[serde(default)]
    overwrite: bool,
}

fn save_project(session: &mut Session, a: SaveArgs) -> Result<Output, ToolError> {
    let path = a.path.as_deref().map(expand);
    let saved = session.save(path.as_deref(), a.overwrite)?;
    let pieces = session.project()?.engine.document().pieces.len();
    Ok(Output::text(format!(
        "Saved {} ({pieces} pieces). In Drapewright: Projects → Restore a backup, or Open pattern, and pick this file.",
        saved.display()
    )))
}

// ---------------------------------------------------------------- import

/// A pattern file, read.
struct Read {
    file_name: String,
    data: Vec<u8>,
    result: ImportResult,
}

fn read_pattern(path: &str) -> Result<Read, ToolError> {
    let path = expand(path);
    let data = std::fs::read(&path)
        .map_err(|e| ToolError::new(format!("Could not read {}: {e}", path.display())))?;
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "pattern".into());
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    let head = String::from_utf8_lossy(&data[..data.len().min(4096)]).to_string();
    let result = if extension == "pdf" || data.starts_with(b"%PDF") {
        drapewright_io::pdf::import_pdf(&data, Some(&file_name))
    } else if extension == "svg" || (extension != "dxf" && head.contains("<svg")) {
        drapewright_io::svg::import_svg(&data, Some(&file_name))
    } else if extension == "dxf" || head.contains("SECTION") {
        drapewright_io::dxf::import_dxf(&data, Some(&file_name))
    } else {
        return Err(ToolError::new(
            "Drapewright reads SVG, DXF and PDF pattern files. This one is none of these.",
        ));
    }
    .map_err(|e| ToolError::new(e.to_string()))?;
    Ok(Read {
        file_name,
        data,
        result,
    })
}

#[derive(Serialize)]
struct FileReport {
    file: String,
    pieces_found: usize,
    pieces: Vec<FoundPiece>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sizes: Option<SizesReport>,
    scale: ScaleReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    seam_allowance_question: Option<AllowanceReport>,
    notes: Vec<String>,
    to_import: String,
}

#[derive(Serialize)]
struct FoundPiece {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<String>,
    measures: String,
}

#[derive(Serialize)]
struct SizesReport {
    sizes: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    made_in: Option<String>,
    note: String,
}

#[derive(Serialize)]
struct ScaleReport {
    certain: bool,
    reading: ScaleBasis,
    why: &'static str,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    readings: Vec<ScaleChoice>,
}

#[derive(Serialize)]
struct ScaleChoice {
    scale: ScaleBasis,
    meaning: &'static str,
    largest_piece_would_be: String,
}

#[derive(Serialize)]
struct AllowanceReport {
    pieces: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_suggests: Option<AllowanceMode>,
    question: String,
}

fn outline_size(piece: &Piece) -> (f64, f64) {
    let b = piece.outline.bounds();
    (b.width(), b.height())
}

fn describe_file(read: &Read) -> FileReport {
    let r = &read.result;
    let graded = r.sizes.names.len() > 1;
    let name_of = |id: &str| {
        r.pieces
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| id.to_string(), |p| p.name.clone())
    };
    // A graded file repeats every piece in every size: one size is enough.
    let shown_size = graded.then(|| {
        r.sizes
            .sample
            .clone()
            .filter(|s| r.sizes.names.contains(s))
            .unwrap_or_else(|| r.sizes.names[r.sizes.names.len() / 2].clone())
    });
    let pieces = r
        .pieces
        .iter()
        .filter(|p| shown_size.is_none() || r.sizes.of(&p.id) == shown_size.as_deref())
        .map(|p| {
            let (w, h) = outline_size(p);
            FoundPiece {
                name: p.name.clone(),
                size: r.sizes.of(&p.id).map(str::to_string),
                measures: text::size(w, h),
            }
        })
        .collect();
    let largest = r.pieces.iter().max_by(|a, b| {
        let (aw, ah) = outline_size(a);
        let (bw, bh) = outline_size(b);
        (aw * ah).total_cmp(&(bw * bh))
    });
    let readings = if r.scale.certain {
        Vec::new()
    } else {
        std::iter::once(&r.scale.chosen)
            .chain(&r.scale.alternatives)
            .map(|o| {
                let factor = o.mm_per_unit / r.scale.chosen.mm_per_unit;
                ScaleChoice {
                    scale: o.basis,
                    meaning: text::scale_basis(o.basis),
                    largest_piece_would_be: largest.map_or_else(String::new, |p| {
                        let (w, h) = outline_size(p);
                        format!("{}: {}", p.name, text::size(w * factor, h * factor))
                    }),
                }
            })
            .collect()
    };
    // Each name once, though a file with several sizes asks about each.
    let mut asked: Vec<String> = Vec::new();
    for name in r.allowance.pieces.iter().map(|id| name_of(id)) {
        if !asked.contains(&name) {
            asked.push(name);
        }
    }
    let seam_allowance_question = (!asked.is_empty()).then(|| AllowanceReport {
        question: format!(
            "Is seam allowance already included in {}, and how wide is it (or how wide should it be)?",
            if r.allowance.pieces.len() == r.pieces.len() {
                "these pieces".to_string()
            } else {
                format!("the pieces drawn with one outline ({})", asked.join(", "))
            }
        ),
        pieces: asked,
        file_suggests: r.allowance.suggested,
    });
    let mut needed = Vec::new();
    if graded {
        needed.push("sizes (ask which size or sizes)");
    }
    if !r.scale.certain {
        needed.push("scale (ask which reading gives the sizes the user expects)");
    }
    if seam_allowance_question.is_some() {
        needed.push("seam_allowance (ask whether it is included, and its width)");
    }
    let to_import = if needed.is_empty() {
        "import_pattern with just the path: nothing needs asking.".to_string()
    } else {
        format!("import_pattern with the path and {}.", needed.join(", "))
    };
    FileReport {
        file: read.file_name.clone(),
        pieces_found: r.pieces.len(),
        pieces,
        sizes: graded.then(|| SizesReport {
            sizes: r.sizes.names.clone(),
            made_in: r.sizes.sample.clone(),
            note: format!(
                "The file holds several sizes; the pieces above are size {}.",
                shown_size.clone().unwrap_or_default()
            ),
        }),
        scale: ScaleReport {
            certain: r.scale.certain,
            reading: r.scale.chosen.basis,
            why: text::scale_reason(r.scale.reason),
            readings,
        },
        seam_allowance_question,
        notes: r.notes.iter().map(|n| text::note(n, &name_of)).collect(),
        to_import,
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct FileArgs {
    /// Path of an SVG, DXF or PDF pattern file. `~/` means the home folder.
    path: String,
}

fn inspect_pattern_file(_: &mut Session, a: FileArgs) -> Result<Output, ToolError> {
    Ok(Output::json(&describe_file(&read_pattern(&a.path)?)))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AllowanceArg {
    /// true: the outlines are cutting lines, with the allowance inside them.
    /// false: they are stitching lines, and the allowance goes outside.
    included: bool,
    /// Width of the seam allowance, mm (15 mm is common; 10 mm in many
    /// European patterns).
    width_mm: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ImportArgs {
    /// Path of an SVG, DXF or PDF pattern file. `~/` means the home folder.
    path: String,
    /// The sizes to add, for a file that holds several (inspect_pattern_file
    /// lists them). Ask the user. With more than one, each piece's name says
    /// its size.
    sizes: Option<Vec<String>>,
    /// How big the file is, when it doesn't say: one of the readings
    /// inspect_pattern_file lists. Ask the user, telling them how big a piece
    /// would be at each.
    scale: Option<ScaleBasis>,
    /// For pieces drawn with one outline: whether seam allowance is included,
    /// and its width. Ask the user.
    seam_allowance: Option<AllowanceArg>,
}

#[derive(Serialize)]
struct Imported {
    project: String,
    added: Vec<AddedPiece>,
    notes: Vec<String>,
    next: &'static str,
}

#[derive(Serialize)]
struct AddedPiece {
    id: Id,
    name: String,
    measures: String,
}

/// Moves new pieces to the right of those already on the table, as the app does.
fn beside_existing(project: &mut OpenProject, pieces: &mut [Piece]) {
    let ids: Vec<Id> = project
        .engine
        .document()
        .pieces
        .iter()
        .map(|p| p.id.clone())
        .collect();
    let mut existing: Option<Rect> = None;
    for id in ids {
        if let Ok((piece, view)) = piece_and_view(project, &id) {
            let b = table_bounds(&piece, &view);
            existing = Some(existing.map_or(b, |e| e.union(b)));
        }
    }
    let Some(existing) = existing else {
        return;
    };
    let points = pieces.iter().flat_map(|p| {
        let a = placement_affine(p);
        p.outline
            .path_segs()
            .into_iter()
            .flat_map(move |s| [a * s.start(), a * s.end()])
    });
    let (min_x, min_y) = points.fold((f64::MAX, f64::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
    if !min_x.is_finite() || min_x >= f64::MAX {
        return;
    }
    let (dx, dy) = (existing.x1 + IMPORT_GAP - min_x, existing.y0 - min_y);
    for p in pieces {
        p.placement.x += dx;
        p.placement.y += dy;
    }
}

fn add_pieces(session: &mut Session, mut pieces: Vec<Piece>) -> Result<Vec<AddedPiece>, ToolError> {
    let project = session.project_mut()?;
    beside_existing(project, &mut pieces);
    let sizes: Vec<(String, String)> = pieces
        .iter()
        .map(|p| {
            let (w, h) = outline_size(p);
            (p.name.clone(), text::size(w, h))
        })
        .collect();
    let set = session.dispatch(vec![Command::AddPieces { pieces }])?;
    Ok(set
        .added
        .into_iter()
        .zip(sizes)
        .map(|(id, (name, measures))| AddedPiece { id, name, measures })
        .collect())
}

fn import_pattern(session: &mut Session, a: ImportArgs) -> Result<Output, ToolError> {
    let Read {
        file_name,
        data,
        mut result,
    } = read_pattern(&a.path)?;
    if result.pieces.is_empty() {
        let notes: Vec<String> = result
            .notes
            .iter()
            .map(|n| text::note(n, &|id| id.into()))
            .collect();
        return Err(ToolError::new(format!(
            "No pieces were found in {file_name}. {}",
            notes.join(" ")
        )));
    }

    // Which sizes: never guessed for a file that holds several.
    let names = &result.sizes.names;
    if names.len() > 1 {
        let Some(sizes) = a.sizes.as_ref().filter(|s| !s.is_empty()) else {
            return Err(ToolError::new(format!(
                "{file_name} holds {} sizes: {}{}. Ask the user which size (or sizes) to add, and pass them as sizes.",
                names.len(),
                names.join(", "),
                result
                    .sizes
                    .sample
                    .as_ref()
                    .map(|s| format!(" (it was made in {s})"))
                    .unwrap_or_default()
            )));
        };
        if let Some(missing) = sizes.iter().find(|s| !names.contains(s)) {
            return Err(ToolError::new(format!(
                "The file has no size \"{missing}\". Its sizes are {}.",
                names.join(", ")
            )));
        }
    }
    result.choose_sizes(a.sizes.as_deref().unwrap_or_default());

    // How big: confirmed by the user when the file doesn't say.
    let options: Vec<_> = std::iter::once(result.scale.chosen.clone())
        .chain(result.scale.alternatives.iter().cloned())
        .collect();
    let option = match a.scale {
        Some(basis) => options
            .iter()
            .find(|o| o.basis == basis)
            .cloned()
            .ok_or_else(|| {
                ToolError::new(format!(
                    "\"{}\" is not a reading of this file. inspect_pattern_file lists them.",
                    serde_json::to_value(basis)
                        .unwrap_or_default()
                        .as_str()
                        .unwrap_or("?")
                ))
            })?,
        None if result.scale.certain => result.scale.chosen.clone(),
        None => {
            return Err(ToolError::new(format!(
                "{file_name} doesn't say how big it is. {} Call inspect_pattern_file to see how big the pieces would be at each reading, ask the user, and pass the reading as scale.",
                text::scale_reason(result.scale.reason)
            )));
        }
    };
    result.rescale(&option, true);

    // Seam allowance: asked, never assumed.
    if !result.allowance.pieces.is_empty() {
        let Some(answer) = &a.seam_allowance else {
            return Err(ToolError::new(format!(
                "{} of the pieces are drawn with one outline, so the file can't say whether seam allowance is included. Ask the user whether it is, and how wide it is (or should be), and pass seam_allowance.",
                result.allowance.pieces.len()
            )));
        };
        check_length("The seam allowance", answer.width_mm, 100.0)?;
        result.answer_allowance(AllowanceAnswer {
            mode: if answer.included {
                AllowanceMode::Included
            } else {
                AllowanceMode::Add
            },
            width: answer.width_mm,
        });
    }

    if !session.is_open() {
        let stem = Path::new(&file_name).file_stem().map_or_else(
            || "Untitled pattern".into(),
            |s| s.to_string_lossy().into_owned(),
        );
        session.create(&stem, false)?;
    }
    let names: HashMap<Id, String> = result
        .pieces
        .iter()
        .map(|p| (p.id.clone(), p.name.clone()))
        .collect();
    let notes = result
        .notes
        .iter()
        .map(|n| text::note(n, &|id| names.get(id).cloned().unwrap_or_else(|| id.into())))
        .collect();
    let added = add_pieces(session, result.pieces)?;
    let project = session.project_mut()?;
    project.keep_source(&result.sha256, &file_name, &data);
    Ok(Output::json(&Imported {
        project: project.engine.document().meta.name.clone(),
        added,
        notes,
        next: "render_preview shows the pieces; save_project keeps them.",
    }))
}

// ---------------------------------------------------------------- pieces

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PieceArg {
    /// The piece's id, e.g. "p12" (project_summary lists them).
    piece: String,
}

#[derive(Serialize)]
struct PieceDetail {
    id: Id,
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    notes: String,
    width_mm: f64,
    height_mm: f64,
    area_cm2: f64,
    seam_allowance: AllowanceDetail,
    cut: Vec<String>,
    grainline: Option<GrainDetail>,
    edges: Vec<EdgeDetail>,
    notches: Vec<NotchDetail>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    marks: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    internal_lines: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    labels: Vec<String>,
    on_the_table: Placed,
    #[serde(skip_serializing_if = "Option::is_none")]
    came_from: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    problems: Vec<String>,
}

#[derive(Serialize)]
struct AllowanceDetail {
    mode: AllowanceMode,
    seam_mm: f64,
    hem_mm: Option<f64>,
    says: String,
}

#[derive(Serialize)]
struct GrainDetail {
    from: [f64; 2],
    to: [f64; 2],
    one_way: bool,
}

#[derive(Serialize)]
struct EdgeDetail {
    id: Id,
    #[serde(skip_serializing_if = "String::is_empty")]
    name: String,
    kind: EdgeKind,
    length_mm: f64,
    allowance_mm: f64,
    from: [f64; 2],
    to: [f64; 2],
    corner_at_end: CornerStyle,
}

#[derive(Serialize)]
struct NotchDetail {
    id: Id,
    edge: Id,
    at_mm: f64,
    from: NotchFrom,
    kind: NotchKind,
}

#[derive(Serialize)]
struct Placed {
    x_mm: f64,
    y_mm: f64,
    rotation_deg: f64,
}

fn get_piece(session: &mut Session, a: PieceArg) -> Result<Output, ToolError> {
    let project = session.project_mut()?;
    let (piece, view) = piece_and_view(project, &a.piece)?;
    let [x0, y0, x1, y1] = view.outline_bounds;
    let fold = has_fold(&piece);
    let detail = PieceDetail {
        id: piece.id.clone(),
        name: piece.name.clone(),
        notes: piece.notes.clone(),
        width_mm: r1(x1 - x0),
        height_mm: r1(y1 - y0),
        area_cm2: r1(view.area / 100.0),
        seam_allowance: AllowanceDetail {
            mode: piece.allowance.mode,
            seam_mm: piece.allowance.seam,
            hem_mm: piece.allowance.hem,
            says: text::allowance(&piece),
        },
        cut: piece.cut.iter().map(|c| text::cut(c, fold)).collect(),
        grainline: piece.grainline.map(|g| GrainDetail {
            from: pt(g.from),
            to: pt(g.to),
            one_way: g.one_way,
        }),
        edges: view
            .edges
            .iter()
            .zip(&piece.outline.edges)
            .map(|(v, e)| EdgeDetail {
                id: v.id.clone(),
                name: v.name.clone(),
                kind: v.kind,
                length_mm: r1(v.length),
                allowance_mm: r1(v.allowance),
                from: pt(v.start),
                to: pt(v.end),
                corner_at_end: e.end_corner,
            })
            .collect(),
        notches: piece
            .notches
            .iter()
            .map(|n| NotchDetail {
                id: n.id.clone(),
                edge: n.edge.clone(),
                at_mm: r1(n.at),
                from: n.from,
                kind: n.kind,
            })
            .collect(),
        marks: piece
            .marks
            .iter()
            .map(|m| {
                let kind = serde_json::to_value(m.kind).unwrap_or_default();
                format!(
                    "{} {} at [{}, {}]",
                    m.id,
                    kind.as_str().unwrap_or("mark"),
                    r1(m.at.x),
                    r1(m.at.y)
                )
            })
            .collect(),
        internal_lines: piece
            .lines
            .iter()
            .map(|l| {
                let kind = serde_json::to_value(l.kind).unwrap_or_default();
                let label = l
                    .label
                    .as_deref()
                    .map(|t| format!(" “{t}”"))
                    .unwrap_or_default();
                format!("{} {} line{label}", l.id, kind.as_str().unwrap_or(""))
            })
            .collect(),
        labels: piece.labels.iter().map(|l| l.text.clone()).collect(),
        on_the_table: Placed {
            x_mm: r1(piece.placement.x),
            y_mm: r1(piece.placement.y),
            rotation_deg: r1(piece.placement.rotation),
        },
        came_from: piece.source.as_ref().map(|s| {
            let format = serde_json::to_value(s.format).unwrap_or_default();
            format!(
                "{} {}{}",
                format.as_str().unwrap_or(""),
                s.file.as_deref().unwrap_or("(no file)"),
                s.reference
                    .as_deref()
                    .map(|r| format!(" ({r})"))
                    .unwrap_or_default()
            )
        }),
        problems: view.issues.iter().map(|i| i.message.clone()).collect(),
    };
    Ok(Output::json(&detail))
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum LabelArg {
    Pieces,
    Edges,
    None,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PreviewArgs {
    /// Ids of the pieces to draw; leave out for all of them.
    pieces: Option<Vec<String>>,
    /// What to write on the picture: "pieces" (their numbers), "edges"
    /// (numbers and every edge's id) or "none". Default: "edges" when six
    /// pieces or fewer are drawn, otherwise "pieces".
    labels: Option<LabelArg>,
    /// Longest side of the picture in pixels, 300 to 2400 (default 1400).
    max_px: Option<u32>,
}

fn render_preview(session: &mut Session, a: PreviewArgs) -> Result<Output, ToolError> {
    let project = session.project_mut()?;
    let ids: Vec<Id> = match a.pieces {
        Some(ids) if !ids.is_empty() => ids,
        _ => project
            .engine
            .document()
            .pieces
            .iter()
            .map(|p| p.id.clone())
            .collect(),
    };
    if ids.is_empty() {
        return Err(ToolError::new("The project has no pieces yet."));
    }
    let mut drawn = Vec::new();
    for id in &ids {
        drawn.push(piece_and_view(project, id)?);
    }
    let labels = match a.labels {
        Some(LabelArg::Pieces) => Labels::Pieces,
        Some(LabelArg::Edges) => Labels::Edges,
        Some(LabelArg::None) => Labels::None,
        None if drawn.len() <= 6 => Labels::Edges,
        None => Labels::Pieces,
    };
    let refs: Vec<(&Piece, &PieceView)> = drawn
        .iter()
        .map(|(p, v)| (p.as_ref(), v.as_ref()))
        .collect();
    let picture = render::draw(&refs, labels, a.max_px.unwrap_or(1400).clamp(300, 2400))
        .map_err(ToolError::new)?;
    let mut legend = format!(
        "{} × {} px, 1 px ≈ {} mm. Solid: cutting line; dashed: stitching line; blue dash-dot: folds; orange: hems; green arrows: grainlines.\n",
        picture.width,
        picture.height,
        r1(picture.mm_per_px)
    );
    for (i, (piece, view)) in drawn.iter().enumerate() {
        let [x0, y0, x1, y1] = view.outline_bounds;
        legend.push_str(&format!(
            "{}. {} {} ({})\n",
            i + 1,
            piece.id,
            piece.name,
            text::size(x1 - x0, y1 - y0)
        ));
        if labels == Labels::Edges {
            let edges: Vec<String> = view
                .edges
                .iter()
                .map(|e| {
                    let kind = serde_json::to_value(e.kind).unwrap_or_default();
                    let name = if e.name.is_empty() {
                        String::new()
                    } else {
                        format!(" “{}”", e.name)
                    };
                    format!(
                        "{}{name} {} {}",
                        e.id,
                        kind.as_str().unwrap_or(""),
                        text::length(e.length)
                    )
                })
                .collect();
            legend.push_str(&format!("   edges: {}\n", edges.join(", ")));
        }
    }
    Ok(Output::text(legend).image(&picture.png))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PieceAllowanceArg {
    /// "add": the outline is the stitching line, allowance goes outside it.
    /// "included": the outline is the cutting line.
    mode: Option<AllowanceMode>,
    /// Allowance on seam edges, mm.
    seam_mm: Option<f64>,
    /// Allowance on hem edges, mm.
    hem_mm: Option<f64>,
    /// Make hem edges use the seam allowance again.
    #[serde(default)]
    hem_same_as_seam: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CutArg {
    /// How many to cut.
    quantity: u32,
    /// "main", "lining", "interfacing", "contrast", or any fabric name.
    /// Default "main".
    material: Option<String>,
    /// Cut as mirrored pairs. Leave out for the usual: yes when 2 or more.
    mirrored: Option<bool>,
    /// Place the fold edge on the fabric fold. Leave out for the usual: yes
    /// when the piece has a fold edge.
    on_fold: Option<bool>,
}

impl From<CutArg> for CutInstruction {
    fn from(c: CutArg) -> Self {
        CutInstruction {
            material: c.material.unwrap_or_else(|| "main".into()),
            quantity: c.quantity,
            mirrored: c.mirrored,
            on_fold: c.on_fold,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdatePiece {
    /// The piece's id, e.g. "p12".
    piece: String,
    /// A new name, e.g. "Front bodice".
    name: Option<String>,
    /// Seam allowance for the whole piece.
    seam_allowance: Option<PieceAllowanceArg>,
    /// How to cut it, one entry per fabric. Replaces what is there.
    cut: Option<Vec<CutArg>>,
}

fn update_piece(session: &mut Session, a: UpdatePiece) -> Result<Output, ToolError> {
    let piece = session
        .project()?
        .engine
        .document()
        .piece(&a.piece)
        .cloned()
        .ok_or_else(|| unknown_piece(&a.piece))?;
    let mut commands = Vec::new();
    if let Some(name) = a.name {
        if name.trim().is_empty() {
            return Err(ToolError::new("A piece needs a name."));
        }
        commands.push(Command::RenamePiece {
            piece: a.piece.clone(),
            name: name.trim().to_string(),
        });
    }
    if let Some(s) = a.seam_allowance {
        let mut allowance: Allowance = piece.allowance;
        if let Some(mode) = s.mode {
            allowance.mode = mode;
        }
        if let Some(seam) = s.seam_mm {
            check_length("The seam allowance", seam, 100.0)?;
            allowance.seam = seam;
        }
        if let Some(hem) = s.hem_mm {
            check_length("The hem allowance", hem, 150.0)?;
            allowance.hem = Some(hem);
        }
        if s.hem_same_as_seam {
            allowance.hem = None;
        }
        commands.push(Command::SetAllowance {
            piece: a.piece.clone(),
            allowance,
        });
    }
    if let Some(cut) = a.cut {
        if cut.iter().any(|c| c.quantity == 0) {
            return Err(ToolError::new(
                "Each cutting line needs a quantity of 1 or more.",
            ));
        }
        commands.push(Command::SetCut {
            piece: a.piece.clone(),
            cut: cut.into_iter().map(Into::into).collect(),
        });
    }
    if commands.is_empty() {
        return Err(ToolError::new(
            "Nothing to change: give name, seam_allowance or cut.",
        ));
    }
    session.dispatch(commands)?;
    get_piece(session, PieceArg { piece: a.piece })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UpdateEdge {
    /// The piece's id.
    piece: String,
    /// The edge's id (get_piece lists them; render_preview can write them).
    edge: String,
    /// "seam", "hem" or "fold". A fold edge gets no allowance and must be
    /// straight.
    kind: Option<EdgeKind>,
    /// A name such as "centre front" or "side seam".
    name: Option<String>,
    /// Seam allowance on this edge only, mm.
    allowance_mm: Option<f64>,
    /// Drop this edge's own allowance, so it uses the piece's again.
    #[serde(default)]
    use_piece_allowance: bool,
    /// Shape of the allowance at the corner where this edge ends and the next
    /// begins.
    corner: Option<CornerStyle>,
}

fn update_edge(session: &mut Session, a: UpdateEdge) -> Result<Output, ToolError> {
    let (piece, edge) = (a.piece.clone(), a.edge.clone());
    let mut commands = Vec::new();
    if let Some(kind) = a.kind {
        commands.push(Command::SetEdgeKind {
            piece: piece.clone(),
            edge: edge.clone(),
            kind,
        });
    }
    if let Some(name) = a.name {
        commands.push(Command::RenameEdge {
            piece: piece.clone(),
            edge: edge.clone(),
            name: name.trim().to_string(),
        });
    }
    if let Some(mm) = a.allowance_mm {
        check_length("The allowance", mm, 150.0)?;
        commands.push(Command::SetEdgeAllowance {
            piece: piece.clone(),
            edge: edge.clone(),
            allowance: Some(mm),
        });
    } else if a.use_piece_allowance {
        commands.push(Command::SetEdgeAllowance {
            piece: piece.clone(),
            edge: edge.clone(),
            allowance: None,
        });
    }
    if let Some(style) = a.corner {
        commands.push(Command::SetCornerStyle {
            piece: piece.clone(),
            edge: edge.clone(),
            style,
        });
    }
    if commands.is_empty() {
        return Err(ToolError::new(
            "Nothing to change: give kind, name, allowance_mm, use_piece_allowance or corner.",
        ));
    }
    session.dispatch(commands)?;
    get_piece(session, PieceArg { piece })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SplitArgs {
    /// The piece's id.
    piece: String,
    /// The edge's id.
    edge: String,
    /// Where the new corner goes, mm along the edge from its start.
    at_mm: f64,
}

fn edge_ids(session: &Session, piece: &str) -> Result<Vec<Id>, ToolError> {
    Ok(session
        .project()?
        .engine
        .document()
        .piece(piece)
        .ok_or_else(|| unknown_piece(piece))?
        .outline
        .edges
        .iter()
        .map(|e| e.id.clone())
        .collect())
}

fn split_edge(session: &mut Session, a: SplitArgs) -> Result<Output, ToolError> {
    let before = edge_ids(session, &a.piece)?;
    session.dispatch(vec![Command::SplitEdge {
        piece: a.piece.clone(),
        edge: a.edge.clone(),
        at: a.at_mm,
    }])?;
    let after = edge_ids(session, &a.piece)?;
    let new: Vec<&Id> = after.iter().filter(|e| !before.contains(e)).collect();
    let mut out = Output::text(format!(
        "Split {} at {}. New edge ids: {}.",
        a.edge,
        text::length(a.at_mm),
        new.iter()
            .map(|e| e.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ));
    out.content
        .extend(get_piece(session, PieceArg { piece: a.piece })?.content);
    Ok(out)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct EdgeArgs {
    /// The piece's id.
    piece: String,
    /// The edge's id: the corner at its end is removed.
    edge: String,
}

fn join_edges(session: &mut Session, a: EdgeArgs) -> Result<Output, ToolError> {
    session.dispatch(vec![Command::JoinEdges {
        piece: a.piece.clone(),
        edge: a.edge,
    }])?;
    get_piece(session, PieceArg { piece: a.piece })
}

#[derive(Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Along {
    Vertical,
    Horizontal,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct GrainArgs {
    /// The piece's id.
    piece: String,
    /// Through the middle of the piece: "vertical" (along its length as
    /// drawn) or "horizontal". Or give from and to instead.
    along: Option<Along>,
    /// One end, [x, y] mm in the piece's own frame.
    from: Option<[f64; 2]>,
    /// The other end.
    to: Option<[f64; 2]>,
    /// Napped or one-way fabric: the top of the fabric is towards from.
    #[serde(default)]
    one_way: bool,
    /// Remove the grainline.
    #[serde(default)]
    remove: bool,
}

fn set_grainline(session: &mut Session, a: GrainArgs) -> Result<Output, ToolError> {
    let piece = session
        .project()?
        .engine
        .document()
        .piece(&a.piece)
        .cloned()
        .ok_or_else(|| unknown_piece(&a.piece))?;
    let grainline = if a.remove {
        None
    } else {
        let (from, to) = match (a.along, a.from, a.to) {
            (_, Some(f), Some(t)) => (Pt::new(f[0], f[1]), Pt::new(t[0], t[1])),
            (Some(along), _, _) => {
                let b = piece.outline.bounds();
                let c = b.center();
                match along {
                    Along::Vertical => (
                        Pt::new(c.x, b.y0 + b.height() * 0.2),
                        Pt::new(c.x, b.y1 - b.height() * 0.2),
                    ),
                    Along::Horizontal => (
                        Pt::new(b.x0 + b.width() * 0.2, c.y),
                        Pt::new(b.x1 - b.width() * 0.2, c.y),
                    ),
                }
            }
            _ => {
                return Err(ToolError::new(
                    "Give along (\"vertical\" or \"horizontal\"), or from and to, or remove: true.",
                ));
            }
        };
        if Point::from(from).distance(Point::from(to)) < 1.0 {
            return Err(ToolError::new(
                "The grainline's ends must be at least 1 mm apart.",
            ));
        }
        Some(Grainline {
            from,
            to,
            one_way: a.one_way,
        })
    };
    session.dispatch(vec![Command::SetGrainline {
        piece: a.piece.clone(),
        grainline,
    }])?;
    get_piece(session, PieceArg { piece: a.piece })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct NotchArgs {
    /// The piece's id.
    piece: String,
    /// The edge's id.
    edge: String,
    /// Distance along the edge, mm.
    at_mm: f64,
    /// Measured from the edge's "start" (default) or "end".
    from: Option<NotchFrom>,
    /// "single" (default), "double" (usually back pieces) or "triple".
    kind: Option<NotchKind>,
}

fn add_notch(session: &mut Session, a: NotchArgs) -> Result<Output, ToolError> {
    let notches = |s: &Session| -> Result<Vec<Id>, ToolError> {
        Ok(s.project()?
            .engine
            .document()
            .piece(&a.piece)
            .ok_or_else(|| unknown_piece(&a.piece))?
            .notches
            .iter()
            .map(|n| n.id.clone())
            .collect())
    };
    let before = notches(session)?;
    session.dispatch(vec![Command::AddNotch {
        piece: a.piece.clone(),
        edge: a.edge.clone(),
        at: a.at_mm,
        from: a.from.unwrap_or(NotchFrom::Start),
        kind: a.kind.unwrap_or(NotchKind::Single),
    }])?;
    let added: Vec<Id> = notches(session)?
        .into_iter()
        .filter(|n| !before.contains(n))
        .collect();
    let mut out = Output::text(format!("Added notch {}.", added.join(", ")));
    out.content.extend(
        get_piece(
            session,
            PieceArg {
                piece: a.piece.clone(),
            },
        )?
        .content,
    );
    Ok(out)
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct DeleteNotchArgs {
    /// The piece's id.
    piece: String,
    /// The notch's id.
    notch: String,
}

fn delete_notch(session: &mut Session, a: DeleteNotchArgs) -> Result<Output, ToolError> {
    session.dispatch(vec![Command::DeleteNotch {
        piece: a.piece.clone(),
        notch: a.notch,
    }])?;
    get_piece(session, PieceArg { piece: a.piece })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RectArg {
    width_mm: f64,
    height_mm: f64,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct CreateArgs {
    /// The piece's name.
    name: String,
    /// A rectangle this wide and tall, mm.
    rectangle: Option<RectArg>,
    /// Or the outline as SVG path data in millimetres, y down, one closed
    /// shape: "M0,0 L200,0 L200,300 C150,320 50,320 0,300 Z".
    path: Option<String>,
    /// Seam allowance. Default: 15 mm added outside the outline.
    seam_allowance: Option<AllowanceArg>,
    /// How to cut it, one entry per fabric.
    cut: Option<Vec<CutArg>>,
}

fn create_piece(session: &mut Session, a: CreateArgs) -> Result<Output, ToolError> {
    let svg = match (&a.rectangle, &a.path) {
        (Some(r), None) => {
            check_length("The width", r.width_mm, 5000.0)?;
            check_length("The height", r.height_mm, 5000.0)?;
            format!(
                "M0,0 L{w},0 L{w},{h} L0,{h} Z",
                w = r.width_mm,
                h = r.height_mm
            )
        }
        (None, Some(p)) => p.clone(),
        _ => return Err(ToolError::new("Give either rectangle or path.")),
    };
    let bez = BezPath::from_svg(&svg)
        .map_err(|e| ToolError::new(format!("The path could not be read: {e}")))?;
    let subpaths = bez
        .elements()
        .iter()
        .filter(|e| matches!(e, kurbo::PathEl::MoveTo(_)))
        .count();
    if subpaths != 1 {
        return Err(ToolError::new(
            "The path must be one closed shape (a single M … Z).",
        ));
    }
    let segs: Vec<kurbo::PathSeg> = bez.segments().collect();
    let closed = match (segs.first(), segs.last()) {
        (Some(first), Some(last)) => first.start().distance(last.end()) < 0.01,
        _ => false,
    };
    if !closed {
        return Err(ToolError::new(
            "The path must end where it starts (close it with Z).",
        ));
    }
    let mut ids = counter("");
    let outline = outline_from_path(&segs, DEFAULT_CORNER_ANGLE, &mut ids)
        .ok_or_else(|| ToolError::new("That path doesn't make a piece outline."))?;
    let name = a.name.trim();
    if name.is_empty() {
        return Err(ToolError::new("A piece needs a name."));
    }
    let mut piece = Piece::new("new", name, outline);
    normalize_orientation(&mut piece);
    if let Some(s) = &a.seam_allowance {
        check_length("The seam allowance", s.width_mm, 100.0)?;
        piece.allowance.mode = if s.included {
            AllowanceMode::Included
        } else {
            AllowanceMode::Add
        };
        piece.allowance.seam = s.width_mm;
    }
    if let Some(cut) = a.cut {
        piece.cut = cut.into_iter().map(Into::into).collect();
    }
    piece.source = Some(Source {
        format: SourceFormat::Drafted,
        file: None,
        sha256: None,
        reference: None,
        scale: None,
    });
    if !session.is_open() {
        session.create("Untitled pattern", false)?;
    }
    let added = add_pieces(session, vec![piece])?;
    let id = added[0].id.clone();
    get_piece(session, PieceArg { piece: id })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PiecesArg {
    /// Ids of the pieces.
    pieces: Vec<String>,
}

fn delete_pieces(session: &mut Session, a: PiecesArg) -> Result<Output, ToolError> {
    if a.pieces.is_empty() {
        return Err(ToolError::new("Say which pieces to delete."));
    }
    session.dispatch(vec![Command::DeletePieces {
        ids: a.pieces.clone(),
    }])?;
    Ok(Output::text(format!(
        "Deleted {}. undo brings them back.",
        a.pieces.join(", ")
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MoveArgs {
    /// Ids of the pieces.
    pieces: Vec<String>,
    /// Rightwards, mm.
    dx_mm: f64,
    /// Downwards, mm.
    dy_mm: f64,
}

fn move_pieces(session: &mut Session, a: MoveArgs) -> Result<Output, ToolError> {
    session.dispatch(vec![Command::MovePieces {
        ids: a.pieces.clone(),
        dx: a.dx_mm,
        dy: a.dy_mm,
    }])?;
    Ok(Output::text(format!(
        "Moved {} by {}, {} mm.",
        a.pieces.join(", "),
        r1(a.dx_mm),
        r1(a.dy_mm)
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct RotateArgs {
    /// The piece's id.
    piece: String,
    /// Clockwise, degrees.
    degrees: f64,
}

fn rotate_piece(session: &mut Session, a: RotateArgs) -> Result<Output, ToolError> {
    if !a.degrees.is_finite() {
        return Err(ToolError::new("Give the turn in degrees."));
    }
    session.dispatch(vec![Command::RotatePiece {
        piece: a.piece.clone(),
        degrees: a.degrees,
    }])?;
    Ok(Output::text(format!(
        "Turned {} by {}°.",
        a.piece,
        r1(a.degrees)
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ArrangeArgs {
    /// Ids of the pieces to arrange; leave out for all.
    pieces: Option<Vec<String>>,
    /// Widest a row may be, mm (default 1500, a common fabric width).
    width_mm: Option<f64>,
    /// Space between pieces, mm (default 30).
    gap_mm: Option<f64>,
}

fn arrange_pieces(session: &mut Session, a: ArrangeArgs) -> Result<Output, ToolError> {
    let width = a.width_mm.unwrap_or(1500.0);
    let gap = a.gap_mm.unwrap_or(30.0);
    check_length("The width", width, 100_000.0)?;
    check_length("The gap", gap, 1000.0)?;
    let project = session.project_mut()?;
    let ids: Vec<Id> = match a.pieces {
        Some(ids) if !ids.is_empty() => ids,
        _ => project
            .engine
            .document()
            .pieces
            .iter()
            .map(|p| p.id.clone())
            .collect(),
    };
    let mut boxes = Vec::new();
    for id in ids {
        let (piece, view) = piece_and_view(project, &id)?;
        boxes.push((id, table_bounds(&piece, &view)));
    }
    if boxes.is_empty() {
        return Err(ToolError::new("The project has no pieces yet."));
    }
    // Tallest first, in rows; the first row starts where the first piece is.
    let origin = boxes
        .iter()
        .fold(Point::new(f64::MAX, f64::MAX), |o, (_, b)| {
            Point::new(o.x.min(b.x0), o.y.min(b.y0))
        });
    boxes.sort_by(|a, b| b.1.height().total_cmp(&a.1.height()));
    let (mut x, mut y, mut row_height, mut rows, mut widest) = (0.0, 0.0, 0.0_f64, 1, 0.0_f64);
    let mut commands = Vec::new();
    for (id, b) in &boxes {
        if x > 0.0 && x + b.width() > width {
            y += row_height + gap;
            x = 0.0;
            row_height = 0.0;
            rows += 1;
        }
        commands.push(Command::MovePieces {
            ids: vec![id.clone()],
            dx: origin.x + x - b.x0,
            dy: origin.y + y - b.y0,
        });
        x += b.width() + gap;
        widest = widest.max(x - gap);
        row_height = row_height.max(b.height());
    }
    session.dispatch(commands)?;
    Ok(Output::text(format!(
        "Arranged {} pieces in {rows} row{}: {} wide and {} long.",
        boxes.len(),
        if rows == 1 { "" } else { "s" },
        text::length(widest),
        text::length(y + row_height)
    )))
}

fn undo(session: &mut Session, _: NoArgs) -> Result<Output, ToolError> {
    let project = session.project_mut()?;
    let label = project.engine.state().undo_label;
    match (label, project.engine.undo()) {
        (Some(label), Some(_)) => Ok(Output::text(format!("Undid “{label}”."))),
        _ => Err(ToolError::new("There is nothing to undo.")),
    }
}

fn redo(session: &mut Session, _: NoArgs) -> Result<Output, ToolError> {
    let project = session.project_mut()?;
    let label = project.engine.state().redo_label;
    match (label, project.engine.redo()) {
        (Some(label), Some(_)) => Ok(Output::text(format!("Redid “{label}”."))),
        _ => Err(ToolError::new("There is nothing to redo.")),
    }
}

// ---------------------------------------------------------------- printing

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PrintArgs {
    /// "a4", "letter", "a3", "tabloid", or "a0" for a copy shop.
    paper: Option<PaperName>,
    /// "portrait" or "landscape".
    orientation: Option<Orientation>,
    /// "trim": cut off the margin on two sides and butt pages edge to edge.
    /// "overlap": lay each page over the next and match the circles.
    assembly: Option<Assembly>,
    /// Unprinted border on each page, mm.
    margin_mm: Option<f64>,
    /// How far pages overlap with "overlap", mm.
    overlap_mm: Option<f64>,
    /// Print the stitching line (dashed) inside the cutting line.
    stitch_line: Option<bool>,
    /// Print the 10 cm and 4 in test square.
    test_square: Option<bool>,
    /// Print a map of the pages on the first page.
    page_map: Option<bool>,
    /// Ids of the pieces to print.
    pieces: Option<Vec<String>>,
    /// Print every piece again.
    #[serde(default)]
    all_pieces: bool,
}

fn paper_label(paper: Paper) -> String {
    match paper {
        Paper::Named(PaperName::A4) => "A4".into(),
        Paper::Named(PaperName::Letter) => "US Letter".into(),
        Paper::Named(PaperName::A3) => "A3".into(),
        Paper::Named(PaperName::Tabloid) => "Tabloid (11 × 17 in)".into(),
        Paper::Named(PaperName::A0) => "A0 (copy shop)".into(),
        Paper::Custom { .. } => "Custom size".into(),
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        one.into()
    } else {
        many.replace('#', &n.to_string())
    }
}

fn print_settings(session: &mut Session, a: PrintArgs) -> Result<Output, ToolError> {
    let mut print = session.project()?.engine.document().print.clone();
    let before = print.clone();
    if let Some(paper) = a.paper {
        print.paper = Paper::Named(paper);
    }
    if let Some(o) = a.orientation {
        print.orientation = o;
    }
    if let Some(assembly) = a.assembly {
        print.assembly = assembly;
    }
    if let Some(m) = a.margin_mm {
        check_length("The margin", m, 50.0)?;
        print.margin = m;
    }
    if let Some(o) = a.overlap_mm {
        check_length("The overlap", o, 100.0)?;
        print.overlap = o;
    }
    if let Some(v) = a.stitch_line {
        print.stitch_line = v;
    }
    if let Some(v) = a.test_square {
        print.test_square = v;
    }
    if let Some(v) = a.page_map {
        print.page_map = v;
    }
    if a.all_pieces {
        print.pieces = None;
    } else if let Some(ids) = a.pieces {
        print.pieces = Some(ids);
    }
    if print != before {
        session.dispatch(vec![Command::SetPrintSettings { print }])?;
    }
    let doc = session.project()?.engine.document().clone();
    let plan = plan_print(&doc).map_err(|e| ToolError::new(e.to_string()))?;
    let printed = plan.pages.iter().filter(|p| p.printed).count();
    let names: Vec<String> = plan
        .placements
        .iter()
        .filter_map(|p| doc.piece(&p.piece).map(|piece| piece.name.clone()))
        .collect();
    let joining = match doc.print.assembly {
        Assembly::Trim => "trim and butt",
        Assembly::Overlap => "overlap",
    };
    let mut out = json!({
        "paper": paper_label(doc.print.paper),
        "orientation": doc.print.orientation,
        "pages": plan.page_count(),
        "pattern_pages": format!("{printed} of a {} × {} grid ({} rows, {} columns)", plan.rows, plan.columns, plan.rows, plan.columns),
        "cover_page": plan.cover,
        "joining": joining,
        "margin_mm": doc.print.margin,
        "stitching_line": doc.print.stitch_line,
        "test_square": doc.print.test_square,
        "page_map": doc.print.page_map,
        "pieces": names,
    });
    if !plan.unprintable.is_empty() {
        out["names_the_pdf_font_cannot_show"] = json!(plan.unprintable);
    }
    Ok(Output::json(&out))
}

/// The words in the PDF, as the app writes them (`web/src/app/print.ts`).
fn pdf_texts(doc: &Document, plan: &PrintPlan) -> PdfTexts {
    let pages = plan.pages.iter().filter(|p| p.printed).count();
    let joining = match doc.print.assembly {
        Assembly::Trim => "trim and butt",
        Assembly::Overlap => "overlap",
    };
    let mut instructions = vec![
        "Print at 100% (“Actual size” or “Scale: 100”), never “Fit to page”, on one side of the paper.".to_string(),
    ];
    if doc.print.test_square {
        instructions.push("Measure the test square before cutting: it must be exactly 10 cm (4 in). If it is not, check the print settings and print again.".into());
    }
    instructions.push(match doc.print.assembly {
        Assembly::Trim => "Trim along the frame on the left and top of each page, except in column 1 and row A. Butt each page against its neighbours so the diamonds and page letters match, then tape them together.".into(),
        Assembly::Overlap => "Trim the left and top margins of each page, except in column 1 and row A. Lay each page over its neighbour up to the dashed line so the circles match, then tape them together.".into(),
    });
    if plan.pages.iter().any(|p| !p.printed) {
        instructions
            .push("Pages with nothing on them are left out; they are dashed on the map.".into());
    }
    let material = |m: &str| match m {
        "main" => "Main fabric".to_string(),
        "lining" => "Lining".into(),
        "interfacing" => "Interfacing".into(),
        "contrast" => "Contrast".into(),
        other => other.into(),
    };
    let piece_lines = doc
        .pieces
        .iter()
        .map(|p| {
            let lines = p
                .cut
                .first()
                .map(|c| {
                    let on_fold = c.on_fold.unwrap_or_else(|| has_fold(p));
                    vec![format!(
                        "Cut {}{} · {}",
                        c.quantity,
                        if on_fold { " on the fold" } else { "" },
                        material(&c.material)
                    )]
                })
                .unwrap_or_default();
            (p.id.clone(), lines)
        })
        .collect();
    PdfTexts {
        title: doc.meta.name.clone(),
        subtitle: format!(
            "{} of {} · {joining}",
            plural(pages, "1 page", "# pages"),
            paper_label(doc.print.paper)
        ),
        instructions,
        square_metric: "10 cm".into(),
        square_imperial: "4 in".into(),
        page_map: "Page map".into(),
        not_printed: "Dashed: not printed (nothing on it)".into(),
        place_on_fold: "Place on fold".into(),
        piece_lines,
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExportArgs {
    /// Where to write the file. `~/` means the home folder.
    path: String,
    /// Replace a file already at that path.
    #[serde(default)]
    overwrite: bool,
}

fn export_pdf_tool(session: &mut Session, a: ExportArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document().clone();
    let plan = plan_print(&doc).map_err(|e| ToolError::new(e.to_string()))?;
    let bytes =
        export_pdf(&doc, &pdf_texts(&doc, &plan)).map_err(|e| ToolError::new(e.to_string()))?;
    let path = expand(&a.path);
    write(&path, &bytes, a.overwrite)?;
    Ok(Output::text(format!(
        "Wrote {} ({}). Print it at 100% and measure the test square before cutting.",
        path.display(),
        plural(plan.page_count(), "1 page", "# pages")
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ExportSvgArgs {
    /// Where to write the file. `~/` means the home folder.
    path: String,
    /// Ids of the pieces to write; leave out for all.
    pieces: Option<Vec<String>>,
    /// Replace a file already at that path.
    #[serde(default)]
    overwrite: bool,
}

fn export_svg_tool(session: &mut Session, a: ExportSvgArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document();
    let pieces: Vec<Piece> = match &a.pieces {
        Some(ids) if !ids.is_empty() => ids
            .iter()
            .map(|id| {
                doc.piece(id)
                    .map(|p| (**p).clone())
                    .ok_or_else(|| unknown_piece(id))
            })
            .collect::<Result<_, _>>()?,
        _ => doc.pieces.iter().map(|p| (**p).clone()).collect(),
    };
    if pieces.is_empty() {
        return Err(ToolError::new("The project has no pieces yet."));
    }
    let svg = export_svg(&pieces, &SvgExportOptions::default());
    let path = expand(&a.path);
    write(&path, svg.as_bytes(), a.overwrite)?;
    Ok(Output::text(format!(
        "Wrote {} ({}).",
        path.display(),
        plural(pieces.len(), "1 piece", "# pieces")
    )))
}
