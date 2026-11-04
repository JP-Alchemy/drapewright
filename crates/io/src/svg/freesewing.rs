//! FreeSewing's SVG conventions (freesewing.eu). Each piece is a group with
//! an id like `fs-stack-sasha.frontInside-part-sasha.frontInside`. Its
//! stitching line has class `fabric` and its cutting line `fabric sa` (open
//! along a fold). Notches are `<use href="#notch">` (`#bnotch` for back
//! notches, which are double), and a text says how many to cut ("Cut 2
//! mirrored", or untranslated "plugin-annotations:cut,2,…mirrored").

use drapewright_pattern_core::{CutInstruction, NotchKind};

/// The readable name of a FreeSewing piece group, from its id:
/// `fs-stack-sasha.frontInside[-part-…]` gives "Front inside".
pub fn part_name(id: &str) -> Option<String> {
    let rest = id.strip_prefix("fs-stack-")?;
    let stack = rest.split("-part-").next().unwrap_or(rest);
    let part = stack.rsplit('.').next().filter(|p| !p.is_empty())?;
    Some(humanise(part))
}

/// The ids FreeSewing gives its paths and texts, which say nothing.
pub fn is_generated_id(id: &str) -> bool {
    id.strip_prefix("fs-")
        .is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// The name of a line FreeSewing writes text along (its `data-text`):
/// `cathrin:waistLine` gives "Waist line", a plain "pocket opening" stays
/// as it is. None for the marks the import reads otherwise (grainline,
/// fold, hem) and for a text without words.
pub fn line_name(text: &str) -> Option<String> {
    let text = text.trim();
    let key = text.rsplit(':').next().unwrap_or(text).trim();
    let lower = key.to_lowercase();
    let marks = ["grainline", "cutonfold", "hem"];
    if key.is_empty()
        || lower.split_whitespace().any(|w| marks.contains(&w))
        || !key.chars().any(char::is_alphabetic)
    {
        return None;
    }
    Some(humanise(key))
}

/// "frontOutsideAbove" → "Front outside above".
fn humanise(camel: &str) -> String {
    let mut out = String::new();
    for (i, c) in camel.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push(' ');
            out.push(c.to_ascii_lowercase());
        } else if i == 0 {
            out.extend(c.to_uppercase());
        } else {
            out.push(c);
        }
    }
    out
}

/// The kind of notch a `<use>` reference draws, if it is one.
pub fn notch_kind(href: &str) -> Option<NotchKind> {
    match href {
        "#notch" => Some(NotchKind::Single),
        "#bnotch" => Some(NotchKind::Double),
        _ => None,
    }
}

/// Cutting instructions from a piece's text: "Cut 2 mirrored from lining",
/// or the untranslated "plugin-annotations:cut,1,plugin-annotations:onFold,…".
pub fn cut_from_text(text: &str) -> Option<CutInstruction> {
    let lower = text.to_ascii_lowercase();
    // FreeSewing always says what to cut from ("…from fabric").
    if !lower.contains("from") {
        return None;
    }
    let after = &lower[lower.find("cut")? + 3..];
    let quantity: u32 = after
        .split(|c: char| !c.is_ascii_digit())
        .find(|t| !t.is_empty())?
        .parse()
        .ok()?;
    let material = if lower.contains("lining") {
        "lining"
    } else if lower.contains("interfacing") {
        "interfacing"
    } else {
        "main"
    };
    Some(CutInstruction {
        material: material.to_string(),
        quantity,
        mirrored: lower.contains("mirror").then_some(true),
        on_fold: (lower.contains("onfold") || lower.contains("on fold")).then_some(true),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn piece_names_from_group_ids() {
        assert_eq!(
            part_name("fs-stack-sasha.frontInside-part-sasha.frontInside").as_deref(),
            Some("Front inside")
        );
        assert_eq!(
            part_name("fs-stack-library.sleeve").as_deref(),
            Some("Sleeve")
        );
        assert_eq!(part_name("front"), None);
        assert!(is_generated_id("fs-12"));
        assert!(!is_generated_id("fs-stack-sasha.pocket"));
    }

    #[test]
    fn lines_named_by_their_text() {
        assert_eq!(
            line_name("cathrin:waistLine").as_deref(),
            Some("Waist line")
        );
        assert_eq!(
            line_name("pocket opening (do not cut)").as_deref(),
            Some("Pocket opening (do not cut)")
        );
        assert_eq!(line_name("grainline"), None);
        assert_eq!(line_name("cutonfold grainline"), None);
        assert_eq!(line_name("hem"), None);
        assert_eq!(line_name(" 12 "), None);
    }

    #[test]
    fn cutting_instructions_in_either_form() {
        let cut = cut_from_text(
            "plugin-annotations:cut,2,plugin-annotations:mirrored,plugin-annotations:from,plugin-annotations:fabric,",
        )
        .unwrap();
        assert_eq!(
            (cut.quantity, cut.mirrored, cut.on_fold),
            (2, Some(true), None)
        );
        let cut = cut_from_text("Cut 1 on fold from lining").unwrap();
        assert_eq!(cut.quantity, 1);
        assert_eq!(cut.on_fold, Some(true));
        assert_eq!(cut.material, "lining");
        assert!(cut_from_text("Sunday, Oct 11, 2026").is_none());
    }
}
