//! PDFs printed from pattern CAD through a plotter driver (Grafis patterns
//! sold as PDFs are made this way): every line is short separate strokes,
//! lettering and logos are strokes too, and arrowheads are drawn round.
//! The drawing here is made up, with each thing such files do.
#![allow(clippy::float_cmp)] // Allowances are snapped to exact values.

use drapewright_io::ImportNote;
use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::{MarkKind, Piece};
use pdf_writer::{Content, Finish, Pdf, Rect, Ref};

/// Page height, mm; the drawing's y runs down from the top.
const HEIGHT: f64 = 600.0;
const MM: f64 = 72.0 / 25.4;

type P = (f64, f64);

struct Plotter {
    c: Content,
}

impl Plotter {
    fn pt(p: P) -> (f32, f32) {
        ((p.0 * MM) as f32, ((HEIGHT - p.1) * MM) as f32)
    }

    /// A line through `points`, one stroke every `step` mm or so.
    fn line(&mut self, points: &[P], step: f64) {
        for w in points.windows(2) {
            let (a, b) = (w[0], w[1]);
            let n = ((b.0 - a.0).hypot(b.1 - a.1) / step).ceil().max(1.0) as usize;
            let at = |t: f64| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            for k in 0..n {
                let (p, q) = (
                    Self::pt(at(k as f64 / n as f64)),
                    Self::pt(at((k + 1) as f64 / n as f64)),
                );
                self.c.move_to(p.0, p.1).line_to(q.0, q.1).stroke();
            }
        }
    }

    /// A closed shape, as one path.
    fn shape(&mut self, points: &[P]) {
        let p = Self::pt(points[0]);
        self.c.move_to(p.0, p.1);
        for &q in &points[1..] {
            let q = Self::pt(q);
            self.c.line_to(q.0, q.1);
        }
        self.c.close_path().stroke();
    }

    /// A closed shape drawn as three strokes that miss each other by a
    /// hairline, as plotter drivers draw logos.
    fn gappy(&mut self, points: &[P]) {
        let mut ring = points.to_vec();
        ring.push(points[0]);
        let third = ring.len() / 3;
        for part in [
            &ring[..=third],
            &ring[third..=2 * third],
            &ring[2 * third..],
        ] {
            let nudge = |p: P| (p.0 + 0.15, p.1);
            let p = Self::pt(nudge(part[0]));
            self.c.move_to(p.0, p.1);
            for &q in &part[1..] {
                let q = Self::pt(q);
                self.c.line_to(q.0, q.1);
            }
            self.c.stroke();
        }
    }

    /// Strokes of lettering: `count` letters 2.5 mm tall in a row.
    fn words(&mut self, at: P, count: usize) {
        for k in 0..count {
            let x = at.0 + 3.0 * k as f64;
            self.line(&[(x, at.1), (x, at.1 + 2.5)], 10.0);
            self.line(&[(x, at.1 + 2.5), (x + 1.5, at.1 + 2.5)], 10.0);
        }
    }

    /// A brand's logo: two letters of different shapes and a dot, `scale`
    /// times the usual size.
    fn logo(&mut self, at: P, scale: f64) {
        let place = |pts: &[P]| -> Vec<P> {
            pts.iter()
                .map(|p| (at.0 + p.0 * scale, at.1 + p.1 * scale))
                .collect()
        };
        // Each side split in three, as a plotter's outline has many points.
        let dense = |pts: &[P]| -> Vec<P> {
            let mut out = Vec::new();
            for k in 0..pts.len() {
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                for t in [0.0, 1.0 / 3.0, 2.0 / 3.0] {
                    out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
                }
            }
            out
        };
        let vee = [
            (0.0, 0.0),
            (4.0, 0.0),
            (8.0, 20.0),
            (12.0, 0.0),
            (16.0, 0.0),
            (10.0, 30.0),
            (6.0, 30.0),
        ];
        let ell = [
            (20.0, 0.0),
            (24.0, 0.0),
            (24.0, 26.0),
            (34.0, 26.0),
            (34.0, 30.0),
            (20.0, 30.0),
        ];
        self.gappy(&place(&dense(&vee)));
        self.gappy(&place(&dense(&ell)));
        let dot: Vec<P> = (0..20)
            .map(|k| {
                let a = std::f64::consts::TAU * f64::from(k) / 20.0;
                (22.0 + 1.5 * a.cos(), -4.0 + 1.5 * a.sin())
            })
            .collect();
        self.shape(&place(&dot));
    }

    /// A "+" drawn round: twelve square corners, 6 mm across.
    fn cross(&mut self, c: P) {
        let (a, b) = (1.0, 3.0);
        let pts = [
            (-a, -b),
            (a, -b),
            (a, -a),
            (b, -a),
            (b, a),
            (a, a),
            (a, b),
            (-a, b),
            (-a, a),
            (-b, a),
            (-b, -a),
            (-a, -a),
        ];
        self.shape(&pts.map(|p| (c.0 + p.0, c.1 + p.1)));
    }

    /// A triangle drawn round with its base's middle at `base` and its
    /// point `length` mm on along `dir`.
    fn head(&mut self, base: P, dir: P, length: f64, width: f64) {
        let side = (-dir.1 * width / 2.0, dir.0 * width / 2.0);
        self.shape(&[
            (base.0 + side.0, base.1 + side.1),
            (base.0 + dir.0 * length, base.1 + dir.1 * length),
            (base.0 - side.0, base.1 - side.1),
        ]);
    }
}

fn plotted() -> Vec<u8> {
    let mut p = Plotter { c: Content::new() };
    p.c.set_line_width(0.6);

    // Piece A: a cut-on fly makes the allowance 40 mm along the top half of
    // the right edge, 10 mm elsewhere. Its notches start on corners of the
    // cutting line's strokes, and come first in the file.
    for y in [150.0, 250.0] {
        p.line(&[(50.0, y), (60.0, y)], 20.0);
    }
    p.line(
        &[
            (50.0, 50.0),
            (280.0, 50.0),
            (280.0, 210.0),
            (250.0, 210.0),
            (250.0, 370.0),
            (50.0, 370.0),
            (50.0, 50.0),
        ],
        2.0,
    );
    p.line(
        &[
            (60.0, 60.0),
            (240.0, 60.0),
            (240.0, 360.0),
            (60.0, 360.0),
            (60.0, 60.0),
        ],
        2.0,
    );
    // Its grainline ends in a small triangle, with "grainline" written
    // right beside it.
    p.line(&[(150.0, 100.0), (150.0, 320.0)], 7.0);
    p.head((150.0, 320.0), (0.0, 1.0), 6.0, 5.0);
    p.words((155.0, 321.0), 9);
    p.words((100.0, 200.0), 12);
    p.logo((80.0, 260.0), 1.0);
    for y in [100.0, 140.0, 180.0] {
        p.cross((200.0, y));
    }

    // Piece B: its stitching line forks above the hem, round an extension
    // of the piece; the seam carries on straight down to the hem.
    p.line(
        &[
            (320.0, 50.0),
            (520.0, 50.0),
            (520.0, 370.0),
            (290.0, 370.0),
            (290.0, 250.0),
            (320.0, 250.0),
            (320.0, 50.0),
        ],
        2.0,
    );
    p.line(
        &[
            (330.0, 260.0),
            (300.0, 260.0),
            (300.0, 360.0),
            (510.0, 360.0),
            (510.0, 60.0),
            (330.0, 60.0),
        ],
        4.0,
    );
    p.line(&[(330.0, 60.0), (330.0, 360.0)], 3.7);
    // A dart whose point is marked with a circle, and a grainline with a
    // flat head at its point.
    p.line(&[(400.0, 360.0), (420.0, 250.0), (440.0, 360.0)], 5.0);
    let circle: Vec<P> = (0..24)
        .map(|k| {
            let a = std::f64::consts::TAU * f64::from(k) / 24.0;
            (420.0 + 2.0 * a.cos(), 250.0 + 2.0 * a.sin())
        })
        .collect();
    p.shape(&circle);
    p.line(&[(470.0, 330.0), (470.0, 90.0)], 6.0);
    p.shape(&[(470.0, 90.0), (475.0, 95.0), (465.0, 95.0)]);
    p.words((350.0, 300.0), 8);
    p.logo((440.0, 120.0), 0.6);

    // Piece C: a strip whose stitching line is drawn on three sides; along
    // the fourth the cutting line is the stitching line. Cut on the bias.
    p.line(
        &[
            (560.0, 50.0),
            (620.0, 50.0),
            (620.0, 350.0),
            (560.0, 350.0),
            (560.0, 50.0),
        ],
        2.0,
    );
    p.line(
        &[(620.0, 60.0), (570.0, 60.0), (570.0, 340.0), (620.0, 340.0)],
        2.0,
    );
    p.line(&[(575.0, 320.0), (605.0, 290.0)], 6.0);
    let d = std::f64::consts::FRAC_1_SQRT_2;
    p.head((605.0, 290.0), (d, -d), 10.0, 6.0);
    p.logo((575.0, 150.0), 0.8);

    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    let mut page = pdf.page(Ref::new(3));
    page.media_box(Rect::new(
        0.0,
        0.0,
        (700.0 * MM) as f32,
        (HEIGHT * MM) as f32,
    ))
    .parent(Ref::new(2))
    .contents(Ref::new(4));
    page.finish();
    pdf.stream(Ref::new(4), &p.c.finish());
    pdf.finish()
}

fn piece_at(pieces: &[Piece], x: f64) -> &Piece {
    pieces
        .iter()
        .find(|p| {
            let b = p.outline.bounds();
            b.x0 <= x && x <= b.x1
        })
        .unwrap_or_else(|| panic!("no piece at x = {x}"))
}

#[test]
fn plotter_drawn_pieces() {
    // PLOTTED_PDF=file.pdf writes the drawing out, to look at.
    if let Some(out) = std::env::var_os("PLOTTED_PDF") {
        std::fs::write(out, plotted()).unwrap();
    }
    let result = import_pdf(&plotted(), None).unwrap();
    assert_eq!(result.pieces.len(), 3, "{:#?}", result.notes);
    let (a, b, c) = (
        piece_at(&result.pieces, 150.0),
        piece_at(&result.pieces, 420.0),
        piece_at(&result.pieces, 590.0),
    );
    let widths = |p: &Piece| -> Vec<f64> {
        (0..p.outline.edges.len())
            .map(|i| p.edge_allowance(i))
            .collect()
    };

    // The fly's edge is broken where its allowance changes, and the cutting
    // line comes out as drawn.
    assert_eq!(a.allowance.seam, 10.0);
    let wa = widths(a);
    assert!(
        wa.contains(&40.0) && wa.iter().filter(|w| **w == 10.0).count() == 4,
        "{wa:?}"
    );
    assert!(
        !result.notes.iter().any(|n| matches!(n,
        ImportNote::CuttingLineDiffers { piece, .. } if *piece == a.id)),
        "{:#?}",
        result.notes
    );
    assert_eq!(a.notches.len(), 2);
    // Its grainline was found, though its head sits by lettering; the
    // round "+" marks are marks; the logo and lettering are gone.
    let grain = a.grainline.as_ref().expect("piece A's grainline");
    assert!((grain.from.x - 150.0).abs() < 0.5 && (grain.to.x - 150.0).abs() < 0.5);
    assert_eq!(
        a.marks.iter().filter(|m| m.kind == MarkKind::Cross).count(),
        3
    );
    assert!(a.lines.is_empty(), "{:#?}", a.lines);

    // The fork: the stitching line goes round the extension; the seam's
    // way on to the hem stays as a line, with the dart.
    assert_eq!(b.allowance.seam, 10.0);
    assert!(widths(b).iter().all(|w| *w == 10.0), "{:?}", widths(b));
    assert!(
        (b.outline.bounds().x0 - 300.0).abs() < 0.5,
        "{:?}",
        b.outline.bounds()
    );
    assert_eq!(b.lines.len(), 2, "{:#?}", b.lines);
    // The longest arrow is the grainline, not the dart's leg ending in the
    // circle; the circle is a mark.
    let grain = b.grainline.as_ref().expect("piece B's grainline");
    assert!(
        (grain.from.x - 470.0).abs() < 0.5 && (grain.to.x - 470.0).abs() < 0.5,
        "{grain:?}"
    );
    assert_eq!(
        b.marks
            .iter()
            .filter(|m| m.kind == MarkKind::Circle)
            .count(),
        1
    );

    // The strip: no allowance where the stitching line wasn't drawn.
    assert_eq!(c.allowance.seam, 10.0);
    let wc = widths(c);
    assert!(
        wc.contains(&0.0) && wc.iter().filter(|w| **w == 10.0).count() == 3,
        "{wc:?}"
    );
    let grain = c.grainline.as_ref().expect("piece C's grainline");
    assert!(((grain.to.x - grain.from.x).abs() - (grain.to.y - grain.from.y).abs()).abs() < 0.5);
    assert!(c.lines.is_empty(), "{:#?}", c.lines);

    // Three logos and the lettering were left out, and said so.
    assert!(
        result
            .notes
            .iter()
            .any(|n| matches!(n, ImportNote::LetteringIgnored { count } if *count >= 3))
    );
    assert!(
        !result
            .notes
            .iter()
            .any(|n| matches!(n, ImportNote::SmallShapesIgnored { .. })),
        "{:#?}",
        result.notes
    );
}
