//! A pattern's pages as many indie patterns print them: a page of
//! instructions first, then tiles that each name themselves ("1A") and
//! their neighbours at the edges they join by, a watermark over every page,
//! and each tile drawing the shapes that cross it, clipped to it. Some
//! print a frame inside each tile to trim it at, with the drawing running on
//! past it to the page's edges, under the next tile.

use drapewright_io::pdf::{import_pdf, import_pdf_pages};
use drapewright_io::{ImportNote, PageLeftOut};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

/// A tile's size, pt.
const W: f32 = 500.0;
const H: f32 = 700.0;

type P = (f32, f32);

/// A piece on the sheet: its corners (pt, y down) and its name.
struct Shape {
    points: Vec<P>,
    name: &'static str,
}

fn sheet() -> Vec<Shape> {
    vec![
        // Down column A, across rows 1 and 2.
        Shape {
            points: vec![
                (100.0, 300.0),
                (300.0, 300.0),
                (320.0, 1100.0),
                (80.0, 1100.0),
            ],
            name: "Front",
        },
        // Across row 3, columns A and B.
        Shape {
            points: vec![
                (300.0, 1500.0),
                (800.0, 1520.0),
                (820.0, 1900.0),
                (550.0, 1960.0),
                (280.0, 1880.0),
            ],
            name: "Back",
        },
        // On tile 2B alone.
        Shape {
            points: vec![
                (620.0, 900.0),
                (820.0, 900.0),
                (820.0, 1150.0),
                (620.0, 1150.0),
            ],
            name: "Pocket",
        },
    ]
}

fn text(c: &mut Content, at: P, size: f32, angle: f32, words: &str) {
    let (s, co) = angle.to_radians().sin_cos();
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([co, s, -s, co, at.0, at.1])
        .show(Str(words.as_bytes()))
        .end_text();
}

/// The PDF: a page of instructions, then tiles 1A to 3B row by row.
fn pattern() -> Vec<u8> {
    pattern_trimmed(0.0)
}

/// The same, each tile overlapping its neighbours by `trim` on each side,
/// a frame drawn there to trim it at: each side a line on its own from
/// corner to corner, running on to the next tiles' corners off the page. A
/// line across each of two pieces too.
fn pattern_trimmed(trim: f32) -> Vec<u8> {
    let (pw, ph) = (W - 2.0 * trim, H - 2.0 * trim);
    let tiles: Vec<(usize, usize)> = (0..3).flat_map(|r| (0..2).map(move |c| (c, r))).collect();
    let mut pdf = Pdf::new();
    let pages = tiles.len() + 1;
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2))
        .kids((0..pages).map(|k| Ref::new(10 + 2 * k as i32)))
        .count(pages as i32);
    pdf.type1_font(Ref::new(3)).base_font(Name(b"Helvetica"));
    let page = |pdf: &mut Pdf, k: usize, content: Content| {
        let (id, stream) = (Ref::new(10 + 2 * k as i32), Ref::new(11 + 2 * k as i32));
        let mut p = pdf.page(id);
        p.media_box(Rect::new(0.0, 0.0, W, H))
            .parent(Ref::new(2))
            .contents(stream);
        p.resources().fonts().pair(Name(b"F1"), Ref::new(3));
        p.finish();
        pdf.stream(stream, &content.finish());
    };
    // Every page has a watermark: words over and over, slanted.
    let watermark = |c: &mut Content| {
        for k in 0..3 {
            text(c, (60.0, 120.0 + 220.0 * k as f32), 48.0, 25.0, "FREE");
        }
    };
    // The page of instructions.
    let mut c = Content::new();
    watermark(&mut c);
    for (k, line) in [
        "Print every page at 100 percent scale.",
        "Check the test square measures five centimetres.",
        "Trim each page along its frame line.",
        "Join the pages where their labels say they meet.",
        "Seam allowances are not included in this pattern.",
        "Cut each piece from a single layer of fabric.",
    ]
    .iter()
    .enumerate()
    {
        text(&mut c, (40.0, 600.0 - 20.0 * k as f32), 11.0, 0.0, line);
    }
    page(&mut pdf, 0, c);
    for (k, &(col, row)) in tiles.iter().enumerate() {
        let (ox, oy) = (col as f32 * pw - trim, row as f32 * ph - trim);
        let mut c = Content::new();
        // Clipped to the page; sheet y runs down, the page's up.
        c.save_state();
        c.rect(0.0, 0.0, W, H).clip_nonzero().end_path();
        let at = |p: P| (p.0 - ox, H - (p.1 - oy));
        for shape in sheet() {
            let crosses = shape
                .points
                .iter()
                .any(|p| (ox..ox + W).contains(&p.0) && (oy..oy + H).contains(&p.1))
                || {
                    let (x0, x1) = shape
                        .points
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.0), b.max(p.0)));
                    let (y0, y1) = shape
                        .points
                        .iter()
                        .fold((f32::MAX, f32::MIN), |(a, b), p| (a.min(p.1), b.max(p.1)));
                    x0 < ox + W && x1 > ox && y0 < oy + H && y1 > oy
                };
            if !crosses {
                continue;
            }
            let first = at(shape.points[0]);
            c.move_to(first.0, first.1);
            for &p in &shape.points[1..] {
                let p = at(p);
                c.line_to(p.0, p.1);
            }
            c.close_path().stroke();
        }
        if trim > 0.0 {
            // A line on the front and one on the back, across the joins:
            // two shapes each pair of tiles shares, to say how far they
            // overlap.
            for (a, b) in [
                ((200.0, 400.0), (200.0, 1000.0)),
                ((350.0, 1700.0), (750.0, 1700.0)),
            ] {
                let (a, b) = (at(a), at(b));
                c.move_to(a.0, a.1).line_to(b.0, b.1).stroke();
            }
            // Each side from corner to corner, and on to the neighbouring
            // tiles' corners (pages run up, rows down the sheet).
            let (x0, x1, y0, y1) = (trim, W - trim, trim, H - trim);
            for k in -1..=1 {
                let (dx, dy) = (k as f32 * pw, -k as f32 * ph);
                let across = (0..2).contains(&(col as i32 + k));
                let down = (0..3).contains(&(row as i32 + k));
                let mut sides = Vec::new();
                if across {
                    sides.extend([
                        ((x0 + dx, y0), (x1 + dx, y0)),
                        ((x0 + dx, y1), (x1 + dx, y1)),
                    ]);
                }
                if down {
                    sides.extend([
                        ((x0, y0 + dy), (x0, y1 + dy)),
                        ((x1, y0 + dy), (x1, y1 + dy)),
                    ]);
                }
                for (a, b) in sides {
                    c.move_to(a.0, a.1).line_to(b.0, b.1).stroke();
                }
            }
        }
        c.restore_state();
        // Each piece's name on the tile holding most of it.
        for shape in sheet() {
            let (sx, sy) = shape.points.iter().fold((0.0, 0.0), |(a, b), p| {
                (
                    a + p.0 / shape.points.len() as f32,
                    b + p.1 / shape.points.len() as f32,
                )
            });
            if (ox..ox + W).contains(&sx) && (oy..oy + H).contains(&sy) {
                let p = at((sx - 30.0, sy));
                text(&mut c, p, 24.0, 0.0, shape.name);
            }
        }
        // The edge marks: a triangle at each side that joins another tile,
        // the neighbour's label beside it.
        let label = |col: usize, row: usize| format!("{}{}", row + 1, (b'A' + col as u8) as char);
        let marks = [
            (col + 1 < 2, (W - 12.0, H / 2.0), label(col + 1, row)),
            (row + 1 < 3, (W / 2.0, 12.0), label(col, row + 1)),
        ];
        for (joins, (x, y), name) in marks {
            if !joins {
                continue;
            }
            c.move_to(x - 10.0, y - 10.0)
                .line_to(x + 10.0, y - 10.0)
                .line_to(x, y + 10.0)
                .close_path()
                .stroke();
            text(&mut c, (x - 30.0, y + 14.0), 12.0, 0.0, &name);
        }
        text(&mut c, (20.0, H - 50.0), 36.0, 0.0, &label(col, row));
        watermark(&mut c);
        page(&mut pdf, k + 1, c);
    }
    pdf.finish()
}

#[test]
fn labelled_tiles_join_where_their_labels_say_and_instructions_are_left_out() {
    let result = import_pdf(&pattern(), Some("tiles.pdf")).unwrap();
    let mut names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    // Whole, across the joins; the pocket alone on its tile too; none named
    // after the watermark.
    assert_eq!(names, ["Back", "Front", "Pocket"], "{:?}", result.notes);
    let size = |name: &str| {
        let p = result.pieces.iter().find(|p| p.name == name).unwrap();
        let b = p.outline.bounds();
        (b.width(), b.height())
    };
    let mm = 25.4 / 72.0;
    let (w, h) = size("Front");
    assert!(
        (w - 240.0 * mm).abs() < 1.0 && (h - 800.0 * mm).abs() < 1.0,
        "{w} x {h}"
    );
    let (w, h) = size("Back");
    assert!(
        (w - 540.0 * mm).abs() < 1.0 && (h - 460.0 * mm).abs() < 1.0,
        "{w} x {h}"
    );
    // The page of instructions left out, the tiles used, each by its label.
    assert_eq!(result.pages.len(), 7);
    assert_eq!(result.pages[0].left_out, Some(PageLeftOut::Instructions));
    let labels: Vec<Option<&str>> = result.pages[1..]
        .iter()
        .map(|p| p.label.as_deref())
        .collect();
    assert_eq!(
        labels,
        [
            Some("1A"),
            Some("1B"),
            Some("2A"),
            Some("2B"),
            Some("3A"),
            Some("3B")
        ]
    );
    assert!(result.pages[1..].iter().all(|p| p.left_out.is_none()));
    assert!(
        result
            .notes
            .contains(&ImportNote::PagesLeftOut { count: 1 })
    );
    assert!(result.notes.contains(&ImportNote::PagesJoined { count: 6 }));
}

#[test]
fn chosen_pages_are_the_ones_used() {
    // Rows 1 and 2 only: the back is not on them; the pages not chosen say
    // so.
    let result = import_pdf_pages(&pattern(), None, Some(&[2, 3, 4, 5])).unwrap();
    let mut names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Front", "Pocket"]);
    let left_out: Vec<Option<PageLeftOut>> = result.pages.iter().map(|p| p.left_out).collect();
    assert_eq!(
        left_out,
        [
            Some(PageLeftOut::NotChosen),
            None,
            None,
            None,
            None,
            Some(PageLeftOut::NotChosen),
            Some(PageLeftOut::NotChosen),
        ]
    );
}

#[test]
fn tiles_trimmed_at_a_frame_inside_the_page_join_without_their_frames() {
    // Tiles overlapping by 12 pt (about 4 mm) on each side. Their frames,
    // put together, would be a box round the whole sheet: no piece, and not
    // a title block that the pieces inside it would be taken for.
    let result = import_pdf(&pattern_trimmed(12.0), Some("trimmed.pdf")).unwrap();
    let mut names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Back", "Front", "Pocket"], "{:?}", result.notes);
    assert!(
        !result
            .notes
            .iter()
            .any(|n| matches!(n, ImportNote::KeysIgnored { .. })),
        "{:?}",
        result.notes
    );
    let front = result.pieces.iter().find(|p| p.name == "Front").unwrap();
    let b = front.outline.bounds();
    let mm = 25.4 / 72.0;
    assert!(
        (b.width() - 240.0 * mm).abs() < 1.0 && (b.height() - 800.0 * mm).abs() < 1.0,
        "{} x {}",
        b.width(),
        b.height()
    );
}
