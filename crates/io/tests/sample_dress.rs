//! The app's sample dress: FreeSewing's Sasha (MIT), drafted to the example
//! figure without its pockets and printed by Drapewright on A0 for a copy
//! shop. Read back from its PDF, Find seams sews it as it is made, nothing
//! left to sew by hand.

use std::collections::BTreeMap;
use std::path::Path;

use drapewright_io::pdf::import_pdf;
use drapewright_pattern_core::EdgeKind;
use drapewright_seams::suggest;

#[test]
fn the_sample_dress_is_sewn_whole_by_find_seams() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/pdf/freesewing-sasha-example-a0.pdf");
    let result = import_pdf(&std::fs::read(path).unwrap(), Some("sasha.pdf")).unwrap();
    let pieces = result.pieces;
    let name = |id: &str| pieces.iter().find(|p| p.id == id).unwrap().name.clone();
    let mut names: Vec<String> = pieces.iter().map(|p| p.name.clone()).collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Back inside",
            "Back outside",
            "Front inside",
            "Front outside",
            "Sleeve"
        ]
    );

    let suggested = suggest(&pieces, &[]);
    let mut seams: Vec<String> = suggested
        .iter()
        .map(|s| {
            let mut sides = [name(&s.seam.a.piece), name(&s.seam.b.piece)];
            sides.sort();
            sides.join(" + ")
        })
        .collect();
    seams.sort();
    // The princess seams, the sides, the shoulders, the centre back and the
    // sleeves' underarms; and each sleeve's head round its armhole, made of
    // four pieces' edges.
    assert_eq!(
        seams,
        [
            "Back inside + Back inside",
            "Back inside + Back outside",
            "Back inside + Front inside",
            "Back inside + Sleeve",
            "Back outside + Front outside",
            "Back outside + Sleeve",
            "Front inside + Front outside",
            "Front inside + Sleeve",
            "Front outside + Sleeve",
            "Sleeve + Sleeve",
        ]
    );

    // Left free: each piece's hem, the necklines and the sleeves' wrists;
    // not two hems sewn to each other, nor the armholes' pieces.
    let mut free: BTreeMap<String, usize> = BTreeMap::new();
    for piece in &pieces {
        let sewn = |edge: &str| {
            suggested.iter().any(|s| {
                (s.seam.a.piece == piece.id && s.seam.a.edge == edge)
                    || (s.seam.b.piece == piece.id && s.seam.b.edge == edge)
            })
        };
        let count = piece
            .outline
            .edges
            .iter()
            .filter(|e| e.kind != EdgeKind::Fold && !sewn(&e.id))
            .count();
        free.insert(piece.name.clone(), count);
    }
    assert_eq!(
        free,
        BTreeMap::from([
            ("Back inside".to_string(), 2),
            ("Back outside".to_string(), 1),
            ("Front inside".to_string(), 2),
            ("Front outside".to_string(), 1),
            ("Sleeve".to_string(), 1),
        ])
    );
}
