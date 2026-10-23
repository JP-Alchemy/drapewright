//! Which way up a piece is worn. Pattern CAD lays long pieces on their side
//! to fit the sheet; the drape stands each upright, and Find seams sews a
//! pocket on with its opening at the top.

use crate::model::{EdgeKind, Piece};
use crate::stitching::stitching_edges;

type P2 = [f64; 2];

/// `p` turned a quarter `quarters` times, counter-clockwise (y up).
pub fn quarter_turned(p: P2, quarters: u8) -> P2 {
    match quarters % 4 {
        0 => p,
        1 => [-p[1], p[0]],
        2 => [-p[0], -p[1]],
        _ => [p[1], -p[0]],
    }
}

/// A hem's allowance is at least this much more than a seam's.
const HEM_DEEPER: f64 = 1.5;

fn length(points: &[P2]) -> f64 {
    points
        .windows(2)
        .map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]))
        .sum()
}

/// The point `s` mm along a polyline.
fn at_length(points: &[P2], s: f64) -> P2 {
    let mut walked = 0.0;
    for w in points.windows(2) {
        let l = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if walked + l >= s && l > 0.0 {
            let t = ((s - walked) / l).clamp(0.0, 1.0);
            return [
                w[0][0] + t * (w[1][0] - w[0][0]),
                w[0][1] + t * (w[1][1] - w[0][1]),
            ];
        }
        walked += l;
    }
    *points.last().unwrap_or(&[0.0, 0.0])
}

/// How many quarter turns, counter-clockwise (y up: the pattern's y turned
/// over), stand a piece upright as worn: its grain up and down the body and
/// its hems at the bottom. A piece is taken as drawn unless its grain or
/// its hems say otherwise; one lying on its side whose hems don't say which
/// way is up is stood with its grain's arrow (at `to`) pointing down, as
/// CAD draws it.
pub fn upright_turns(piece: &Piece) -> u8 {
    let lines: Vec<Vec<P2>> = stitching_edges(piece)
        .into_iter()
        .map(|e| e.into_iter().map(|p| [p.x, -p.y]).collect())
        .collect();
    let grain = piece.grainline.as_ref().map(|g| {
        let d = [g.to.x - g.from.x, -(g.to.y - g.from.y)];
        let l = d[0].hypot(d[1]).max(1e-9);
        [d[0] / l, d[1] / l]
    });
    // Hems: marked as such, or given a deeper allowance than the seams.
    let seam = piece.allowance.seam.max(1.0);
    let hems: Vec<P2> = (0..lines.len())
        .filter(|&e| {
            piece.outline.edges[e].kind == EdgeKind::Hem
                || (piece.outline.edges[e].kind == EdgeKind::Seam
                    && piece.edge_allowance(e) >= HEM_DEEPER * seam
                    && piece.edge_allowance(e) >= 15.0)
        })
        .map(|e| at_length(&lines[e], length(&lines[e]) / 2.0))
        .collect();
    let all: Vec<P2> = lines.iter().flatten().copied().collect();
    let score = |quarters: u8| {
        let mut score = if quarters == 0 { 1.0 } else { 0.0 };
        if let Some(g) = grain {
            let g = quarter_turned(g, quarters);
            if g[1].abs() > std::f64::consts::FRAC_1_SQRT_2 {
                score += 2.0;
            }
            if g[1] < -0.5 {
                score += 0.75;
            }
        }
        let ys: Vec<f64> = all
            .iter()
            .map(|&p| quarter_turned(p, quarters)[1])
            .collect();
        let (low, high) = ys
            .iter()
            .fold((f64::MAX, f64::MIN), |(l, h), &y| (l.min(y), h.max(y)));
        if high > low {
            for &hem in &hems {
                let height = (quarter_turned(hem, quarters)[1] - low) / (high - low);
                score += 2.0 * (1.0 - 2.0 * height);
            }
        }
        score
    };
    (0..4u8)
        .max_by(|&a, &b| score(a).total_cmp(&score(b)).then(b.cmp(&a)))
        .unwrap_or(0)
}

/// Up the body, on the pattern as drawn (y down), as a unit vector: the way
/// a piece stood upright as worn (`upright_turns`) points up.
pub fn up_on_page(piece: &Piece) -> P2 {
    // Up, stood upright, is +y; drawn, it is that turned back, y turned
    // over again.
    let [x, y] = quarter_turned([0.0, 1.0], 4 - upright_turns(piece) % 4);
    [x, -y]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Edge, Grainline, Outline, Pt, Segment};

    fn rectangle(w: f64, h: f64) -> Piece {
        Piece::new(
            "p",
            "Panel",
            Outline {
                start: Pt::new(0.0, 0.0),
                edges: vec![
                    Edge::new("top", vec![Segment::line([w, 0.0])]),
                    Edge::new("right", vec![Segment::line([w, h])]),
                    Edge::new("bottom", vec![Segment::line([0.0, h])]),
                    Edge::new("left", vec![Segment::line([0.0, 0.0])]),
                ],
            },
        )
    }

    #[test]
    fn a_piece_drawn_upright_points_up_the_page() {
        let mut p = rectangle(200.0, 500.0);
        p.grainline = Some(Grainline {
            from: Pt::new(100.0, 100.0),
            to: Pt::new(100.0, 400.0),
            one_way: false,
        });
        assert_eq!(upright_turns(&p), 0);
        let up = up_on_page(&p);
        assert!(up[0].abs() < 1e-9 && (up[1] + 1.0).abs() < 1e-9, "{up:?}");
    }

    #[test]
    fn a_piece_on_its_side_points_up_against_its_grain_arrow() {
        // Lying on its side, the grain's arrow pointing right: worn, it
        // points down, so up is to the left on the page.
        let mut p = rectangle(500.0, 200.0);
        p.grainline = Some(Grainline {
            from: Pt::new(100.0, 100.0),
            to: Pt::new(400.0, 100.0),
            one_way: false,
        });
        let up = up_on_page(&p);
        assert!((up[0] + 1.0).abs() < 1e-9 && up[1].abs() < 1e-9, "{up:?}");
    }
}
