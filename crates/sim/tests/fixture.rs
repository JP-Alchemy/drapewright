//! The garments the drape tests use, as project files for the app's tests
//! and for trying in the app (`fixtures/drapewright/`), written from them
//! so the two cannot drift. CI fails if a written file differs from the
//! committed one.

mod common;

use std::path::Path;

use drapewright_pattern_core::Piece;
use drapewright_project::{Command, Engine, container};
use drapewright_seams::{Seam, SeamSide};

/// When the projects were made: fixed, so the files are the same every time.
const MADE: &str = "2026-10-11T00:00:00Z";

/// `pieces`, side by side on the table at `across` (mm), sewn by `seams`,
/// as project `id`. Also gives each seam's difference in length between
/// its sides (ease), mm.
fn project(
    (id, name): (&str, &str),
    mut pieces: Vec<Piece>,
    seams: Vec<Seam>,
    across: &[f64],
) -> (String, Vec<f64>) {
    for (piece, &x) in pieces.iter_mut().zip(across) {
        piece.placement.x = x;
    }
    let mut engine = Engine::new(id, name, MADE, "drapewright tests");
    engine
        .dispatch(
            Command::AddPieces {
                pieces: pieces.clone(),
            },
            Default::default(),
        )
        .unwrap();
    // The pieces and their edges have new ids in the project, in the same
    // order.
    let doc = engine.document();
    let renamed = |side: &SeamSide| {
        let k = pieces.iter().position(|p| p.id == side.piece).unwrap();
        let e = pieces[k].outline.edge_index(&side.edge).unwrap();
        let piece = &doc.pieces[k];
        SeamSide {
            piece: piece.id.clone(),
            edge: piece.outline.edges[e].id.clone(),
            ..side.clone()
        }
    };
    let count = seams.len();
    let seams: Vec<Seam> = seams
        .iter()
        .map(|s| Seam {
            a: renamed(&s.a),
            b: renamed(&s.b),
            ..s.clone()
        })
        .collect();
    engine
        .dispatch(Command::AddSeams { seams }, Default::default())
        .unwrap();
    assert_eq!(engine.document().seams.len(), count);
    let reports = engine.seam_reports();
    for report in &reports {
        assert!(report.problem.is_none(), "{report:?}");
    }
    (
        engine.to_json(MADE),
        reports.iter().map(|r| r.difference).collect(),
    )
}

/// Writes `json` as `fixtures/drapewright/{file}` unless it is there already.
fn write(file: &str, json: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/drapewright")
        .join(file);
    // Compared as project.json: the archive's compression may change.
    let written = std::fs::read(&path)
        .ok()
        .and_then(|bytes| container::unpack(&bytes).ok())
        .map(|u| u.project_json);
    if written.as_deref() != Some(json) {
        std::fs::write(&path, container::pack(json, &[]).unwrap()).unwrap();
    }
}

#[test]
fn the_skirt_project_is_up_to_date() {
    let (pieces, seams) = common::skirt();
    let (json, differences) = project(("skirt", "Test skirt"), pieces, seams, &[0.0, 400.0]);
    // Sewn as the drape tests sew it, the sides the same length.
    assert!(differences.iter().all(|d| d.abs() < 1.0), "{differences:?}");
    write("skirt.drapewright", &json);
}

#[test]
fn the_blouse_project_is_up_to_date() {
    let (pieces, seams) = common::blouse();
    let (json, differences) = project(
        ("blouse", "Test blouse"),
        pieces,
        seams,
        &[0.0, 350.0, 850.0],
    );
    // The sleeve's cap a little longer than the armhole, eased in.
    assert!(
        differences.iter().all(|d| d.abs() < 30.0),
        "{differences:?}"
    );
    write("blouse.drapewright", &json);
}

#[test]
fn the_corset_project_is_up_to_date() {
    // As a pattern drawn for one side leaves it: each panel cut once, and
    // nothing sewn yet.
    let pieces = drapewright_pattern_core::testing::garments::corset(false);
    let (json, _) = project(
        ("corset", "Test corset"),
        pieces,
        Vec::new(),
        &[0.0, 0.0, 0.0, 0.0],
    );
    write("corset.drapewright", &json);
}

#[test]
fn the_lined_skirt_project_is_up_to_date() {
    let (pieces, seams) = common::lined_skirt();
    let (json, differences) = project(
        ("lined-skirt", "Test lined skirt"),
        pieces,
        seams,
        &[0.0, 400.0, 800.0, 1200.0],
    );
    assert!(differences.iter().all(|d| d.abs() < 1.0), "{differences:?}");
    write("lined-skirt.drapewright", &json);
}

#[test]
fn the_shirt_project_is_up_to_date() {
    // The yoked blouse upright, sewn by the seams found for it: buttoned
    // down the front, a facing at the back of its neck.
    let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(false);
    let seams: Vec<Seam> = drapewright_seams::suggest(&pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    let (json, _) = project(
        ("shirt", "Test shirt"),
        pieces,
        seams,
        &[0.0, 650.0, 1100.0, 1500.0, 1900.0, 2500.0, 2800.0],
    );
    write("shirt.drapewright", &json);
}
