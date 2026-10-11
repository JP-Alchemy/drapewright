//! The skirt the drape tests use, as a project file for the app's tests
//! (`fixtures/drapewright/skirt.drapewright`), written from it so the two
//! cannot drift. CI fails if the written file differs from the committed
//! one.

mod common;

use std::path::Path;

use drapewright_project::{Command, Engine, container};
use drapewright_seams::{Seam, SeamSide};

/// When the project was made: fixed, so the file is the same every time.
const MADE: &str = "2026-10-11T00:00:00Z";

#[test]
fn the_skirt_project_is_up_to_date() {
    let (mut pieces, seams) = common::skirt();
    // Side by side on the table.
    pieces[1].placement.x = 400.0;
    let mut engine = Engine::new("skirt", "Test skirt", MADE, "drapewright tests");
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
    // Sewn as the drape tests sew it, the sides the same length.
    assert_eq!(engine.document().seams.len(), 2);
    for report in engine.seam_reports() {
        assert!(report.problem.is_none(), "{report:?}");
        assert!(report.difference.abs() < 1.0, "{report:?}");
    }
    let json = engine.to_json(MADE);
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/drapewright/skirt.drapewright");
    // Compared as project.json: the archive's compression may change.
    let written = std::fs::read(&path)
        .ok()
        .and_then(|bytes| container::unpack(&bytes).ok())
        .map(|u| u.project_json);
    if written.as_deref() != Some(json.as_str()) {
        std::fs::write(&path, container::pack(&json, &[]).unwrap()).unwrap();
    }
}
