//! A piece printed in six sizes, nested, as many indie patterns print
//! them: the sizes' lines take three styles in turn (black, black dashed,
//! red, then black again), each size drawing its whole outline, the
//! stretches all sizes share too, and its letter on its line. Every size
//! draws its bottom edge in black. Dashed corners are marked by solid
//! stubs. The piece's number is set large with its name under it, its
//! grainline crosses it with the arrow partway along, and a bust line is
//! drawn once, out to the largest size, named along it.

use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::Piece;
use pdf_writer::{Content, Finish, Name, Pdf, Rect, Ref, Str};

const MM: f64 = 25.4 / 72.0;
const SIZES: [&str; 6] = ["A", "B", "C", "D", "E", "F"];

type P = (f32, f32);

/// Shared corners: top left, bottom left, bottom right (pt, y up).
const TOP: P = (100.0, 700.0);
const BOTTOM: P = (100.0, 100.0);
const BOTTOM_RIGHT: P = (300.0, 100.0);

/// Size k's top right corner.
fn top_right(k: usize) -> P {
    (300.0 + 15.0 * k as f32, 700.0 + 10.0 * k as f32)
}

/// Size k's line: (red?, dashed?).
fn style(k: usize) -> (bool, bool) {
    match k % 3 {
        0 => (false, false),
        1 => (false, true),
        _ => (true, false),
    }
}

fn line(c: &mut Content, a: P, b: P) {
    c.move_to(a.0, a.1).line_to(b.0, b.1).stroke();
}

/// A dashed side as an illustration program prints it: half a dash, solid,
/// at either corner, then a gap, then the dashes.
fn dashed_side(c: &mut Content, a: P, b: P) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy);
    let at = |d: f32| (a.0 + dx * d / len, a.1 + dy * d / len);
    c.set_dash_pattern([], 0.0);
    line(c, a, at(1.5));
    line(c, at(len - 1.5), b);
    c.set_dash_pattern([3.0, 3.0], 0.0);
    line(c, at(4.5), at(len - 4.5));
    c.set_dash_pattern([], 0.0);
}

fn text(c: &mut Content, at: P, size: f32, words: &str) {
    c.begin_text()
        .set_font(Name(b"F1"), size)
        .set_text_matrix([1.0, 0.0, 0.0, 1.0, at.0, at.1])
        .show(Str(words.as_bytes()))
        .end_text();
}

fn pattern() -> Vec<u8> {
    let mut pdf = Pdf::new();
    pdf.catalog(Ref::new(1)).pages(Ref::new(2));
    pdf.pages(Ref::new(2)).kids([Ref::new(3)]).count(1);
    pdf.type1_font(Ref::new(5)).base_font(Name(b"Helvetica"));
    let mut page = pdf.page(Ref::new(3));
    page.media_box(Rect::new(0.0, 0.0, 600.0, 900.0))
        .parent(Ref::new(2))
        .contents(Ref::new(4));
    page.resources().fonts().pair(Name(b"F1"), Ref::new(5));
    page.finish();
    let mut c = Content::new();
    c.set_line_width(0.5);
    // Largest first, as such patterns are drawn.
    for k in (0..SIZES.len()).rev() {
        let (red, dashed) = style(k);
        let colour = |c: &mut Content| {
            if red {
                c.set_stroke_rgb(0.7, 0.0, 0.0);
            } else {
                c.set_stroke_rgb(0.0, 0.0, 0.0);
            }
        };
        colour(&mut c);
        let tr = top_right(k);
        for (a, b) in [(BOTTOM_RIGHT, tr), (tr, TOP), (TOP, BOTTOM)] {
            if dashed {
                dashed_side(&mut c, a, b);
            } else {
                line(&mut c, a, b);
            }
        }
        // The bottom, black for every size.
        c.set_stroke_rgb(0.0, 0.0, 0.0);
        line(&mut c, BOTTOM, BOTTOM_RIGHT);
        // The size's letter on its line, halfway up its side.
        colour(&mut c);
        let mid = ((BOTTOM_RIGHT.0 + tr.0) / 2.0, (BOTTOM_RIGHT.1 + tr.1) / 2.0);
        c.set_fill_rgb(0.0, 0.0, 0.0);
        text(&mut c, (mid.0 - 2.0, mid.1 - 2.5), 7.0, SIZES[k]);
    }
    c.set_stroke_rgb(0.0, 0.0, 0.0);
    // The grainline, its arrow partway down.
    line(&mut c, (150.0, 150.0), (150.0, 650.0));
    c.move_to(150.0, 392.0)
        .line_to(146.0, 404.0)
        .line_to(154.0, 404.0)
        .close_path()
        .fill_nonzero();
    text(&mut c, (156.0, 410.0), 8.0, "Grain Line");
    // The bust line, drawn once out past the largest size; the waist line
    // out far past it, halfway along it past the smallest sizes' side.
    line(&mut c, (100.0, 600.0), (380.0, 600.0));
    text(&mut c, (110.0, 604.0), 8.0, "BUST");
    line(&mut c, (100.0, 300.0), (540.0, 300.0));
    text(&mut c, (110.0, 304.0), 8.0, "WAIST");
    // The piece's number, and its name under it; the words above it.
    text(&mut c, (190.0, 520.0), 9.0, "Overbust");
    text(&mut c, (186.0, 470.0), 26.0, "- 1 -");
    text(&mut c, (190.0, 450.0), 10.0, "Front");
    pdf.stream(Ref::new(4), &c.finish());
    pdf.finish()
}

fn size_of(p: &Piece) -> (f64, f64) {
    let b = p.outline.bounds();
    (b.width(), b.height())
}

#[test]
fn nested_sizes_in_styles_taken_in_turn() {
    let result = import_pdf(&pattern(), Some("nest.pdf")).unwrap();
    assert_eq!(result.sizes.names, SIZES, "{:?}", result.notes);
    // The sizes' lines: black, black dashed, red, and round again.
    let swatches: Vec<(&str, bool)> = result
        .sizes
        .swatches
        .iter()
        .map(|s| (s.colour.as_str(), s.dashed))
        .collect();
    assert_eq!(
        swatches,
        [
            ("#000000", false),
            ("#000000", true),
            ("#b30000", false),
            ("#000000", false),
            ("#000000", true),
            ("#b30000", false),
        ]
    );
    for (k, size) in SIZES.iter().enumerate() {
        let pieces: Vec<&Piece> = result
            .pieces
            .iter()
            .filter(|p| {
                result
                    .sizes
                    .pieces
                    .iter()
                    .any(|ps| ps.piece == p.id && ps.size == *size)
            })
            .collect();
        assert_eq!(pieces.len(), 1, "size {size}: one piece");
        let piece = pieces[0];
        // Named on the line under its number.
        assert_eq!(piece.name, "Front", "size {size}");
        // Each size its own outline, the bottom too.
        let (w, h) = size_of(piece);
        let tr = top_right(k);
        let (want_w, want_h) = (f64::from(tr.0 - 100.0) * MM, f64::from(tr.1 - 100.0) * MM);
        assert!(
            (w - want_w).abs() < 1.0 && (h - want_h).abs() < 1.0,
            "size {size}: {w} x {h}, not {want_w} x {want_h}"
        );
        assert!(piece.grainline.is_some(), "size {size}: grainline");
        // The bust line stops at the size's own side, named for the level
        // it marks on the body.
        let lines: Vec<_> = piece
            .lines
            .iter()
            .filter(|l| (l.start.y - (900.0 - 600.0) * MM).abs() < 1.0)
            .collect();
        assert!(
            lines.iter().all(|l| l.label.as_deref() == Some("Bust")),
            "size {size}: {:?}",
            lines.iter().map(|l| &l.label).collect::<Vec<_>>()
        );
        let bust: Vec<f64> = lines
            .iter()
            .flat_map(|l| std::iter::once(l.start.x).chain(l.segments.iter().map(|s| s.to().x)))
            .collect();
        assert!(!bust.is_empty(), "size {size}: bust line");
        let side = f64::from(300.0 + 15.0 * k as f32 * 500.0 / (600.0 + 10.0 * k as f32)) * MM;
        let reach = bust.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            (reach - side).abs() < 1.0,
            "size {size}: bust line to {reach}, side at {side}"
        );
        // The waist line too, though it runs on far past the smallest.
        let waist: Vec<f64> = piece
            .lines
            .iter()
            .filter(|l| l.label.as_deref() == Some("Waist"))
            .flat_map(|l| std::iter::once(l.start.x).chain(l.segments.iter().map(|s| s.to().x)))
            .collect();
        let side = f64::from(300.0 + 15.0 * k as f32 * 200.0 / (600.0 + 10.0 * k as f32)) * MM;
        let reach = waist.iter().copied().fold(f64::MIN, f64::max);
        assert!(
            (reach - side).abs() < 1.0,
            "size {size}: waist line to {reach}, side at {side}"
        );
    }
}
