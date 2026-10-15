//! One-page templates, as small makes (a face mask) print them: a page for
//! the main fabric and one for the lining, each a single piece drawn in
//! nearly the same place, covered in notes (what to cut, how to check the
//! print, the copyright, where the instructions are). The notes are the
//! piece's, not a page of instructions, and two pages alike are no tiles.
//! And a page of instructions with a lay plan on it, its pieces named in
//! their boxes, is still left out; so is a page of instructions drawn as
//! illustrations, its words in narrow columns beside them.

use drapewright_io::pdf::import_pdf;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

/// Points per millimetre.
const MM: f32 = 72.0 / 25.4;

fn text(c: &mut Content, (x, y): (f32, f32), size: f32, words: &str) {
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([1.0, 0.0, 0.0, 1.0, x * MM, y * MM])
        .show(Str(words.as_bytes()))
        .end_text();
}

fn document(pages: Vec<Content>) -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    let n = pages.len() as i32;
    pdf.pages(Ref::new(2))
        .kids((0..n).map(|k| Ref::new(10 + 2 * k)))
        .count(n);
    pdf.type1_font(Ref::new(5)).base_font(Name(b"Helvetica"));
    for (k, content) in pages.into_iter().enumerate() {
        let (page_ref, content_ref) = (Ref::new(10 + 2 * k as i32), Ref::new(11 + 2 * k as i32));
        let mut page = pdf.page(page_ref);
        page.media_box(Rect::new(0.0, 0.0, 216.0 * MM, 279.0 * MM))
            .parent(Ref::new(2))
            .contents(content_ref);
        page.resources().fonts().pair(Name(b"F1"), Ref::new(5));
        page.finish();
        pdf.stream(content_ref, &content.finish());
    }
    pdf.finish()
}

/// A mask's half: a curved outline about 150 by 165 mm, `shift` mm from
/// where the other page draws it, covered in notes; the page's heading
/// and footer outside it.
fn template(fabric: &str, shift: f32) -> Content {
    let mut c = Content::new();
    let p = |x: f32, y: f32| ((x + shift) * MM, y * MM);
    c.set_line_width(0.5);
    let (x0, y0) = p(30.0, 50.0);
    c.move_to(x0, y0);
    let (a, b, d) = (p(60.0, 20.0), p(150.0, 30.0), p(180.0, 60.0));
    c.cubic_to(a.0, a.1, b.0, b.1, d.0, d.1);
    let (x, y) = p(180.0, 215.0);
    c.line_to(x, y);
    let (a, b, d) = (p(120.0, 225.0), p(60.0, 240.0), p(30.0, 200.0));
    c.cubic_to(a.0, a.1, b.0, b.1, d.0, d.1);
    c.close_path().stroke();
    // The heading and the footer, outside the piece.
    text(&mut c, (40.0, 255.0), 22.0, "Face Mask Template");
    text(
        &mut c,
        (40.0, 247.0),
        10.0,
        "Instructions and pictures on our website",
    );
    text(&mut c, (180.0, 10.0), 10.0, "page 1");
    // On the piece: which fabric, bigger than the notes.
    text(&mut c, (55.0 + shift, 175.0), 16.0, fabric);
    for (k, line) in [
        "Fold your fabric to",
        "CUT 2",
        "Face Mask Template for all of the family",
        "Please check your scale here after print",
        "Meant for hobby and personal use only",
        "Do not sell the pattern or the masks",
        "Instructions and pictures on our website",
        "1/4 inch seam allowance included in the pattern",
    ]
    .iter()
    .enumerate()
    {
        text(&mut c, (55.0 + shift, 160.0 - 9.0 * k as f32), 9.0, line);
    }
    c
}

#[test]
fn a_template_covered_in_notes_is_a_piece_and_its_lining_another() {
    let pdf = document(vec![
        template("Main Fabric", 0.0),
        template("Lining Fabric", 3.0),
    ]);
    let result = import_pdf(&pdf, Some("mask.pdf")).unwrap();
    let names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        ["Main Fabric", "Lining Fabric"],
        "{:?}",
        result.notes
    );
    assert!(result.pieces.iter().all(|p| p.cut[0].quantity == 2));
}

#[test]
fn a_page_of_instructions_with_a_lay_plan_is_still_left_out() {
    let mut c = Content::new();
    // The fabric, 140 by 100 mm, and two pieces' outlines in it, named.
    c.rect(30.0 * MM, 30.0 * MM, 140.0 * MM, 100.0 * MM)
        .stroke();
    c.rect(40.0 * MM, 40.0 * MM, 50.0 * MM, 80.0 * MM).stroke();
    text(&mut c, (45.0, 80.0), 8.0, "FRONT cut 2");
    text(&mut c, (100.0, 80.0), 8.0, "BACK cut 1 on fold");
    for k in 0..8 {
        text(
            &mut c,
            (30.0, 250.0 - 10.0 * k as f32),
            10.0,
            "Pin the pieces to the fabric as the lay plan shows and cut them out.",
        );
    }
    let result = import_pdf(&document(vec![c]), Some("plan.pdf")).unwrap();
    assert!(
        result.pieces.is_empty(),
        "{:?}",
        result.pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}

#[test]
fn a_page_of_illustrations_with_words_in_narrow_columns_is_instructions() {
    let mut c = Content::new();
    // An illustration: filled shapes, a sewing machine in coloured blocks.
    for k in 0..30 {
        let (x, y) = (20.0 + 6.0 * (k % 10) as f32, 60.0 + 8.0 * (k / 10) as f32);
        c.set_fill_rgb(0.1 * (k % 7) as f32, 0.5, 0.3)
            .rect(x * MM, y * MM, 5.0 * MM, 7.0 * MM)
            .fill_nonzero();
    }
    // A box round the list of what's needed, the only line drawn.
    c.set_fill_rgb(0.0, 0.0, 0.0);
    c.rect(100.0 * MM, 40.0 * MM, 90.0 * MM, 150.0 * MM)
        .stroke();
    for (k, line) in [
        "What you need",
        "- Paper scissors.",
        "- Fabric scissors.",
        "Flat fabric, see",
        "the cutting plan",
        "for how much.",
        "An iron.",
        "A sewing machine.",
        "Thread to match.",
        "Pins or clips.",
        "A tape measure.",
        "Chalk or a pen",
        "to mark the fabric.",
        "First print the page",
        "with the test square",
        "and measure it.",
    ]
    .iter()
    .enumerate()
    {
        text(&mut c, (105.0, 180.0 - 9.0 * k as f32), 9.0, line);
    }
    let result = import_pdf(&document(vec![c]), Some("needs.pdf")).unwrap();
    assert!(
        result.pieces.is_empty(),
        "{:?}",
        result.pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
    );
}
