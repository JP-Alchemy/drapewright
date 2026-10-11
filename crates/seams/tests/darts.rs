//! Seams along edges with darts in them, measured as sewn: darts closed.

use drapewright_pattern_core::{
    Dart, Edge, EdgeKind, Notch, NotchFrom, NotchKind, Outline, Piece, Pt, Segment,
};
use drapewright_seams::{Reason, Seam, SeamSide, measure, suggest};

/// A front 200 by 400 whose side has a dart opening 40 mm wide at 180–220
/// down, its tip 80 mm in; and a notch on the side 300 down.
fn front() -> Piece {
    let mut p = Piece::new(
        "front",
        "Front",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([200.0, 0.0])]),
                Edge::new(
                    "side",
                    vec![
                        Segment::line([200.0, 180.0]),
                        Segment::line([120.0, 200.0]),
                        Segment::line([200.0, 220.0]),
                        Segment::line([200.0, 400.0]),
                    ],
                ),
                Edge::new("hem", vec![Segment::line([0.0, 400.0])]),
                Edge::new("centre", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    let leg = 80f64.hypot(20.0);
    p.darts.push(Dart {
        id: "d".into(),
        edge: "side".into(),
        from: 180.0,
        to: 180.0 + 2.0 * leg,
    });
    p.notches.push(Notch {
        id: "n".into(),
        edge: "side".into(),
        at: 180.0 + 2.0 * leg + 80.0,
        from: NotchFrom::Start,
        kind: NotchKind::default(),
        angle: 0.0,
    });
    p
}

/// A back as long as the front's side is with its dart closed, notched 260
/// down.
fn back() -> Piece {
    let mut p = Piece::new(
        "back",
        "Back",
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([200.0, 0.0])]),
                Edge::new("side", vec![Segment::line([200.0, 360.0])]),
                Edge::new("hem", vec![Segment::line([0.0, 360.0])]),
                Edge::new("centre", vec![Segment::line([0.0, 0.0])]),
            ],
        },
    );
    p.notches.push(Notch {
        id: "n".into(),
        edge: "side".into(),
        at: 260.0,
        from: NotchFrom::Start,
        kind: NotchKind::default(),
        angle: 0.0,
    });
    p
}

#[test]
fn a_side_with_a_dart_is_measured_with_the_dart_closed() {
    let seam = Seam {
        id: "s".into(),
        name: "Side".into(),
        a: SeamSide::whole("front", "side"),
        b: SeamSide::whole("back", "side"),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    };
    let report = measure(&seam, &[front(), back()]);
    // 180 above the dart and 180 below it: 360, as long as the back.
    assert!((report.a_length - 360.0).abs() < 1e-6, "{report:?}");
    assert!(report.difference.abs() < 1e-6);
    // The notches meet: 260 down the closed side, 260 down the back.
    let meeting = &report.notches[0];
    assert!(meeting.apart.unwrap().abs() < 1e-6, "{meeting:?}");
}

#[test]
fn a_side_with_a_dart_is_suggested_for_a_side_as_long_as_it_closed() {
    // Hemmed: the sides run from the tops to the hems.
    let hemmed = |mut p: Piece| {
        p.outline.edges[2].kind = EdgeKind::Hem;
        p
    };
    let suggestions = suggest(&[hemmed(front()), hemmed(back())], &[]);
    let side = suggestions
        .iter()
        .find(|s| s.seam.a.edge == "side" && s.seam.b.edge == "side")
        .expect("the sides suggested");
    assert!(side.reasons.contains(&Reason::SameLength), "{side:?}");
    assert!(side.reasons.contains(&Reason::NotchesMatch), "{side:?}");
}
