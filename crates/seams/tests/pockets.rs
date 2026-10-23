//! Patch pockets sewn onto their placements: found by Find seams, sewn on
//! by hand, and measured.

use drapewright_pattern_core::testing::garments::pocket_skirt;
use drapewright_pattern_core::{
    AllowanceMode, Edge, EdgeKind, InternalLine, LineKind, Piece, Pt, Segment,
};
use drapewright_seams::{Reason, Seam, Suggestion, measure, sew_onto, suggest};

/// The pocket's suggested seams, in the order they go round the pocket.
fn pocket_seams(suggested: &[Suggestion]) -> Vec<&Seam> {
    suggested
        .iter()
        .filter(|s| s.reasons.contains(&Reason::OnPlacement))
        .map(|s| &s.seam)
        .collect()
}

fn line_part(seam: &Seam) -> (f64, f64) {
    (seam.b.from.unwrap(), seam.b.to.unwrap())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1.0
}

/// The placement's perimeter: 130 across, 140 down each side, and the
/// bottom's 90 between two quarter circles of radius 20.
const ROUND_BOTTOM: f64 = 90.0 + std::f64::consts::PI * 20.0;
const ALL_ROUND: f64 = 130.0 + 2.0 * 140.0 + ROUND_BOTTOM;

#[test]
fn find_seams_sews_the_pocket_onto_its_placement() {
    let pieces = pocket_skirt();
    let suggested = suggest(&pieces, &[]);
    let pocket = pocket_seams(&suggested);
    // Its sides and bottom, its top (the hem) left open, round the
    // placement from the top right corner as drawn.
    let edges: Vec<&str> = pocket.iter().map(|s| s.a.edge.as_str()).collect();
    assert_eq!(edges, ["pr", "pb", "pl"], "{pocket:#?}");
    let parts: Vec<(f64, f64)> = pocket.iter().map(|s| line_part(s)).collect();
    let corners = [130.0, 270.0, 270.0 + ROUND_BOTTOM, ALL_ROUND];
    for (k, &(from, to)) in parts.iter().enumerate() {
        assert!(
            close(from, corners[k]) && close(to, corners[k + 1]),
            "{parts:?}"
        );
    }
    for s in &pocket {
        assert!(!s.reversed && !s.a.mirror && !s.confirmed);
        assert_eq!(s.a.piece, "pocket");
        assert_eq!(
            (s.b.piece.as_str(), s.b.line.as_deref()),
            ("front", Some("fpocket"))
        );
        assert!(s.a.from.is_none() && s.a.to.is_none(), "whole edges: {s:?}");
        assert_eq!(s.name, "Patch pocket");
        // As long on the pocket as on the placement.
        let report = measure(s, &pieces);
        assert!(
            report.problem.is_none() && report.difference.abs() < 0.5,
            "{report:?}"
        );
        assert!(report.notches.is_empty());
    }
    let confidence = suggested
        .iter()
        .find(|s| s.reasons.contains(&Reason::OnPlacement))
        .unwrap()
        .confidence;
    assert!(confidence > 0.9, "{confidence}");
    // The pocket's edges sewn to nothing else; the skirt's sides and centre
    // back sewn as before.
    for s in suggested
        .iter()
        .filter(|s| !s.reasons.contains(&Reason::OnPlacement))
    {
        assert!(!s.seam.touches("pocket"), "{s:?}");
    }
    let sewn = |a: &str, b: &str| {
        suggested.iter().any(|s| {
            let sides = [s.seam.a.edge.as_str(), s.seam.b.edge.as_str()];
            sides == [a, b] || sides == [b, a]
        })
    };
    assert!(sewn("fs", "bs"), "{suggested:#?}");
    // Sewn already, it is not suggested again.
    let confirmed: Vec<Seam> = pocket
        .iter()
        .map(|&s| Seam {
            confirmed: true,
            ..s.clone()
        })
        .collect();
    assert!(pocket_seams(&suggest(&pieces, &confirmed)).is_empty());
}

/// The skirt with its placement drawn otherwise: `segments` from `start`.
fn placed(start: [f64; 2], segments: Vec<Segment>) -> Vec<Piece> {
    let mut pieces = pocket_skirt();
    let line = &mut pieces[0].lines[0];
    line.start = Pt::new(start[0], start[1]);
    line.segments = segments;
    pieces
}

/// The placement's bottom from right to left, its corners rounded.
fn round_bottom() -> Vec<Segment> {
    let k = 0.5523 * 20.0;
    vec![
        Segment::cubic([190.0, 260.0 + k], [170.0 + k, 280.0], [170.0, 280.0]),
        Segment::line([80.0, 280.0]),
        Segment::cubic([80.0 - k, 280.0], [60.0, 260.0 + k], [60.0, 260.0]),
    ]
}

#[test]
fn a_placement_drawn_along_the_three_sides_sewn_fits_too() {
    // From the top right corner down, round and up to the top left.
    let pieces = placed(
        [190.0, 120.0],
        [
            vec![Segment::line([190.0, 260.0])],
            round_bottom(),
            vec![Segment::line([60.0, 120.0])],
        ]
        .concat(),
    );
    let suggested = suggest(&pieces, &[]);
    let pocket = pocket_seams(&suggested);
    let edges: Vec<&str> = pocket.iter().map(|s| s.a.edge.as_str()).collect();
    assert_eq!(edges, ["pr", "pb", "pl"], "{pocket:#?}");
    assert!(close(line_part(pocket[0]).0, 0.0));
    assert!(close(line_part(pocket[2]).1, 280.0 + ROUND_BOTTOM));
    // Drawn the other way round, the seams are sewn the other way.
    let pieces = placed(
        [60.0, 120.0],
        [
            vec![Segment::line([60.0, 260.0])],
            {
                let k = 0.5523 * 20.0;
                vec![
                    Segment::cubic([60.0, 260.0 + k], [80.0 - k, 280.0], [80.0, 280.0]),
                    Segment::line([170.0, 280.0]),
                    Segment::cubic([170.0 + k, 280.0], [190.0, 260.0 + k], [190.0, 260.0]),
                ]
            },
            vec![Segment::line([190.0, 120.0])],
        ]
        .concat(),
    );
    let suggested = suggest(&pieces, &[]);
    let pocket = pocket_seams(&suggested);
    assert_eq!(pocket.len(), 3, "{pocket:#?}");
    assert!(pocket.iter().all(|s| s.reversed));
    let right = pocket.iter().find(|s| s.a.edge == "pr").unwrap();
    assert!(close(line_part(right).1, 280.0 + ROUND_BOTTOM), "{right:?}");
}

#[test]
fn a_seam_past_a_closed_placements_start_is_sewn_in_two() {
    // Drawn from the middle of its bottom, round clockwise.
    let k = 0.5523 * 20.0;
    let pieces = placed(
        [125.0, 280.0],
        vec![
            Segment::line([80.0, 280.0]),
            Segment::cubic([80.0 - k, 280.0], [60.0, 260.0 + k], [60.0, 260.0]),
            Segment::line([60.0, 120.0]),
            Segment::line([190.0, 120.0]),
            Segment::line([190.0, 260.0]),
            Segment::cubic([190.0, 260.0 + k], [170.0 + k, 280.0], [170.0, 280.0]),
            Segment::line([125.0, 280.0]),
        ],
    );
    let suggested = suggest(&pieces, &[]);
    let pocket = pocket_seams(&suggested);
    let edges: Vec<&str> = pocket.iter().map(|s| s.a.edge.as_str()).collect();
    assert_eq!(edges, ["pr", "pb", "pb", "pl"], "{pocket:#?}");
    // The bottom's two parts meet where the placement starts and ends.
    let (first, second) = (pocket[1], pocket[2]);
    assert!(close(line_part(first).1, ALL_ROUND), "{first:?}");
    assert!(close(line_part(second).0, 0.0), "{second:?}");
    assert!(close(first.a.to.unwrap(), second.a.from.unwrap()));
    for s in &pocket {
        let report = measure(s, &pieces);
        assert!(
            report.problem.is_none() && report.difference.abs() < 0.5,
            "{report:?}"
        );
    }
}

/// A rectangular pocket cut 15 × 20 cm with its allowance included, all of
/// it 1 cm: its top's 3 cm turn-down not marked, so its stitching line is
/// 2 cm above the placement's top.
fn cut_pocket() -> Piece {
    let edge = |id: &str, to: [f64; 2]| Edge::new(id, vec![Segment::line(to)]);
    let mut p = Piece::new(
        "pocket",
        "Pocket",
        drapewright_pattern_core::Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                edge("pt", [150.0, 0.0]),
                edge("pr", [150.0, 200.0]),
                edge("pb", [0.0, 200.0]),
                edge("pl", [0.0, 0.0]),
            ],
        },
    );
    p.allowance.mode = AllowanceMode::Included;
    p.allowance.seam = 10.0;
    p.cut[0].quantity = 2;
    p.grainline = pocket_skirt()[2].grainline;
    p
}

#[test]
fn a_pocket_with_its_turn_down_unmarked_is_sewn_up_to_it() {
    let mut pieces = placed(
        [60.0, 120.0],
        vec![
            Segment::line([190.0, 120.0]),
            Segment::line([190.0, 280.0]),
            Segment::line([60.0, 280.0]),
            Segment::line([60.0, 120.0]),
        ],
    );
    pieces[2] = cut_pocket();
    let suggested = suggest(&pieces, &[]);
    let pocket = pocket_seams(&suggested);
    let edges: Vec<&str> = pocket.iter().map(|s| s.a.edge.as_str()).collect();
    assert_eq!(edges, ["pr", "pb", "pl"], "{pocket:#?}");
    // The right side sewn from 2 cm down it, the left up to 2 cm short of
    // its top; the bottom whole.
    let (right, bottom, left) = (pocket[0], pocket[1], pocket[2]);
    assert!(
        close(right.a.from.unwrap(), 20.0) && close(right.a.to.unwrap(), 180.0),
        "{right:?}"
    );
    assert!(
        bottom.a.from.is_none() && bottom.a.to.is_none(),
        "{bottom:?}"
    );
    assert!(
        close(left.a.from.unwrap(), 0.0) && close(left.a.to.unwrap(), 160.0),
        "{left:?}"
    );
    for s in &pocket {
        assert!(measure(s, &pieces).difference.abs() < 0.5);
    }
}

#[test]
fn a_placement_of_another_shape_is_left_alone_but_can_be_sewn_onto_by_hand() {
    // A square 12 cm across.
    let square = vec![
        Segment::line([180.0, 120.0]),
        Segment::line([180.0, 240.0]),
        Segment::line([60.0, 240.0]),
        Segment::line([60.0, 120.0]),
    ];
    let pieces = placed([60.0, 120.0], square);
    assert!(pocket_seams(&suggest(&pieces, &[])).is_empty());
    assert!(sew_onto(&pieces, &[], "pocket", "front", "fpocket").is_none());
    // A placement drawn 5 mm bigger all round than the pocket: not found,
    // but sewn on by hand.
    let k = 0.5523 * 25.0;
    let bigger = vec![
        Segment::line([195.0, 115.0]),
        Segment::line([195.0, 260.0]),
        Segment::cubic([195.0, 260.0 + k], [170.0 + k, 285.0], [170.0, 285.0]),
        Segment::line([80.0, 285.0]),
        Segment::cubic([80.0 - k, 285.0], [55.0, 260.0 + k], [55.0, 260.0]),
        Segment::line([55.0, 115.0]),
    ];
    let pieces = placed([55.0, 115.0], bigger);
    let seams = sew_onto(&pieces, &[], "pocket", "front", "fpocket").expect("sewn by hand");
    let edges: Vec<&str> = seams.iter().map(|s| s.a.edge.as_str()).collect();
    assert_eq!(edges, ["pr", "pb", "pl"]);
    assert!(seams.iter().all(|s| s.confirmed));
    assert!(seams.iter().all(|s| measure(s, &pieces).problem.is_none()));
    // Not onto itself, nor onto a line that isn't there.
    assert!(sew_onto(&pieces, &[], "pocket", "pocket", "fpocket").is_none());
    assert!(sew_onto(&pieces, &[], "pocket", "front", "nothing").is_none());
}

#[test]
fn a_pair_of_pockets_goes_on_a_front_drawn_whole_one_each_side() {
    // The front drawn whole, cut once: its half and that half mirrored in
    // the centre front, the placement on each side.
    let mut pieces = pocket_skirt();
    let half = pieces[0].clone();
    let mirror = |p: Pt| Pt::new(-p.x, p.y);
    let front = &mut pieces[0];
    front.outline.start = Pt::new(-185.0, 8.0);
    front.outline.edges = vec![
        Edge::new("fwl", vec![Segment::line([0.0, 0.0])]),
        Edge::new("fwr", vec![Segment::line([185.0, 8.0])]),
        half.outline.edges[1].clone(),
        Edge {
            kind: EdgeKind::Hem,
            ..Edge::new(
                "fh",
                vec![Segment::cubic(
                    [190.0, 608.0],
                    [-190.0, 608.0],
                    [-290.0, 600.0],
                )],
            )
        },
        Edge::new(
            "fsl",
            vec![Segment::cubic(
                [-255.0, 200.0],
                [-215.0, 80.0],
                [-185.0, 8.0],
            )],
        ),
    ];
    front.cut[0].on_fold = Some(false);
    let mut other = front.lines[0].clone();
    other.id = "fpocket2".into();
    other.start = mirror(other.start);
    for s in &mut other.segments {
        s.map_points(mirror);
    }
    front.lines.push(other);
    let suggested = suggest(&pieces, &[]);
    let pocket = pocket_seams(&suggested);
    assert_eq!(pocket.len(), 6, "{pocket:#?}");
    let mirrored = |line: &str| -> Vec<bool> {
        pocket
            .iter()
            .filter(|s| s.b.line.as_deref() == Some(line))
            .map(|s| s.a.mirror)
            .collect()
    };
    // By hand too: the pocket as drawn onto one, then its mirror copy onto
    // the other.
    let first = sew_onto(&pieces, &[], "pocket", "front", "fpocket").unwrap();
    assert!(first.iter().all(|s| !s.a.mirror));
    let second = sew_onto(&pieces, &first, "pocket", "front", "fpocket2").unwrap();
    assert!(second.iter().all(|s| s.a.mirror), "{second:#?}");
    // One pocket as drawn, the other its mirror copy.
    let (one, two) = (mirrored("fpocket"), mirrored("fpocket2"));
    assert_eq!(one.len(), 3);
    assert!(
        one.iter().all(|&m| m == one[0]) && two.iter().all(|&m| m != one[0]),
        "{one:?} {two:?}"
    );
}

#[test]
fn a_line_side_names_a_line_that_must_be_there() {
    let pieces = pocket_skirt();
    let mut seam = Seam {
        id: "s".into(),
        name: String::new(),
        a: drapewright_seams::SeamSide::whole("pocket", "pr"),
        b: drapewright_seams::SeamSide::on_line("front", "fpocket", 130.0, 270.0),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    };
    let report = measure(&seam, &pieces);
    assert!(report.problem.is_none());
    assert!((report.b_length - 140.0).abs() < 0.01, "{report:?}");
    seam.b.to = Some(ALL_ROUND + 5.0);
    assert!(matches!(
        measure(&seam, &pieces).problem,
        Some(drapewright_seams::SeamProblem::OutsideLine { .. })
    ));
    seam.b.line = Some("gone".into());
    assert!(matches!(
        measure(&seam, &pieces).problem,
        Some(drapewright_seams::SeamProblem::MissingLine { .. })
    ));
    // The placement is a placement line, as the DXF importer makes them.
    assert_eq!(pieces[0].lines[0].kind, LineKind::Placement);
    let _: &InternalLine = &pieces[0].lines[0];
    assert_eq!(pieces[2].outline.edges[0].kind, EdgeKind::Hem);
}

/// A rectangle `w` by `h` mm as a piece, its top left at the origin.
fn rectangle(id: &str, name: &str, w: f64, h: f64) -> Piece {
    let edge = |e: &str, to: [f64; 2]| Edge::new(format!("{id}{e}"), vec![Segment::line(to)]);
    let mut p = Piece::new(
        id,
        name,
        drapewright_pattern_core::Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                edge("t", [w, 0.0]),
                edge("r", [w, h]),
                edge("b", [0.0, h]),
                edge("l", [0.0, 0.0]),
            ],
        },
    );
    p.grainline = pocket_skirt()[2].grainline;
    p
}

#[test]
fn a_band_or_a_tab_is_no_pocket() {
    // A waistband as long as a straight line across the back (a hip line):
    // its edges run straight on, as no pocket's sides and bottom do.
    let mut pieces = pocket_skirt();
    pieces[0].lines.clear();
    pieces[1].lines.push(InternalLine {
        id: "hip".into(),
        kind: LineKind::Line,
        start: Pt::new(10.0, 200.0),
        segments: vec![Segment::line([290.0, 200.0])],
        label: Some("Hip".into()),
    });
    let mut band = rectangle("band", "Waistband", 280.0, 40.0);
    band.outline.edges[0].kind = EdgeKind::Hem;
    pieces[2] = band;
    assert!(pocket_seams(&suggest(&pieces, &[])).is_empty());
    // A tab 2.5 cm wide, its outline drawn on the front where it goes:
    // too small for a pocket.
    let mut pieces = pocket_skirt();
    pieces[2] = rectangle("tab", "Tab", 25.0, 80.0);
    let line = &mut pieces[0].lines[0];
    line.start = Pt::new(100.0, 100.0);
    line.segments = vec![
        Segment::line([125.0, 100.0]),
        Segment::line([125.0, 180.0]),
        Segment::line([100.0, 180.0]),
        Segment::line([100.0, 100.0]),
    ];
    assert!(pocket_seams(&suggest(&pieces, &[])).is_empty());
    // The same 13 × 16 cm, cut from interfacing: not a pocket either.
    let mut pieces = pocket_skirt();
    pieces[2].cut[0].material = "interfacing".into();
    assert!(pocket_seams(&suggest(&pieces, &[])).is_empty());
}

#[test]
fn a_pocket_sewn_on_in_part_has_the_rest_of_its_seams_suggested() {
    let pieces = pocket_skirt();
    let all: Vec<Seam> = pocket_seams(&suggest(&pieces, &[]))
        .into_iter()
        .map(|s| Seam {
            id: format!("s{}", s.a.edge),
            confirmed: true,
            ..s.clone()
        })
        .collect();
    // Its bottom sewn, by itself: its sides still suggested, as before.
    let bottom: Vec<Seam> = all.iter().filter(|s| s.a.edge == "pb").cloned().collect();
    let rest = suggest(&pieces, &bottom);
    let sides: Vec<&Seam> = pocket_seams(&rest);
    let edges: Vec<&str> = sides.iter().map(|s| s.a.edge.as_str()).collect();
    assert_eq!(edges, ["pr", "pl"], "{sides:#?}");
    for s in &sides {
        let before = all.iter().find(|x| x.a.edge == s.a.edge).unwrap();
        assert_eq!((s.b.from, s.b.to), (before.b.from, before.b.to));
    }
    // All three sewn: nothing more.
    assert!(pocket_seams(&suggest(&pieces, &all)).is_empty());
}
