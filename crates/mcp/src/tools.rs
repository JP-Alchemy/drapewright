//! The tools an assistant can call. Every change goes through the same
//! commands as the app, as one undo step, so it can be undone and the
//! project stays valid.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, OnceLock};

use drapewright_body::BodyModel;
use drapewright_io::pdf::{PdfTexts, PrintPlan, export_pdf, plan_print};
use drapewright_io::svg::{SvgExportOptions, export_svg};
use drapewright_io::{AllowanceAnswer, ImportResult, PageLeftOut};
use drapewright_pattern_core::build::{DEFAULT_CORNER_ANGLE, counter, outline_from_path};
use drapewright_pattern_core::ops::normalize_orientation;
use drapewright_pattern_core::{
    Allowance, AllowanceMode, CornerStyle, CutInstruction, EdgeKind, Grainline, Id, NotchFrom,
    NotchKind, Piece, Pt, ScaleBasis, Source, SourceFormat,
};
use drapewright_project::document::{Assembly, Document, Orientation, Paper, PaperName};
use drapewright_project::hit::placement_affine;
use drapewright_project::measurements::{
    EarlierMeasurements, MeasurementSet, is_valid_key, vocabulary,
};
use drapewright_project::{Command, Figure, MeasurementKey, PieceView};
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

    pub(crate) fn image(mut self, png: &[u8]) -> Self {
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
            "Reads an SVG, DXF or PDF sewing pattern without adding it: the pieces found, the sizes the file holds, how sure its scale is (with what each reading would make the pieces measure), whether seam allowance must be asked about, and what was left out (for a PDF, which pages were read and which left out, and why). Use it before import_pattern to know which questions to ask the user.",
            inspect_pattern_file,
        ),
        tool(
            "import_pattern",
            "Import pattern",
            Edits,
            "Adds the pieces of an SVG, DXF or PDF pattern to the open project (starting a project if none is open), beside any pieces already there. When the file holds several sizes, doesn't say how big it is, or has pieces drawn with one outline, the matching argument is required: ask the user, never guess. For a PDF, pages chooses the pages to read when the ones read by default are wrong. The original file is kept in the project.",
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
            "piece_labels",
            "Words on the pieces",
            Reads,
            "The words printed on each piece in the PDF it was imported from, biggest first, each with what it seems to be: the piece's number, its cutting (\"Cut 2 on fold\"), an instruction, a size, a page's label, or words printed on most pieces (the pattern's title, the designer's name; listed once at the top). Sizes and page labels are left off unless all is true. namedIt marks the words the import named the piece by. The import names pieces by rules that a title, a translation (big pattern companies print each name in four languages, the translations smaller) or a stray note can fool: read the words, name each piece as the pattern does, in its language, and rename it with update_piece. Ask the user when unsure. Pieces drawn in several sizes carry the same words in each.",
            crate::words::piece_labels,
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
        tool(
            "get_measurements",
            "Get measurements",
            Reads,
            "Each person's body measurements kept in the project (mm) and their age, with the measurements Drapewright knows: their keys, whether taken around or along the body, their usual adult range, and FreeSewing's name for each.",
            get_measurements,
        ),
        tool(
            "set_measurements",
            "Set measurements",
            Edits,
            "Adds a person's measurements, or changes one person's: name, figure (\"woman\" or \"man\"), the date taken, and values in mm by key (see get_measurements; custom keys start with \"x-\"). With measure_again the current values are kept as an earlier measurement first. Values outside the usual range are reported, as they are often in the wrong units.",
            set_measurements,
        ),
        tool(
            "fit_avatar",
            "Fit the avatar",
            Reads,
            "Fits the avatar (the figure patterns will be draped on) to a person's measurements, and says how close it comes to each: the person's, the avatar's and the difference, mm. The avatar is an adult's (25 to 90); from pattern sizes 6 to 30W it matches height, bust, waist and hip within a few mm, and says where it cannot come closer.",
            fit_avatar,
        ),
        tool(
            "drape",
            "Drape on the avatar",
            Reads,
            "Drapes the project's garment on the avatar, as the app's Drape does, and shows it: a picture of the figure wearing it from the front, side and back, and how it fits standing. At the bust, waist and hip: the garment's size round the body, the body's, the ease, how hard the cloth pulls there (N/m, typically and in its hardest-pulled tenth: the app's heatmap runs from pale at 0 to red at 100) and the room it leaves (typically, and the least: 0 is touching); and the band of heights where it pulls hardest (always somewhat where it hangs from: the shoulders, or a skirt's waist). The avatar is fitted to a person's measurements (as fit_avatar) or is the standard figure. Only pieces sewn by seams are draped (sew_edges); facings and linings go inside. With pose, it then moves into that pose; with shows \"pull\" or \"room\" the picture is a heatmap. It takes from a few seconds to a minute.",
            crate::drape::drape,
        ),
        tool(
            "get_seams",
            "Get seams",
            Reads,
            "The seams: which edges are sewn together, each side's length on the stitching line, the difference, and how the notches meet. Also the seams Drapewright suggests for edges not sewn yet, with how likely each is and why; ask the user before sewing them with sew_edges. drawnForOneSide lists pieces that look drawn for one side and cut once (a pattern that draws each panel once): tell the user and set them to cut 2 as mirrored pairs first.",
            get_seams,
        ),
        tool(
            "sew_edges",
            "Sew edges",
            Edits,
            "Sews pairs of edges together (adds seams). Each side is a piece id and edge id, optionally a part of the edge (from_mm, to_mm along its stitching line) or the piece's mirror copy (mirror: true, for a piece cut as a pair sewn to itself at the centre back). reversed: true when one side's start meets the other's end. A side can instead be part of a line drawn inside a piece (line, from_mm, to_mm), where a patch pocket is sewn onto its placement: one seam for each of the pocket's sewn edges. get_seams suggests pockets onto their placements itself.",
            sew_edges,
        ),
        tool(
            "unpick_seams",
            "Unpick seams",
            Edits,
            "Removes seams (the edges are no longer sewn together).",
            unpick_seams,
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

pub(crate) fn unknown_piece(id: &str) -> ToolError {
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

fn read_pattern(path: &str, pages: Option<&[usize]>) -> Result<Read, ToolError> {
    if pages.is_some_and(|p| p.is_empty() || p.contains(&0)) {
        return Err(ToolError::new(
            "pages lists the pages to read, numbered from 1; leave it out to read them all.",
        ));
    }
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
    let pdf = extension == "pdf" || data.starts_with(b"%PDF");
    if pages.is_some() && !pdf {
        return Err(ToolError::new(
            "pages chooses the pages of a PDF; this file is not one.",
        ));
    }
    let result = if pdf {
        drapewright_io::pdf::import_pdf_pages(&data, Some(&file_name), pages)
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
    let count = result.pages.len();
    if let Some(past) = pages.and_then(|p| p.iter().find(|&&n| n > count)) {
        return Err(ToolError::new(format!(
            "{file_name} has {count} pages, so no page {past}."
        )));
    }
    Ok(Read {
        file_name,
        data,
        result,
    })
}

/// A PDF's pages: those read, and those left out and why.
#[derive(Serialize)]
struct PagesReport {
    /// Page numbers (from 1), with the label printed on each when it has
    /// one: "2 (1B)".
    read: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    left_out: Vec<LeftOutPage>,
    note: &'static str,
}

#[derive(Serialize)]
struct LeftOutPage {
    page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    why: &'static str,
}

fn pages_report(r: &ImportResult) -> Option<PagesReport> {
    if r.pages.is_empty() {
        return None;
    }
    let shown = |n: u32, label: &Option<String>| match label {
        Some(l) => format!("{n} ({l})"),
        None => n.to_string(),
    };
    Some(PagesReport {
        read: r
            .pages
            .iter()
            .filter(|p| p.left_out.is_none())
            .map(|p| shown(p.number, &p.label))
            .collect(),
        left_out: r
            .pages
            .iter()
            .filter_map(|p| {
                Some(LeftOutPage {
                    page: p.number,
                    label: p.label.clone(),
                    why: match p.left_out? {
                        PageLeftOut::Instructions => "a page of instructions or other words",
                        PageLeftOut::NotJoined => {
                            "not part of the pattern sheet: it shares no lines with the pages that are (a cover, a legend)"
                        }
                        PageLeftOut::NotChosen => "not among the pages chosen",
                    },
                })
            })
            .collect(),
        note: "If a page left out holds pattern pieces, or a page read holds none, pass pages with the numbers of the pages to read.",
    })
}

#[derive(Serialize)]
struct FileReport {
    file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<PagesReport>,
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
        pages: pages_report(r),
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
    /// For a PDF, the pages to read, numbered from 1. Leave it out to read
    /// them all: pages of instructions, and pages not part of the pattern
    /// sheet, are then left out (the report says which).
    pages: Option<Vec<usize>>,
}

fn inspect_pattern_file(_: &mut Session, a: FileArgs) -> Result<Output, ToolError> {
    Ok(Output::json(&describe_file(&read_pattern(
        &a.path,
        a.pages.as_deref(),
    )?)))
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
    /// For a PDF, the pages to read, numbered from 1, when those read by
    /// default are wrong (inspect_pattern_file says which were read and
    /// which left out, and why).
    pages: Option<Vec<usize>>,
}

#[derive(Serialize)]
struct Imported {
    project: String,
    added: Vec<AddedPiece>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pages: Option<PagesReport>,
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
    } = read_pattern(&a.path, a.pages.as_deref())?;
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
    let pages = pages_report(&result);
    let added = add_pieces(session, result.pieces)?;
    let project = session.project_mut()?;
    project.keep_source(&result.sha256, &file_name, &data);
    Ok(Output::json(&Imported {
        project: project.engine.document().meta.name.clone(),
        added,
        pages,
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
        words: Vec::new(),
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
        about: None,
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

// ------------------------------------------------------------------ seams

/// "Front · side seam" for a seam side.
fn side_label(doc: &Document, side: &drapewright_seams::SeamSide) -> String {
    let piece = doc.piece(&side.piece);
    let edge = piece.and_then(|p| {
        if let Some(id) = &side.line {
            let line = p.lines.iter().find(|l| l.id == *id)?;
            return Some(
                line.label
                    .clone()
                    .unwrap_or_else(|| "line inside it".into()),
            );
        }
        let i = p.outline.edge_index(&side.edge)?;
        let name = &p.outline.edges[i].name;
        Some(if name.is_empty() {
            format!("edge {}", i + 1)
        } else {
            name.clone()
        })
    });
    format!(
        "{}{} · {}",
        piece.map_or("?", |p| p.name.as_str()),
        if side.mirror { " (other half)" } else { "" },
        edge.unwrap_or_else(|| "?".into())
    )
}

fn side_json(doc: &Document, side: &drapewright_seams::SeamSide) -> Value {
    json!({
        "piece": side.piece,
        "edge": side.edge,
        "line": side.line,
        "label": side_label(doc, side),
        "fromMm": side.from.map(r1),
        "toMm": side.to.map(r1),
        "mirror": side.mirror,
    })
}

fn get_seams(session: &mut Session, _: NoArgs) -> Result<Output, ToolError> {
    let engine = &session.project()?.engine;
    let doc = engine.document();
    let reports = engine.seam_reports();
    let seams: Vec<Value> = doc
        .seams
        .iter()
        .zip(&reports)
        .map(|(s, r)| {
            json!({
                "id": s.id,
                "name": s.name,
                "a": side_json(doc, &s.a),
                "b": side_json(doc, &s.b),
                "reversed": s.reversed,
                "confirmed": s.confirmed,
                "aLengthMm": r1(r.a_length),
                "bLengthMm": r1(r.b_length),
                "differenceMm": r1(r.difference),
                "notchesApartMm": r.notches.iter().map(|n| n.apart.map(r1)).collect::<Vec<_>>(),
                "problem": r.problem,
            })
        })
        .collect();
    let suggested: Vec<Value> = engine
        .suggest_seams()
        .iter()
        .map(|s| {
            json!({
                "a": side_json(doc, &s.seam.a),
                "b": side_json(doc, &s.seam.b),
                "reversed": s.seam.reversed,
                "name": s.seam.name,
                "confidence": (s.confidence * 100.0).round() / 100.0,
                "reasons": s.reasons,
            })
        })
        .collect();
    let mut out = json!({ "seams": seams, "suggested": suggested });
    let one_side: Vec<Value> = engine
        .drawn_for_one_side()
        .iter()
        .filter_map(|id| doc.piece(id))
        .map(|p| json!({ "id": p.id, "name": p.name }))
        .collect();
    if !one_side.is_empty() {
        out["drawnForOneSide"] = json!({
            "pieces": one_side,
            "note": "These pieces look drawn for one side of the body and are cut once, so the garment goes only half way round. Tell the user, set each to cut 2 as mirrored pairs (update_piece with cut), then get_seams again: their centres are then sewn to their other halves.",
        });
    }
    Ok(Output::json(&out))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SideArgs {
    /// Piece id.
    piece: String,
    /// Edge id (render_preview with labels "edges" shows them). Leave it
    /// out to give `line` instead.
    #[serde(default)]
    edge: String,
    /// A line drawn inside the piece (a pocket's placement, from
    /// get_piece) sewn instead of an edge: the other side, a pocket's edge,
    /// is sewn onto the piece's face there. Needs from_mm and to_mm.
    line: Option<String>,
    /// Sewn from this far along the edge's stitching line (or the line), mm.
    from_mm: Option<f64>,
    /// Sewn up to this far along, mm.
    to_mm: Option<f64>,
    /// The piece's mirror copy (its other half).
    #[serde(default)]
    mirror: bool,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SeamArgs {
    a: SideArgs,
    b: SideArgs,
    /// a's start meets b's end.
    #[serde(default)]
    reversed: bool,
    /// "Side seam".
    name: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct SewArgs {
    seams: Vec<SeamArgs>,
}

fn sew_edges(session: &mut Session, a: SewArgs) -> Result<Output, ToolError> {
    if a.seams.is_empty() {
        return Err(ToolError::new("Give at least one pair of edges to sew."));
    }
    let side = |s: SideArgs| drapewright_seams::SeamSide {
        piece: s.piece,
        edge: s.edge,
        line: s.line,
        from: s.from_mm,
        to: s.to_mm,
        mirror: s.mirror,
    };
    let seams: Vec<drapewright_seams::Seam> = a
        .seams
        .into_iter()
        .map(|s| drapewright_seams::Seam {
            id: String::new(),
            name: s.name.unwrap_or_default(),
            a: side(s.a),
            b: side(s.b),
            reversed: s.reversed,
            ease: Default::default(),
            confirmed: true,
        })
        .collect();
    let count = seams.len();
    session.dispatch(vec![Command::AddSeams { seams }])?;
    Ok(Output::text(format!(
        "Sewed {count} seam{}. get_seams shows their lengths.",
        if count == 1 { "" } else { "s" }
    )))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct UnpickArgs {
    /// Seam ids, from get_seams.
    seams: Vec<String>,
}

fn unpick_seams(session: &mut Session, a: UnpickArgs) -> Result<Output, ToolError> {
    let count = a.seams.len();
    session.dispatch(vec![Command::DeleteSeams { ids: a.seams }])?;
    Ok(Output::text(format!(
        "Unpicked {count} seam{}.",
        if count == 1 { "" } else { "s" }
    )))
}

// ------------------------------------------------------------ measurements

fn get_measurements(session: &mut Session, _: NoArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document();
    let people: Vec<Value> = doc
        .measurement_sets
        .iter()
        .map(|m| {
            json!({
                "id": m.id,
                "name": m.name,
                "figure": m.figure,
                "measurement": m.revision,
                "takenOn": m.taken_on,
                "age": m.age,
                "valuesMm": m.values,
                "earlierMeasurements": m.previous.len(),
            })
        })
        .collect();
    let known: Vec<Value> = vocabulary()
        .into_iter()
        .map(|m| {
            json!({
                "key": m.key.name(),
                "kind": m.kind,
                "womenOnly": m.women_only,
                "usualMm": [m.usual.0, m.usual.1],
                "freesewing": m.freesewing,
            })
        })
        .collect();
    Ok(Output::json(
        &json!({ "people": people, "measurements": known }),
    ))
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct MeasurementArgs {
    /// Id of the person's measurements to change; leave out to add a person.
    person: Option<String>,
    /// Whose measurements they are ("Me", a client's name).
    name: Option<String>,
    /// "woman" or "man".
    figure: Option<Figure>,
    /// The day they were taken, YYYY-MM-DD.
    taken_on: Option<String>,
    /// Their age in years, for the avatar's shape.
    age: Option<u32>,
    /// Values to set, mm, by key ("bust": 960). Keys from get_measurements,
    /// or custom ones starting with "x-".
    values_mm: Option<std::collections::BTreeMap<String, f64>>,
    /// Keys whose values to clear.
    remove: Option<Vec<String>>,
    /// Keep the current values as an earlier measurement before changing
    /// anything.
    #[serde(default)]
    measure_again: bool,
}

fn set_measurements(session: &mut Session, a: MeasurementArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document();
    let existing = match &a.person {
        Some(id) => Some(doc.measurement_set(id).cloned().ok_or_else(|| {
            ToolError::new(format!(
                "There are no measurements \"{id}\". get_measurements lists them."
            ))
        })?),
        None => None,
    };
    for key in a
        .values_mm
        .iter()
        .flat_map(|v| v.keys())
        .chain(a.remove.iter().flatten())
    {
        if !is_valid_key(key) {
            let keys: Vec<String> = MeasurementKey::ALL.iter().map(|k| k.name()).collect();
            return Err(ToolError::new(format!(
                "\"{key}\" is not a measurement Drapewright knows. Use one of {}, or a custom key starting with \"x-\".",
                keys.join(", ")
            )));
        }
    }
    let mut set = match existing {
        Some(mut set) => {
            if a.measure_again {
                set.previous.insert(
                    0,
                    EarlierMeasurements {
                        revision: set.revision,
                        taken_on: set.taken_on.take(),
                        values: set.values.clone(),
                    },
                );
                set.revision += 1;
            }
            set
        }
        None => MeasurementSet {
            id: String::new(),
            name: "Me".into(),
            figure: Figure::Woman,
            revision: 1,
            taken_on: None,
            age: None,
            values: Default::default(),
            previous: Vec::new(),
        },
    };
    if let Some(name) = a.name {
        set.name = name.trim().to_string();
    }
    if let Some(figure) = a.figure {
        set.figure = figure;
    }
    if let Some(date) = a.taken_on {
        set.taken_on = Some(date);
    }
    if let Some(age) = a.age {
        set.age = Some(age);
    }
    for key in a.remove.iter().flatten() {
        set.values.remove(key);
    }
    let mut unusual = Vec::new();
    for (key, mm) in a.values_mm.into_iter().flatten() {
        if let Some(info) = MeasurementKey::from_name(&key).map(MeasurementKey::info)
            && (mm < info.usual.0 || mm > info.usual.1)
        {
            unusual.push(format!(
                "{key} {} mm (usually {}–{} mm)",
                r1(mm),
                info.usual.0,
                info.usual.1
            ));
        }
        set.values.insert(key, mm);
    }
    let adding = set.id.is_empty();
    session.dispatch(vec![if adding {
        Command::AddMeasurementSet { set }
    } else {
        Command::SetMeasurementSet { set }
    }])?;
    let doc = session.project()?.engine.document();
    let saved = if adding {
        doc.measurement_sets.last()
    } else {
        a.person.as_deref().and_then(|id| doc.measurement_set(id))
    }
    .expect("the set was just written");
    let mut text = format!(
        "{} {}'s measurements ({}, {} values).",
        if adding { "Added" } else { "Updated" },
        saved.name,
        saved.id,
        saved.values.len()
    );
    if !unusual.is_empty() {
        text.push_str(&format!(
            " Unusual, perhaps in the wrong units: {}.",
            unusual.join("; ")
        ));
    }
    Ok(Output::text(text))
}

/// The avatar's body, built into the server and loaded when first wanted.
pub(crate) fn body_model() -> &'static BodyModel {
    static BODY: OnceLock<BodyModel> = OnceLock::new();
    BODY.get_or_init(|| {
        BodyModel::from_bytes(include_bytes!("../../../assets/body/adult.dwbody"))
            .expect("the body file built in")
    })
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AvatarArgs {
    /// Id of the person's measurements (see get_measurements); leave out
    /// for the first person.
    person: Option<String>,
    /// Their age in years (25 to 90); if left out, the age kept with their
    /// measurements, or 35.
    age: Option<f64>,
}

/// A person's measurements for the avatar.
pub(crate) struct ForAvatar {
    pub name: String,
    pub figure: Figure,
    pub woman: bool,
    /// Those the avatar can take: a man's figure has no bust's shape, as
    /// in the app.
    pub values: BTreeMap<String, f64>,
    pub age: f64,
}

/// The measurements of `person` (by id; the first person's if none), and
/// their age (`age`, or the one kept with them, or 35).
pub(crate) fn measurements_for_avatar(
    doc: &Document,
    person: Option<&str>,
    age: Option<f64>,
) -> Result<ForAvatar, ToolError> {
    let set = match person {
        Some(id) => doc.measurement_sets.iter().find(|m| m.id == id),
        None => doc.measurement_sets.first(),
    }
    .ok_or_else(|| {
        ToolError::new(match person {
            Some(id) => {
                format!("There are no measurements \"{id}\". get_measurements lists them.")
            }
            None => "There are no measurements yet: add some with set_measurements.".into(),
        })
    })?;
    let woman = set.figure == Figure::Woman;
    let values = set
        .values
        .iter()
        .filter(|(k, _)| {
            MeasurementKey::from_name(k).is_none_or(|key| woman || !key.info().women_only)
        })
        .map(|(k, v)| (k.clone(), *v))
        .collect();
    let age = age
        .or(set.age.map(f64::from))
        .unwrap_or(35.0)
        .clamp(25.0, 90.0);
    Ok(ForAvatar {
        name: set.name.clone(),
        figure: set.figure,
        woman,
        values,
        age,
    })
}

fn fit_avatar(session: &mut Session, a: AvatarArgs) -> Result<Output, ToolError> {
    let doc = session.project()?.engine.document();
    let ForAvatar {
        name,
        figure,
        woman,
        values,
        age,
    } = measurements_for_avatar(doc, a.person.as_deref(), a.age)?;
    let fitted = drapewright_body::fit(body_model(), &values, woman, age);
    let rows: Vec<Value> = MeasurementKey::ALL
        .iter()
        .filter_map(|key| {
            let name = key.name();
            let difference = fitted.residuals.get(&name)?;
            Some(json!({
                "key": name,
                "personMm": values[&name],
                "avatarMm": round_tenth(values[&name] + difference),
                "differenceMm": round_tenth(*difference),
            }))
        })
        .collect();
    let close = fitted
        .residuals
        .values()
        .filter(|d| d.abs() <= 10.0)
        .count();
    let not_taken: Vec<&String> = values
        .keys()
        .filter(|k| !fitted.residuals.contains_key(*k))
        .collect();
    Ok(Output::json(&json!({
        "person": name,
        "figure": figure,
        "age": age,
        "summary": format!(
            "The avatar matches {close} of {} measurements to within 1 cm.",
            rows.len()
        ),
        "measurements": rows,
        "notOnTheAvatar": not_taken,
    })))
}

fn round_tenth(mm: f64) -> f64 {
    (mm * 10.0).round() / 10.0
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
