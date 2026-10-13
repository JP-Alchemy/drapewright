//! A pattern's sizes sewn and measured on the avatar, to choose one by.

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::{Piece, Pt};
use drapewright_project::{Engine, container};
use drapewright_wasm_3d::sizes::{
    BlendRequest, LevelSize, OneSize, SizePieces, SizesRequest, blend_sizes, measure_sizes,
};

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

/// A garment in three sizes, each 5% wider than the last, each size's
/// pieces their own.
fn graded(name: &str) -> Vec<SizePieces> {
    let pieces = pieces(name);
    [("A", 1.0), ("B", 1.05), ("C", 1.1)]
        .into_iter()
        .map(|(size, by)| SizePieces {
            size: size.into(),
            pieces: pieces
                .iter()
                .map(|p| Piece {
                    id: format!("{}-{size}", p.id),
                    ..wider(p.clone(), by)
                })
                .collect(),
        })
        .collect()
}

fn at(level: &str, size: &str) -> LevelSize {
    LevelSize {
        level: level.into(),
        size: size.into(),
    }
}

/// The garment's girth at `level`, sewn from `pieces`.
fn girth(pieces: &[Piece], level: &str) -> f64 {
    let measured = measure_sizes(
        &model(),
        &SizesRequest {
            sizes: vec![SizePieces {
                size: "-".into(),
                pieces: pieces.to_vec(),
            }],
            settings: Shape::default().into(),
        },
    );
    measured[0]
        .girths
        .iter()
        .find(|g| g.level == level)
        .and_then(|g| g.garment)
        .unwrap_or_else(|| panic!("{level}: {measured:?}"))
}

#[test]
fn a_skirt_is_cut_one_size_at_the_waist_and_another_at_the_hip() {
    let sizes = graded("skirt");
    let blended = blend_sizes(
        &model(),
        &BlendRequest {
            sizes: sizes.clone(),
            settings: Shape::default().into(),
            at: vec![at("waist", "A"), at("hip", "C")],
        },
    )
    .unwrap();
    assert_eq!(blended.pieces.len(), sizes[0].pieces.len());
    assert_eq!(blended.notes, []);
    // Each blended piece keeps its name and its edges.
    for (piece, a) in blended.pieces.iter().zip(&sizes[0].pieces) {
        assert_eq!(piece.name, a.name);
        let ids = |p: &Piece| {
            p.outline
                .edges
                .iter()
                .map(|e| e.id.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(piece), ids(a));
    }
    // As big round the waist as size A, and round the hip as size C.
    let (a, c) = (&sizes[0].pieces, &sizes[2].pieces);
    let waist = girth(&blended.pieces, "waist");
    let hip = girth(&blended.pieces, "hip");
    assert!(
        (waist - girth(a, "waist")).abs() < 3.0,
        "waist {waist} vs {}",
        girth(a, "waist")
    );
    assert!(
        (hip - girth(c, "hip")).abs() < 3.0,
        "hip {hip} vs {}",
        girth(c, "hip")
    );
}

#[test]
fn sleeves_are_cut_in_the_size_at_the_bust() {
    let sizes = graded("blouse");
    let blended = blend_sizes(
        &model(),
        &BlendRequest {
            sizes: sizes.clone(),
            settings: Shape::default().into(),
            at: vec![at("bust", "A"), at("waist", "B"), at("hip", "C")],
        },
    )
    .unwrap();
    assert_eq!(blended.pieces.len(), sizes[0].pieces.len());
    let sleeves: Vec<_> = blended
        .notes
        .iter()
        .filter(|n| n.why == OneSize::RoundAnArm)
        .collect();
    assert!(!sleeves.is_empty(), "{:?}", blended.notes);
    for note in sleeves {
        assert_eq!(note.size, "A", "{note:?}");
        let sleeve = blended
            .pieces
            .iter()
            .find(|p| p.name == note.piece)
            .unwrap();
        let a = sizes[0]
            .pieces
            .iter()
            .find(|p| p.name == note.piece)
            .unwrap();
        assert_eq!(sleeve, a);
    }
}

#[test]
fn a_crotch_piece_is_cut_in_the_size_at_the_hip() {
    let sizes = graded("corset");
    let blended = blend_sizes(
        &model(),
        &BlendRequest {
            sizes,
            settings: Shape::default().into(),
            at: vec![at("bust", "A"), at("waist", "B"), at("hip", "C")],
        },
    )
    .unwrap();
    let crotch: Vec<_> = blended
        .notes
        .iter()
        .filter(|n| n.why == OneSize::BetweenLegs)
        .collect();
    assert!(
        matches!(crotch[..], [n] if n.size == "C"),
        "{:?}",
        blended.notes
    );
}

#[test]
fn a_corset_measures_in_proportion_however_big() {
    let corset = pieces("corset");
    let sizes = [1.0, 1.3, 1.6]
        .into_iter()
        .map(|by| SizePieces {
            size: format!("{by}"),
            pieces: corset.iter().map(|p| wider(p.clone(), by)).collect(),
        })
        .collect();
    let measured = measure_sizes(
        &model(),
        &SizesRequest {
            sizes,
            settings: Shape::default().into(),
        },
    );
    let at = |k: usize, level: &str| {
        measured[k]
            .girths
            .iter()
            .find(|g| g.level == level)
            .and_then(|g| g.garment)
            .unwrap_or_else(|| panic!("{level}: {measured:?}"))
    };
    for level in ["bust", "waist"] {
        for (k, by) in [(1, 1.3), (2, 1.6)] {
            let ratio = at(k, level) / at(0, level);
            assert!(
                (ratio - by).abs() < 0.03,
                "{level} ×{by}: {ratio} {measured:?}"
            );
        }
    }
}
