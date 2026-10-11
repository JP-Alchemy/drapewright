//! Alterations: changes to a piece's shape kept as steps, with the piece as
//! drawn kept beside them. The steps are applied in order to the piece as
//! drawn; a step switched off is kept but not applied, so switching every
//! step off gives the piece back exactly as drawn. Nothing changes for good.

use kurbo::common::solve_cubic;
use kurbo::{ParamCurve, PathSeg, Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Pt;
use crate::geom::{EdgeGeom, SAME_POINT, segs_of};
use crate::model::{
    Grainline, Id, InternalLine, Label, Mark, Notch, NotchFrom, Outline, Piece, Segment,
};

/// The piece's shape: what alterations change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Drawn {
    pub outline: Outline,
    #[serde(default)]
    pub grainline: Option<Grainline>,
    #[serde(default)]
    pub notches: Vec<Notch>,
    #[serde(default)]
    pub marks: Vec<Mark>,
    #[serde(default)]
    pub lines: Vec<InternalLine>,
    #[serde(default)]
    pub labels: Vec<Label>,
}

impl Drawn {
    /// The piece's shape as it is.
    pub fn of(piece: &Piece) -> Drawn {
        Drawn {
            outline: piece.outline.clone(),
            grainline: piece.grainline,
            notches: piece.notches.clone(),
            marks: piece.marks.clone(),
            lines: piece.lines.clone(),
            labels: piece.labels.clone(),
        }
    }

    /// Gives `piece` this shape.
    pub fn put(self, piece: &mut Piece) {
        piece.outline = self.outline;
        piece.grainline = self.grainline;
        piece.notches = self.notches;
        piece.marks = self.marks;
        piece.lines = self.lines;
        piece.labels = self.labels;
    }
}

/// The line an alteration works along.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum AlterLine {
    /// One of the piece's own lines (a lengthen or shorten line), from its
    /// start to its end.
    Own { line: Id },
    /// Through two points, in the piece's own coordinates (mm).
    Through { from: Pt, to: Pt },
}

/// What an alteration does.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum Change {
    /// Longer by `by` mm across `line` (shorter where `by` is less than
    /// 0): the piece spread apart along the line, or a tuck folded out
    /// there. The part below the line moves (right of an upright line); a
    /// side crossing the line is bridged straight, or jogs along it, to be
    /// trued up.
    Lengthen { line: AlterLine, by: f64 },
}

fn yes() -> bool {
    true
}

/// A change to a piece, kept as a step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Alteration {
    pub id: Id,
    /// Applied; switched off, it is kept but not applied.
    #[serde(default = "yes")]
    pub on: bool,
    pub change: Change,
}

/// Why an alteration cannot be applied.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum AlterError {
    #[error("The line to alter along is no longer on the piece.")]
    NoLine { line: Id },
    #[error("The line to alter along must have two different ends.")]
    NoDirection,
    #[error("The line to alter along does not cross the piece.")]
    Misses,
    #[error("The piece can be shortened there by {most:.1} mm at most.")]
    TooShort { most: f64 },
}

/// The shape `drawn` with `alterations` applied in order (those switched
/// off left out).
pub fn alter(drawn: &Drawn, alterations: &[Alteration]) -> Result<Drawn, AlterError> {
    let mut shape = drawn.clone();
    for a in alterations.iter().filter(|a| a.on) {
        shape = match &a.change {
            Change::Lengthen { line, by } => lengthen(&shape, line, *by)?,
        };
    }
    Ok(shape)
}

/// Where a line meets the piece: a point on it, and the way its far side
/// moves (a unit vector across it: down, or right for an upright line).
#[derive(Clone, Copy, Debug)]
struct Cut {
    at: Point,
    across: Vec2,
}

impl Cut {
    fn of(shape: &Drawn, line: &AlterLine) -> Result<Cut, AlterError> {
        let (from, to) = match line {
            AlterLine::Own { line } => {
                let l = shape
                    .lines
                    .iter()
                    .find(|l| &l.id == line)
                    .ok_or_else(|| AlterError::NoLine { line: line.clone() })?;
                let end = l.segments.last().map_or(l.start, Segment::to);
                (l.start, end)
            }
            AlterLine::Through { from, to } => (*from, *to),
        };
        let (from, to) = (Point::from(from), Point::from(to));
        let along = to - from;
        if along.hypot() < SAME_POINT {
            return Err(AlterError::NoDirection);
        }
        let along = along / along.hypot();
        let mut across = Vec2::new(-along.y, along.x);
        if across.y < -1e-9 || (across.y.abs() <= 1e-9 && across.x < 0.0) {
            across = -across;
        }
        Ok(Cut { at: from, across })
    }

    /// How far beyond the line `p` is (negative: before it), mm.
    fn beyond(&self, p: Point) -> f64 {
        (p - self.at).dot(self.across)
    }
}

/// Where points go when the piece is lengthened by `by` along `cut`: those
/// beyond the line move on by `by`; shortened, those beyond the tuck move
/// back by it and those in the tuck onto the line.
#[derive(Clone, Copy, Debug)]
struct Spread {
    cut: Cut,
    by: f64,
}

impl Spread {
    /// The levels (distances beyond the line) where points start moving
    /// differently.
    fn levels(&self) -> Vec<f64> {
        if self.by >= 0.0 {
            vec![0.0]
        } else {
            vec![0.0, -self.by]
        }
    }

    /// Where point `p` goes, as part of a run whose middle is `region`
    /// beyond the line: moved on (or back), onto the line, or left. On
    /// the line itself (as near as rounding allows) nothing moves.
    fn moved(&self, p: Point, region: f64) -> Point {
        let s = self.cut.beyond(p);
        if self.by >= 0.0 {
            if region > SAME_POINT {
                p + self.cut.across * self.by
            } else {
                p
            }
        } else if region >= -self.by - SAME_POINT {
            p + self.cut.across * self.by
        } else if region > SAME_POINT {
            p - self.cut.across * s
        } else {
            p
        }
    }

    /// A point on its own (a mark, a label, a grainline's end).
    fn point(&self, p: Point) -> Point {
        self.moved(p, self.cut.beyond(p))
    }

    /// Where a run of curves crosses the levels, as parameters in each.
    fn crossings(&self, seg: &PathSeg) -> Vec<f64> {
        let mut ts = Vec::new();
        for level in self.levels() {
            let s = |p: Point| self.cut.beyond(p) - level;
            match seg {
                PathSeg::Line(l) => {
                    let (a, b) = (s(l.p0), s(l.p1));
                    if (a < 0.0) != (b < 0.0) && (a - b).abs() > 1e-12 {
                        ts.push(a / (a - b));
                    }
                }
                PathSeg::Quad(q) => ts.extend(self.cubic_crossings(&q.raise(), level)),
                PathSeg::Cubic(c) => ts.extend(self.cubic_crossings(c, level)),
            }
        }
        ts.retain(|t| *t > 1e-9 && *t < 1.0 - 1e-9);
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        ts
    }

    fn cubic_crossings(&self, c: &kurbo::CubicBez, level: f64) -> Vec<f64> {
        let [s0, s1, s2, s3] = [c.p0, c.p1, c.p2, c.p3].map(|p| self.cut.beyond(p) - level);
        solve_cubic(
            s0,
            3.0 * (s1 - s0),
            3.0 * (s0 - 2.0 * s1 + s2),
            -s0 + 3.0 * s1 - 3.0 * s2 + s3,
        )
        .to_vec()
    }

    /// A run of curves, moved: cut where it crosses the levels, each part
    /// moved as its middle is, and bridged straight where its parts no
    /// longer meet. `from` is where the run must start (the previous run's
    /// end), if anywhere.
    fn path(&self, segs: &[PathSeg], from: Option<Point>) -> Vec<PathSeg> {
        let mut out: Vec<PathSeg> = Vec::new();
        let mut at = from;
        for seg in segs {
            let mut ts = vec![0.0];
            ts.extend(self.crossings(seg));
            ts.push(1.0);
            for w in ts.windows(2) {
                let part = seg.subsegment(w[0]..w[1]);
                if part.start().distance(part.end()) < 1e-9 && seg_len_zero(&part) {
                    continue;
                }
                let region = self.cut.beyond(part.eval(0.5));
                let moved = match part {
                    PathSeg::Line(l) => PathSeg::Line(kurbo::Line::new(
                        self.moved(l.p0, region),
                        self.moved(l.p1, region),
                    )),
                    PathSeg::Quad(q) => {
                        let c = q.raise();
                        cubic([c.p0, c.p1, c.p2, c.p3].map(|p| self.moved(p, region)))
                    }
                    PathSeg::Cubic(c) => {
                        cubic([c.p0, c.p1, c.p2, c.p3].map(|p| self.moved(p, region)))
                    }
                };
                if let Some(end) = at
                    && end.distance(moved.start()) > SAME_POINT
                {
                    out.push(PathSeg::Line(kurbo::Line::new(end, moved.start())));
                }
                at = Some(moved.end());
                out.push(moved);
            }
        }
        out
    }
}

fn cubic(p: [Point; 4]) -> PathSeg {
    PathSeg::Cubic(kurbo::CubicBez::new(p[0], p[1], p[2], p[3]))
}

fn seg_len_zero(seg: &PathSeg) -> bool {
    match seg {
        PathSeg::Line(l) => l.p0.distance(l.p1) < 1e-9,
        PathSeg::Quad(q) => q.p0.distance(q.p1) + q.p1.distance(q.p2) < 1e-9,
        PathSeg::Cubic(c) => c.p0.distance(c.p1) + c.p1.distance(c.p2) + c.p2.distance(c.p3) < 1e-9,
    }
}

fn to_segments(segs: &[PathSeg]) -> Vec<Segment> {
    segs.iter().map(|s| Segment::from_path_seg(*s)).collect()
}

/// The shape lengthened (or shortened) by `by` mm across `line`.
fn lengthen(shape: &Drawn, line: &AlterLine, by: f64) -> Result<Drawn, AlterError> {
    let cut = Cut::of(shape, line)?;
    let spread = Spread { cut, by };
    let outline = &shape.outline;
    let n = outline.edges.len();
    // The line must cross the piece, and a tuck fit in what lies beyond.
    let corners: Vec<f64> = (0..n)
        .flat_map(|i| {
            let start = outline.edge_start(i);
            segs_of(start, &outline.edges[i].segments)
                .into_iter()
                .flat_map(|s| {
                    let mut pts = vec![s.start(), s.end()];
                    // Enough along a curve to see where it reaches.
                    pts.extend((1..8).map(|k| s.eval(f64::from(k) / 8.0)));
                    pts
                })
                .collect::<Vec<_>>()
        })
        .map(|p| cut.beyond(p))
        .collect();
    let (least, most) = corners
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), &s| (lo.min(s), hi.max(s)));
    if least >= -SAME_POINT || most <= SAME_POINT {
        return Err(AlterError::Misses);
    }
    if by < 0.0 && -by >= most - SAME_POINT {
        return Err(AlterError::TooShort { most });
    }

    // The outline, edge by edge, each starting where the last ended.
    let start = spread.point(outline.start.into());
    let mut at = Some(start);
    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let segs = segs_of(outline.edge_start(i), &outline.edges[i].segments);
        let mut moved = spread.path(&segs, at);
        // The last edge closes the outline where it starts.
        if i == n - 1
            && let Some(end) = moved.last().map(ParamCurve::end)
            && end.distance(start) > SAME_POINT
        {
            moved.push(PathSeg::Line(kurbo::Line::new(end, start)));
        }
        at = moved.last().map(ParamCurve::end).or(at);
        edges.push(crate::model::Edge {
            segments: to_segments(&moved),
            ..outline.edges[i].clone()
        });
    }
    let altered = Outline {
        start: start.into(),
        edges,
    };

    // Notches stay where they were on their edges' cloth.
    let notches = shape
        .notches
        .iter()
        .map(|notch| {
            let Some(i) = outline.edge_index(&notch.edge) else {
                return notch.clone();
            };
            let before = outline.edge_geom(i);
            let along = match notch.from {
                NotchFrom::Start => notch.at,
                NotchFrom::End => before.length() - notch.at,
            };
            let p = spread.point(before.point_at(along));
            let after: EdgeGeom = altered.edge_geom(i);
            let Some(nearest) = after.nearest(p) else {
                return notch.clone();
            };
            let at = match notch.from {
                NotchFrom::Start => nearest.along,
                NotchFrom::End => after.length() - nearest.along,
            };
            Notch {
                at: at.max(0.0),
                ..notch.clone()
            }
        })
        .collect();
    let point = |p: Pt| -> Pt { spread.point(p.into()).into() };
    Ok(Drawn {
        outline: altered,
        grainline: shape.grainline.map(|g| Grainline {
            from: point(g.from),
            to: point(g.to),
            ..g
        }),
        notches,
        marks: shape
            .marks
            .iter()
            .map(|m| Mark {
                at: point(m.at),
                ..m.clone()
            })
            .collect(),
        lines: shape
            .lines
            .iter()
            .map(|l| {
                let start = spread.point(l.start.into());
                let moved = spread.path(&segs_of(l.start, &l.segments), Some(start));
                InternalLine {
                    start: start.into(),
                    segments: to_segments(&moved),
                    ..l.clone()
                }
            })
            .collect(),
        labels: shape
            .labels
            .iter()
            .map(|t| Label {
                at: point(t.at),
                ..t.clone()
            })
            .collect(),
    })
}
