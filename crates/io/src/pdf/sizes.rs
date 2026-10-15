//! Patterns that hold several sizes. Each size's lines are on a layer of
//! its own ("Size 10"), or drawn in a colour (or dash) of its own, often
//! with a legend: a short sample of each line beside its size. Each size
//! is then assembled on its own, so nested size lines are never taken for
//! a cutting line and its stitching line.

use std::collections::HashMap;

use drapewright_pattern_core::geom::{
    distance_to_polyline, flatten_segs, label_point, point_in_polygon, polygon_area,
};
use kurbo::{Point, Rect};

use super::import::{Label, Style};
use crate::assemble::{MIN_PIECE_AREA, RawPath, ascending, bounds};

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

/// A legend's entry as a size, if it is one: "Size 10", "XS", or a size
/// with a note after it ("36 (UK 8)", "10, bust 87 cm").
fn legend_size(text: &str) -> Option<String> {
    size_name(text).or_else(|| {
        let first = text.split(['(', ',', ':']).next()?;
        (first.len() < text.len())
            .then(|| size_name(first))
            .flatten()
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

/// Whether two paths are drawn in the same line.
fn same(a: &Style, b: &Style) -> bool {
    a.stroke == b.stroke && a.dashed == b.dashed && a.width == b.width
}

/// Whether two outlines' boxes are about as big, and about in one place:
/// the same piece in neighbouring sizes.
fn alike(a: &Rect, b: &Rect) -> bool {
    let ratio = a.area() / b.area().max(1e-9);
    let reach = 0.15 * a.width().hypot(a.height());
    (0.6..=1.7).contains(&ratio) && a.center().distance(b.center()) <= reach
}

/// A size's letter or number on its line is this near it, mm.
const ON_LINE: f64 = 2.5;
/// Sizes drawn alike are told apart by their names on their lines: each
/// line's name at least this many times…
const NAMED_TIMES: usize = 3;
/// …and this many times as often as any other size's name on it.
const NAMED_OVER: usize = 2;

/// Sizes nested in fewer line styles than there are sizes, the styles
/// taken in turn (A black, B black dashed, C red, D red dashed, E black
/// again…), or drawn alike with each size's name along its line: each
/// piece's nest holds an outline for each size, smallest first. The lines
/// inside the pieces, drawn once, are every size's.
fn by_nesting(
    paths: &[(RawPath, Style)],
    labels: &[Label],
    styles: &[(Style, usize, f64)],
) -> Option<Sizes> {
    // The outlines in the sizes' styles: (path, outline, area, box).
    let outlines: Vec<(usize, Vec<Point>, f64, Rect)> = paths
        .iter()
        .enumerate()
        .filter(|(_, (r, s))| is_outline(r) && styles.iter().any(|(t, _, _)| same(t, s)))
        .map(|(i, (r, _))| {
            let pts = flatten_segs(&r.segs, 0.5);
            let area = polygon_area(&pts).abs();
            let b = bounds(&pts);
            (i, pts, area, b)
        })
        .collect();
    // Nests: outlines alike, one inside the other.
    let n = outlines.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for a in 0..n {
        for b in a + 1..n {
            let (oa, ob) = (&outlines[a], &outlines[b]);
            if !alike(&oa.3, &ob.3) {
                continue;
            }
            let (small, big) = if oa.2 < ob.2 { (oa, ob) } else { (ob, oa) };
            if !point_in_polygon(label_point(&small.1).0, &big.1) {
                continue;
            }
            let (ra, rb) = (root(&mut parent, a), root(&mut parent, b));
            if ra != rb {
                parent[ra] = rb;
            }
        }
    }
    let mut by_root: Vec<Vec<usize>> = vec![Vec::new(); n];
    for k in 0..n {
        let r = root(&mut parent, k);
        by_root[r].push(k);
    }
    // Each nest smallest first, the nests in drawing order.
    let nests: Vec<Vec<usize>> = by_root
        .into_iter()
        .filter(|v| v.len() >= 2)
        .map(|nest| {
            let order = ascending(&nest.iter().map(|&k| outlines[k].2).collect::<Vec<_>>());
            order.into_iter().map(|x| nest[x]).collect()
        })
        .collect();
    let order = ascending(
        &nests
            .iter()
            .map(|nest: &Vec<usize>| outlines[nest[0]].0 as f64)
            .collect::<Vec<_>>(),
    );
    let nests: Vec<Vec<usize>> = order.into_iter().map(|x| nests[x].clone()).collect();
    // As many sizes as most nests hold.
    let style_of = |k: usize| &paths[outlines[k].0].1;
    let mut lengths: HashMap<usize, usize> = HashMap::new();
    for nest in &nests {
        *lengths.entry(nest.len()).or_default() += 1;
    }
    let count = lengths
        .iter()
        .max_by_key(|(len, times)| (**times, **len))
        .map(|(len, _)| *len)?;
    let whole: Vec<&Vec<usize>> = nests.iter().filter(|nest| nest.len() == count).collect();
    // Only when the styles come round in turn, two or more of them, and
    // again: a cutting line round its stitching line, drawn alike, is no
    // nest of sizes.
    let in_turn = |nest: &&Vec<usize>| {
        let mut kinds: Vec<&Style> = Vec::new();
        for &k in nest.iter() {
            if !kinds.iter().any(|s| same(s, style_of(k))) {
                kinds.push(style_of(k));
            }
        }
        let period = kinds.len();
        period >= 2
            && nest.len() > period
            && nest
                .iter()
                .enumerate()
                .all(|(x, &k)| same(style_of(k), style_of(nest[x % period])))
    };
    if count < 3 {
        return None;
    }
    // Each size's name: the name most often on its line.
    let mut votes: Vec<Vec<(String, usize)>> = vec![Vec::new(); count];
    for nest in &whole {
        for l in labels {
            if l.size > 6.0 || size_name(&l.text).is_none() {
                continue;
            }
            let near = nest
                .iter()
                .enumerate()
                .map(|(rank, &k)| (rank, distance_to_polyline(l.at, &outlines[k].1, true)))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((rank, d)) = near
                && d <= ON_LINE
            {
                let name = size_name(&l.text).unwrap_or_default();
                match votes[rank].iter_mut().find(|(n, _)| *n == name) {
                    Some((_, times)) => *times += 1,
                    None => votes[rank].push((name, 1)),
                }
            }
        }
    }
    let named: Vec<Option<String>> = votes
        .iter()
        .map(|v| {
            v.iter()
                .max_by(|a, b| a.1.cmp(&b.1).then(b.0.cmp(&a.0)))
                .map(|(name, _)| name.clone())
        })
        .collect();
    let distinct = named.iter().enumerate().all(|(x, name)| {
        name.as_ref().is_some_and(|name| {
            named[x + 1..]
                .iter()
                .all(|other| other.as_ref() != Some(name))
        })
    });
    // Sizes drawn alike (all in one line, or in lines that don't come
    // round in turn) are sizes still when each line carries its own size's
    // name, over and over, more than the other sizes' (a piece's number
    // beside its smallest line is no size's): a cutting line round its
    // stitching line carries none.
    let surely_named = distinct
        && votes.iter().zip(&named).all(|(v, name)| {
            let times = |n: &String| v.iter().find(|(m, _)| m == n).map_or(0, |(_, t)| *t);
            let own = name.as_ref().map_or(0, times);
            let others = named
                .iter()
                .flatten()
                .filter(|n| Some(*n) != name.as_ref())
                .map(times)
                .max()
                .unwrap_or(0);
            own >= NAMED_TIMES && own >= NAMED_OVER * others
        });
    if !(whole.iter().all(in_turn) || surely_named) {
        return None;
    }
    let names: Vec<String> = if distinct {
        named.into_iter().flatten().collect()
    } else {
        (1..=count).map(|k| k.to_string()).collect()
    };
    // Each size's line, from a whole nest.
    let first = whole.first()?;
    let swatches = first
        .iter()
        .map(|&k| {
            let s = style_of(k);
            let [r, g, b] = s.stroke.unwrap_or([0, 0, 0]);
            (format!("#{r:02x}{g:02x}{b:02x}"), s.dashed)
        })
        .collect();
    // Each outline's size: its place in its nest, from the smallest (a
    // nest short of some sizes, from its smallest too); everything else
    // drawn in those styles is every size's.
    let mut of_path: Vec<Option<usize>> = vec![None; paths.len()];
    for nest in &nests {
        for (rank, &k) in nest.iter().enumerate().take(count) {
            of_path[outlines[k].0] = Some(rank);
        }
    }
    Some(Sizes {
        names,
        swatches,
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
    if let Some(sizes) = by_nesting(paths, labels, &styles) {
        return Some(sizes);
    }
    // A legend: a short line of a style, outside every outline, with a
    // size just after it. (A key to the lines, "seam allowance", "Pink
    // line", names no sizes.)
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
                .and_then(|l| legend_size(&l.text))
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
    fn legend_entries_name_sizes_only_when_they_read_as_sizes() {
        assert_eq!(legend_size("Size 12").as_deref(), Some("12"));
        assert_eq!(legend_size("36 (UK 8)").as_deref(), Some("36"));
        assert_eq!(legend_size("XS, bust 82 cm").as_deref(), Some("XS"));
        assert_eq!(legend_size("1cm seam allowance"), None);
        assert_eq!(legend_size("Pink line"), None);
        assert_eq!(legend_size("Cutting line (with allowance)"), None);
    }

    #[test]
    fn colour_names() {
        assert_eq!(colour_name([217, 26, 26]), "red");
        assert_eq!(colour_name([26, 140, 38]), "green");
        assert_eq!(colour_name([26, 64, 217]), "blue");
        assert_eq!(colour_name([0, 0, 0]), "black");
    }
}
