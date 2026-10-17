//! Moving an edge, as dragged on the table: out or in, the edges either
//! side carried on the way they run (extended) or square to it (extruded),
//! or cut back along themselves.

use drapewright_pattern_core::{
    AlterError, Alteration, Change, Dart, Drawn, Edge, EdgeKind, EdgeMove, Notch, NotchFrom,
    NotchKind, Outline, Piece, Pt, Segment, alter,
};
use kurbo::{ParamCurve, ParamCurveNearest, Point};

/// An A-line panel: the waist 200 wide at the top, the hem 300 wide 400
/// below, its sides flaring 1 in 8. A notch on the right side 100 down
/// it, and one on the hem 50 from its end.
fn panel() -> Drawn {
    let mut p = Piece::new(
        "p",
        "Panel",
        Outline {
            start: Pt::new(50.0, 0.0),
            edges: vec![
                Edge::new("waist", vec![Segment::line([250.0, 0.0])]),
                Edge::new("right", vec![Segment::line([300.0, 400.0])]),
                Edge {
                    kind: EdgeKind::Hem,
                    ..Edge::new("hem", vec![Segment::line([0.0, 400.0])])
                },
                Edge::new("left", vec![Segment::line([50.0, 0.0])]),
            ],
        },
    );
    for (id, edge, at, from) in [
        ("n1", "right", 100.0, NotchFrom::Start),
        ("n2", "hem", 50.0, NotchFrom::End),
    ] {
        p.notches.push(Notch {
            id: id.into(),
            edge: edge.into(),
            at,
            from,
            kind: NotchKind::default(),
            angle: 0.0,
        });
    }
    Drawn::of(&p)
}

fn moved(shape: &Drawn, edge: &str, by: f64, mode: EdgeMove) -> Result<Drawn, AlterError> {
    let step = Alteration {
        id: "a".into(),
        on: true,
        change: Change::MoveEdge {
            edge: edge.into(),
            by,
            mode,
        },
    };
    alter(shape, &[step])
}

fn edge<'a>(shape: &'a Drawn, id: &str) -> &'a Edge {
    shape.outline.edges.iter().find(|e| e.id == id).unwrap()
}

fn ends(shape: &Drawn, id: &str) -> (Point, Point) {
    let i = shape.outline.edge_index(id).unwrap();
    (
        shape.outline.edge_start(i).into(),
        shape.outline.edge_end(i).into(),
    )
}

fn near(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-6
}

#[test]
fn extended_the_hem_moves_down_and_the_sides_carry_on_flaring() {
    let shape = moved(&panel(), "hem", 40.0, EdgeMove::Extend).unwrap();
    assert!(shape.outline.closure_gap() < 1e-9);
    // 40 lower, and 5 wider each side: the sides flare on as they did.
    let (a, b) = ends(&shape, "hem");
    assert!(near(a, Point::new(305.0, 440.0)), "{a:?}");
    assert!(near(b, Point::new(-5.0, 440.0)), "{b:?}");
    // Each side still one straight line, the hem still a hem.
    assert_eq!(edge(&shape, "right").segments.len(), 1);
    assert_eq!(edge(&shape, "left").segments.len(), 1);
    assert_eq!(edge(&shape, "hem").kind, EdgeKind::Hem);
    // The waist as it was; the notches where they were on the cloth.
    assert_eq!(edge(&shape, "waist"), edge(&panel(), "waist"));
    let notch = |id: &str| shape.notches.iter().find(|n| n.id == id).unwrap().at;
    assert!((notch("n1") - 100.0).abs() < 1e-6);
    assert!((notch("n2") - 55.0).abs() < 1e-6, "{}", notch("n2"));
}

#[test]
fn extruded_the_hem_adds_a_strip_with_square_sides() {
    let shape = moved(&panel(), "hem", 40.0, EdgeMove::Extrude).unwrap();
    let (a, b) = ends(&shape, "hem");
    assert!(near(a, Point::new(300.0, 440.0)), "{a:?}");
    assert!(near(b, Point::new(0.0, 440.0)), "{b:?}");
    // Each side carries on straight down, square to the hem, from where
    // it ended.
    let right = edge(&shape, "right");
    assert_eq!(right.segments.len(), 2);
    assert!(near(
        right.segments[0].to().into(),
        Point::new(300.0, 400.0)
    ));
    let left = edge(&shape, "left");
    assert_eq!(left.segments.len(), 2);
    assert!(near(left.segments[0].to().into(), Point::new(0.0, 400.0)));
}

#[test]
fn moved_in_the_sides_are_cut_back_either_way() {
    let extended = moved(&panel(), "hem", -40.0, EdgeMove::Extend).unwrap();
    let extruded = moved(&panel(), "hem", -40.0, EdgeMove::Extrude).unwrap();
    assert_eq!(extended, extruded);
    let (a, b) = ends(&extended, "hem");
    assert!(near(a, Point::new(295.0, 360.0)), "{a:?}");
    assert!(near(b, Point::new(5.0, 360.0)), "{b:?}");
    assert_eq!(edge(&extended, "right").segments.len(), 1);
}

#[test]
fn a_side_moved_out_lets_the_piece_out_there() {
    // The right side 10 out, square to itself: the waist and hem carry on
    // across to meet it.
    let shape = moved(&panel(), "right", 10.0, EdgeMove::Extend).unwrap();
    let (a, b) = ends(&shape, "right");
    let out = 10.0 * (1.0f64 + 1.0 / 64.0).sqrt();
    assert!(near(a, Point::new(250.0 + out, 0.0)), "{a:?}");
    assert!(near(b, Point::new(300.0 + out, 400.0)), "{b:?}");
}

#[test]
fn a_fold_carries_on_along_itself_to_stay_straight() {
    // Cut on the fold down its left side, the hem sloping.
    let p = Piece::new(
        "p",
        "Front",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("waist", vec![Segment::line([200.0, 0.0])]),
                Edge::new("side", vec![Segment::line([250.0, 420.0])]),
                Edge::new("hem", vec![Segment::line([0.0, 400.0])]),
                Edge {
                    kind: EdgeKind::Fold,
                    ..Edge::new("fold", vec![Segment::line([0.0, 0.0])])
                },
            ],
        },
    );
    let shape = moved(&Drawn::of(&p), "hem", 30.0, EdgeMove::Extrude).unwrap();
    let fold = edge(&shape, "fold");
    assert_eq!(fold.segments.len(), 1, "{fold:?}");
    let (start, _) = ends(&shape, "fold");
    assert!(start.x.abs() < 1e-9, "{start:?}");
    // The side, not a fold, carries on square to the hem.
    assert_eq!(edge(&shape, "side").segments.len(), 2);
}

#[test]
fn a_curved_hem_moves_as_a_parallel_curve() {
    let p = Piece::new(
        "p",
        "Circle skirt quarter",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("waist", vec![Segment::line([100.0, 0.0])]),
                Edge::new("side", vec![Segment::line([300.0, 300.0])]),
                Edge::new(
                    "hem",
                    vec![Segment::cubic([200.0, 400.0], [80.0, 420.0], [0.0, 400.0])],
                ),
                Edge::new("centre", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    let drawn = Drawn::of(&p);
    let shape = moved(&drawn, "hem", 30.0, EdgeMove::Extend).unwrap();
    let before = drawn.outline.edge_geom(2);
    let after = shape.outline.edge_geom(2);
    // Away from its ends, where the sides meet it, every point 30 out.
    let curve = before.segs()[0];
    for k in 1..10 {
        let p = after.point_at(after.length() * f64::from(k) / 10.0);
        let d = curve.nearest(p, 1e-9).distance_sq.sqrt();
        assert!((d - 30.0).abs() < 0.05, "{k}: {d}");
    }
    // The centre carries on straight down along itself.
    let (_, end) = ends(&shape, "hem");
    assert!(end.x.abs() < 1e-6, "{end:?}");
    assert!(
        after
            .segs()
            .iter()
            .all(|s| s.start().distance(s.end()) > 1e-6)
    );
}

#[test]
fn a_dart_in_the_edge_keeps_its_tip_and_its_legs_carry_on() {
    let leg = (10.0f64.powi(2) + 100.0f64.powi(2)).sqrt();
    let mut p = Piece::new(
        "p",
        "Skirt front",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new(
                    "waist",
                    vec![
                        Segment::line([90.0, 0.0]),
                        Segment::line([100.0, 100.0]),
                        Segment::line([110.0, 0.0]),
                        Segment::line([200.0, 0.0]),
                    ],
                ),
                Edge::new("side", vec![Segment::line([220.0, 500.0])]),
                Edge::new("hem", vec![Segment::line([0.0, 500.0])]),
                Edge::new("centre", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    p.darts.push(Dart {
        id: "d".into(),
        edge: "waist".into(),
        from: 90.0,
        to: 90.0 + 2.0 * leg,
    });
    let shape = moved(&Drawn::of(&p), "waist", 20.0, EdgeMove::Extend).unwrap();
    assert!(shape.outline.closure_gap() < 1e-9);
    let waist = shape.outline.edge_geom(0);
    let dart = &shape.darts[0];
    // Its legs carried on up to the waist 20 higher, the tip where it was.
    let longer = (12.0f64.powi(2) + 120.0f64.powi(2)).sqrt();
    assert!((dart.from - 88.0).abs() < 1e-6, "{dart:?}");
    assert!(
        (dart.to - dart.from - 2.0 * longer).abs() < 1e-6,
        "{dart:?}"
    );
    assert!(waist.point_at(dart.from).distance(Point::new(88.0, -20.0)) < 1e-6);
    assert!(waist.point_at(dart.to).distance(Point::new(112.0, -20.0)) < 1e-6);
    let tip = waist.point_at((dart.from + dart.to) / 2.0);
    assert!(tip.distance(Point::new(100.0, 100.0)) < 1e-6, "{tip:?}");
    let (start, end) = ends(&shape, "waist");
    assert!(near(start, Point::new(0.0, -20.0)), "{start:?}");
    assert!(near(end, Point::new(199.2, -20.0)), "{end:?}");
}

#[test]
fn moved_too_far_or_an_edge_not_there_is_refused() {
    let rectangle = Piece::new(
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
    let drawn = Drawn::of(&rectangle);
    // In further than the sides are long.
    assert_eq!(
        moved(&drawn, "bottom", -450.0, EdgeMove::Extend),
        Err(AlterError::NoMeeting)
    );
    assert_eq!(
        moved(&drawn, "gone", 10.0, EdgeMove::Extend),
        Err(AlterError::NoEdge {
            edge: "gone".into()
        })
    );
    // Not moved at all: as it was.
    assert_eq!(moved(&drawn, "bottom", 0.0, EdgeMove::Extrude), Ok(drawn));
}

#[test]
fn moved_and_moved_back_the_piece_is_as_it_was() {
    let drawn = panel();
    let out = moved(&drawn, "hem", 25.0, EdgeMove::Extend).unwrap();
    let back = moved(&out, "hem", -25.0, EdgeMove::Extend).unwrap();
    for (a, b) in back.outline.edges.iter().zip(&drawn.outline.edges) {
        for (s, t) in a.segments.iter().zip(&b.segments) {
            assert!(Point::from(s.to()).distance(t.to().into()) < 1e-6);
        }
    }
}

#[test]
fn on_a_piece_of_two_edges_the_other_carries_on_at_both_ends() {
    // A half circle of radius 100: straight across the top, round below.
    let k = 100.0 * 0.552_284_75;
    let p = Piece::new(
        "p",
        "Half circle",
        Outline {
            start: Pt::new(-100.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([100.0, 0.0])]),
                Edge::new(
                    "round",
                    vec![
                        Segment::cubic([100.0, k], [k, 100.0], [0.0, 100.0]),
                        Segment::cubic([-k, 100.0], [-100.0, k], [-100.0, 0.0]),
                    ],
                ),
            ],
        },
    );
    let shape = moved(&Drawn::of(&p), "top", 15.0, EdgeMove::Extend).unwrap();
    assert!(shape.outline.closure_gap() < 1e-9);
    let (a, b) = ends(&shape, "top");
    assert!(near(a, Point::new(-100.0, -15.0)), "{a:?}");
    assert!(near(b, Point::new(100.0, -15.0)), "{b:?}");
    // The round carried on straight up at both its ends.
    let round = edge(&shape, "round");
    assert_eq!(round.segments.len(), 4, "{round:?}");
}
