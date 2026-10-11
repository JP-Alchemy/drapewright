//! Seam allowance: exact cases, sewing corner constructions, and
//! properties over random pieces.

use drapewright_pattern_core::allowance::{AllowanceNote, FLATTEN_TOLERANCE, allowance_lines};
use drapewright_pattern_core::geom::flatten_segs;
use drapewright_pattern_core::testing::{arb_gentle_piece, arb_piece};
use drapewright_pattern_core::validate::self_intersects;
use drapewright_pattern_core::{
    AllowanceMode, CornerStyle, Edge, EdgeKind, Outline, Piece, Pt, Segment,
};
use kurbo::{Point, Vec2};
use proptest::prelude::*;

fn polygon_area(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].to_vec2().cross(pts[(i + 1) % n].to_vec2()))
        .sum::<f64>()
        / 2.0
}

fn distance_to_polyline(p: Point, pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % n]);
            let ab = b - a;
            let t = ((p - a).dot(ab) / ab.hypot2()).clamp(0.0, 1.0);
            p.distance(a + ab * t)
        })
        .fold(f64::INFINITY, f64::min)
}

/// Winding number of a closed polyline around `p` (non-zero: inside).
fn winding(p: Point, pts: &[Point]) -> i32 {
    let n = pts.len();
    let mut w = 0;
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let cross = (b - a).cross(p - a);
        if a.y <= p.y {
            if b.y > p.y && cross > 0.0 {
                w += 1;
            }
        } else if b.y <= p.y && cross < 0.0 {
            w -= 1;
        }
    }
    w
}

/// Inside, or on the line. With no allowance the cutting line is the curve
/// itself, and a point on the curve can lie just outside the polyline
/// standing for it: the offset curve's fit and the flattening each allow
/// `FLATTEN_TOLERANCE`. 0.01 mm is far below anything a printer shows.
fn inside_or_on(p: Point, pts: &[Point]) -> bool {
    winding(p, pts) != 0 || distance_to_polyline(p, pts) <= 5.0 * FLATTEN_TOLERANCE
}

fn has_vertex_near(pts: &[Point], q: Point, tol: f64) -> bool {
    pts.iter().any(|p| p.distance(q) < tol)
}

fn rect(w: f64, h: f64) -> Piece {
    let outline = Outline {
        start: Pt::new(0.0, 0.0),
        edges: vec![
            Edge::new("top", vec![Segment::line([w, 0.0])]),
            Edge::new("right", vec![Segment::line([w, h])]),
            Edge::new("bottom", vec![Segment::line([0.0, h])]),
            Edge::new("left", vec![Segment::line([0.0, 0.0])]),
        ],
    };
    let mut p = Piece::new("p1", "Rectangle", outline);
    p.allowance.seam = 10.0;
    p
}

#[test]
fn rectangle_with_even_allowance() {
    let lines = allowance_lines(&rect(200.0, 100.0));
    assert!((polygon_area(&lines.cutting) - 220.0 * 120.0).abs() < 1e-6);
    for corner in [
        (-10.0, -10.0),
        (210.0, -10.0),
        (210.0, 110.0),
        (-10.0, 110.0),
    ] {
        assert!(
            has_vertex_near(&lines.cutting, Point::new(corner.0, corner.1), 1e-6),
            "{corner:?}"
        );
    }
    assert!(lines.notes.is_empty());
}

#[test]
fn each_edge_gets_its_own_allowance() {
    let mut p = rect(200.0, 100.0);
    p.outline.edges[0].allowance = Some(20.0); // top
    p.outline.edges[2].allowance = Some(0.0); // bottom
    let lines = allowance_lines(&p);
    for corner in [
        (-10.0, -20.0),
        (210.0, -20.0),
        (210.0, 100.0),
        (-10.0, 100.0),
    ] {
        assert!(
            has_vertex_near(&lines.cutting, Point::new(corner.0, corner.1), 1e-6),
            "{corner:?}"
        );
    }
    assert!((polygon_area(&lines.cutting) - 220.0 * 120.0).abs() < 1e-6);
}

#[test]
fn fold_edges_get_no_allowance_and_meet_the_fold_line() {
    let mut p = rect(200.0, 100.0);
    p.outline.edges[3].kind = EdgeKind::Fold; // left
    let lines = allowance_lines(&p);
    for corner in [(0.0, -10.0), (210.0, -10.0), (210.0, 110.0), (0.0, 110.0)] {
        assert!(
            has_vertex_near(&lines.cutting, Point::new(corner.0, corner.1), 1e-6),
            "{corner:?}"
        );
    }
}

#[test]
fn hems_fold_back_to_match_a_flared_side_seam() {
    // A flared skirt panel: waist, side seam flaring out, straight hem,
    // centre front on the fold. Clockwise with y down.
    let outline = Outline {
        start: Pt::new(0.0, 0.0),
        edges: vec![
            Edge::new("waist", vec![Segment::line([200.0, 0.0])]),
            Edge::new("side", vec![Segment::line([300.0, 600.0])]),
            Edge {
                kind: EdgeKind::Hem,
                ..Edge::new("hem", vec![Segment::line([0.0, 600.0])])
            },
            Edge {
                kind: EdgeKind::Fold,
                ..Edge::new("cf", vec![Segment::line([0.0, 0.0])])
            },
        ],
    };
    let mut p = Piece::new("p1", "Skirt", outline);
    p.allowance.seam = 15.0;
    p.allowance.hem = Some(40.0);
    let lines = allowance_lines(&p);

    // The side seam's cutting line is parallel to the seam, 15 mm out.
    let side = Vec2::new(100.0, 600.0).normalize();
    let n = Vec2::new(side.y, -side.x);
    let s_line = |t: f64| Point::new(200.0, 0.0) + side * t + n * 15.0;
    // It crosses the hem line (y = 600) at P.
    let t_p = (600.0 - s_line(0.0).y) / side.y;
    let p_pt = s_line(t_p);
    // Folded back, the hem allowance's side follows the mirror image of the
    // side seam's cutting line, up to the hem's cutting line at y = 640.
    let mirrored = Vec2::new(side.x, -side.y);
    let r = p_pt - mirrored * (40.0 / side.y);
    assert!(has_vertex_near(&lines.cutting, p_pt, 1e-3), "P {p_pt:?}");
    assert!(has_vertex_near(&lines.cutting, r, 1e-3), "R {r:?}");
    // The hem allowance angles back in: R is inside P horizontally.
    assert!(r.x < p_pt.x);
    // At the fold, the hem's cutting line meets the fold line.
    assert!(has_vertex_near(
        &lines.cutting,
        Point::new(0.0, 640.0),
        1e-6
    ));
    assert!(lines.notes.is_empty(), "{:?}", lines.notes);
}

#[test]
fn deep_hem_on_a_wide_flare_is_trimmed_not_widened() {
    // 20° flare with a 50 mm hem: the folded-back end must be narrower
    // than the hem band, so the band is trimmed at the corner.
    let flare = 20f64.to_radians();
    let bottom = (600.0 * flare.tan() + 200.0, 600.0);
    let outline = Outline {
        start: Pt::new(0.0, 0.0),
        edges: vec![
            Edge::new("waist", vec![Segment::line([200.0, 0.0])]),
            Edge::new("side", vec![Segment::line([bottom.0, bottom.1])]),
            Edge {
                kind: EdgeKind::Hem,
                ..Edge::new("hem", vec![Segment::line([0.0, 600.0])])
            },
            Edge::new("cf", vec![Segment::line([0.0, 0.0])]),
        ],
    };
    let mut p = Piece::new("p1", "Wide skirt", outline);
    p.allowance.seam = 15.0;
    p.allowance.hem = Some(50.0);
    let lines = allowance_lines(&p);
    // The hem's cutting line at its side end lies inside the side seam
    // line extended straight down (x of the seam corner).
    let corner_x = bottom.0;
    let hem_cut_end = lines
        .cutting
        .iter()
        .filter(|q| (q.y - 650.0).abs() < 1e-6)
        .map(|q| q.x)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(hem_cut_end < corner_x, "{hem_cut_end} vs {corner_x}");
    assert!(!self_intersects(&lines.cutting));
}

#[test]
fn sharp_points_are_cut_square_at_the_mitre_limit() {
    // A long thin triangle with a 10° point.
    let half = 5f64.to_radians();
    let len = 300.0;
    let outline = Outline {
        start: Pt::new(0.0, 0.0),
        edges: vec![
            Edge::new(
                "a",
                vec![Segment::line([len * half.cos(), len * half.sin()])],
            ),
            Edge::new(
                "b",
                vec![Segment::line([len * half.cos(), -len * half.sin()])],
            ),
            Edge::new("c", vec![Segment::line([0.0, 0.0])]),
        ],
    };
    // Make it clockwise if needed.
    let mut p = Piece::new("p1", "Point", outline);
    drapewright_pattern_core::ops::normalize_orientation(&mut p);
    p.allowance.seam = 10.0;
    let lines = allowance_lines(&p);
    // The tip is at the origin and points along -x, its bisector: the cut
    // across the point sits 4 × 10 mm out along it.
    let reach = lines.cutting.iter().map(|q| -q.x).fold(0.0, f64::max);
    assert!((reach - 40.0).abs() < 1e-6, "point reaches {reach} mm");
}

#[test]
fn squared_corner_cuts_off_the_point() {
    // Right angle plus a 30° acute corner where squaring matters.
    let outline = Outline {
        start: Pt::new(0.0, 0.0),
        edges: vec![
            Edge::new("a", vec![Segment::line([200.0, 0.0])]),
            Edge {
                end_corner: CornerStyle::SquareThis,
                ..Edge::new("b", vec![Segment::line([0.0, 115.47])])
            },
            Edge::new("c", vec![Segment::line([0.0, 0.0])]),
        ],
    };
    let mut p = Piece::new("p1", "Wedge", outline);
    p.allowance.seam = 10.0;
    let mitre = {
        let mut q = p.clone();
        q.outline.edges[1].end_corner = CornerStyle::Mitre;
        allowance_lines(&q)
    };
    let squared = allowance_lines(&p);
    assert!(polygon_area(&squared.cutting) < polygon_area(&mitre.cutting));
    assert!(squared.notes.is_empty(), "{:?}", squared.notes);
}

#[test]
fn circle_allowance_is_a_bigger_circle() {
    let k = 0.552_284_749_830_793_4 * 100.0;
    let outline = Outline {
        start: Pt::new(100.0, 0.0),
        edges: vec![Edge::new(
            "round",
            vec![
                Segment::cubic([100.0, k], [k, 100.0], [0.0, 100.0]),
                Segment::cubic([-k, 100.0], [-100.0, k], [-100.0, 0.0]),
                Segment::cubic([-100.0, -k], [-k, -100.0], [0.0, -100.0]),
                Segment::cubic([k, -100.0], [100.0, -k], [100.0, 0.0]),
            ],
        )],
    };
    let mut p = Piece::new("p1", "Circle", outline);
    p.allowance.seam = 15.0;
    let lines = allowance_lines(&p);
    // The cubic circle is within 0.03 mm of radius 100; its offset should
    // be within that plus the flattening tolerance of radius 115.
    for q in &lines.cutting {
        let r = q.distance(Point::ZERO);
        assert!((r - 115.0).abs() < 0.03, "{r}");
    }
}

#[test]
fn tight_inward_curve_never_crosses_itself() {
    // A U-shaped notch of radius 8 mm with 15 mm allowance: the naive offset
    // of the curve would loop back on itself.
    let k = 0.552_284_749_830_793_4 * 8.0;
    let outline = Outline {
        start: Pt::new(0.0, 0.0),
        edges: vec![
            Edge::new("top-left", vec![Segment::line([92.0, 0.0])]),
            Edge::new(
                "notch",
                vec![
                    Segment::cubic([92.0, k], [100.0 - k, 8.0], [100.0, 8.0]),
                    Segment::cubic([100.0 + k, 8.0], [108.0, k], [108.0, 0.0]),
                ],
            ),
            Edge::new("top-right", vec![Segment::line([200.0, 0.0])]),
            Edge::new(
                "rest",
                vec![
                    Segment::line([200.0, 100.0]),
                    Segment::line([0.0, 100.0]),
                    Segment::line([0.0, 0.0]),
                ],
            ),
        ],
    };
    // The notch dips into the piece: the outline is still clockwise.
    let mut p = Piece::new("p1", "Notched", outline);
    p.allowance.seam = 15.0;
    assert!(p.outline.signed_area() > 0.0);
    let lines = allowance_lines(&p);
    assert!(!self_intersects(&lines.cutting));
    let outline_pts = flatten_segs(&p.outline.path_segs(), 0.01);
    assert!(outline_pts.iter().all(|q| inside_or_on(*q, &lines.cutting)));
    // Over the notch the cutting line runs straight at y = -15.
    assert!(distance_to_polyline(Point::new(100.0, -15.0), &lines.cutting) < 1e-6);
}

#[test]
fn included_allowance_gives_the_stitching_line_inside() {
    let mut p = rect(200.0, 100.0);
    p.allowance.mode = AllowanceMode::Included;
    let lines = allowance_lines(&p);
    assert!((polygon_area(&lines.stitching) - 180.0 * 80.0).abs() < 1e-6);
    assert!(has_vertex_near(
        &lines.stitching,
        Point::new(10.0, 10.0),
        1e-6
    ));
    assert!((polygon_area(&lines.cutting) - 200.0 * 100.0).abs() < 1e-6);
}

#[test]
fn included_allowance_wider_than_the_piece_leaves_nothing() {
    let mut p = rect(20.0, 20.0);
    p.allowance.mode = AllowanceMode::Included;
    p.allowance.seam = 15.0;
    let lines = allowance_lines(&p);
    assert!(lines.stitching.is_empty());
    assert_eq!(lines.notes, [AllowanceNote::NothingLeft]);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Whatever the allowances and corner styles, the cut piece contains the
    /// whole stitched piece and its outline never crosses itself.
    #[test]
    fn cutting_line_encloses_the_piece(piece in arb_piece()) {
        let lines = allowance_lines(&piece);
        prop_assert!(lines.cutting.len() >= 3);
        prop_assert!(!self_intersects(&lines.cutting));
        for q in flatten_segs(&piece.outline.path_segs(), 0.05) {
            prop_assert!(inside_or_on(q, &lines.cutting), "{q:?} outside");
        }
        prop_assert!(polygon_area(&lines.cutting) >= piece.outline.signed_area() - 1e-3);
    }

    /// Away from corners, the cutting line is exactly the allowance away
    /// from the stitching line.
    #[test]
    fn cutting_line_is_the_allowance_away(mut piece in arb_gentle_piece(), width in 1.0f64..25.0) {
        piece.allowance.seam = width;
        for e in &mut piece.outline.edges {
            e.kind = EdgeKind::Seam;
            e.allowance = None;
            e.end_corner = CornerStyle::Auto;
        }
        let lines = allowance_lines(&piece);
        let outline_pts = flatten_segs(&piece.outline.path_segs(), 0.001);
        for i in 0..piece.outline.edges.len() {
            let geom = piece.outline.edge_geom(i);
            let len = geom.length();
            let margin = 5.0 * width;
            if len < 2.0 * margin {
                continue;
            }
            for s in 0..=8 {
                let along = margin + (len - 2.0 * margin) * s as f64 / 8.0;
                let o = geom.point_at(along) + geom.normal_at(along) * width;
                // Only where no other part of the outline is closer.
                if distance_to_polyline(o, &outline_pts) < width - 1e-3 {
                    continue;
                }
                let d = distance_to_polyline(o, &lines.cutting);
                prop_assert!(d < 0.005, "edge {i} at {along:.1}: {d} mm off");
            }
        }
    }

    /// A piece with no allowance is cut on its stitching line.
    #[test]
    fn zero_allowance_cuts_on_the_line(mut piece in arb_piece()) {
        piece.allowance.seam = 0.0;
        piece.allowance.hem = None;
        for e in &mut piece.outline.edges {
            e.allowance = None;
        }
        let lines = allowance_lines(&piece);
        let area = polygon_area(&lines.cutting);
        prop_assert!((area - piece.outline.signed_area()).abs() < 0.05 * piece.outline.length(), "{area}");
    }

    /// With allowance included, the stitching line lies inside the cutting
    /// line.
    #[test]
    fn included_stitching_line_is_inside(mut piece in arb_piece()) {
        piece.allowance.mode = AllowanceMode::Included;
        let lines = allowance_lines(&piece);
        for q in &lines.stitching {
            prop_assert!(inside_or_on(*q, &lines.cutting));
        }
    }
}
