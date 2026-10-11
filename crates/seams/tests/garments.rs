//! Seams on made-up garments whose seams are known: measuring them, and
//! what is suggested for them.

use drapewright_pattern_core::{
    Allowance, AllowanceMode, CutInstruction, Edge, EdgeKind, Grainline, Notch, NotchFrom,
    NotchKind, Outline, Piece, Pt, Segment,
};
use drapewright_seams::{Reason, Seam, SeamSide, measure, suggest};

fn edge(id: &str, kind: EdgeKind, segment: Segment) -> Edge {
    Edge {
        kind,
        ..Edge::new(id, vec![segment])
    }
}

fn piece(id: &str, name: &str, start: [f64; 2], edges: Vec<Edge>, quantity: u32) -> Piece {
    let mut p = Piece::new(
        id,
        name,
        Outline {
            start: Pt::new(start[0], start[1]),
            edges,
        },
    );
    p.cut = vec![CutInstruction {
        quantity,
        ..Default::default()
    }];
    p.grainline = Some(Grainline {
        from: Pt::new(100.0, 100.0),
        to: Pt::new(100.0, 400.0),
        one_way: false,
    });
    p
}

fn notch(id: &str, edge: &str, at: f64) -> Notch {
    Notch {
        id: id.into(),
        edge: edge.into(),
        at,
        from: NotchFrom::Start,
        kind: NotchKind::Single,
        angle: 0.0,
    }
}

/// A skirt: the front cut on the fold, the back cut as a pair with a centre
/// back seam; the side seams curve over the hip and are notched alike.
fn skirt() -> Vec<Piece> {
    use EdgeKind::*;
    let mut front = piece(
        "front",
        "Skirt front",
        [0.0, 0.0],
        vec![
            edge("fw", Seam, Segment::line([250.0, 10.0])),
            edge(
                "fs",
                Seam,
                Segment::cubic([275.0, 60.0], [290.0, 250.0], [285.0, 600.0]),
            ),
            edge(
                "fh",
                Hem,
                Segment::cubic([190.0, 610.0], [95.0, 618.0], [0.0, 620.0]),
            ),
            edge("fcf", Fold, Segment::line([0.0, 0.0])),
        ],
        1,
    );
    front.notches.push(notch("fn", "fs", 180.0));
    let mut back = piece(
        "back",
        "Skirt back",
        [0.0, 0.0],
        vec![
            edge("bw", Seam, Segment::line([250.0, 15.0])),
            edge(
                "bs",
                Seam,
                Segment::cubic([275.0, 65.0], [290.0, 255.0], [285.0, 605.0]),
            ),
            edge(
                "bh",
                Hem,
                Segment::cubic([190.0, 615.0], [95.0, 623.0], [0.0, 625.0]),
            ),
            edge("bcb", Seam, Segment::line([0.0, 0.0])),
        ],
        2,
    );
    back.notches.push(notch("bn", "bs", 182.0));
    vec![front, back]
}

/// A bodice and sleeve: front and back cut on the fold, a sleeve cut as a
/// pair with its underarm seam.
fn bodice() -> Vec<Piece> {
    use EdgeKind::*;
    let front = piece(
        "front",
        "Front",
        [0.0, 80.0],
        vec![
            edge(
                "fneck",
                Seam,
                Segment::cubic([40.0, 80.0], [65.0, 40.0], [70.0, 0.0]),
            ),
            edge("fsh", Seam, Segment::line([180.0, 25.0])),
            edge(
                "farm",
                Seam,
                Segment::cubic([165.0, 100.0], [160.0, 200.0], [200.0, 230.0]),
            ),
            edge("fside", Seam, Segment::line([210.0, 500.0])),
            edge("fhem", Hem, Segment::line([0.0, 500.0])),
            edge("fcf", Fold, Segment::line([0.0, 80.0])),
        ],
        1,
    );
    let back = piece(
        "back",
        "Back",
        [0.0, 20.0],
        vec![
            edge(
                "bneck",
                Seam,
                Segment::cubic([40.0, 20.0], [65.0, 10.0], [70.0, 0.0]),
            ),
            edge("bsh", Seam, Segment::line([181.0, 22.0])),
            edge(
                "barm",
                Seam,
                Segment::cubic([170.0, 90.0], [168.0, 200.0], [205.0, 230.0]),
            ),
            edge("bside", Seam, Segment::line([210.0, 500.0])),
            edge("bhem", Hem, Segment::line([0.0, 500.0])),
            edge("bcb", Fold, Segment::line([0.0, 20.0])),
        ],
        1,
    );
    let sleeve = piece(
        "sleeve",
        "Sleeve",
        [0.0, 150.0],
        vec![
            edge(
                "cap",
                Seam,
                Segment::cubic([100.0, -20.0], [300.0, -20.0], [400.0, 150.0]),
            ),
            edge("uback", Seam, Segment::line([330.0, 600.0])),
            edge("wrist", Hem, Segment::line([70.0, 600.0])),
            edge("ufront", Seam, Segment::line([0.0, 150.0])),
        ],
        2,
    );
    vec![front, back, sleeve]
}

/// The suggested seams as "piece.edge-piece.edge" (mirror sides marked *),
/// sorted.
fn pairs(pieces: &[Piece]) -> Vec<String> {
    let side = |s: &SeamSide| format!("{}.{}{}", s.piece, s.edge, if s.mirror { "*" } else { "" });
    let mut out: Vec<String> = suggest(pieces, &[])
        .iter()
        .map(|s| {
            assert!(!s.seam.confirmed);
            let (a, b) = (side(&s.seam.a), side(&s.seam.b));
            if a <= b {
                format!("{a}-{b}")
            } else {
                format!("{b}-{a}")
            }
        })
        .collect();
    out.sort();
    out
}

#[test]
fn a_skirts_seams_are_suggested() {
    let pieces = skirt();
    // Side seams, and the back's centre seam to its other half; not the
    // waists (each goes to the waistband) nor the hems.
    assert_eq!(pairs(&pieces), ["back.bcb-back.bcb*", "back.bs-front.fs"]);
    let side = suggest(&pieces, &[])
        .into_iter()
        .find(|s| s.seam.a.edge == "fs" || s.seam.b.edge == "fs")
        .unwrap();
    // Both run from the waist down: start to start.
    assert!(!side.seam.reversed);
    assert!(side.confidence > 0.9, "{}", side.confidence);
    for reason in [Reason::SameShape, Reason::NotchesMatch, Reason::EndsMatch] {
        assert!(
            side.reasons.contains(&reason),
            "{reason:?} in {:?}",
            side.reasons
        );
    }
}

#[test]
fn a_bodices_seams_are_suggested_but_not_its_armholes() {
    // Shoulders, side seams and the sleeve's underarm (sewn to itself, wrist
    // to wrist). The armholes are as long as each other, but each is sewn
    // to the sleeve.
    assert_eq!(
        pairs(&bodice()),
        [
            "back.bsh-front.fsh",
            "back.bside-front.fside",
            "sleeve.uback-sleeve.ufront"
        ]
    );
}

#[test]
fn seams_already_made_are_left_alone() {
    let pieces = skirt();
    let made = Seam {
        id: "s1".into(),
        name: "Side seam".into(),
        a: SeamSide::whole("front", "fs"),
        b: SeamSide::whole("back", "bs"),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    };
    assert_eq!(
        suggest(&pieces, std::slice::from_ref(&made))
            .iter()
            .map(|s| s.seam.a.edge.as_str())
            .collect::<Vec<_>>(),
        ["bcb"]
    );
}

#[test]
fn a_seam_measures_both_sides_and_their_notches() {
    let pieces = skirt();
    let seam = Seam {
        id: "s1".into(),
        name: String::new(),
        a: SeamSide::whole("front", "fs"),
        b: SeamSide::whole("back", "bs"),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    };
    let report = measure(&seam, &pieces);
    assert_eq!(report.problem, None);
    assert!(
        (report.a_length - report.b_length).abs() < 1.0,
        "{report:?}"
    );
    assert_eq!(report.notches.len(), 1);
    let apart = report.notches[0].apart.unwrap();
    assert!((apart - 2.0).abs() < 0.01, "{apart}");

    // Sewn the other way, the back's notch is measured from its hem end.
    let other_way = Seam {
        reversed: true,
        ..seam.clone()
    };
    let report = measure(&other_way, &pieces);
    let at = report.notches[0].b.as_ref().unwrap().at;
    assert!((at - (report.b_length - 182.0)).abs() < 0.01, "{at}");

    // Part of an edge, and an edge that is gone.
    let part = Seam {
        a: SeamSide {
            from: Some(100.0),
            to: Some(300.0),
            ..SeamSide::whole("front", "fs")
        },
        ..seam.clone()
    };
    let report = measure(&part, &pieces);
    assert!((report.a_length - 200.0).abs() < 1e-9);
    assert!((report.notches[0].a.as_ref().unwrap().at - 80.0).abs() < 1e-9);
    let gone = Seam {
        b: SeamSide::whole("back", "nope"),
        ..seam
    };
    assert!(measure(&gone, &pieces).problem.is_some());
}

#[test]
fn allowance_included_is_measured_inside_it() {
    let mut pieces = skirt();
    for p in &mut pieces {
        p.allowance = Allowance {
            mode: AllowanceMode::Included,
            seam: 15.0,
            hem: Some(40.0),
        };
    }
    let seam = Seam {
        id: "s1".into(),
        name: String::new(),
        a: SeamSide::whole("front", "fs"),
        b: SeamSide::whole("back", "bs"),
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    };
    let added = measure(&seam, &skirt()).a_length;
    let included = measure(&seam, &pieces).a_length;
    // Shorter by the waist's 15 mm and the hem's 40 mm, near enough.
    assert!((added - included - 55.0).abs() < 5.0, "{added} {included}");
}

#[test]
fn edges_chosen_by_hand_are_turned_the_right_way() {
    // The sleeve's underarm edges run opposite ways round its outline:
    // wrist meets wrist when one is turned.
    let pieces = bodice();
    assert!(drapewright_seams::sewn_reversed(
        &SeamSide::whole("sleeve", "uback"),
        &SeamSide::whole("sleeve", "ufront"),
        &pieces
    ));
    let pieces = skirt();
    assert!(!drapewright_seams::sewn_reversed(
        &SeamSide::whole("front", "fs"),
        &SeamSide::whole("back", "bs"),
        &pieces
    ));
}
