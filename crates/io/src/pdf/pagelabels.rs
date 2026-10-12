//! Pages labelled for putting together. Each page names itself once ("1B",
//! "A3"), larger than any other label of the kind on it, and may name the
//! pages beside it at its edges, where they join ("2B" at its foot). The
//! labels say where the pages go on the sheet: one part of the label
//! counts the rows, the other the columns, and the names at the edges say
//! which is which and which way they run. The labels are page furniture,
//! not part of the drawing.

use kurbo::Point;

use super::import::Label;

/// An edge name is this near its page's edge, mm.
const EDGE: f64 = 15.0;
/// A page's own label is this much larger than the names at its edges.
const LARGER: f64 = 1.3;

/// Where labelled pages go, and which of their labels are furniture.
#[derive(Debug, PartialEq)]
pub(crate) struct PageLabels {
    /// Each page's own label, as printed.
    pub names: Vec<Option<String>>,
    /// Each page's place on the sheet (column, row), when labelled.
    pub places: Vec<Option<(i64, i64)>>,
    /// Each page's labels that are its own or its edges' names (indices
    /// into its labels).
    pub furniture: Vec<Vec<usize>>,
}

/// A label as one, if it reads as one: a number and a letter either way
/// round ("1B", "B1", "12 C", "A-3"), as (number, letter from 0).
pub(crate) fn tile_label(text: &str) -> Option<(i64, i64)> {
    let t: String = text
        .trim()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '.'))
        .collect();
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    let (number, letter) = if digits > 0 {
        t.split_at(digits)
    } else {
        let letters = t.chars().take_while(char::is_ascii_alphabetic).count();
        let (l, n) = t.split_at(letters);
        (n, l)
    };
    if number.is_empty()
        || number.len() > 2
        || !number.chars().all(|c| c.is_ascii_digit())
        || letter.len() != 1
    {
        return None;
    }
    let c = letter.chars().next()?.to_ascii_uppercase();
    c.is_ascii_uppercase()
        .then(|| (number.parse().ok(), i64::from(c as u8 - b'A')))
        .and_then(|(n, l)| n.map(|n| (n, l)))
}

/// A page's own label: (its index, (number, letter), size).
type Own = (usize, (i64, i64), f64);

/// One page's labels, and its size (mm).
pub(crate) type Page<'a> = (&'a [Label], f64, f64);

/// The pages' labels, if most pages are labelled for putting together.
pub(crate) fn read(pages: &[Page]) -> Option<PageLabels> {
    let n = pages.len();
    // Each page's own label: the largest of the kind on it, by some way.
    let own: Vec<Option<Own>> = pages
        .iter()
        .map(|(labels, w, h)| {
            let inside = |at: Point| (0.0..=*w).contains(&at.x) && (0.0..=*h).contains(&at.y);
            let mut found: Vec<(usize, (i64, i64), f64)> = labels
                .iter()
                .enumerate()
                .filter(|(_, l)| inside(l.at))
                .filter_map(|(k, l)| Some((k, tile_label(&l.text)?, l.size)))
                .collect();
            found.sort_by(|a, b| b.2.total_cmp(&a.2));
            match found.as_slice() {
                [first] => Some(*first),
                [first, second, ..] if first.2 >= LARGER * second.2 => Some(*first),
                _ => None,
            }
        })
        .collect();
    let labelled = own.iter().flatten().count();
    if labelled < 2 || 3 * labelled < 2 * n {
        return None;
    }
    // One size, and each once.
    let mut sizes: Vec<f64> = own.iter().flatten().map(|o| o.2).collect();
    sizes.sort_by(f64::total_cmp);
    let median = sizes[sizes.len() / 2];
    if own
        .iter()
        .flatten()
        .any(|o| (o.2 - median).abs() > 0.25 * median)
    {
        return None;
    }
    let mut keys: Vec<(i64, i64)> = own.iter().flatten().map(|o| o.1).collect();
    keys.sort_unstable();
    keys.dedup();
    if keys.len() < labelled {
        return None;
    }
    // The names at each page's edges: (side, label), and their indices.
    // Sides: 0 left, 1 right, 2 top, 3 bottom.
    let mut furniture: Vec<Vec<usize>> = own
        .iter()
        .map(|o| o.map(|(k, _, _)| vec![k]).unwrap_or_default())
        .collect();
    let mut edges: Vec<Vec<(usize, (i64, i64))>> = vec![Vec::new(); n];
    for (p, (labels, w, h)) in pages.iter().enumerate() {
        for (k, l) in labels.iter().enumerate() {
            let Some(key) = tile_label(&l.text) else {
                continue;
            };
            if own[p].is_some_and(|o| o.0 == k) {
                continue;
            }
            // The other half of a page's own name, printed past its edge
            // for the page beside it: furniture too.
            let off = l.at.x < 0.0 || l.at.x > *w || l.at.y < 0.0 || l.at.y > *h;
            if l.size > median / LARGER * 1.05 {
                continue;
            }
            furniture[p].push(k);
            if off {
                continue;
            }
            let near = [l.at.x, w - l.at.x, l.at.y, h - l.at.y];
            let (side, d) = near
                .iter()
                .enumerate()
                .min_by(|a, b| a.1.total_cmp(b.1))
                .map(|(s, d)| (s, *d))
                .expect("four sides");
            if d <= EDGE {
                edges[p].push((side, key));
            }
        }
    }
    // Which part counts the columns, and which way each runs: as the
    // edge names say, or else as one reads (rows down by number, columns
    // across by letter).
    let page_of = |key: (i64, i64)| own.iter().position(|o| o.is_some_and(|o| o.1 == key));
    let place = |key: (i64, i64), m: (bool, i64, i64)| {
        let (number_is_row, row_way, column_way) = m;
        let (number, letter) = key;
        if number_is_row {
            (letter * column_way, number * row_way)
        } else {
            (number * column_way, letter * row_way)
        }
    };
    let ways = [
        (true, 1, 1),
        (true, 1, -1),
        (true, -1, 1),
        (true, -1, -1),
        (false, 1, 1),
        (false, 1, -1),
        (false, -1, 1),
        (false, -1, -1),
    ];
    let score = |m: (bool, i64, i64)| -> i64 {
        let mut score = 0;
        for (p, sides) in edges.iter().enumerate() {
            let Some(own) = own[p] else { continue };
            let (c, r) = place(own.1, m);
            for &(side, key) in sides {
                if page_of(key).is_none() {
                    continue;
                }
                let expected = match side {
                    0 => (c - 1, r),
                    1 => (c + 1, r),
                    2 => (c, r - 1),
                    _ => (c, r + 1),
                };
                score += if place(key, m) == expected { 1 } else { -1 };
            }
        }
        score
    };
    // Of ways the edges say equally, as one reads: down and across, the
    // number counting rows.
    let best = ways
        .iter()
        .copied()
        .max_by_key(|&m| (score(m), m.1 > 0, m.2 > 0, m.0))
        .expect("ways");
    let places = own.iter().map(|o| o.map(|o| place(o.1, best))).collect();
    Some(PageLabels {
        names: own
            .iter()
            .zip(pages)
            .map(|(o, (labels, _, _))| o.map(|o| labels[o.0].text.trim().to_string()))
            .collect(),
        places,
        furniture,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pdf::import::Naming;
    use kurbo::Vec2;

    fn label(text: &str, x: f64, y: f64, size: f64) -> Label {
        Label {
            text: text.into(),
            at: Point::new(x, y),
            size,
            dir: Vec2::new(1.0, 0.0),
            naming: Naming::Never,
        }
    }

    #[test]
    fn labels_read_either_way_round() {
        assert_eq!(tile_label("1B"), Some((1, 1)));
        assert_eq!(tile_label("B1"), Some((1, 1)));
        assert_eq!(tile_label(" 12 c "), Some((12, 2)));
        assert_eq!(tile_label("A-3"), Some((3, 0)));
        for not in ["Back", "1", "B", "AB1", "123A", "size 4"] {
            assert_eq!(tile_label(not), None, "{not}");
        }
    }

    #[test]
    fn pages_go_where_their_labels_and_edges_say() {
        // Rows by number, columns by letter: 1A 1B over 2A 2B. 1A names
        // 2A at its foot and 1B at its right.
        let a1 = vec![
            label("1A", 12.0, 24.0, 10.0),
            label("2A", 90.0, 265.0, 5.0),
            label("1B", 180.0, 140.0, 5.0),
            label("Front", 90.0, 120.0, 4.0),
        ];
        let b1 = vec![label("1B", 12.0, 24.0, 10.0)];
        let a2 = vec![label("2A", 12.0, 24.0, 10.0)];
        let b2 = vec![label("2B", 12.0, 24.0, 10.0)];
        let pages: Vec<Page> = [&a1, &b1, &a2, &b2]
            .into_iter()
            .map(|l| (l.as_slice(), 185.0, 271.0))
            .collect();
        let read = read(&pages).unwrap();
        assert_eq!(
            read.places,
            [Some((0, 1)), Some((1, 1)), Some((0, 2)), Some((1, 2))]
        );
        assert_eq!(read.names[1].as_deref(), Some("1B"));
        // Its own label and the edge names: not the piece's name.
        assert_eq!(read.furniture[0], [0, 1, 2]);
    }

    #[test]
    fn columns_by_number_when_the_edges_say_so() {
        // A1 B1 side by side: A1 names B1 at its foot, so letters count
        // the rows.
        let a1 = vec![label("A1", 12.0, 24.0, 10.0), label("B1", 90.0, 265.0, 5.0)];
        let b1 = vec![label("B1", 12.0, 24.0, 10.0)];
        let pages: Vec<Page> = [&a1, &b1]
            .into_iter()
            .map(|l| (l.as_slice(), 185.0, 271.0))
            .collect();
        let read = read(&pages).unwrap();
        assert_eq!(read.places, [Some((1, 0)), Some((1, 1))]);
    }

    #[test]
    fn unlabelled_pages_are_not_placed() {
        let one = vec![label("Front", 12.0, 24.0, 10.0)];
        let two = vec![label("1A", 12.0, 24.0, 10.0)];
        let pages: Vec<Page> = [&one, &one, &two]
            .into_iter()
            .map(|l| (l.as_slice(), 185.0, 271.0))
            .collect();
        assert_eq!(read(&pages), None);
    }
}
