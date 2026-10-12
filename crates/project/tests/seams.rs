//! Seams in a project (schema version 3): sewing edges together, and what
//! happens to seams when pieces and edges change.

use drapewright_pattern_core::{Edge, Outline, Piece, Pt, Segment};
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

/// A project with a front and a back, and their ids and right-hand edges.
fn front_and_back() -> (Engine, [(String, String); 2]) {
    let mut e = Engine::new("prj", "Test", "2026-10-11T00:00:00Z", "test");
    e.dispatch(
        Command::AddPieces {
            pieces: vec![rectangle("Front"), rectangle("Back")],
        },
        Default::default(),
    )
    .unwrap();
    let doc = e.document();
    let right = |i: usize| {
        let p = &doc.pieces[i];
        (p.id.clone(), p.outline.edges[1].id.clone())
    };
    let ids = [right(0), right(1)];
    (e, ids)
}

fn seam(a: &(String, String), b: &(String, String)) -> Seam {
    Seam {
        id: String::new(),
        name: "Side seam".into(),
        a: SeamSide::whole(&a.0, &a.1),
        b: SeamSide::whole(&b.0, &b.1),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    }
}

#[test]
fn version_2_projects_open_with_no_seams() {
    for (version, file) in [
        (1, include_str!("fixtures/project-v1.json")),
        (2, include_str!("fixtures/project-v2.json")),
    ] {
        assert!(file.contains(&format!("\"schemaVersion\": {version}")));
        let (e, issues) = Engine::open(file, "test").unwrap();
        assert!(issues.is_empty(), "{issues:?}");
        assert!(e.document().seams.is_empty());
        if version == 2 {
            assert_eq!(e.document().measurement_sets[0].name, "Me");
        }
        let saved = e.to_json("now");
        assert!(saved.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
        assert!(saved.contains("\"seams\": []"));
    }
}

#[test]
fn edges_are_sewn_measured_and_unpicked() {
    let (mut e, [front, back]) = front_and_back();
    e.dispatch(
        Command::AddSeams {
            seams: vec![seam(&front, &back)],
        },
        Default::default(),
    )
    .unwrap();
    let sewn = e.document().seams[0].clone();
    assert!(sewn.id.starts_with('s'), "{}", sewn.id);
    let report = &e.seam_reports()[0];
    assert_eq!((report.a_length, report.b_length), (500.0, 500.0));

    // Turned round, renamed: still one seam.
    e.dispatch(
        Command::SetSeam {
            seam: Seam {
                reversed: true,
                name: "Left side".into(),
                ..sewn.clone()
            },
        },
        Default::default(),
    )
    .unwrap();
    assert!(e.document().seams[0].reversed);
    let (opened, _) = Engine::open(&e.to_json("now"), "test").unwrap();
    assert_eq!(opened.document().seams, e.document().seams);

    // An edge can't be sewn to itself, nor to an edge that isn't there.
    for bad in [
        seam(&front, &front),
        seam(&front, &(back.0.clone(), "nope".into())),
    ] {
        assert!(matches!(
            e.dispatch(Command::AddSeams { seams: vec![bad] }, Default::default()),
            Err(CommandError::Rejected { .. })
        ));
    }

    e.dispatch(
        Command::DeleteSeams {
            ids: vec![sewn.id.clone()],
        },
        Default::default(),
    )
    .unwrap();
    assert!(e.document().seams.is_empty());
    e.undo();
    assert_eq!(e.document().seams.len(), 1);
}

#[test]
fn sewn_edges_keep_their_corners_and_seams_go_with_their_pieces() {
    let (mut e, [front, back]) = front_and_back();
    e.dispatch(
        Command::AddSeams {
            seams: vec![seam(&front, &back)],
        },
        Default::default(),
    )
    .unwrap();
    let err = e
        .dispatch(
            Command::SplitEdge {
                piece: front.0.clone(),
                edge: front.1.clone(),
                at: 100.0,
            },
            Default::default(),
        )
        .unwrap_err();
    assert!(err.to_string().contains("Unpick"), "{err}");
    // Joining the edge before it would swallow the sewn edge too.
    let top = e.document().pieces[0].outline.edges[0].id.clone();
    assert!(
        e.dispatch(
            Command::JoinEdges {
                piece: front.0.clone(),
                edge: top,
            },
            Default::default(),
        )
        .is_err()
    );

    let set = e
        .dispatch(
            Command::DeletePieces {
                ids: vec![back.0.clone()],
            },
            Default::default(),
        )
        .unwrap();
    assert!(set.document);
    assert!(e.document().seams.is_empty());
    e.undo();
    assert_eq!(e.document().seams.len(), 1);
}

#[test]
fn suggestions_skip_what_is_sewn() {
    let (mut e, [front, back]) = front_and_back();
    assert!(!e.suggest_seams().is_empty());
    e.dispatch(
        Command::AddSeams {
            seams: vec![seam(&front, &back)],
        },
        Default::default(),
    )
    .unwrap();
    assert!(
        e.suggest_seams()
            .iter()
            .all(|s| ![&s.seam.a.edge, &s.seam.b.edge].contains(&&front.1)
                && ![&s.seam.a.edge, &s.seam.b.edge].contains(&&back.1))
    );
}

#[test]
fn panels_drawn_for_one_side_are_found_and_cut_as_pairs() {
    use drapewright_pattern_core::CutInstruction;
    use drapewright_pattern_core::testing::garments::corset;
    let mut e = Engine::new("prj", "Corset", "2026-10-12T00:00:00Z", "test");
    e.dispatch(
        Command::AddPieces {
            pieces: corset(false),
        },
        Default::default(),
    )
    .unwrap();
    let name = |e: &Engine, id: &str| e.document().piece(id).unwrap().name.clone();
    let found = e.drawn_for_one_side();
    let names: Vec<String> = found.iter().map(|id| name(&e, id)).collect();
    assert_eq!(names, ["Front Center", "Front Side", "Back Center"]);
    // Cut as pairs, they are found no more, and their centres are sewn.
    for id in &found {
        e.dispatch(
            Command::SetCut {
                piece: id.clone(),
                cut: vec![CutInstruction {
                    quantity: 2,
                    mirrored: Some(true),
                    ..Default::default()
                }],
            },
            Default::default(),
        )
        .unwrap();
    }
    assert!(e.drawn_for_one_side().is_empty());
    let centres = e
        .suggest_seams()
        .iter()
        .filter(|s| s.seam.a.piece == s.seam.b.piece && s.seam.a.mirror != s.seam.b.mirror)
        .count();
    assert_eq!(centres, 2);
}
