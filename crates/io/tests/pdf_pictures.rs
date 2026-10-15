//! A pattern that is a picture: pages scanned or photographed, with no
//! lines to read. Nothing can be found in them, and the import says why.

use drapewright_io::ImportNote;
use drapewright_io::pdf::import_pdf;
use pdf_writer::{Finish, Name, Pdf, Rect, Ref};

/// A PDF of A4 pages, each painting an image over `cover` of its width
/// and height from its bottom left corner, and drawing `lines` after it
/// (raw content).
fn pages(count: usize, cover: f32, lines: &str) -> Vec<u8> {
    let (w, h) = (595.0, 842.0);
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    let image = Ref::new(3);
    pdf.pages(Ref::new(2))
        .kids((0..count).map(|k| Ref::new(10 + 2 * k as i32)))
        .count(count as i32);
    for k in 0..count {
        let (id, stream) = (Ref::new(10 + 2 * k as i32), Ref::new(11 + 2 * k as i32));
        let mut p = pdf.page(id);
        p.media_box(Rect::new(0.0, 0.0, w, h))
            .parent(Ref::new(2))
            .contents(stream);
        p.resources().x_objects().pair(Name(b"Im1"), image);
        p.finish();
        let content = format!("q {} 0 0 {} 0 0 cm /Im1 Do Q {lines}", w * cover, h * cover);
        pdf.stream(stream, content.as_bytes());
    }
    let mut im = pdf.image_xobject(image, &[0x40, 0x80, 0xc0, 0xff]);
    im.width(2).height(2).color_space().device_gray();
    im.bits_per_component(8);
    im.finish();
    pdf.finish()
}

#[test]
fn a_scanned_pattern_says_its_pages_are_pictures() {
    let result = import_pdf(&pages(2, 1.0, ""), Some("scan.pdf")).unwrap();
    assert!(result.pieces.is_empty());
    assert!(
        result
            .notes
            .contains(&ImportNote::PagesArePictures { count: 2 }),
        "{:?}",
        result.notes
    );
}

#[test]
fn a_photo_beside_the_pieces_is_no_matter() {
    // A piece drawn in lines beside a photo of the garment.
    let piece = "0 0 0 RG 1 w 300 100 m 550 100 l 550 700 l 300 700 l h S";
    let result = import_pdf(&pages(1, 0.5, piece), Some("photo.pdf")).unwrap();
    assert_eq!(result.pieces.len(), 1);
    assert!(
        !result
            .notes
            .iter()
            .any(|n| matches!(n, ImportNote::PagesArePictures { .. })),
        "{:?}",
        result.notes
    );
}
