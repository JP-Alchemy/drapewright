//! Garments draped on the avatar.

mod common;

use drapewright_body::{BodyModel, Shape};
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::{assemble, draped_pieces};
use drapewright_sim::place::{Figure, half_round, place};

fn avatar() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).unwrap()).unwrap()
}

#[test]
fn a_skirt_is_sewn_and_hangs_from_the_waist() {
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let hip = drapewright_body::measure(&model, &body).1.levels["hip"];
    let (pieces, seams) = common::skirt();
    let garment = assemble(&pieces, &seams, 20.0).unwrap();
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
    // And stays there: settled cloth corrected unevenly at every step
    // creeps round the body.
    let settled = drape.cloth.x.clone();
    for _ in 0..120 {
        drape.step(1.0 / 60.0);
    }
    let crept = settled
        .iter()
        .zip(&drape.cloth.x)
        .map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt())
        .fold(0.0, f32::max);
    assert!(crept < 0.002, "a point crept {crept} m in two seconds");
}

#[test]
fn a_blouse_hangs_from_the_shoulders_with_its_sleeves_round_the_arms() {
    blouse_hangs(common::blouse());
}

#[test]
fn long_sleeves_follow_the_arms_to_the_wrists() {
    blouse_hangs(common::long_sleeved_blouse());
}

#[test]
fn a_blouse_with_a_full_bust_adjustment_drapes_with_its_darts_closed() {
    blouse_hangs(common::full_bust_blouse());
}

#[test]
fn a_blouse_with_a_dart_in_its_armhole_drapes_with_it_closed() {
    blouse_hangs(common::armhole_dart_blouse());
}

fn blouse_hangs(
    (pieces, seams): (
        Vec<drapewright_pattern_core::Piece>,
        Vec<drapewright_seams::Seam>,
    ),
) {
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let garment = assemble(&pieces, &seams, 20.0).unwrap();
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
    let x = &drape.cloth.x;
    assert!(
        drape.settled(),
        "still moving at {} m/s",
        drape.cloth.mean_speed()
    );
    let gap = drape.cloth.widest_stitch();
    assert!(gap < 0.005, "a seam is {gap} m open");
    let deepest = x
        .iter()
        .map(|&p| drape.clearance(p))
        .fold(f32::MAX, f32::min);
    assert!(deepest > 0.002, "cloth {deepest} m from the body");
    // From the shoulders: its top by the neck, its hem by the hip.
    let top = x.iter().map(|p| p[1]).fold(f32::MIN, f32::max) * 1000.0;
    let hem = x.iter().map(|p| p[1]).fold(f32::MAX, f32::min) * 1000.0;
    eprintln!(
        "{frames} frames; widest stitch {:.1} mm; top {top:.0} mm (shoulders {:.0}); hem {hem:.0} mm",
        gap * 1000.0,
        figure.shoulders
    );
    assert!(
        (0.0..80.0).contains(&(f64::from(top) - figure.shoulders)),
        "top {top}"
    );
    // Evenly: cut alike on both sides, it hangs alike.
    let middle = x.iter().map(|p| p[0]).sum::<f32>() / x.len() as f32 * 1000.0;
    let lowest = |left: bool| {
        x.iter()
            .filter(|p| (p[0] > 0.0) == left)
            .map(|p| p[1])
            .fold(f32::MAX, f32::min)
            * 1000.0
    };
    eprintln!(
        "middle {middle:.1} mm; lowest {:.0} left, {:.0} right",
        lowest(true),
        lowest(false)
    );
    assert!(middle.abs() < 3.0, "off to one side by {middle} mm");
    assert!((lowest(true) - lowest(false)).abs() < 5.0);
    // Each sleeve round an arm: near the line down its middle, and out
    // beyond the shoulder.
    for (k, panel) in garment.panels.iter().enumerate() {
        if panel.piece != "sleeve" {
            continue;
        }
        let points = &x[drape.firsts[k] as usize..][..panel.mesh.points.len()];
        let left = points.iter().map(|p| p[0]).sum::<f32>() > 0.0;
        let arm = figure
            .arms
            .iter()
            .find(|a| (a.line[20][0] > 0.0) == left)
            .unwrap();
        let farthest = points
            .iter()
            .map(|p| {
                arm.line
                    .iter()
                    .map(|q| {
                        (0..3)
                            .map(|c| (f64::from(p[c]) * 1000.0 - q[c]).powi(2))
                            .sum::<f64>()
                            .sqrt()
                    })
                    .fold(f64::MAX, f64::min)
            })
            .fold(0.0, f64::max);
        let out = points.iter().map(|p| p[0].abs()).sum::<f32>() / points.len() as f32 * 1000.0;
        eprintln!("sleeve {k}: within {farthest:.0} mm of its arm, {out:.0} mm out");
        assert!(farthest < 120.0, "sleeve {k} {farthest} mm from its arm");
        assert!(out > 180.0, "sleeve {k} only {out} mm out");
    }
}

#[test]
fn a_blouse_laid_out_on_its_side_drapes_sewn_as_its_seams_are_found() {
    // As pattern CAD lays it on the sheet: its back and fronts on their
    // sides, its cuffs upside down; sewn by the seams found for it, its
    // fronts buttoned, its facing left inside.
    let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(true);
    let seams: Vec<drapewright_seams::Seam> = drapewright_seams::suggest(&pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| drapewright_seams::Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    let draped = draped_pieces(&pieces, &seams);
    let sewn = draped.sewn;
    assert!(draped.alone.is_empty(), "{:?}", draped.alone);
    assert_eq!(draped.inside, ["facing"]);
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let garment = assemble(&sewn, &seams, 20.0).unwrap();
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
    let x = &drape.cloth.x;
    assert!(
        drape.settled(),
        "still moving at {} m/s",
        drape.cloth.mean_speed()
    );
    // Seams closed, the gathers drawn up and the buttons done up.
    let gap = drape.cloth.widest_stitch();
    assert!(gap < 0.008, "a seam is {gap} m open");
    let deepest = x
        .iter()
        .map(|&p| drape.clearance(p))
        .fold(f32::MAX, f32::min);
    assert!(deepest > 0.0, "cloth {deepest} m into the body");
    // From the shoulders, by way of the yokes: its top by the neck, its
    // hem below the hip.
    let hip = drapewright_body::measure(&model, &body).1.levels["hip"];
    let top = x.iter().map(|p| p[1]).fold(f32::MIN, f32::max) * 1000.0;
    let hem = x.iter().map(|p| p[1]).fold(f32::MAX, f32::min) * 1000.0;
    eprintln!(
        "{frames} frames; widest stitch {:.1} mm; top {top:.0} mm (shoulders {:.0}); hem {hem:.0} mm (hip {hip:.0})",
        gap * 1000.0,
        figure.shoulders
    );
    assert!(
        (0.0..80.0).contains(&(f64::from(top) - figure.shoulders)),
        "top {top}"
    );
    assert!(f64::from(hem) < hip, "hem {hem}");
    // Each sleeve round an arm, and each cuff round a wrist.
    for (k, panel) in garment.panels.iter().enumerate() {
        let points = &x[drape.firsts[k] as usize..][..panel.mesh.points.len()];
        let left = points.iter().map(|p| p[0]).sum::<f32>() > 0.0;
        let arm = figure
            .arms
            .iter()
            .find(|a| (a.line[20][0] > 0.0) == left)
            .unwrap();
        let near = |p: &[f32; 3], q: &[f64; 3]| {
            (0..3)
                .map(|c| (f64::from(p[c]) * 1000.0 - q[c]).powi(2))
                .sum::<f64>()
                .sqrt()
        };
        match panel.piece.as_str() {
            "sleeve" => {
                let farthest = points
                    .iter()
                    .map(|p| arm.line.iter().map(|q| near(p, q)).fold(f64::MAX, f64::min))
                    .fold(0.0, f64::max);
                assert!(farthest < 150.0, "sleeve {k} {farthest} mm from its arm");
            }
            "cuff" => {
                // Nearer the wrist than the elbow.
                let wrist = arm.line[arm.line.len() - 6];
                let elbow = arm.line[arm.line.len() / 2];
                let mean = points.iter().fold([0.0f32; 3], |m, p| {
                    [0, 1, 2].map(|c| m[c] + p[c] / points.len() as f32)
                });
                let (to_wrist, to_elbow) = (near(&mean, &wrist), near(&mean, &elbow));
                eprintln!(
                    "cuff {k}: {to_wrist:.0} mm from the wrist, {to_elbow:.0} from the elbow"
                );
                assert!(to_wrist < to_elbow, "cuff {k} not at the wrist");
            }
            _ => {}
        }
    }
}

/// The seams found for `pieces`, confirmed.
fn found(pieces: &[drapewright_pattern_core::Piece]) -> Vec<drapewright_seams::Seam> {
    drapewright_seams::suggest(pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| drapewright_seams::Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect()
}

#[test]
fn a_corset_is_put_on_by_its_waist_line_its_crotch_piece_left_off() {
    use drapewright_pattern_core::testing::garments::corset;
    // Cut once, as the pattern leaves its panels: half way round, from the
    // centre front to the centre back, and said so.
    let pieces = corset(false);
    let seams = found(&pieces);
    let draped = draped_pieces(&pieces, &seams);
    assert_eq!(draped.alone, ["cr"]);
    assert!(half_round(&assemble(&draped.sewn, &seams, 20.0).unwrap()));
    // Cut as pairs: all the way round, the crotch piece (between the legs)
    // left off ...
    let pieces = corset(true);
    let seams = found(&pieces);
    let draped = draped_pieces(&pieces, &seams);
    assert!(draped.alone.is_empty(), "{:?}", draped.alone);
    assert_eq!(draped.between_legs, ["cr"]);
    let garment = assemble(&draped.sewn, &seams, 20.0).unwrap();
    assert!(!half_round(&garment));
    assert_eq!(garment.panels.len(), 6);
    // ... and each panel put on with its waist line at the avatar's waist,
    // not hung from its top.
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let starts = place(&garment, &figure);
    for (k, panel) in garment.panels.iter().enumerate() {
        let waist = panel.waist.expect("a waist line");
        let off = panel
            .mesh
            .points
            .iter()
            .zip(&starts[k])
            .map(|(q, p)| (p[1] - figure.waist) - (q[1] - waist))
            .fold(0.0f64, |m, d| m.max(d.abs()));
        assert!(off < 0.5, "{} {:?}: {off} mm off", panel.piece, panel.copy);
    }
}
