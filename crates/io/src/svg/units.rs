//! Working out how big an SVG drawing really is.
//!
//! usvg resolves `viewBox`, `width` and `height` into CSS pixels (96 per
//! inch). When the root element states its size in physical units, those
//! pixels are exact. When it does not, the pixels are a guess: Illustrator
//! writes points (72 per inch) as plain numbers and Inkscape before 0.92
//! used 90 per inch. Clues in the file decide which reading comes first,
//! but only stated units make the scale certain.

use drapewright_pattern_core::ScaleBasis;

use crate::import::{ScaleInfo, ScaleOption, ScaleReason};

const MM_PER_INCH: f64 = 25.4;

fn option(per_inch: f64, basis: ScaleBasis) -> ScaleOption {
    ScaleOption {
        mm_per_unit: MM_PER_INCH / per_inch,
        basis,
    }
}

#[derive(Debug, PartialEq)]
enum Unit {
    Physical,
    Pixels,
    Relative,
}

/// Splits an SVG length like `"210mm"` into its number and unit.
fn parse_length(s: &str) -> Option<(f64, Unit)> {
    let s = s.trim();
    let split = s
        .find(|c: char| c.is_ascii_alphabetic() || c == '%')
        .unwrap_or(s.len());
    let value: f64 = s[..split].trim().parse().ok()?;
    let unit = match s[split..].trim().to_ascii_lowercase().as_str() {
        "mm" | "cm" | "in" | "pt" | "pc" | "q" => Unit::Physical,
        "" | "px" => Unit::Pixels,
        _ => Unit::Relative,
    };
    Some((value, unit))
}

/// The root viewBox as (x, y, width, height).
pub(super) fn view_box(doc: &roxmltree::Document) -> Option<(f64, f64, f64, f64)> {
    let nums: Vec<f64> = doc
        .root_element()
        .attribute("viewBox")?
        .split(|c: char| c.is_whitespace() || c == ',')
        .filter(|p| !p.is_empty())
        .filter_map(|p| p.parse().ok())
        .collect();
    match nums.as_slice() {
        [x, y, w, h] if *w > 0.0 && *h > 0.0 => Some((*x, *y, *w, *h)),
        _ => None,
    }
}

/// Paper sizes in inches, either way round.
const PAPERS: [(f64, f64); 6] = [
    (8.267_716, 11.692_913),  // A4
    (11.692_913, 16.535_433), // A3
    (33.110_236, 46.811_024), // A0
    (8.5, 11.0),              // US Letter
    (11.0, 17.0),             // Tabloid
    (36.0, 48.0),             // Arch E-ish wide format
];

fn matches_paper(size: (f64, f64), per_inch: f64) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() <= 0.005 * b;
    PAPERS.iter().any(|&(w, h)| {
        let (w, h) = (w * per_inch, h * per_inch);
        (close(size.0, w) && close(size.1, h)) || (close(size.0, h) && close(size.1, w))
    })
}

fn inkscape_older_than_092(version: &str) -> bool {
    let mut parts = version
        .split(|c: char| !c.is_ascii_digit())
        .filter(|p| !p.is_empty());
    let major: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(1);
    let minor: u32 = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
    major == 0 && minor < 92
}

pub(super) fn detect_scale(text: &str, doc: &roxmltree::Document) -> ScaleInfo {
    let root = doc.root_element();
    let width = root.attribute("width").and_then(parse_length);
    let height = root.attribute("height").and_then(parse_length);

    let px96 = option(96.0, ScaleBasis::DeclaredUnits);
    if matches!(
        (&width, &height),
        (Some((_, Unit::Physical)), Some((_, Unit::Physical)))
    ) {
        return ScaleInfo {
            chosen: px96,
            certain: true,
            reason: ScaleReason::DeclaredUnits,
            alternatives: Vec::new(),
        };
    }

    let p96 = option(96.0, ScaleBasis::Assumed96Dpi);
    let p72 = option(72.0, ScaleBasis::Assumed72Dpi);
    let p90 = option(90.0, ScaleBasis::Assumed90Dpi);

    let head = &text[..text.len().min(4096)];
    let illustrator = head.contains("Adobe Illustrator");
    let old_inkscape = root
        .attribute(("http://www.inkscape.org/namespaces/inkscape", "version"))
        .is_some_and(inkscape_older_than_092);
    // The drawing's size in its own units: the viewBox, or else width/height.
    let size = view_box(doc)
        .map(|(_, _, w, h)| (w, h))
        .or(match (&width, &height) {
            (Some((w, _)), Some((h, _))) => Some((*w, *h)),
            _ => None,
        });

    let (order, reason) = if illustrator {
        ([p72, p96, p90], ScaleReason::IllustratorFile)
    } else if old_inkscape {
        ([p90, p96, p72], ScaleReason::OldInkscapeFile)
    } else if size.is_some_and(|s| matches_paper(s, 72.0)) {
        ([p72, p96, p90], ScaleReason::PaperSize)
    } else if size.is_some_and(|s| matches_paper(s, 90.0)) {
        ([p90, p96, p72], ScaleReason::PaperSize)
    } else if size.is_some_and(|s| matches_paper(s, 96.0)) {
        ([p96, p72, p90], ScaleReason::PaperSize)
    } else {
        ([p96, p72, p90], ScaleReason::NoUnits)
    };
    let [chosen, a, b] = order;
    ScaleInfo {
        chosen,
        certain: false,
        reason,
        alternatives: vec![a, b],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scale_of(svg: &str) -> ScaleInfo {
        let doc = roxmltree::Document::parse(svg).unwrap();
        detect_scale(svg, &doc)
    }

    #[test]
    fn physical_units_are_certain() {
        let s = scale_of(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="210mm" height="297mm" viewBox="0 0 210 297"/>"#,
        );
        assert!(s.certain);
        assert!((s.chosen.mm_per_unit - 25.4 / 96.0).abs() < 1e-15);
    }

    #[test]
    fn illustrator_points_come_first() {
        let s = scale_of(
            r#"<!-- Generator: Adobe Illustrator 27.0.0 --><svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 400 300"/>"#,
        );
        assert!(!s.certain);
        assert_eq!(s.reason, ScaleReason::IllustratorFile);
        assert_eq!(s.chosen.basis, ScaleBasis::Assumed72Dpi);
    }

    #[test]
    fn a4_in_points_suggests_72() {
        let s =
            scale_of(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 595.28 841.89"/>"#);
        assert_eq!(s.reason, ScaleReason::PaperSize);
        assert_eq!(s.chosen.basis, ScaleBasis::Assumed72Dpi);
    }

    #[test]
    fn old_inkscape_suggests_90() {
        let s = scale_of(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" inkscape:version="0.48.4 r9939" width="800" height="600"/>"#,
        );
        assert_eq!(s.chosen.basis, ScaleBasis::Assumed90Dpi);
    }

    #[test]
    fn plain_pixels_are_a_guess() {
        let s = scale_of(r#"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"/>"#);
        assert!(!s.certain);
        assert_eq!(s.reason, ScaleReason::NoUnits);
        assert_eq!(s.chosen.basis, ScaleBasis::Assumed96Dpi);
        assert_eq!(s.alternatives.len(), 2);
    }
}
