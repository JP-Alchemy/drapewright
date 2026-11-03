//! Boning and lacing: a bone sewn into the cloth keeps it from bending
//! along it, and a laced seam is held open by its lace.

mod common;

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::{InternalLine, LineKind, LineSeg, Piece, Pt, Segment};
use drapewright_seams::Seam;
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::{Garment, assemble};
use drapewright_sim::girth::girths;
use drapewright_sim::place::{Figure, place};

fn avatar() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).unwrap()).unwrap()
}

/// The test skirt, a bone down its front 60 mm from the fold, its centre
/// back laced `gap` mm open (sewn where none).
fn skirt(bone: bool, gap: Option<f64>) -> (Vec<Piece>, Vec<Seam>) {
    let (mut pieces, mut seams) = common::skirt();
    if bone {
        pieces[0].lines.push(InternalLine {
            id: "bone".into(),
            kind: LineKind::Boning,
            start: Pt::new(60.0, 40.0),
            segments: vec![Segment::Line(LineSeg {
                to: Pt::new(60.0, 560.0),
            })],
            label: Some("Boning".into()),
        });
    }
    seams[1].lacing = gap;
    (pieces, seams)
}

/// The skirt sewn, draped on the standard figure until it settles.
fn draped(pieces: &[Piece], seams: &[Seam]) -> (Garment, Drape, Figure) {
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let garment = assemble(pieces, seams, 20.0).unwrap();
    let starts = place(&garment, &figure);
    let mut drape = Drape::new(
        &garment,
        &starts,
        preset("poplin").unwrap(),
        (&body.vertices, &model.skin()),
    );
    let mut frames = 0;
    while !drape.settled() && frames < 60 * 8 {
        drape.step(1.0 / 60.0);
        frames += 1;
    }
    assert!(drape.settled(), "still moving");
    (garment, drape, figure)
}

#[test]
fn a_bone_and_a_lacing_are_sewn_into_the_garment() {
    let (pieces, seams) = skirt(true, Some(50.0));
    let g = assemble(&pieces, &seams, 20.0).unwrap();
    // The front is cut on the fold: its bone on both halves.
    assert_eq!(g.bones.len(), 2, "{:?}", g.bones);
    for bone in &g.bones {
        assert_eq!(g.panels[bone.panel].piece, "front");
        // About every 20 mm down its 520.
        assert!((20..=40).contains(&bone.points.len()), "{bone:?}");
        assert!(bone.path.len() >= bone.points.len(), "{bone:?}");
    }
    // The centre back laced, not sewn: its pairs held apart, none of its
    // points stitched to each other.
    assert_eq!(g.lacings.len(), 1);
    let lacing = &g.lacings[0];
    assert!((lacing.gap - 50.0).abs() < 1e-9);
    assert!(lacing.pairs.len() >= 25, "{} pairs", lacing.pairs.len());
    for pair in &lacing.pairs {
        assert!(!g.stitches.contains(pair) && !g.stitches.contains(&[pair[1], pair[0]]));
    }
    // Still joining the back's halves, to place them (and the sides, each
    // side of the body).
    assert_eq!(g.links.len(), 3);

    // Sewn, the same garment has neither.
    let (pieces, seams) = skirt(false, None);
    let g = assemble(&pieces, &seams, 20.0).unwrap();
    assert!(g.bones.is_empty() && g.lacings.is_empty());
}

#[test]
fn a_laced_back_is_held_open_by_the_lace() {
    let (pieces, seams) = skirt(false, Some(50.0));
    let (g, drape, figure) = draped(&pieces, &seams);
    let lacing = &g.lacings[0];
    let at = |(p, k): (usize, u32)| drape.cloth.x[(drape.firsts[p] + k) as usize];
    let apart: Vec<f32> = lacing
        .pairs
        .iter()
        .map(|&[a, b]| {
            let (x, y) = (at(a), at(b));
            ((x[0] - y[0]).powi(2) + (x[1] - y[1]).powi(2) + (x[2] - y[2]).powi(2)).sqrt() * 1000.0
        })
        .collect();
    let mean = apart.iter().sum::<f32>() / apart.len() as f32;
    assert!((mean - 50.0).abs() < 4.0, "{apart:?}");
    // Measured round the body as placed, the gap counts.
    let laced = girths(&g, &place(&g, &figure), &figure);
    let (pieces, seams) = skirt(false, None);
    let sewn_garment = assemble(&pieces, &seams, 20.0).unwrap();
    let sewn = girths(&sewn_garment, &place(&sewn_garment, &figure), &figure);
    for level in ["waist", "hip"] {
        let at = |gs: &[(&str, Option<f64>)]| gs.iter().find(|g| g.0 == level).unwrap().1.unwrap();
        let more = at(&laced) - at(&sewn);
        assert!(
            (more - 50.0).abs() < 1.0,
            "{level}: laced {laced:?}, sewn {sewn:?}"
        );
    }
}

/// How far a bone's path through the cloth (each point a triangle's
/// corners, weighed) turns along its length, all told, in radians: how
/// much the cloth there bends and creases.
fn turning(drape: &Drape, first: u32, path: &[([u32; 3], [f32; 3])]) -> f32 {
    let points: Vec<[f32; 3]> = path
        .iter()
        .map(|(t, w)| {
            let mut p = [0.0; 3];
            for (&k, &wk) in t.iter().zip(w) {
                let q = drape.cloth.x[(first + k) as usize];
                p = [p[0] + q[0] * wk, p[1] + q[1] * wk, p[2] + q[2] * wk];
            }
            p
        })
        .collect();
    points
        .windows(3)
        .map(|w| {
            let (a, b, c) = (w[0], w[1], w[2]);
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - b[0], c[1] - b[1], c[2] - b[2]];
            let dot = u[0] * v[0] + u[1] * v[1] + u[2] * v[2];
            let len = |d: [f32; 3]| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
            (dot / (len(u) * len(v)).max(1e-12)).clamp(-1.0, 1.0).acos()
        })
        .sum()
}

#[test]
fn a_bone_keeps_the_cloth_along_it_from_bending() {
    let (pieces, seams) = skirt(true, None);
    let (g, boned, _) = draped(&pieces, &seams);
    let (pieces, seams) = skirt(false, None);
    let (_, plain, _) = draped(&pieces, &seams);
    // The same points of the cloth, boned and not: the bone doesn't change
    // the mesh.
    for bone in &g.bones {
        let first = boned.firsts[bone.panel];
        let (with, without) = (
            turning(&boned, first, &bone.path),
            turning(&plain, first, &bone.path),
        );
        eprintln!("turning along the bone: {with:.3} boned, {without:.3} not");
        assert!(with < 0.6 * without, "{with} boned, {without} not");
    }
}
