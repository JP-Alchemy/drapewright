//! A coat pattern as an indie designer's PDF prints it, drawn here for the
//! tests: a copy shop sheet, each size on a layer of its own drawn in a
//! dash of its own, the dashes stretched to fit each side, so that a side
//! stops short of its corner and a solid stub turns the corner, a side in
//! two parts a dash apart; a centre back open along its fold, the fold a
//! line of its own; pieces numbered with the name beside the number or
//! under it on three lines; a box holding the key to the lines; centre
//! lines labelled with the grain. And the notice every sheet prints, which
//! is no page of instructions.

use drapewright_io::ImportNote;
use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::{AllowanceMode, EdgeKind, Piece};

/// Points per millimetre.
const PT: f64 = 72.0 / 25.4;

type P = (f64, f64);

/// A PDF page in the making, in millimetres from its top left corner.
struct Sheet {
    width: f64,
    height: f64,
    content: String,
}

impl Sheet {
    fn new(width: f64, height: f64) -> Sheet {
        Sheet {
            width,
            height,
            content: String::new(),
        }
    }

    fn pt(&self, (x, y): P) -> String {
        format!("{:.3} {:.3}", x * PT, (self.height - y) * PT)
    }

    fn raw(&mut self, s: &str) {
        self.content.push_str(s);
        self.content.push('\n');
    }

    /// A line through `points`, closed or not, solid or dashed (`dash`
    /// in points).
    fn path(&mut self, points: &[P], closed: bool, dash: Option<(f64, f64)>) {
        let mut s = match dash {
            Some((on, off)) => format!("[{on:.2} {off:.2}] 0 d "),
            None => "[] 0 d ".to_string(),
        };
        s.push_str(&format!("{} m ", self.pt(points[0])));
        for &p in &points[1..] {
            s.push_str(&format!("{} l ", self.pt(p)));
        }
        s.push_str(if closed { "h S" } else { "S" });
        self.raw(&s);
    }

    /// Words from `at` (their baseline's start), `size` points, turned
    /// `angle` degrees anticlockwise as printed.
    fn text(&mut self, at: P, size: f64, angle: f64, words: &str) {
        let (c, s) = (angle.to_radians().cos(), angle.to_radians().sin());
        let at = self.pt(at);
        self.raw(&format!(
            "BT /F1 {size} Tf {c:.4} {s:.4} {:.4} {c:.4} {at} Tm ({words}) Tj ET",
            -s
        ));
    }

    /// On a layer: what `draw` draws.
    fn layer(&mut self, name: &str, draw: impl FnOnce(&mut Sheet)) {
        self.raw(&format!("/OC /{name} BDC"));
        draw(self);
        self.raw("EMC");
    }

    /// An outline as a drawing program prints it dashed: each side dashed
    /// on its own, stopping `before` mm short of the solid stub turning
    /// the corner (`stub` mm each way round it) and starting `after` mm
    /// past it; side `split` in two halves `apart` mm apart.
    fn dashed_outline(&mut self, corners: &[P], dash: (f64, f64), split: usize) {
        let (stub, before, after, apart) = (0.75, 0.9, 1.8, 4.0);
        let n = corners.len();
        let toward = |a: P, b: P, d: f64| {
            let l = (b.0 - a.0).hypot(b.1 - a.1);
            (a.0 + (b.0 - a.0) * d / l, a.1 + (b.1 - a.1) * d / l)
        };
        for i in 0..n {
            let (a, b) = (corners[i], corners[(i + 1) % n]);
            let from = toward(a, b, stub + after);
            let to = toward(b, a, stub + before);
            if i == split {
                let l = (to.0 - from.0).hypot(to.1 - from.1);
                let mid = toward(from, to, l / 2.0);
                self.path(&[from, toward(mid, from, apart / 2.0)], false, Some(dash));
                self.path(&[toward(mid, to, apart / 2.0), to], false, Some(dash));
            } else {
                self.path(&[from, to], false, Some(dash));
            }
            // The stub round corner b.
            let c = corners[(i + 2) % n];
            self.path(&[toward(b, a, stub), b, toward(b, c, stub)], false, None);
        }
    }
}

/// A PDF of `sheet`, with these layers (name, label).
fn pdf(sheet: &Sheet, layers: &[(&str, &str)]) -> Vec<u8> {
    let content = sheet.content.as_bytes();
    let first_layer = 6;
    let ocgs: Vec<String> = (0..layers.len())
        .map(|i| format!("{} 0 R", first_layer + i))
        .collect();
    let props: Vec<String> = layers
        .iter()
        .enumerate()
        .map(|(i, (name, _))| format!("/{name} {} 0 R", first_layer + i))
        .collect();
    let mut objects: Vec<Vec<u8>> = vec![
        if layers.is_empty() {
            b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()
        } else {
            format!(
                "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [{0}] /D << /Order [{0}] >> >> >>",
                ocgs.join(" ")
            )
            .into_bytes()
        },
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Resources << /Font << /F1 4 0 R >> /Properties << {} >> >> /Contents 5 0 R >>",
            sheet.width * PT,
            sheet.height * PT,
            props.join(" ")
        )
        .into_bytes(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec(),
        [
            format!("<< /Length {} >>\nstream\n", content.len()).into_bytes(),
            content.to_vec(),
            b"\nendstream".to_vec(),
        ]
        .concat(),
    ];
    for (_, label) in layers {
        objects.push(format!("<< /Type /OCG /Name ({label}) >>").into_bytes());
    }
    let mut out = b"%PDF-1.5\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n", i + 1).into_bytes());
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).into_bytes());
    for o in offsets {
        out.extend(format!("{o:010} 00000 n \n").into_bytes());
    }
    out.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            objects.len() + 1
        )
        .into_bytes(),
    );
    out
}

/// A rectangle's corners, clockwise from the top left.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<P> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// The notice printed at the foot of every sheet: prose, a few lines of it.
fn notice(sheet: &mut Sheet, at: P) {
    for (k, words) in [
        "Copyright 2015 the designer. All rights reserved.",
        "All designs, illustrations and images are protected",
        "by copyright and are for personal use only.",
        "Designs are not to be made for sale.",
        "Inch notations are approximate here.",
        "Please refer to the other file for the lining.",
    ]
    .iter()
    .enumerate()
    {
        sheet.text((at.0, at.1 + 5.0 * k as f64), 10.0, 0.0, words);
    }
}

fn size_of(p: &Piece) -> (f64, f64) {
    let b = p.outline.bounds();
    (b.width(), b.height())
}

#[test]
fn a_copy_shop_sheet_of_sizes_drawn_in_stretched_dashes_is_read() {
    // A copy shop sheet, 1.2 by 0.9 metres: a side front in two sizes, its
    // cutting line and its stitching line 12 mm inside it in each size's
    // dash, teal and black.
    let mut sheet = Sheet::new(1200.0, 900.0);
    notice(&mut sheet, (20.0, 850.0));
    for (layer, dash, grow) in [("S36", (4.2, 5.0), 0.0), ("S40", (14.2, 2.8), 10.0)] {
        sheet.layer(layer, |s| {
            s.raw("0 0.5 0.5 RG 0.7 w");
            s.dashed_outline(
                &rect(100.0 - grow, 100.0, 312.0 + grow, 712.0 + grow),
                dash,
                1,
            );
            s.raw("0.14 0.12 0.13 RG");
            s.dashed_outline(
                &rect(112.0 - grow, 112.0, 300.0 + grow, 700.0 + grow),
                dash,
                3,
            );
        });
    }
    sheet.layer("Shared", |s| {
        s.raw("0 0 0 RG");
        s.text((150.0, 300.0), 30.0, 0.0, "Side Front");
        s.text((150.0, 320.0), 12.0, 0.0, "cut 2");
    });
    let r = import_pdf(
        &pdf(
            &sheet,
            &[("S36", "36"), ("S40", "40"), ("Shared", "sign on pattern")],
        ),
        None,
    )
    .unwrap();
    assert_eq!(r.sizes.names, ["36", "40"]);
    assert!(
        !r.notes
            .iter()
            .any(|n| matches!(n, ImportNote::PagesLeftOut { .. })),
        "the sheet is read: {:?}",
        r.notes
    );
    for (size, width) in [("36", 188.0), ("40", 208.0)] {
        let pieces: Vec<&Piece> = r
            .pieces
            .iter()
            .filter(|p| r.sizes.of(&p.id) == Some(size))
            .collect();
        assert_eq!(pieces.len(), 1, "size {size}: {pieces:#?}");
        let p = pieces[0];
        assert_eq!(p.name, "Side Front");
        assert_eq!(p.outline.edges.len(), 4, "size {size}");
        // The stitching line, closed, found inside the cutting line.
        assert_eq!(p.allowance.mode, AllowanceMode::Add);
        assert!(
            (p.allowance.seam - 12.0).abs() < 0.5,
            "{}",
            p.allowance.seam
        );
        assert!((size_of(p).0 - width).abs() < 1.0, "{:?}", size_of(p));
    }
}

#[test]
fn a_piece_open_along_its_fold_is_closed_by_the_fold_drawn() {
    // A centre back cut on the fold: its cutting line drawn down three
    // sides, the fold along the fourth a dashed line of its own colour,
    // named along it.
    let mut sheet = Sheet::new(400.0, 1000.0);
    sheet.raw("0 0.5 0.5 RG 0.7 w");
    sheet.path(
        &[
            (100.0, 100.0),
            (180.0, 100.0),
            (190.0, 900.0),
            (100.0, 900.0),
        ],
        false,
        None,
    );
    sheet.raw("0 0 0 RG");
    sheet.path(&[(100.0, 900.0), (100.0, 100.0)], false, Some((11.0, 11.0)));
    sheet.text((105.0, 600.0), 12.0, 90.0, "Center Back Fold");
    sheet.text((140.0, 400.0), 20.0, 90.0, "Center Back");
    let r = import_pdf(&pdf(&sheet, &[]), None).unwrap();
    assert_eq!(r.pieces.len(), 1, "{:#?}", r.notes);
    let folds = r.pieces[0]
        .outline
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Fold)
        .count();
    assert_eq!(folds, 1);
}

#[test]
fn names_beside_their_number_and_set_on_several_lines() {
    let mut sheet = Sheet::new(600.0, 400.0);
    sheet.raw("0 0 0 RG 0.7 w");
    sheet.path(&rect(20.0, 20.0, 280.0, 120.0), true, None);
    sheet.path(&rect(320.0, 20.0, 580.0, 300.0), true, None);
    // "7 Collar" on one line, a mark of where the centre back is below it.
    sheet.text((40.0, 60.0), 37.0, 0.0, "7");
    sheet.text((55.0, 60.0), 28.0, 0.0, "Collar");
    sheet.text((60.0, 90.0), 13.0, 0.0, "Center Back");
    // "10" over "Armhole / Front / Facing".
    sheet.text((340.0, 60.0), 26.0, 0.0, "10");
    for (k, word) in ["Armhole", "Front", "Facing"].iter().enumerate() {
        sheet.text((350.0, 75.0 + 7.0 * k as f64), 17.0, 0.0, word);
    }
    sheet.text((350.0, 110.0), 11.0, 0.0, "cut 2");
    let r = import_pdf(&pdf(&sheet, &[]), None).unwrap();
    let mut names: Vec<&str> = r.pieces.iter().map(|p| p.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, ["Armhole Front Facing", "Collar"]);
}

#[test]
fn a_box_round_the_key_to_the_lines_is_no_piece() {
    let mut sheet = Sheet::new(600.0, 400.0);
    sheet.raw("0 0 0 RG 0.7 w");
    // The key: a box with a picture of the garment, a layout and the lines.
    sheet.path(&rect(10.0, 10.0, 200.0, 265.0), true, None);
    for k in 0..3 {
        let y = 30.0 + 70.0 * f64::from(k);
        sheet.path(&rect(30.0, y, 80.0, y + 40.0), true, None);
    }
    sheet.text((100.0, 40.0), 14.0, 0.0, "Main Pattern");
    // A piece.
    sheet.path(&rect(300.0, 50.0, 500.0, 350.0), true, None);
    sheet.text((350.0, 200.0), 24.0, 0.0, "Back");
    let r = import_pdf(&pdf(&sheet, &[]), None).unwrap();
    let names: Vec<&str> = r.pieces.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(names, ["Back"]);
    assert!(
        r.notes.contains(&ImportNote::KeysIgnored { count: 1 }),
        "{:?}",
        r.notes
    );
}

#[test]
fn a_line_labelled_for_the_centre_and_the_grain_is_both() {
    let mut sheet = Sheet::new(600.0, 900.0);
    sheet.raw("0 0 0 RG 0.7 w");
    // A centre front, its centre line 20 mm in from its front edge.
    sheet.path(&rect(50.0, 50.0, 250.0, 850.0), true, None);
    sheet.path(&[(230.0, 100.0), (230.0, 450.0)], false, None);
    sheet.text((226.0, 420.0), 13.0, 90.0, "Center Front      Fabric grain");
    // A collar: its grain across it, the arrow too short to read.
    sheet.path(&rect(350.0, 50.0, 420.0, 600.0), true, None);
    sheet.path(&[(370.0, 300.0), (402.0, 300.0)], false, None);
    sheet.text((372.0, 297.0), 11.0, 0.0, "Fabric grain");
    let r = import_pdf(&pdf(&sheet, &[]), None).unwrap();
    assert_eq!(r.pieces.len(), 2);
    let front = r
        .pieces
        .iter()
        .find(|p| size_of(p).1 > 790.0)
        .expect("the front");
    assert!(
        front
            .lines
            .iter()
            .any(|l| l.label.as_deref() == Some("Centre front")),
        "{:#?}",
        front.lines
    );
    let grain = front.grainline.as_ref().expect("a grainline");
    assert!((grain.from.x - grain.to.x).abs() < 0.5, "{grain:?}");
    let collar = r
        .pieces
        .iter()
        .find(|p| size_of(p).0 < 80.0)
        .expect("the collar");
    let grain = collar.grainline.as_ref().expect("a grainline");
    assert!((grain.from.y - grain.to.y).abs() < 0.5, "{grain:?}");
}

#[test]
fn a_deep_hem_turned_back_at_its_corners_is_read() {
    // An A-line side panel: 12 mm seams, a 40 mm hem, its side bending out
    // more just above the hem. Its cutting line shaped there for the hem
    // turned up (as the coat it is drawn from has it): over the last half
    // of the side's last stretch it runs out to 22 mm and back.
    let stitching: Vec<P> = vec![
        (100.0, 100.0),
        (300.0, 100.0),
        (330.0, 655.0),
        (352.0, 700.0),
        (48.0, 700.0),
        (70.0, 655.0),
    ];
    let n = stitching.len();
    // Side i's point `t` of the way along, `by` mm out.
    let along = |i: usize, t: f64, by: f64| -> P {
        let (a, b) = (stitching[i], stitching[(i + 1) % n]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let l = dx.hypot(dy);
        (a.0 + dx * t + dy / l * by, a.1 + dy * t - dx / l * by)
    };
    let allowance = |i: usize| if i == 3 { 40.0 } else { 12.0 };
    // Where the lines out from sides i - 1 and i meet.
    let corner = |i: usize| -> P {
        let (p, q) = (
            along((i + n - 1) % n, 0.0, allowance((i + n - 1) % n)),
            along((i + n - 1) % n, 1.0, allowance((i + n - 1) % n)),
        );
        let (r, s) = (along(i, 0.0, allowance(i)), along(i, 1.0, allowance(i)));
        let (d, e) = ((q.0 - p.0, q.1 - p.1), (s.0 - r.0, s.1 - r.1));
        let det = d.0 * e.1 - d.1 * e.0;
        let t = ((r.0 - p.0) * e.1 - (r.1 - p.1) * e.0) / det;
        (p.0 + d.0 * t, p.1 + d.1 * t)
    };
    let cutting = vec![
        corner(0),
        corner(1),
        corner(2),
        along(2, 0.5, 12.0),
        along(2, 0.8, 22.0),
        corner(3),
        corner(4),
        corner(5),
    ];
    let mut sheet = Sheet::new(500.0, 800.0);
    sheet.raw("0 0 0 RG 0.7 w");
    sheet.path(&cutting, true, None);
    sheet.path(&stitching, true, Some((4.0, 3.0)));
    sheet.text((150.0, 300.0), 24.0, 0.0, "Side");
    let r = import_pdf(&pdf(&sheet, &[]), None).unwrap();
    assert_eq!(r.pieces.len(), 1);
    let p = &r.pieces[0];
    assert_eq!(p.allowance.mode, AllowanceMode::Add, "{:#?}", r.notes);
    let widths: Vec<f64> = (0..p.outline.edges.len())
        .map(|i| p.edge_allowance(i))
        .collect();
    assert_eq!(
        widths.iter().filter(|w| (**w - 40.0).abs() < 1.0).count(),
        1,
        "{widths:?}"
    );
    assert_eq!(
        widths.iter().filter(|w| (**w - 12.0).abs() < 1.0).count(),
        5,
        "{widths:?}"
    );
}
