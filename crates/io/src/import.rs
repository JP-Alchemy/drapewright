//! What every importer returns.

use drapewright_pattern_core::ops::transform;
use drapewright_pattern_core::{AllowanceMode, Id, Piece, ScaleBasis};
use kurbo::Affine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use ts_rs::TS;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ImportError {
    #[error("This file could not be read as {format}: {detail}")]
    Unreadable {
        format: &'static str,
        detail: String,
    },
    #[error(
        "Compressed SVG files (.svgz) are not supported yet. Save it as a plain SVG and try again."
    )]
    CompressedSvg,
}

/// One way of reading the size of a file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScaleOption {
    /// Millimetres per unit of the source file.
    pub mm_per_unit: f64,
    pub basis: ScaleBasis,
}

/// Why the importer chose the scale it did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ScaleReason {
    /// The file states its size in physical units.
    DeclaredUnits,
    /// Made by Adobe Illustrator, which writes points as unitless numbers.
    IllustratorFile,
    /// Made by Inkscape before version 0.92, which used 90 pixels per inch.
    OldInkscapeFile,
    /// The drawing is exactly a standard paper size at this reading.
    PaperSize,
    /// No units and no clues: the web standard of 96 per inch is a guess.
    NoUnits,
    /// A CAD drawing that does not say which unit it measures in.
    NoDrawingUnits,
    /// A garment CAD (AAMA/ASTM) file marked METRIC, usually millimetres.
    AamaMetric,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ScaleInfo {
    /// The reading used for the pieces in the result.
    pub chosen: ScaleOption,
    /// True when the file states its size. Otherwise the user must confirm
    /// or measure before the pieces are used.
    pub certain: bool,
    pub reason: ScaleReason,
    /// Other plausible readings, most likely first.
    pub alternatives: Vec<ScaleOption>,
}

/// Something the user should know about an import. The app turns these
/// into plain-language messages.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(
    rename_all = "camelCase",
    tag = "kind",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum ImportNote {
    /// The file has both the cutting line and the stitching line of a
    /// piece. The stitching line is used, with this allowance.
    SeamLineFound { piece: Id, allowance: f64 },
    /// Drapewright's cutting line for the piece differs from the one in the file
    /// by up to this much, usually at corners.
    CuttingLineDiffers { piece: Id, max_mm: f64 },
    /// Small closed shapes that are not pieces, such as arrowheads,
    /// notches drawn as triangles, or logos.
    SmallShapesIgnored { count: u32 },
    /// Lines outside every piece, such as titles, borders or a scale box.
    LooseLinesIgnored { count: u32 },
    /// The file has lines but no closed shapes, so no pieces were found.
    NoClosedShapes,
    /// Parts of the drawing Drapewright does not read yet (3D faces, hatches…).
    UnsupportedEntities { count: u32 },
    /// Curves stored only as points they pass through, joined by straight
    /// lines: their shape may differ slightly from the original.
    CurvesTraced { count: u32 },
    /// A piece outline crosses itself.
    OutlineCrossesItself { piece: Id },
}

/// Seam allowance the file does not settle. A piece drawn with one outline
/// could have been drawn on either line, and the two differ by the
/// allowance all round, so the user says which rather than Drapewright guessing.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AllowanceQuestion {
    /// Pieces drawn with one outline. Empty when every piece's allowance
    /// was measured from the file.
    pub pieces: Vec<Id>,
    /// Which line the file says those outlines are, when it says. Garment
    /// CAD files mark their cutting lines as such.
    pub suggested: Option<AllowanceMode>,
}

impl AllowanceQuestion {
    /// The question for every piece whose allowance was not measured.
    pub(crate) fn for_unmeasured(
        pieces: &[Piece],
        notes: &[ImportNote],
        suggested: Option<AllowanceMode>,
    ) -> Self {
        let measured = measured(notes);
        let pieces: Vec<Id> = pieces
            .iter()
            .filter(|p| !measured.contains(&p.id))
            .map(|p| p.id.clone())
            .collect();
        let suggested = suggested.filter(|_| !pieces.is_empty());
        Self { pieces, suggested }
    }
}

/// The user's answer to an [`AllowanceQuestion`].
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AllowanceAnswer {
    /// `add`: the outlines are stitching lines and allowance goes around
    /// them. `included`: they are cutting lines with the allowance inside.
    pub mode: AllowanceMode,
    /// Width of the allowance, mm.
    pub width: f64,
}

/// Pieces whose allowance was measured from a cutting and stitching line pair.
fn measured(notes: &[ImportNote]) -> Vec<Id> {
    notes
        .iter()
        .filter_map(|n| match n {
            ImportNote::SeamLineFound { piece, .. } => Some(piece.clone()),
            _ => None,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportResult {
    pub pieces: Vec<Piece>,
    pub scale: ScaleInfo,
    pub allowance: AllowanceQuestion,
    pub notes: Vec<ImportNote>,
    /// SHA-256 of the imported file, hex.
    pub sha256: String,
}

impl ImportResult {
    /// Applies the user's answer to the pieces drawn with one outline, which
    /// settles the question. Pieces whose allowance was measured keep it.
    pub fn answer_allowance(&mut self, answer: AllowanceAnswer) {
        let asked = std::mem::take(&mut self.allowance).pieces;
        for piece in self.pieces.iter_mut().filter(|p| asked.contains(&p.id)) {
            piece.allowance.mode = answer.mode;
            piece.allowance.seam = answer.width;
        }
    }

    /// Re-reads the pieces at another scale: every length found in the
    /// file (geometry, detected allowances, notch positions) is multiplied
    /// by the ratio of the two scales.
    pub fn rescale(&mut self, option: &ScaleOption, confirmed: bool) {
        let factor = option.mm_per_unit / self.scale.chosen.mm_per_unit;
        let same = |a: f64, b: f64| (a - b).abs() <= 1e-12 * a.abs().max(b.abs());
        let detected = measured(&self.notes);
        for piece in &mut self.pieces {
            if !same(factor, 1.0) {
                transform(piece, Affine::scale(factor));
                for notch in &mut piece.notches {
                    notch.at *= factor;
                }
                for mark in &mut piece.marks {
                    mark.size = mark.size.map(|s| s * factor);
                    mark.length = mark.length.map(|l| l * factor);
                }
                if detected.contains(&piece.id) {
                    piece.allowance.seam *= factor;
                    piece.allowance.hem = piece.allowance.hem.map(|h| h * factor);
                    for edge in &mut piece.outline.edges {
                        edge.allowance = edge.allowance.map(|a| a * factor);
                    }
                }
            }
            if let Some(scale) = piece.source.as_mut().and_then(|s| s.scale.as_mut()) {
                scale.mm_per_unit = option.mm_per_unit;
                scale.basis = option.basis;
                scale.confirmed = confirmed;
            }
        }
        for note in &mut self.notes {
            match note {
                ImportNote::SeamLineFound { allowance, .. } => *allowance *= factor,
                ImportNote::CuttingLineDiffers { max_mm, .. } => *max_mm *= factor,
                _ => {}
            }
        }
        if !same(option.mm_per_unit, self.scale.chosen.mm_per_unit) {
            let previous = std::mem::replace(&mut self.scale.chosen, option.clone());
            self.scale
                .alternatives
                .retain(|a| !same(a.mm_per_unit, option.mm_per_unit));
            self.scale.alternatives.insert(0, previous);
        }
        self.scale.certain = self.scale.certain || confirmed;
    }
}

/// Hex SHA-256 of some bytes, for provenance.
pub fn sha256_hex(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Rounds a coordinate to 0.0001 mm. Imports pass through single
/// precision on the way in, so digits beyond this are noise.
pub(crate) fn round_mm(v: f64) -> f64 {
    (v * 1e4).round() / 1e4
}
