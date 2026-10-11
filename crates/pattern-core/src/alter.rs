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
use crate::geom::{EdgeGeom, SAME_POINT, flatten_segs, point_in_polygon, polygon_area, segs_of};
use crate::model::{
    CubicSeg, Dart, EdgeKind, Grainline, Id, InternalLine, Label, LineSeg, Mark, Notch, NotchFrom,
    Outline, Piece, Segment,
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
    /// A dart pivoted to `to`, on the edge: the piece slashed from there to
    /// the dart's tip, and the smaller part turned about the tip until the
    /// dart is closed by `share` (1 all of it, 0.5 half) and the slash is
    /// open as much, a dart there. Moved whole, the dart keeps its id; in
    /// part, the new one is the step's id with `.dart` after it.
    Pivot { dart: Id, to: Pt, share: f64 },
    /// A swayback adjustment on a back: a wedge `by` mm deep at the centre
    /// back (at `centre`, on it) folded out, tapering to nothing at the
    /// side seam (at `side`, on it); let out where less than 0. The smaller
    /// part turns about the side.
    Swayback { centre: Pt, side: Pt, by: f64 },
    /// Broad shoulders, or narrow where `by` is less than 0: slashed from
    /// `shoulder`, on the shoulder seam, to `armhole`, on the armhole, where
    /// it hinges, and the shoulder's end turned out until the slash is `by`
    /// mm open at the shoulder seam (overlapping where less than 0).
    Shoulder { shoulder: Pt, armhole: Pt, by: f64 },
    /// More room at the waist, or less where `by` is less than 0: a side
    /// seam moved out square to the grain by `by` mm at `at` (on it, at the
    /// waist) and beyond, tapering to nothing at `to` (on it too, at the
    /// hip). The edges meeting it stretch to meet it.
    Waist { at: Pt, to: Pt, by: f64 },
    /// More room at the hip, or less: as for the waist, the side seam moved
    /// out by `by` mm at `at` (at the hip) and beyond (down to the hem),
    /// tapering to nothing at `to` (at the waist).
    Hip { at: Pt, to: Pt, by: f64 },
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
            Change::Pivot { to, .. } => *to = f(*to),
            Change::Swayback { centre, side, .. } => {
                *centre = f(*centre);
                *side = f(*side);
            }
            Change::Shoulder {
                shoulder, armhole, ..
            } => {
                *shoulder = f(*shoulder);
                *armhole = f(*armhole);
            }
            Change::Waist { at, to, .. } | Change::Hip { at, to, .. } => {
                *at = f(*at);
                *to = f(*to);
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

    /// The piece's dart it moves, if one.
    pub fn own_dart(&mut self) -> Option<&mut Id> {
        match self {
            Change::Pivot { dart, .. } => Some(dart),
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
    #[error("The dart to move is no longer on the piece.")]
    NoDart { dart: Id },
    #[error("A dart must move to another place on the edge, away from where it is.")]
    OntoDart,
    #[error(
        "A swayback adjustment needs a centre back seam: this piece is cut on the fold there. Make the fold a seam first."
    )]
    OnFold,
    #[error("Both points must be on the same seam, apart.")]
    NotOneSeam,
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
            } => spread(
                &shape,
                (*from).into(),
                &[],
                (*hinge).into(),
                *by,
                Some(*turns),
            )?,
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
            Change::Pivot { dart, to, share } => pivot(&shape, &a.id, dart, (*to).into(), *share)?,
            Change::Swayback { centre, side, by } => {
                let centre: Point = (*centre).into();
                if edge_of(&shape.outline, centre)
                    .is_some_and(|i| shape.outline.edges[i].kind == EdgeKind::Fold)
                {
                    return Err(AlterError::OnFold);
                }
                spread(&shape, centre, &[], (*side).into(), -*by, None)?
            }
            Change::Shoulder {
                shoulder,
                armhole,
                by,
            } => {
                // Slashed down the grain from the shoulder seam, then across
                // to the armhole.
                let (shoulder, armhole): (Point, Point) = ((*shoulder).into(), (*armhole).into());
                let down = down_the_grain(&shape);
                let bend = shoulder + down * (armhole - shoulder).dot(down);
                let opening = shoulder_opening(&shape.outline, shoulder, armhole, *by)?;
                spread(&shape, shoulder, &[bend], armhole, opening, None)?
            }
            Change::Waist { at, to, by } | Change::Hip { at, to, by } => {
                widen(&shape, (*at).into(), (*to).into(), *by)?
            }
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

/// A slash from one point of the edge to another, straight or bending at
/// points inside the piece, the part on one side of it turning about the
/// hinge at its end.
struct Slash {
    /// From its start, on the edge, through its bends, to the hinge.
    path: Vec<Point>,
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

impl Slash {
    fn hinge(&self) -> Point {
        self.path[self.path.len() - 1]
    }
}

impl Cut for Slash {
    fn crossings(&self, seg: &PathSeg) -> Vec<f64> {
        let legs = self.path.len() - 1;
        let mut ts = Vec::new();
        for (k, w) in self.path.windows(2).enumerate() {
            let length = w[0].distance(w[1]);
            if length < 1e-9 {
                continue;
            }
            let way = (w[1] - w[0]) / length;
            // Its ends on the edge made longer, to find curves meeting it
            // there.
            let before = if k == 0 { REACH } else { 0.0 };
            let after = if k == legs - 1 { REACH } else { 0.0 };
            let longer = Line::new(w[0] - way * before, w[1] + way * after);
            ts.extend(
                seg.intersect_line(longer)
                    .into_iter()
                    .filter(|i| {
                        let along = i.line_t * (length + before + after) - before;
                        (-SAME_POINT..=length + SAME_POINT).contains(&along)
                    })
                    .map(|i| i.segment_t),
            );
        }
        ts.retain(|t| *t > 1e-9 && *t < 1.0 - 1e-9);
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

/// The shape slashed from `from` (through `bends`, inside it) to `hinge`,
/// and spread (or closed) by `by` at `from`, the part on the `turns` side
/// turning (or the smaller part).
fn spread(
    shape: &Drawn,
    from: Point,
    bends: &[Point],
    hinge: Point,
    by: f64,
    turns: Option<Turns>,
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
    let mut path = vec![from];
    path.extend(bends);
    path.push(hinge);
    let polygon = outline_points(outline);
    let segs = outline.path_segs();
    for w in path.windows(2) {
        let leg = Line::new(w[0], w[1]);
        let inside = [0.25, 0.5, 0.75].iter().all(|&t| {
            let p = leg.eval(t);
            point_in_polygon(p, &polygon) && on_outline(outline, p).is_some_and(|o| o.2 > 0.5)
        });
        let crosses = segs.iter().any(|seg| {
            seg.intersect_line(leg).into_iter().any(|i| {
                let p = leg.eval(i.line_t);
                (0.0..=1.0).contains(&i.line_t)
                    && p.distance(from) > ON_EDGE
                    && p.distance(hinge) > ON_EDGE
            })
        });
        if !inside || crosses {
            return Err(AlterError::SlashOutside);
        }
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
    // The bends from the hinge back to the start, and the other way.
    let back: Vec<Point> = bends.iter().rev().copied().collect();
    let (to_first, to_last): (&[Point], &[Point]) = if a <= b {
        (&back, bends)
    } else {
        (bends, &back)
    };
    // Between the ends, going along the outline, and back by the slash.
    let mut inner = vec![first];
    inner.extend(
        flat.iter()
            .filter(|(_, s)| *s > lo && *s < hi)
            .map(|(p, _)| *p),
    );
    inner.push(last);
    inner.extend(to_first.iter());
    // The rest, on round past the outline's start.
    let mut outer = vec![last];
    outer.extend(
        flat.iter()
            .filter(|(_, s)| *s > hi)
            .chain(flat.iter().filter(|(_, s)| *s < lo))
            .map(|(p, _)| *p),
    );
    outer.push(first);
    outer.extend(to_last.iter());
    let along = (hinge - from) / length;
    // Right of the slash, going along it as the pattern is seen (y down).
    let right = Vec2::new(-along.y, along.x);
    let side = |points: &[Point]| {
        points.iter().map(|p| (*p - from).dot(right)).sum::<f64>() / points.len() as f64
    };
    let (between, want) = match turns {
        Some(turns) => {
            let want = match turns {
                Turns::Right => 1.0,
                Turns::Left => -1.0,
            };
            (side(&inner) * want > side(&outer) * want, want)
        }
        None => {
            let between = polygon_area(&inner).abs() <= polygon_area(&outer).abs();
            (
                between,
                side(if between { &inner } else { &outer }).signum(),
            )
        }
    };
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
            path,
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
    // Down the grain to the waist, and across to the side and to the
    // centre front.
    let down = down_the_grain(shape);
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

/// How far a shoulder's slash, from `shoulder` (on the shoulder seam) to
/// the hinge at `armhole`, opens for the shoulder's end to move `by` mm
/// along the shoulder seam.
fn shoulder_opening(
    outline: &Outline,
    shoulder: Point,
    armhole: Point,
    by: f64,
) -> Result<f64, AlterError> {
    let n = outline.edges.len();
    let (Some(k), Some(m)) = (edge_of(outline, shoulder), edge_of(outline, armhole)) else {
        return Err(AlterError::NotOnEdge);
    };
    // The shoulder's end, by the armhole, and the way along it there.
    let (start, end) = (
        Point::from(outline.edge_start(k)),
        Point::from(outline.edge_end(k)),
    );
    let (neck, end) = if m == (k + n - 1) % n
        || (m != (k + 1) % n && start.distance(armhole) < end.distance(armhole))
    {
        (end, start)
    } else {
        (start, end)
    };
    let along = (end - neck) / (end - neck).hypot().max(1e-9);
    // Turned about the hinge, the end moves square to its line from it.
    let r = end - armhole;
    let moves = Vec2::new(-r.y, r.x) / r.hypot().max(1e-9);
    let share = moves.dot(along).abs();
    if share < 0.1 {
        return Err(AlterError::SlashOutside);
    }
    Ok(by * shoulder.distance(armhole) / (r.hypot() * share))
}

/// Down the shape's grainline, as the pattern is seen (y down without one).
fn down_the_grain(shape: &Drawn) -> Vec2 {
    shape
        .grainline
        .map(|g| {
            let d = Point::from(g.to) - Point::from(g.from);
            let d = d / d.hypot().max(1e-9);
            if d.y < 0.0 { -d } else { d }
        })
        .unwrap_or(Vec2::new(0.0, 1.0))
}

/// The edge nearest to `p` (by index).
fn edge_of(outline: &Outline, p: Point) -> Option<usize> {
    (0..outline.edges.len())
        .filter_map(|i| outline.edge_geom(i).nearest(p).map(|n| (i, n.distance)))
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(i, _)| i)
}

// ── Pivoting a dart ──────────────────────────────────────────────────────

/// A dart pivoted: the slash to its tip opens into a new dart as the part
/// beyond it turns.
struct Pivoting {
    slash: Slash,
    /// The new dart's mouth: where the slash starts, on the part that stays
    /// and on the part that turns.
    mouth: (Point, Point),
}

impl Cut for Pivoting {
    fn crossings(&self, seg: &PathSeg) -> Vec<f64> {
        self.slash.crossings(seg)
    }

    fn part(&self, at: Point) -> Affine {
        self.slash.part(at)
    }

    fn join(&self, end: Point, start: Point) -> Option<Vec<PathSeg>> {
        let near = |p: Point, q: Point| p.distance(q) < 1e-3;
        let (p, q) = self.mouth;
        let tip = self.slash.hinge();
        (near(end, p) && near(start, q) || near(end, q) && near(start, p)).then(|| {
            vec![
                PathSeg::Line(Line::new(end, tip)),
                PathSeg::Line(Line::new(tip, start)),
            ]
        })
    }
}

/// A dart's tip on its edge (`geom`), between where its legs meet the edge
/// (`from` and `to`, mm along it): the point furthest from its mouth (a
/// corner, or where its legs curve, a point along them), and how far along
/// the edge it is.
fn dart_tip(geom: &EdgeGeom, from: f64, to: f64) -> (Point, f64) {
    let (a, b) = (geom.point_at(from), geom.point_at(to));
    let mouth = b - a;
    let off = |p: Point| (p - a).cross(mouth).abs();
    let corners = (0..geom.segs().len())
        .map(|k| geom.along_at(k, 1.0))
        .filter(|s| *s > from && *s < to);
    let along = (0..=200).map(|k| from + (to - from) * f64::from(k) / 200.0);
    corners
        .chain(along)
        .map(|s| (geom.point_at(s), s))
        .reduce(|best, x| if off(x.0) > off(best.0) { x } else { best })
        .unwrap_or((a.midpoint(b), (from + to) / 2.0))
}

/// Curves `segs` with the stretch between `from` and `to` (mm along them)
/// taken out, the ends either side joined straight where they do not meet:
/// the curves, and how long the join is.
fn without(segs: &[PathSeg], from: f64, to: f64) -> (Vec<PathSeg>, f64) {
    let geom = EdgeGeom::new(segs.to_vec());
    let ((k0, t0), (k1, t1)) = (geom.locate(from), geom.locate(to));
    let (a, b) = (geom.point_at(from), geom.point_at(to));
    let gap = if a.distance(b) > SAME_POINT {
        a.distance(b)
    } else {
        0.0
    };
    let mut out: Vec<PathSeg> = segs[..k0].to_vec();
    let head = segs[k0].subsegment(0.0..t0);
    if !seg_len_zero(&head) {
        out.push(head);
    }
    if gap > 0.0 {
        out.push(PathSeg::Line(Line::new(a, b)));
    }
    let tail = segs[k1].subsegment(t1..1.0);
    let rest = (!seg_len_zero(&tail))
        .then_some(tail)
        .into_iter()
        .chain(segs[k1 + 1..].iter().copied());
    for (n, seg) in rest.enumerate() {
        // Going on from where the curves before end.
        let seg = if n == 0 {
            with_start(seg, out.last().map_or(a, ParamCurve::end))
        } else {
            seg
        };
        out.push(seg);
    }
    (out, gap)
}

/// The shape with its dart `dart_id` pivoted to `to`, on its edge, by
/// alteration `id`: `share` of it (0 to 1) moved there.
fn pivot(
    shape: &Drawn,
    id: &str,
    dart_id: &str,
    to: Point,
    share: f64,
) -> Result<Drawn, AlterError> {
    const ON_EDGE: f64 = 2.0;
    let outline = &shape.outline;
    let no_dart = || AlterError::NoDart {
        dart: dart_id.to_owned(),
    };
    let dart = shape
        .darts
        .iter()
        .find(|d| d.id == dart_id)
        .ok_or_else(no_dart)?;
    let i = outline.edge_index(&dart.edge).ok_or_else(no_dart)?;
    let geom = outline.edge_geom(i);
    let (a, b) = (geom.point_at(dart.from), geom.point_at(dart.to));
    let (tip, tip_along) = dart_tip(&geom, dart.from, dart.to);
    let Some((p, s_p, d_p)) = on_outline(outline, to) else {
        return Err(AlterError::NotOnEdge);
    };
    if d_p > ON_EDGE {
        return Err(AlterError::NotOnEdge);
    }
    // Along the outline from its start: the dart's ends and its tip.
    let before: f64 = (0..i).map(|k| outline.edge_geom(k).length()).sum();
    let (s_a, s_t, s_b) = (before + dart.from, before + tip_along, before + dart.to);
    if s_p > s_a - ON_EDGE && s_p < s_b + ON_EDGE {
        return Err(AlterError::OntoDart);
    }
    let share = share.clamp(0.0, 1.0);
    if share == 0.0 {
        return Ok(shape.clone());
    }
    // The slash runs inside the piece, meeting its edge only at its ends.
    let polygon = outline_points(outline);
    let slash = Line::new(p, tip);
    let inside = [0.25, 0.5, 0.75].iter().all(|&t| {
        let q = slash.eval(t);
        point_in_polygon(q, &polygon) && on_outline(outline, q).is_some_and(|o| o.2 > 0.5)
    });
    let crosses = outline.path_segs().iter().any(|seg| {
        seg.intersect_line(slash).into_iter().any(|x| {
            let q = slash.eval(x.line_t);
            (0.0..=1.0).contains(&x.line_t) && q.distance(p) > ON_EDGE && q.distance(tip) > ON_EDGE
        })
    });
    if !inside || crosses {
        return Err(AlterError::SlashOutside);
    }
    // The piece in two along the slash, each part closed by it into a
    // polygon: the smaller part turns.
    let flat = outline_flat_with_along(outline);
    let (lo, hi) = (s_p.min(s_t), s_p.max(s_t));
    let (first, last) = if s_p <= s_t { (p, tip) } else { (tip, p) };
    let mut inner = vec![first];
    inner.extend(
        flat.iter()
            .filter(|(_, s)| *s > lo && *s < hi)
            .map(|(q, _)| *q),
    );
    inner.push(last);
    let mut outer = vec![last];
    outer.extend(
        flat.iter()
            .filter(|(_, s)| *s > hi)
            .chain(flat.iter().filter(|(_, s)| *s < lo))
            .map(|(q, _)| *q),
    );
    outer.push(first);
    let between = polygon_area(&inner).abs() <= polygon_area(&outer).abs();
    let turning = if between { inner } else { outer };
    // The dart's leg on the turning part turns towards the other, closing
    // the dart by `share`.
    let b_turns = (s_b > lo && s_b < hi) == between;
    let (moving, staying) = if b_turns { (b, a) } else { (a, b) };
    let (u, v) = (moving - tip, staying - tip);
    let angle = share * u.cross(v).atan2(u.dot(v));
    let turn = Affine::translate(tip.to_vec2())
        * Affine::rotate(angle)
        * Affine::translate(-tip.to_vec2());
    let opened = turn * p;
    let mut altered = cut_shape(
        &Pivoting {
            slash: Slash {
                path: vec![p, tip],
                outline: outline.clone(),
                lo,
                hi,
                between,
                turning,
                turn,
            },
            mouth: (p, opened),
        },
        shape,
    );
    // The edge the new dart opens in: where both sides of the slash meet it.
    let off_edge = |o: &Outline, k: usize| {
        let g = o.edge_geom(k);
        [p, opened]
            .iter()
            .map(|q| g.nearest(*q).map_or(f64::MAX, |n| n.distance))
            .sum::<f64>()
    };
    let j = (0..altered.outline.edges.len())
        .min_by(|&x, &y| off_edge(&altered.outline, x).total_cmp(&off_edge(&altered.outline, y)))
        .unwrap_or(i);
    // Where the dart was on its edge: as before, unless the new one opened
    // in that edge before it.
    let opens_at = |o: &Outline| {
        let g = o.edge_geom(j);
        let (x, y) = (g.nearest(p), g.nearest(opened));
        x.zip(y)
            .map(|(x, y)| (x.along.min(y.along), x.along.max(y.along)))
    };
    let legs = p.distance(tip) + opened.distance(tip);
    let shift = match opens_at(&altered.outline) {
        Some((from, _)) if j == i && from < dart.from => legs,
        _ => 0.0,
    };
    let (from, to) = (dart.from + shift, dart.to + shift);
    let moved_whole = share >= 1.0 - 1e-9;
    if moved_whole {
        // Closed: its legs lie along each other, and are taken out of the
        // edge. Notches and darts beyond it come back as far.
        let edge = &altered.outline.edges[i];
        let length = altered.outline.edge_geom(i).length();
        let (kept, gap) = without(
            &segs_of(altered.outline.edge_start(i), &edge.segments),
            from,
            to,
        );
        let removed = to - from - gap;
        let back = |s: f64| {
            if s >= to { s - removed } else { s.min(from) }
        };
        let edge_id = edge.id.clone();
        altered.outline.edges[i].segments = to_segments(&kept);
        for notch in altered.notches.iter_mut().filter(|n| n.edge == edge_id) {
            notch.at = match notch.from {
                NotchFrom::Start => back(notch.at),
                NotchFrom::End => length - removed - back(length - notch.at),
            };
        }
        for other in altered.darts.iter_mut().filter(|d| d.edge == edge_id) {
            other.from = back(other.from);
            other.to = back(other.to);
        }
        altered.darts.retain(|d| d.id != dart_id);
    } else if let Some(old) = altered.darts.iter_mut().find(|d| d.id == dart_id) {
        old.from = from;
        old.to = to;
    }
    // The new dart, where the slash opened.
    if let Some((from, to)) = opens_at(&altered.outline) {
        altered.darts.push(Dart {
            id: if moved_whole {
                dart_id.to_owned()
            } else {
                format!("{id}.dart")
            },
            edge: altered.outline.edges[j].id.clone(),
            from,
            to,
        });
    }
    Ok(altered)
}

// ── Waist and hip ────────────────────────────────────────────────────────

/// `segs` cut where they are `alongs` mm along them (each place inside them).
fn split_along(segs: &[PathSeg], alongs: &[f64]) -> Vec<PathSeg> {
    let geom = EdgeGeom::new(segs.to_vec());
    let mut cuts: Vec<(usize, f64)> = alongs
        .iter()
        .map(|&s| geom.locate(s))
        .filter(|&(_, t)| t > 1e-9 && t < 1.0 - 1e-9)
        .collect();
    cuts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    let mut out = Vec::new();
    for (k, seg) in segs.iter().enumerate() {
        let mut from = 0.0;
        for &(_, t) in cuts.iter().filter(|c| c.0 == k) {
            out.push(seg.subsegment(from..t));
            from = t;
        }
        out.push(seg.subsegment(from..1.0));
    }
    out
}

/// Each point of `seg` moved by `moved`.
fn warped(seg: PathSeg, moved: impl Fn(Point) -> Point) -> PathSeg {
    match seg {
        PathSeg::Line(l) => PathSeg::Line(Line::new(moved(l.p0), moved(l.p1))),
        PathSeg::Quad(q) => {
            let c = q.raise();
            cubic([c.p0, c.p1, c.p2, c.p3].map(moved))
        }
        PathSeg::Cubic(c) => cubic([c.p0, c.p1, c.p2, c.p3].map(moved)),
    }
}

/// The shape with a side seam moved out square to the grain: by `by` mm
/// at `at` and beyond it, tapering to nothing at `to` (both on the seam),
/// the edges either side stretched to meet it.
fn widen(shape: &Drawn, at: Point, to: Point, by: f64) -> Result<Drawn, AlterError> {
    const ON_EDGE: f64 = 2.0;
    /// The two points are at least this far apart along the grain, mm.
    const APART: f64 = 10.0;
    let outline = &shape.outline;
    let on = |p: Point, k: usize| {
        outline
            .edge_geom(k)
            .nearest(p)
            .filter(|n| n.distance <= ON_EDGE)
    };
    let edges = 0..outline.edges.len();
    if !edges.clone().any(|k| on(at, k).is_some()) || !edges.clone().any(|k| on(to, k).is_some()) {
        return Err(AlterError::NotOnEdge);
    }
    // The seam both are on (either may be at one of its ends).
    let Some((i, near_at, near_to)) = edges
        .filter_map(|k| Some((k, on(at, k)?, on(to, k)?)))
        .min_by(|x, y| (x.1.distance + x.2.distance).total_cmp(&(y.1.distance + y.2.distance)))
    else {
        return Err(AlterError::NotOneSeam);
    };
    let geom = outline.edge_geom(i);
    // Out of the piece, square to the grain.
    let down = down_the_grain(shape);
    let mut out = Vec2::new(-down.y, down.x);
    if geom.normal_at(near_at.along).dot(out) < 0.0 {
        out = -out;
    }
    let (y_at, y_to) = (
        near_at.point.to_vec2().dot(down),
        near_to.point.to_vec2().dot(down),
    );
    if (y_at - y_to).abs() < APART {
        return Err(AlterError::NotOneSeam);
    }
    let shift =
        |p: Point| out * (by * ((p.to_vec2().dot(down) - y_to) / (y_at - y_to)).clamp(0.0, 1.0));
    let moved = |p: Point| p + shift(p);
    // The seam, cut where its taper starts and stops so that it bends
    // there, every point moved.
    let n = outline.edges.len();
    let segs = split_along(
        &segs_of(outline.edge_start(i), &outline.edges[i].segments),
        &[near_at.along, near_to.along],
    );
    let seam: Vec<PathSeg> = segs.into_iter().map(|s| warped(s, moved)).collect();
    let (start, end) = (
        Point::from(outline.edge_start(i)),
        Point::from(outline.edge_end(i)),
    );
    let (d_start, d_end) = (shift(start), shift(end));
    let mut altered = shape.clone();
    let edges = &mut altered.outline.edges;
    edges[i].segments = to_segments(&seam);
    // The edge before ends where the seam now starts, its last curve
    // leaving at the same angle; the edge after starts where it now ends.
    let before = (i + n - 1) % n;
    if let Some(last) = edges[before].segments.last_mut() {
        *last = shifted_end(last, d_start);
    }
    let after = (i + 1) % n;
    if let Some(first) = edges[after].segments.first_mut() {
        *first = shifted_start(first, d_end);
    }
    if i == 0 {
        altered.outline.start = (Point::from(altered.outline.start) + d_start).into();
    }
    if after == 0 {
        altered.outline.start = (Point::from(altered.outline.start) + d_end).into();
    }
    // Notches and darts where they were on the cloth of their edges.
    let mapped = |edge: &Id, along: f64| -> f64 {
        let Some(k) = outline.edge_index(edge) else {
            return along;
        };
        let p = outline.edge_geom(k).point_at(along);
        let p = if k == i { moved(p) } else { p };
        altered
            .outline
            .edge_geom(k)
            .nearest(p)
            .map_or(along, |n| n.along)
    };
    let notches = altered
        .notches
        .iter()
        .map(|notch| {
            let Some(k) = outline.edge_index(&notch.edge) else {
                return notch.clone();
            };
            let (length, after) = (
                outline.edge_geom(k).length(),
                altered.outline.edge_geom(k).length(),
            );
            let at = match notch.from {
                NotchFrom::Start => mapped(&notch.edge, notch.at),
                NotchFrom::End => after - mapped(&notch.edge, length - notch.at),
            };
            Notch {
                at: at.max(0.0),
                ..notch.clone()
            }
        })
        .collect();
    let darts = altered
        .darts
        .iter()
        .map(|d| {
            let (from, to) = (mapped(&d.edge, d.from), mapped(&d.edge, d.to));
            Dart {
                from: from.min(to),
                to: from.max(to),
                ..d.clone()
            }
        })
        .collect();
    altered.notches = notches;
    altered.darts = darts;
    Ok(altered)
}

/// A curve of the model ending `by` further on, a cubic's last control
/// point moved with its end so that it arrives at the same angle.
fn shifted_end(seg: &Segment, by: Vec2) -> Segment {
    let shift = |p: Pt| -> Pt { (Point::from(p) + by).into() };
    match *seg {
        Segment::Line(l) => Segment::Line(LineSeg { to: shift(l.to) }),
        Segment::Cubic(c) => Segment::Cubic(CubicSeg {
            c2: shift(c.c2),
            to: shift(c.to),
            ..c
        }),
    }
}

/// A curve of the model starting `by` further on (where the curve before
/// now ends), a cubic's first control point moved with it.
fn shifted_start(seg: &Segment, by: Vec2) -> Segment {
    match *seg {
        Segment::Line(l) => Segment::Line(l),
        Segment::Cubic(c) => Segment::Cubic(CubicSeg {
            c1: (Point::from(c.c1) + by).into(),
            ..c
        }),
    }
}
