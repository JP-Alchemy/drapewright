//! Garments laid out and sewn, ready to drape.

mod common;

use common::skirt;
use drapewright_sim::garment::{Copy, Half, assemble};

fn along(points: &[[f64; 2]], indices: &[u32], upto: usize) -> f64 {
    indices[..=upto]
        .windows(2)
        .map(|w| {
            let (a, b) = (points[w[0] as usize], points[w[1] as usize]);
            (b[0] - a[0]).hypot(b[1] - a[1])
        })
        .sum()
}

#[test]
fn a_skirt_is_unfolded_paired_and_sewn() {
    let (pieces, seams) = skirt();
    let g = assemble(&pieces, &seams, 20.0).unwrap();
    let copies: Vec<(&str, Copy)> = g
        .panels
        .iter()
        .map(|p| (p.piece.as_str(), p.copy))
        .collect();
    assert_eq!(
        copies,
        [
            ("front", Copy::Unfolded),
            ("back", Copy::Original),
            ("back", Copy::Mirror)
        ]
    );
    // The front unfolded has both side seams; up the panel is up.
    let front = &g.panels[0];
    assert!(front.edge("fs", Half::A).is_some() && front.edge("fs", Half::B).is_some());
    assert!(
        front.edge("fcf", Half::A).is_none(),
        "the fold is inside the panel"
    );
    let waist = front.edge("fw", Half::A).unwrap();
    let hem = front.edge("fh", Half::A).unwrap();
    let height = |e: &drapewright_sim::garment::PanelEdge| {
        e.points
            .iter()
            .map(|&i| front.mesh.points[i as usize][1])
            .sum::<f64>()
            / e.points.len() as f64
    };
    assert!(height(waist) > height(hem) + 500.0);
    // Side seams on both sides (front half A to the back, half B to its
    // mirror), the centre back once: three seams.
    assert_eq!(g.links.len(), 3);
    // Each stitch joins points as far along their edges.
    for link in &g.links {
        let (pa, ea, ha) = &link.a;
        let (pb, eb, hb) = &link.b;
        let a = g.panels[*pa].edge(ea, *ha).unwrap();
        let b = g.panels[*pb].edge(eb, *hb).unwrap();
        assert_eq!(a.points.len(), b.points.len(), "{ea} and {eb}");
        let la = along(&g.panels[*pa].mesh.points, &a.points, a.points.len() - 1);
        let lb = along(&g.panels[*pb].mesh.points, &b.points, b.points.len() - 1);
        for k in 0..a.points.len() {
            let sa = along(&g.panels[*pa].mesh.points, &a.points, k) / la;
            let sb = along(&g.panels[*pb].mesh.points, &b.points, k) / lb;
            assert!((sa - sb).abs() < 0.02, "{ea} {k}: {sa} {sb}");
        }
    }
    let side_points = g.panels[0].edge("fs", Half::A).unwrap().points.len();
    let back_points = g.panels[1].edge("bcb", Half::A).unwrap().points.len();
    assert_eq!(g.stitches.len(), 2 * side_points + back_points);
    // Unfolded, the front is twice the drawn piece.
    let area = |p: &drapewright_sim::garment::Panel| -> f64 {
        p.mesh
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| p.mesh.points[i as usize]);
                ((b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1])) / 2.0
            })
            .sum()
    };
    assert!((area(&g.panels[0]) / area(&g.panels[1]) - 2.0).abs() < 0.05);
}
