//! Plain words for what the engine reports, worded like the app
//! (`web/src/strings/en.ts`), so an assistant can pass them on as they are.

use drapewright_io::{ImportNote, ScaleReason};
use drapewright_pattern_core::{AllowanceMode, CutInstruction, Piece, ScaleBasis};

/// Millimetres as a short length: "15 mm" below 1 cm, otherwise centimetres
/// to 0.1 ("1.5 cm", "61.3 cm").
pub fn length(mm: f64) -> String {
    if mm.abs() < 10.0 {
        format!("{} mm", round1(mm))
    } else {
        format!("{} cm", round1(mm / 10.0))
    }
}

/// Rounded to 0.1, without a trailing ".0".
pub fn round1(v: f64) -> f64 {
    let r = (v * 10.0).round() / 10.0;
    if r == 0.0 { 0.0 } else { r }
}

pub fn size(width: f64, height: f64) -> String {
    format!("{} × {} cm", round1(width / 10.0), round1(height / 10.0))
}

pub fn scale_reason(r: ScaleReason) -> &'static str {
    match r {
        ScaleReason::DeclaredUnits => "The file says how big it is.",
        ScaleReason::IllustratorFile => {
            "Made with Adobe Illustrator, which usually measures in points (72 to the inch). Check the size."
        }
        ScaleReason::OldInkscapeFile => {
            "Made with an older Inkscape, which used 90 dots to the inch. Check the size."
        }
        ScaleReason::PaperSize => {
            "The drawing is exactly a paper size at this reading. Check the size."
        }
        ScaleReason::NoUnits => {
            "The file does not say how big it is. Pick the reading where a piece has the size the user expects."
        }
        ScaleReason::NoDrawingUnits => {
            "The drawing does not say which unit it was drawn in. Pick the reading where a piece has the size the user expects."
        }
        ScaleReason::AamaMetric => {
            "This garment CAD file says it is metric, which usually means millimetres. Check the size."
        }
    }
}

pub fn scale_basis(b: ScaleBasis) -> &'static str {
    match b {
        ScaleBasis::DeclaredUnits => "as the file says",
        ScaleBasis::Assumed96Dpi => "96 to the inch (most apps)",
        ScaleBasis::Assumed72Dpi => "72 to the inch (Adobe Illustrator)",
        ScaleBasis::Assumed90Dpi => "90 to the inch (Inkscape before 2017)",
        ScaleBasis::AssumedMm => "millimetres",
        ScaleBasis::AssumedCm => "centimetres",
        ScaleBasis::AssumedInches => "inches",
        ScaleBasis::Measured => "as measured",
    }
}

/// "1.5 cm added outside the outline (hems 4 cm)" or "included: the outline
/// is the cutting line (1 cm)".
pub fn allowance(piece: &Piece) -> String {
    let a = piece.allowance;
    match a.mode {
        AllowanceMode::Add => {
            let hem = a
                .hem
                .filter(|h| (h - a.seam).abs() > 1e-9)
                .map(|h| format!(" (hems {})", length(h)))
                .unwrap_or_default();
            format!("{} added outside the outline{hem}", length(a.seam))
        }
        AllowanceMode::Included => format!(
            "included: the outline is the cutting line ({})",
            length(a.seam)
        ),
    }
}

/// "Cut 2 mirrored on the fold · main".
pub fn cut(c: &CutInstruction, has_fold: bool) -> String {
    let mirrored = c.mirrored.unwrap_or(c.quantity >= 2);
    let on_fold = c.on_fold.unwrap_or(has_fold);
    format!(
        "Cut {}{}{} · {}",
        c.quantity,
        if mirrored && c.quantity >= 2 {
            " mirrored"
        } else {
            ""
        },
        if on_fold { " on the fold" } else { "" },
        c.material
    )
}

/// An import note, with `name` giving piece names for piece ids.
pub fn note(n: &ImportNote, name: &dyn Fn(&str) -> String) -> String {
    let count = |n: u32, one: &str, many: &str| {
        if n == 1 {
            one.to_string()
        } else {
            many.replace('#', &n.to_string())
        }
    };
    match n {
        ImportNote::SeamLineFound { piece, allowance } => format!(
            "{}: the file has both the cutting line and the stitching line, so the allowance ({}) was measured from the file.",
            name(piece),
            length(*allowance)
        ),
        ImportNote::CuttingLineDiffers { piece, max_mm } => format!(
            "{}: the file's cutting line differs from Drapewright's by up to {}, usually at corners.",
            name(piece),
            length(*max_mm)
        ),
        ImportNote::SmallShapesIgnored { count: c } => count(
            *c,
            "Left out 1 small shape (an arrowhead, notch or logo).",
            "Left out # small shapes (arrowheads, notches or logos).",
        ),
        ImportNote::LooseLinesIgnored { count: c } => count(
            *c,
            "Left out 1 line outside the pieces (a title or border).",
            "Left out # lines outside the pieces (titles or borders).",
        ),
        ImportNote::LinesOnOutlineIgnored { count: c } => count(
            *c,
            "Left out 1 line drawn over a piece's outline (a second copy of a seam).",
            "Left out # lines drawn over the pieces' outlines (second copies of seams).",
        ),
        ImportNote::NoClosedShapes => {
            "The file has lines but no closed shapes. Each piece's outline must be one closed shape.".into()
        }
        ImportNote::UnsupportedEntities { count: c } => count(
            *c,
            "Left out 1 part of the drawing Drapewright cannot read yet (such as a hatch).",
            "Left out # parts of the drawing Drapewright cannot read yet (such as hatches).",
        ),
        ImportNote::CurvesTraced { count: c } => count(
            *c,
            "1 curve was stored only as points, so it was joined with straight lines.",
            "# curves were stored only as points, so they were joined with straight lines.",
        ),
        ImportNote::OutlineCrossesItself { piece } => {
            format!("{}: the outline crosses itself.", name(piece))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths_read_naturally() {
        assert_eq!(length(15.0), "1.5 cm");
        assert_eq!(length(6.0), "6 mm");
        assert_eq!(length(9.84), "9.8 mm");
        assert_eq!(size(241.2, 613.0), "24.1 × 61.3 cm");
    }
}
