//! Where everything goes on paper: pieces laid out on one big sheet, and the
//! sheet cut into a grid of pages.
//!
//! Sheet coordinates are millimetres with y pointing down, starting at the
//! top-left corner of page A1's printable area. Page `(row, column)` shows
//! the part of the sheet at `(column · step_x, row · step_y)`, the size of
//! the printable area. With trimmed pages the step is the printable size,
//! so pages meet edge to edge; with overlapping pages it is shorter by the
//! overlap.

use kurbo::{Affine, Point, Rect};
use serde::{Deserialize, Serialize};
use soom_pattern_core::allowance::allowance_lines;
use soom_pattern_core::geom::point_in_polygon;
use soom_pattern_core::{Id, Piece};
use soom_project::Document;
use soom_project::document::{Assembly, Orientation};
use thiserror::Error;
use ts_rs::TS;

use super::text;

/// Space between pieces laid out automatically, mm.
pub const LAYOUT_GAP: f64 = 10.0;
/// Pieces start this far in from the edges of the sheet, mm.
const SHEET_INSET: f64 = 5.0;
/// How far marks may stand outside a cutting line (V notches), mm. A page
/// this close to a piece counts as having something on it.
const PAGE_REACH: f64 = 6.0;
/// Page columns the automatic layout tries beyond the fewest possible.
const EXTRA_COLUMNS: usize = 6;
/// Smallest printable area: the cover page must hold the 10 cm test square.
pub const MIN_PRINTABLE: f64 = 120.0;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PrintError {
    #[error("There are no pieces to print.")]
    NothingToPrint,
    #[error(
        "The paper is too small for its margins: each page needs at least {min_mm} mm by {min_mm} mm inside the margins."
    )]
    PaperTooSmall { min_mm: f64 },
    #[error("The overlap must be less than half of the printed area of a page.")]
    OverlapTooLarge,
    #[error("A print setting is not a usable number.")]
    InvalidSetting,
}

/// Where a piece goes on the sheet: rotated about its own origin by
/// `rotation` degrees, then moved by `(x, y)`, as on the work table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SheetPlacement {
    pub piece: Id,
    pub x: f64,
    pub y: f64,
    pub rotation: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlannedPage {
    pub row: u32,
    pub column: u32,
    /// Row letter and column number: "A1", "B3".
    pub label: String,
    /// False for a page with nothing on it, when empty pages are skipped.
    pub printed: bool,
}

/// How a project prints: the layout, the page grid and which pages print.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrintPlan {
    /// The paper as it comes out of the printer (orientation applied), mm.
    pub paper_width: f64,
    pub paper_height: f64,
    pub margin: f64,
    /// The part of each page that is printed on, inside the margins, mm.
    pub printable_width: f64,
    pub printable_height: f64,
    /// Distance between neighbouring pages on the sheet, mm: the printable
    /// size, less the overlap when pages overlap.
    pub step_x: f64,
    pub step_y: f64,
    pub rows: u32,
    pub columns: u32,
    pub placements: Vec<SheetPlacement>,
    /// Every page of the grid, row by row.
    pub pages: Vec<PlannedPage>,
    /// Whether a cover page (test square, page map) comes first.
    pub cover: bool,
    /// Names with letters the printed font cannot show yet (they print as
    /// "?").
    pub unprintable: Vec<String>,
}

impl PrintPlan {
    /// Pages in the PDF, the cover included.
    pub fn page_count(&self) -> usize {
        usize::from(self.cover) + self.pages.iter().filter(|p| p.printed).count()
    }

    /// The part of the sheet a page shows, mm.
    pub fn page_rect(&self, row: u32, column: u32) -> Rect {
        let (x, y) = (
            f64::from(column) * self.step_x,
            f64::from(row) * self.step_y,
        );
        Rect::new(x, y, x + self.printable_width, y + self.printable_height)
    }
}

/// A piece ready to print: its lines and where it goes on the sheet.
pub(crate) struct Prepared<'a> {
    pub piece: &'a Piece,
    /// Cutting and stitching lines in piece coordinates (closed polylines).
    pub cutting: Vec<Point>,
    pub stitching: Vec<Point>,
    pub to_sheet: Affine,
}

impl Prepared<'_> {
    pub fn sheet_cutting(&self) -> Vec<Point> {
        self.cutting.iter().map(|p| self.to_sheet * *p).collect()
    }
}

/// Row label in the style of spreadsheet columns: A … Z, AA, AB …
pub(crate) fn row_label(mut row: u32) -> String {
    let mut out = Vec::new();
    loop {
        out.push(b'A' + (row % 26) as u8);
        if row < 26 {
            break;
        }
        row = row / 26 - 1;
    }
    out.reverse();
    String::from_utf8(out).unwrap_or_default()
}

fn bounds(points: &[Point]) -> Rect {
    points
        .iter()
        .fold(Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN), |r, p| {
            Rect::new(r.x0.min(p.x), r.y0.min(p.y), r.x1.max(p.x), r.y1.max(p.y))
        })
}

/// Whether any part of a closed polygon lies inside a rectangle.
pub(crate) fn touches(polygon: &[Point], rect: Rect) -> bool {
    let n = polygon.len();
    let b = bounds(polygon);
    if n == 0 || b.x1 < rect.x0 || b.x0 > rect.x1 || b.y1 < rect.y0 || b.y0 > rect.y1 {
        return false;
    }
    // Any edge reaching into the rectangle (Liang–Barsky).
    let crosses = |a: Point, b: Point| {
        let d = b - a;
        let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
        for (p, q) in [
            (-d.x, a.x - rect.x0),
            (d.x, rect.x1 - a.x),
            (-d.y, a.y - rect.y0),
            (d.y, rect.y1 - a.y),
        ] {
            if p.abs() < 1e-12 {
                if q < 0.0 {
                    return false;
                }
            } else {
                let r = q / p;
                if p < 0.0 {
                    t0 = t0.max(r);
                } else {
                    t1 = t1.min(r);
                }
                if t0 > t1 {
                    return false;
                }
            }
        }
        true
    };
    (0..n).any(|i| crosses(polygon[i], polygon[(i + 1) % n]))
        // Or the rectangle lies wholly inside the polygon.
        || point_in_polygon(rect.center(), polygon)
}

/// Lays out boxes of the given sizes on shelves `width` wide, tallest
/// first, starting at `top`. Returns each box's top-left corner.
fn shelf_pack(sizes: &[(f64, f64)], width: f64, top: f64) -> Vec<Point> {
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by(|&a, &b| {
        sizes[b]
            .1
            .total_cmp(&sizes[a].1)
            .then(sizes[b].0.total_cmp(&sizes[a].0))
            .then(a.cmp(&b))
    });
    let mut out = vec![Point::ZERO; sizes.len()];
    let (mut x, mut y, mut shelf) = (SHEET_INSET, top, 0.0_f64);
    for i in order {
        let (w, h) = sizes[i];
        if x > SHEET_INSET && x + w > width - SHEET_INSET {
            x = SHEET_INSET;
            y += shelf + LAYOUT_GAP;
            shelf = 0.0;
        }
        out[i] = Point::new(x, y);
        x += w + LAYOUT_GAP;
        shelf = shelf.max(h);
    }
    out
}

/// Pages needed across a length of sheet.
fn pages_across(length: f64, printable: f64, step: f64) -> u32 {
    if length <= printable {
        1
    } else {
        ((length - printable) / step - 1e-9).ceil() as u32 + 1
    }
}

struct Grid {
    rows: u32,
    columns: u32,
    printed: Vec<bool>,
}

fn grid(sheet: &[Vec<Point>], printable: (f64, f64), step: (f64, f64), skip_empty: bool) -> Grid {
    let extent = sheet
        .iter()
        .map(|p| bounds(p))
        .fold(Rect::ZERO, |a, b| a.union(b));
    let columns = pages_across(extent.x1 + PAGE_REACH, printable.0, step.0);
    let rows = pages_across(extent.y1 + PAGE_REACH, printable.1, step.1);
    let printed = (0..rows)
        .flat_map(|r| (0..columns).map(move |c| (r, c)))
        .map(|(r, c)| {
            let (x, y) = (f64::from(c) * step.0, f64::from(r) * step.1);
            let rect =
                Rect::new(x, y, x + printable.0, y + printable.1).inflate(PAGE_REACH, PAGE_REACH);
            !skip_empty || sheet.iter().any(|p| touches(p, rect))
        })
        .collect();
    Grid {
        rows,
        columns,
        printed,
    }
}

/// Validates the settings, prepares the pieces to print and lays them out.
pub(crate) fn prepare(doc: &Document) -> Result<(PrintPlan, Vec<Prepared<'_>>), PrintError> {
    let print = &doc.print;
    let (pw, ph) = print.paper.size();
    let (paper_width, paper_height) = match print.orientation {
        Orientation::Portrait => (pw, ph),
        Orientation::Landscape => (ph, pw),
    };
    let numbers = [paper_width, paper_height, print.margin, print.overlap];
    if numbers.iter().any(|n| !n.is_finite()) || print.margin < 0.0 || print.overlap < 0.0 {
        return Err(PrintError::InvalidSetting);
    }
    let printable = (
        paper_width - 2.0 * print.margin,
        paper_height - 2.0 * print.margin,
    );
    if printable.0 < MIN_PRINTABLE || printable.1 < MIN_PRINTABLE {
        return Err(PrintError::PaperTooSmall {
            min_mm: MIN_PRINTABLE,
        });
    }
    let step = match print.assembly {
        Assembly::Trim => printable,
        Assembly::Overlap => {
            if print.overlap * 2.0 >= printable.0.min(printable.1) {
                return Err(PrintError::OverlapTooLarge);
            }
            (printable.0 - print.overlap, printable.1 - print.overlap)
        }
    };

    let mut prepared: Vec<Prepared> = doc
        .pieces
        .iter()
        .filter(|p| print.pieces.as_ref().is_none_or(|ids| ids.contains(&p.id)))
        .filter_map(|piece| {
            let lines = allowance_lines(piece);
            (lines.cutting.len() >= 3).then(|| Prepared {
                piece,
                cutting: lines.cutting,
                stitching: lines.stitching,
                to_sheet: Affine::IDENTITY,
            })
        })
        .collect();
    if prepared.is_empty() {
        return Err(PrintError::NothingToPrint);
    }

    // Placed by hand, or rotated as on the table and laid out on shelves.
    let manual = |id: &Id| {
        print
            .layout
            .as_ref()
            .and_then(|items| items.iter().find(|item| &item.piece == id))
    };
    let mut auto: Vec<usize> = Vec::new();
    let mut bottom = 0.0_f64;
    for (i, p) in prepared.iter_mut().enumerate() {
        if let Some(item) = manual(&p.piece.id) {
            p.to_sheet =
                Affine::translate((item.x, item.y)) * Affine::rotate(item.rotation.to_radians());
            bottom = bottom.max(bounds(&p.sheet_cutting()).y1);
        } else {
            p.to_sheet = Affine::rotate(p.piece.placement.rotation.to_radians());
            auto.push(i);
        }
    }
    let top = if auto.len() < prepared.len() {
        bottom + LAYOUT_GAP
    } else {
        SHEET_INSET
    };
    let boxes: Vec<Rect> = auto
        .iter()
        .map(|&i| bounds(&prepared[i].sheet_cutting()))
        .collect();
    let sizes: Vec<(f64, f64)> = boxes.iter().map(|b| (b.width(), b.height())).collect();
    let widest = sizes.iter().map(|s| s.0).fold(0.0, f64::max);
    let fewest = pages_across(widest + 2.0 * SHEET_INSET, printable.0, step.0) as usize;
    let fixed: Vec<Vec<Point>> = prepared
        .iter()
        .enumerate()
        .filter(|(i, _)| !auto.contains(i))
        .map(|(_, p)| p.sheet_cutting())
        .collect();
    let mut best: Option<(usize, u32, Vec<Point>)> = None;
    for columns in fewest..=fewest + EXTRA_COLUMNS {
        let width = (columns - 1) as f64 * step.0 + printable.0;
        let corners = shelf_pack(&sizes, width, top);
        let mut sheet = fixed.clone();
        for ((&i, b), c) in auto.iter().zip(&boxes).zip(&corners) {
            let shift = Affine::translate((c.x - b.x0, c.y - b.y0));
            sheet.push(
                prepared[i]
                    .sheet_cutting()
                    .iter()
                    .map(|p| shift * *p)
                    .collect(),
            );
        }
        let g = grid(&sheet, printable, step, print.skip_empty_pages);
        let count = g.printed.iter().filter(|p| **p).count();
        let cells = g.rows * g.columns;
        if best
            .as_ref()
            .is_none_or(|(n, c, _)| count < *n || (count == *n && cells < *c))
        {
            best = Some((count, cells, corners));
        }
        if auto.is_empty() {
            break;
        }
    }
    let corners = best.map(|b| b.2).unwrap_or_default();
    for ((&i, b), c) in auto.iter().zip(&boxes).zip(&corners) {
        prepared[i].to_sheet = Affine::translate((c.x - b.x0, c.y - b.y0)) * prepared[i].to_sheet;
    }

    let sheet: Vec<Vec<Point>> = prepared.iter().map(Prepared::sheet_cutting).collect();
    let g = grid(&sheet, printable, step, print.skip_empty_pages);
    let pages = (0..g.rows)
        .flat_map(|r| (0..g.columns).map(move |c| (r, c)))
        .zip(&g.printed)
        .map(|((row, column), printed)| PlannedPage {
            row,
            column,
            label: format!("{}{}", row_label(row), column + 1),
            printed: *printed,
        })
        .collect();
    let placements = prepared
        .iter()
        .map(|p| {
            let c = p.to_sheet.as_coeffs();
            SheetPlacement {
                piece: p.piece.id.clone(),
                x: c[4],
                y: c[5],
                rotation: c[1].atan2(c[0]).to_degrees(),
            }
        })
        .collect();
    let unprintable = std::iter::once(&doc.meta.name)
        .chain(prepared.iter().map(|p| &p.piece.name))
        .filter(|name| !text::encodable(name))
        .cloned()
        .collect();
    let plan = PrintPlan {
        paper_width,
        paper_height,
        margin: print.margin,
        printable_width: printable.0,
        printable_height: printable.1,
        step_x: step.0,
        step_y: step.1,
        rows: g.rows,
        columns: g.columns,
        placements,
        pages,
        cover: print.test_square || print.page_map,
        unprintable,
    };
    Ok((plan, prepared))
}

/// How the document prints, without writing the PDF.
pub fn plan_print(doc: &Document) -> Result<PrintPlan, PrintError> {
    prepare(doc).map(|(plan, _)| plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_labels_run_like_spreadsheet_columns() {
        assert_eq!(row_label(0), "A");
        assert_eq!(row_label(25), "Z");
        assert_eq!(row_label(26), "AA");
        assert_eq!(row_label(27), "AB");
        assert_eq!(row_label(701), "ZZ");
        assert_eq!(row_label(702), "AAA");
    }

    #[test]
    fn pages_across_counts_overlapping_steps() {
        assert_eq!(pages_across(100.0, 190.0, 190.0), 1);
        assert_eq!(pages_across(190.0, 190.0, 190.0), 1);
        assert_eq!(pages_across(190.1, 190.0, 190.0), 2);
        // Overlapping by 15: two pages cover 190 + 175.
        assert_eq!(pages_across(365.0, 190.0, 175.0), 2);
        assert_eq!(pages_across(365.1, 190.0, 175.0), 3);
    }

    #[test]
    fn a_polygon_touches_the_rectangles_it_reaches() {
        let square = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
        ];
        assert!(touches(&square, Rect::new(5.0, 5.0, 20.0, 20.0)));
        assert!(touches(&square, Rect::new(2.0, 2.0, 3.0, 3.0)), "inside");
        assert!(
            touches(&square, Rect::new(-5.0, -5.0, 20.0, 20.0)),
            "around"
        );
        assert!(!touches(&square, Rect::new(11.0, 0.0, 20.0, 10.0)));
        // Crossing the rectangle without a vertex inside it.
        let thin = [
            Point::new(-5.0, 4.0),
            Point::new(25.0, 4.0),
            Point::new(25.0, 6.0),
            Point::new(-5.0, 6.0),
        ];
        assert!(touches(&thin, Rect::new(5.0, 0.0, 15.0, 10.0)));
    }
}
