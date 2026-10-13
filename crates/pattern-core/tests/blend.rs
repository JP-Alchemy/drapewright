//! A piece blended between its sizes by height on the figure.

use drapewright_pattern_core::blend::{Height, Sized, blend};
use drapewright_pattern_core::geom::flatten_segs;
use drapewright_pattern_core::{
    Edge, EdgeKind, InternalLine, LineKind, Notch, NotchFrom, NotchKind, Outline, Piece, Pt,
    Segment,
};

/// Worn with its top 1000 mm up the figure, y down the piece.
const WORN: Height = Height {
    up: Pt::new(0.0, -1.0),
    at: 1000.0,
};

/// Size 0 at 950 mm up and above, size 1 at 650 and below, blended
/// smoothly between.
fn index(h: f64) -> f64 {
    let s = ((950.0 - h) / 300.0).clamp(0.0, 1.0);
    s * s * (3.0 - 2.0 * s)
}

/// A front panel `width` wide, 400 long, cut on the fold down its left side
/// (at `x`), its hem curved; clockwise on screen from its top left.
fn panel(width: f64, x: f64) -> Piece {
    let at = |p: [f64; 2]| [p[0] + x, p[1]];
    let mut fold = Edge::new("fold", vec![Segment::line(at([0.0, 0.0]))]);
    fold.kind = EdgeKind::Fold;
    let edges = vec![
        Edge::new("top", vec![Segment::line(at([width, 0.0]))]),
        Edge::new("side", vec![Segment::line(at([width, 400.0]))]),
        Edge::new(
            "hem",
            vec![Segment::cubic(
                at([width * 0.66, 410.0]),
                at([width * 0.33, 410.0]),
                at([0.0, 400.0]),
            )],
        ),
        fold,
    ];
    let mut piece = Piece::new(
        "front",
        "Front",
        Outline {
            start: at([0.0, 0.0]).into(),
            edges,
        },
    );
    piece.notches.push(Notch {
        id: "n".into(),
        edge: "side".into(),
        at: 300.0,
        from: NotchFrom::Start,
        kind: NotchKind::Single,
        angle: 0.0,
    });
    piece.lines.push(InternalLine {
        id: "waist".into(),
        kind: LineKind::Line,
        start: at([0.0, 200.0]).into(),
        segments: vec![Segment::line(at([width, 200.0]))],
        label: Some("Waist".into()),
    });
    piece
}

fn outline_points(piece: &Piece) -> Vec<kurbo::Point> {
    flatten_segs(&piece.outline.path_segs(), 0.01)
}

/// Where the outline crosses height `y` on the piece: its leftmost and
/// rightmost crossing.
fn across(piece: &Piece, y: f64) -> (f64, f64) {
    let points = outline_points(piece);
    let mut xs = Vec::new();
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a.y - y) * (b.y - y) <= 0.0 && (a.y - b.y).abs() > 1e-9 {
            xs.push(a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y));
        }
    }
    let low = xs.iter().copied().fold(f64::MAX, f64::min);
    let high = xs.iter().copied().fold(f64::MIN, f64::max);
    (low, high)
}

/// The furthest any point of one outline is from the other.
fn apart(a: &Piece, b: &Piece) -> f64 {
    let (pa, pb) = (outline_points(a), outline_points(b));
    let near = |p: kurbo::Point, among: &[kurbo::Point]| {
        among
            .windows(2)
            .map(|w| {
                let line = kurbo::Line::new(w[0], w[1]);
                kurbo::ParamCurveNearest::nearest(&line, p, 1e-9)
                    .distance_sq
                    .sqrt()
            })
            .fold(f64::MAX, f64::min)
    };
    let one = pa.iter().map(|&p| near(p, &pb)).fold(0.0, f64::max);
    let other = pb.iter().map(|&p| near(p, &pa)).fold(0.0, f64::max);
    one.max(other)
}

fn sized(piece: &Piece) -> Sized<'_> {
    Sized {
        piece,
        height: Some(WORN),
    }
}

#[test]
fn a_piece_blended_with_itself_is_as_drawn() {
    let front = panel(200.0, 0.0);
    let blended = blend(&[sized(&front)], 0, |_| 0.0).unwrap();
    assert!(
        apart(&blended, &front) < 0.01,
        "{}",
        apart(&blended, &front)
    );
    // Its edges kept, the hem still one curve.
    let ids: Vec<&str> = blended
        .outline
        .edges
        .iter()
        .map(|e| e.id.as_str())
        .collect();
    assert_eq!(ids, ["top", "side", "hem", "fold"]);
    assert!(matches!(
        blended.outline.edges[2].segments[..],
        [Segment::Cubic(_)]
    ));
    assert!((blended.notches[0].at - 300.0).abs() < 1e-9);
}

#[test]
fn each_size_is_cut_at_its_height_and_blended_between() {
    // The larger size drawn beside the smaller, as a CAD file lays them out.
    let (small, large) = (panel(200.0, 0.0), panel(240.0, 500.0));
    let blended = blend(&[sized(&small), sized(&large)], 0, index).unwrap();
    let width = |y: f64| {
        let (l, r) = across(&blended, y);
        (l, r - l)
    };
    // The small size at 950 mm up and above, the large at 650 and below.
    assert!((width(30.0).1 - 200.0).abs() < 0.1, "{:?}", width(30.0));
    assert!((width(50.0).1 - 200.0).abs() < 0.1, "{:?}", width(50.0));
    assert!((width(350.0).1 - 240.0).abs() < 0.1, "{:?}", width(350.0));
    // Halfway between, halfway between; and growing all the way.
    assert!((width(200.0).1 - 220.0).abs() < 0.1, "{:?}", width(200.0));
    let widths: Vec<f64> = (5..80).map(|k| width(k as f64 * 5.0).1).collect();
    assert!(widths.windows(2).all(|w| w[1] >= w[0] - 1e-6), "{widths:?}");
    // The fold stays where it was, straight.
    for y in [10.0, 100.0, 200.0, 300.0, 390.0] {
        assert!(width(y).0.abs() < 1e-6, "{y}: {:?}", width(y));
    }
}

#[test]
fn sizes_drawn_with_other_corners_are_matched_by_shape() {
    let (small, large) = (panel(200.0, 0.0), panel(240.0, 500.0));
    let expected = blend(&[sized(&small), sized(&large)], 0, index).unwrap();
    // The large size drawn from its bottom left, its side in two edges.
    let mut redrawn = large.clone();
    let mut edges = redrawn.outline.edges.clone();
    let side = edges.remove(1);
    let mut upper = side.clone();
    upper.id = "upper".into();
    upper.segments = vec![Segment::line([740.0, 150.0])];
    let mut lower = side;
    lower.id = "lower".into();
    edges.insert(1, upper);
    edges.insert(2, lower);
    edges.rotate_left(4);
    redrawn.outline = Outline {
        start: [500.0, 400.0].into(),
        edges,
    };
    let blended = blend(&[sized(&small), sized(&redrawn)], 0, index).unwrap();
    assert!(
        apart(&blended, &expected) < 0.05,
        "{}",
        apart(&blended, &expected)
    );
}

#[test]
fn nested_sizes_are_blended_where_they_are_drawn() {
    // Drawn over each other, the larger growing at its side only: the fold
    // shared.
    let (small, large) = (panel(200.0, 0.0), panel(240.0, 0.0));
    let blended = blend(&[sized(&small), sized(&large)], 0, index).unwrap();
    assert!((across(&blended, 350.0).1 - 240.0).abs() < 0.1);
    assert!(across(&blended, 350.0).0.abs() < 1e-6);
}

#[test]
fn notches_and_lines_move_with_the_outline() {
    let (small, large) = (panel(200.0, 0.0), panel(240.0, 500.0));
    let blended = blend(&[sized(&small), sized(&large)], 0, index).unwrap();
    // The waist line, halfway down, reaches the blended side.
    let waist = &blended.lines[0];
    let Segment::Line(end) = waist.segments[0] else {
        panic!("{waist:?}")
    };
    assert!(
        waist.start.x.abs() < 0.5 && (end.to.x - 220.0).abs() < 0.5,
        "{waist:?}"
    );
    // The notch 300 mm down the side is still at that height, on it.
    let side = blended.outline.edge_index("side").unwrap();
    let notch = &blended.notches[0];
    let at = blended.outline.edge_geom(side).point_at(notch.at);
    assert!((at.y - 300.0).abs() < 0.5, "{at:?}");
}

#[test]
fn a_piece_not_worn_round_the_body_is_not_blended() {
    let front = panel(200.0, 0.0);
    let unworn = Sized {
        piece: &front,
        height: None,
    };
    assert!(blend(&[unworn], 0, |_| 0.0).is_none());
}
