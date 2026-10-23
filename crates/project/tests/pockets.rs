//! Pockets sewn onto lines inside pieces (schema version 8): saved and read
//! back, sewn on by hand, unpicked, and projects from before.

use drapewright_pattern_core::Pt;
use drapewright_pattern_core::testing::garments::pocket_skirt;
use drapewright_project::{Command, Engine, SCHEMA_VERSION};
use drapewright_seams::Reason;

fn skirt() -> Engine {
    let mut e = Engine::new("prj", "Pocket skirt", "2026-10-23T00:00:00Z", "test");
    e.dispatch(
        Command::AddPieces {
            pieces: pocket_skirt(),
        },
        Default::default(),
    )
    .unwrap();
    e
}

/// The ids the project gave the front, its placement and the pocket.
fn ids(e: &Engine) -> (String, String, String) {
    let doc = e.document();
    let front = doc.pieces.iter().find(|p| p.name == "Skirt front").unwrap();
    let pocket = doc
        .pieces
        .iter()
        .find(|p| p.name == "Patch pocket")
        .unwrap();
    (
        front.id.clone(),
        front.lines[0].id.clone(),
        pocket.id.clone(),
    )
}

#[test]
fn a_pocket_is_sewn_on_by_hand_saved_and_read_back() {
    let mut e = skirt();
    let (front, line, pocket) = ids(&e);
    let seams = e.pocket_seams(&pocket, &front, &line).expect("it fits");
    assert_eq!(seams.len(), 3);
    // The pocket as drawn: on the front's both halves, its pair one each.
    assert!(seams.iter().all(|s| !s.a.mirror && !s.b.mirror));
    e.dispatch(Command::AddSeams { seams }, Default::default())
        .unwrap();
    let reports = e.seam_reports();
    assert!(
        reports
            .iter()
            .all(|r| r.problem.is_none() && r.difference.abs() < 0.5)
    );
    // Saved with its line, and read back the same.
    let json = e.to_json("2026-10-23T00:00:00Z");
    assert!(json.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
    assert!(json.contains(&format!("\"line\": \"{line}\"")));
    let (again, issues) = Engine::open(&json, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(again.document().seams, e.document().seams);
    // Not suggested again once sewn; and gone with the pocket.
    let again: Vec<_> = e
        .suggest_seams()
        .into_iter()
        .filter(|s| s.reasons.contains(&Reason::OnPlacement))
        .collect();
    assert!(
        again.is_empty(),
        "{again:#?} after {:#?}",
        e.document().seams
    );
    e.dispatch(
        Command::DeletePieces {
            ids: vec![pocket.clone()],
        },
        Default::default(),
    )
    .unwrap();
    assert!(e.document().seams.is_empty());
}

#[test]
fn find_seams_suggests_the_pocket_and_a_tap_finds_its_placement() {
    let mut e = skirt();
    let (front, line, pocket) = ids(&e);
    let suggested: Vec<_> = e
        .suggest_seams()
        .into_iter()
        .filter(|s| s.reasons.contains(&Reason::OnPlacement))
        .collect();
    assert_eq!(suggested.len(), 3);
    assert!(
        suggested
            .iter()
            .all(|s| s.seam.a.piece == pocket && s.seam.b.piece == front)
    );
    // A tap inside the front, near its placement, finds it; off the
    // piece, there is nothing to sew onto.
    let placement = e.document().piece(&front).unwrap().placement;
    let tap = Pt::new(placement.x + 120.0, placement.y + 200.0);
    assert_eq!(e.nearest_line(&front, tap), Some(line.clone()));
    assert_eq!(e.nearest_line("nothing", tap), None);
    // A seam onto a line that isn't there is refused.
    let mut wrong = suggested[0].seam.clone();
    wrong.b.line = Some("gone".into());
    assert!(
        e.dispatch(Command::AddSeams { seams: vec![wrong] }, Default::default())
            .is_err()
    );
}

#[test]
fn a_version_7_project_opens_with_its_seams_on_edges() {
    let v7 = include_str!("fixtures/project-v7.json");
    assert!(v7.contains("\"schemaVersion\": 7"));
    assert!(!v7.contains("\"line\""));
    let (e, issues) = Engine::open(v7, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let seams = &e.document().seams;
    assert_eq!(seams.len(), 2);
    assert!(seams.iter().all(|s| !s.a.is_line() && !s.b.is_line()));
    assert!(e.seam_reports().iter().all(|r| r.problem.is_none()));
    let saved = e.to_json("now");
    assert!(saved.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
}
