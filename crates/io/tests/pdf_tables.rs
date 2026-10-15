//! What is printed beside the pieces in boxes and is no piece: a key to
//! the pattern's marks set out as a table, a word in each box, and a box
//! of instructions. A piece that is a plain rectangle, with its name and
//! cutting on it, is still a piece; and boxes the size of pages, side by
//! side, are no table.

use drapewright_io::ImportNote;
use drapewright_io::pdf::import_pdf;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

const MM: f32 = 72.0 / 25.4;

fn text(c: &mut Content, at: (f32, f32), size: f32, words: &str) {
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([1.0, 0.0, 0.0, 1.0, at.0, at.1])
        .show(Str(words.as_bytes()))
        .end_text();
}

fn page(draw: impl FnOnce(&mut Content), size: (f32, f32)) -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    pdf.type1_font(Ref::new(5)).base_font(Name(b"Helvetica"));
    let mut page = pdf.page(Ref::new(3));
    page.media_box(Rect::new(0.0, 0.0, size.0, size.1))
        .parent(Ref::new(2))
        .contents(Ref::new(4));
    page.resources().fonts().pair(Name(b"F1"), Ref::new(5));
    page.finish();
    let mut c = Content::new();
    c.set_line_width(0.7).set_stroke_rgb(0.0, 0.0, 0.0);
    draw(&mut c);
    pdf.stream(Ref::new(4), &c.finish());
    pdf.finish()
}

/// A cuff, a key of 4 by 4 boxes 20 by 25 mm, each with a word, and a box
/// of six lines of instructions.
fn sheet() -> Vec<u8> {
    page(
        |c| {
            // The cuff: 60 by 40 mm.
            c.rect(40.0 * MM, 40.0 * MM, 60.0 * MM, 40.0 * MM).stroke();
            text(c, (55.0 * MM, 62.0 * MM), 14.0, "CUFF");
            text(c, (55.0 * MM, 54.0 * MM), 9.0, "Cut 2");
            // The key, each box drawn on its own.
            let words = ["Centre", "Milieu", "Centro", "Mitte"];
            for (row, word) in words.iter().enumerate() {
                for col in 0..4 {
                    let (x, y) = (
                        (140.0 + 20.0 * col as f32) * MM,
                        (40.0 + 25.0 * row as f32) * MM,
                    );
                    c.rect(x, y, 20.0 * MM, 25.0 * MM).stroke();
                    text(c, (x + 2.0 * MM, y + 10.0 * MM), 6.0, word);
                }
            }
            // The instructions: 80 by 60 mm.
            c.rect(40.0 * MM, 120.0 * MM, 80.0 * MM, 60.0 * MM).stroke();
            text(c, (45.0 * MM, 170.0 * MM), 12.0, "PREPARING THE PATTERN");
            for (k, line) in [
                "Cut out the pattern pieces you need for",
                "your view and size, and press them flat",
                "with a warm dry iron before you lay them",
                "out on the fabric as the cutting layout",
                "shows, matching the grainlines to the",
                "selvedge and the fold lines to the fold.",
            ]
            .iter()
            .enumerate()
            {
                text(c, (45.0 * MM, (160.0 - 6.0 * k as f32) * MM), 9.0, line);
            }
        },
        (400.0 * MM, 300.0 * MM),
    )
}

#[test]
fn a_key_set_out_as_a_table_and_a_box_of_instructions_are_no_pieces() {
    let result = import_pdf(&sheet(), Some("key.pdf")).unwrap();
    let names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Cuff"], "{:?}", result.notes);
    assert_eq!(result.pieces[0].cut[0].quantity, 2);
    assert!(
        result
            .notes
            .contains(&ImportNote::TablesIgnored { count: 2 }),
        "{:?}",
        result.notes
    );
}

#[test]
fn boxes_as_big_as_pages_side_by_side_are_no_table() {
    // Six frames 160 by 247 mm in two rows, as a book's tiles are drawn,
    // each holding a piece.
    let pdf = page(
        |c| {
            for row in 0..2 {
                for col in 0..3 {
                    let (x, y) = (
                        (10.0 + 160.0 * col as f32) * MM,
                        (10.0 + 247.0 * row as f32) * MM,
                    );
                    c.rect(x, y, 160.0 * MM, 247.0 * MM).stroke();
                    c.rect(x + 40.0 * MM, y + 40.0 * MM, 60.0 * MM, 100.0 * MM)
                        .stroke();
                }
            }
        },
        (500.0 * MM, 520.0 * MM),
    );
    let result = import_pdf(&pdf, Some("frames.pdf")).unwrap();
    assert!(
        !result
            .notes
            .iter()
            .any(|n| matches!(n, ImportNote::TablesIgnored { .. })),
        "{:?}",
        result.notes
    );
    assert!(!result.pieces.is_empty());
}
