//! Patch pockets sewn onto a skirt's front where their placement is
//! drawn: the front meshed with the placement in it, the pockets stitched
//! to it point for point, laid on it, and draped over it.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::testing::garments::pocket_skirt;
use drapewright_pattern_core::{CutInstruction, Piece, Segment};
use drapewright_seams::Seam;
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::{Copy, Garment, draped_pieces, shell};
use drapewright_sim::inside::{Dressed, dressed};
use drapewright_sim::place::{Figure, place};

fn avatar() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).unwrap()).unwrap()
}

/// The pieces sewn by the seams Find seams suggests for them.
fn found(pieces: &[Piece]) -> Vec<Seam> {
    drapewright_seams::suggest(pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect()
}

/// The garment made up, placed on the standard figure: its shell, and all
/// of it.
fn made_up(
    model: &BodyModel,
    pieces: &[Piece],
    seams: &[Seam],
    spacing: f64,
) -> (Garment, Dressed) {
    let body = model.shape(&Shape::default());
    let figure = Figure::of(model, &body);
    let draped = draped_pieces(pieces, seams);
    let shell = shell(&draped, seams, spacing).unwrap();
    let starts = place(&shell, &figure);
    let made = dressed(&shell, &starts, &draped, seams, spacing);
    (shell, made)
}

#[test]
fn the_pockets_are_sewn_onto_the_front_point_for_point() {
    let model = avatar();
    let pieces = pocket_skirt();
    let seams = found(&pieces);
    let draped = draped_pieces(&pieces, &seams);
    assert_eq!(
        draped.on.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        ["pocket"]
    );
    assert!(
        draped.alone.is_empty() && draped.off.is_empty(),
        "{draped:?}"
    );
    let (shell, made) = made_up(&model, &pieces, &seams, 20.0);
    assert!(made.unplaced.is_empty() && made.lined);
    let g = &made.garment;
    // The front unfolded, the back's pair, then the pocket's pair over the
    // front.
    let panels: Vec<(&str, Copy, bool)> = g
        .panels
        .iter()
        .map(|p| (p.piece.as_str(), p.copy, p.sewn_on))
        .collect();
    assert_eq!(
        panels,
        [
            ("front", Copy::Unfolded, false),
            ("back", Copy::Original, false),
            ("back", Copy::Mirror, false),
            ("pocket", Copy::Original, true),
            ("pocket", Copy::Mirror, true),
        ]
    );
    assert_eq!(shell.panels.len(), 3);
    assert!(g.panels[3..].iter().all(|p| p.over == Some(0)));
    // The placement in the front's mesh on each half, the mirror of each
    // other: the front meshed as one half mirrored.
    let front = &g.panels[0].mesh;
    assert_eq!(front.inner.len(), 2, "{:?}", front.inner);
    let at = |i: u32| front.points[i as usize];
    for (a, b) in front.inner[0].iter().zip(&front.inner[1]) {
        let (p, q) = (at(*a), at(*b));
        assert!(
            (p[0] + q[0]).abs() < 1e-6 && (p[1] - q[1]).abs() < 1e-6,
            "{p:?} {q:?}"
        );
    }
    let left = front.points.iter().filter(|p| p[0] < -1e-9).count();
    let right = front.points.iter().filter(|p| p[0] > 1e-9).count();
    assert_eq!(left, right);
    // Each pocket stitched along its sides and bottom to its own side's
    // placement, its stitches' ends at the same place flat.
    for pocket in [3, 4] {
        let stitched: Vec<([f64; 2], [f64; 2])> = g
            .stitches
            .iter()
            .filter_map(|&[(pa, a), (pb, b)]| {
                let (p, q) = if pa == pocket {
                    ((pa, a), (pb, b))
                } else {
                    ((pb, b), (pa, a))
                };
                (p.0 == pocket).then(|| {
                    assert_eq!(q.0, 0, "a pocket sewn to the front only");
                    (
                        g.panels[p.0].mesh.points[p.1 as usize],
                        g.panels[0].mesh.points[q.1 as usize],
                    )
                })
            })
            .collect();
        // About 13 + 2 × 16 cm sewn, a point every 2 cm.
        assert!((20..=28).contains(&stitched.len()), "{}", stitched.len());
        // The pocket's points as far apart as the front's they are sewn to.
        for w in stitched.windows(2) {
            let d = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).hypot(a[1] - b[1]);
            let (on_pocket, on_front) = (d(w[0].0, w[1].0), d(w[0].1, w[1].1));
            assert!((on_pocket - on_front).abs() < 1.0, "{on_pocket} {on_front}");
        }
    }
}

/// The share of the pocket's points not in front of the front: no further
/// from the body than the front's nearest point, where they lie on it.
fn behind(drape: &Drape) -> f32 {
    let x = &drape.cloth.x;
    let d = |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
    let on: Vec<usize> = (0..x.len()).filter(|&k| drape.cloth.sewn_on(k)).collect();
    let under: Vec<usize> = (0..drape.firsts[1] as usize).collect();
    let near: Vec<usize> = on
        .iter()
        .copied()
        .filter(|&i| under.iter().any(|&j| d(x[i], x[j]) < 0.01))
        .collect();
    let wrong = near
        .iter()
        .filter(|&&i| {
            let j = *under
                .iter()
                .min_by(|&&a, &&b| d(x[i], x[a]).total_cmp(&d(x[i], x[b])))
                .expect("the front");
            drape.clearance(x[i]) < drape.clearance(x[j]) - 0.0005
        })
        .count();
    wrong as f32 / near.len().max(1) as f32
}

#[test]
fn the_pockets_drape_over_the_front() {
    let model = avatar();
    let body = model.shape(&Shape::default());
    let pieces = pocket_skirt();
    let seams = found(&pieces);
    let (_, made) = made_up(&model, &pieces, &seams, 20.0);
    let mut drape = Drape::new(
        &made.garment,
        &made.starts,
        preset("poplin").unwrap(),
        (&body.vertices, &model.skin()),
    );
    let pocket = (0..drape.cloth.x.len())
        .filter(|&k| drape.cloth.sewn_on(k))
        .count();
    assert!(pocket > 100, "{pocket} points sewn on");
    assert!((0..drape.cloth.x.len()).all(|k| !drape.cloth.inside(k)));
    let mut frames = 0;
    while !drape.settled() && frames < 60 * 10 {
        drape.step(1.0 / 60.0);
        frames += 1;
    }
    eprintln!(
        "settled after {frames} frames; widest stitch {:.1} mm; {:.4} of the pockets behind the front",
        drape.cloth.widest_stitch() * 1000.0,
        behind(&drape)
    );
    assert!(drape.settled(), "still moving after {frames} frames");
    assert!(drape.cloth.widest_stitch() < 0.005);
    assert!(behind(&drape) < 0.02, "{} behind the front", behind(&drape));
    // On the front, either side of the centre, at the hip: the pockets'
    // middles to the front of the body and apart.
    let x = &drape.cloth.x;
    let first = drape.firsts[3] as usize;
    let half = drape.firsts[4] as usize;
    let middle = |r: std::ops::Range<usize>| {
        let n = r.len() as f32;
        r.fold([0.0f32; 3], |m, k| {
            [m[0] + x[k][0] / n, m[1] + x[k][1] / n, m[2] + x[k][2] / n]
        })
    };
    let (a, b) = (middle(first..half), middle(half..x.len()));
    eprintln!("pockets at {a:?} and {b:?}");
    assert!(a[2] > 0.05 && b[2] > 0.05, "{a:?} {b:?}");
    assert!(
        (a[0] - b[0]).abs() > 0.1 && a[0] * b[0] < 0.0,
        "{a:?} {b:?}"
    );
}

#[test]
fn the_pockets_settle_from_nudged_starts() {
    let model = avatar();
    let body = model.shape(&Shape::default());
    let pieces = pocket_skirt();
    let seams = found(&pieces);
    let (_, made) = made_up(&model, &pieces, &seams, 20.0);
    for seed in 1..=3u64 {
        let mut drape = Drape::new(
            &made.garment,
            &made.starts,
            preset("poplin").unwrap(),
            (&body.vertices, &model.skin()),
        );
        let mut h = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
        for p in &mut drape.cloth.x {
            for c in p.iter_mut() {
                h ^= h << 13;
                h ^= h >> 7;
                h ^= h << 17;
                *c += ((h % 2001) as f32 - 1000.0) * 1e-6;
            }
        }
        let mut frames = 0;
        while !drape.settled() && frames < 60 * 10 {
            drape.step(1.0 / 60.0);
            frames += 1;
        }
        assert!(
            drape.settled(),
            "seed {seed}: still moving after {frames} frames"
        );
        assert!(
            behind(&drape) < 0.02,
            "seed {seed}: {} behind",
            behind(&drape)
        );
    }
}

#[test]
fn a_pocket_whose_placement_runs_over_the_edge_is_left_off() {
    let model = avatar();
    let mut pieces = pocket_skirt();
    let seams = found(&pieces);
    // The front's side seam moved in over the placement, after sewing.
    let side = &mut pieces[0].outline.edges[1];
    side.segments = vec![Segment::cubic(
        [180.0, 80.0],
        [170.0, 200.0],
        [290.0, 600.0],
    )];
    let (_, made) = made_up(&model, &pieces, &seams, 20.0);
    assert_eq!(made.unplaced, ["pocket"]);
    assert!(made.garment.panels.iter().all(|p| p.piece != "pocket"));
    assert_eq!(made.garment.panels.len(), made.starts.len());
}

#[test]
fn a_pocket_sewn_onto_a_lining_is_left_off() {
    let mut pieces = pocket_skirt();
    let seams = found(&pieces);
    // The front cut from lining instead.
    pieces[0].cut = vec![CutInstruction {
        material: "lining".into(),
        ..Default::default()
    }];
    let draped = draped_pieces(&pieces, &seams);
    assert_eq!(draped.off, ["pocket"]);
    assert!(draped.on.is_empty());
}
