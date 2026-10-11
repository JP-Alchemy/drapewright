//! Alterations in a project (schema version 5): kept as steps on a piece,
//! with the piece as drawn, undone and redone like any change.

use drapewright_pattern_core::{
    AlterLine, Alteration, Change, Edge, EdgeKind, NotchFrom, NotchKind, Outline, Piece, Pt,
    Segment,
};
use drapewright_project::{Command, CommandError, Engine, SCHEMA_VERSION};
use drapewright_seams::{Seam, SeamSide};

fn rectangle(name: &str) -> Piece {
    Piece::new(
        "tmp",
        name,
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([200.0, 0.0])]),
                Edge::new("right", vec![Segment::line([200.0, 500.0])]),
                Edge::new("bottom", vec![Segment::line([0.0, 500.0])]),
                Edge::new("left", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    )
}

/// A project with a bodice-length rectangle; its id and its edges' ids.
fn project() -> (Engine, String, Vec<String>) {
    let mut e = Engine::new("prj", "Test", "2026-10-12T00:00:00Z", "test");
    e.dispatch(
        Command::AddPieces {
            pieces: vec![rectangle("Front")],
        },
        Default::default(),
    )
    .unwrap();
    let p = e.document().pieces[0].clone();
    let edges = p.outline.edges.iter().map(|e| e.id.clone()).collect();
    (e, p.id.clone(), edges)
}

/// Lengthened by `by` mm across the piece 250 mm down.
fn lengthen(by: f64) -> Alteration {
    Alteration {
        id: String::new(),
        on: true,
        change: Change::Lengthen {
            line: AlterLine::Through {
                from: Pt::new(-10.0, 250.0),
                to: Pt::new(210.0, 250.0),
            },
            by,
        },
    }
}

fn height(e: &Engine) -> f64 {
    e.document().pieces[0].outline.bounds().height()
}

#[test]
fn a_piece_is_lengthened_then_as_drawn_again() {
    let (mut e, piece, _) = project();
    e.dispatch(
        Command::AddAlteration {
            piece: piece.clone(),
            alteration: lengthen(30.0),
        },
        Default::default(),
    )
    .unwrap();
    assert!((height(&e) - 530.0).abs() < 1e-9);
    let p = e.document().pieces[0].clone();
    let step = p.alterations[0].clone();
    assert!(!step.id.is_empty());
    assert!((p.drawn.as_ref().unwrap().outline.bounds().height() - 500.0).abs() < 1e-9);

    // Switched off: as drawn, the step kept.
    e.dispatch(
        Command::SetAlteration {
            piece: piece.clone(),
            alteration: Alteration {
                on: false,
                ..step.clone()
            },
        },
        Default::default(),
    )
    .unwrap();
    assert!((height(&e) - 500.0).abs() < 1e-9);
    assert_eq!(e.document().pieces[0].alterations.len(), 1);
    // Shortened instead, and on.
    e.dispatch(
        Command::SetAlteration {
            piece: piece.clone(),
            alteration: Alteration {
                id: step.id.clone(),
                ..lengthen(-20.0)
            },
        },
        Default::default(),
    )
    .unwrap();
    assert!((height(&e) - 480.0).abs() < 1e-9);

    // Taken away: as drawn, and no longer kept apart.
    e.dispatch(
        Command::DeleteAlteration {
            piece: piece.clone(),
            alteration: step.id.clone(),
        },
        Default::default(),
    )
    .unwrap();
    let p = &e.document().pieces[0];
    assert!((height(&e) - 500.0).abs() < 1e-9);
    assert!(p.alterations.is_empty() && p.drawn.is_none());

    // Undone step by step, back to lengthened; redone.
    e.undo();
    assert!((height(&e) - 480.0).abs() < 1e-9);
    e.undo();
    e.undo();
    assert!((height(&e) - 530.0).abs() < 1e-9);
    e.redo();
    assert!((height(&e) - 500.0).abs() < 1e-9);
}

#[test]
fn an_altered_piece_changes_as_drawn_and_is_altered_again() {
    let (mut e, piece, edges) = project();
    e.dispatch(
        Command::AddAlteration {
            piece: piece.clone(),
            alteration: lengthen(30.0),
        },
        Default::default(),
    )
    .unwrap();
    // Which edge is a hem: the same edge as drawn and altered.
    e.dispatch(
        Command::SetEdgeKind {
            piece: piece.clone(),
            edge: edges[2].clone(),
            kind: EdgeKind::Hem,
        },
        Default::default(),
    )
    .unwrap();
    let p = &e.document().pieces[0];
    assert_eq!(p.outline.edges[2].kind, EdgeKind::Hem);
    assert_eq!(
        p.drawn.as_ref().unwrap().outline.edges[2].kind,
        EdgeKind::Hem
    );
    assert!((height(&e) - 530.0).abs() < 1e-9);

    // Where a notch goes is said of the piece as shown: not while altered.
    let notch = Command::AddNotch {
        piece: piece.clone(),
        edge: edges[3].clone(),
        at: 100.0,
        from: NotchFrom::Start,
        kind: NotchKind::default(),
    };
    let refused = e.dispatch(notch.clone(), Default::default());
    assert!(
        matches!(&refused, Err(CommandError::Rejected { message }) if message.contains("altered")),
        "{refused:?}"
    );
    // Switched off it is as drawn, and the notch goes on as drawn too.
    let step = e.document().pieces[0].alterations[0].clone();
    e.dispatch(
        Command::SetAlteration {
            piece: piece.clone(),
            alteration: Alteration { on: false, ..step },
        },
        Default::default(),
    )
    .unwrap();
    e.dispatch(notch, Default::default()).unwrap();
    let p = &e.document().pieces[0];
    assert_eq!(p.notches.len(), 1);
    assert_eq!(p.drawn.as_ref().unwrap().notches.len(), 1);
}

#[test]
fn an_alteration_that_cannot_be_made_is_refused() {
    let (mut e, piece, _) = project();
    let missing = Alteration {
        change: Change::Lengthen {
            line: AlterLine::Through {
                from: Pt::new(-10.0, 900.0),
                to: Pt::new(210.0, 900.0),
            },
            by: 30.0,
        },
        ..lengthen(30.0)
    };
    for alteration in [
        missing,
        lengthen(f64::NAN),
        lengthen(5000.0),
        lengthen(-400.0),
    ] {
        let result = e.dispatch(
            Command::AddAlteration {
                piece: piece.clone(),
                alteration,
            },
            Default::default(),
        );
        assert!(
            matches!(result, Err(CommandError::Rejected { .. })),
            "{result:?}"
        );
    }
    let p = &e.document().pieces[0];
    assert!(p.alterations.is_empty() && p.drawn.is_none());
}

#[test]
fn alterations_are_saved_and_opened_again() {
    let (mut e, piece, _) = project();
    e.dispatch(
        Command::AddAlteration {
            piece,
            alteration: lengthen(30.0),
        },
        Default::default(),
    )
    .unwrap();
    let json = e.to_json("2026-10-12T10:00:00Z");
    assert!(json.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
    let (again, issues) = Engine::open(&json, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(again.document().pieces, e.document().pieces);
}

#[test]
fn a_version_4_project_opens_with_no_alterations() {
    let v4 = include_str!("fixtures/project-v4.json");
    assert!(v4.contains("\"schemaVersion\": 4"));
    let (e, issues) = Engine::open(v4, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(e.document().pieces.len(), 2);
    for p in &e.document().pieces {
        assert!(p.alterations.is_empty() && p.drawn.is_none());
    }
}

#[test]
fn seams_are_measured_on_the_altered_piece() {
    let (mut e, piece, edges) = project();
    e.dispatch(
        Command::AddPieces {
            pieces: vec![rectangle("Back")],
        },
        Default::default(),
    )
    .unwrap();
    let back = e.document().pieces[1].clone();
    e.dispatch(
        Command::AddSeams {
            seams: vec![Seam {
                id: String::new(),
                name: "Side".into(),
                a: SeamSide::whole(&piece, &edges[1]),
                b: SeamSide::whole(&back.id, &back.outline.edges[1].id),
                reversed: false,
                ease: Default::default(),
                confirmed: true,
            }],
        },
        Default::default(),
    )
    .unwrap();
    assert!(e.seam_reports()[0].difference.abs() < 1e-6);
    // The front lengthened, its side seam 30 mm longer than the back's.
    e.dispatch(
        Command::AddAlteration {
            piece,
            alteration: lengthen(30.0),
        },
        Default::default(),
    )
    .unwrap();
    let report = &e.seam_reports()[0];
    assert!((report.difference + 30.0).abs() < 1e-6, "{report:?}");
}

#[test]
fn slashed_and_spread_or_closed_by_command() {
    let (mut e, piece, _) = project();
    let spread = |by: f64| Alteration {
        id: String::new(),
        on: true,
        change: Change::Spread {
            from: Pt::new(100.0, 500.0),
            hinge: Pt::new(100.0, 0.0),
            by,
            turns: drapewright_pattern_core::alter::Turns::Right,
        },
    };
    let width = |e: &Engine| e.document().pieces[0].outline.bounds().width();
    e.dispatch(
        Command::AddAlteration {
            piece: piece.clone(),
            alteration: spread(30.0),
        },
        Default::default(),
    )
    .unwrap();
    assert!(width(&e) > 225.0, "{}", width(&e));
    let step = e.document().pieces[0].alterations[0].clone();
    e.dispatch(
        Command::SetAlteration {
            piece,
            alteration: Alteration {
                id: step.id,
                ..spread(f64::INFINITY)
            },
        },
        Default::default(),
    )
    .expect_err("no endless spread");
    assert!(width(&e) > 225.0);
}
