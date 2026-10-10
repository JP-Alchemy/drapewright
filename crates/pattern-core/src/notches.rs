//! Where notches go and how they are drawn: a notch sits on the outline at
//! a distance along its edge and is marked where its line crosses the
//! cutting line.

use kurbo::{Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Pt;
use crate::geom::{SAME_POINT, ray_hit};
use crate::model::{AllowanceMode, Id, Notch, NotchFrom, Piece};

/// How notches are drawn on the cutting line.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum NotchStyle {
    /// A short cut into the allowance.
    #[default]
    Slit,
    /// A small triangle outside the cutting line.
    V,
}

/// Length of a slit notch, mm (the usual 1/4 inch).
pub const SLIT_LENGTH: f64 = 6.0;
/// Size of a V notch, mm.
pub const V_SIZE: f64 = 4.0;

/// One drawn mark of a notch (a double notch has two).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct NotchMark {
    pub notch: Id,
    /// On the outline.
    pub on_outline: Pt,
    /// Where the notch line crosses the cutting line.
    pub on_cut: Pt,
    /// Unit vector pointing out of the piece.
    pub outward: Pt,
    /// Lines to draw: `[x0, y0, x1, y1]` each. A slit is one line; a V is
    /// two.
    pub lines: Vec<[f64; 4]>,
}

/// Distance of a notch from the start of its edge, mm.
pub fn notch_along(notch: &Notch, edge_length: f64) -> f64 {
    match notch.from {
        NotchFrom::Start => notch.at,
        NotchFrom::End => edge_length - notch.at,
    }
}

/// The marks for every notch of a piece, given its cutting line.
pub fn notch_marks(
    piece: &Piece,
    cutting: &[Point],
    style: NotchStyle,
    spacing: f64,
) -> Vec<NotchMark> {
    let outline = &piece.outline;
    let mut marks = Vec::new();
    for notch in &piece.notches {
        let Some(i) = outline.edge_index(&notch.edge) else {
            continue;
        };
        let geom = outline.edge_geom(i);
        let len = geom.length();
        let centre = notch_along(notch, len);
        let count = notch.kind.count();
        for k in 0..count {
            // Marks spread symmetrically about the notch position.
            let offset = (k as f64 - (count - 1) as f64 / 2.0) * spacing;
            let along = (centre + offset).clamp(0.0, len);
            let p = geom.point_at(along);
            let normal = geom.normal_at(along);
            let outward = if notch.angle.abs() < 1e-12 {
                normal
            } else {
                // Tilted away from perpendicular, as the notch says.
                Vec2::from_angle(normal.atan2() + notch.angle.to_radians())
            };
            let on_cut = if piece.allowance.mode == AllowanceMode::Included {
                p
            } else {
                ray_hit(p, outward, cutting).unwrap_or(p)
            };
            let depth = on_cut.distance(p);
            let lines = match style {
                NotchStyle::Slit => {
                    // Into the allowance from the cutting line, never past
                    // the stitching line; into the piece if there is none.
                    let reach = if depth > SAME_POINT {
                        SLIT_LENGTH.min(depth * 0.6)
                    } else {
                        SLIT_LENGTH
                    };
                    let inner = on_cut - outward * reach;
                    vec![[on_cut.x, on_cut.y, inner.x, inner.y]]
                }
                NotchStyle::V => {
                    let side = outward.turn_90() * (V_SIZE / 2.0);
                    let tip = on_cut + outward * V_SIZE;
                    let (l, r) = (on_cut + side, on_cut - side);
                    vec![[l.x, l.y, tip.x, tip.y], [tip.x, tip.y, r.x, r.y]]
                }
            };
            marks.push(NotchMark {
                notch: notch.id.clone(),
                on_outline: Pt::from(p),
                on_cut: Pt::from(on_cut),
                outward: Pt::new(outward.x, outward.y),
                lines,
            });
        }
    }
    marks
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::allowance::allowance_lines;
    use crate::model::{Edge, NotchKind, Outline, Segment};

    fn rect_with_notch(kind: NotchKind, from: NotchFrom, at: f64) -> Piece {
        let outline = Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("top", vec![Segment::line([200.0, 0.0])]),
                Edge::new("right", vec![Segment::line([200.0, 100.0])]),
                Edge::new("bottom", vec![Segment::line([0.0, 100.0])]),
                Edge::new("left", vec![Segment::line([0.0, 0.0])]),
            ],
        };
        let mut p = Piece::new("p1", "Test", outline);
        p.allowance.seam = 10.0;
        p.notches.push(Notch {
            id: "n1".into(),
            edge: "top".into(),
            at,
            from,
            kind,
            angle: 0.0,
        });
        p
    }

    #[test]
    fn a_slit_runs_into_the_allowance_from_the_cutting_line() {
        let p = rect_with_notch(NotchKind::Single, NotchFrom::Start, 50.0);
        let cut = allowance_lines(&p).cutting;
        let marks = notch_marks(&p, &cut, NotchStyle::Slit, 6.0);
        assert_eq!(marks.len(), 1);
        let m = &marks[0];
        assert_eq!(m.on_outline, Pt::new(50.0, 0.0));
        assert!((m.on_cut.y + 10.0).abs() < 1e-9 && (m.on_cut.x - 50.0).abs() < 1e-9);
        // 6 mm long, inside the allowance.
        let [x0, y0, x1, y1] = m.lines[0];
        assert!(((x1 - x0).hypot(y1 - y0) - 6.0).abs() < 1e-9);
        assert!(y0 < y1 && y1 < 0.0);
    }

    #[test]
    fn double_notches_straddle_their_position() {
        let p = rect_with_notch(NotchKind::Double, NotchFrom::End, 50.0);
        let cut = allowance_lines(&p).cutting;
        let marks = notch_marks(&p, &cut, NotchStyle::Slit, 6.0);
        let xs: Vec<f64> = marks.iter().map(|m| m.on_outline.x).collect();
        // 50 mm from the end of a 200 mm edge is x = 150; marks 6 mm apart.
        assert_eq!(xs, vec![147.0, 153.0]);
    }

    #[test]
    fn v_notches_point_out_of_the_piece() {
        let p = rect_with_notch(NotchKind::Single, NotchFrom::Start, 100.0);
        let cut = allowance_lines(&p).cutting;
        let marks = notch_marks(&p, &cut, NotchStyle::V, 6.0);
        let tip_y = marks[0].lines[0][3];
        assert!((tip_y + 14.0).abs() < 1e-9, "{tip_y}");
    }
}
