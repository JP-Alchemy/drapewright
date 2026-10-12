//! Made-up garments for tests across crates.

use crate::Pt;
use crate::model::{
    Allowance, AllowanceMode, CutInstruction, Edge, Grainline, InternalLine, LineKind, Mark,
    MarkKind, Notch, NotchFrom, NotchKind, Outline, Piece, Segment,
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
