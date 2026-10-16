//! The words printed on an imported piece, kept with its source (schema
//! version 7): saved and read back, and none in projects from before.

use drapewright_pattern_core::{
    Edge, Outline, Piece, PrintedText, Pt, Segment, Source, SourceFormat,
};
use drapewright_project::{Command, Engine, SCHEMA_VERSION};

fn printed(text: &str, x: f64, y: f64, size: f64) -> PrintedText {
    PrintedText {
        text: text.into(),
        at: Pt::new(x, y),
        size,
        angle: 90.0,
    }
}

/// A rectangle from a PDF, with the words printed on it.
fn piece() -> Piece {
    let mut piece = Piece::new(
        "",
        "Piece 1",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([300.0, 0.0])]),
                Edge::new("right", vec![Segment::line([300.0, 500.0])]),
                Edge::new("bottom", vec![Segment::line([0.0, 500.0])]),
                Edge::new("left", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    piece.source = Some(Source {
        format: SourceFormat::Pdf,
        file: Some("blouse.pdf".into()),
        sha256: Some("0f3a9c".into()),
        reference: None,
        scale: None,
        words: vec![
            printed("4", 120.0, 200.0, 8.8),
            printed("FRONT FACING -A,B", 140.0, 210.0, 2.2),
            printed("PAREMENTURE DEVANT", 143.0, 210.0, 1.1),
        ],
    });
    piece
}

#[test]
fn the_words_printed_on_a_piece_are_saved_and_read_back() {
    let mut e = Engine::new("prj", "Test", "2026-10-16T00:00:00Z", "test");
    e.dispatch(
        Command::AddPieces {
            pieces: vec![piece()],
        },
        Default::default(),
    )
    .unwrap();
    let json = e.to_json("2026-10-16T00:00:00Z");
    assert!(json.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
    assert!(json.contains("PAREMENTURE DEVANT"));
    let (again, issues) = Engine::open(&json, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let words = &again.document().pieces[0].source.as_ref().unwrap().words;
    assert_eq!(words, &piece().source.unwrap().words);
}

#[test]
fn a_version_6_project_opens_with_no_words_kept() {
    let v6 = include_str!("fixtures/project-v6.json");
    assert!(v6.contains("\"schemaVersion\": 6"));
    let (e, issues) = Engine::open(v6, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let source = e.document().pieces[0].source.as_ref().unwrap();
    assert_eq!(source.file.as_deref(), Some("skirt.pdf"));
    assert!(source.words.is_empty());
}
