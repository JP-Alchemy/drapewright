//! A garment altered to fit at its side seams.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::{Alteration, Change, Drawn, Piece, alter};
use drapewright_project::{Command, Engine, container};
use drapewright_seams::{Seam, drawn_for_one_side, suggest};
use drapewright_wasm_3d::refit::{FitRequest, LevelChange, fit_steps};
use drapewright_wasm_3d::sizes::{SizePieces, SizesRequest, measure_sizes};

fn model() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).expect("the body file")).expect("a body file")
}

/// A test garment's project (`fixtures/drapewright/{name}.drapewright`).
fn project(name: &str) -> Engine {
    let path = format!(
        "{}/../../fixtures/drapewright/{name}.drapewright",
        env!("CARGO_MANIFEST_DIR")
    );
    let unpacked = container::unpack(&std::fs::read(path).unwrap()).unwrap();
    Engine::open(&unpacked.project_json, "test").unwrap().0
}

fn pieces(engine: &Engine) -> Vec<Piece> {
    engine
        .document()
        .pieces
        .iter()
        .map(|p| (**p).clone())
        .collect()
}

/// The garment round each level, mm.
fn girths(model: &BodyModel, pieces: Vec<Piece>) -> Vec<(String, Option<f64>)> {
    measure_sizes(
        model,
        &SizesRequest {
            sizes: vec![SizePieces {
                size: "-".into(),
                pieces,
            }],
            settings: Shape::default().into(),
        },
    )[0]
    .girths
    .iter()
    .map(|g| (g.level.clone(), g.garment))
    .collect()
}

fn at(girths: &[(String, Option<f64>)], level: &str) -> f64 {
    girths
        .iter()
        .find(|g| g.0 == level)
        .and_then(|g| g.1)
        .unwrap_or_else(|| panic!("{level}: {girths:?}"))
}

/// The engine with `change` asked for, its steps added: the steps.
fn refit(model: &BodyModel, engine: &mut Engine, change: &[(&str, f64)]) -> Vec<Change> {
    let request = FitRequest {
        pieces: pieces(engine),
        seams: engine.document().seams.clone(),
        settings: Shape::default().into(),
        change: change
            .iter()
            .map(|(level, by)| LevelChange {
                level: (*level).into(),
                by: *by,
            })
            .collect(),
    };
    let fit = fit_steps(model, &request);
    assert_eq!(fit.missed, Vec::<String>::new());
    engine
        .dispatch_all(
            fit.steps
                .iter()
                .map(|s| Command::AddAlteration {
                    piece: s.piece.clone(),
                    alteration: s.alteration.clone(),
                })
                .collect(),
        )
        .unwrap();
    fit.steps.into_iter().map(|s| s.alteration.change).collect()
}

#[test]
fn a_skirt_is_let_out_at_the_waist_its_hip_as_it_was() {
    let model = model();
    let mut skirt = project("skirt");
    let before = girths(&model, pieces(&skirt));
    let steps = refit(&model, &mut skirt, &[("waist", 40.0), ("hip", 0.0)]);
    // At the side seams: each edge of a front and a back cut on the fold,
    // let out a quarter of it.
    assert!(!steps.is_empty());
    for step in &steps {
        let Change::Waist { by, .. } = step else {
            panic!("{step:?}")
        };
        assert!((by - 10.0).abs() < 1e-9, "{steps:?}");
    }
    let after = girths(&model, pieces(&skirt));
    let grown = at(&after, "waist") - at(&before, "waist");
    assert!(
        (grown - 40.0).abs() < 3.0,
        "waist {before:?} then {after:?}"
    );
    assert!(
        (at(&after, "hip") - at(&before, "hip")).abs() < 3.0,
        "hip {before:?} then {after:?}"
    );
}

#[test]
fn a_corset_is_let_out_at_the_bust_and_taken_in_at_the_waist() {
    // Its panels have no front and back side seams: each seam takes a
    // share. Cut as pairs and sewn as Find seams would.
    let model = model();
    let mut corset = pieces(&project("corset"));
    for id in drawn_for_one_side(&corset, &[]) {
        let piece = corset.iter_mut().find(|p| p.id == id).unwrap();
        for cut in &mut piece.cut {
            cut.quantity = 2;
            cut.mirrored = Some(true);
        }
    }
    let seams: Vec<Seam> = suggest(&corset, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    let before = girths(&model, corset.clone());
    let fit = fit_steps(
        &model,
        &FitRequest {
            pieces: corset.clone(),
            seams,
            settings: Shape::default().into(),
            change: [("bust", 30.0), ("waist", -30.0), ("hip", 0.0)]
                .iter()
                .map(|(level, by)| LevelChange {
                    level: (*level).into(),
                    by: *by,
                })
                .collect(),
        },
    );
    assert!(
        fit.steps
            .iter()
            .any(|s| matches!(s.alteration.change, Change::BustSide { .. })),
        "{fit:?}"
    );
    // Each piece's steps, applied after its own.
    for piece in &mut corset {
        let steps: Vec<Alteration> = fit
            .steps
            .iter()
            .filter(|s| s.piece == piece.id)
            .map(|s| s.alteration.clone())
            .collect();
        alter(&Drawn::of(piece), &steps).unwrap().put(piece);
    }
    let after = girths(&model, corset);
    for (level, by) in [("bust", 30.0), ("waist", -30.0)] {
        let changed = at(&after, level) - at(&before, level);
        assert!(
            (changed - by).abs() < 4.0,
            "{level}: {before:?} then {after:?}"
        );
    }
}
