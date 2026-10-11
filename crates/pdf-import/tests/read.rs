//! The reader on PDFs written here with pdf-writer, so every value is known.

// The values written are exact in binary, so they read back exactly.
#![allow(clippy::float_cmp)]

use drapewright_pdf_import::{Document, PdfError};
use kurbo::{Point, Shape as _};
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str, TextStr};

/// A one-page PDF with layers, colours, text and a reused drawing.
fn sample() -> Vec<u8> {
    let (catalog, tree, page, content, font, form, size_s, size_m) = (
        Ref::new(1),
        Ref::new(2),
        Ref::new(3),
        Ref::new(4),
        Ref::new(5),
        Ref::new(6),
        Ref::new(7),
        Ref::new(8),
    );
    let mut pdf = Pdf::new();
    let mut cat = pdf.catalog(catalog);
    cat.pages(tree);
    {
        let mut oc = cat.insert(Name(b"OCProperties")).dict();
        oc.insert(Name(b"OCGs")).array().items([size_s, size_m]);
        let mut d = oc.insert(Name(b"D")).dict();
        d.insert(Name(b"OFF")).array().item(size_m);
    }
    cat.finish();
    pdf.pages(tree).kids([page]).count(1);
    let mut p = pdf.page(page);
    p.media_box(Rect::new(0.0, 0.0, 400.0, 300.0));
    p.parent(tree);
    p.contents(content);
    {
        let mut res = p.resources();
        res.fonts().pair(Name(b"F1"), font);
        res.x_objects().pair(Name(b"X1"), form);
        res.insert(Name(b"Properties"))
            .dict()
            .pair(Name(b"s"), size_s)
            .pair(Name(b"m"), size_m);
    }
    p.finish();
    pdf.indirect(size_s)
        .dict()
        .pair(Name(b"Type"), Name(b"OCG"))
        .pair(Name(b"Name"), TextStr("Size S"));
    pdf.indirect(size_m)
        .dict()
        .pair(Name(b"Type"), Name(b"OCG"))
        .pair(Name(b"Name"), TextStr("Size M"));
    pdf.type1_font(font)
        .base_font(Name(b"Helvetica"))
        .encoding_predefined(Name(b"WinAnsiEncoding"));

    let mut square = Content::new();
    square.rect(0.0, 0.0, 10.0, 10.0).fill_nonzero();
    let square = square.finish();
    pdf.form_xobject(form, &square)
        .bbox(Rect::new(0.0, 0.0, 10.0, 10.0));

    let mut c = Content::new();
    // A red dashed rectangle in "Size S".
    c.begin_marked_content_with_properties(Name(b"OC"))
        .properties_named(Name(b"s"));
    c.set_stroke_rgb(1.0, 0.0, 0.0)
        .set_dash_pattern([6.0, 2.0], 0.0)
        .set_line_width(2.0)
        .rect(50.0, 50.0, 100.0, 80.0)
        .stroke();
    c.end_marked_content();
    // A blue triangle in "Size M", which is hidden when the file opens.
    c.begin_marked_content_with_properties(Name(b"OC"))
        .properties_named(Name(b"m"));
    c.set_fill_rgb(0.0, 0.0, 1.0)
        .move_to(200.0, 50.0)
        .line_to(300.0, 50.0)
        .line_to(250.0, 120.0)
        .close_path()
        .fill_nonzero();
    c.end_marked_content();
    // The same drawing twice, moved.
    for x in [20.0, 320.0] {
        c.save_state()
            .transform([1.0, 0.0, 0.0, 1.0, x, 250.0])
            .x_object(Name(b"X1"))
            .restore_state();
    }
    c.begin_text()
        .set_font(Name(b"F1"), 20.0)
        .next_line(100.0, 150.0)
        .show(Str(b"FRONT"))
        .end_text();
    pdf.stream(content, &c.finish());
    pdf.finish()
}

#[test]
fn reads_paths_colours_text_and_layers() {
    let doc = Document::load(&sample()).unwrap();
    assert_eq!(doc.page_count(), 1);
    let layers: Vec<(&str, bool)> = doc
        .layers()
        .iter()
        .map(|l| (l.name.as_str(), l.visible))
        .collect();
    assert_eq!(layers, [("Size S", true), ("Size M", false)]);
    let (s, m) = (doc.layers()[0].id, doc.layers()[1].id);

    let page = doc.page(0).unwrap();
    assert_eq!(page.media_box, kurbo::Rect::new(0.0, 0.0, 400.0, 300.0));
    let shapes = &page.drawing.shapes;
    assert_eq!(shapes.len(), 4);

    let rect = &shapes[0];
    assert_eq!(rect.stroke, Some([1.0, 0.0, 0.0]));
    assert_eq!(rect.fill, None);
    assert_eq!(rect.dash, [6.0, 2.0]);
    assert_eq!(rect.line_width, 2.0);
    assert_eq!(rect.layers, [s]);
    assert_eq!(
        rect.path.bounding_box(),
        kurbo::Rect::new(50.0, 50.0, 150.0, 130.0)
    );

    let triangle = &shapes[1];
    assert_eq!(triangle.fill, Some([0.0, 0.0, 1.0]));
    assert_eq!(triangle.layers, [m]);

    // The form drawn twice, each where it was put, clipped to its box.
    for (shape, x) in shapes[2..].iter().zip([20.0, 320.0]) {
        assert_eq!(
            shape.path.bounding_box(),
            kurbo::Rect::new(x, 250.0, x + 10.0, 260.0)
        );
        assert_eq!(
            shape.clip,
            Some(kurbo::Rect::new(x, 250.0, x + 10.0, 260.0))
        );
        assert!(shape.layers.is_empty());
    }

    let texts = &page.drawing.texts;
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0].text, "FRONT");
    assert_eq!(texts[0].start, Point::new(100.0, 150.0));
    assert_eq!(texts[0].size, 20.0);
    // No widths in the file: half an em per letter.
    assert_eq!(texts[0].end, Point::new(150.0, 150.0));
}

#[test]
fn page_boxes_rotation_and_units() {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    let mut p = pdf.page(Ref::new(3));
    p.media_box(Rect::new(0.0, 0.0, 600.0, 800.0))
        .crop_box(Rect::new(10.0, 20.0, 590.0, 780.0))
        .rotate(90)
        .user_unit(2.0)
        .parent(Ref::new(2));
    p.finish();
    let doc = Document::load(&pdf.finish()).unwrap();
    let page = doc.page(0).unwrap();
    assert_eq!(page.crop_box, kurbo::Rect::new(10.0, 20.0, 590.0, 780.0));
    assert_eq!(page.rotate, 90);
    assert_eq!(page.user_unit, 2.0);
    assert!(page.drawing.shapes.is_empty());
}

#[test]
fn damaged_files_are_read_as_far_as_possible() {
    let good = sample();
    // A wrong startxref offset: the objects are found by scanning.
    let text = String::from_utf8_lossy(&good).into_owned();
    let at = text.rfind("startxref").unwrap();
    let mut bad = good[..at].to_vec();
    bad.extend_from_slice(b"startxref\n999999\n%%EOF\n");
    let doc = Document::load(&bad).unwrap();
    assert_eq!(doc.page(0).unwrap().drawing.shapes.len(), 4);
    assert_eq!(doc.layers().len(), 2);

    assert_eq!(Document::load(b"not a pdf").unwrap_err(), PdfError::NotAPdf);
    assert!(Document::load(b"%PDF-1.7\nnothing else").is_err());
}
