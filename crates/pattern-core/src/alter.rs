//! Alterations: changes to a piece's shape kept as steps, with the piece as
//! drawn kept beside them. The steps are applied in order to the piece as
//! drawn; a step switched off is kept but not applied, so switching every
//! step off gives the piece back exactly as drawn. Nothing changes for good.

use kurbo::common::solve_cubic;
use kurbo::{Affine, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::Pt;
use crate::geom::{EdgeGeom, SAME_POINT, flatten_segs, point_in_polygon, segs_of};
use crate::model::{
    Dart, Grainline, Id, InternalLine, Label, Mark, Notch, NotchFrom, Outline, Piece, Segment,
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
    pub darts: Vec<Dart>,
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
            darts: piece.darts.clone(),
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
        piece.darts = self.darts;
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
    /// Slash and spread: the piece slashed straight from `from`, on its
    /// edge, to `hinge`, on its edge too, and the part on the `turns`
    /// side of the slash turned about the hinge until the slash is `by` mm
    /// open at `from` (overlapping where less than 0). The edge it opens
    /// in is bridged straight across, or meets where the parts overlap,
    /// to be trued up.
    Spread {
        from: Pt,
        hinge: Pt,
        by: f64,
        turns: Turns,
    },
    /// Full bust adjustment by the Y method on a bodice front, or a small
    /// one where `by` is less than 0. The front is slashed from the waist
    /// straight up (along the grain) to the bust point `apex` and on to
    /// `armhole`, on the armhole, where it hinges; from `side`, on the
    /// side seam, to the bust point; and from the bust point straight
    /// across to the centre front. The side turns out about the hinge
    /// until the bust point has moved `by` mm, a dart opens at the side
    /// seam (its tip 25 mm from the bust point), and the centre front below
    /// the bust drops to meet the side at the waist.
    FullBust {
        apex: Pt,
        armhole: Pt,
        side: Pt,
        by: f64,
    },
}

impl Change {
    /// Its points moved by `f`, which mirrors the piece if `mirrors` (so
    /// what was right of a slash is left of it).
    pub fn map_points(&mut self, f: impl Fn(Pt) -> Pt, mirrors: bool) {
        match self {
            Change::Lengthen { line, .. } => {
                if let AlterLine::Through { from, to } = line {
                    *from = f(*from);
                    *to = f(*to);
                }
            }
            Change::Spread {
                from, hinge, turns, ..
            } => {
                *from = f(*from);
                *hinge = f(*hinge);
                if mirrors {
                    *turns = match turns {
                        Turns::Right => Turns::Left,
                        Turns::Left => Turns::Right,
                    };
                }
            }
            Change::FullBust {
                apex,
                armhole,
                side,
                ..
            } => {
                *apex = f(*apex);
                *armhole = f(*armhole);
                *side = f(*side);
            }
        }
    }

    /// The piece's own line it works along, if one.
    pub fn own_line(&mut self) -> Option<&mut Id> {
        match self {
            Change::Lengthen {
                line: AlterLine::Own { line },
                ..
            } => Some(line),
            _ => None,
        }
    }
}

/// Which side of a slash turns: on the right or the left, going along it
/// from where it starts to the hinge, as the pattern is seen.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Turns {
    Right,
    Left,
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
    #[error("A slash must start and end on the piece's edge.")]
    NotOnEdge,
    #[error("A slash must run inside the piece, from edge to edge.")]
    SlashOutside,
    #[error("A slash can open or close by {most:.1} mm at most.")]
    TooWide { most: f64 },
    #[error(
        "The bust point must be inside the piece, with its edge straight below it and across from it."
    )]
    BustPoint,
}

/// The shape `drawn` with `alterations` applied in order (those switched
/// off left out).
pub fn alter(drawn: &Drawn, alterations: &[Alteration]) -> Result<Drawn, AlterError> {
    let mut shape = drawn.clone();
    for a in alterations.iter().filter(|a| a.on) {
        shape = match &a.change {
            Change::Lengthen { line, by } => lengthen(&shape, line, *by)?,
            Change::Spread {
                from,
                hinge,
                by,
                turns,
            } => spread(&shape, (*from).into(), (*hinge).into(), *by, *turns)?,
            Change::FullBust {
                apex,
                armhole,
                side,
                by,
            } => full_bust(
                &shape,
                &a.id,
                [(*apex).into(), (*armhole).into(), (*side).into()],
                *by,
            )?,
        };
    }
    Ok(shape)
}

/// How an alteration cuts the shape and moves its parts.
trait Cut {
    /// Where a curve crosses the cuts, as parameters inside it (0 to 1).
    fn crossings(&self, seg: &PathSeg) -> Vec<f64>;
    /// How the part of the shape around `at` moves.
    fn part(&self, at: Point) -> Affine;
    /// How to join parts that no longer meet, from `end` to `start`, where
    /// the change says (a dart); otherwise straight across, or where they
    /// cross.
    fn join(&self, _end: Point, _start: Point) -> Option<Vec<PathSeg>> {
        None
    }
}

fn moved(seg: PathSeg, a: Affine) -> PathSeg {
    match seg {
        PathSeg::Line(l) => PathSeg::Line(Line::new(a * l.p0, a * l.p1)),
        PathSeg::Quad(q) => {
            let c = q.raise();
            cubic([c.p0, c.p1, c.p2, c.p3].map(|p| a * p))
        }
        PathSeg::Cubic(c) => cubic([c.p0, c.p1, c.p2, c.p3].map(|p| a * p)),
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

/// Where curves `a` and `b` cross, nearest `b`'s start, as a parameter in
/// each: found on them flattened finely.
fn crossing(a: &PathSeg, b: &PathSeg) -> Option<(f64, f64)> {
    const STEPS: usize = 64;
    let points = |s: &PathSeg| -> Vec<Point> {
        (0..=STEPS)
            .map(|k| s.eval(k as f64 / STEPS as f64))
            .collect()
    };
    let (pa, pb) = (points(a), points(b));
    for j in 0..STEPS {
        for i in (0..STEPS).rev() {
            let (p, r) = (pa[i], pa[i + 1] - pa[i]);
            let (q, s) = (pb[j], pb[j + 1] - pb[j]);
            let denominator = r.cross(s);
            if denominator.abs() < 1e-12 {
                continue;
            }
            let t = (q - p).cross(s) / denominator;
            let u = (q - p).cross(r) / denominator;
            if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
                let step = 1.0 / STEPS as f64;
                return Some(((i as f64 + t) * step, (j as f64 + u) * step));
            }
        }
    }
    None
}

/// A run of curves cut where it crosses the cuts, each part moved, and the
/// parts joined where they no longer meet: straight across a gap, or where
/// they cross if they overlap. `from` is where the run must start (the
/// previous run's end), if anywhere.
fn cut_path(cut: &dyn Cut, segs: &[PathSeg], from: Option<Point>) -> Vec<PathSeg> {
    let mut out: Vec<PathSeg> = Vec::new();
    let mut at = from;
    for seg in segs {
        let mut ts = vec![0.0];
        ts.extend(cut.crossings(seg));
        ts.push(1.0);
        for w in ts.windows(2) {
            let part = seg.subsegment(w[0]..w[1]);
            if seg_len_zero(&part) {
                continue;
            }
            let mut next = moved(part, cut.part(part.eval(0.5)));
            if let Some(end) = at
                && end.distance(next.start()) > SAME_POINT
                && let Some(joined) = cut.join(end, next.start())
            {
                out.extend(joined);
            } else if let Some(end) = at
                && end.distance(next.start()) > SAME_POINT
            {
                let overlap = out.last().and_then(|last| crossing(last, &next));
                // Lying along each other, the later starting back over the
                // earlier: the earlier stops beside where the later starts.
                let back = out.last().and_then(|last| {
                    let ahead = last.eval(1.0) - last.eval(0.99);
                    let near = last.nearest(next.start(), 1e-9);
                    ((next.start() - end).dot(ahead) < 0.0 && near.t > 1e-6).then_some(near.t)
                });
                match (out.last_mut(), overlap, back) {
                    (Some(last), Some((ta, tb)), _) if ta > 1e-6 && tb < 1.0 - 1e-6 => {
                        *last = last.subsegment(0.0..ta);
                        next = next.subsegment(tb..1.0);
                        let joint = last.end();
                        next = with_start(next, joint);
                    }
                    (Some(last), _, Some(t)) => {
                        *last = last.subsegment(0.0..t);
                        let beside = last.end();
                        if beside.distance(next.start()) > SAME_POINT {
                            out.push(PathSeg::Line(Line::new(beside, next.start())));
                        }
                    }
                    _ => out.push(PathSeg::Line(Line::new(end, next.start()))),
                }
            }
            at = Some(next.end());
            out.push(next);
        }
    }
    out
}

/// `seg` starting exactly at `p` (a point it nearly starts at).
fn with_start(seg: PathSeg, p: Point) -> PathSeg {
    match seg {
        PathSeg::Line(l) => PathSeg::Line(Line::new(p, l.p1)),
        PathSeg::Quad(q) => {
            let c = q.raise();
            cubic([p, c.p1, c.p2, c.p3])
        }
        PathSeg::Cubic(c) => cubic([p, c.p1, c.p2, c.p3]),
    }
}

fn to_segments(segs: &[PathSeg]) -> Vec<Segment> {
    segs.iter().map(|s| Segment::from_path_seg(*s)).collect()
}

/// The whole shape cut and moved: its outline edge by edge (each starting
/// where the last ended, the last closing it), its notches where they were
/// on their edges' cloth, and its marks, lines, labels and grainline with
/// the parts they are on.
fn cut_shape(cut: &dyn Cut, shape: &Drawn) -> Drawn {
    let outline = &shape.outline;
    let n = outline.edges.len();
    let start_point: Point = outline.start.into();
    let start = cut.part(start_point) * start_point;
    let mut at = Some(start);
    let mut edges = Vec::with_capacity(n);
    for i in 0..n {
        let segs = segs_of(outline.edge_start(i), &outline.edges[i].segments);
        let mut path = cut_path(cut, &segs, at);
        if i == n - 1
            && let Some(end) = path.last().map(ParamCurve::end)
            && end.distance(start) > SAME_POINT
        {
            path.push(PathSeg::Line(Line::new(end, start)));
        }
        at = path.last().map(ParamCurve::end).or(at);
        edges.push(crate::model::Edge {
            segments: to_segments(&path),
            ..outline.edges[i].clone()
        });
    }
    let altered = Outline {
        start: start.into(),
        edges,
    };
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
            let p = before.point_at(along);
            let p = cut.part(p) * p;
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
    // Darts too: each end where it was on its edge's cloth.
    let darts = shape
        .darts
        .iter()
        .map(|dart| {
            let Some(i) = outline.edge_index(&dart.edge) else {
                return dart.clone();
            };
            let (before, after) = (outline.edge_geom(i), altered.edge_geom(i));
            let moved = |along: f64| {
                let p = before.point_at(along);
                let p = cut.part(p) * p;
                after.nearest(p).map_or(along, |n| n.along)
            };
            let (from, to) = (moved(dart.from), moved(dart.to));
            Dart {
                from: from.min(to),
                to: from.max(to),
                ..dart.clone()
            }
        })
        .collect();
    let point = |p: Pt| -> Pt {
        let p: Point = p.into();
        (cut.part(p) * p).into()
    };
    Drawn {
        outline: altered,
        grainline: shape.grainline.map(|g| Grainline {
            from: point(g.from),
            to: point(g.to),
            ..g
        }),
        notches,
        darts,
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
                let start = point(l.start);
                let path = cut_path(cut, &segs_of(l.start, &l.segments), Some(start.into()));
                InternalLine {
                    start,
                    segments: to_segments(&path),
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
    }
}

/// Every point a shape's outline reaches, closely enough to tell where it
/// lies against a line.
fn outline_points(outline: &Outline) -> Vec<Point> {
    let segs = outline.path_segs();
    flatten_segs(&segs, 0.1)
}

// ── Lengthen and shorten ─────────────────────────────────────────────────

/// A line across the piece: a point on it, and the way the part beyond it
/// moves (a unit vector across it: down, or right for an upright line).
#[derive(Clone, Copy, Debug)]
struct Across {
    at: Point,
    across: Vec2,
}

impl Across {
    fn of(shape: &Drawn, line: &AlterLine) -> Result<Across, AlterError> {
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
        Ok(Across { at: from, across })
    }

    /// How far beyond the line `p` is (negative: before it), mm.
    fn beyond(&self, p: Point) -> f64 {
        (p - self.at).dot(self.across)
    }
}

/// Lengthened by `by` across a line: beyond it, the piece moves on by
/// `by`. Shortened, a tuck as deep as `-by` folds out beyond it: what lay
/// in the tuck goes onto the line, what lay beyond it moves back.
struct Level {
    line: Across,
    by: f64,
}

impl Level {
    /// The depths (beyond the line) where the piece moves differently.
    fn levels(&self) -> Vec<f64> {
        if self.by >= 0.0 {
            vec![0.0]
        } else {
            vec![0.0, -self.by]
        }
    }

    fn cubic_crossings(&self, c: &kurbo::CubicBez, level: f64) -> Vec<f64> {
        let [s0, s1, s2, s3] = [c.p0, c.p1, c.p2, c.p3].map(|p| self.line.beyond(p) - level);
        solve_cubic(
            s0,
            3.0 * (s1 - s0),
            3.0 * (s0 - 2.0 * s1 + s2),
            -s0 + 3.0 * s1 - 3.0 * s2 + s3,
        )
        .to_vec()
    }
}

impl Cut for Level {
    fn crossings(&self, seg: &PathSeg) -> Vec<f64> {
        let mut ts = Vec::new();
        for level in self.levels() {
            let s = |p: Point| self.line.beyond(p) - level;
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

    /// On the line itself (as near as rounding allows) nothing moves.
    fn part(&self, at: Point) -> Affine {
        let region = self.line.beyond(at);
        let n = self.line.across;
        if self.by >= 0.0 {
            if region > SAME_POINT {
                Affine::translate(n * self.by)
            } else {
                Affine::IDENTITY
            }
        } else if region >= -self.by - SAME_POINT {
            Affine::translate(n * self.by)
        } else if region > SAME_POINT {
            // Onto the line: p − ((p − at)·n) n.
            let a = self.line.at.to_vec2();
            let linear = Affine::new([
                1.0 - n.x * n.x,
                -n.x * n.y,
                -n.x * n.y,
                1.0 - n.y * n.y,
                0.0,
                0.0,
            ]);
            Affine::translate(n * a.dot(n)) * linear
        } else {
            Affine::IDENTITY
        }
    }
}

/// The shape lengthened (or shortened) by `by` mm across `line`.
fn lengthen(shape: &Drawn, line: &AlterLine, by: f64) -> Result<Drawn, AlterError> {
    let line = Across::of(shape, line)?;
    // The line must cross the piece, and a tuck fit in what lies beyond.
    let (least, most) = outline_points(&shape.outline)
        .into_iter()
        .map(|p| line.beyond(p))
        .fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(s), hi.max(s)));
    if least >= -SAME_POINT || most <= SAME_POINT {
        return Err(AlterError::Misses);
    }
    if by < 0.0 && -by >= most - SAME_POINT {
        return Err(AlterError::TooShort { most });
    }
    Ok(cut_shape(&Level { line, by }, shape))
}

// ── Slash and spread ─────────────────────────────────────────────────────

/// A slash from one point of the edge to another, the part on one side of
/// it turning about the hinge at its end.
struct Slash {
    from: Point,
    hinge: Point,
    outline: Outline,
    /// Where the turning part's edge is along the outline: between `lo`
    /// and `hi` mm from its start, or outside them.
    lo: f64,
    hi: f64,
    between: bool,
    /// The turning part, as a polygon.
    turning: Vec<Point>,
    turn: Affine,
}

/// How far a slash is made longer each way to find where curves meet it
/// at its very ends, mm.
const REACH: f64 = 10.0;

impl Cut for Slash {
    fn crossings(&self, seg: &PathSeg) -> Vec<f64> {
        let length = self.from.distance(self.hinge);
        let way = (self.hinge - self.from) / length;
        let longer = Line::new(self.from - way * REACH, self.hinge + way * REACH);
        let mut ts: Vec<f64> = seg
            .intersect_line(longer)
            .into_iter()
            .filter(|i| {
                let along = i.line_t * (length + 2.0 * REACH) - REACH;
                (-SAME_POINT..=length + SAME_POINT).contains(&along)
            })
            .map(|i| i.segment_t)
            .filter(|t| *t > 1e-9 && *t < 1.0 - 1e-9)
            .collect();
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        ts
    }

    /// On the edge, by where it is along it; inside the piece, by the
    /// turning part's shape.
    fn part(&self, at: Point) -> Affine {
        let turns = match on_outline(&self.outline, at) {
            Some((_, along, distance)) if distance < 1e-6 => {
                (along > self.lo && along < self.hi) == self.between
            }
            _ => point_in_polygon(at, &self.turning),
        };
        if turns { self.turn } else { Affine::IDENTITY }
    }
}

/// The nearest point of the outline to `p`, and how far along it is from
/// the outline's start, mm (and how far `p` is from it).
fn on_outline(outline: &Outline, p: Point) -> Option<(Point, f64, f64)> {
    let mut walked = 0.0;
    let mut best: Option<(Point, f64, f64)> = None;
    for i in 0..outline.edges.len() {
        let geom = outline.edge_geom(i);
        if let Some(near) = geom.nearest(p)
            && best.is_none_or(|b| near.distance < b.2)
        {
            best = Some((near.point, walked + near.along, near.distance));
        }
        walked += geom.length();
    }
    best
}

/// The shape slashed from `from` to `hinge` and spread (or closed) by `by`
/// at `from`, the part on the `turns` side turning.
fn spread(
    shape: &Drawn,
    from: Point,
    hinge: Point,
    by: f64,
    turns: Turns,
) -> Result<Drawn, AlterError> {
    const ON_EDGE: f64 = 2.0;
    let outline = &shape.outline;
    let (Some((from, a, d_from)), Some((hinge, b, d_hinge))) =
        (on_outline(outline, from), on_outline(outline, hinge))
    else {
        return Err(AlterError::NotOnEdge);
    };
    if d_from > ON_EDGE || d_hinge > ON_EDGE {
        return Err(AlterError::NotOnEdge);
    }
    let length = from.distance(hinge);
    if length < 1.0 {
        return Err(AlterError::NoDirection);
    }
    // The slash runs inside the piece, crossing its edge only at its ends.
    let polygon = outline_points(outline);
    let slash = Line::new(from, hinge);
    let inside = [0.25, 0.5, 0.75].iter().all(|&t| {
        let p = slash.eval(t);
        point_in_polygon(p, &polygon) && on_outline(outline, p).is_some_and(|o| o.2 > 0.5)
    });
    if !inside {
        return Err(AlterError::SlashOutside);
    }
    let crosses = outline.path_segs().iter().any(|seg| {
        seg.intersect_line(slash).into_iter().any(|i| {
            let p = slash.eval(i.line_t);
            (0.0..=1.0).contains(&i.line_t)
                && p.distance(from) > ON_EDGE
                && p.distance(hinge) > ON_EDGE
        })
    });
    if crosses {
        return Err(AlterError::SlashOutside);
    }
    let most = 2.0 * length * 0.5f64.sin();
    if by.abs() > most {
        return Err(AlterError::TooWide { most });
    }

    // The outline in two between the slash's ends, each half closed by the
    // slash into a polygon: the half on the `turns` side turns.
    let flat = outline_flat_with_along(outline);
    let (lo, hi) = (a.min(b), a.max(b));
    let (first, last) = if a <= b { (from, hinge) } else { (hinge, from) };
    // Between the ends, going along the outline.
    let mut inner = vec![first];
    inner.extend(
        flat.iter()
            .filter(|(_, s)| *s > lo && *s < hi)
            .map(|(p, _)| *p),
    );
    inner.push(last);
    // The rest, on round past the outline's start.
    let mut outer = vec![last];
    outer.extend(
        flat.iter()
            .filter(|(_, s)| *s > hi)
            .chain(flat.iter().filter(|(_, s)| *s < lo))
            .map(|(p, _)| *p),
    );
    outer.push(first);
    let along = (hinge - from) / length;
    // Right of the slash, going along it as the pattern is seen (y down).
    let right = Vec2::new(-along.y, along.x);
    let want = match turns {
        Turns::Right => 1.0,
        Turns::Left => -1.0,
    };
    let side = |points: &[Point]| {
        points.iter().map(|p| (*p - from).dot(right)).sum::<f64>() / points.len() as f64
    };
    let between = side(&inner) * want > side(&outer) * want;
    let turning = if between { inner } else { outer };
    // Turned away from the rest to open the slash by `by` at its start.
    let r = from - hinge;
    let opening = Vec2::new(-r.y, r.x).dot(right * want).signum();
    let angle = opening * by.signum() * 2.0 * (by.abs() / (2.0 * length)).asin();
    let turn = Affine::translate(hinge.to_vec2())
        * Affine::rotate(angle)
        * Affine::translate(-hinge.to_vec2());
    Ok(cut_shape(
        &Slash {
            from,
            hinge,
            outline: outline.clone(),
            lo,
            hi,
            between,
            turning,
            turn,
        },
        shape,
    ))
}

/// The outline flattened, each point with how far along the outline it is.
fn outline_flat_with_along(outline: &Outline) -> Vec<(Point, f64)> {
    let mut out = Vec::new();
    let mut walked = 0.0;
    for i in 0..outline.edges.len() {
        let geom = outline.edge_geom(i);
        let length = geom.length();
        let steps = ((length / 2.0).ceil() as usize).max(1);
        for k in 0..steps {
            let s = length * k as f64 / steps as f64;
            out.push((geom.point_at(s), walked + s));
        }
        walked += length;
    }
    out
}

// ── Full and small bust ──────────────────────────────────────────────────

/// The Y method's slashes, from the bust point: their ends on the edge, in
/// order round it, and how each part between two of them moves.
struct Bust {
    apex: Point,
    /// The slashes' ends: at the waist, the side seam, the armhole (the
    /// hinge) and the centre front.
    ends: [Point; 4],
    /// Each part between one slash and the next, round the bust point the
    /// way the slashes are in order: lower side, upper side, upper centre,
    /// lower centre.
    moves: [Affine; 4],
    /// Round the bust point, the slashes' angles in order, and which way.
    angles: [f64; 4],
    turning: f64,
    /// The dart at the side seam, if it opens: its legs' ends as moved,
    /// and its tip.
    dart: Option<(Point, Point, Point)>,
}

impl Bust {
    /// How far round from slash `k`, the way the slashes go, `a` is.
    fn round_from(&self, k: usize, a: f64) -> f64 {
        (self.turning * (a - self.angles[k])).rem_euclid(std::f64::consts::TAU)
    }
}

impl Cut for Bust {
    fn crossings(&self, seg: &PathSeg) -> Vec<f64> {
        let mut ts = Vec::new();
        for end in self.ends {
            let length = self.apex.distance(end);
            let way = (end - self.apex) / length;
            let longer = Line::new(self.apex, end + way * REACH);
            ts.extend(
                seg.intersect_line(longer)
                    .into_iter()
                    .filter(|i| i.line_t * (length + REACH) <= length + SAME_POINT)
                    .map(|i| i.segment_t),
            );
        }
        ts.retain(|t| *t > 1e-9 && *t < 1.0 - 1e-9);
        ts.sort_by(f64::total_cmp);
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
        ts
    }

    fn part(&self, at: Point) -> Affine {
        let a = (at - self.apex).atan2();
        // The part a point is in: after the last slash it is round from.
        let k = (0..4)
            .min_by(|&i, &j| self.round_from(i, a).total_cmp(&self.round_from(j, a)))
            .expect("four slashes");
        self.moves[k]
    }

    fn join(&self, end: Point, start: Point) -> Option<Vec<PathSeg>> {
        let (upper, lower, tip) = self.dart?;
        let near = |p: Point, q: Point| p.distance(q) < 1e-3;
        let legs = |a: Point, b: Point| {
            vec![
                PathSeg::Line(Line::new(a, tip)),
                PathSeg::Line(Line::new(tip, b)),
            ]
        };
        if near(end, upper) && near(start, lower) || near(end, lower) && near(start, upper) {
            Some(legs(end, start))
        } else {
            None
        }
    }
}

/// Where a ray from `from` going `way` first meets the outline.
fn ray_to_outline(outline: &Outline, from: Point, way: Vec2) -> Option<Point> {
    let far = Line::new(from, from + way * 10_000.0);
    outline
        .path_segs()
        .iter()
        .flat_map(|seg| seg.intersect_line(far))
        .map(|i| far.eval(i.line_t))
        .filter(|p| p.distance(from) > SAME_POINT)
        .min_by(|p, q| p.distance(from).total_cmp(&q.distance(from)))
}

/// The front with a full (or small) bust adjustment by the Y method, by
/// alteration `id`, its points the bust point, the armhole's hinge and the
/// side seam's slash.
fn full_bust(shape: &Drawn, id: &str, points: [Point; 3], by: f64) -> Result<Drawn, AlterError> {
    let [apex, armhole, side] = points;
    const ON_EDGE: f64 = 2.0;
    /// The dart's tip stops this far short of the bust point, mm.
    const TIP: f64 = 25.0;
    let outline = &shape.outline;
    let polygon = outline_points(outline);
    if !point_in_polygon(apex, &polygon) {
        return Err(AlterError::BustPoint);
    }
    let (Some((armhole, _, d_armhole)), Some((side, _, d_side))) =
        (on_outline(outline, armhole), on_outline(outline, side))
    else {
        return Err(AlterError::NotOnEdge);
    };
    if d_armhole > ON_EDGE || d_side > ON_EDGE {
        return Err(AlterError::NotOnEdge);
    }
    // Down the grain to the waist (y down without a grainline), and across
    // to the side and to the centre front.
    let down = shape
        .grainline
        .map(|g| {
            let d = Point::from(g.to) - Point::from(g.from);
            let d = d / d.hypot().max(1e-9);
            if d.y < 0.0 { -d } else { d }
        })
        .unwrap_or(Vec2::new(0.0, 1.0));
    let mut out = Vec2::new(-down.y, down.x);
    if (side - apex).dot(out) < 0.0 {
        out = -out;
    }
    let (Some(waist), Some(centre)) = (
        ray_to_outline(outline, apex, down),
        ray_to_outline(outline, apex, -out),
    ) else {
        return Err(AlterError::BustPoint);
    };
    // The side turns about the hinge until the bust point moves `by` out.
    let v = apex - armhole;
    let moved_out = |angle: f64| {
        let (sin, cos) = angle.sin_cos();
        (Vec2::new(v.x * cos - v.y * sin, v.x * sin + v.y * cos) - v).dot(out)
    };
    let rate = Vec2::new(-v.y, v.x).dot(out);
    if rate.abs() < 1e-6 || by.abs() > v.hypot() / 2.0 {
        return Err(AlterError::TooWide {
            most: v.hypot() / 2.0,
        });
    }
    let mut angle = by / rate;
    for _ in 0..8 {
        let (sin, cos) = angle.sin_cos();
        let slope = Vec2::new(-v.x * sin - v.y * cos, v.x * cos - v.y * sin).dot(out);
        angle -= (moved_out(angle) - by) / slope;
    }
    let turn = Affine::translate(armhole.to_vec2())
        * Affine::rotate(angle)
        * Affine::translate(-armhole.to_vec2());
    let shift = turn * apex - apex;
    // The slashes in order round the bust point: waist, side, armhole,
    // centre front; the parts between them move as their sides do.
    let ends = [waist, side, armhole, centre];
    let angles = ends.map(|e| (e - apex).atan2());
    // Which way round they go: the side seam between the waist and the
    // armhole.
    let ccw = |from: f64, to: f64| (to - from).rem_euclid(std::f64::consts::TAU);
    let turning = if ccw(angles[0], angles[1]) < ccw(angles[0], angles[2]) {
        1.0
    } else {
        -1.0
    };
    let moves = [
        Affine::translate(shift),
        turn,
        Affine::IDENTITY,
        Affine::translate(down * shift.dot(down)),
    ];
    let upper = turn * side;
    let lower = side + shift;
    let mouth = upper.midpoint(lower);
    let to_mouth = mouth - (apex + shift);
    let tip =
        apex + shift + to_mouth / to_mouth.hypot().max(1e-9) * TIP.min(to_mouth.hypot() / 2.0);
    let mut altered = cut_shape(
        &Bust {
            apex,
            ends,
            moves,
            angles,
            turning,
            dart: (by > 0.0).then_some((upper, lower, tip)),
        },
        shape,
    );
    // The dart it opens, known as a dart: closed when sewn.
    if by > 0.0
        && let Some(i) = edge_of(outline, side)
    {
        let edge = altered.outline.edge_geom(i);
        if let (Some(a), Some(b)) = (edge.nearest(upper), edge.nearest(lower)) {
            altered.darts.push(Dart {
                id: format!("{id}.dart"),
                edge: outline.edges[i].id.clone(),
                from: a.along.min(b.along),
                to: a.along.max(b.along),
            });
        }
    }
    Ok(altered)
}

/// The edge nearest to `p` (by index).
fn edge_of(outline: &Outline, p: Point) -> Option<usize> {
    (0..outline.edges.len())
        .filter_map(|i| outline.edge_geom(i).nearest(p).map(|n| (i, n.distance)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}
