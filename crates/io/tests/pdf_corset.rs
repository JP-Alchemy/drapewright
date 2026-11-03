//! A corset's boning and lacing read from a PDF pattern
//! (`fixtures/pdf/corset-boning.pdf`, written by `make_corset.py`): a
//! channel with "Boning" printed between its lines, a bone down a seam to a
//! "Stop / Boning / Here" mark, and eyelets down the centre back between
//! two lacing bones.

use std::path::Path;

use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::geom::distance_to_polyline;
use drapewright_pattern_core::stitching::{BONE_INSET, BONE_SHORT, line_points, stitching_edges};
use drapewright_pattern_core::{LineKind, MarkKind, Piece};
use drapewright_seams::{Reason, suggest};

fn corset() -> Vec<Piece> {
    let bytes = std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/pdf/corset-boning.pdf"),
    )
    .unwrap();
    import_pdf(&bytes, None).unwrap().pieces
}

fn named<'a>(pieces: &'a [Piece], name: &str) -> &'a Piece {
    pieces.iter().find(|p| p.name == name).unwrap()
}

/// A piece's boning lines: their names, and each one's ends (mm).
fn bones(piece: &Piece) -> Vec<(String, [f64; 4])> {
    piece
        .lines
        .iter()
        .filter(|l| l.kind == LineKind::Boning)
        .map(|l| {
            let pts = line_points(l);
            let (a, b) = (pts[0], pts[pts.len() - 1]);
            (l.label.clone().unwrap_or_default(), [a.x, a.y, b.x, b.y])
        })
        .collect()
}

#[test]
fn a_channel_with_boning_printed_between_its_lines_is_boning() {
    let pieces = corset();
    let front = bones(named(&pieces, "Front center"));
    assert_eq!(front.len(), 2, "{front:?}");
    for (name, [x0, _, x1, _]) in &front {
        assert_eq!(name, "Boning");
        assert!((x0 - x1).abs() < 0.1 && [66.0, 74.0].iter().any(|x| (x0 - x).abs() < 0.5));
    }
    // Its waist line is still its waist.
    assert!(
        named(&pieces, "Front center")
            .lines
            .iter()
            .any(|l| l.kind == LineKind::Line && l.label.as_deref() == Some("Waist"))
    );
}

#[test]
fn a_bone_runs_up_the_seam_from_the_mark_where_it_stops() {
    let pieces = corset();
    let side = named(&pieces, "Front side");
    // The mark itself is a plain line, named for what it says.
    assert!(
        side.lines
            .iter()
            .any(|l| l.kind == LineKind::Line && l.label.as_deref() == Some("Stop boning here"))
    );
    // The bone: up the right seam, beside it, from the mark (y = 300)
    // towards the waist, to near the top (y = 30).
    let right = &side.outline.edges[1].id;
    let bone = side
        .lines
        .iter()
        .find(|l| l.id == format!("bone-{right}"))
        .unwrap();
    assert_eq!(bone.kind, LineKind::Boning);
    let pts = line_points(bone);
    let ys: Vec<f64> = pts.iter().map(|p| p.y).collect();
    let (low, high) = ys
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), &y| (l.min(y), h.max(y)));
    assert!((low - (30.0 + BONE_SHORT)).abs() < 1.0, "{low}");
    assert!((high - 300.0).abs() < 1.0, "{high}");
    let seam = &stitching_edges(side)[1];
    for &p in &pts {
        let d = distance_to_polyline(p, seam, false);
        assert!((d - BONE_INSET).abs() < 0.5, "{p:?} {d}");
    }
}

#[test]
fn eyelets_down_the_centre_back_lace_it_between_its_lacing_bones() {
    let pieces = corset();
    let back = named(&pieces, "Back center");
    let lacing = bones(back);
    assert_eq!(lacing.len(), 4, "{lacing:?}");
    assert!(lacing.iter().all(|(name, _)| name == "Back Lacing Bone"));
    let eyelets = back
        .marks
        .iter()
        .filter(|m| m.kind == MarkKind::Circle)
        .count();
    assert_eq!(eyelets, 11);
    // Cut as pairs, as printed: the back is laced to its other half, and
    // the front sewn to its own.
    assert!(pieces.iter().all(|p| p.cut[0].quantity == 2));
    let suggested = suggest(&pieces, &[]);
    let back_id = &back.id;
    let laced = suggested
        .iter()
        .find(|s| s.seam.a.piece == *back_id && s.seam.b.piece == *back_id)
        .unwrap();
    assert_eq!(laced.seam.lacing, Some(50.0));
    assert!(laced.reasons.contains(&Reason::Eyelets));
    let front_id = &named(&pieces, "Front center").id;
    let front = suggested
        .iter()
        .find(|s| s.seam.a.piece == *front_id && s.seam.b.piece == *front_id)
        .unwrap();
    assert_eq!(front.seam.lacing, None);
}
