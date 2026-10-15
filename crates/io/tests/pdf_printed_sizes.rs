//! Sizes nested in one line, as big pattern companies print them: every
//! size's outline drawn alike, in one colour on no layer, told apart only
//! by its number printed along its line, over and over. A piece's number
//! printed by the smallest line is no size. The numbers are printed several
//! times on every page: they are no watermark.

use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::Piece;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

const MM: f64 = 25.4 / 72.0;
const SIZES: [&str; 5] = ["8", "10", "12", "14", "16"];
const PAGES: usize = 4;

/// Size k's outline on page p: its lower left and upper right corners
/// (pt, y up), each size 10 pt out from the last.
fn corners(p: usize, k: usize) -> ((f32, f32), (f32, f32)) {
    let out = 10.0 * k as f32;
    let wide = 150.0 + 20.0 * p as f32;
    (
        (100.0 - out, 100.0 - out),
        (100.0 + wide + out, 600.0 + out),
    )
}

fn text(c: &mut Content, at: (f32, f32), size: f32, words: &str) {
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([1.0, 0.0, 0.0, 1.0, at.0, at.1])
        .show(Str(words.as_bytes()))
        .end_text();
}

/// Each page: one piece in every size, all in one black line, each size's
/// number three times on its right side; the piece's number "4" twice by
/// the smallest. `named`: with the sizes' numbers printed at all.
fn pattern(named: bool) -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    let kids: Vec<Ref> = (0..PAGES).map(|p| Ref::new(10 + 2 * p as i32)).collect();
    pdf.pages(Ref::new(2))
        .kids(kids.iter().copied())
        .count(PAGES as i32);
    pdf.type1_font(Ref::new(5)).base_font(Name(b"Helvetica"));
    for p in 0..PAGES {
        let (page_ref, content_ref) = (Ref::new(10 + 2 * p as i32), Ref::new(11 + 2 * p as i32));
        let mut page = pdf.page(page_ref);
        page.media_box(Rect::new(0.0, 0.0, 500.0, 800.0))
            .parent(Ref::new(2))
            .contents(content_ref);
        page.resources().fonts().pair(Name(b"F1"), Ref::new(5));
        page.finish();
        let mut c = Content::new();
        c.set_line_width(1.0).set_stroke_rgb(0.0, 0.0, 0.0);
        for (k, size) in SIZES.iter().enumerate() {
            let ((x0, y0), (x1, y1)) = corners(p, k);
            c.rect(x0, y0, x1 - x0, y1 - y0).stroke();
            if named {
                // Over the right side, a little to its left, three times up
                // it: "10" is about 8 pt wide at 7 pt.
                let wide = 3.9 * size.len() as f32;
                for y in [200.0, 350.0, 500.0] {
                    text(&mut c, (x1 - wide / 2.0, y - 2.5), 7.0, size);
                }
            }
        }
        // The piece's number by its smallest line, twice.
        let ((_, _), (x1, _)) = corners(p, 0);
        for y in [260.0, 420.0] {
            text(&mut c, (x1 - 2.0, y), 7.0, "4");
        }
        text(&mut c, (130.0, 400.0), 12.0, "FRONT");
        pdf.stream(content_ref, &c.finish());
    }
    pdf.finish()
}

fn of_size<'a>(result: &'a drapewright_io::ImportResult, size: &str) -> Vec<&'a Piece> {
    result
        .pieces
        .iter()
        .filter(|p| {
            result
                .sizes
                .pieces
                .iter()
                .any(|ps| ps.piece == p.id && ps.size == size)
        })
        .collect()
}

#[test]
fn sizes_drawn_alike_are_told_apart_by_the_numbers_on_their_lines() {
    let result = import_pdf(&pattern(true), Some("nested.pdf")).unwrap();
    assert_eq!(result.sizes.names, SIZES, "{:?}", result.notes);
    for (k, size) in SIZES.iter().enumerate() {
        let mut pieces = of_size(&result, size);
        assert_eq!(pieces.len(), PAGES, "size {size}");
        pieces.sort_by(|a, b| a.outline.bounds().x0.total_cmp(&b.outline.bounds().x0));
        for (p, piece) in pieces.iter().enumerate() {
            let ((x0, y0), (x1, y1)) = corners(p, k);
            let b = piece.outline.bounds();
            let (w, h) = (f64::from(x1 - x0) * MM, f64::from(y1 - y0) * MM);
            assert!(
                (b.width() - w).abs() < 0.5 && (b.height() - h).abs() < 0.5,
                "size {size}, page {p}: {} x {}, not {w} x {h}",
                b.width(),
                b.height()
            );
            assert_eq!(piece.name, "Front", "size {size}");
        }
    }
}

#[test]
fn outlines_nested_alike_with_no_sizes_on_them_are_no_sizes() {
    let result = import_pdf(&pattern(false), Some("nested.pdf")).unwrap();
    assert!(result.sizes.names.is_empty(), "{:?}", result.sizes.names);
}
