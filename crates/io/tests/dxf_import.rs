//! DXF import against the fixtures in `fixtures/dxf` (made by
//! `fixtures/dxf/make_fixtures.py`).

// Measured allowances snap to exact constants, so they compare exactly.
#![allow(clippy::float_cmp)]

use std::path::Path;

use soom_io::dxf::import_dxf;
use soom_io::{AllowanceAnswer, ImportNote, ImportResult, ScaleReason};
use soom_pattern_core::{AllowanceMode, EdgeKind, MarkKind, Piece, ScaleBasis};

fn fixture(name: &str) -> ImportResult {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/dxf")
        .join(name);
    import_dxf(&std::fs::read(&path).unwrap(), Some(name)).unwrap()
}

fn piece<'a>(r: &'a ImportResult, name: &str) -> &'a Piece {
    r.pieces.iter().find(|p| p.name == name).unwrap_or_else(|| {
        panic!(
            "no {name:?} in {:?}",
            r.pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
        )
    })
}

#[test]
fn aama_piece_with_cutting_and_stitching_lines() {
    let r = fixture("aama-skirt.dxf");
    assert!(
        !r.scale.certain,
        "METRIC is read as millimetres but confirmed by the user"
    );
    assert_eq!(r.scale.reason, ScaleReason::AamaMetric);
    assert_eq!(r.scale.chosen.basis, ScaleBasis::AssumedMm);
    assert_eq!(r.pieces.len(), 2);

    let p = piece(&r, "Skirt front");
    // The stitching line is the outline, flipped so the waist is at the top.
    let b = p.outline.bounds();
    assert!(
        (b.width() - 250.0).abs() < 1e-6 && (b.height() - 600.0).abs() < 1e-6,
        "{b:?}"
    );
    assert!(b.y0 < b.y1 && (b.y0 + 600.0).abs() < 1e-6);
    // Four corners from the turn points.
    assert_eq!(p.outline.edges.len(), 4);
    // Allowance measured edge by edge: 15 mm seams, 40 mm hem, centre front on the fold.
    assert_eq!(p.allowance.mode, AllowanceMode::Add);
    assert_eq!(p.allowance.seam, 15.0);
    let mut widths: Vec<f64> = (0..4).map(|i| p.edge_allowance(i)).collect();
    widths.sort_by(f64::total_cmp);
    assert_eq!(widths, [0.0, 15.0, 15.0, 40.0]);
    assert_eq!(
        p.outline
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Fold)
            .count(),
        1
    );
    assert!(r.notes.contains(&ImportNote::SeamLineFound {
        piece: p.id.clone(),
        allowance: 15.0
    }));

    assert_eq!(p.notches.len(), 1);
    assert!(p.grainline.is_some());
    assert_eq!(p.marks.len(), 1);
    assert_eq!(p.marks[0].kind, MarkKind::Drill);
    assert_eq!(p.cut[0].quantity, 1);
    assert_eq!(p.notes, "Size 38");

    // The waistband has only its cutting line: allowance is included, and
    // the user is asked how wide it is.
    let band = piece(&r, "Waistband");
    assert_eq!(band.allowance.mode, AllowanceMode::Included);
    let b = band.outline.bounds();
    assert!(
        (b.width() - 380.0).abs() < 1e-6 && (b.height() - 80.0).abs() < 1e-6,
        "{b:?}"
    );
    assert_eq!(band.outline.edges.len(), 4);
    assert_eq!(
        band.outline
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Fold)
            .count(),
        1
    );
    assert!(band.grainline.is_some());
    assert_eq!(r.allowance.pieces, std::slice::from_ref(&band.id));
    assert_eq!(r.allowance.suggested, Some(AllowanceMode::Included));
    insta::assert_json_snapshot!(r);

    // Her answer reaches only the piece that was asked about.
    let mut answered = r.clone();
    answered.answer_allowance(AllowanceAnswer {
        mode: AllowanceMode::Included,
        width: 10.0,
    });
    assert_eq!(piece(&answered, "Waistband").allowance.seam, 10.0);
    assert_eq!(piece(&answered, "Skirt front").allowance.seam, 15.0);
    assert!(answered.allowance.pieces.is_empty());
}

#[test]
fn generic_drawing_in_millimetres() {
    let r = fixture("generic-mm.dxf");
    assert!(r.scale.certain);
    assert_eq!(r.pieces.len(), 2);

    // Rounded corners from polyline bulges: 140 × 160 mm.
    let pocket = piece(&r, "POCKET");
    let b = pocket.outline.bounds();
    assert!(
        (b.width() - 140.0).abs() < 1e-3 && (b.height() - 160.0).abs() < 1e-3,
        "{b:?}"
    );
    assert!(
        pocket.grainline.is_some(),
        "grainline from the GRAINLINE layer"
    );
    assert_eq!(pocket.marks.len(), 1);

    // Drawn as four separate lines, chained into one outline, named by its text.
    let flap = piece(&r, "Pocket flap");
    let b = flap.outline.bounds();
    assert!(
        (b.width() - 140.0).abs() < 1e-6 && (b.height() - 50.0).abs() < 1e-6,
        "{b:?}"
    );
    assert!(
        r.notes
            .contains(&ImportNote::UnsupportedEntities { count: 1 })
    );
}

#[test]
fn inches_are_converted() {
    let r = fixture("inches.dxf");
    assert!(r.scale.certain);
    let b = piece(&r, "CUFF").outline.bounds();
    assert!((b.width() - 101.6).abs() < 1e-9 && (b.height() - 50.8).abs() < 1e-9);
}

#[test]
fn a_drawing_without_units_must_be_confirmed() {
    let r = fixture("unitless.dxf");
    assert!(!r.scale.certain);
    assert_eq!(r.scale.reason, ScaleReason::NoDrawingUnits);
    assert_eq!(r.scale.chosen.basis, ScaleBasis::AssumedMm);
    assert_eq!(r.scale.alternatives.len(), 2);
}

#[test]
fn rejects_binary_and_broken_files() {
    assert!(import_dxf(b"AutoCAD Binary DXF\r\n\x1a\0", None).is_err());
    assert!(import_dxf(b"not a dxf\nat all\n", None).is_err());
}
