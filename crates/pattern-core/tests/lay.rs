//! Pieces laid out on fabric to cut.

use drapewright_pattern_core::lay::{LayNote, LayRequest, Layout, lay};
use drapewright_pattern_core::{
    Allowance, AllowanceMode, CutInstruction, Edge, EdgeKind, Grainline, Outline, Piece, Pt,
    Segment,
};

/// A rectangle `w` wide and `h` long, its grain down its length, as its
/// cutting line; its left side a fold if `fold`.
fn panel(id: &str, w: f64, h: f64, quantity: u32, fold: bool) -> Piece {
    let mut left = Edge::new(format!("{id}l"), vec![Segment::line([0.0, 0.0])]);
    if fold {
        left.kind = EdgeKind::Fold;
    }
    let mut p = Piece::new(
        id,
        id,
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new(format!("{id}t"), vec![Segment::line([w, 0.0])]),
                Edge::new(format!("{id}r"), vec![Segment::line([w, h])]),
                Edge::new(format!("{id}b"), vec![Segment::line([0.0, h])]),
                left,
            ],
        },
    );
    p.allowance = Allowance {
        mode: AllowanceMode::Included,
        seam: 10.0,
        hem: None,
    };
    p.cut = vec![CutInstruction {
        quantity,
        ..Default::default()
    }];
    p.grainline = Some(Grainline {
        from: Pt::new(w / 2.0, 50.0),
        to: Pt::new(w / 2.0, h - 50.0),
        one_way: false,
    });
    p
}

fn laid(pieces: Vec<Piece>, width: f64, folded: bool) -> Layout {
    lay(&LayRequest {
        pieces,
        material: "main".into(),
        width,
        folded,
        one_way: false,
    })
}

/// A placed piece's extent: (x0, y0, x1, y1).
fn extent(rings: &[Vec<Pt>]) -> (f64, f64, f64, f64) {
    rings
        .iter()
        .flatten()
        .fold((f64::MAX, f64::MAX, f64::MIN, f64::MIN), |e, p| {
            (e.0.min(p.x), e.1.min(p.y), e.2.max(p.x), e.3.max(p.y))
        })
}

#[test]
fn a_pair_is_laid_side_by_side_its_grain_along_the_fabric() {
    let layout = laid(vec![panel("front", 300.0, 500.0, 2, false)], 1500.0, false);
    assert_eq!(layout.placed.len(), 2);
    assert_eq!(
        layout.placed.iter().filter(|p| p.mirrored).count(),
        1,
        "one face down"
    );
    // Its length along the fabric, its width across: one beside the other.
    assert!((layout.length - 500.0).abs() < 1e-6, "{}", layout.length);
    for p in &layout.placed {
        let (x0, y0, x1, y1) = extent(&p.rings);
        assert!(((x1 - x0) - 500.0).abs() < 1e-6 && ((y1 - y0) - 300.0).abs() < 1e-6);
        assert!(y0 >= 0.0 && y1 <= 1500.0);
        // The grain along the fabric.
        assert!((p.grain[0].y - p.grain[1].y).abs() < 1e-6, "{:?}", p.grain);
    }
    let used = 2.0 * 300.0 * 500.0 / (1500.0 * 500.0);
    assert!((layout.used - used).abs() < 1e-6, "{}", layout.used);
}

#[test]
fn on_folded_fabric_a_pair_is_cut_through_both_layers() {
    let layout = laid(vec![panel("front", 300.0, 500.0, 2, false)], 1500.0, true);
    assert_eq!(layout.placed.len(), 1);
    assert_eq!(layout.placed[0].layers, 2);
    assert_eq!(layout.notes, []);
    // Three of them: the third through one layer.
    let layout = laid(vec![panel("front", 300.0, 500.0, 3, false)], 1500.0, true);
    assert_eq!(layout.placed.len(), 2);
    assert_eq!(
        layout.notes,
        [LayNote::OneLayer {
            piece: "front".into()
        }]
    );
}

#[test]
fn a_piece_cut_on_the_fold_goes_on_the_fabrics_fold() {
    let layout = laid(vec![panel("front", 250.0, 600.0, 1, true)], 1400.0, true);
    let p = &layout.placed[0];
    assert!(p.on_fold);
    let (_, y0, _, y1) = extent(&p.rings);
    assert!(y0.abs() < 1e-6 && (y1 - 250.0).abs() < 1e-6, "{y0}, {y1}");
    // The fold edge is the one at the fabric's fold: two corners there.
    let on = p.rings[0].iter().filter(|q| q.y.abs() < 1e-6).count();
    assert!(on >= 2, "{:?}", p.rings);
    // On open fabric, laid out whole.
    let layout = laid(vec![panel("front", 250.0, 600.0, 1, true)], 1400.0, false);
    let p = &layout.placed[0];
    assert_eq!(p.rings.len(), 2);
    let (_, y0, _, y1) = extent(&p.rings);
    assert!(((y1 - y0) - 500.0).abs() < 1e-6, "{y0}, {y1}");
}

#[test]
fn a_piece_drawn_on_its_side_is_turned_to_its_grain() {
    // Drawn lying down, its grain across the page.
    let mut sideways = panel("sleeve", 600.0, 300.0, 1, false);
    sideways.grainline = Some(Grainline {
        from: Pt::new(50.0, 150.0),
        to: Pt::new(550.0, 150.0),
        one_way: false,
    });
    let layout = laid(vec![sideways], 1500.0, false);
    assert!((layout.length - 600.0).abs() < 1e-6, "{}", layout.length);
    // Upright, the same piece lies along the fabric the other way.
    let layout = laid(vec![panel("sleeve", 300.0, 600.0, 1, false)], 1500.0, false);
    assert!((layout.length - 600.0).abs() < 1e-6, "{}", layout.length);
}

#[test]
fn pieces_are_laid_close_and_never_over_each_other() {
    let pieces = vec![
        panel("front", 300.0, 600.0, 2, false),
        panel("back", 320.0, 620.0, 2, false),
        panel("sleeve", 400.0, 550.0, 2, false),
        panel("cuff", 250.0, 80.0, 2, false),
        panel("collar", 420.0, 120.0, 1, false),
    ];
    let layout = laid(pieces, 1400.0, false);
    assert_eq!(layout.placed.len(), 9);
    let boxes: Vec<_> = layout.placed.iter().map(|p| extent(&p.rings)).collect();
    for (i, a) in boxes.iter().enumerate() {
        assert!(a.1 >= -1e-6 && a.3 <= 1400.0 + 1e-6);
        for b in &boxes[i + 1..] {
            let apart =
                a.2 <= b.0 + 1e-6 || b.2 <= a.0 + 1e-6 || a.3 <= b.1 + 1e-6 || b.3 <= a.1 + 1e-6;
            assert!(apart, "{a:?} and {b:?}");
        }
    }
    // Less fabric than laid end to end, two across.
    assert!(layout.length < 1600.0, "{}", layout.length);
    assert!(layout.used > 0.6, "{}", layout.used);
}

#[test]
fn a_piece_wider_than_the_fabric_is_left_out() {
    let layout = laid(vec![panel("cape", 900.0, 500.0, 2, false)], 1400.0, true);
    assert_eq!(layout.placed.len(), 0);
    assert_eq!(
        layout.notes,
        [LayNote::TooWide {
            piece: "cape".into()
        }]
    );
}
