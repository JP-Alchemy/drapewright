//! Words printed on pieces in several languages. A big company's pieces
//! carry the pattern's number and the piece's number, with the piece's
//! name beside them in English and, smaller, in French and Spanish; the
//! translation may sit nearer the number than the name. A Spanish pattern
//! says "Cortar 1 al lomo" (cut 1 on the fold) along the edge to fold.

use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::{EdgeKind, Piece};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

fn text(c: &mut Content, at: (f32, f32), size: f32, words: &str) {
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([1.0, 0.0, 0.0, 1.0, at.0, at.1])
        .show(Str(words.as_bytes()))
        .end_text();
}

/// Text turned to read up the page.
fn text_up(c: &mut Content, at: (f32, f32), size: f32, words: &str) {
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([0.0, 1.0, -1.0, 0.0, at.0, at.1])
        .show(Str(words.as_bytes()))
        .end_text();
}

/// A piece numbered `number` at `at` (pt, y up), with the pattern's title
/// just under the number's line and the name a little above it, its
/// translations, smaller, on the number's line.
fn numbered(c: &mut Content, at: (f32, f32), number: &str, name: &str, translations: &[&str]) {
    text(c, at, 25.0, number);
    let x = at.0 + 25.0;
    text(c, (x, at.1 + 14.0), 11.0, name);
    for (k, t) in translations.iter().enumerate() {
        text(c, (x, at.1 + 6.0 - 7.0 * k as f32), 6.0, t);
    }
    text(c, (x, at.1 - 12.0), 11.0, "V1234");
}

fn sheet() -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    pdf.type1_font(Ref::new(5)).base_font(Name(b"Helvetica"));
    let mut page = pdf.page(Ref::new(3));
    page.media_box(Rect::new(0.0, 0.0, 1200.0, 800.0))
        .parent(Ref::new(2))
        .contents(Ref::new(4));
    page.resources().fonts().pair(Name(b"F1"), Ref::new(5));
    page.finish();
    let mut c = Content::new();
    c.set_line_width(0.7).set_stroke_rgb(0.0, 0.0, 0.0);
    // A front facing, named in three languages.
    c.rect(50.0, 50.0, 300.0, 500.0).stroke();
    numbered(
        &mut c,
        (80.0, 300.0),
        "4",
        "FRONT FACING",
        &["PAREMENTURE DEVANT", "VISTA DEL FRENTE"],
    );
    // A collar, in English only.
    c.rect(400.0, 50.0, 300.0, 200.0).stroke();
    numbered(&mut c, (430.0, 140.0), "5", "COLLAR", &[]);
    // A Spanish pattern's back, cut on the fold at its left edge.
    c.rect(750.0, 50.0, 300.0, 500.0).stroke();
    text(&mut c, (850.0, 300.0), 14.0, "TRASERO");
    text(&mut c, (880.0, 280.0), 11.0, "V1234");
    text_up(&mut c, (762.0, 200.0), 9.0, "Cortar 1 al lomo");
    pdf.stream(Ref::new(4), &c.finish());
    pdf.finish()
}

fn named<'a>(pieces: &'a [Piece], name: &str) -> &'a Piece {
    pieces.iter().find(|p| p.name == name).unwrap_or_else(|| {
        panic!(
            "no {name}: {:?}",
            pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
        )
    })
}

#[test]
fn a_piece_is_named_in_english_beside_its_number_not_by_the_title_or_a_translation() {
    let result = import_pdf(&sheet(), Some("words.pdf")).unwrap();
    assert_eq!(result.pieces.len(), 3, "{:?}", result.notes);
    named(&result.pieces, "Front facing");
    named(&result.pieces, "Collar");
}

#[test]
fn cutting_on_the_fold_is_read_in_spanish() {
    let result = import_pdf(&sheet(), Some("words.pdf")).unwrap();
    let back = named(&result.pieces, "Trasero");
    let folds = back
        .outline
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Fold)
        .count();
    assert_eq!(folds, 1);
    assert_eq!(back.cut.len(), 1);
    assert_eq!(back.cut[0].quantity, 1);
    assert_eq!(back.cut[0].on_fold, Some(true));
}
