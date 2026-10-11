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
