//! Printing: plans checked for coverage, and the PDF read back and measured.

// Paper sizes and page origins are exact constants, so they compare exactly.
#![allow(clippy::float_cmp)]

use std::collections::HashMap;
use std::sync::Arc;

use drapewright_io::pdf::{PdfTexts, PrintError, PrintPlan, export_pdf, plan_print};
use drapewright_pattern_core::allowance::allowance_lines;
use drapewright_pattern_core::{Edge, EdgeKind, Grainline, Outline, Piece, Pt, Segment};
use drapewright_project::Document;
use drapewright_project::document::{
    Assembly, LayoutItem, Meta, Orientation, Paper, PaperName, PrintSettings,
};
use kurbo::{Affine, Point};
use lopdf::Object;

/// Points per millimetre.
const PT: f64 = 72.0 / 25.4;

fn rectangle(id: &str, name: &str, w: f64, h: f64) -> Piece {
    let edge = |e: &str, x: f64, y: f64| Edge::new(format!("{id}{e}"), vec![Segment::line([x, y])]);
    let mut p = Piece::new(
        id,
        name,
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                edge("a", w, 0.0),
                edge("b", w, h),
                edge("c", 0.0, h),
                edge("d", 0.0, 0.0),
            ],
        },
    );
    p.allowance.seam = 15.0;
    p
}

/// A skirt panel with a curved waist and hem and a fold at centre front.
fn panel() -> Piece {
    let mut p = Piece::new(
        "skirt",
        "Skirt front",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new(
                    "waist",
                    vec![Segment::cubic([70.0, 4.0], [140.0, 8.0], [200.0, 12.0])],
                ),
                Edge::new("side", vec![Segment::line([290.0, 580.0])]),
                Edge::new(
                    "hem",
                    vec![Segment::cubic([200.0, 595.0], [100.0, 600.0], [0.0, 600.0])],
                ),
                Edge::new("cf", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    p.outline.edges[2].kind = EdgeKind::Hem;
    p.outline.edges[3].kind = EdgeKind::Fold;
    p.allowance.hem = Some(40.0);
    p.grainline = Some(Grainline {
        from: Pt::new(100.0, 100.0),
        to: Pt::new(100.0, 450.0),
        one_way: false,
    });
    p
}

fn document(pieces: Vec<Piece>) -> Document {
    Document {
        meta: Meta {
            id: "test".into(),
            name: "Test pattern".into(),
            created: "2026-01-01T00:00:00Z".into(),
            modified: "2026-01-01T00:00:00Z".into(),
            notes: String::new(),
        },
        settings: Default::default(),
        pieces: pieces.into_iter().map(Arc::new).collect(),
        print: PrintSettings::default(),
        measurement_sets: Vec::new(),
        fabrics: Vec::new(),
        seams: Vec::new(),
        extensions: Default::default(),
    }
}

fn texts() -> PdfTexts {
    PdfTexts {
        title: "Test pattern".into(),
        subtitle: "Printed on A4".into(),
        instructions: vec!["Print at 100%. Measure the test square.".into()],
        square_metric: "10 cm".into(),
        square_imperial: "4 in".into(),
        page_map: "Page map".into(),
        not_printed: "Dashed pages are not printed.".into(),
        place_on_fold: "Place on fold".into(),
        piece_lines: HashMap::from([("skirt".to_string(), vec!["Cut 1 on the fold".to_string()])]),
    }
}

/// A page's drawing operators, each with its numeric operands.
type Ops = Vec<(String, Vec<f64>)>;

/// Each page's operators.
fn pages(bytes: &[u8]) -> (lopdf::Document, Vec<Ops>) {
    let pdf = lopdf::Document::load_mem(bytes).expect("a readable PDF");
    let ops = pdf
        .get_pages()
        .values()
        .map(|&id| {
            pdf.get_and_decode_page_content(id)
                .expect("page content")
                .operations
                .into_iter()
                .map(|op| {
                    let nums = op
                        .operands
                        .iter()
                        .filter_map(|o| o.as_float().ok().map(f64::from))
                        .collect();
                    (op.operator, nums)
                })
                .collect()
        })
        .collect();
    (pdf, ops)
}

fn media_box(pdf: &lopdf::Document, page: u32) -> Vec<f64> {
    let id = pdf.get_pages()[&page];
    pdf.get_dictionary(id)
        .unwrap()
        .get(b"MediaBox")
        .unwrap()
        .as_array()
        .unwrap()
        .iter()
        .map(|o| f64::from(o.as_float().unwrap()))
        .collect()
}

/// Lengths (mm) of every straight segment drawn with `m`/`l` on a page.
fn segment_lengths(ops: &[(String, Vec<f64>)]) -> Vec<f64> {
    let mut out = Vec::new();
    let mut at: Option<(f64, f64)> = None;
    for (op, n) in ops {
        match op.as_str() {
            "m" => at = Some((n[0], n[1])),
            "l" => {
                if let Some((x, y)) = at {
                    out.push((n[0] - x).hypot(n[1] - y) / PT);
                }
                at = Some((n[0], n[1]));
            }
            "c" => at = Some((n[4], n[5])),
            _ => {}
        }
    }
    out
}

/// Sheet coordinates of every point of every piece's cutting line.
fn cutting_on_sheet(doc: &Document, plan: &PrintPlan) -> Vec<Point> {
    plan.placements
        .iter()
        .flat_map(|pl| {
            let piece = doc.piece(&pl.piece).unwrap();
            let tf = Affine::translate((pl.x, pl.y)) * Affine::rotate(pl.rotation.to_radians());
            allowance_lines(piece)
                .cutting
                .into_iter()
                .map(move |p| tf * p)
        })
        .collect()
}

fn assert_covered(doc: &Document, plan: &PrintPlan) {
    for p in cutting_on_sheet(doc, plan) {
        assert!(
            plan.pages
                .iter()
                .filter(|pg| pg.printed)
                .any(|pg| plan.page_rect(pg.row, pg.column).contains(p)),
            "{p:?} is on no printed page"
        );
    }
}

#[test]
fn prints_at_true_size_on_a4() {
    let doc = document(vec![rectangle("r", "Pocket", 200.0, 300.0), panel()]);
    let plan = plan_print(&doc).unwrap();
    let bytes = export_pdf(&doc, &texts()).unwrap();
    let (pdf, pages) = pages(&bytes);
    assert_eq!(pages.len(), plan.page_count());

    // Every page is A4, and readers are told not to scale it.
    for n in 1..=pages.len() as u32 {
        let b = media_box(&pdf, n);
        assert!(b[0] == 0.0 && b[1] == 0.0);
        assert!(
            (b[2] - 210.0 * PT).abs() < 1e-3 && (b[3] - 297.0 * PT).abs() < 1e-3,
            "{b:?}"
        );
    }
    let catalog = pdf.catalog().unwrap();
    let prefs = catalog.get(b"ViewerPreferences").unwrap();
    let prefs = match prefs {
        Object::Reference(id) => pdf.get_dictionary(*id).unwrap(),
        other => other.as_dict().unwrap(),
    };
    assert_eq!(
        prefs.get(b"PrintScaling").unwrap().as_name().unwrap(),
        b"None"
    );
    assert_eq!(prefs.get(b"Duplex").unwrap().as_name().unwrap(), b"Simplex");

    // The cover's test squares: exactly 100 mm and 4 in.
    let squares: Vec<(f64, f64)> = pages[0]
        .iter()
        .filter(|(op, _)| op == "re")
        .map(|(_, n)| (n[2] / PT, n[3] / PT))
        .collect();
    assert!(
        squares
            .iter()
            .any(|(w, h)| (w - 100.0).abs() < 1e-3 && (h - 100.0).abs() < 1e-3),
        "{squares:?}"
    );
    assert!(
        squares
            .iter()
            .any(|(w, h)| (w - 101.6).abs() < 1e-3 && (h - 101.6).abs() < 1e-3),
        "{squares:?}"
    );

    // Known edges keep their exact lengths: the pocket's stitching line (200
    // and 300 mm) and its cutting line, 15 mm all round (230 and 330 mm).
    let lengths: Vec<f64> = pages[1..].iter().flat_map(|p| segment_lengths(p)).collect();
    for expected in [200.0, 300.0, 230.0, 330.0] {
        assert!(
            lengths.iter().any(|l| (l - expected).abs() < 1e-3),
            "no {expected} mm segment"
        );
    }
    assert_covered(&doc, &plan);
}

#[test]
fn layout_covers_every_piece_and_skips_nothing_needed() {
    let mut pieces = vec![panel()];
    for (i, (w, h)) in [(140.0, 160.0), (380.0, 80.0), (120.0, 60.0)]
        .iter()
        .enumerate()
    {
        pieces.push(rectangle(&format!("r{i}"), "Rectangle", *w, *h));
    }
    let mut doc = document(pieces);
    let plan = plan_print(&doc).unwrap();
    assert_covered(&doc, &plan);
    // Laid out side by side, never on top of each other.
    let boxes: Vec<kurbo::Rect> = plan
        .placements
        .iter()
        .map(|pl| {
            let piece = doc.piece(&pl.piece).unwrap();
            let tf = Affine::translate((pl.x, pl.y)) * Affine::rotate(pl.rotation.to_radians());
            allowance_lines(piece).cutting.iter().map(|p| tf * *p).fold(
                kurbo::Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                |r, p| r.union_pt(p),
            )
        })
        .collect();
    let overlap =
        |a: &kurbo::Rect, b: &kurbo::Rect| a.x0 < b.x1 && b.x0 < a.x1 && a.y0 < b.y1 && b.y0 < a.y1;
    for (i, a) in boxes.iter().enumerate() {
        assert!(a.x0 >= 0.0 && a.y0 >= 0.0);
        for b in &boxes[i + 1..] {
            assert!(!overlap(a, b), "{a:?} overlaps {b:?}");
        }
    }
    insta::assert_json_snapshot!(plan);

    // Overlapping pages, in landscape.
    doc.print.assembly = Assembly::Overlap;
    doc.print.orientation = Orientation::Landscape;
    let plan = plan_print(&doc).unwrap();
    assert!((plan.step_x - (plan.printable_width - 15.0)).abs() < 1e-9);
    assert!((plan.page_rect(0, 1).x0 - plan.step_x).abs() < 1e-9);
    assert_eq!(plan.paper_width, 297.0);
    assert_covered(&doc, &plan);
}

#[test]
fn empty_pages_are_skipped() {
    let mut doc = document(vec![
        rectangle("a", "A", 50.0, 50.0),
        rectangle("b", "B", 50.0, 50.0),
    ]);
    // A4 pages show 190 × 277 mm: one piece on A1, one on B2.
    doc.print.layout = Some(vec![
        LayoutItem {
            piece: "a".into(),
            x: 20.0,
            y: 20.0,
            rotation: 0.0,
        },
        LayoutItem {
            piece: "b".into(),
            x: 250.0,
            y: 330.0,
            rotation: 0.0,
        },
    ]);
    let plan = plan_print(&doc).unwrap();
    assert_eq!((plan.rows, plan.columns), (2, 2));
    let printed: Vec<&str> = plan
        .pages
        .iter()
        .filter(|p| p.printed)
        .map(|p| p.label.as_str())
        .collect();
    assert_eq!(printed, ["A1", "B2"]);
    assert_eq!(plan.page_count(), 3);
    assert_covered(&doc, &plan);

    doc.print.skip_empty_pages = false;
    assert_eq!(plan_print(&doc).unwrap().page_count(), 5);
    doc.print.test_square = false;
    doc.print.page_map = false;
    assert_eq!(plan_print(&doc).unwrap().page_count(), 4);
}

#[test]
fn letter_landscape_and_a0() {
    let mut doc = document(vec![panel()]);
    doc.print.paper = Paper::Named(PaperName::Letter);
    doc.print.orientation = Orientation::Landscape;
    let (pdf, pages) = pages(&export_pdf(&doc, &texts()).unwrap());
    let b = media_box(&pdf, 1);
    assert!(
        (b[2] - 792.0).abs() < 1e-3 && (b[3] - 612.0).abs() < 1e-3,
        "{b:?}"
    );
    assert!(pages.len() > 2);

    // The whole skirt on one sheet of A0, after the cover.
    doc.print.paper = Paper::Named(PaperName::A0);
    doc.print.orientation = Orientation::Portrait;
    let plan = plan_print(&doc).unwrap();
    assert_eq!(plan.page_count(), 2);
}

#[test]
fn the_same_project_prints_the_same_bytes() {
    let doc = document(vec![panel(), rectangle("r", "Pocket", 140.0, 160.0)]);
    assert_eq!(
        export_pdf(&doc, &texts()).unwrap(),
        export_pdf(&doc, &texts()).unwrap()
    );
}

#[test]
fn settings_that_cannot_print_are_refused() {
    let mut doc = document(vec![]);
    assert_eq!(plan_print(&doc), Err(PrintError::NothingToPrint));
    doc = document(vec![panel()]);
    doc.print.margin = 50.0;
    assert!(matches!(
        plan_print(&doc),
        Err(PrintError::PaperTooSmall { .. })
    ));
    doc.print.margin = 10.0;
    doc.print.assembly = Assembly::Overlap;
    doc.print.overlap = 100.0;
    assert_eq!(plan_print(&doc), Err(PrintError::OverlapTooLarge));
    doc.print.overlap = f64::NAN;
    assert_eq!(plan_print(&doc), Err(PrintError::InvalidSetting));
}

#[test]
fn names_the_font_cannot_show_are_listed() {
    let mut piece = rectangle("r", "Spódnica przód ś", 100.0, 100.0);
    piece.allowance.seam = 10.0;
    let plan = plan_print(&document(vec![piece])).unwrap();
    assert_eq!(plan.unprintable, ["Spódnica przód ś"]);
}
