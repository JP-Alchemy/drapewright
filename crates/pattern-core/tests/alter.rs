//! Alterations: lengthening and shortening a piece across a line, kept as
//! steps that can be switched off.

use drapewright_pattern_core::build::reassign_ids;
use drapewright_pattern_core::ops::transform;
use drapewright_pattern_core::{
    AlterError, AlterLine, Alteration, Change, Drawn, Edge, Grainline, InternalLine, LineKind,
    Mark, MarkKind, Notch, NotchFrom, NotchKind, Outline, Piece, Pt, Segment, alter,
};
use kurbo::Affine;

/// A rectangle 200 mm wide and 400 tall (y down), its left side notched
/// 300 mm down, a drill hole at (100, 350) and a grainline down its middle.
fn rectangle() -> Piece {
    let mut p = Piece::new(
        "p",
        "Rectangle",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([200.0, 0.0])]),
                Edge::new("right", vec![Segment::line([200.0, 400.0])]),
                Edge::new("bottom", vec![Segment::line([0.0, 400.0])]),
                Edge::new("left", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    p.notches.push(Notch {
        id: "n".into(),
        edge: "left".into(),
        at: 100.0,
        from: NotchFrom::Start,
        kind: NotchKind::default(),
        angle: 0.0,
    });
    p.marks.push(Mark {
        id: "m".into(),
        kind: MarkKind::Drill,
        at: Pt::new(100.0, 350.0),
        size: None,
        length: None,
        angle: 0.0,
        ..mark_defaults()
    });
    p.grainline = Some(Grainline {
        from: Pt::new(100.0, 50.0),
        to: Pt::new(100.0, 350.0),
        one_way: false,
    });
    p
}

fn mark_defaults() -> Mark {
    serde_json::from_str(r#"{"id": "", "kind": "drill", "at": [0, 0]}"#).unwrap()
}

/// Lengthened or shortened by `by` across y = `y`.
fn across(y: f64, by: f64) -> Alteration {
    Alteration {
        id: "a".into(),
        on: true,
        change: Change::Lengthen {
            line: AlterLine::Through {
                from: Pt::new(-10.0, y),
                to: Pt::new(210.0, y),
            },
            by,
        },
    }
}

fn height(shape: &Drawn) -> f64 {
    shape.outline.bounds().height()
}

#[test]
fn lengthened_the_part_below_the_line_moves_down() {
    let drawn = Drawn::of(&rectangle());
    let longer = alter(&drawn, &[across(200.0, 30.0)]).unwrap();
    assert!((height(&longer) - 430.0).abs() < 1e-9);
    assert!((longer.outline.bounds().width() - 200.0).abs() < 1e-9);
    // The same edges, the sides each bridged across the gap.
    let ids: Vec<&str> = longer.outline.edges.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["top", "right", "bottom", "left"]);
    assert_eq!(longer.outline.edges[1].segments.len(), 3);
    assert!(longer.outline.closure_gap() < 1e-9);
    // The notch 100 mm up the left side from its start at the bottom left
    // (300 mm down) is below the line: still 100 mm from that corner.
    let left = longer.outline.edge_geom(3);
    assert!((left.length() - 430.0).abs() < 1e-9);
    assert!((longer.notches[0].at - 100.0).abs() < 1e-6);
    // Below the line, moved down; above, where it was.
    assert_eq!(longer.marks[0].at, Pt::new(100.0, 380.0));
    let grain = longer.grainline.unwrap();
    assert_eq!(
        (grain.from, grain.to),
        (Pt::new(100.0, 50.0), Pt::new(100.0, 380.0))
    );
}

#[test]
fn shortened_a_tuck_is_folded_out_below_the_line() {
    let drawn = Drawn::of(&rectangle());
    let shorter = alter(&drawn, &[across(200.0, -30.0)]).unwrap();
    assert!((height(&shorter) - 370.0).abs() < 1e-9);
    assert!(shorter.outline.closure_gap() < 1e-9);
    assert_eq!(shorter.marks[0].at, Pt::new(100.0, 320.0));
    assert!((shorter.outline.edge_geom(3).length() - 370.0).abs() < 1e-9);
    // Too much: no more than reaches below the line.
    assert_eq!(
        alter(&drawn, &[across(200.0, -250.0)]),
        Err(AlterError::TooShort { most: 200.0 })
    );
}

#[test]
fn switched_off_or_taken_away_the_piece_is_as_drawn() {
    let drawn = Drawn::of(&rectangle());
    let mut step = across(200.0, 30.0);
    step.on = false;
    assert_eq!(alter(&drawn, &[step]).unwrap(), drawn);
    assert_eq!(alter(&drawn, &[]).unwrap(), drawn);
    // Steps add up, in order.
    let both = alter(&drawn, &[across(100.0, 20.0), across(300.0, -10.0)]).unwrap();
    assert!((height(&both) - 410.0).abs() < 1e-9);
}

#[test]
fn a_line_given_either_way_round_moves_the_part_below() {
    let drawn = Drawn::of(&rectangle());
    let backwards = Alteration {
        change: Change::Lengthen {
            line: AlterLine::Through {
                from: Pt::new(210.0, 200.0),
                to: Pt::new(-10.0, 200.0),
            },
            by: 30.0,
        },
        ..across(200.0, 30.0)
    };
    assert_eq!(
        alter(&drawn, &[backwards]).unwrap(),
        alter(&drawn, &[across(200.0, 30.0)]).unwrap()
    );
}

#[test]
fn a_slanted_side_is_bridged_straight_or_jogs_along_the_line() {
    // A skirt panel: 200 wide at the top, 300 at the hem.
    let mut p = rectangle();
    p.outline.edges[1].segments = vec![Segment::line([300.0, 400.0])];
    let drawn = Drawn::of(&p);
    let longer = alter(&drawn, &[across(200.0, 30.0)]).unwrap();
    // The side: down to the line, straight down 30 mm, then on as it was.
    let side = &longer.outline.edges[1].segments;
    assert_eq!(side.len(), 3);
    assert_eq!(side[0].to(), Pt::new(250.0, 200.0));
    assert_eq!(side[1].to(), Pt::new(250.0, 230.0));
    assert_eq!(side[2].to(), Pt::new(300.0, 430.0));
    // Shortened, the part of the side in the tuck lies along the line: a
    // jog to true up.
    let shorter = alter(&drawn, &[across(200.0, -30.0)]).unwrap();
    let side = &shorter.outline.edges[1].segments;
    assert_eq!(side.len(), 3);
    assert_eq!(side[0].to(), Pt::new(250.0, 200.0));
    assert!(side[1].to().distance(Pt::new(257.5, 200.0)) < 1e-9);
    assert!(side[2].to().distance(Pt::new(300.0, 370.0)) < 1e-9);
}

#[test]
fn a_curved_side_keeps_its_curve_either_side_of_the_line() {
    let mut p = rectangle();
    p.outline.edges[1].segments = vec![Segment::cubic(
        [260.0, 100.0],
        [260.0, 300.0],
        [200.0, 400.0],
    )];
    let drawn = Drawn::of(&p);
    let longer = alter(&drawn, &[across(200.0, 30.0)]).unwrap();
    assert!((height(&longer) - 430.0).abs() < 1e-9);
    assert!(longer.outline.closure_gap() < 1e-9);
    // Cut at the line (where the curve is widest, 245 mm across), bridged,
    // and its lower half moved down whole.
    let side = longer.outline.edge_geom(1);
    let at = |along: f64| side.point_at(along);
    let before = drawn.outline.edge_geom(1);
    let half = before.length() / 2.0;
    assert!((side.length() - (before.length() + 30.0)).abs() < 1e-6);
    let p = at(half + 30.0 + 10.0);
    let q = before.point_at(half + 10.0);
    assert!((p.x - q.x).abs() < 1e-6 && (p.y - q.y - 30.0).abs() < 1e-6);
}

#[test]
fn along_the_pieces_own_lengthen_line() {
    let mut p = rectangle();
    p.lines.push(InternalLine {
        id: "ls".into(),
        kind: LineKind::LengthenShorten,
        start: Pt::new(0.0, 150.0),
        segments: vec![Segment::line([200.0, 150.0])],
        label: None,
    });
    let drawn = Drawn::of(&p);
    let step = Alteration {
        id: "a".into(),
        on: true,
        change: Change::Lengthen {
            line: AlterLine::Own { line: "ls".into() },
            by: 25.0,
        },
    };
    let longer = alter(&drawn, std::slice::from_ref(&step)).unwrap();
    assert!((height(&longer) - 425.0).abs() < 1e-9);
    // The line itself stays where it was.
    assert_eq!(longer.lines[0], drawn.lines[0]);
    // Gone, the step cannot be applied.
    p.lines.clear();
    assert_eq!(
        alter(&Drawn::of(&p), &[step]),
        Err(AlterError::NoLine { line: "ls".into() })
    );
}

#[test]
fn a_line_that_misses_the_piece_alters_nothing() {
    let drawn = Drawn::of(&rectangle());
    assert_eq!(
        alter(&drawn, &[across(500.0, 30.0)]),
        Err(AlterError::Misses)
    );
    let point = Alteration {
        change: Change::Lengthen {
            line: AlterLine::Through {
                from: Pt::new(5.0, 5.0),
                to: Pt::new(5.0, 5.0),
            },
            by: 10.0,
        },
        ..across(0.0, 0.0)
    };
    assert_eq!(alter(&drawn, &[point]), Err(AlterError::NoDirection));
}

/// The rectangle lengthened 30 mm at its middle, kept as drawn.
fn altered() -> Piece {
    let mut p = rectangle();
    let drawn = Drawn::of(&p);
    p.alterations = vec![across(200.0, 30.0)];
    alter(&drawn, &p.alterations).unwrap().put(&mut p);
    p.drawn = Some(Box::new(drawn));
    p
}

#[test]
fn new_ids_are_the_same_in_the_piece_and_as_drawn() {
    let mut p = altered();
    let mut n = 0;
    reassign_ids(&mut p, &mut |prefix| {
        n += 1;
        format!("{prefix}{n}")
    });
    let drawn = p.drawn.as_ref().unwrap();
    let ids = |o: &Outline| o.edges.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
    assert_eq!(ids(&p.outline), ids(&drawn.outline));
    assert_eq!(p.notches[0].id, drawn.notches[0].id);
    assert_eq!(p.notches[0].edge, drawn.notches[0].edge);
    assert_eq!(p.marks[0].id, drawn.marks[0].id);
    assert_ne!(p.alterations[0].id, "a");
    // Altered again from the renamed drawing: the same piece.
    assert_eq!(alter(drawn, &p.alterations).unwrap(), Drawn::of(&p));
}

#[test]
fn scaled_the_piece_as_drawn_and_its_steps_scale_alike() {
    let mut p = altered();
    transform(&mut p, Affine::scale(2.0));
    let drawn = p.drawn.as_ref().unwrap();
    assert!((drawn.outline.bounds().height() - 800.0).abs() < 1e-9);
    // The line scaled with it; the 30 mm stays 30 mm.
    let again = alter(drawn, &p.alterations).unwrap();
    assert!((height(&again) - 830.0).abs() < 1e-9);
    assert!((p.outline.bounds().height() - 860.0).abs() < 1e-9);
}
