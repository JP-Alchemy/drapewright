//! Property tests for measuring and editing outlines.

use proptest::prelude::*;
use soom_pattern_core::geom::ARCLEN_ACCURACY;
use soom_pattern_core::ops::{join_with_next, normalize_orientation, reverse, split_edge};
use soom_pattern_core::testing::{arb_outline, arb_piece};
use soom_pattern_core::{Notch, NotchFrom, NotchKind, Piece};

use kurbo::ParamCurveArclen;

fn piece_with(outline: soom_pattern_core::Outline) -> Piece {
    Piece::new("p1", "Random", outline)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn generated_outlines_are_closed_and_clockwise(outline in arb_outline()) {
        prop_assert!(outline.closure_gap() < 1e-9);
        prop_assert!(outline.signed_area() > 0.0);
    }

    #[test]
    fn edge_lengths_add_up_to_the_outline_length(outline in arb_outline()) {
        let sum: f64 = (0..outline.edges.len()).map(|i| outline.edge_geom(i).length()).sum();
        let total: f64 = outline.to_bezpath().segments().map(|s| s.arclen(ARCLEN_ACCURACY)).sum();
        prop_assert!((sum - total).abs() < 1e-6, "{sum} vs {total}");
    }

    #[test]
    fn reversing_twice_is_the_identity(piece in arb_piece()) {
        let mut p = piece.clone();
        reverse(&mut p);
        prop_assert!((p.outline.signed_area() + piece.outline.signed_area()).abs() < 1e-6);
        reverse(&mut p);
        prop_assert_eq!(p, piece);
    }

    #[test]
    fn normalizing_a_reversed_piece_restores_its_geometry(piece in arb_piece()) {
        let mut p = piece.clone();
        reverse(&mut p);
        prop_assert!(normalize_orientation(&mut p));
        prop_assert_eq!(p, piece);
    }

    #[test]
    fn point_at_distance_round_trips_through_nearest(outline in arb_outline(), frac in 0.0f64..=1.0) {
        for i in 0..outline.edges.len() {
            let geom = outline.edge_geom(i);
            let d = frac * geom.length();
            let near = geom.nearest(geom.point_at(d)).unwrap();
            prop_assert!(near.distance < 1e-6);
            // Distances agree unless the curve passes near itself.
            let back = geom.point_at(near.along);
            prop_assert!(back.distance(geom.point_at(d)) < 1e-6);
        }
    }

    #[test]
    fn split_then_join_preserves_the_edge(
        outline in arb_outline(),
        edge_pick in any::<prop::sample::Index>(),
        frac in 0.05f64..0.95,
        notch_frac in 0.0f64..1.0,
        from_end in any::<bool>(),
    ) {
        let mut piece = piece_with(outline);
        let i = edge_pick.index(piece.outline.edges.len());
        let id = piece.outline.edges[i].id.clone();
        let geom = piece.outline.edge_geom(i);
        let len = geom.length();
        let notch_at = notch_frac * len;
        piece.notches.push(Notch {
            id: "n1".into(),
            edge: id.clone(),
            at: notch_at,
            from: if from_end { NotchFrom::End } else { NotchFrom::Start },
            kind: NotchKind::Single,
            angle: 0.0,
        });
        let notch_point = |p: &Piece| {
            let n = &p.notches[0];
            let j = p.outline.edge_index(&n.edge).unwrap();
            let g = p.outline.edge_geom(j);
            let along = match n.from { NotchFrom::Start => n.at, NotchFrom::End => g.length() - n.at };
            g.point_at(along)
        };
        let before = notch_point(&piece);
        let samples: Vec<_> = (0..=10).map(|s| geom.point_at(len * s as f64 / 10.0)).collect();

        split_edge(&mut piece, &id, frac * len, "split".into()).unwrap();
        prop_assert!(piece.outline.closure_gap() < 1e-9);
        let a = piece.outline.edge_index(&id).unwrap();
        let first = piece.outline.edge_geom(a).length();
        let second = piece.outline.edge_geom(a + 1).length();
        prop_assert!((first + second - len).abs() < 1e-6, "{first} + {second} vs {len}");
        prop_assert!((first - frac * len).abs() < 1e-6);
        prop_assert!(notch_point(&piece).distance(before) < 1e-6);

        join_with_next(&mut piece, &id).unwrap();
        let j = piece.outline.edge_index(&id).unwrap();
        let joined = piece.outline.edge_geom(j);
        prop_assert!((joined.length() - len).abs() < 1e-6);
        for (s, p) in samples.iter().enumerate() {
            let q = joined.point_at(len * s as f64 / 10.0);
            prop_assert!(q.distance(*p) < 1e-5, "sample {s}: {q:?} vs {p:?}");
        }
        prop_assert!(notch_point(&piece).distance(before) < 1e-6);
    }

    #[test]
    fn pieces_survive_a_json_round_trip(piece in arb_piece()) {
        let json = serde_json::to_string(&piece).unwrap();
        let back: Piece = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back, piece);
    }
}
