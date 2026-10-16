//! The validation rules of `docs/file-format.md` §9 that apply within one
//! piece. Project-wide rules (unique ids across pieces) live in the project
//! crate.

use std::collections::HashSet;

use kurbo::{Line, ParamCurve, Point};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::geom::{SAME_POINT, flatten_segs};
use crate::model::{EdgeKind, Id, NotchFrom, Piece, Segment};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Severity {
    /// The piece cannot be used as it is.
    Error,
    /// The piece can be used, but something looks wrong.
    Warning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum IssueCode {
    NonFinite,
    NoEdges,
    EmptyEdge,
    ZeroLengthEdge,
    OpenOutline,
    WrongOrientation,
    SelfIntersection,
    FoldNotStraight,
    NegativeAllowance,
    NotchUnknownEdge,
    NotchOutOfRange,
    /// A dart refers to an edge that does not exist, or is not on it.
    DartOffEdge,
    DuplicateId,
    ZeroQuantity,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Issue {
    pub severity: Severity,
    pub code: IssueCode,
    pub piece: Id,
    /// The edge, notch or other item concerned, if any.
    pub item: Option<Id>,
    pub message: String,
}

/// Largest distance a fold edge may stray from a straight line, mm.
const FOLD_STRAIGHTNESS: f64 = 0.01;

/// Every id used inside a piece, including the piece's own.
pub fn piece_ids(piece: &Piece) -> impl Iterator<Item = &Id> {
    std::iter::once(&piece.id)
        .chain(piece.outline.edges.iter().map(|e| &e.id))
        .chain(piece.notches.iter().map(|n| &n.id))
        .chain(piece.darts.iter().map(|d| &d.id))
        .chain(piece.marks.iter().map(|m| &m.id))
        .chain(piece.lines.iter().map(|l| &l.id))
        .chain(piece.labels.iter().map(|l| &l.id))
}

pub fn validate_piece(piece: &Piece) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut push = |severity, code, item: Option<&Id>, message: String| {
        issues.push(Issue {
            severity,
            code,
            piece: piece.id.clone(),
            item: item.cloned(),
            message,
        });
    };
    let name = &piece.name;
    let outline = &piece.outline;

    if !all_points_finite(piece) {
        push(
            Severity::Error,
            IssueCode::NonFinite,
            None,
            format!("“{name}” has a point with no valid position."),
        );
        return issues;
    }

    let mut seen = HashSet::new();
    for id in piece_ids(piece) {
        if !seen.insert(id) {
            push(
                Severity::Error,
                IssueCode::DuplicateId,
                Some(id),
                format!("“{name}” uses the id “{id}” twice."),
            );
        }
    }

    if outline.edges.is_empty() {
        push(
            Severity::Error,
            IssueCode::NoEdges,
            None,
            format!("“{name}” has no outline."),
        );
        return issues;
    }

    let mut lengths = Vec::with_capacity(outline.edges.len());
    for (i, edge) in outline.edges.iter().enumerate() {
        if edge.segments.is_empty() {
            push(
                Severity::Error,
                IssueCode::EmptyEdge,
                Some(&edge.id),
                format!("An edge of “{name}” has no shape."),
            );
            lengths.push(0.0);
            continue;
        }
        let geom = outline.edge_geom(i);
        let len = geom.length();
        lengths.push(len);
        if len <= SAME_POINT {
            push(
                Severity::Error,
                IssueCode::ZeroLengthEdge,
                Some(&edge.id),
                format!("An edge of “{name}” has no length."),
            );
        }
        if edge.allowance.is_some_and(|a| a < 0.0) {
            push(
                Severity::Error,
                IssueCode::NegativeAllowance,
                Some(&edge.id),
                format!("An edge of “{name}” has a negative seam allowance."),
            );
        }
        if edge.kind == EdgeKind::Fold && !is_straight(outline.edge_start(i), &edge.segments) {
            push(
                Severity::Error,
                IssueCode::FoldNotStraight,
                Some(&edge.id),
                format!(
                    "The fold edge of “{name}” is not straight, so it cannot be placed on a fold."
                ),
            );
        }
    }

    let gap = outline.closure_gap();
    if gap > SAME_POINT {
        push(
            Severity::Error,
            IssueCode::OpenOutline,
            None,
            format!(
                "The outline of “{name}” does not close: its end is {gap:.2} mm from its start."
            ),
        );
    } else {
        if outline.signed_area() <= 0.0 {
            push(
                Severity::Error,
                IssueCode::WrongOrientation,
                None,
                format!("The outline of “{name}” runs anticlockwise."),
            );
        }
        if self_intersects(&flatten_segs(&outline.path_segs(), 0.05)) {
            push(
                Severity::Warning,
                IssueCode::SelfIntersection,
                None,
                format!("The outline of “{name}” crosses itself. Seam allowance may look wrong."),
            );
        }
    }

    if piece.allowance.seam < 0.0 || piece.allowance.hem.is_some_and(|h| h < 0.0) {
        push(
            Severity::Error,
            IssueCode::NegativeAllowance,
            None,
            format!("“{name}” has a negative seam allowance."),
        );
    }
    for cut in &piece.cut {
        if cut.quantity == 0 {
            push(
                Severity::Warning,
                IssueCode::ZeroQuantity,
                None,
                format!("“{name}” says to cut 0 pieces."),
            );
        }
    }

    for notch in &piece.notches {
        match outline.edge_index(&notch.edge) {
            None => push(
                Severity::Error,
                IssueCode::NotchUnknownEdge,
                Some(&notch.id),
                format!("A notch on “{name}” refers to an edge that does not exist."),
            ),
            Some(i) => {
                let len = lengths[i];
                if !(0.0..=len + SAME_POINT).contains(&notch.at) {
                    let side = if notch.from == NotchFrom::Start {
                        "start"
                    } else {
                        "end"
                    };
                    push(
                        Severity::Error,
                        IssueCode::NotchOutOfRange,
                        Some(&notch.id),
                        format!(
                            "A notch on “{name}” is {:.1} mm from the {side} of an edge that is only {len:.1} mm long.",
                            notch.at
                        ),
                    );
                }
            }
        }
    }
    for dart in &piece.darts {
        let fits = outline.edge_index(&dart.edge).is_some_and(|i| {
            0.0 <= dart.from && dart.from < dart.to && dart.to <= lengths[i] + SAME_POINT
        });
        if !fits {
            push(
                Severity::Error,
                IssueCode::DartOffEdge,
                Some(&dart.id),
                format!("A dart on “{name}” is not where its edge is."),
            );
        }
    }
    issues
}

fn all_points_finite(piece: &Piece) -> bool {
    let seg_ok = |s: &Segment| match s {
        Segment::Line(l) => l.to.is_finite(),
        Segment::Cubic(c) => c.c1.is_finite() && c.c2.is_finite() && c.to.is_finite(),
    };
    piece.outline.start.is_finite()
        && piece
            .outline
            .edges
            .iter()
            .all(|e| e.segments.iter().all(seg_ok) && e.allowance.is_none_or(f64::is_finite))
        && piece
            .notches
            .iter()
            .all(|n| n.at.is_finite() && n.angle.is_finite())
        && piece
            .grainline
            .is_none_or(|g| g.from.is_finite() && g.to.is_finite())
        && piece.marks.iter().all(|m| m.at.is_finite())
        && piece
            .lines
            .iter()
            .all(|l| l.start.is_finite() && l.segments.iter().all(seg_ok))
        && piece.labels.iter().all(|l| l.at.is_finite())
        && piece
            .source
            .iter()
            .flat_map(|s| &s.words)
            .all(|w| w.at.is_finite() && w.size.is_finite() && w.angle.is_finite())
        && piece.allowance.seam.is_finite()
}

/// Whether all points of a run of segments, control points included, lie
/// on the line from its start to its end.
fn is_straight(start: crate::Pt, segments: &[Segment]) -> bool {
    let Some(end) = segments.last().map(Segment::to) else {
        return false;
    };
    let chord = Line::new(start, end);
    let dir = chord.p1 - chord.p0;
    let len = dir.hypot();
    if len <= SAME_POINT {
        return false;
    }
    let off_line = |p: crate::Pt| (Point::from(p) - chord.p0).cross(dir).abs() / len;
    segments.iter().all(|s| match s {
        Segment::Line(l) => off_line(l.to) <= FOLD_STRAIGHTNESS,
        Segment::Cubic(c) => [c.c1, c.c2, c.to]
            .into_iter()
            .all(|p| off_line(p) <= FOLD_STRAIGHTNESS),
    })
}

/// Whether a closed polyline (last point joins the first) crosses itself.
/// Touching at shared vertices of neighbouring segments does not count.
pub fn self_intersects(points: &[Point]) -> bool {
    let mut pts: Vec<Point> = points.to_vec();
    if pts.len() > 1 && pts[0].distance(pts[pts.len() - 1]) < SAME_POINT {
        pts.pop();
    }
    let n = pts.len();
    if n < 4 {
        return false;
    }
    let segs: Vec<Line> = (0..n)
        .map(|i| Line::new(pts[i], pts[(i + 1) % n]))
        .collect();
    // Sweep along x: only segments whose x-ranges overlap can cross.
    let mut order: Vec<usize> = (0..n).collect();
    let min_x = |i: usize| segs[i].p0.x.min(segs[i].p1.x);
    let max_x = |i: usize| segs[i].p0.x.max(segs[i].p1.x);
    order.sort_by(|&a, &b| min_x(a).total_cmp(&min_x(b)));
    let mut active: Vec<usize> = Vec::new();
    for &i in &order {
        active.retain(|&j| max_x(j) >= min_x(i));
        for &j in &active {
            let adjacent = (i + 1) % n == j || (j + 1) % n == i;
            if !adjacent && segments_cross(segs[i], segs[j]) {
                return true;
            }
        }
        active.push(i);
    }
    false
}

fn segments_cross(a: Line, b: Line) -> bool {
    let orient = |p: Point, q: Point, r: Point| (q - p).cross(r - p);
    let d1 = orient(a.p0, a.p1, b.p0);
    let d2 = orient(a.p0, a.p1, b.p1);
    let d3 = orient(b.p0, b.p1, a.p0);
    let d4 = orient(b.p0, b.p1, a.p1);
    d1 * d2 < 0.0 && d3 * d4 < 0.0 && a.start() != b.start()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Pt;
    use crate::model::{Edge, Notch, NotchKind, Outline};

    fn piece(edges: Vec<Edge>) -> Piece {
        Piece::new(
            "p1",
            "Test",
            Outline {
                start: Pt::new(0.0, 0.0),
                edges,
            },
        )
    }

    fn codes(p: &Piece) -> Vec<IssueCode> {
        validate_piece(p).into_iter().map(|i| i.code).collect()
    }

    #[test]
    fn a_clean_rectangle_is_valid() {
        let p = piece(vec![
            Edge::new("a", vec![Segment::line([100.0, 0.0])]),
            Edge::new(
                "b",
                vec![Segment::line([100.0, 50.0]), Segment::line([0.0, 50.0])],
            ),
            Edge::new("c", vec![Segment::line([0.0, 0.0])]),
        ]);
        assert_eq!(codes(&p), []);
    }

    #[test]
    fn reports_open_and_anticlockwise_outlines() {
        let open = piece(vec![Edge::new(
            "a",
            vec![Segment::line([100.0, 0.0]), Segment::line([100.0, 50.0])],
        )]);
        assert_eq!(codes(&open), [IssueCode::OpenOutline]);
        let anticlockwise = piece(vec![Edge::new(
            "a",
            vec![
                Segment::line([0.0, 50.0]),
                Segment::line([100.0, 50.0]),
                Segment::line([100.0, 0.0]),
                Segment::line([0.0, 0.0]),
            ],
        )]);
        assert_eq!(codes(&anticlockwise), [IssueCode::WrongOrientation]);
    }

    #[test]
    fn reports_a_figure_of_eight() {
        let p = piece(vec![Edge::new(
            "a",
            vec![
                Segment::line([100.0, 100.0]),
                Segment::line([100.0, 0.0]),
                Segment::line([0.0, 100.0]),
                Segment::line([0.0, 0.0]),
            ],
        )]);
        assert!(codes(&p).contains(&IssueCode::SelfIntersection));
    }

    #[test]
    fn fold_edges_must_be_straight() {
        // (0,0) → (0,100) → (-100,100) → back: clockwise on screen.
        let mut fold = Edge::new(
            "f",
            vec![Segment::cubic([0.0, 30.0], [5.0, 60.0], [0.0, 100.0])],
        );
        fold.kind = EdgeKind::Fold;
        let p = piece(vec![
            fold,
            Edge::new("a", vec![Segment::line([-100.0, 100.0])]),
            Edge::new("b", vec![Segment::line([0.0, 0.0])]),
        ]);
        assert_eq!(codes(&p), [IssueCode::FoldNotStraight]);
    }

    #[test]
    fn notches_must_lie_on_their_edge() {
        let mut p = piece(vec![
            Edge::new("a", vec![Segment::line([100.0, 0.0])]),
            Edge::new(
                "b",
                vec![
                    Segment::line([100.0, 50.0]),
                    Segment::line([0.0, 50.0]),
                    Segment::line([0.0, 0.0]),
                ],
            ),
        ]);
        let notch = |id: &str, edge: &str, at| Notch {
            id: id.into(),
            edge: edge.into(),
            at,
            from: NotchFrom::Start,
            kind: NotchKind::Single,
            angle: 0.0,
        };
        p.notches = vec![
            notch("n1", "a", 50.0),
            notch("n2", "a", 120.0),
            notch("n3", "zz", 1.0),
        ];
        assert_eq!(
            codes(&p),
            [IssueCode::NotchOutOfRange, IssueCode::NotchUnknownEdge]
        );
    }
}
