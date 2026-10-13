//! The test garments laid out on fabric to cut.

use drapewright_pattern_core::Piece;
use drapewright_pattern_core::lay::{LayRequest, lay};
use drapewright_project::{Engine, container};

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

#[test]
fn the_test_garments_are_laid_out_on_fabric_folded_and_open() {
    for name in ["blouse", "skirt", "corset"] {
        for folded in [true, false] {
            let layout = lay(&LayRequest {
                pieces: pieces(name),
                material: "main".into(),
                width: 1500.0,
                folded,
                one_way: false,
            });
            assert!(!layout.placed.is_empty(), "{name}");
            assert!(
                layout.length > 0.0 && layout.used > 0.1 && layout.used <= 1.0,
                "{name}, folded {folded}: {} long, {} used",
                layout.length,
                layout.used
            );
            // Every piece of it within the fabric.
            let across = if folded { 750.0 } else { 1500.0 };
            for p in &layout.placed {
                for q in p.rings.iter().flatten() {
                    assert!(q.x >= -1e-6 && q.x <= layout.length + 1e-6, "{name}: {q:?}");
                    assert!(q.y >= -1e-6 && q.y <= across + 1e-6, "{name}: {q:?}");
                }
            }
        }
    }
}

#[test]
fn a_blouse_cut_on_the_fold_has_its_front_and_back_at_the_fold() {
    let layout = lay(&LayRequest {
        pieces: pieces("blouse"),
        material: "main".into(),
        width: 1500.0,
        folded: true,
        one_way: false,
    });
    let mut at_fold: Vec<&str> = layout
        .placed
        .iter()
        .filter(|p| p.on_fold)
        .map(|p| p.name.as_str())
        .collect();
    at_fold.sort_unstable();
    assert_eq!(at_fold, ["Blouse back", "Blouse front"]);
    for p in layout.placed.iter().filter(|p| p.on_fold) {
        let top = p.rings[0].iter().map(|q| q.y).fold(f64::MAX, f64::min);
        assert!(top.abs() < 1e-6, "{}: {top}", p.name);
    }
}
