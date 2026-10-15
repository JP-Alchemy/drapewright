//! A sheet tiled over home-print pages as tiling tools print it: every
//! tile draws the whole sheet, moved to show its own part, with its frame
//! to cut along and its neighbours' frames past its edges, boxes at its
//! edges to match the joins, and its page number ("P1"). Then the same
//! furniture on a layer named for it, and an outline drawn three times
//! over.

use drapewright_io::pdf::import_pdf;

/// Points per millimetre.
const PT: f64 = 72.0 / 25.4;
const PAGE: (f64, f64) = (210.0, 297.0);
/// The frame each tile is cut along, inset from the page; tiles butt there.
const INSET: f64 = 15.0;
const STEP: (f64, f64) = (PAGE.0 - 2.0 * INSET, PAGE.1 - 2.0 * INSET);
const COLUMNS: usize = 3;
const ROWS: usize = 2;

type P = (f64, f64);

/// One page's content, in millimetres from its top left corner.
#[derive(Default)]
struct Page {
    content: String,
}

impl Page {
    fn pt(&self, (x, y): P) -> String {
        format!("{:.3} {:.3}", x * PT, (PAGE.1 - y) * PT)
    }

    fn path(&mut self, points: &[P], closed: bool, dashed: bool) {
        let mut s = String::from(if dashed { "[3 3] 0 d " } else { "[] 0 d " });
        s.push_str(&format!("{} m ", self.pt(points[0])));
        for &p in &points[1..] {
            s.push_str(&format!("{} l ", self.pt(p)));
        }
        s.push_str(if closed { "h S\n" } else { "S\n" });
        self.content.push_str(&s);
    }

    fn rect(&mut self, (x, y): P, (w, h): P, dashed: bool) {
        self.path(
            &[(x, y), (x + w, y), (x + w, y + h), (x, y + h)],
            true,
            dashed,
        );
    }

    fn text(&mut self, at: P, size: f64, words: &str) {
        let at = self.pt(at);
        self.content.push_str(&format!(
            "BT /F1 {size} Tf 1 0 0 1 {at} Tm ({words}) Tj ET\n"
        ));
    }

    fn layer(&mut self, name: &str, draw: impl FnOnce(&mut Page)) {
        self.content.push_str(&format!("/OC /{name} BDC\n"));
        draw(self);
        self.content.push_str("EMC\n");
    }
}

/// A PDF of `pages`, with these layers (resource name, label).
fn pdf(pages: &[Page], layers: &[(&str, &str)]) -> Vec<u8> {
    let n = pages.len();
    // Objects: 1 catalog, 2 pages, 3 font, layers from 4, then each page
    // and its content.
    let first_layer = 4;
    let first_page = first_layer + layers.len();
    let mut objects: Vec<String> = Vec::new();
    let ocgs: Vec<String> = (0..layers.len())
        .map(|i| format!("{} 0 R", first_layer + i))
        .collect();
    let catalog = if layers.is_empty() {
        "<< /Type /Catalog /Pages 2 0 R >>".to_string()
    } else {
        format!(
            "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [{0}] /D << /ON [{0}] >> >> >>",
            ocgs.join(" ")
        )
    };
    objects.push(catalog);
    let kids: Vec<String> = (0..n)
        .map(|k| format!("{} 0 R", first_page + 2 * k))
        .collect();
    objects.push(format!(
        "<< /Type /Pages /Kids [{}] /Count {n} >>",
        kids.join(" ")
    ));
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".into());
    for (_, label) in layers {
        objects.push(format!("<< /Type /OCG /Name ({label}) >>"));
    }
    let props: Vec<String> = layers
        .iter()
        .enumerate()
        .map(|(i, (name, _))| format!("/{name} {} 0 R", first_layer + i))
        .collect();
    let mut streams: Vec<Option<String>> = vec![None; objects.len()];
    for (k, page) in pages.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> /Properties << {} >> >> >>",
            PAGE.0 * PT,
            PAGE.1 * PT,
            first_page + 2 * k + 1,
            props.join(" ")
        ));
        streams.push(None);
        objects.push(String::new());
        streams.push(Some(page.content.clone()));
    }
    let mut out = b"%PDF-1.6\n".to_vec();
    let mut offsets = Vec::new();
    for (i, (object, stream)) in objects.iter().zip(&streams).enumerate() {
        offsets.push(out.len());
        match stream {
            Some(content) => out.extend(
                format!(
                    "{} 0 obj\n<< /Length {} >>\nstream\n{content}\nendstream\nendobj\n",
                    i + 1,
                    content.len() + 1
                )
                .bytes(),
            ),
            None => out.extend(format!("{} 0 obj\n{object}\nendobj\n", i + 1).bytes()),
        }
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .bytes(),
    );
    out
}

/// The sheet's drawing, in sheet millimetres: a skirt panel across all
/// the tiles, with its grainline and hip line; and a waistband.
fn sheet(page: &mut Page, at: P) {
    let s = |(x, y): P| (x - at.0, y - at.1);
    page.path(
        &[
            s((60.0, 40.0)),
            s((440.0, 40.0)),
            s((480.0, 480.0)),
            s((20.0, 480.0)),
        ],
        true,
        false,
    );
    page.path(&[s((250.0, 80.0)), s((250.0, 440.0))], false, false);
    page.path(&[s((45.0, 200.0)), s((455.0, 200.0))], false, false);
    page.rect(s((60.0, 500.0)), (400.0, 30.0), false);
    page.text(s((200.0, 300.0)), 24.0, "FRONT");
    page.text(s((200.0, 520.0)), 12.0, "WAISTBAND");
}

/// A tile's furniture: its frame, its neighbours' (all eight round it),
/// and a box at the middle of each edge, half on either side of it.
fn furniture(page: &mut Page) {
    for dc in -1..=1 {
        for dr in -1..=1 {
            let at = (INSET + dc as f64 * STEP.0, INSET + dr as f64 * STEP.1);
            page.rect(at, STEP, true);
        }
    }
    for at in [
        (INSET - 20.0, PAGE.1 / 2.0 - 20.0),
        (PAGE.0 - INSET - 20.0, PAGE.1 / 2.0 - 20.0),
        (PAGE.0 / 2.0 - 20.0, INSET - 20.0),
        (PAGE.0 / 2.0 - 20.0, PAGE.1 - INSET - 20.0),
    ] {
        page.rect(at, (40.0, 40.0), false);
    }
}

fn tiles(on_layer: bool) -> Vec<u8> {
    let mut pages = Vec::new();
    for row in 0..ROWS {
        for col in 0..COLUMNS {
            let mut page = Page::default();
            // The page's place on the sheet, less its inset.
            let at = (col as f64 * STEP.0 - INSET, row as f64 * STEP.1 - INSET);
            sheet(&mut page, at);
            if on_layer {
                page.layer("G", furniture);
            } else {
                furniture(&mut page);
            }
            page.text(
                (INSET + 3.0, INSET + 8.0),
                14.0,
                &format!("P{}", pages.len() + 1),
            );
            pages.push(page);
        }
    }
    let layers: &[(&str, &str)] = if on_layer {
        &[("G", "Paper Grid")]
    } else {
        &[]
    };
    pdf(&pages, layers)
}

#[test]
fn tiles_numbered_in_one_line_with_their_furniture_make_one_sheet() {
    let result = import_pdf(&tiles(false), Some("tiles.pdf")).unwrap();
    let mut names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Front", "Waistband"], "{:?}", result.notes);
    let front = result.pieces.iter().find(|p| p.name == "Front").unwrap();
    let b = front.outline.bounds();
    assert!(
        (b.width() - 460.0).abs() < 1.0 && (b.height() - 440.0).abs() < 1.0,
        "{} x {}",
        b.width(),
        b.height()
    );
}

#[test]
fn a_layer_for_the_page_grid_holds_no_pieces() {
    let result = import_pdf(&tiles(true), Some("tiles.pdf")).unwrap();
    let mut names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Front", "Waistband"], "{:?}", result.notes);
}

#[test]
fn an_outline_drawn_three_times_over_is_one_piece() {
    let mut page = Page::default();
    for _ in 0..3 {
        page.rect((40.0, 40.0), (100.0, 150.0), false);
    }
    page.text((60.0, 100.0), 14.0, "POCKET");
    let result = import_pdf(&pdf(&[page], &[]), Some("pocket.pdf")).unwrap();
    let names: Vec<&str> = result.pieces.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Pocket"], "{:?}", result.notes);
}
