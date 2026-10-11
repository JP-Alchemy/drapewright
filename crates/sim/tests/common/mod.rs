//! Garments for the tests.

use drapewright_pattern_core::{
    CutInstruction, Edge, EdgeKind, Grainline, Outline, Piece, Pt, Segment,
};
use drapewright_seams::{Seam, SeamSide};

fn edge(id: &str, kind: EdgeKind, segment: Segment) -> Edge {
    Edge {
        kind,
        ..Edge::new(id, vec![segment])
    }
}

fn piece(id: &str, name: &str, edges: Vec<Edge>, quantity: u32) -> Piece {
    let mut p = Piece::new(
        id,
        name,
        Outline {
            start: Pt::new(0.0, 0.0),
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

/// A skirt for the standard figure (waist 71 cm, hip 99 cm): the front cut
/// on the fold, the back cut as a pair with a centre back seam.
pub fn skirt() -> (Vec<Piece>, Vec<Seam>) {
    use EdgeKind::*;
    let front = piece(
        "front",
        "Skirt front",
        vec![
            edge("fw", Seam, Segment::line([185.0, 8.0])),
            edge(
                "fs",
                Seam,
                Segment::cubic([215.0, 80.0], [255.0, 200.0], [290.0, 600.0]),
            ),
            edge(
                "fh",
                Hem,
                Segment::cubic([190.0, 608.0], [95.0, 612.0], [0.0, 612.0]),
            ),
            edge("fcf", Fold, Segment::line([0.0, 0.0])),
        ],
        1,
    );
    let back = piece(
        "back",
        "Skirt back",
        vec![
            edge("bw", Seam, Segment::line([185.0, 12.0])),
            edge(
                "bs",
                Seam,
                Segment::cubic([215.0, 84.0], [255.0, 204.0], [290.0, 604.0]),
            ),
            edge(
                "bh",
                Hem,
                Segment::cubic([190.0, 612.0], [95.0, 616.0], [0.0, 616.0]),
            ),
            edge("bcb", Seam, Segment::line([0.0, 0.0])),
        ],
        2,
    );
    let seam = |id: &str, a: SeamSide, b: SeamSide| drapewright_seams::Seam {
        id: id.into(),
        name: String::new(),
        a,
        b,
        reversed: false,
        ease: Default::default(),
        confirmed: true,
    };
    let seams = vec![
        seam(
            "s1",
            SeamSide::whole("front", "fs"),
            SeamSide::whole("back", "bs"),
        ),
        seam(
            "s2",
            SeamSide::whole("back", "bcb"),
            SeamSide {
                mirror: true,
                ..SeamSide::whole("back", "bcb")
            },
        ),
    ];
    (vec![front, back], seams)
}

/// A loose blouse with short sleeves for the standard figure (bust
/// 89.5 cm): the front and back each cut on the fold, sewn at the
/// shoulders and sides, and a sleeve cut as a pair, sewn up its underarm
/// and into the armholes, the back half of its cap to the back, each half
/// a centimetre or so longer than its armhole (eased in).
#[allow(dead_code)] // Not every test drapes it.
pub fn blouse() -> (Vec<Piece>, Vec<Seam>) {
    use EdgeKind::*;
    let mut front = piece(
        "front",
        "Blouse front",
        vec![
            edge(
                "fn",
                Seam,
                Segment::cubic([40.0, 80.0], [68.0, 40.0], [70.0, 0.0]),
            ),
            edge("fsh", Seam, Segment::line([174.0, 42.0])),
            edge(
                "fa",
                Seam,
                Segment::cubic([168.0, 100.0], [185.0, 215.0], [250.0, 215.0]),
            ),
            edge("fs", Seam, Segment::line([255.0, 600.0])),
            edge("fh", Hem, Segment::line([0.0, 600.0])),
            edge("fcf", Fold, Segment::line([0.0, 80.0])),
        ],
        1,
    );
    front.outline.start = Pt::new(0.0, 80.0);
    let mut back = piece(
        "back",
        "Blouse back",
        vec![
            edge(
                "bn",
                Seam,
                Segment::cubic([40.0, 20.0], [70.0, 12.0], [72.0, 0.0]),
            ),
            edge("bsh", Seam, Segment::line([178.0, 35.0])),
            edge(
                "ba",
                Seam,
                Segment::cubic([172.0, 110.0], [190.0, 212.0], [250.0, 215.0]),
            ),
            edge("bs", Seam, Segment::line([255.0, 600.0])),
            edge("bh", Hem, Segment::line([0.0, 600.0])),
            edge("bcb", Fold, Segment::line([0.0, 20.0])),
        ],
        1,
    );
    back.outline.start = Pt::new(0.0, 20.0);
    let mut sleeve = piece(
        "sleeve",
        "Sleeve",
        vec![
            Edge {
                kind: Seam,
                ..Edge::new(
                    "sc",
                    vec![
                        Segment::cubic([-125.0, 130.0], [-80.0, 0.0], [0.0, 0.0]),
                        Segment::cubic([80.0, 0.0], [125.0, 130.0], [162.0, 135.0]),
                    ],
                )
            },
            edge("sr", Seam, Segment::line([150.0, 215.0])),
            edge("sh", Hem, Segment::line([-150.0, 215.0])),
            edge("sl", Seam, Segment::line([-162.0, 135.0])),
        ],
        2,
    );
    sleeve.outline.start = Pt::new(-162.0, 135.0);
    sleeve.grainline = Some(Grainline {
        from: Pt::new(0.0, 40.0),
        to: Pt::new(0.0, 180.0),
        one_way: false,
    });
    let cap = sleeve
        .outline
        .edge_geom(sleeve.outline.edge_index("sc").unwrap())
        .length();
    let seam = |id: &str, a: SeamSide, b: SeamSide, reversed: bool| drapewright_seams::Seam {
        id: id.into(),
        name: String::new(),
        a,
        b,
        reversed,
        ease: Default::default(),
        confirmed: true,
    };
    let part = |from: f64, to: f64| SeamSide {
        from: Some(from),
        to: Some(to),
        ..SeamSide::whole("sleeve", "sc")
    };
    let seams = vec![
        seam(
            "shoulder",
            SeamSide::whole("front", "fsh"),
            SeamSide::whole("back", "bsh"),
            false,
        ),
        seam(
            "side",
            SeamSide::whole("front", "fs"),
            SeamSide::whole("back", "bs"),
            false,
        ),
        seam(
            "underarm",
            SeamSide::whole("sleeve", "sl"),
            SeamSide::whole("sleeve", "sr"),
            true,
        ),
        seam(
            "armhole back",
            part(0.0, cap / 2.0),
            SeamSide::whole("back", "ba"),
            true,
        ),
        seam(
            "armhole front",
            part(cap / 2.0, cap),
            SeamSide::whole("front", "fa"),
            false,
        ),
    ];
    (vec![front, back, sleeve], seams)
}

/// The blouse with sleeves to the wrist, 22 cm round there.
#[allow(dead_code)] // Not every test drapes it.
pub fn long_sleeved_blouse() -> (Vec<Piece>, Vec<Seam>) {
    let (mut pieces, seams) = blouse();
    let sleeve = pieces.iter_mut().find(|p| p.id == "sleeve").unwrap();
    for edge in &mut sleeve.outline.edges {
        match edge.id.as_str() {
            "sr" => edge.segments = vec![Segment::line([110.0, 590.0])],
            "sh" => edge.segments = vec![Segment::line([-110.0, 590.0])],
            _ => {}
        }
    }
    (pieces, seams)
}
