//! Garments laid out and sewn, ready to drape.

mod common;

use std::collections::{HashMap, HashSet};

use common::{blouse, full_bust_blouse, skirt};
use drapewright_sim::garment::{Copy, Half, assemble};
use drapewright_sim::mesh::PanelMesh;

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

/// Points as whole hundredths of a millimetre, to find them again.
fn key(p: [f64; 2]) -> (i64, i64) {
    ((p[0] * 100.0).round() as i64, (p[1] * 100.0).round() as i64)
}

/// The triangles of `b` that are not `a`'s reflected in x = 0.
fn not_reflected(a: &PanelMesh, b: &PanelMesh) -> usize {
    let at: HashMap<(i64, i64), u32> = b
        .points
        .iter()
        .enumerate()
        .map(|(i, &p)| (key(p), i as u32))
        .collect();
    let triangles: HashSet<[u32; 3]> = b
        .triangles
        .iter()
        .map(|t| {
            let mut t = *t;
            t.sort_unstable();
            t
        })
        .collect();
    a.triangles
        .iter()
        .filter(|t| {
            let reflected: Option<Vec<u32>> = t
                .iter()
                .map(|&i| {
                    let p = a.points[i as usize];
                    at.get(&key([-p[0], p[1]])).copied()
                })
                .collect();
            reflected.is_none_or(|mut r| {
                r.sort_unstable();
                !triangles.contains(&[r[0], r[1], r[2]])
            })
        })
        .count()
}

#[test]
fn both_sides_of_a_garment_are_meshed_alike() {
    // Cut alike on both sides, a garment must hang alike: a triangle's
    // diagonal one way on one side and the other way on the other makes
    // the two sides give differently, and it leans.
    for (pieces, seams) in [skirt(), blouse()] {
        let g = assemble(&pieces, &seams, 20.0).unwrap();
        for (k, panel) in g.panels.iter().enumerate() {
            match panel.copy {
                // About the fold (x = 0 here): each side the other's mirror.
                Copy::Unfolded => {
                    assert_eq!(not_reflected(&panel.mesh, &panel.mesh), 0, "{}", panel.name);
                }
                // Each of a pair the other's mirror.
                Copy::Mirror => {
                    let original = &g.panels[k - 1];
                    assert_eq!(original.copy, Copy::Original);
                    assert_eq!(
                        not_reflected(&original.mesh, &panel.mesh),
                        0,
                        "{}",
                        panel.name
                    );
                    assert_eq!(original.mesh.points.len(), panel.mesh.points.len());
                }
                Copy::Original => {}
            }
            // Counter-clockwise still, and every edge's points where they were sampled.
            for t in &panel.mesh.triangles {
                let [a, b, c] = t.map(|i| panel.mesh.points[i as usize]);
                let twice = (b[0] - a[0]) * (c[1] - a[1]) - (c[0] - a[0]) * (b[1] - a[1]);
                assert!(
                    twice > 0.0,
                    "{}: a triangle wound the wrong way",
                    panel.name
                );
            }
        }
    }
}

#[test]
fn a_dart_is_sewn_closed_and_its_seam_matched_past_it() {
    let (pieces, seams) = full_bust_blouse();
    // Measured with the dart closed, the front's side is as long as the
    // back's still: the adjustment turned it, and the dart took up the rest.
    let side = seams.iter().find(|s| s.id == "side").unwrap();
    let report = drapewright_seams::measure(side, &pieces);
    assert!(report.difference.abs() < 1.0, "{report:?}");
    let g = assemble(&pieces, &seams, 20.0).unwrap();
    // Only seams join panels: the darts sew the front to itself.
    assert_eq!(g.links.len(), 10);
    let (front, back) = (&g.panels[0], &g.panels[1]);
    let at = |panel: &drapewright_sim::garment::Panel, i: u32| panel.mesh.points[i as usize];
    let apart = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).hypot(p[1] - q[1]);
    for half in [Half::A, Half::B] {
        let side = front.edge("fs", half).unwrap();
        let on_side = |s: &(usize, u32)| s.0 == 0 && side.points.contains(&s.1);
        // The dart: its legs sewn point for point, from its mouth in to its
        // tip, each stitch shorter than the last.
        let dart: Vec<(u32, u32)> = g
            .stitches
            .iter()
            .filter(|[x, y]| on_side(x) && on_side(y))
            .map(|[x, y]| (x.1, y.1))
            .collect();
        let widths: Vec<f64> = dart
            .iter()
            .map(|&(p, q)| apart(at(front, p), at(front, q)))
            .collect();
        assert!(widths.len() >= 3, "{half:?}: {widths:?}");
        assert!(widths[0] > 10.0, "{half:?}: its mouth {widths:?}");
        assert!(
            widths.windows(2).all(|w| w[1] < w[0]),
            "{half:?}: {widths:?}"
        );
        // Every point of the side seam sewn, to the back or in the dart,
        // but the dart's tip, where its legs meet.
        let sewn: HashSet<u32> = g
            .stitches
            .iter()
            .flatten()
            .filter(|s| on_side(s))
            .map(|s| s.1)
            .collect();
        assert_eq!(sewn.len(), side.points.len() - 1, "{half:?}");
        // Sewn to the back as far along each side, the dart closed.
        let index = |i: u32| side.points.iter().position(|&p| p == i).unwrap();
        let (mouth0, mouth1) = (index(dart[0].0), index(dart[0].1));
        let (mouth0, mouth1) = (mouth0.min(mouth1), mouth0.max(mouth1));
        let front_points: Vec<[f64; 2]> = side.points.iter().map(|&i| at(front, i)).collect();
        let open = |k: usize| along(&front.mesh.points, &side.points, k);
        let closed = |k: usize| {
            let (a0, a1) = (open(mouth0), open(mouth1));
            let s = open(k);
            s - (s - a0).clamp(0.0, a1 - a0)
        };
        let front_total = closed(front_points.len() - 1);
        let back_side = back.edge("bs", half).unwrap();
        let back_total = along(
            &back.mesh.points,
            &back_side.points,
            back_side.points.len() - 1,
        );
        let mut matched = 0;
        for [x, y] in &g.stitches {
            let (f, b) = match (on_side(x), y.0 == 1 && back_side.points.contains(&y.1)) {
                (true, true) => (x.1, y.1),
                _ if on_side(y) && x.0 == 1 && back_side.points.contains(&x.1) => (y.1, x.1),
                _ => continue,
            };
            let k = back_side.points.iter().position(|&p| p == b).unwrap();
            let sf = closed(index(f)) / front_total;
            let sb = along(&back.mesh.points, &back_side.points, k) / back_total;
            assert!(
                (sf - sb).abs() < 0.01,
                "{half:?}: {sf} on the front, {sb} on the back"
            );
            matched += 1;
        }
        // The back's point where the dart closes sewn to both its legs.
        assert_eq!(matched, back_side.points.len() + 1, "{half:?}");
    }
}

#[test]
fn a_blouse_is_sewn_into_its_armholes() {
    let (pieces, seams) = blouse();
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
            ("back", Copy::Unfolded),
            ("sleeve", Copy::Original),
            ("sleeve", Copy::Mirror)
        ]
    );
    // Shoulders, sides, underarms and both halves of each armhole, on
    // both sides of the body.
    assert_eq!(g.links.len(), 10);
    // Each sleeve's cap is sewn all along, half to the front and half to
    // the back: none of its points left unsewn but its ends' neighbours.
    for sleeve in [2, 3] {
        let cap = g.panels[sleeve]
            .edges
            .iter()
            .find(|e| e.edge == "sc")
            .unwrap();
        let sewn: HashSet<u32> = g
            .stitches
            .iter()
            .flat_map(|&[a, b]| [a, b])
            .filter(|&(p, _)| p == sleeve)
            .map(|(_, i)| i)
            .collect();
        assert!(cap.points.iter().all(|i| sewn.contains(i)));
    }
}

/// The made-up yoked blouse laid out as CAD lays it on the sheet, sewn by
/// the seams found for it: its pieces and seams as draped.
fn yoked() -> (
    Vec<drapewright_pattern_core::Piece>,
    Vec<drapewright_seams::Seam>,
) {
    let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(true);
    let seams: Vec<drapewright_seams::Seam> = drapewright_seams::suggest(&pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| drapewright_seams::Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    (pieces, seams)
}

#[test]
fn pieces_laid_on_their_side_are_stood_upright() {
    let (pieces, seams) = yoked();
    let sewn = drapewright_sim::garment::draped_pieces(&pieces, &seams).sewn;
    let g = assemble(&sewn, &seams, 20.0).unwrap();
    for panel in &g.panels {
        // Up the panel is up the body: the grain up and down it ...
        assert!(
            panel.grain[1].abs() > 0.99,
            "{} {:?}: grain {:?}",
            panel.piece,
            panel.copy,
            panel.grain
        );
        let low = panel
            .mesh
            .points
            .iter()
            .map(|p| p[1])
            .fold(f64::MAX, f64::min);
        // ... and hems at the bottom.
        for e in panel.edges.iter().filter(|e| e.edge.ends_with("hem")) {
            let mean = e
                .points
                .iter()
                .map(|&i| panel.mesh.points[i as usize][1])
                .sum::<f64>()
                / e.points.len() as f64;
            assert!(
                mean - low < 20.0,
                "{}'s {} {mean} above {low}",
                panel.piece,
                e.edge
            );
        }
    }
}

#[test]
fn facings_are_left_inside_and_fronts_buttoned() {
    let (pieces, seams) = yoked();
    let draped = drapewright_sim::garment::draped_pieces(&pieces, &seams);
    let sewn = draped.sewn;
    assert!(draped.alone.is_empty());
    assert_eq!(draped.inside, ["facing"]);
    assert!(sewn.iter().all(|p| p.id != "facing"));
    let g = assemble(&sewn, &seams, 20.0).unwrap();
    // Each of the left front's five buttons held to the right front's edge.
    let index = |id: &str| g.panels.iter().position(|p| p.piece == id).unwrap();
    let (left, right) = (index("left"), index("right"));
    let buttons = g
        .stitches
        .iter()
        .filter(|[(pa, _), (pb, _)]| (*pa == left && *pb == right) || (*pa == right && *pb == left))
        .count();
    assert_eq!(buttons, 5);
    let edge = &g.panels[right].edge("fedge", Half::A).unwrap().points;
    for [(pa, a), (_, b)] in g
        .stitches
        .iter()
        .filter(|[(pa, _), (pb, _)]| *pa == left && *pb == right)
    {
        assert_eq!(*pa, left);
        assert!(
            edge.contains(b),
            "button {a} held off the right front's edge"
        );
    }
}
