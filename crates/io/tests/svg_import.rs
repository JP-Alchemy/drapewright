//! SVG import against the fixtures in `fixtures/svg`, plus export → import
//! round trips.

// Detected allowances snap to exact constants, so they compare exactly.
#![allow(clippy::float_cmp)]

use std::path::Path;

use drapewright_io::svg::{SvgExportOptions, export_svg, import_svg};
use drapewright_io::{ImportNote, ImportResult, ScaleReason};
use drapewright_pattern_core::geom::flatten_segs;
use drapewright_pattern_core::{
    Edge, EdgeKind, Grainline, MarkKind, NotchKind, Outline, Piece, Pt, ScaleBasis, Segment,
};
use kurbo::{Point, Rect};

fn fixture(name: &str) -> ImportResult {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/svg")
        .join(name);
    let data = std::fs::read(&path).unwrap();
    import_svg(&data, Some(name)).unwrap()
}

fn bounds(piece: &Piece) -> Rect {
    piece.outline.bounds()
}

fn assert_rect(r: Rect, x0: f64, y0: f64, x1: f64, y1: f64) {
    let ok = [(r.x0, x0), (r.y0, y0), (r.x1, x1), (r.y1, y1)]
        .iter()
        .all(|(a, b)| (a - b).abs() < 2e-3);
    assert!(ok, "{r:?} vs ({x0}, {y0})–({x1}, {y1})");
}

fn piece<'a>(result: &'a ImportResult, name: &str) -> &'a Piece {
    result
        .pieces
        .iter()
        .find(|p| p.name == name)
        .unwrap_or_else(|| {
            panic!(
                "no piece {name:?} in {:?}",
                result.pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
            )
        })
}

#[test]
fn inkscape_document_in_millimetres() {
    let r = fixture("inkscape-mm.svg");
    assert!(r.scale.certain);
    assert_eq!(r.scale.reason, ScaleReason::DeclaredUnits);
    assert_eq!(r.pieces.len(), 2);

    let skirt = piece(&r, "Skirt front");
    // Translated by (10, 20) from its drawn coordinates; the hem curve dips
    // to y = 638.75 inside the control polygon.
    let b = bounds(skirt);
    assert!(
        (b.x0 - 10.0).abs() < 1e-6 && (b.x1 - 310.0).abs() < 1e-6 && (b.y0 - 20.0).abs() < 1e-6
    );
    assert!(skirt.outline.signed_area() > 0.0);
    assert_eq!(skirt.outline.edges.len(), 4);
    assert_eq!(
        skirt.grainline,
        Some(Grainline {
            from: Pt::new(130.0, 170.0),
            to: Pt::new(130.0, 470.0),
            one_way: false
        })
    );
    assert_eq!(skirt.marks.len(), 1);
    assert_eq!(skirt.marks[0].kind, MarkKind::Dot);
    assert!((skirt.marks[0].size.unwrap() - 6.0).abs() < 0.01);

    let waistband = piece(&r, "Waistband");
    assert_rect(bounds(waistband), 360.0, 20.0, 440.0, 720.0);

    assert!(
        r.notes
            .contains(&ImportNote::SmallShapesIgnored { count: 1 })
    );
    assert!(
        r.notes
            .contains(&ImportNote::LooseLinesIgnored { count: 1 })
    );
    // One outline each, and an SVG cannot say which line it is: ask.
    assert_eq!(r.allowance.pieces.len(), 2);
    assert_eq!(r.allowance.suggested, None);
    insta::assert_json_snapshot!(r);
}

#[test]
fn illustrator_points_and_rescaling() {
    let mut r = fixture("illustrator-pt.svg");
    assert!(!r.scale.certain);
    assert_eq!(r.scale.reason, ScaleReason::IllustratorFile);
    assert_eq!(r.scale.chosen.basis, ScaleBasis::Assumed72Dpi);
    let pocket = piece(&r, "Pocket");
    // 283.46 × 141.73 pt is 100 × 50 mm.
    let b = bounds(pocket);
    assert!(
        (b.width() - 100.0).abs() < 0.01 && (b.height() - 50.0).abs() < 0.01,
        "{b:?}"
    );

    // The user says it is really 96 per inch: three quarters the size.
    let other = r
        .scale
        .alternatives
        .iter()
        .find(|a| a.basis == ScaleBasis::Assumed96Dpi)
        .unwrap()
        .clone();
    r.rescale(&other, true);
    let b = bounds(&r.pieces[0]);
    assert!(
        (b.width() - 75.0).abs() < 0.01 && (b.height() - 37.5).abs() < 0.01,
        "{b:?}"
    );
    assert!(r.scale.certain);
    let scale = r.pieces[0].source.as_ref().unwrap().scale.unwrap();
    assert_eq!(scale.basis, ScaleBasis::Assumed96Dpi);
    assert!(scale.confirmed);
}

#[test]
fn no_units_means_the_scale_is_a_guess() {
    let r = fixture("unitless.svg");
    assert!(!r.scale.certain);
    assert_eq!(r.scale.reason, ScaleReason::NoUnits);
    let collar = piece(&r, "Collar");
    // 300 × 100 px at 96 per inch.
    assert!((bounds(collar).width() - 79.375).abs() < 1e-3);
    assert!(!collar.source.as_ref().unwrap().scale.unwrap().confirmed);
}

#[test]
fn cutting_and_stitching_lines_pair_up() {
    let r = fixture("cut-and-seam.svg");
    assert_eq!(r.pieces.len(), 1);
    let facing = piece(&r, "Facing");
    // The stitching line is the outline; the allowance is measured.
    assert_rect(bounds(facing), 20.0, 20.0, 220.0, 120.0);
    assert_eq!(facing.allowance.seam, 15.0);
    let deep: Vec<_> = facing
        .outline
        .edges
        .iter()
        .filter_map(|e| e.allowance)
        .collect();
    assert_eq!(deep, [30.0]);
    assert!(r.notes.contains(&ImportNote::SeamLineFound {
        piece: facing.id.clone(),
        allowance: 15.0
    }));
    assert!(
        !r.notes
            .iter()
            .any(|n| matches!(n, ImportNote::CuttingLineDiffers { .. })),
        "{:?}",
        r.notes
    );
    // Measured, so nothing to ask.
    assert!(r.allowance.pieces.is_empty());
    insta::assert_json_snapshot!(r);
}

#[test]
fn transforms_and_use_elements() {
    let r = fixture("transforms.svg");
    assert_eq!(r.pieces.len(), 2);
    let mut sizes: Vec<(f64, f64)> = r
        .pieces
        .iter()
        .map(|p| {
            let b = bounds(p);
            (
                (b.width() * 1000.0).round() / 1000.0,
                (b.height() * 1000.0).round() / 1000.0,
            )
        })
        .collect();
    sizes.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(sizes, [(60.0, 100.0), (100.0, 60.0)]);
    assert!(r.pieces.iter().all(|p| p.outline.signed_area() > 0.0));
}

#[test]
fn freesewing_patterns_keep_their_names_folds_notches_and_cutting() {
    let r = fixture("freesewing-sasha-sa10.svg");
    let names: Vec<&str> = r.pieces.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Front inside",
            "Pocket",
            "Front outside above",
            "Front outside below",
            "Back inside",
            "Back outside",
            "Sleeve"
        ]
    );
    // Every cutting line pairs with its stitching line, whether FreeSewing
    // left it open along the fold or at a corner: the allowance is measured.
    let measured = r
        .notes
        .iter()
        .filter(|n| matches!(n, ImportNote::SeamLineFound { allowance, .. } if *allowance == 10.0))
        .count();
    assert_eq!(measured, 7);
    assert!(r.allowance.pieces.is_empty());

    let front = piece(&r, "Front inside");
    let folds = front
        .outline
        .edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Fold)
        .count();
    assert_eq!(folds, 1);
    assert_eq!(
        (front.cut[0].quantity, front.cut[0].on_fold),
        (1, Some(true))
    );
    assert!(
        front.grainline.is_some(),
        "from the cut-on-fold-and-grainline bracket"
    );

    let pocket = piece(&r, "Pocket");
    assert_eq!(
        (pocket.cut[0].quantity, pocket.cut[0].mirrored),
        (2, Some(true))
    );
    // Every notch symbol lands on an outline; the back notch is double.
    let notches: Vec<NotchKind> = r
        .pieces
        .iter()
        .flat_map(|p| p.notches.iter().map(|n| n.kind))
        .collect();
    assert_eq!(notches.len(), 14);
    assert_eq!(
        notches.iter().filter(|k| **k == NotchKind::Double).count(),
        1
    );
    assert!(r.pieces.iter().all(|p| p.grainline.is_some()));
    // The logo on the sleeve and the second copies FreeSewing draws of the
    // front neckline and back armhole are left out; the pocket opening stays.
    assert!(piece(&r, "Sleeve").lines.is_empty());
    assert!(piece(&r, "Back inside").lines.is_empty());
    assert!(piece(&r, "Front inside").lines.is_empty());
    assert_eq!(piece(&r, "Front outside above").lines.len(), 1);
    assert!(
        r.notes
            .contains(&ImportNote::LinesOnOutlineIgnored { count: 2 })
    );
}

#[test]
fn freesewing_folds_without_seam_allowance() {
    let r = fixture("freesewing-sasha-no-allowance.svg");
    // No cutting lines, so each outline could be either line: she is asked.
    assert_eq!(r.allowance.pieces.len(), 7);
    let front = piece(&r, "Front inside");
    // The fold comes from the cut-on-fold bracket.
    assert_eq!(
        front
            .outline
            .edges
            .iter()
            .filter(|e| e.kind == EdgeKind::Fold)
            .count(),
        1
    );
}

#[test]
fn rejects_files_that_are_not_svg() {
    assert!(import_svg(b"not xml at all", None).is_err());
    assert!(import_svg(b"<html></html>", None).is_err());
    assert!(import_svg(&[0x1f, 0x8b, 0x08, 0x00], None).is_err());
}

fn sample_outline(p: &Piece) -> Vec<Point> {
    flatten_segs(&p.outline.path_segs(), 0.01)
}

fn max_gap(a: &[Point], b: &[Point]) -> f64 {
    let d = |p: Point, pts: &[Point]| {
        drapewright_pattern_core::geom::distance_to_polyline(p, pts, true)
    };
    a.iter()
        .map(|p| d(*p, b))
        .chain(b.iter().map(|p| d(*p, a)))
        .fold(0.0, f64::max)
}

fn test_pieces() -> Vec<Piece> {
    let mut skirt = Piece::new(
        "p1",
        "Skirt back",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new(
                    "w",
                    vec![Segment::cubic([70.0, 4.0], [140.0, 8.0], [200.0, 12.0])],
                ),
                Edge::new("s", vec![Segment::line([290.0, 580.0])]),
                Edge::new(
                    "h",
                    vec![Segment::cubic([200.0, 595.0], [100.0, 600.0], [0.0, 600.0])],
                ),
                Edge::new("cb", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    skirt.allowance.seam = 15.0;
    skirt.outline.edges[2].allowance = Some(30.0);
    skirt.grainline = Some(Grainline {
        from: Pt::new(100.0, 100.0),
        to: Pt::new(100.0, 400.0),
        one_way: false,
    });
    skirt.placement.x = 20.0;

    let mut patch = Piece::new(
        "p2",
        "Patch pocket",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("a", vec![Segment::line([140.0, 0.0])]),
                Edge::new("b", vec![Segment::line([140.0, 160.0])]),
                Edge::new("c", vec![Segment::line([0.0, 160.0])]),
                Edge::new("d", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    patch.allowance.seam = 10.0;
    patch.placement.x = 380.0;
    vec![skirt, patch]
}

#[test]
fn export_then_import_gives_back_the_pieces() {
    let original = test_pieces();
    let svg = export_svg(&original, &SvgExportOptions::default());
    let back = import_svg(svg.as_bytes(), Some("roundtrip.svg")).unwrap();
    assert!(back.scale.certain);
    assert_eq!(back.pieces.len(), 2);
    for o in &original {
        let b = piece(&back, &o.name);
        // Same stitching line (moved by the placement used for export).
        let mut moved = o.clone();
        drapewright_pattern_core::ops::transform(
            &mut moved,
            kurbo::Affine::translate((o.placement.x, o.placement.y)),
        );
        let gap = max_gap(&sample_outline(&moved), &sample_outline(b));
        assert!(gap < 1e-3, "{}: outline moved by {gap} mm", o.name);
        // Same allowance, found by measuring between the two lines.
        assert_eq!(b.allowance.seam, o.allowance.seam, "{}", o.name);
        let widths: Vec<f64> = (0..b.outline.edges.len())
            .map(|i| b.edge_allowance(i))
            .collect();
        let mut expected: Vec<f64> = (0..o.outline.edges.len())
            .map(|i| o.edge_allowance(i))
            .collect();
        let mut got = widths.clone();
        got.sort_by(f64::total_cmp);
        expected.sort_by(f64::total_cmp);
        assert_eq!(got, expected, "{}", o.name);
        assert!(
            !back
                .notes
                .iter()
                .any(|n| matches!(n, ImportNote::CuttingLineDiffers { .. })),
            "{:?}",
            back.notes
        );
    }
    let skirt = piece(&back, "Skirt back");
    assert!(skirt.grainline.is_some());
}
