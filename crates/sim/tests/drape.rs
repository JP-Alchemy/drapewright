//! Garments draped on the avatar.

mod common;

use drapewright_body::measure::trunk_sections;
use drapewright_body::{Body, BodyModel, Shape, measure};
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::assemble;
use drapewright_sim::place::{Figure, Section, place};

fn avatar() -> (BodyModel, Body) {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    let model = BodyModel::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let body = model.shape(&Shape::default());
    (model, body)
}

fn figure(model: &BodyModel, body: &Body) -> (Figure, f64) {
    let (_, marks) = measure(model, body);
    let shoulders = marks.points["shoulderPoint"][1];
    let hip = marks.levels["hip"];
    let sections = trunk_sections(model, body, hip - 400.0, shoulders, 20.0)
        .into_iter()
        .map(|(y, middle, half)| Section { y, middle, half })
        .collect();
    (
        Figure {
            waist: marks.levels["waist"],
            shoulders,
            sections,
        },
        hip,
    )
}

#[test]
fn a_skirt_is_sewn_and_hangs_from_the_waist() {
    let (model, body) = avatar();
    let (figure, hip) = figure(&model, &body);
    let (pieces, seams) = common::skirt();
    let garment = assemble(&pieces, &seams, 20.0).unwrap();
    let starts = place(&garment, &figure);
    let shown = model.asset.shown;
    let triangles: Vec<[u32; 3]> = model
        .asset
        .triangles
        .iter()
        .filter(|t| t.iter().all(|&v| v < shown))
        .copied()
        .collect();
    let mut drape = Drape::new(
        &garment,
        &starts,
        preset("poplin").unwrap(),
        (&body.vertices, &triangles),
    );
    let mut frames = 0;
    while !drape.settled() && frames < 60 * 8 {
        drape.step(1.0 / 60.0);
        frames += 1;
    }
    eprintln!(
        "{} points; {frames} frames; widest stitch {:.1} mm; mean speed {:.4}",
        drape.cloth.x.len(),
        drape.cloth.widest_stitch() * 1000.0,
        drape.cloth.mean_speed()
    );
    assert!(
        drape.settled(),
        "still moving at {} m/s",
        drape.cloth.mean_speed()
    );
    // Seams closed: within a few millimetres where they lie on the body.
    let gap = drape.cloth.widest_stitch();
    assert!(gap < 0.005, "a seam is {gap} m open");
    // Clear of the body, at its points and across its triangles.
    let deepest = drape
        .cloth
        .x
        .iter()
        .map(|&p| drape.clearance(p))
        .fold(f32::MAX, f32::min);
    assert!(deepest > 0.002, "cloth {deepest} m from the body");
    let middles = drape
        .cloth
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| drape.cloth.x[i as usize]);
            drape.clearance([0, 1, 2].map(|k| (a[k] + b[k] + c[k]) / 3.0))
        })
        .fold(f32::MAX, f32::min);
    assert!(
        middles > 0.002,
        "a triangle's middle {middles} m from the body"
    );
    // It hangs from the waist to well below the hip.
    let top = drape.cloth.x.iter().map(|p| p[1]).fold(f32::MIN, f32::max) * 1000.0;
    let bottom = drape.cloth.x.iter().map(|p| p[1]).fold(f32::MAX, f32::min) * 1000.0;
    eprintln!(
        "top {top:.0} mm (waist {:.0}), bottom {bottom:.0} mm (hip {hip:.0})",
        figure.waist
    );
    assert!((f64::from(top) - figure.waist).abs() < 80.0, "top {top}");
    assert!(f64::from(bottom) < hip - 200.0, "bottom {bottom}");
}
