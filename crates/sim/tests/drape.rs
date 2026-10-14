//! Garments draped on the avatar.

mod common;

use drapewright_body::{BodyModel, Pose, Shape};
use drapewright_sim::drape::{Drape, Posable};
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
    // fronts buttoned (its facing goes inside: see below).
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
    let lined: Vec<&str> = draped.lined.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(lined, ["facing"]);
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
fn a_corset_is_put_on_by_its_waist_line_its_crotch_piece_under_the_crotch() {
    use drapewright_pattern_core::testing::garments::corset;
    // Cut once, as the pattern leaves its panels: half way round, from the
    // centre front to the centre back, and said so.
    let pieces = corset(false);
    let seams = found(&pieces);
    let draped = draped_pieces(&pieces, &seams);
    assert_eq!(draped.alone, ["cr"]);
    assert!(half_round(&assemble(&draped.sewn, &seams, 20.0).unwrap()));
    // Cut as pairs: all the way round, with the crotch piece ...
    let pieces = corset(true);
    let seams = found(&pieces);
    let draped = draped_pieces(&pieces, &seams);
    assert!(draped.alone.is_empty(), "{:?}", draped.alone);
    let garment = assemble(&draped.sewn, &seams, 20.0).unwrap();
    assert!(!half_round(&garment));
    assert_eq!(garment.panels.len(), 7);
    // ... and each panel put on with its waist line at the avatar's waist,
    // not hung from its top.
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let starts = place(&garment, &figure);
    for (k, panel) in garment.panels.iter().enumerate() {
        if panel.piece == "cr" {
            continue;
        }
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
    // The crotch piece (its front: sewn to the fronts) from in front of the
    // body to under the crotch.
    let crotch = garment.panels.iter().position(|p| p.piece == "cr").unwrap();
    let ys: Vec<f64> = starts[crotch].iter().map(|p| p[1]).collect();
    let zs: Vec<f64> = starts[crotch].iter().map(|p| p[2]).collect();
    let lowest = ys.iter().copied().fold(f64::MAX, f64::min);
    assert!(
        lowest < figure.crotch && lowest > figure.crotch - 30.0,
        "lowest {lowest}, crotch {}",
        figure.crotch
    );
    let middle = figure.middle(figure.crotch + 70.0)[1];
    let (front, back) = zs
        .iter()
        .fold((f64::MIN, f64::MAX), |(f, b), &z| (f.max(z), b.min(z)));
    assert!(
        front > middle + 50.0 && (back - middle).abs() < 10.0,
        "{back}..{front} round {middle}"
    );
    // Its centre front, where the centre front's halves are sewn, at the
    // front of the body, not off to one side.
    for (k, panel) in garment.panels.iter().enumerate() {
        let Some(centre) = panel.edges.iter().find(|e| e.edge == "fcl") else {
            continue;
        };
        for &i in &centre.points {
            let p = starts[k][i as usize];
            let section = figure
                .sections
                .iter()
                .min_by(|a, b| (a.y - p[1]).abs().total_cmp(&(b.y - p[1]).abs()))
                .unwrap();
            let across = p[0] - section.middle[0];
            assert!(
                across.abs() < 5.0 && p[2] > section.middle[1],
                "{:?}: the centre front {across:.1} mm across",
                panel.copy
            );
        }
    }
}

#[test]
fn a_garment_is_measured_round_the_body_flat_without_its_sleeves() {
    use drapewright_sim::girth::girths;
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let measured = |(pieces, seams): (
        Vec<drapewright_pattern_core::Piece>,
        Vec<drapewright_seams::Seam>,
    )| {
        let draped = draped_pieces(&pieces, &seams);
        let g = assemble(&draped.sewn, &seams, 20.0).unwrap();
        let starts = place(&g, &figure);
        girths(&g, &starts, &figure)
    };
    let at = |gs: &[(&str, Option<f64>)], level: &str| gs.iter().find(|g| g.0 == level).unwrap().1;
    // A tube of two 300 mm rectangles is 600 mm round where it reaches,
    // though it starts stretched round a bigger body.
    let tube = measured(tube());
    let reached: Vec<f64> = tube.iter().filter_map(|g| g.1).collect();
    assert!(!reached.is_empty(), "{tube:?}");
    for girth in reached {
        assert!((girth - 600.0).abs() < 1.0, "{tube:?}");
    }
    // A skirt reaches the waist and hip, not the bust.
    let skirt = measured(common::skirt());
    assert_eq!(at(&skirt, "bust"), None);
    assert!(
        at(&skirt, "hip").is_some_and(|g| (900.0..1100.0).contains(&g)),
        "{skirt:?}"
    );
    // The blouse's sleeves are not counted round the bust.
    let (pieces, seams) = common::blouse();
    let with_sleeves = measured((pieces.clone(), seams.clone()));
    let bodice: Vec<_> = pieces
        .into_iter()
        .filter(|p| !p.name.contains("leeve"))
        .collect();
    let ids: Vec<&str> = bodice.iter().map(|p| p.id.as_str()).collect();
    let seams: Vec<_> = seams
        .into_iter()
        .filter(|s| ids.contains(&s.a.piece.as_str()) && ids.contains(&s.b.piece.as_str()))
        .collect();
    let without = measured((bodice, seams));
    let (a, b) = (
        at(&with_sleeves, "bust").unwrap(),
        at(&without, "bust").unwrap(),
    );
    assert!((a - b).abs() < 1.0, "with sleeves {a}, without {b}");
}

#[test]
fn cloth_only_part_of_the_way_round_is_not_measured() {
    // A bunnysuit cut up for the legs doesn't go round the hips: here, the
    // tube sewn at its sides only down to just below the waist, open below.
    use drapewright_sim::girth::girths;
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let (pieces, mut seams) = tube();
    for seam in &mut seams {
        // Each side edge runs down one piece and up the other.
        for side in [&mut seam.a, &mut seam.b] {
            let down = side.edge.ends_with("right");
            (side.from, side.to) = if down {
                (Some(0.0), Some(80.0))
            } else {
                (Some(320.0), Some(400.0))
            };
        }
    }
    let draped = draped_pieces(&pieces, &seams);
    let g = assemble(&draped.sewn, &seams, 20.0).unwrap();
    let starts = place(&g, &figure);
    let measured = girths(&g, &starts, &figure);
    let at = |level: &str| measured.iter().find(|m| m.0 == level).unwrap().1;
    assert!(
        at("waist").is_some_and(|w| (w - 600.0).abs() < 1.0),
        "{measured:?}"
    );
    assert_eq!(at("hip"), None, "{measured:?}");
}

/// A tube of two rectangles, each 300 mm wide and 400 mm long, sewn at both
/// sides: 600 mm round wherever it reaches.
fn tube() -> (
    Vec<drapewright_pattern_core::Piece>,
    Vec<drapewright_seams::Seam>,
) {
    use drapewright_pattern_core::{
        CutInstruction, Edge, EdgeKind, Grainline, Outline, Piece, Pt, Segment,
    };
    use drapewright_seams::{Seam, SeamSide};
    let edge = |id: String, kind: EdgeKind, to: [f64; 2]| Edge {
        kind,
        ..Edge::new(&id, vec![Segment::line(to)])
    };
    let rectangle = |id: &str, name: &str| {
        let mut p = Piece::new(
            id,
            name,
            Outline {
                start: Pt::new(0.0, 0.0),
                edges: vec![
                    edge(format!("{id}top"), EdgeKind::Seam, [300.0, 0.0]),
                    edge(format!("{id}right"), EdgeKind::Seam, [300.0, 400.0]),
                    edge(format!("{id}hem"), EdgeKind::Hem, [0.0, 400.0]),
                    edge(format!("{id}left"), EdgeKind::Seam, [0.0, 0.0]),
                ],
            },
        );
        p.cut = vec![CutInstruction {
            quantity: 1,
            ..Default::default()
        }];
        p.grainline = Some(Grainline {
            from: Pt::new(150.0, 100.0),
            to: Pt::new(150.0, 300.0),
            one_way: false,
        });
        p
    };
    let side = |id: &str, a: (&str, &str), b: (&str, &str)| Seam {
        id: id.into(),
        name: String::new(),
        a: SeamSide::whole(a.0, a.1),
        b: SeamSide::whole(b.0, b.1),
        reversed: true,
        ease: Default::default(),
        confirmed: true,
    };
    (
        vec![
            rectangle("front", "Tube front"),
            rectangle("back", "Tube back"),
        ],
        vec![
            side("s1", ("front", "frontright"), ("back", "backleft")),
            side("s2", ("back", "backright"), ("front", "frontleft")),
        ],
    )
}

#[test]
fn panels_are_placed_where_their_seams_meet_not_by_their_tops() {
    // A front whose sides are sewn from 100 mm below its top (above them,
    // armholes, say), to a back sewn all down its sides: placed with the
    // seams' two sides level, the back's top 100 mm below the front's (not
    // the tops level, the seams 100 mm apart until sewn).
    use drapewright_pattern_core::{
        CutInstruction, Edge, EdgeKind, Grainline, Outline, Piece, Pt, Segment,
    };
    use drapewright_seams::{Seam, SeamSide};
    let edge = |id: &str, kind: EdgeKind, to: [f64; 2]| Edge {
        kind,
        ..Edge::new(id, vec![Segment::line(to)])
    };
    let panel = |id: &str, name: &str, edges: Vec<Edge>| {
        let mut p = Piece::new(
            id,
            name,
            Outline {
                start: Pt::new(0.0, 0.0),
                edges,
            },
        );
        p.cut = vec![CutInstruction {
            quantity: 1,
            ..Default::default()
        }];
        p.grainline = Some(Grainline {
            from: Pt::new(150.0, 150.0),
            to: Pt::new(150.0, 350.0),
            one_way: false,
        });
        p
    };
    use EdgeKind::{Hem, Seam as Sewn};
    let front = panel(
        "front",
        "Front",
        vec![
            edge("ftop", Sewn, [300.0, 0.0]),
            edge("farm", Sewn, [300.0, 100.0]),
            edge("fright", Sewn, [300.0, 500.0]),
            edge("fhem", Hem, [0.0, 500.0]),
            edge("fleft", Sewn, [0.0, 100.0]),
            edge("farm2", Sewn, [0.0, 0.0]),
        ],
    );
    let back = panel(
        "back",
        "Back",
        vec![
            edge("btop", Sewn, [300.0, 0.0]),
            edge("bright", Sewn, [300.0, 400.0]),
            edge("bhem", Hem, [0.0, 400.0]),
            edge("bleft", Sewn, [0.0, 0.0]),
        ],
    );
    let side = |id: &str, a: (&str, &str), b: (&str, &str)| Seam {
        id: id.into(),
        name: String::new(),
        a: SeamSide::whole(a.0, a.1),
        b: SeamSide::whole(b.0, b.1),
        reversed: true,
        ease: Default::default(),
        confirmed: true,
    };
    let seams = vec![
        side("s1", ("front", "fright"), ("back", "bleft")),
        side("s2", ("back", "bright"), ("front", "fleft")),
    ];
    let pieces = vec![front, back];
    let draped = draped_pieces(&pieces, &seams);
    let g = assemble(&draped.sewn, &seams, 20.0).unwrap();
    let model = avatar();
    let body = model.shape(&Shape::default());
    let starts = place(&g, &Figure::of(&model, &body));
    let apart = g
        .stitches
        .iter()
        .map(|&[(pa, a), (pb, b)]| (starts[pa][a as usize][1] - starts[pb][b as usize][1]).abs())
        .fold(0.0, f64::max);
    assert!(apart < 2.0, "the seams' sides {apart} mm apart");
}

#[test]
fn trousers_go_round_each_leg_below_the_crotch() {
    use drapewright_pattern_core::testing::garments::trousers;
    let pieces = trousers();
    let seams = found(&pieces);
    let draped = draped_pieces(&pieces, &seams);
    let garment = assemble(&draped.sewn, &seams, 20.0).unwrap();
    let model = avatar();
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    assert_eq!(figure.legs.len(), 2);
    let starts = place(&garment, &figure);
    // Well below the crotch, each panel all on one side: two legs, not one
    // tube round both; and each side with a front and a back on it.
    let below = figure.crotch - 150.0;
    // Which side each panel is on below the crotch: the figure's left (+x).
    let mut sides: Vec<(bool, String)> = Vec::new();
    for (k, panel) in garment.panels.iter().enumerate() {
        let xs: Vec<f64> = starts[k]
            .iter()
            .filter(|p| p[1] < below)
            .map(|p| p[0])
            .collect();
        assert!(!xs.is_empty(), "{}", panel.name);
        let (low, high) = xs
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), &x| (l.min(x), h.max(x)));
        assert!(
            low > 0.0 || high < 0.0,
            "{} {:?}: {low}..{high}",
            panel.name,
            panel.copy
        );
        sides.push((low > 0.0, panel.name.clone()));
    }
    for side in [true, false] {
        let names: Vec<&str> = sides
            .iter()
            .filter(|(s, _)| *s == side)
            .map(|(_, n)| n.as_str())
            .collect();
        assert!(
            names.contains(&"Trouser front") && names.contains(&"Trouser back"),
            "{side}: {names:?}"
        );
    }
}

/// Drapes `garment` on the standard figure until it settles, ready to pose.
fn settled(
    model: &BodyModel,
    pieces: &[drapewright_pattern_core::Piece],
    seams: &[drapewright_seams::Seam],
) -> Drape {
    let body = model.shape(&Shape::default());
    let figure = Figure::of(model, &body);
    let garment = assemble(pieces, seams, 20.0).unwrap();
    let starts = place(&garment, &figure);
    let mut drape = Drape::new(
        &garment,
        &starts,
        preset("poplin").unwrap(),
        (&body.vertices, &model.skin()),
    );
    drape.posable(Posable::of(model, &body).unwrap());
    let mut frames = 0;
    while !drape.settled() && frames < 60 * 8 {
        drape.step(1.0 / 60.0);
        frames += 1;
    }
    assert!(drape.settled());
    drape
}

/// Moves the body into `pose` until the cloth settles again, in at most
/// `seconds`; how long that took.
fn pose(drape: &mut Drape, pose: Pose, seconds: f32) -> f32 {
    let start = drape.time;
    drape.pose(pose);
    assert!(drape.moving());
    while (drape.moving() || !drape.settled()) && drape.time - start < seconds {
        drape.step(1.0 / 60.0);
    }
    assert!(!drape.moving(), "still moving after {seconds} s");
    drape.time - start
}

/// How much of the cloth is within the body, as a share of its points.
fn inside(drape: &Drape) -> f32 {
    let x = &drape.cloth.x;
    x.iter().filter(|&&p| drape.clearance(p) < 0.0).count() as f32 / x.len() as f32
}

#[test]
fn sleeves_go_with_the_arms_as_they_reach_forward() {
    let model = avatar();
    let (pieces, seams) = common::long_sleeved_blouse();
    let mut drape = settled(&model, &pieces, &seams);
    let front = |d: &Drape| d.cloth.x.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
    let before = front(&drape);
    let took = pose(&mut drape, Pose::Reaching, 8.0);
    let after = front(&drape);
    eprintln!(
        "reaching: {took:.1} s; the cloth's front from {before:.3} to {after:.3} m; {:.4} inside; widest stitch {:.1} mm",
        inside(&drape),
        drape.cloth.widest_stitch() * 1000.0
    );
    // The sleeves reach forward with the arms, to near the wrists half a
    // metre in front.
    assert!(
        after > 0.45 && after - before > 0.2,
        "the cloth reaches {before} m, then {after} m"
    );
    assert!(
        inside(&drape) < 0.01,
        "{} of the cloth inside the body",
        inside(&drape)
    );
    // The seams stay closed.
    assert!(
        drape.cloth.widest_stitch() < 0.005,
        "a seam {} mm open",
        drape.cloth.widest_stitch() * 1000.0
    );
    // The body is shown as it moved.
    let (_, points) = drape.body().unwrap();
    assert_eq!(points.len(), model.asset.base.len());
}

#[test]
fn a_skirt_moves_with_the_legs_in_a_stride() {
    let model = avatar();
    let (pieces, seams) = common::skirt();
    let mut drape = settled(&model, &pieces, &seams);
    let took = pose(&mut drape, Pose::Stepping, 8.0);
    eprintln!("stepping: {took:.1} s; {:.4} inside", inside(&drape));
    assert!(
        inside(&drape) < 0.01,
        "{} of the cloth inside the body",
        inside(&drape)
    );
    // And back to standing.
    pose(&mut drape, Pose::Standing, 8.0);
    assert!(
        inside(&drape) < 0.01,
        "{} of the cloth inside the body",
        inside(&drape)
    );
}

#[test]
fn a_skirt_sat_in_rests_on_the_seat() {
    let model = avatar();
    let (pieces, seams) = common::skirt();
    let mut drape = settled(&model, &pieces, &seams);
    pose(&mut drape, Pose::Sitting, 8.0);
    let seat = drape.seat().expect("a seat");
    // Nothing through the seat: over it, the cloth is on it or above.
    let over = |p: &&[f32; 3]| {
        (seat.low[0]..seat.high[0]).contains(&p[0]) && (seat.low[2]..seat.high[2]).contains(&p[2])
    };
    let lowest = drape
        .cloth
        .x
        .iter()
        .filter(over)
        .map(|p| p[1])
        .fold(f32::MAX, f32::min);
    assert!(
        lowest > seat.high[1] - 0.001,
        "the cloth {lowest} m, the seat {} m",
        seat.high[1]
    );
    // Resting on it, the figure lowered onto it and its feet on the floor.
    assert!(
        lowest < seat.high[1] + 0.03,
        "the cloth {lowest} m over the seat"
    );
    assert!(seat.low[1].abs() < 1e-6, "the seat from {} m", seat.low[1]);
    // Standing again: no seat.
    pose(&mut drape, Pose::Standing, 8.0);
    assert!(drape.seat().is_none());
}

/// Drapes the pieces and their facings and linings, sewn by `seams`, until
/// settled; and where the cloth inside starts.
fn lined(
    model: &BodyModel,
    pieces: &[drapewright_pattern_core::Piece],
    seams: &[drapewright_seams::Seam],
) -> Drape {
    let body = model.shape(&Shape::default());
    let figure = Figure::of(model, &body);
    let draped = draped_pieces(pieces, seams);
    assert!(
        draped.alone.is_empty() && draped.inside.is_empty(),
        "{draped:?}"
    );
    let shell = assemble(&draped.sewn, seams, 20.0).unwrap();
    let starts = place(&shell, &figure);
    let (whole, starts) = drapewright_sim::inside::with_inside(
        &shell,
        &starts,
        (&draped.sewn, &draped.lined),
        seams,
        20.0,
    )
    .expect("placed inside");
    let mut drape = Drape::new(
        &whole,
        &starts,
        preset("poplin").unwrap(),
        (&body.vertices, &model.skin()),
    );
    let mut frames = 0;
    while !drape.settled() && frames < 60 * 10 {
        drape.step(1.0 / 60.0);
        frames += 1;
    }
    assert!(drape.settled(), "still moving after {frames} frames");
    drape
}

/// The share of the cloth inside that is further from the body than the
/// outside cloth nearest it, where the body is near: through it.
fn through(drape: &Drape) -> f32 {
    let x = &drape.cloth.x;
    let (outer, inner): (Vec<usize>, Vec<usize>) =
        (0..x.len()).partition(|&k| !drape.cloth.inside(k));
    let d = |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
    let near: Vec<usize> = inner
        .iter()
        .copied()
        .filter(|&i| drape.clearance(x[i]) < 0.02)
        .collect();
    let out = near
        .iter()
        .filter(|&&i| {
            let j = *outer
                .iter()
                .min_by(|&&a, &&b| d(x[i], x[a]).total_cmp(&d(x[i], x[b])))
                .expect("cloth outside");
            drape.clearance(x[i]) > drape.clearance(x[j]) + 0.002
        })
        .count();
    out as f32 / near.len().max(1) as f32
}

#[test]
fn a_lining_drapes_inside_the_skirt() {
    let model = avatar();
    let (pieces, seams) = common::lined_skirt();
    let drape = lined(&model, &pieces, &seams);
    let x = &drape.cloth.x;
    let inside = (0..x.len()).filter(|&k| drape.cloth.inside(k)).count();
    assert!(inside > 100, "{inside} points inside");
    // Hanging inside the skirt, shorter than it: none of it through it.
    let lowest = |inner: bool| {
        (0..x.len())
            .filter(|&k| drape.cloth.inside(k) == inner)
            .map(|k| x[k][1])
            .fold(f32::MAX, f32::min)
    };
    eprintln!(
        "skirt to {:.3} m, lining to {:.3} m; {:.4} of it through the skirt; widest stitch {:.1} mm",
        lowest(false),
        lowest(true),
        through(&drape),
        drape.cloth.widest_stitch() * 1000.0
    );
    assert!(lowest(true) > lowest(false) + 0.015);
    assert!(
        through(&drape) < 0.02,
        "{} through the skirt",
        through(&drape)
    );
    assert!(drape.cloth.widest_stitch() < 0.006);
}

#[test]
fn a_facing_drapes_inside_the_neck_it_faces() {
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
    let model = avatar();
    let drape = lined(&model, &pieces, &seams);
    eprintln!("{:.4} of the facing through the back", through(&drape));
    assert!(
        through(&drape) < 0.02,
        "{} through the back",
        through(&drape)
    );
    // The facing at the back of the neck, inside.
    let x = &drape.cloth.x;
    let facing: Vec<[f32; 3]> = (0..x.len())
        .filter(|&k| drape.cloth.inside(k))
        .map(|k| x[k])
        .collect();
    let back = facing.iter().map(|p| p[2]).sum::<f32>() / facing.len() as f32;
    let high = facing.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    assert!(back < 0.0, "the facing {back} m to the front");
    assert!(high > 1.3, "the facing down to {high} m");
}

#[test]
fn cuffs_stay_sewn_on_as_the_arms_reach_forward() {
    let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(false);
    let seams: Vec<drapewright_seams::Seam> = drapewright_seams::suggest(&pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| drapewright_seams::Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    let model = avatar();
    let mut drape = settled(&model, &pieces, &seams);
    let standing = drape.cloth.widest_stitch();
    pose(&mut drape, Pose::Reaching, 8.0);
    let reaching = drape.cloth.widest_stitch();
    eprintln!(
        "widest stitch standing {:.1} mm, reaching {:.1} mm; {:.4} inside",
        standing * 1000.0,
        reaching * 1000.0,
        inside(&drape)
    );
    // The sleeves rest on the wrists, held there by the body's friction,
    // while the cuffs are pulled on round the hands: the seams between
    // them hold all the same.
    assert!(standing < 0.005, "a seam {} mm open", standing * 1000.0);
    assert!(reaching < 0.005, "a seam {} mm open", reaching * 1000.0);
    assert!(inside(&drape) < 0.01);
}
