//! A pattern's sizes sewn and measured on the avatar, to choose one by.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::{Piece, Pt};
use drapewright_project::{Engine, container};
use drapewright_wasm_3d::sizes::{SizePieces, SizesRequest, measure_sizes};

fn model() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).expect("the body file")).expect("a body file")
}

/// A test garment's pieces (`fixtures/drapewright/{name}.drapewright`).
fn pieces(name: &str) -> Vec<Piece> {
    let path = format!(
        "{}/../../fixtures/drapewright/{name}.drapewright",
        env!("CARGO_MANIFEST_DIR")
    );
    let unpacked = container::unpack(&std::fs::read(path).unwrap()).unwrap();
    let (engine, _) = Engine::open(&unpacked.project_json, "test").unwrap();
    engine
        .document()
        .pieces
        .iter()
        .map(|p| (**p).clone())
        .collect()
}

/// `piece` drawn `by` times wider, as long as it was: a bigger size of it,
/// as grading mostly makes one.
fn wider(mut piece: Piece, by: f64) -> Piece {
    let scale = |p: Pt| Pt::new(p.x * by, p.y);
    piece.outline.start = scale(piece.outline.start);
    for edge in &mut piece.outline.edges {
        for segment in &mut edge.segments {
            segment.map_points(scale);
        }
    }
    for line in &mut piece.lines {
        line.start = scale(line.start);
        for segment in &mut line.segments {
            segment.map_points(scale);
        }
    }
    if let Some(g) = &mut piece.grainline {
        g.from = scale(g.from);
        g.to = scale(g.to);
    }
    piece
}

#[test]
fn each_size_is_sewn_and_measured_round_the_body() {
    let blouse = pieces("blouse");
    let request = SizesRequest {
        sizes: vec![
            SizePieces {
                size: "A".into(),
                pieces: blouse.clone(),
            },
            SizePieces {
                size: "B".into(),
                pieces: blouse.into_iter().map(|p| wider(p, 1.05)).collect(),
            },
        ],
        settings: Shape::default().into(),
    };
    let measured = measure_sizes(&model(), &request);
    assert_eq!(measured.len(), 2);
    let [a, b] = &measured[..] else {
        unreachable!()
    };
    assert_eq!((a.size.as_str(), b.size.as_str()), ("A", "B"));
    assert_eq!(a.girths.len(), 3, "{a:?}");
    for (ga, gb) in a.girths.iter().zip(&b.girths) {
        assert_eq!(ga.level, gb.level);
        assert!((ga.body - gb.body).abs() < 1e-9, "the same body");
        // Each level of the bigger size measures 5% bigger.
        let (x, y) = (ga.garment.unwrap(), gb.garment.unwrap());
        assert!((y / x - 1.05).abs() < 0.01, "{}: {x} then {y}", ga.level);
    }
}

#[test]
fn panels_drawn_for_one_side_are_measured_as_pairs() {
    // The test corset, cut once as its pattern leaves it: measured as a
    // whole corset, not half of one.
    let corset = pieces("corset");
    let measured = measure_sizes(
        &model(),
        &SizesRequest {
            sizes: vec![SizePieces {
                size: "C".into(),
                pieces: corset,
            }],
            settings: Shape::default().into(),
        },
    );
    let waist = measured[0]
        .girths
        .iter()
        .find(|g| g.level == "waist")
        .and_then(|g| g.garment);
    // Three panels a side, about 55 mm wide at the waist each.
    assert!(
        waist.is_some_and(|w| (250.0..400.0).contains(&w)),
        "{measured:?}"
    );
}
