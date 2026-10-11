//! Patterns that hold several sizes. Each size's lines are on a layer of
//! its own ("Size 10"), or drawn in a colour (or dash) of its own, often
//! with a legend: a short sample of each line beside its size. Each size
//! is then assembled on its own, so nested size lines are never taken for
//! a cutting line and its stitching line.

use drapewright_pattern_core::geom::{flatten_segs, point_in_polygon, polygon_area};
use kurbo::Point;

use super::import::{Label, Style};
use crate::assemble::{MIN_PIECE_AREA, RawPath};

/// How each size's lines are told apart, and which paths draw each size.
#[derive(Debug)]
pub(crate) struct Sizes {
    pub names: Vec<String>,
    /// Each size's line colour (hex) and whether it is dashed, when sizes
    /// are told apart by their lines.
    pub swatches: Vec<(String, bool)>,
    /// For each path: the size it belongs to, or none when all sizes share it.
    pub of_path: Vec<Option<usize>>,
}

/// A layer or legend name as a size, if it reads as one: "Size 10" → "10",
/// "XS", "38", "UK 12", "10/12", "2XL".
pub(crate) fn size_name(name: &str) -> Option<String> {
    let trimmed = name.trim();
    let lower = trimmed.to_lowercase();
    let mut rest = lower.as_str();
    for prefix in [
        "size", "sizes", "taille", "gr.", "größe", "grösse", "maat", "talla", "talle", "tg",
        "taglia",
    ] {
        if let Some(r) = rest.strip_prefix(prefix)
            && r.starts_with([' ', '.', ':', '-', '_'])
        {
            rest = r.trim_start_matches([' ', '.', ':', '-', '_']);
            break;
        }
    }
    let rest = rest.trim();
    if rest.is_empty() || rest.chars().count() > 12 {
        return None;
    }
    let letters = |w: &str| {
        let w = w.trim_start_matches(|c: char| c.is_ascii_digit());
        matches!(
            w,
            "xxs" | "xs" | "s" | "m" | "l" | "xl" | "xxl" | "xxxl" | "xxxxl" | "os"
        ) || (w.len() <= 5 && w.chars().all(|c| c == 'x') && !w.is_empty())
    };
    let number = |w: &str| {
        let w = w.trim_end_matches('w').trim_end_matches('+');
        !w.is_empty()
            && w.len() <= 4
            && w.chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == ',')
    };
    let region = |w: &str| matches!(w, "us" | "uk" | "eu" | "fr" | "it" | "au" | "de" | "rtw");
    let words: Vec<&str> = rest
        .split(|c: char| c.is_whitespace() || c == '/' || c == '-' || c == '_' || c == '–')
        .filter(|w| !w.is_empty())
        .collect();
    let ok = !words.is_empty()
        && words.iter().all(|w| letters(w) || number(w) || region(w))
        && words.iter().any(|w| letters(w) || number(w));
    // A single capital letter names a size in some patterns ("A" to "J").
    let letter = trimmed.len() == 1 && trimmed.chars().all(|c| c.is_ascii_uppercase());
    (ok || letter).then(|| {
        // Keep the name as written, without the word "size".
        let start = trimmed.len() - rest.len().min(trimmed.len());
        let shown = trimmed[start..].trim();
        if shown
            .chars()
            .all(|c| !c.is_alphabetic() || c.is_uppercase())
            || shown.chars().any(char::is_numeric)
        {
            shown.to_uppercase()
        } else {
            shown.to_string()
        }
    })
}

/// Whether a path is a piece-sized closed outline.
fn is_outline(r: &RawPath) -> bool {
    r.closed && polygon_area(&flatten_segs(&r.segs, 0.5)).abs() >= MIN_PIECE_AREA
}

/// A plain name for a line colour: "red", "blue", "grey"…
fn colour_name([r, g, b]: [u8; 3]) -> &'static str {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let spread = f64::from(max - min);
    if spread < 30.0 {
        return match max {
            0..60 => "black",
            60..180 => "grey",
            _ => "light grey",
        };
    }
    let (r, g, b) = (f64::from(r), f64::from(g), f64::from(b));
    // Hue in degrees, from whichever channel is strongest.
    let hue = match (r >= g && r >= b, g >= b) {
        (true, _) => 60.0 * ((g - b) / spread).rem_euclid(6.0),
        (false, true) => 60.0 * ((b - r) / spread + 2.0),
        (false, false) => 60.0 * ((r - g) / spread + 4.0),
    };
    match hue {
        h if !(15.0..345.0).contains(&h) => "red",
        h if h < 45.0 => "orange",
        h if h < 70.0 => "yellow",
        h if h < 160.0 => "green",
        h if h < 200.0 => "turquoise",
        h if h < 255.0 => "blue",
        h if h < 290.0 => "purple",
        _ => "pink",
    }
}

/// The sizes in `paths`, if they hold more than one.
pub(crate) fn find_sizes(
    paths: &[(RawPath, Style)],
    layer_name: &dyn Fn(u32) -> String,
    layer_order: &[u32],
    labels: &[Label],
) -> Option<Sizes> {
    by_layers(paths, layer_name, layer_order).or_else(|| by_lines(paths, labels))
}

/// Sizes on layers named as sizes.
fn by_layers(
    paths: &[(RawPath, Style)],
    layer_name: &dyn Fn(u32) -> String,
    order: &[u32],
) -> Option<Sizes> {
    let mut sized: Vec<(u32, String)> = Vec::new();
    for &layer in order {
        let draws_outline = paths
            .iter()
            .any(|(r, s)| s.layers.contains(&layer) && is_outline(r));
        if draws_outline && let Some(name) = size_name(&layer_name(layer)) {
            sized.push((layer, name));
        }
    }
    if sized.len() < 2 {
        return None;
    }
    let of_path = paths
        .iter()
        .map(|(_, s)| sized.iter().position(|(l, _)| s.layers.contains(l)))
        .collect();
    Some(Sizes {
        names: sized.into_iter().map(|(_, n)| n).collect(),
        swatches: Vec::new(),
        of_path,
    })
}

/// Sizes told apart by their lines: three or more line styles that each
/// draw outlines, or fewer with a legend naming them.
fn by_lines(paths: &[(RawPath, Style)], labels: &[Label]) -> Option<Sizes> {
    // Each line style that draws outlines, with the outlines' total area.
    let mut styles: Vec<(Style, usize, f64)> = Vec::new();
    for (r, s) in paths
        .iter()
        .filter(|(r, s)| is_outline(r) && s.stroke.is_some())
    {
        let area = polygon_area(&flatten_segs(&r.segs, 0.5)).abs();
        let key = Style {
            layers: Vec::new(),
            ..s.clone()
        };
        match styles.iter_mut().find(|(k, _, _)| *k == key) {
            Some((_, n, a)) => {
                *n += 1;
                *a += area;
            }
            None => styles.push((key, 1, area)),
        }
    }
    let same =
        |a: &Style, b: &Style| a.stroke == b.stroke && a.dashed == b.dashed && a.width == b.width;
    // A legend: a short line of a style, outside every outline, with words
    // just after it.
    let outlines: Vec<Vec<Point>> = paths
        .iter()
        .filter(|(r, _)| is_outline(r))
        .map(|(r, _)| flatten_segs(&r.segs, 0.5))
        .collect();
    let legend_name = |style: &Style| -> Option<String> {
        paths.iter().find_map(|(r, s)| {
            if r.closed || !same(s, style) {
                return None;
            }
            let pts = flatten_segs(&r.segs, 0.1);
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            let length = a.distance(b);
            if !(5.0..=60.0).contains(&length)
                || outlines.iter().any(|o| point_in_polygon(a.midpoint(b), o))
            {
                return None;
            }
            let end = if a.x > b.x { a } else { b };
            labels
                .iter()
                .filter(|l| {
                    let d = l.at - end;
                    d.x > 0.0
                        && d.x < 30.0 + l.text.len() as f64 * l.size
                        && d.y.abs() < l.size.max(3.0)
                })
                .min_by(|x, y| (x.at - end).hypot().total_cmp(&(y.at - end).hypot()))
                .map(|l| size_name(&l.text).unwrap_or_else(|| l.text.clone()))
        })
    };
    let named: Vec<Option<String>> = styles.iter().map(|(s, _, _)| legend_name(s)).collect();
    let legend = named.iter().filter(|n| n.is_some()).count() >= 2;
    // Two styles alone are usually a cutting line and a stitching line.
    if styles.len() < 3 && !legend {
        return None;
    }
    if styles.len() < 2 {
        return None;
    }
    // Sizes nest: most outlines of one style have one of each other style
    // around or within them, about as big. Pieces merely drawn in
    // different colours don't.
    if !legend {
        let boxes = |style: &Style| -> Vec<kurbo::Rect> {
            paths
                .iter()
                .filter(|(r, s)| is_outline(r) && same(s, style))
                .map(|(r, _)| crate::assemble::bounds(&flatten_segs(&r.segs, 0.5)))
                .collect()
        };
        let all: Vec<Vec<kurbo::Rect>> = styles.iter().map(|(s, _, _)| boxes(s)).collect();
        let alike = |a: &kurbo::Rect, b: &kurbo::Rect| {
            let ratio = a.area() / b.area().max(1e-9);
            let reach = 0.15 * a.width().hypot(a.height());
            (0.6..=1.7).contains(&ratio) && a.center().distance(b.center()) <= reach
        };
        let nested = all.iter().enumerate().all(|(i, mine)| {
            all.iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .all(|(_, theirs)| {
                    let matched = mine
                        .iter()
                        .filter(|a| theirs.iter().any(|b| alike(a, b)))
                        .count();
                    matched * 2 >= mine.len()
                })
        });
        if !nested {
            return None;
        }
    }
    // Smallest size first, unless a legend gives the order.
    let mut order: Vec<usize> = (0..styles.len()).collect();
    order.sort_by(|&a, &b| {
        let mean = |i: usize| styles[i].2 / styles[i].1 as f64;
        mean(a).total_cmp(&mean(b))
    });
    let names: Vec<String> = order
        .iter()
        .map(|&i| {
            named[i].clone().unwrap_or_else(|| {
                let (s, _, _) = &styles[i];
                let colour = s.stroke.map_or("black", colour_name);
                let mut name = colour.to_string();
                if s.dashed {
                    name.push_str(" dashed");
                }
                let mut chars = name.chars();
                chars
                    .next()
                    .map(|c| c.to_uppercase().chain(chars).collect::<String>() + " line")
                    .unwrap_or_default()
            })
        })
        .collect();
    let swatches = order
        .iter()
        .map(|&i| {
            let (s, _, _) = &styles[i];
            let [r, g, b] = s.stroke.unwrap_or([0, 0, 0]);
            (format!("#{r:02x}{g:02x}{b:02x}"), s.dashed)
        })
        .collect();
    let of_path = paths
        .iter()
        .map(|(r, s)| {
            let size = order.iter().position(|&i| same(&styles[i].0, s))?;
            // Legend samples belong to no size.
            let pts = flatten_segs(&r.segs, 0.1);
            let outside = !r.closed
                && pts.len() >= 2
                && !outlines
                    .iter()
                    .any(|o| point_in_polygon(pts[0].midpoint(pts[pts.len() - 1]), o));
            let short = pts
                .first()
                .zip(pts.last())
                .is_some_and(|(a, b)| a.distance(*b) <= 60.0);
            (!(outside && short)).then_some(size)
        })
        .collect();
    Some(Sizes {
        names,
        swatches,
        of_path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_names() {
        assert_eq!(size_name("Size 10").as_deref(), Some("10"));
        assert_eq!(size_name("size_12").as_deref(), Some("12"));
        assert_eq!(size_name("XS").as_deref(), Some("XS"));
        assert_eq!(size_name("2XL").as_deref(), Some("2XL"));
        assert_eq!(size_name("UK 12").as_deref(), Some("UK 12"));
        assert_eq!(size_name("10/12").as_deref(), Some("10/12"));
        assert_eq!(size_name("Taille 38").as_deref(), Some("38"));
        assert_eq!(size_name("A").as_deref(), Some("A"));
        assert_eq!(size_name("Grainlines"), None);
        assert_eq!(size_name("Text"), None);
        assert_eq!(size_name("Cutting lines"), None);
        assert_eq!(size_name("Layer 1"), None);
    }

    #[test]
    fn colour_names() {
        assert_eq!(colour_name([217, 26, 26]), "red");
        assert_eq!(colour_name([26, 140, 38]), "green");
        assert_eq!(colour_name([26, 64, 217]), "blue");
        assert_eq!(colour_name([0, 0, 0]), "black");
    }
}
