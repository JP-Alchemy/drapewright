//! Made-up garments for tests across crates.

use crate::Pt;
use crate::model::{
    Allowance, AllowanceMode, CutInstruction, Edge, EdgeKind, Grainline, InternalLine, LineKind,
    Mark, MarkKind, Notch, NotchFrom, NotchKind, Outline, Piece, Segment,
};

fn edge(id: &str, segments: Vec<Segment>) -> Edge {
    Edge::new(id, segments)
}

fn line(to: [f64; 2]) -> Vec<Segment> {
    vec![Segment::line(to)]
}

fn cubic(c1: [f64; 2], c2: [f64; 2], to: [f64; 2]) -> Vec<Segment> {
    vec![Segment::cubic(c1, c2, to)]
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
    // 10 mm seam allowances added, as a bought pattern's often are.
    p.allowance = Allowance {
        mode: AllowanceMode::Add,
        seam: 10.0,
        hem: None,
    };
    p
}

fn grain(p: &mut Piece, from: [f64; 2], to: [f64; 2]) {
    p.grainline = Some(Grainline {
        from: Pt::new(from[0], from[1]),
        to: Pt::new(to[0], to[1]),
        one_way: false,
    });
}

fn notch(p: &mut Piece, edge: &str, at: f64, from: NotchFrom) {
    let id = format!("{}n{}", p.id, p.notches.len());
    p.notches.push(Notch {
        id,
        edge: edge.into(),
        at,
        from,
        kind: NotchKind::Single,
        angle: 0.0,
    });
}

/// A wavy line from `a` to `b`, `out` mm to its right (y down): pattern
/// CAD's sign for gathering, drawn along the edge to gather.
fn gathering(p: &mut Piece, a: [f64; 2], b: [f64; 2], out: f64) {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = dx.hypot(dy);
    let (ux, uy) = (dx / length, dy / length);
    // Right of the way along, as seen on screen.
    let (nx, ny) = (-uy, ux);
    let steps = (length / 3.0).round() as usize;
    let at = |k: usize| {
        let t = k as f64 / steps as f64;
        let wave = out + if k.is_multiple_of(2) { 2.5 } else { -2.5 };
        [a[0] + dx * t + nx * wave, a[1] + dy * t + ny * wave]
    };
    let start = at(0);
    let id = format!("{}l{}", p.id, p.lines.len());
    p.lines.push(InternalLine {
        id,
        kind: LineKind::Line,
        start: Pt::new(start[0], start[1]),
        segments: (1..=steps).map(|k| Segment::line(at(k))).collect(),
        label: None,
    });
}

/// `p` turned a quarter clockwise on screen (y down), as pattern CAD lays a
/// long piece on its side to fit the sheet: its top to the right.
fn on_its_side(p: &mut Piece) {
    let turn = |q: Pt| Pt::new(-q.y, q.x);
    p.outline.start = turn(p.outline.start);
    let turned = |s: &Segment| match s {
        Segment::Line(l) => Segment::line(turn(l.to)),
        Segment::Cubic(c) => Segment::cubic(turn(c.c1), turn(c.c2), turn(c.to)),
    };
    for e in &mut p.outline.edges {
        e.segments = e.segments.iter().map(turned).collect();
    }
    if let Some(g) = &mut p.grainline {
        g.from = turn(g.from);
        g.to = turn(g.to);
    }
    for m in &mut p.marks {
        m.at = turn(m.at);
    }
    for l in &mut p.lines {
        l.start = turn(l.start);
        l.segments = l.segments.iter().map(turned).collect();
    }
}

/// A line across a piece from `a` to `b`, named for where it goes on the
/// body.
fn level(p: &mut Piece, name: &str, a: [f64; 2], b: [f64; 2]) {
    let id = format!("{}l{}", p.id, p.lines.len());
    p.lines.push(InternalLine {
        id,
        kind: LineKind::Line,
        start: Pt::new(a[0], a[1]),
        segments: vec![Segment::line(b)],
        label: Some(name.into()),
    });
}

/// An overbust corset drawn for one side, as indie patterns print one: a
/// centre front, a side and a centre back panel (`pairs`: cut as pairs, as
/// sewn; else cut once, as the pattern leaves them), with no notches but
/// their bust, underbust, waist and hip lines drawn across, meeting at each
/// seam. The seams are shaped in at the waist, the centres straight; the
/// tops slope up towards the back, level with each other at each seam, so
/// the lines meet each seam a different way down. The centre front runs on
/// 40 mm below the side panel. A crotch piece goes along both centre
/// fronts' lower edges, across the centre front. The tops of the centre
/// front and back are as long as each other.
pub fn corset(pairs: bool) -> Vec<Piece> {
    let quantity = if pairs { 2 } else { 1 };
    let levels = [
        ("Bust", 100.0),
        ("Underbust", 160.0),
        ("Waist", 260.0),
        ("Hip", 380.0),
    ];
    // An upright side, shaped in at its middle (a seam) or straight (a
    // centre). Its control points a third and two thirds of the way along,
    // it is `inwards` × 3t(1 - t) across at t of the way down.
    let side = |from: [f64; 2], to: [f64; 2], inwards: f64| {
        if inwards == 0.0 {
            return line(to);
        }
        let at = |t: f64| from[1] + (to[1] - from[1]) * t;
        cubic(
            [from[0] + inwards, at(1.0 / 3.0)],
            [to[0] + inwards, at(2.0 / 3.0)],
            to,
        )
    };
    let across = |from: [f64; 2], to: [f64; 2], inwards: f64, y: f64| {
        let t = (y - from[1]) / (to[1] - from[1]);
        from[0] + 3.0 * inwards * t * (1.0 - t)
    };
    // `top` and `bottom`: the heights of the left and right corners.
    let panel =
        |id: &str, name: &str, x: f64, w: f64, top: [f64; 2], bottom: [f64; 2], centre: char| {
            // Clockwise on screen: top, right side down, bottom, left side up;
            // a centre side straight, a seam's shaped in 15 mm.
            let (l, r) = match centre {
                'l' => (0.0, -20.0),
                'r' => (20.0, 0.0),
                _ => (20.0, -20.0),
            };
            let (tl, tr) = ([x, top[0]], [x + w, top[1]]);
            let (bl, br) = ([x, bottom[0]], [x + w, bottom[1]]);
            let mut p = piece(
                id,
                name,
                tl,
                vec![
                    edge(&format!("{id}t"), line(tr)),
                    edge(&format!("{id}r"), side(tr, br, r)),
                    edge(&format!("{id}b"), line(bl)),
                    edge(&format!("{id}l"), side(bl, tl, l)),
                ],
                quantity,
            );
            grain(&mut p, [x + w / 2.0, 30.0], [x + w / 2.0, 330.0]);
            for (name, y) in levels {
                let ends = (across(bl, tl, l, y), across(tr, br, r, y));
                level(&mut p, name, [ends.0, y], [ends.1, y]);
            }
            p
        };
    // The centre front's left side is its centre, its right sewn to the
    // side panel's left; the back's right side is the centre back.
    let front = panel(
        "fc",
        "Front Center",
        0.0,
        70.0,
        [10.0, 0.0],
        [520.0, 500.0],
        'l',
    );
    let middle = panel(
        "fs",
        "Front Side",
        200.0,
        80.0,
        [0.0, -15.0],
        [460.0, 440.0],
        ' ',
    );
    let back = panel(
        "bk",
        "Back Center",
        400.0,
        70.0,
        [-15.0, -25.0],
        [440.0, 430.0],
        'r',
    );
    // Along both centre fronts' lower edges (each 72.8 mm), across the
    // centre: 145.6 mm.
    let bottom = (70.0f64).hypot(20.0);
    let mut crotch = piece(
        "cr",
        "Crotch Front",
        [600.0, 0.0],
        vec![
            edge("crt", line([600.0 + 2.0 * bottom, 0.0])),
            edge("crr", line([710.0, 100.0])),
            edge("crb", line([650.0, 100.0])),
            edge("crl", line([600.0, 0.0])),
        ],
        1,
    );
    grain(&mut crotch, [670.0, 20.0], [670.0, 90.0]);
    vec![front, middle, back, crotch]
}

/// A long blouse made up as some bought patterns are: a back cut whole, a
/// left and a right front buttoned down the front below a V neck, each
/// gathered into a narrow yoke over the shoulder (cut as a pair) that is
/// sewn to the back's shoulder; sleeves gathered into cuffs, their heads
/// sewn round armholes made of the back, the yoke's end and the front; and
/// a back neck facing. Hems have a deeper allowance than seams; no edge is
/// named. `sideways`: the back and fronts drawn lying on their sides, and
/// the cuffs upside down, as CAD lays them out on the sheet.
pub fn yoked_blouse(sideways: bool) -> Vec<Piece> {
    let hem = |e: Edge| Edge {
        allowance: Some(20.0),
        ..e
    };
    // The back, its neck and shoulders at the top.
    let mut back = piece(
        "back",
        "Back",
        [-80.0, 0.0],
        vec![
            edge("bneck", cubic([-40.0, 18.0], [40.0, 18.0], [80.0, 0.0])),
            edge("bshr", line([180.0, 45.0])),
            edge(
                "barmr",
                cubic([172.0, 120.0], [190.0, 225.0], [250.0, 230.0]),
            ),
            edge("bsider", line([260.0, 700.0])),
            hem(edge("bhem", line([-260.0, 700.0]))),
            edge("bsidel", line([-250.0, 230.0])),
            edge(
                "barml",
                cubic([-190.0, 225.0], [-172.0, 120.0], [-180.0, 45.0]),
            ),
            edge("bshl", line([-80.0, 0.0])),
        ],
        1,
    );
    grain(&mut back, [0.0, 100.0], [0.0, 600.0]);
    // The armholes' double notches, the back's own.
    for (e, from) in [("barmr", NotchFrom::End), ("barml", NotchFrom::Start)] {
        notch(&mut back, e, 60.0, from);
        notch(&mut back, e, 70.0, from);
    }
    let shoulder = 100.0f64.hypot(45.0);
    // The yoke: its long sides to the back's shoulder and the front.
    let mut yoke = piece(
        "yoke",
        "Front yoke",
        [0.0, 0.0],
        vec![
            edge("yneck", line([40.0, 0.0])),
            edge("yfront", line([40.0, shoulder])),
            edge("yarm", line([0.0, shoulder])),
            edge("yback", line([0.0, 0.0])),
        ],
        2,
    );
    grain(&mut yoke, [20.0, 10.0], [20.0, shoulder - 10.0]);
    // A notch along the shoulder, nearer the armhole, on the yoke and on
    // both of the back's shoulders: which way round they go.
    notch(&mut yoke, "yback", 30.0, NotchFrom::Start);
    notch(&mut back, "bshr", 30.0, NotchFrom::End);
    notch(&mut back, "bshl", 30.0, NotchFrom::Start);
    // The right front, as seen from outside: its side seam on its left,
    // its front edge on its right.
    let mut right = piece(
        "right",
        "Right front",
        [-230.0, 60.0],
        vec![
            edge("ftop", line([-80.0, 0.0])),
            edge("fvneck", line([0.0, 250.0])),
            edge("fedge", line([0.0, 700.0])),
            hem(edge("fhem", line([-260.0, 700.0]))),
            edge("fside", line([-250.0, 230.0])),
            edge(
                "farm",
                cubic([-190.0, 225.0], [-235.0, 150.0], [-230.0, 60.0]),
            ),
        ],
        1,
    );
    grain(&mut right, [-130.0, 300.0], [-130.0, 650.0]);
    gathering(&mut right, [-230.0, 60.0], [-80.0, 0.0], 8.0);
    // The left front, its mirror image.
    let mut left = piece(
        "left",
        "Left front",
        [80.0, 0.0],
        vec![
            edge("ftop", line([230.0, 60.0])),
            edge(
                "farm",
                cubic([235.0, 150.0], [190.0, 225.0], [250.0, 230.0]),
            ),
            edge("fside", line([260.0, 700.0])),
            hem(edge("fhem", line([0.0, 700.0]))),
            edge("fedge", line([0.0, 250.0])),
            edge("fvneck", line([80.0, 0.0])),
        ],
        1,
    );
    grain(&mut left, [130.0, 300.0], [130.0, 650.0]);
    gathering(&mut left, [80.0, 0.0], [230.0, 60.0], 8.0);
    // Buttons down its front edge, under the right front's.
    for k in 0..5 {
        let id = format!("button{k}");
        left.marks.push(Mark {
            id,
            kind: MarkKind::Button,
            at: Pt::new(12.0, 300.0 + 80.0 * f64::from(k)),
            size: Some(10.0),
            length: None,
            angle: 0.0,
            label: None,
        });
    }
    // The sleeve: its head from the back's underarm over the top to the
    // front's, as long round as the armhole.
    let mut sleeve = piece(
        "sleeve",
        "Sleeve",
        [-170.0, 130.0],
        vec![
            edge(
                "shead",
                vec![
                    Segment::cubic([-130.0, 35.0], [-70.0, 0.0], [0.0, 0.0]),
                    Segment::cubic([70.0, 0.0], [130.0, 35.0], [170.0, 130.0]),
                ],
            ),
            edge("sfront", line([180.0, 530.0])),
            edge("shem", line([-180.0, 530.0])),
            edge("sback", line([-170.0, 130.0])),
        ],
        2,
    );
    grain(&mut sleeve, [0.0, 80.0], [0.0, 480.0]);
    gathering(&mut sleeve, [180.0, 530.0], [-180.0, 530.0], 8.0);
    // Its head notched where it meets the shoulder and the yoke's end, a
    // little off where they fall eased in, and doubly at the back.
    let length = |p: &Piece, e: &str| {
        p.outline
            .edge_geom(p.outline.edge_index(e).expect("an edge"))
            .length()
    };
    let (back_arm, front_arm) = (length(&back, "barmr"), length(&right, "farm"));
    let head = length(&sleeve, "shead");
    let ease = head / (back_arm + 40.0 + front_arm);
    for at in [
        60.0,
        70.0,
        back_arm * ease + 1.5,
        (back_arm + 40.0) * ease - 1.0,
    ] {
        notch(&mut sleeve, "shead", at, NotchFrom::Start);
    }
    // The cuff, as worn: its top sewn to the sleeve, narrower at the wrist.
    let mut cuff = piece(
        "cuff",
        "Cuff",
        [-10.0, 0.0],
        vec![
            edge("ctop", line([190.0, 0.0])),
            edge("cend", line([180.0, 80.0])),
            edge("cwrist", line([0.0, 80.0])),
            edge("cstart", line([-10.0, 0.0])),
        ],
        2,
    );
    grain(&mut cuff, [90.0, 10.0], [90.0, 70.0]);
    // The back neck facing: its top as the back's neck, 50 mm deep.
    let mut facing = piece(
        "facing",
        "Back neck facing",
        [-80.0, 0.0],
        vec![
            edge("fcneck", cubic([-40.0, 18.0], [40.0, 18.0], [80.0, 0.0])),
            edge("fcend", line([75.0, 50.0])),
            edge("fcout", cubic([40.0, 66.0], [-40.0, 66.0], [-75.0, 50.0])),
            edge("fcstart", line([-80.0, 0.0])),
        ],
        1,
    );
    grain(&mut facing, [0.0, 20.0], [0.0, 45.0]);
    if sideways {
        for p in [&mut back, &mut right, &mut left] {
            on_its_side(p);
        }
        // The cuff upside down: turned twice.
        on_its_side(&mut cuff);
        on_its_side(&mut cuff);
    }
    vec![back, yoke, right, left, sleeve, cuff, facing]
}

/// Trousers, cut as pairs: a front and a back for each leg, each drawn as
/// the left leg is, its outseam on the left and its centre and inseam on
/// the right. The centre runs down from the waist and curves out to the
/// crotch point, 60 mm out on the front and 90 on the back; the inseam runs
/// down from there to the hem. The back is a little wider, and rises a
/// little higher at the centre back. The fronts are notched where the
/// inseam meets the hem, and so are the backs; each piece's grain runs down
/// its middle.
pub fn trousers() -> Vec<Piece> {
    let half = |id: &str, name: &str, waist: f64, rise: f64, out: f64, hem: f64| {
        // Clockwise on screen from the outseam's top: along the waist,
        // down the centre and round to the crotch point, down the inseam,
        // back along the hem, up the outseam.
        let (crotch, length) = (260.0, 1000.0);
        let mut p = piece(
            id,
            name,
            [0.0, 0.0],
            vec![
                edge(&format!("{id}w"), line([waist, -rise])),
                edge(
                    &format!("{id}c"),
                    vec![
                        Segment::line([waist, 170.0]),
                        Segment::cubic(
                            [waist, crotch - 30.0],
                            [waist + out * 0.6, crotch],
                            [waist + out, crotch],
                        ),
                    ],
                ),
                edge(
                    &format!("{id}i"),
                    line([waist + out - (waist + out - hem) / 2.0, length]),
                ),
                edge(&format!("{id}h"), line([(waist + out - hem) / 2.0, length])),
                edge(&format!("{id}o"), line([0.0, 0.0])),
            ],
            2,
        );
        grain(
            &mut p,
            [(waist + out) / 2.0, 200.0],
            [(waist + out) / 2.0, 800.0],
        );
        notch(&mut p, &format!("{id}i"), 0.0, NotchFrom::End);
        p
    };
    vec![
        half("tf", "Trouser front", 190.0, 0.0, 60.0, 220.0),
        half("tb", "Trouser back", 210.0, 30.0, 90.0, 260.0),
    ]
}

/// A cape coat, as a designer's pattern draws one, every piece cut as a
/// pair but the centre back (on the fold): an A-line coat in princess
/// panels, the centre front crossing 20 mm past its centre (done up with
/// snaps, which the pattern leaves to the instructions) with its centre
/// line drawn; a cape in a front and a back, each sewn by its front (or
/// back) edge into a princess seam from the neck, the two sewn together
/// over the shoulder; and a stand collar, a little longer than the
/// neckline (to button over), cut twice, inside and outside. Drawn as
/// seen from the front (from the back for the back pieces), x out from
/// the centre and y down from the shoulders; waist and hip lines across
/// every panel; 12 mm seams, 40 mm hems.
pub fn cape_coat() -> Vec<Piece> {
    let pair = |mut p: Piece| {
        p.allowance.seam = 12.0;
        p.cut = vec![CutInstruction {
            quantity: 2,
            mirrored: Some(true),
            ..Default::default()
        }];
        p
    };
    let hem = |p: &mut Piece, id: &str| {
        let e = p
            .outline
            .edges
            .iter_mut()
            .find(|e| e.id == id)
            .expect("the hem");
        e.allowance = Some(40.0);
    };
    // Where a straight edge from `a` to `b` is at height y.
    let at = |a: [f64; 2], b: [f64; 2], y: f64| a[0] + (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]);
    // Lines across every panel: (name, height), lower the further back each
    // meets a seam (at the front's edge, its princess seam, the side seam,
    // the back's princess seam and its centre), so each meets its own only.
    const LEVELS: [(&str, f64); 2] = [("Waist", 400.0), ("Hip", 600.0)];
    let (front_edge, front_seam, side_seam, back_seam, centre_back) = (-5.0, 0.0, 10.0, 20.0, 25.0);
    // The princess seam, from the neckline down; the side seam, from the
    // underarm down.
    let (princess_top, princess_hem) = ([55.0, 20.0], [110.0, 860.0]);
    let (underarm, side_hem) = ([200.0, 230.0], [260.0, 860.0]);
    let mut front = pair(piece(
        "cf",
        "Center Front",
        [-20.0, 80.0],
        vec![
            edge("cfn", cubic([10.0, 80.0], [40.0, 50.0], princess_top)),
            edge("cfp", line(princess_hem)),
            edge("cfh", line([-20.0, 860.0])),
            edge("cfe", line([-20.0, 80.0])),
        ],
        2,
    ));
    hem(&mut front, "cfh");
    grain(&mut front, [-20.0, 150.0], [-20.0, 750.0]);
    for (name, y) in LEVELS {
        let (y0, y1) = (y + front_edge, y + front_seam);
        level(
            &mut front,
            name,
            [-20.0, y0],
            [at(princess_top, princess_hem, y1), y1],
        );
    }
    let centre = format!("{}l{}", front.id, front.lines.len());
    front.lines.push(InternalLine {
        id: centre,
        kind: LineKind::Line,
        start: Pt::new(0.0, 120.0),
        segments: vec![Segment::line([0.0, 800.0])],
        label: Some("Centre front".into()),
    });
    // The back's princess seam, nearer the centre; its neckline higher, its
    // armhole fuller.
    let (back_top, back_hem) = ([35.0, 12.0], [120.0, 860.0]);
    // The side panels, from the princess seam round to the side seam.
    let side =
        |id: &str, name: &str, top: [f64; 2], hem_at: [f64; 2], arm: [[f64; 2]; 2], seam: f64| {
            let mut p = pair(piece(
                id,
                name,
                top,
                vec![
                    edge(&format!("{id}n"), line([75.0, 0.0])),
                    edge(&format!("{id}s"), line([180.0, 30.0])),
                    edge(&format!("{id}a"), cubic(arm[0], arm[1], underarm)),
                    edge(&format!("{id}d"), line(side_hem)),
                    edge(&format!("{id}h"), line(hem_at)),
                    edge(&format!("{id}p"), line(top)),
                ],
                2,
            ));
            hem(&mut p, &format!("{id}h"));
            grain(&mut p, [150.0, 300.0], [150.0, 700.0]);
            for (name, y) in LEVELS {
                let (y0, y1) = (y + seam, y + side_seam);
                level(
                    &mut p,
                    name,
                    [at(hem_at, top, y0), y0],
                    [at(underarm, side_hem, y1), y1],
                );
            }
            p
        };
    let side_front = side(
        "sf",
        "Side Front",
        princess_top,
        princess_hem,
        [[170.0, 110.0], [175.0, 200.0]],
        front_seam,
    );
    let side_back = side(
        "sb",
        "Side Back",
        back_top,
        back_hem,
        [[178.0, 100.0], [182.0, 190.0]],
        back_seam,
    );
    let mut back = piece(
        "cb",
        "Center Back",
        [0.0, 20.0],
        vec![
            edge("cbn", cubic([12.0, 20.0], [25.0, 16.0], back_top)),
            edge("cbp", line(back_hem)),
            edge("cbh", line([0.0, 860.0])),
            Edge {
                kind: EdgeKind::Fold,
                ..edge("cbf", line([0.0, 20.0]))
            },
        ],
        1,
    );
    back.allowance.seam = 12.0;
    back.cut[0].on_fold = Some(true);
    hem(&mut back, "cbh");
    grain(&mut back, [40.0, 150.0], [40.0, 750.0]);
    for (name, y) in LEVELS {
        let (y0, y1) = (y + centre_back, y + back_seam);
        level(&mut back, name, [0.0, y0], [at(back_top, back_hem, y1), y1]);
    }
    // The cape's front and back: each from the neck down the princess seam
    // `along` mm, out over the shoulder and arm, and round its hem.
    let cape = |id: &str, name: &str, top: [f64; 2], hem_at: [f64; 2], along: f64| {
        let (dx, dy) = (hem_at[0] - top[0], hem_at[1] - top[1]);
        let l = dx.hypot(dy);
        let down = [top[0] + dx / l * along, top[1] + dy / l * along];
        let mut p = pair(piece(
            id,
            name,
            top,
            vec![
                edge(&format!("{id}n"), line([75.0, 0.0])),
                edge(
                    &format!("{id}s"),
                    cubic([250.0, 60.0], [400.0, 180.0], [480.0, 330.0]),
                ),
                edge(
                    &format!("{id}h"),
                    cubic([380.0, 400.0], [200.0, 420.0], down),
                ),
                edge(&format!("{id}e"), line(top)),
            ],
            2,
        ));
        p.outline.edges[2].allowance = Some(35.0);
        grain(&mut p, [150.0, 100.0], [150.0, 350.0]);
        p
    };
    let front_cape = cape("fc", "Front Cape", princess_top, princess_hem, 390.0);
    let back_cape = cape("bc", "Back Cape", back_top, back_hem, 430.0);
    // The collar: round the neckline and 8% more, 50 mm deep, its corners
    // square; cut twice, outside and inside.
    let neckline: f64 = 2.0
        * [&front, &side_front, &side_back, &back]
            .iter()
            .map(|p| p.outline.edge_geom(0).length())
            .sum::<f64>();
    let long = 1.08 * neckline;
    let mut collar = piece(
        "co",
        "Collar",
        [0.0, 0.0],
        vec![
            edge("cot", line([long, 0.0])),
            edge("coe", line([long, 50.0])),
            edge("con", line([0.0, 50.0])),
            edge("cos", line([0.0, 0.0])),
        ],
        2,
    );
    collar.allowance.seam = 12.0;
    collar.cut[0].mirrored = Some(false);
    grain(&mut collar, [long / 2.0, 5.0], [long / 2.0, 45.0]);
    vec![
        front, side_front, side_back, back, front_cape, back_cape, collar,
    ]
}

/// A quarter circle's handles are this share of its radius from its ends.
const ROUND: f64 = 0.5523;

/// A patch pocket's outline as drawn (y down), `w` by `h` mm with its
/// bottom corners rounded `r`, its top left corner at `at`: (start, top,
/// right side, bottom, left side).
fn pocket_outline(at: [f64; 2], w: f64, h: f64, r: f64) -> ([f64; 2], [Vec<Segment>; 4]) {
    let [x, y] = at;
    let k = ROUND * r;
    (
        at,
        [
            line([x + w, y]),
            line([x + w, y + h - r]),
            vec![
                Segment::cubic(
                    [x + w, y + h - r + k],
                    [x + w - r + k, y + h],
                    [x + w - r, y + h],
                ),
                Segment::line([x + r, y + h]),
                Segment::cubic([x + r - k, y + h], [x, y + h - r + k], [x, y + h - r]),
            ],
            line([x, y]),
        ],
    )
}

/// The skirt the drape tests use (a front cut on the fold, a back cut as a
/// pair with a centre back seam) with patch pockets on the front: a pocket
/// 13 × 16 cm with rounded bottom corners, cut as a pair, its top a hem
/// turned down 3 cm, and its placement drawn on the front's half all round,
/// from its top left corner, as pattern CAD draws it (AAMA's layer 9).
pub fn pocket_skirt() -> Vec<Piece> {
    let mut front = piece(
        "front",
        "Skirt front",
        [0.0, 0.0],
        vec![
            edge("fw", line([185.0, 8.0])),
            edge("fs", cubic([215.0, 80.0], [255.0, 200.0], [290.0, 600.0])),
            Edge {
                kind: EdgeKind::Hem,
                ..edge("fh", cubic([190.0, 608.0], [95.0, 612.0], [0.0, 612.0]))
            },
            Edge {
                kind: EdgeKind::Fold,
                ..edge("fcf", line([0.0, 0.0]))
            },
        ],
        1,
    );
    grain(&mut front, [100.0, 100.0], [100.0, 400.0]);
    let (start, [top, right, bottom, left]) = pocket_outline([60.0, 120.0], 130.0, 160.0, 20.0);
    front.lines.push(InternalLine {
        id: "fpocket".into(),
        kind: LineKind::Placement,
        start: Pt::new(start[0], start[1]),
        segments: [top, right, bottom, left].concat(),
        label: Some("Pocket placement".into()),
    });
    let mut back = piece(
        "back",
        "Skirt back",
        [0.0, 0.0],
        vec![
            edge("bw", line([185.0, 12.0])),
            edge("bs", cubic([215.0, 84.0], [255.0, 204.0], [290.0, 604.0])),
            Edge {
                kind: EdgeKind::Hem,
                ..edge("bh", cubic([190.0, 612.0], [95.0, 616.0], [0.0, 616.0]))
            },
            edge("bcb", line([0.0, 0.0])),
        ],
        2,
    );
    grain(&mut back, [100.0, 100.0], [100.0, 400.0]);
    let (start, [top, right, bottom, left]) = pocket_outline([0.0, 0.0], 130.0, 160.0, 20.0);
    let mut pocket = piece(
        "pocket",
        "Patch pocket",
        start,
        vec![
            Edge {
                kind: EdgeKind::Hem,
                ..edge("pt", top)
            },
            edge("pr", right),
            edge("pb", bottom),
            edge("pl", left),
        ],
        2,
    );
    pocket.allowance.hem = Some(30.0);
    grain(&mut pocket, [65.0, 30.0], [65.0, 130.0]);
    vec![front, back, pocket]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_yoked_blouse_is_made_to_measure_itself() {
        for sideways in [false, true] {
            let pieces = yoked_blouse(sideways);
            let length = |p: &str, e: &str| {
                let piece = pieces.iter().find(|x| x.id == p).unwrap();
                piece
                    .outline
                    .edge_geom(piece.outline.edge_index(e).unwrap())
                    .length()
            };
            // Shoulders, sides and the front's gathers to the yoke.
            assert!((length("yoke", "yback") - length("back", "bshr")).abs() < 0.01);
            assert!((length("back", "bsider") - length("right", "fside")).abs() < 0.01);
            assert!((length("back", "bsidel") - length("left", "fside")).abs() < 0.01);
            let gathered = length("right", "ftop") / length("yoke", "yfront");
            assert!((1.3..1.7).contains(&gathered), "{gathered}");
            // The sleeve's head round the armhole, eased a little.
            let armhole =
                length("back", "barmr") + length("yoke", "yarm") + length("right", "farm");
            let head = length("sleeve", "shead");
            assert!(
                (head / armhole - 1.0).abs() < 0.04,
                "{head} round {armhole}"
            );
            // Every outline clockwise on screen.
            for p in &pieces {
                assert!(
                    crate::validate::validate_piece(p)
                        .iter()
                        .all(|i| i.severity != crate::Severity::Error),
                    "{}: {:?}",
                    p.id,
                    crate::validate::validate_piece(p)
                );
            }
        }
    }
}
