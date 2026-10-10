//! What the app draws for a piece: derived lines and measurements, computed
//! once per piece version and cached.

use kurbo::Point;
use serde::{Deserialize, Serialize};
use soom_pattern_core::allowance::{AllowanceNote, allowance_lines};
use soom_pattern_core::{AllowanceMode, EdgeKind, Id, Issue, Piece, Pt, validate_piece};
use ts_rs::TS;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct EdgeView {
    pub id: Id,
    pub name: String,
    pub kind: EdgeKind,
    /// Length along the outline, mm.
    pub length: f64,
    /// Allowance that applies to this edge, mm.
    pub allowance: f64,
    pub start: Pt,
    pub end: Pt,
    /// Halfway along the edge, and the outward normal there (for labels).
    pub middle: Pt,
    pub middle_normal: Pt,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PieceView {
    pub id: Id,
    /// Cutting line as a flat list `x0, y0, x1, y1, …` in piece
    /// coordinates (mm), closed.
    pub cutting: Vec<f64>,
    /// Stitching line, same layout.
    pub stitching: Vec<f64>,
    /// True when the outline is the cutting line (allowance included).
    pub outline_is_cutting: bool,
    pub edges: Vec<EdgeView>,
    /// Bounds of the cutting line in piece coordinates: x0, y0, x1, y1.
    pub bounds: [f64; 4],
    /// Bounds of the outline as drawn (the piece's own size), same layout.
    pub outline_bounds: [f64; 4],
    /// Area inside the stitching line, mm².
    pub area: f64,
    pub allowance_notes: Vec<AllowanceNote>,
    pub issues: Vec<Issue>,
}

fn flat(points: &[Point]) -> Vec<f64> {
    points.iter().flat_map(|p| [p.x, p.y]).collect()
}

impl PieceView {
    pub fn of(piece: &Piece) -> Self {
        let lines = allowance_lines(piece);
        let outline = &piece.outline;
        let edges = (0..outline.edges.len())
            .map(|i| {
                let geom = outline.edge_geom(i);
                let len = geom.length();
                let n = geom.normal_at(len / 2.0);
                let e = &outline.edges[i];
                EdgeView {
                    id: e.id.clone(),
                    name: e.name.clone(),
                    kind: e.kind,
                    length: len,
                    allowance: piece.edge_allowance(i),
                    start: outline.edge_start(i),
                    end: outline.edge_end(i),
                    middle: Pt::from(geom.point_at(len / 2.0)),
                    middle_normal: Pt::new(n.x, n.y),
                }
            })
            .collect();
        let bounds = lines
            .cutting
            .iter()
            .fold([f64::MAX, f64::MAX, f64::MIN, f64::MIN], |b, p| {
                [b[0].min(p.x), b[1].min(p.y), b[2].max(p.x), b[3].max(p.y)]
            });
        let area = soom_pattern_core::geom::polygon_area(&lines.stitching).abs();
        let ob = outline.bounds();
        PieceView {
            id: piece.id.clone(),
            cutting: flat(&lines.cutting),
            stitching: flat(&lines.stitching),
            outline_is_cutting: piece.allowance.mode == AllowanceMode::Included,
            edges,
            bounds,
            outline_bounds: [ob.x0, ob.y0, ob.x1, ob.y1],
            area,
            allowance_notes: lines.notes,
            issues: validate_piece(piece),
        }
    }

    /// The cutting line as points.
    pub fn cutting_points(&self) -> Vec<Point> {
        self.cutting
            .chunks_exact(2)
            .map(|c| Point::new(c[0], c[1]))
            .collect()
    }
}
