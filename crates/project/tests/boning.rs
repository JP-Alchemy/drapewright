//! Corsets in a project (schema version 9): bones sewn along edges and
//! lines marked as boning channels, and seams laced instead of sewn.

use drapewright_pattern_core::stitching::{BONE_INSET, BONE_SHORT, line_points};
use drapewright_pattern_core::{Edge, InternalLine, LineKind, Outline, Piece, Pt, Segment};
use drapewright_project::{Command, Engine, SCHEMA_VERSION};
use drapewright_seams::{Seam, SeamSide};

/// A panel 100 mm wide and 400 tall, a line across it.
fn panel(name: &str) -> Piece {
    let mut p = Piece::new(
        "tmp",
        name,
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([100.0, 0.0])]),
                Edge::new("right", vec![Segment::line([100.0, 400.0])]),
                Edge::new("bottom", vec![Segment::line([0.0, 400.0])]),
                Edge::new("left", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    p.lines.push(InternalLine {
        id: "across".into(),
        kind: LineKind::Line,
        start: Pt::new(10.0, 200.0),
        segments: vec![Segment::line([90.0, 200.0])],
        label: None,
    });
    p
}

fn engine() -> (Engine, Vec<(String, String)>) {
    let mut e = Engine::new("prj", "Test", "2026-11-03T00:00:00Z", "test");
    e.dispatch(
        Command::AddPieces {
            pieces: vec![panel("Front"), panel("Back")],
        },
        Default::default(),
    )
    .unwrap();
    let rights = e
        .document()
        .pieces
        .iter()
        .map(|p| (p.id.clone(), p.outline.edges[1].id.clone()))
        .collect();
    (e, rights)
}

#[test]
fn a_bone_is_sewn_along_an_edge_and_taken_out() {
    let (mut e, rights) = engine();
    let (piece, edge) = rights[0].clone();
    e.dispatch(
        Command::BoneEdge {
            piece: piece.clone(),
            edge: edge.clone(),
            boned: true,
        },
        Default::default(),
    )
    .unwrap();
    let p = e.document().piece(&piece).unwrap().clone();
    let bone = p.lines.iter().find(|l| l.kind == LineKind::Boning).unwrap();
    assert_eq!(bone.id, format!("bone-{edge}"));
    // Beside the right edge, inside the panel, short of its ends.
    let pts = line_points(bone);
    for q in &pts {
        assert!((q.x - (100.0 - BONE_INSET)).abs() < 1e-6, "{q:?}");
    }
    let ys: Vec<f64> = pts.iter().map(|q| q.y).collect();
    let (low, high) = ys
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), &y| (l.min(y), h.max(y)));
    assert!((low - BONE_SHORT).abs() < 1e-6 && (high - (400.0 - BONE_SHORT)).abs() < 1e-6);
    // Again: still the one bone.
    e.dispatch(
        Command::BoneEdge {
            piece: piece.clone(),
            edge: edge.clone(),
            boned: true,
        },
        Default::default(),
    )
    .unwrap();
    let count = |e: &Engine| {
        e.document()
            .piece(&piece)
            .unwrap()
            .lines
            .iter()
            .filter(|l| l.kind == LineKind::Boning)
            .count()
    };
    assert_eq!(count(&e), 1);
    e.dispatch(
        Command::BoneEdge {
            piece: piece.clone(),
            edge,
            boned: false,
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(count(&e), 0);
    // Undone, it is back.
    e.undo();
    assert_eq!(count(&e), 1);
}

#[test]
fn a_line_is_marked_as_a_boning_channel() {
    let (mut e, rights) = engine();
    let piece = rights[0].0.clone();
    // Added, the line has an id of its own.
    let line = e.document().piece(&piece).unwrap().lines[0].id.clone();
    e.dispatch(
        Command::SetLineKind {
            piece: piece.clone(),
            line,
            kind: LineKind::Boning,
        },
        Default::default(),
    )
    .unwrap();
    let p = e.document().piece(&piece).unwrap();
    assert_eq!(p.lines[0].kind, LineKind::Boning);
    let gone = e.dispatch(
        Command::SetLineKind {
            piece,
            line: "nothing".into(),
            kind: LineKind::Boning,
        },
        Default::default(),
    );
    assert!(gone.is_err());
}

#[test]
fn a_seam_is_laced_open_by_up_to_30_cm() {
    let (mut e, rights) = engine();
    let laced = |gap: f64| Seam {
        id: String::new(),
        name: "Centre back".into(),
        a: SeamSide::whole(&rights[0].0, &rights[0].1),
        b: SeamSide::whole(&rights[1].0, &rights[1].1),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
        lacing: Some(gap),
    };
    e.dispatch(
        Command::AddSeams {
            seams: vec![laced(50.0)],
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(e.document().seams[0].lacing, Some(50.0));
    for gap in [-1.0, 301.0, f64::NAN] {
        let refused = e.dispatch(
            Command::AddSeams {
                seams: vec![laced(gap)],
            },
            Default::default(),
        );
        assert!(refused.is_err(), "{gap}");
    }
}

#[test]
fn a_laced_seam_and_a_boning_line_are_saved_and_opened() {
    let (mut e, rights) = engine();
    e.dispatch(
        Command::BoneEdge {
            piece: rights[0].0.clone(),
            edge: rights[0].1.clone(),
            boned: true,
        },
        Default::default(),
    )
    .unwrap();
    e.dispatch(
        Command::AddSeams {
            seams: vec![Seam {
                id: String::new(),
                name: String::new(),
                a: SeamSide::whole(&rights[0].0, &rights[0].1),
                b: SeamSide::whole(&rights[1].0, &rights[1].1),
                reversed: false,
                ease: Default::default(),
                confirmed: true,
                lacing: Some(60.0),
            }],
        },
        Default::default(),
    )
    .unwrap();
    let json = e.to_json("2026-11-03T00:00:00Z");
    assert!(json.contains("\"boning\"") && json.contains("\"lacing\": 60.0"));
    let (opened, _) = Engine::open(&json, "test").unwrap();
    assert_eq!(opened.document().seams[0].lacing, Some(60.0));
    assert!(
        opened.document().pieces[0]
            .lines
            .iter()
            .any(|l| l.kind == LineKind::Boning)
    );
}

#[test]
fn a_version_8_project_opens_with_its_seams_sewn() {
    let v8 = include_str!("fixtures/project-v8.json");
    assert!(v8.contains("\"schemaVersion\": 8"));
    assert!(!v8.contains("\"lacing\"") && !v8.contains("\"boning\""));
    let (e, issues) = Engine::open(v8, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let seams = &e.document().seams;
    assert!(!seams.is_empty());
    assert!(seams.iter().all(|s| s.lacing.is_none()));
    let saved = e.to_json("now");
    assert!(saved.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
}
