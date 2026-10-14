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
fn a_back_shoulder_a_little_longer_is_eased_onto_the_front() {
    // The back shoulder 6 mm longer than the front's, as patterns ease it:
    // still the shoulder seam.
    let mut pieces = bodice();
    let back = pieces.iter_mut().find(|p| p.id == "back").unwrap();
    let sh = back
        .outline
        .edges
        .iter_mut()
        .find(|e| e.id == "bsh")
        .unwrap();
    sh.segments = vec![Segment::line([187.0, 23.0])];
    assert_eq!(
        pairs(&pieces),
        [
            "back.bsh-front.fsh",
            "back.bside-front.fside",
            "sleeve.uback-sleeve.ufront"
        ]
    );
    let shoulder = suggest(&pieces, &[])
        .into_iter()
        .find(|s| s.seam.a.edge == "bsh" || s.seam.b.edge == "bsh")
        .unwrap();
    assert!(
        shoulder.reasons.contains(&Reason::Eased),
        "{:?}",
        shoulder.reasons
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

/// The seams suggested for `pieces`, sewn by `existing` so far, and each as
/// "piece.edge[*]-piece.edge[*]" (mirror copies marked *), sorted; parts
/// of an edge marked with where they end, to the millimetre.
fn listed(
    pieces: &[Piece],
    existing: &[Seam],
) -> (Vec<drapewright_seams::Suggestion>, Vec<String>) {
    let suggested = suggest(pieces, existing);
    let side = |s: &SeamSide| {
        let part = match (s.from, s.to) {
            (None, None) => String::new(),
            (from, to) => format!(
                "[{:.0}..{}]",
                from.unwrap_or(0.0),
                to.map_or("end".into(), |t| format!("{t:.0}"))
            ),
        };
        format!(
            "{}.{}{}{}",
            s.piece,
            s.edge,
            part,
            if s.mirror { "*" } else { "" }
        )
    };
    let mut out: Vec<String> = suggested
        .iter()
        .map(|s| {
            let (a, b) = (side(&s.seam.a), side(&s.seam.b));
            if a <= b {
                format!("{a}-{b}")
            } else {
                format!("{b}-{a}")
            }
        })
        .collect();
    out.sort();
    (suggested, out)
}

#[test]
fn a_yoked_blouse_is_put_together_from_its_gathers_yokes_and_armholes() {
    for sideways in [false, true] {
        let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(sideways);
        let (suggested, pairs) = listed(&pieces, &[]);
        let has = |pair: &str| pairs.iter().any(|p| p == pair);
        // Each front's side to one of the back's (which, the back being
        // alike either side, is for the garment to say).
        let sides = pairs
            .iter()
            .filter(|p| p.contains("back.bside") && p.contains(".fside"))
            .count();
        assert_eq!(sides, 2, "{pairs:#?}");
        // The yoke to both of the back's shoulders, one copy each.
        let shoulders: Vec<&String> = pairs.iter().filter(|p| p.contains("yback")).collect();
        assert_eq!(shoulders.len(), 2, "{pairs:#?}");
        assert_eq!(shoulders.iter().filter(|p| p.contains("yback*")).count(), 1);
        // Each front gathered to a yoke: one copy each.
        let gathered: Vec<&drapewright_seams::Suggestion> = suggested
            .iter()
            .filter(|s| s.reasons.contains(&Reason::Gathered))
            .collect();
        let tops: Vec<&String> = pairs.iter().filter(|p| p.contains("yfront")).collect();
        assert_eq!(tops.len(), 2, "{pairs:#?}");
        assert!(tops.iter().any(|p| p.contains("left.ftop")));
        assert!(tops.iter().any(|p| p.contains("right.ftop")));
        assert_eq!(tops.iter().filter(|p| p.contains("yfront*")).count(), 1);
        // The sleeve sewn to itself and gathered into the cuff.
        assert!(has("sleeve.sback-sleeve.sfront"), "{pairs:#?}");
        assert!(has("cuff.ctop-sleeve.shem"), "{pairs:#?}");
        assert_eq!(gathered.len(), 3, "{pairs:#?}");
        // The back's neck to its facing.
        assert!(has("back.bneck-facing.fcneck"), "{pairs:#?}");
        // Each sleeve's head round an armhole of three pieces: the back's,
        // the yoke's end and the front's, split where its own notches say
        // the shoulder and the yoke's end are (not where the lengths alone
        // would, a little off).
        let heads: Vec<&String> = pairs.iter().filter(|p| p.contains("shead")).collect();
        for armhole in ["back.barml", "back.barmr", "left.farm", "right.farm"] {
            assert_eq!(
                heads.iter().filter(|p| p.contains(armhole)).count(),
                1,
                "{armhole}: {pairs:#?}"
            );
        }
        let yoke_ends = heads.iter().filter(|p| p.contains("yoke.yarm")).count();
        assert!((1..=2).contains(&yoke_ends), "{pairs:#?}");
        let sleeve = pieces.iter().find(|p| p.id == "sleeve").unwrap();
        let notched: Vec<f64> = sleeve.notches.iter().skip(2).map(|n| n.at).collect();
        for s in suggested
            .iter()
            .filter(|s| s.reasons.contains(&Reason::FitsOpening))
        {
            let side = if s.seam.a.piece == "sleeve" {
                &s.seam.a
            } else {
                &s.seam.b
            };
            for at in [side.from, side.to].into_iter().flatten() {
                let at_end = !(0.5..=400.0).contains(&at);
                assert!(
                    at_end || notched.iter().any(|n| (n - at).abs() < 0.01),
                    "split at {at}, not at a notch {notched:?}"
                );
            }
        }
        // Not the fronts' armholes to each other, as long and shaped alike
        // as they are.
        assert!(!has("left.farm-right.farm"), "{pairs:#?}");
        // Nothing sewn twice.
        let mut edges: Vec<String> = pairs
            .iter()
            .flat_map(|p| p.split('-').map(str::to_string).collect::<Vec<_>>())
            .collect();
        let count = edges.len();
        edges.sort();
        edges.dedup();
        assert_eq!(edges.len(), count, "{pairs:#?}");
    }
}

#[test]
fn a_sleeve_heads_parts_measure_with_their_notches_met() {
    let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(false);
    let parts: Vec<Seam> = suggest(&pieces, &[])
        .into_iter()
        .filter(|s| s.reasons.contains(&Reason::FitsOpening))
        .map(|s| s.seam)
        .collect();
    assert!(!parts.is_empty());
    for seam in &parts {
        let report = measure(seam, &pieces);
        assert_eq!(report.problem, None);
        // Eased in a little: a few percent.
        assert!(
            report.difference.abs() < 0.06 * report.a_length,
            "{report:?}"
        );
        // The back's double notch meets the head's; the notches where the
        // parts meet each other are not left without partners.
        assert!(
            report.notches.iter().all(|n| n.apart.is_some()),
            "{seam:?}: {report:?}"
        );
        let back = [&seam.a, &seam.b]
            .iter()
            .any(|s| s.edge.starts_with("barm"));
        assert_eq!(report.notches.len(), if back { 2 } else { 0 }, "{report:?}");
    }
}

#[test]
fn a_corsets_panels_are_sewn_where_the_lines_across_them_meet() {
    use drapewright_pattern_core::testing::garments::corset;
    // Cut as pairs: the side panel along the top of the centre front's
    // edge, where the lines across them meet (the centre front running on
    // below it), and the back to the side panel; each centre to its other
    // half, and the crotch piece along both centre fronts' lower edges,
    // across the centre. Not the tops of the centre front and back to each
    // other, as long as each other as they are.
    let (suggested, pairs) = listed(&corset(true), &[]);
    assert_eq!(
        pairs,
        [
            "bk.bkl-fs.fsr",
            "bk.bkr-bk.bkr*",
            "cr.crt[0..73]-fc.fcb",
            "cr.crt[73..146]-fc.fcb*",
            "fc.fcl-fc.fcl*",
            "fc.fcr[0..461]-fs.fsl",
        ]
    );
    let why = |edge: &str| -> Vec<Reason> {
        suggested
            .iter()
            .filter(|s| s.seam.a.edge == edge || s.seam.b.edge == edge)
            .flat_map(|s| s.reasons.clone())
            .collect()
    };
    for edge in ["fsl", "fsr"] {
        assert!(
            why(edge).contains(&Reason::LinesMeet),
            "{edge}: {:?}",
            why(edge)
        );
    }
    for edge in ["fcl", "bkr"] {
        assert_eq!(why(edge), [Reason::CentreOfPair], "{edge}");
    }
    assert_eq!(why("crt"), [Reason::FitsAlong, Reason::FitsAlong]);
    // Cut once, as the pattern leaves them: half a corset, with no centre
    // to sew or to go along.
    let (once, pairs) = listed(&corset(false), &[]);
    assert_eq!(pairs, ["bk.bkl-fs.fsr", "fc.fcr[0..461]-fs.fsl"]);
    // Those sewn, then the panels cut as pairs: the rest, and still not
    // a panel's top to another's bottom, as long as each other as they are.
    let sewn: Vec<Seam> = once
        .into_iter()
        .enumerate()
        .map(|(k, s)| Seam {
            id: format!("s{k}"),
            ..s.seam
        })
        .collect();
    let (_, pairs) = listed(&corset(true), &sewn);
    assert_eq!(
        pairs,
        [
            "bk.bkr-bk.bkr*",
            "cr.crt[0..73]-fc.fcb",
            "cr.crt[73..146]-fc.fcb*",
            "fc.fcl-fc.fcl*",
        ]
    );
}

#[test]
fn pieces_drawn_for_one_side_are_found_to_be_cut_as_pairs() {
    use drapewright_pattern_core::testing::garments::{corset, yoked_blouse};
    use drapewright_seams::drawn_for_one_side;
    // A corset's panels drawn once, from its centre front to its centre
    // back, both left open: cut as pairs. Not its crotch piece, sewn to
    // nothing as it is.
    assert_eq!(drawn_for_one_side(&corset(false), &[]), ["fc", "fs", "bk"]);
    let (once, _) = listed(&corset(false), &[]);
    let sewn: Vec<Seam> = once.into_iter().map(|s| s.seam).collect();
    assert_eq!(
        drawn_for_one_side(&corset(false), &sewn),
        ["fc", "fs", "bk"]
    );
    // Cut as pairs already, or cut on the fold: nothing to change.
    assert!(drawn_for_one_side(&corset(true), &[]).is_empty());
    assert!(drawn_for_one_side(&skirt(), &[]).is_empty());
    assert!(drawn_for_one_side(&bodice(), &[]).is_empty());
    assert!(drawn_for_one_side(&yoked_blouse(false), &[]).is_empty());
    // A skirt drawn as half a front and half a back, each cut once: not its
    // centre front sewn to its centre back, though both are straight and
    // nearly as long as each other ...
    let mut half = skirt();
    half[0].outline.edges[3].kind = EdgeKind::Seam;
    half[1].cut[0].quantity = 1;
    assert_eq!(drawn_for_one_side(&half, &[]), ["front", "back"]);
    // ... and cut as pairs, each centre to its other half; not the waists
    // to each other (each goes to the waistband).
    for p in &mut half {
        p.cut[0].quantity = 2;
    }
    assert_eq!(
        pairs(&half),
        [
            "back.bcb-back.bcb*",
            "back.bs-front.fs",
            "front.fcf-front.fcf*"
        ]
    );
}

#[test]
fn a_crotch_in_two_pieces_is_sewn_to_itself_never_to_a_top() {
    use EdgeKind::{Fold, Seam};
    let line = |to: [f64; 2]| Segment::line(to);
    // A front and a back, each cut on the fold: 160 wide at the top, 200 at
    // the bottom; their sides sewn.
    let panel = |id: &str, name: &str| {
        piece(
            id,
            name,
            [0.0, 0.0],
            vec![
                edge(&format!("{id}t"), Seam, line([160.0, 0.0])),
                edge(&format!("{id}s"), Seam, line([200.0, 500.0])),
                edge(&format!("{id}b"), Seam, line([0.0, 500.0])),
                edge(&format!("{id}f"), Fold, line([0.0, 0.0])),
            ],
            1,
        )
    };
    // A crotch's front and back, where they meet each other 160 and 158
    // across (the front's as long as either panel's top: by length alone,
    // the closer match), 100 where they meet the panels' bottom halves
    // (200, both halves, along).
    let crotch = |id: &str, name: &str, across: f64| {
        piece(
            id,
            name,
            [0.0, 0.0],
            vec![
                edge(&format!("{id}m"), Seam, line([across, 0.0])),
                edge(&format!("{id}r"), Seam, line([across / 2.0 + 50.0, 120.0])),
                edge(&format!("{id}p"), Seam, line([across / 2.0 - 50.0, 120.0])),
                edge(&format!("{id}l"), Seam, line([0.0, 0.0])),
            ],
            1,
        )
    };
    let pieces = vec![
        panel("f", "Front"),
        panel("b", "Back"),
        crotch("cf", "Crotch front", 160.0),
        crotch("cb", "Crotch back", 158.0),
    ];
    let found = pairs(&pieces);
    assert!(found.contains(&"cb.cbm-cf.cfm".to_string()), "{found:?}");
    // No crotch edge sewn to a panel's top.
    let crotch_to_top = |s: &String| s.contains('c') && (s.contains(".ft") || s.contains(".bt"));
    assert!(!found.iter().any(crotch_to_top), "{found:?}");
}

#[test]
fn trousers_are_sewn_at_their_inseams_outseams_and_centres() {
    // Each leg's front and back at the outseam and the inseam; each crotch
    // curve to the other leg's.
    assert_eq!(
        pairs(&drapewright_pattern_core::testing::garments::trousers()),
        [
            "tb.tbc-tb.tbc*",
            "tb.tbi-tf.tfi",
            "tb.tbo-tf.tfo",
            "tf.tfc-tf.tfc*"
        ]
    );
}
