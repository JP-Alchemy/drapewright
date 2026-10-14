//! Suggesting seams for a pattern that has none yet. Edges that are about
//! as long as each other, shaped alike, notched alike or named alike are
//! likely sewn together, as are edges where the lines across their pieces
//! (a corset's waist and bust lines) meet; so is a hem end with a hem end,
//! and an edge marked to be gathered with a shorter one. A centre edge of a
//! piece cut as a pair may be sewn to its other half. Each likely seam is
//! tried on the garment as far as it is sewn (`assembly`): one that would
//! pinch a piece, or have it both right side and wrong side out, is passed
//! over. Then a sleeve's head goes round what is left of an armhole, which
//! may be the edges of several pieces, and a piece left over (a crotch
//! piece) along an opening, across the centre seam there. Each suggestion
//! says why, and waits to be confirmed.

use std::borrow::Borrow;
use std::cmp::Reverse;

use drapewright_pattern_core::geom::{flatten_segs, segs_of};
use drapewright_pattern_core::stitching::{
    closed_along, closed_polyline, dart_spans, opened_along, polyline_length, stitching_edges,
};
use drapewright_pattern_core::{EdgeKind, Id, NotchFrom, NotchKind, Piece, Pt};
use kurbo::{Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::assembly::{Assembly, CopyEdge, Part};
use crate::model::{Ease, Seam, SeamSide};

/// Why a pair of edges looks like a seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Reason {
    /// Within a few millimetres of each other's length.
    SameLength,
    /// A straight back a little longer than the front it is sewn to, eased
    /// onto it (as a back shoulder is).
    Eased,
    /// Curved alike.
    SameShape,
    /// The same notches, in the same places.
    NotchesMatch,
    /// The lines across both pieces (a waist, a bust line) meet at the
    /// seam, at the same places.
    LinesMeet,
    /// Named alike.
    SameName,
    /// Hems, or folds, meet at the same end.
    EndsMatch,
    /// One edge of a front and one of a back.
    FrontAndBack,
    /// A centre edge of a piece cut as a pair, sewn to its other half.
    CentreOfPair,
    /// Marked to be gathered, to fit a shorter edge.
    Gathered,
    /// Goes on from a seam already suggested, along both pieces.
    Continues,
    /// Round an opening left by other seams (a sleeve's head round an
    /// armhole), as long as it.
    FitsOpening,
    /// Along part of an opening, across the centre seam there (a crotch
    /// piece along a pair of fronts' lower edges), as long as it.
    FitsAlong,
}

/// A seam Drapewright thinks likely, not yet confirmed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Suggestion {
    pub seam: Seam,
    /// How likely, from 0 to 1.
    pub confidence: f64,
    pub reasons: Vec<Reason>,
}

/// Edges longer than this may be seams, mm.
const SHORTEST: f64 = 20.0;
/// Lengths further apart than this share are not one seam.
const MOST_APART: f64 = 0.1;
/// Lengths this close are "the same", mm.
const SAME_LENGTH: f64 = 3.0;
/// A straight back is eased onto a straight front by up to this, mm, and
/// this share of its length: a back shoulder's ease.
const EASED_BACK: f64 = 10.0;
const EASED_SHARE: f64 = 0.06;
/// How likely an eased pair is by its lengths: as likely as a pair a
/// millimetre or two apart, whatever its length.
const EASED_SCORE: f64 = 1.15;
/// Curves are compared at this many points.
const SAMPLES: usize = 21;
/// An edge whose furthest point from its chord is under this is straight, mm.
const STRAIGHT: f64 = 2.0;
/// A pair needs at least this score to be suggested.
const THRESHOLD: f64 = 1.4;
/// A centre edge may bow this much of its length from straight.
const CENTRE_BOW: f64 = 0.02;
/// A seam between two pieces sewn to others where the lines across them
/// meet, but not so itself, needs at least this score.
const PANELLED: f64 = 2.0;
/// A line across a piece ends on an edge when it ends this near it, mm.
const MARK_NEAR: f64 = 2.0;
/// Lines ending this near an edge's ends (at its corners) say nothing, mm.
const MARK_CORNER: f64 = 5.0;
/// Lines ending this near each other say nothing either (hatching, a
/// double line), mm.
const MARK_APART: f64 = 6.0;
/// Lines across two pieces meet at a seam when they reach it this near
/// each other, mm.
const MARKS_MEET: f64 = 3.0;
/// A pair going on from a seam suggested needs only this.
const CONTINUING: f64 = 0.8;
/// An edge gathered is at least this much longer than what it is gathered
/// to, and at most `MOST_GATHERED`.
const LEAST_GATHERED: f64 = 1.08;
const MOST_GATHERED: f64 = 4.0;
/// A sleeve's head is within this share of its armhole's length (eased in,
/// or stretched a little).
const EASED: f64 = 0.06;
/// A notch within this of where a sleeve's head meets a seam of the armhole
/// marks that place, mm.
const NOTCH_SNAP: f64 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Front,
    Back,
    Neither,
}

/// What an edge's neighbour at one end is, as far as seams care.
#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Hem,
    /// A seam with a hem's allowance: likely a hem, though not marked one.
    Deep,
    Fold,
    Other,
}

/// An edge that could be one side of a seam.
struct Candidate {
    piece: usize,
    edge: usize,
    /// How many edges its piece has (to find its neighbours).
    edges: usize,
    piece_id: Id,
    edge_id: Id,
    name: String,
    points: Vec<Point>,
    length: f64,
    /// Kinds and places along the stitching line, from the start.
    notches: Vec<(NotchKind, f64)>,
    /// Where lines across the piece (a waist, a bust line) end on the
    /// edge, along its stitching line from the start.
    marks: Vec<f64>,
    /// The first piece shaped as this one is (as many edges, as long, in
    /// either order): its copies (a lining, the other sleeve) have its
    /// lines in the same places, sewn to it or not.
    shape: usize,
    ends: (End, End),
    side: Side,
    /// A sleeve or a leg: a tube, which may be sewn to itself.
    tube: bool,
    /// The piece's name without "left" or "right", and whether it had one.
    stem: String,
    handed: bool,
    /// On a piece cut once and drawn whole, its mirror image's edge (the
    /// other side's armhole, say).
    counterpart: Option<usize>,
    /// The piece is cut as a mirrored pair.
    pair: bool,
    /// Running with the grain, near enough straight (a centre back may be
    /// shaped a little at the waist).
    with_grain: bool,
    /// The piece's one edge with the grain, and it has no fold: a front's
    /// or a back's, its centre.
    only_straight: bool,
    /// Next to the centre of a piece cut as a pair (which is sewn to its
    /// other half), as a waist or a neckline is.
    at_centre: bool,
    /// Furthest from its chord, mm.
    depth: f64,
    /// Along the top of a piece drawn upright (its grain up and down the
    /// page, or none): a neckline, a waist, a corset's top.
    top: bool,
    /// Marked to be gathered.
    gathered: bool,
    /// On a facing or a lining: sewn only to edges shaped as it is.
    inside: bool,
    /// On a strip (a rouleau, a tie, binding): sewn to nothing by an edge.
    strip: bool,
    /// What the piece is, as far as what it is sewn to goes.
    role: Role,
    /// Its darts, where they are along its stitching line.
    darts: Vec<(f64, f64)>,
    /// On a left or a right piece, the mirror image of this edge on the
    /// piece for the other side (by index), as long as it.
    mirror_of: Option<usize>,
    /// The piece drawn as this one's mirror image, edge for edge, if any:
    /// the other side's, though not named so (by index).
    twin: Option<usize>,
}

/// For each piece, the first piece shaped as it is: as many edges, each as
/// long as one of its own (within a millimetre), in any order.
fn shape_classes(lengths: &[Vec<f64>]) -> Vec<usize> {
    let mut out = Vec::new();
    for (p, a) in lengths.iter().enumerate() {
        let alike = |b: &Vec<f64>| {
            b.len() == a.len() && a.iter().all(|x| b.iter().any(|y| (x - y).abs() <= 1.0))
        };
        out.push(lengths.iter().position(alike).unwrap_or(p));
    }
    out
}

/// For each piece, another drawn as its mirror image if there is one: as
/// many edges, as long as its own taken the other way round.
fn mirror_twins(lengths: &[Vec<f64>]) -> Vec<Option<usize>> {
    let alike = |x: f64, y: f64| (x - y).abs() <= 1.0f64.max(0.005 * x.max(y));
    (0..lengths.len())
        .map(|p| {
            let a = &lengths[p];
            let n = a.len();
            (0..lengths.len()).find(|&q| {
                let b = &lengths[q];
                q != p
                    && b.len() == n
                    && n > 0
                    && (0..n).any(|k| (0..n).all(|i| alike(a[i], b[(k + n - i) % n])))
            })
        })
        .collect()
}

/// What a piece is, from its name, as far as what it is sewn to goes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Role {
    Sleeve,
    Cuff,
    Yoke,
    /// Between the legs: a bodysuit's crotch, a gusset.
    Crotch,
    Other,
}

fn role_of(name: &str) -> Role {
    let name = name.to_lowercase();
    if name.contains("cuff") {
        Role::Cuff
    } else if name.contains("yoke") {
        Role::Yoke
    } else if name.contains("sleeve") {
        Role::Sleeve
    } else if name.contains("crotch") || name.contains("gusset") {
        Role::Crotch
    } else {
        Role::Other
    }
}

/// A facing or a lining.
pub(crate) fn inside(piece: &Piece) -> bool {
    let name = piece.name.to_lowercase();
    name.contains("facing") || name.contains("lining")
}

/// A strip made into something other than a seam: a rouleau (button
/// loops), a tie, binding.
fn strip(piece: &Piece) -> bool {
    let name = piece.name.to_lowercase();
    [
        "rouleau",
        "loop",
        "tie",
        "binding",
        "bias strip",
        "drawstring",
        "belt loop",
    ]
    .iter()
    .any(|w| name.contains(w))
}

/// Which of a piece's edges are marked to be gathered: a wavy line drawn
/// along it (pattern CAD's sign for gathering), or named so.
fn gathered_edges(piece: &Piece) -> Vec<bool> {
    let n = piece.outline.edges.len();
    let outline: Vec<Vec<Point>> = (0..n)
        .map(|i| piece.outline.edge_geom(i).flatten(0.1))
        .collect();
    let mut out: Vec<bool> = piece
        .outline
        .edges
        .iter()
        .map(|e| e.name.to_lowercase().contains("gather"))
        .collect();
    for line in &piece.lines {
        let points = flatten_segs(&segs_of(line.start, &line.segments), 0.1);
        if points.len() < 12 {
            continue;
        }
        let drawn = polyline_length(&points);
        for (e, edge) in outline.iter().enumerate() {
            let length = polyline_length(edge);
            if length <= 0.0 {
                continue;
            }
            // Its points beside the edge, and how far along it each is.
            let beside: Vec<f64> = points
                .iter()
                .filter_map(|&p| {
                    let (d, along) = nearest_on(edge, p);
                    (d <= 25.0).then_some(along)
                })
                .collect();
            if (beside.len() as f64) < 0.8 * points.len() as f64 {
                continue;
            }
            let (low, high) = beside
                .iter()
                .fold((f64::MAX, f64::MIN), |(l, h), &a| (l.min(a), h.max(a)));
            let span = high - low;
            // Along a good part of it, and far longer than it is straight.
            if span >= 0.4 * length && drawn >= 1.12 * span {
                out[e] = true;
            }
        }
    }
    out
}

/// How far `p` is from a polyline, and how far along it the nearest point
/// is.
fn nearest_on(points: &[Point], p: Point) -> (f64, f64) {
    let (mut best, mut best_along, mut walked) = (f64::MAX, 0.0, 0.0);
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        let d = b - a;
        let l = d.hypot();
        let t = if l > 0.0 {
            ((p - a).dot(d) / (l * l)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let q = a + d * t;
        let dist = q.distance(p);
        if dist < best {
            best = dist;
            best_along = walked + t * l;
        }
        walked += l;
    }
    (best, best_along)
}

/// Where lines across a piece (a corset's waist and bust lines, a hip
/// line) end on one of its edges, along its stitching line as sewn: the
/// piece sewn to it has its lines end at the same places. Lines along the
/// edge (gathering marks), and ends at its corners, say nothing. `scale`
/// takes the outline's length to the stitching line's.
fn line_marks(piece: &Piece, edge: &[Point], scale: f64, darts: &[(f64, f64)]) -> Vec<f64> {
    let length = polyline_length(edge);
    let on = |p: Pt| {
        let (d, along) = nearest_on(edge, p.into());
        (d <= MARK_NEAR).then_some(along)
    };
    let mut out: Vec<f64> = Vec::new();
    for line in &piece.lines {
        let last = line.segments.last().map_or(line.start, |s| s.to());
        // One end on this edge: across the piece (both: along the edge).
        let along = match (on(line.start), on(last)) {
            (Some(along), None) | (None, Some(along)) => along,
            _ => continue,
        };
        if (MARK_CORNER..=length - MARK_CORNER).contains(&along) {
            out.push(closed_along(darts, along * scale));
        }
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|a, b| (*a - *b).abs() <= 1.5);
    // Only lines on their own: hatching marks the edge all along.
    let mut marks = Vec::new();
    for (k, &m) in out.iter().enumerate() {
        if (k == 0 || m - out[k - 1] > MARK_APART)
            && out.get(k + 1).is_none_or(|&n| n - m > MARK_APART)
        {
            marks.push(m);
        }
    }
    // Lines evenly spaced (a ruffle marked in equal parts) meet any edge so
    // marked.
    let n = marks.len();
    let (mut least, mut most) = (f64::MAX, f64::MIN);
    for w in marks.windows(2) {
        least = least.min(w[1] - w[0]);
        most = most.max(w[1] - w[0]);
    }
    if n >= 3 && most - least <= 0.1 * (marks[n - 1] - marks[0]) / (n - 1) as f64 {
        return Vec::new();
    }
    marks
}

/// Whether the lines across `a`'s and `b`'s pieces meet at the seam
/// between them, sewn the given way round, at the same places: most of the
/// marks on the edge with fewer, and two at least, have one on the other
/// within `MARKS_MEET`. Sewn from both starts, or (edges not as long as
/// each other) from both far ends: how far `b` is moved along `a`.
fn lines_meet(a: &Candidate, b: &Candidate, reversed: bool) -> Option<f64> {
    // Copies of one shape (a lining and its front, two sleeves) have their
    // lines in the same places whether sewn together or not.
    let fewer = a.marks.len().min(b.marks.len());
    if fewer < 2 || a.shape == b.shape || a.inside || b.inside {
        return None;
    }
    let met = |shift: f64| {
        a.marks
            .iter()
            .filter(|&&m| {
                b.marks.iter().any(|&n| {
                    let at = if reversed { b.length - n } else { n } + shift;
                    (m - at).abs() <= MARKS_MEET
                })
            })
            .count()
    };
    let mut shifts = vec![0.0];
    if (a.length - b.length).abs() > SAME_LENGTH {
        shifts.push(a.length - b.length);
    }
    shifts
        .into_iter()
        .map(|shift| (met(shift), shift))
        .filter(|&(m, _)| m >= 2 && 4 * m >= 3 * fewer)
        .max_by_key(|&(m, _)| m)
        .map(|(_, shift)| shift)
}

/// The two sides of a seam between `a` and `b` (which copy of each), sewn
/// the given way round: whole edges, or, edges not as long as each other
/// whose lines meet, the shorter whole along the part of the longer the
/// lines say (a corset's front panel running on below the next).
fn sides(
    a: &Candidate,
    a_mirror: bool,
    b: &Candidate,
    b_mirror: bool,
    reversed: bool,
) -> (SeamSide, SeamSide) {
    let (mut sa, mut sb) = (whole(a, a_mirror), whole(b, b_mirror));
    if (a.length - b.length).abs() <= SAME_LENGTH {
        return (sa, sb);
    }
    let Some(shift) = lines_meet(a, b, reversed) else {
        return (sa, sb);
    };
    if a.length > b.length {
        sa.from = Some(opened_along(&a.darts, shift));
        sa.to = Some(opened_along(&a.darts, shift + b.length));
    } else {
        // `a` lies along `b` from -shift (from b's start, or its end when
        // sewn reversed).
        let (low, high) = (-shift, a.length - shift);
        let (from, to) = if reversed {
            (b.length - high, b.length - low)
        } else {
            (low, high)
        };
        sb.from = Some(opened_along(&b.darts, from.max(0.0)));
        sb.to = Some(opened_along(&b.darts, to.min(b.length)));
    }
    (sa, sb)
}

/// Notches on the same place (a pattern drawn with a notch on its cutting
/// line and another on its stitching line) as one.
fn one_each(mut notches: Vec<(NotchKind, f64)>) -> Vec<(NotchKind, f64)> {
    notches.sort_by(|a, b| a.1.total_cmp(&b.1));
    let mut out: Vec<(NotchKind, f64)> = Vec::new();
    for n in notches {
        match out.last() {
            Some(last) if last.0 == n.0 && (n.1 - last.1).abs() <= 1.5 => {}
            _ => out.push(n),
        }
    }
    out
}

/// A piece's name without "left" and "right": "Left front" → "front".
fn stem(name: &str) -> String {
    name.to_lowercase()
        .split_whitespace()
        .filter(|w| *w != "left" && *w != "right")
        .collect::<Vec<_>>()
        .join(" ")
}

/// For each edge of a piece drawn whole, the edge that is its mirror image
/// across the piece's middle (along the grain), if any.
fn counterparts(piece: &Piece, stitching: &[Vec<Point>], axis: Vec2) -> Vec<Option<usize>> {
    let b = piece.outline.bounds();
    let centre = Point::new((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0);
    let reflect = |p: Point| {
        let v = p - centre;
        centre + (axis * (2.0 * v.dot(axis)) - v)
    };
    let samples = |points: &[Point]| -> Vec<Point> {
        let total = polyline_length(points);
        (0..11)
            .map(|k| point_along(points, total * f64::from(k) / 10.0))
            .collect()
    };
    let n = stitching.len();
    (0..n)
        .map(|f| {
            if stitching[f].len() < 2 {
                return None;
            }
            // Mirrored, an outline runs the other way.
            let mut image: Vec<Point> = stitching[f].iter().map(|p| reflect(*p)).collect();
            image.reverse();
            let image = samples(&image);
            (0..n)
                .filter(|&g| g != f && stitching[g].len() >= 2)
                .find(|&g| {
                    let other = samples(&stitching[g]);
                    image
                        .iter()
                        .zip(&other)
                        .map(|(p, q)| p.distance(*q))
                        .sum::<f64>()
                        / 11.0
                        < 3.0
                })
        })
        .collect()
}

fn side_of(name: &str) -> Side {
    let name = name.to_lowercase();
    if name.contains("front") {
        Side::Front
    } else if name.contains("back") {
        Side::Back
    } else {
        Side::Neither
    }
}

/// What a piece's edge is, as an end of its neighbours: a hem is marked as
/// one, or likely one where it has a deeper allowance than the seams (as a
/// pattern with its allowance drawn gives it).
fn end_of(piece: &Piece, index: usize) -> End {
    match piece.outline.edges[index].kind {
        EdgeKind::Hem => End::Hem,
        EdgeKind::Fold => End::Fold,
        EdgeKind::Seam if deeper(piece, index) => End::Deep,
        EdgeKind::Seam => End::Other,
    }
}

/// A grainline this much up and down the page (the cosine) has its piece
/// drawn upright.
const UPRIGHT_GRAIN: f64 = 0.9;
/// An edge along this top share of a piece drawn upright is along its top.
const TOP_SHARE: f64 = 0.15;

/// A seam's allowance this much deeper than the piece's own, and at least
/// `HEM_ALLOWANCE` mm, is a hem's.
const HEM_DEEPER: f64 = 1.5;
const HEM_ALLOWANCE: f64 = 15.0;

/// Whether an edge marked as a seam has a hem's allowance.
fn deeper(piece: &Piece, index: usize) -> bool {
    let allowance = piece.edge_allowance(index);
    allowance >= HEM_DEEPER * piece.allowance.seam.max(1.0) && allowance >= HEM_ALLOWANCE
}

/// The edges that could be seams (`every`: all edges, for judging a pair
/// chosen by hand), leaving out those in `taken`.
fn candidates<P: Borrow<Piece>>(pieces: &[P], taken: &[(Id, Id)], every: bool) -> Vec<Candidate> {
    let lengths = sewn_lengths(pieces);
    let twins = mirror_twins(&lengths);
    let shapes = shape_classes(&lengths);
    let mut out = Vec::new();
    for (pi, piece) in pieces.iter().map(Borrow::borrow).enumerate() {
        let stitching = stitching_edges(piece);
        // As sewn: each edge's darts closed.
        let darts: Vec<Vec<(f64, f64)>> = (0..stitching.len())
            .map(|e| dart_spans(piece, e, &stitching[e]))
            .collect();
        let sewn: Vec<Vec<Point>> = stitching
            .iter()
            .zip(&darts)
            .map(|(line, spans)| closed_polyline(line, spans))
            .collect();
        let edges = &piece.outline.edges;
        let n = edges.len();
        let pair = piece
            .cut
            .iter()
            .any(|c| c.quantity >= 2 && c.mirrored != Some(false));
        let grain = piece.grainline.as_ref().map(|g| {
            let d = Point::from(g.to) - Point::from(g.from);
            d / d.hypot().max(1e-9)
        });
        let mirrored = if pair || piece.has_fold() {
            vec![None; n]
        } else {
            counterparts(piece, &sewn, grain.unwrap_or(Vec2::new(0.0, 1.0)))
        };
        let gathered = gathered_edges(piece);
        let first = out.len();
        // How high the piece reaches on the page, as sewn.
        let (piece_top, piece_bottom) = sewn
            .iter()
            .flatten()
            .fold((f64::MAX, f64::MIN), |(t, b), p| (t.min(p.y), b.max(p.y)));
        for (ei, edge) in edges.iter().enumerate() {
            let points = &sewn[ei];
            let length = polyline_length(points);
            let used = taken.iter().any(|(p, e)| *p == piece.id && *e == edge.id);
            if points.len() < 2
                || (!every && (edge.kind != EdgeKind::Seam || length < SHORTEST || used))
            {
                continue;
            }
            let outline_length = piece.outline.edge_geom(ei).length();
            let scale = if outline_length > 0.0 {
                polyline_length(&stitching[ei]) / outline_length
            } else {
                1.0
            };
            let notches = one_each(
                piece
                    .notches
                    .iter()
                    .filter(|m| m.edge == edge.id)
                    .map(|m| {
                        let at = match m.from {
                            NotchFrom::Start => m.at,
                            NotchFrom::End => outline_length - m.at,
                        };
                        (m.kind, closed_along(&darts[ei], at * scale))
                    })
                    .collect(),
            );
            let marks = line_marks(
                piece,
                &piece.outline.edge_geom(ei).flatten(0.1),
                scale,
                &darts[ei],
            );
            let chord = points[points.len() - 1] - points[0];
            let depth = profile(points).iter().fold(0.0f64, |m, y| m.max(y.abs()));
            let with_grain = depth < STRAIGHT.max(CENTRE_BOW * length)
                && grain.is_some_and(|g| (chord / chord.hypot().max(1e-9)).dot(g).abs() > 0.985);
            let upright = grain.is_none_or(|g| g.y.abs() > UPRIGHT_GRAIN);
            let mean_y = points.iter().map(|p| p.y).sum::<f64>() / points.len() as f64;
            let top = upright
                && chord.y.abs() < 0.5 * length
                && mean_y <= piece_top + TOP_SHARE * (piece_bottom - piece_top);
            out.push(Candidate {
                piece: pi,
                edge: ei,
                edges: n,
                piece_id: piece.id.clone(),
                edge_id: edge.id.clone(),
                name: edge.name.trim().to_lowercase(),
                points: points.clone(),
                length,
                notches,
                marks,
                shape: shapes[pi],
                ends: (end_of(piece, (ei + n - 1) % n), end_of(piece, (ei + 1) % n)),
                side: side_of(&piece.name),
                // A front or back leg is half a tube.
                tube: side_of(&piece.name) == Side::Neither
                    && ["sleeve", "leg"]
                        .iter()
                        .any(|w| piece.name.to_lowercase().contains(w)),
                stem: stem(&piece.name),
                handed: stem(&piece.name) != piece.name.to_lowercase().trim(),
                counterpart: mirrored[ei],
                pair,
                with_grain,
                depth,
                top,
                gathered: gathered[ei],
                inside: inside(piece),
                strip: strip(piece),
                role: role_of(&piece.name),
                darts: darts[ei].clone(),
                mirror_of: None,
                twin: twins[pi],
                only_straight: false,
                at_centre: false,
            });
        }
        let straight = out[first..].iter().filter(|c| c.with_grain).count();
        for c in &mut out[first..] {
            c.only_straight = c.with_grain && straight == 1 && !piece.has_fold();
        }
        let centre = out[first..]
            .iter()
            .find(|c| c.only_straight && c.pair)
            .map(|c| c.edge);
        for c in &mut out[first..] {
            c.at_centre = centre
                .is_some_and(|e| c.edge != e && ((c.edge + 1) % n == e || (e + 1) % n == c.edge));
        }
    }
    // A left piece's edges mirror a right piece's: as long, and shaped
    // alike run the other way.
    for i in 0..out.len() {
        let found = (0..out.len()).find(|&j| {
            let (a, b) = (&out[i], &out[j]);
            ((a.handed && b.handed && a.stem == b.stem) || a.twin == Some(b.piece))
                && a.piece != b.piece
                && (a.length - b.length).abs() <= 1.0
                && (a.depth < STRAIGHT && b.depth < STRAIGHT || unlike(a, b, true) < 2.0)
                && a.notches.len() == b.notches.len()
        });
        out[i].mirror_of = found;
    }
    out
}

/// Each piece's edges' lengths as sewn (darts closed), mm: for putting the
/// garment together.
fn sewn_lengths<P: Borrow<Piece>>(pieces: &[P]) -> Vec<Vec<f64>> {
    pieces
        .iter()
        .map(|p| {
            let piece = p.borrow();
            let stitching = stitching_edges(piece);
            (0..stitching.len())
                .map(|e| {
                    let spans = dart_spans(piece, e, &stitching[e]);
                    polyline_length(&closed_polyline(&stitching[e], &spans))
                })
                .collect()
        })
        .collect()
}

/// How far each of `SAMPLES` points, evenly along the curve, is to the
/// left of its chord, mm.
fn profile(points: &[Point]) -> Vec<f64> {
    let (a, b) = (points[0], points[points.len() - 1]);
    let chord = b - a;
    let len = chord.hypot();
    if len <= 0.0 {
        return vec![0.0; SAMPLES];
    }
    let along = chord / len;
    let left = Vec2::new(along.y, -along.x);
    let total = polyline_length(points);
    (0..SAMPLES)
        .map(|k| {
            let p = point_along(points, total * k as f64 / (SAMPLES - 1) as f64);
            (p - a).dot(left)
        })
        .collect()
}

fn point_along(points: &[Point], at: f64) -> Point {
    let mut walked = 0.0;
    for w in points.windows(2) {
        let d = w[0].distance(w[1]);
        if walked + d >= at && d > 0.0 {
            return w[0].lerp(w[1], (at - walked) / d);
        }
        walked += d;
    }
    points[points.len() - 1]
}

/// How unlike two curves are, mm: their profiles compared point by point,
/// one of them perhaps run backwards and seen from the other side of the
/// fabric (so mirrored), whichever is closest.
fn unlike(a: &Candidate, b: &Candidate, reversed: bool) -> f64 {
    let pa = profile(&a.points);
    let mut pb = profile(&b.points);
    if reversed {
        pb.reverse();
        // Run backwards, left is right.
        pb.iter_mut().for_each(|y| *y = -*y);
    }
    let mean = |sign: f64| {
        pa.iter()
            .zip(&pb)
            .map(|(x, y)| (x - sign * y).abs())
            .sum::<f64>()
            / SAMPLES as f64
    };
    mean(1.0).min(mean(-1.0))
}

/// How likely `a` and `b` are sewn together, sewn the given way round,
/// and why.
fn score(a: &Candidate, b: &Candidate, reversed: bool) -> (f64, Vec<Reason>) {
    let mut reasons = Vec::new();
    let longer = a.length.max(b.length);
    let apart = (a.length - b.length).abs();
    let mut score = 1.0 - apart / longer / MOST_APART;
    let meet = lines_meet(a, b, reversed);
    // A back a little longer than its front, both straight: eased onto it.
    let back_longer = match (a.side, b.side) {
        (Side::Back, Side::Front) => a.length > b.length,
        (Side::Front, Side::Back) => b.length > a.length,
        _ => false,
    };
    let eased = back_longer
        && a.depth < STRAIGHT
        && b.depth < STRAIGHT
        && apart <= EASED_BACK
        && apart <= EASED_SHARE * longer;
    if apart <= SAME_LENGTH {
        reasons.push(Reason::SameLength);
        // Pattern CAD trues seams to the millimetre.
        score += if apart <= 1.0 { 0.3 } else { 0.15 };
        // A short back shoulder a millimetre or two longer: as likely as
        // one eased more.
        if eased {
            score = score.max(EASED_SCORE);
        }
    } else if eased {
        reasons.push(Reason::Eased);
        score = EASED_SCORE;
    } else if meet.is_some() {
        // Sewn along the part of the longer edge where the lines meet: as
        // likely as edges as long as each other.
        score = 1.0;
    }
    if meet.is_some() {
        score += 0.9;
        reasons.push(Reason::LinesMeet);
    }
    // Shape says something only about curves.
    if a.depth >= 3.0 * STRAIGHT && b.depth >= 3.0 * STRAIGHT {
        let d = unlike(a, b, reversed);
        let alike = 1.0 / (1.0 + (d / 2.0).powi(2));
        score += 0.8 * alike;
        if d < 2.0 {
            reasons.push(Reason::SameShape);
        }
    }
    // Notches: the same kinds, in the same places.
    let kinds = |c: &Candidate| {
        let mut k: Vec<usize> = c.notches.iter().map(|n| n.0.count()).collect();
        k.sort_unstable();
        k
    };
    match (a.notches.is_empty(), b.notches.is_empty()) {
        (true, true) => {}
        (false, false) if kinds(a) == kinds(b) => {
            let places_b: Vec<f64> = b
                .notches
                .iter()
                .map(|n| if reversed { b.length - n.1 } else { n.1 })
                .collect();
            let mut places_b = places_b;
            places_b.sort_by(f64::total_cmp);
            let off = a
                .notches
                .iter()
                .zip(&places_b)
                .map(|(n, pb)| (n.1 - pb).abs())
                .sum::<f64>()
                / a.notches.len() as f64;
            if off <= 8.0 {
                score += 0.9;
                reasons.push(Reason::NotchesMatch);
            } else {
                score += 0.2;
            }
        }
        _ => score -= 0.6,
    }
    if !a.name.is_empty() && a.name == b.name {
        score += 1.0;
        reasons.push(Reason::SameName);
    }
    // Hems meet hems, folds meet folds, at the same end.
    let b_ends = if reversed {
        (b.ends.1, b.ends.0)
    } else {
        b.ends
    };
    let mut matched = false;
    let hemmed = |e: End| matches!(e, End::Hem | End::Deep);
    for (x, y) in [(a.ends.0, b_ends.0), (a.ends.1, b_ends.1)] {
        match (x, y) {
            (End::Other, End::Other) => {}
            (x, y) if x == y || (hemmed(x) && hemmed(y)) => {
                score += 0.35;
                matched = true;
            }
            // A deeper allowance says less against: a facing's end at a
            // front's hem has a seam's.
            (End::Deep, End::Other) | (End::Other, End::Deep) => {}
            _ => score -= 0.6,
        }
    }
    if matched {
        reasons.push(Reason::EndsMatch);
    }
    // A crotch goes between the legs: from the bottom of a garment, never
    // its top; and a crotch in two pieces is sewn to itself there.
    let crotch_to_top =
        |x: &Candidate, y: &Candidate| x.role == Role::Crotch && y.role != Role::Crotch && y.top;
    if crotch_to_top(a, b) || crotch_to_top(b, a) {
        score -= 1.0;
    }
    if a.role == Role::Crotch
        && b.role == Role::Crotch
        && a.piece != b.piece
        && apart <= SAME_LENGTH
    {
        score += 0.6;
    }
    let front_and_back = matches!(
        (a.side, b.side),
        (Side::Front, Side::Back) | (Side::Back, Side::Front)
    );
    // A front's centre and a back's, cut as pairs: each is sewn to its own
    // other half.
    if front_and_back && a.pair && b.pair && a.only_straight && b.only_straight {
        score -= 1.0;
    }
    match (a.side, b.side) {
        (Side::Front, Side::Back) | (Side::Back, Side::Front) => {
            // Both at a fold, or at a centre sewn to its other half: a
            // front's and a back's waist or neckline, each sewn to
            // something else.
            let at_fold =
                |c: &Candidate| c.ends.0 == End::Fold || c.ends.1 == End::Fold || c.at_centre;
            if at_fold(a) && at_fold(b) {
                score -= 0.8;
            } else {
                score += 0.3;
                reasons.push(Reason::FrontAndBack);
            }
        }
        _ => {}
    }
    if a.piece == b.piece {
        // A sleeve's underarm, sewn to itself.
        score += 0.3;
    } else if a.handed && b.handed && a.stem == b.stem {
        // A left and a right piece mirror each other edge for edge; few of
        // those edges are sewn together, and those are notched or named.
        // (Halves drawn alike but not named so are often a garment's left
        // and right halves, sewn at the centre: no less likely.)
        score -= 1.0;
    }
    (score, reasons)
}

/// How likely `long`, marked to be gathered, is gathered to fit `short`,
/// and why; none where it can't be (not marked, or not that much longer).
/// Gathered a little is likelier than a lot; a sleeve's to a cuff and a
/// front's or a back's to a yoke likelier than others.
fn gathered_score(long: &Candidate, short: &Candidate) -> Option<(f64, Vec<Reason>)> {
    if !long.gathered
        || short.gathered
        || long.inside
        || short.inside
        || short.strip
        || long.piece == short.piece
    {
        return None;
    }
    let ratio = long.length / short.length.max(1e-9);
    if !(LEAST_GATHERED..=MOST_GATHERED).contains(&ratio) {
        return None;
    }
    let mut score = 1.7 - 0.1 * ratio;
    let fits = match (long.role, short.role) {
        (Role::Sleeve, Role::Cuff) => true,
        (_, Role::Yoke) => long.side != Side::Neither,
        _ => false,
    };
    if fits {
        score += 0.6;
    }
    Some((score, vec![Reason::Gathered]))
}

/// A pair being considered: its score, the two candidates, which way
/// round, and why.
type Pair = (f64, usize, usize, bool, Vec<Reason>);

/// An edge by piece and edge index.
type Place = (usize, usize);

/// Two edges sewn together.
type Sewn = (Place, Place);

/// A pair taken: its seams (with any made on the other side of the body
/// with it), and the candidates they use.
struct Taken {
    pair: Pair,
    seams: Vec<Seam>,
    used: Vec<usize>,
}

/// A whole edge as a side of a seam.
fn whole(c: &Candidate, mirror: bool) -> SeamSide {
    SeamSide {
        mirror,
        ..SeamSide::whole(&c.piece_id, &c.edge_id)
    }
}

fn seam(a: SeamSide, b: SeamSide, reversed: bool) -> Seam {
    Seam {
        id: String::new(),
        name: String::new(),
        a,
        b,
        reversed,
        ease: Ease::Even,
        confirmed: false,
    }
}

/// The ways a pair may be sewn, likeliest first, each as its seams: a
/// piece cut as a pair sewn to a piece drawn whole has its other half sewn
/// to the mirror image of the edge there (a yoke, to both shoulders of a
/// back), or to its mirror image on the piece for the other side (to a
/// left front and a right front); which half goes to which is left to the
/// garment to say.
fn ways(
    cands: &[Candidate],
    i: usize,
    j: usize,
    reversed: bool,
    either: bool,
) -> Vec<(Vec<Seam>, Vec<usize>)> {
    let mut out = ways_round(cands, i, j, reversed);
    if either {
        out.extend(ways_round(cands, i, j, !reversed));
    }
    // One seam, sewn on both copies of pairs alike, before two.
    out.sort_by_key(|(seams, _)| seams.len());
    out
}

/// `ways`, sewn the given way round.
fn ways_round(
    cands: &[Candidate],
    i: usize,
    j: usize,
    reversed: bool,
) -> Vec<(Vec<Seam>, Vec<usize>)> {
    let (a, b) = (&cands[i], &cands[j]);
    let index_of = |piece: usize, edge: usize| {
        cands
            .iter()
            .position(|c| c.piece == piece && c.edge == edge)
    };
    let mut out = Vec::new();
    for (pair, other, pair_is_a) in [(a, b, true), (b, a, false)] {
        if !pair.pair || other.pair {
            continue;
        }
        let k = other
            .counterpart
            .and_then(|g| index_of(other.piece, g))
            .or(other.mirror_of);
        let Some(k) = k else {
            continue;
        };
        let there = &cands[k];
        let sewn = |x: &Candidate, xm: bool, y: &Candidate, ym: bool, r: bool| {
            let (sx, sy) = sides(x, xm, y, ym, r);
            seam(sx, sy, r)
        };
        for half in [false, true] {
            let first = if pair_is_a {
                sewn(pair, half, other, false, reversed)
            } else {
                sewn(other, false, pair, half, reversed)
            };
            let second = if pair_is_a {
                sewn(pair, !half, there, false, !reversed)
            } else {
                sewn(there, false, pair, !half, !reversed)
            };
            out.push((vec![first, second], vec![i, j, k]));
        }
        return out;
    }
    let sewn = |am: bool, bm: bool| {
        let (sa, sb) = sides(a, am, b, bm, reversed);
        seam(sa, sb, reversed)
    };
    out.push((vec![sewn(false, false)], vec![i, j]));
    // Two pairs sewn, each half to the other's other half.
    if a.pair && b.pair && a.piece != b.piece {
        out.push((vec![sewn(true, false), sewn(false, true)], vec![i, j]));
    }
    out
}

/// The garment's pieces and edges by index, for a side of a seam.
fn placer<'a, P: Borrow<Piece>>(
    pieces: &'a [P],
) -> impl Fn(&SeamSide) -> Option<(usize, usize)> + 'a {
    move |side: &SeamSide| {
        let pi = pieces.iter().position(|p| p.borrow().id == side.piece)?;
        let ei = pieces[pi].borrow().outline.edge_index(&side.edge)?;
        Some((pi, ei))
    }
}

/// Pairs this close in score are as likely as each other.
const TIE: f64 = 0.05;
/// At most this many pairs as likely as each other, after one edge, are
/// weighed against each other.
const RIVALS: usize = 12;

/// A way to sew a pair, tried on the garment: how far apart the corners
/// it brings together were, how many openings it leaves (counted when
/// needed), how many of its parts are right side out, which pair, its
/// seams and the candidates they use, and the garment so sewn.
struct Way {
    apart: f64,
    openings: Option<usize>,
    faced: usize,
    pair: usize,
    seams: Vec<Seam>,
    uses: Vec<usize>,
    garment: Assembly,
}

impl Way {
    fn openings(&mut self) -> usize {
        *self
            .openings
            .get_or_insert_with(|| self.garment.loops().len())
    }
}

/// The likeliest pairs, each candidate in one at most and none in
/// `banned`, each tried on the garment and taken if it sews.
///
/// Of pairs as likely as each other after the same edge, and of the ways
/// each may be sewn, the one taken brings together corners nearest each
/// other round the garment as sewn so far (a back neck facing's end to the
/// front facing on its own side, beyond a yoke's neck end, not the one
/// across the neck), then the one that closes most openings, then the one
/// in fewest seams (a pair sewn to a pair alike on both sides, not
/// crosswise), then the one that keeps most pieces right side out beside
/// each other (patterns are drawn right side up: a cuff on a sleeve not
/// inside out); `either`: pairs whose way round is not known (gathered) are
/// tried both ways.
fn take<P: Borrow<Piece>>(
    pairs: &[(Pair, bool)],
    banned: &[(usize, usize)],
    cands: &[Candidate],
    pieces: &[P],
    garment: &mut Assembly,
    used: &mut [bool],
) -> Vec<Taken> {
    let place = placer(pieces);
    let mut out = Vec::new();
    let mut tried = vec![false; pairs.len()];
    let open = |k: usize, used: &[bool], tried: &[bool]| {
        let (_, i, j, ..) = pairs[k].0;
        !tried[k] && !used[i] && !used[j] && !banned.contains(&(i, j))
    };
    let mut k = 0;
    while k < pairs.len() {
        if !open(k, used, &tried) {
            k += 1;
            continue;
        }
        let ((top, ti, tj, tr, _), te) = &pairs[k];
        // Its rivals: pairs as likely after the same edges.
        let wanted: Vec<usize> = ways(cands, *ti, *tj, *tr, *te)
            .into_iter()
            .flat_map(|(_, uses)| uses)
            .collect();
        let rivals: Vec<usize> = (k..pairs.len())
            .take_while(|&m| pairs[m].0.0 >= top - TIE)
            .filter(|&m| {
                let (_, i, j, ..) = pairs[m].0;
                open(m, used, &tried) && (m == k || wanted.contains(&i) || wanted.contains(&j))
            })
            .take(RIVALS)
            .collect();
        let mut best: Option<Way> = None;
        for m in rivals {
            let ((_, i, j, reversed, _), either) = &pairs[m];
            let mut sews = false;
            for (seams, uses) in ways(cands, *i, *j, *reversed, *either) {
                if uses.iter().any(|&u| u != *i && u != *j && used[u]) {
                    continue;
                }
                let parts: Vec<Part> = seams
                    .iter()
                    .flat_map(|s| garment.parts(s, &place))
                    .collect();
                let joins: Vec<(usize, usize)> =
                    parts.iter().flat_map(|p| garment.joins(p)).collect();
                let apart: f64 = joins
                    .into_iter()
                    .map(|(x, y)| garment.distance(x, y).unwrap_or(1e6))
                    .sum();
                let mut trial = garment.trial();
                if !trial.sew(&parts) {
                    continue;
                }
                sews = true;
                // Laid out flat beside each other, right sides up.
                let faced = parts
                    .iter()
                    .filter(|p| p.4 == (p.0.mirror == p.2.mirror))
                    .count();
                let mut way = Way {
                    apart,
                    openings: None,
                    faced,
                    pair: m,
                    seams,
                    uses,
                    garment: trial,
                };
                let better = match &mut best {
                    None => true,
                    Some(b) if way.apart < b.apart - 1e-6 => true,
                    Some(b) if way.apart <= b.apart + 1e-6 => {
                        // Nearer, then more openings closed, then fewer
                        // seams, then more right side out.
                        let theirs = (Reverse(b.openings()), b.seams.len(), Reverse(b.faced));
                        let mine = (Reverse(way.openings()), way.seams.len(), Reverse(way.faced));
                        mine < theirs
                    }
                    Some(_) => false,
                };
                if better {
                    best = Some(way);
                }
            }
            if !sews {
                tried[m] = true;
            }
        }
        let Some(way) = best else {
            k += 1;
            continue;
        };
        *garment = way.garment;
        for &u in &way.uses {
            used[u] = true;
        }
        // Sewn the other way round than scored: so suggested.
        let mut pair = pairs[way.pair].0.clone();
        pair.3 = way.seams[0].reversed;
        out.push(Taken {
            pair,
            seams: way.seams,
            used: way.uses,
        });
    }
    out
}

/// At most this many times are the likeliest pairs taken again, without
/// those found not to be seams.
const ROUNDS: usize = 8;

/// Of the seams taken, one meeting where the cloth is closed round a point
/// into a spike, if any: one of those meeting there is not a seam. A seam
/// folding back from another's end (a hem sewn to a hem from a princess
/// seam's) meets one spike, at that end; of those, the least likely. Seams
/// meeting spikes at both ends are left: pieces sewn all round, a cushion.
fn spiked<P: Borrow<Piece>>(
    accepted: &[Taken],
    garment: &mut Assembly,
    pieces: &[P],
) -> Option<(usize, usize)> {
    let spikes = garment.spikes();
    if spikes.is_empty() {
        return None;
    }
    let place = placer(pieces);
    let mut least: Option<(f64, usize, usize)> = None;
    for t in accepted {
        let (score, i, j, ..) = t.pair;
        // How many spikes the seam meets on each copy it is sewn on (a pair's
        // two copies alike): one, or one at each end.
        let parts: Vec<Part> = t
            .seams
            .iter()
            .flat_map(|s| garment.parts(s, &place))
            .collect();
        let mut most = 0;
        for part in &parts {
            let ends: Vec<usize> = garment.joins(part).into_iter().map(|(x, _)| x).collect();
            let met = ends
                .iter()
                .filter(|&&x| spikes.iter().any(|&c| garment.same_place(c, x)))
                .count();
            most = most.max(met);
        }
        if most == 1 && least.is_none_or(|(s, ..)| score < s) {
            least = Some((score, i, j));
        }
    }
    least.map(|(_, i, j)| (i, j))
}

/// Pairs that run alongside other seams rather than being one: both ends
/// meet edges sewn to each other, as a front's and a back's armholes do
/// (each is sewn to the sleeve; the shoulders and sides are the seams). A
/// pair is dropped only when the seams beside it are themselves solid (not
/// alongside others): two rectangles sewn all round are a cushion.
/// `already` holds the seams sewn before, by piece and edge index.
fn alongside(accepted: &[Pair], cands: &[Candidate], already: &[Sewn]) -> Vec<(usize, usize)> {
    /// What sews two ends' neighbours together.
    #[derive(Clone, Copy)]
    enum Support {
        Accepted(usize),
        Sewn,
    }
    let key = |c: &Candidate, step: isize| {
        let n = c.edges as isize;
        (c.piece, ((c.edge as isize + step).rem_euclid(n)) as usize)
    };
    let place = |k: usize| {
        let (_, i, j, ..) = &accepted[k];
        (
            (cands[*i].piece, cands[*i].edge),
            (cands[*j].piece, cands[*j].edge),
        )
    };
    let same = |(p, q): Sewn, x, y| (p == x && q == y) || (p == y && q == x);
    let support = |x: (usize, usize), y: (usize, usize)| {
        if already.iter().any(|pq| same(*pq, x, y)) {
            return Some(Support::Sewn);
        }
        (0..accepted.len())
            .find(|&k| same(place(k), x, y))
            .map(Support::Accepted)
    };
    let supports: Vec<Option<(Support, Support)>> = accepted
        .iter()
        .map(|(_, i, j, reversed, _)| {
            let (a, b) = (&cands[*i], &cands[*j]);
            let (before, after) = if *reversed { (1, -1) } else { (-1, 1) };
            Some((
                support(key(a, -1), key(b, before))?,
                support(key(a, 1), key(b, after))?,
            ))
        })
        .collect();
    let solid = |s: Support| match s {
        Support::Sewn => true,
        Support::Accepted(k) => supports[k].is_none(),
    };
    accepted
        .iter()
        .zip(&supports)
        .filter(|(_, s)| s.is_some_and(|(x, y)| solid(x) && solid(y)))
        .map(|((_, i, j, ..), _)| (*i, *j))
        .collect()
}

/// Seams likely for these pieces, most likely first. Edges already in
/// `existing` seams are left alone; each edge is in one suggestion at most.
pub fn suggest<P: Borrow<Piece>>(pieces: &[P], existing: &[Seam]) -> Vec<Suggestion> {
    let taken: Vec<(Id, Id)> = existing
        .iter()
        .flat_map(|s| [&s.a, &s.b])
        .map(|side| (side.piece.clone(), side.edge.clone()))
        .collect();
    let cands = candidates(pieces, &taken, false);
    let place = placer(pieces);
    // The seams already sewn, by piece and edge index.
    let already: Vec<Sewn> = existing
        .iter()
        .filter_map(|s| Some((place(&s.a)?, place(&s.b)?)))
        .collect();
    // The garment as sewn so far, as far as it can be.
    let mut sewn = Assembly::new(
        &pieces.iter().map(Borrow::borrow).collect::<Vec<_>>(),
        sewn_lengths(pieces),
    );
    for s in existing {
        let parts = sewn.parts(s, &place);
        sewn.sew(&parts);
    }
    // Every pair likely enough (and, less likely, those that might go on
    // from a seam suggested): which way round, and whether that is known.
    let mut pairs: Vec<(Pair, bool)> = Vec::new();
    let mut weak: Vec<(Pair, bool)> = Vec::new();
    let mut gathers: Vec<(Pair, bool)> = Vec::new();
    for i in 0..cands.len() {
        for j in i + 1..cands.len() {
            let (a, b) = (&cands[i], &cands[j]);
            // Only a tube (a sleeve) is sewn to itself; a piece's left and
            // right sides match but are not sewn together.
            if a.piece == b.piece && !a.tube {
                continue;
            }
            // Facings and linings are sewn to edges shaped as they are, and
            // strips to nothing.
            if a.strip || b.strip {
                continue;
            }
            if (a.length - b.length).abs() / a.length.max(b.length) <= MOST_APART {
                let ((s1, r1), (s2, r2)) = (score(a, b, false), score(a, b, true));
                let (s, reversed, reasons) = if s2 > s1 {
                    (s2, true, r2.clone())
                } else {
                    (s1, false, r1.clone())
                };
                let either = (s1 - s2).abs() < 1e-9;
                if s >= THRESHOLD {
                    pairs.push(((s, i, j, reversed, reasons), either));
                    // The other way round, likely too: tried if the garment
                    // will not have it this way (one of two seams between
                    // the same pieces sewn so would turn a piece over).
                    let (other, other_reasons) = if reversed { (s1, r1) } else { (s2, r2) };
                    if !either && other >= THRESHOLD {
                        pairs.push(((other, i, j, !reversed, other_reasons), false));
                    }
                } else if s >= CONTINUING {
                    weak.push(((s, i, j, reversed, reasons), either));
                }
            }
            if let Some((s, reasons)) = gathered_score(a, b).or_else(|| gathered_score(b, a)) {
                // Which way round: as its ends say, if they say.
                let (s1, s2) = (score(a, b, false).0, score(a, b, true).0);
                if s >= THRESHOLD {
                    gathers.push(((s, i, j, s2 > s1, reasons), true));
                }
            }
        }
    }
    // Pieces sewn where the lines across them meet (a corset's panels), or
    // sewn so already: a pattern that marks its seams so marks them all so,
    // and another seam between two of those pieces (a panel's top to
    // another's) needs more than a matching length.
    let mut panelled = vec![false; pieces.len()];
    for ((_, i, j, _, reasons), _) in &pairs {
        if reasons.contains(&Reason::LinesMeet) {
            panelled[cands[*i].piece] = true;
            panelled[cands[*j].piece] = true;
        }
    }
    if !existing.is_empty() {
        let all = candidates(pieces, &[], true);
        let find = |s: &SeamSide| {
            all.iter()
                .find(|c| c.piece_id == s.piece && c.edge_id == s.edge)
        };
        for s in existing {
            if let (Some(a), Some(b)) = (find(&s.a), find(&s.b))
                && lines_meet(a, b, s.reversed).is_some()
            {
                panelled[a.piece] = true;
                panelled[b.piece] = true;
            }
        }
    }
    let likely = |((score, i, j, _, reasons), _): &(Pair, bool)| {
        let (a, b) = (&cands[*i], &cands[*j]);
        !(panelled[a.piece]
            && panelled[b.piece]
            && !reasons.contains(&Reason::LinesMeet)
            && *score < PANELLED)
    };
    pairs.retain(likely);
    weak.retain(likely);
    for list in [&mut pairs, &mut weak, &mut gathers] {
        list.sort_by(|x, y| y.0.0.total_cmp(&x.0.0));
    }
    // The likeliest pairs, then again without any that run alongside the
    // others.
    let mut banned = Vec::new();
    let (mut accepted, mut garment, mut used);
    let mut rounds = 0;
    loop {
        garment = sewn.clone();
        used = vec![false; cands.len()];
        accepted = take(&pairs, &banned, &cands, pieces, &mut garment, &mut used);
        // Gathered edges last, once the garment they are gathered to is put
        // together: which yoke goes to which front, say, is then known.
        accepted.extend(take(
            &gathers,
            &banned,
            &cands,
            pieces,
            &mut garment,
            &mut used,
        ));
        let firsts: Vec<Pair> = accepted.iter().map(|t| t.pair.clone()).collect();
        let mut dropped = alongside(&firsts, &cands, &already);
        // Then a seam that closes the cloth round a point into a spike (two
        // hems sewn to each other from a seam's end, or a front's and a
        // back's armholes from the side seam's top).
        if dropped.is_empty() {
            dropped.extend(spiked(&accepted, &mut garment, pieces));
        }
        if dropped.is_empty() || rounds == ROUNDS {
            break;
        }
        rounds += 1;
        banned.extend(dropped);
    }
    // Then pairs less likely by themselves that go on from a seam taken,
    // along both pieces: a facing's edges, one after another, laid on the
    // edges it faces. (Other pieces are opened out flat at a seam: what
    // goes on from it, a front's waist and a back's, is not sewn
    // together.)
    loop {
        let going_on: Vec<(Pair, bool)> = weak
            .iter()
            .filter(|((_, i, j, reversed, _), either)| {
                let (a, b) = (&cands[*i], &cands[*j]);
                !used[*i]
                    && !used[*j]
                    && a.inside != b.inside
                    && (continues(&mut garment, a, b, *reversed)
                        || (*either && continues(&mut garment, a, b, !*reversed)))
            })
            .map(|((s, i, j, r, reasons), either)| {
                let mut reasons = reasons.clone();
                reasons.push(Reason::Continues);
                ((*s + 0.6, *i, *j, *r, reasons), *either)
            })
            .collect();
        let more = take(&going_on, &banned, &cands, pieces, &mut garment, &mut used);
        if more.is_empty() {
            break;
        }
        accepted.extend(more);
    }
    let mut out = Vec::new();
    for t in &accepted {
        let (s, i, j, _, reasons) = &t.pair;
        let (a, b) = (&cands[*i], &cands[*j]);
        let name = if a.name == b.name {
            sentence(&a.name)
        } else {
            String::new()
        };
        for seam in &t.seams {
            out.push(Suggestion {
                seam: Seam {
                    name: name.clone(),
                    ..seam.clone()
                },
                confidence: (s / 3.0).clamp(0.0, 1.0),
                reasons: reasons.clone(),
            });
        }
        debug_assert!(t.used.iter().all(|&k| used[k]));
    }
    // A centre edge of a piece cut as a pair, left over: sewn to its other
    // half (a centre back seam). A piece has one centre: none where one of
    // its edges is sewn to its other half already.
    let mut centred: Vec<bool> = (0..pieces.len())
        .map(|p| {
            already
                .iter()
                .any(|&((pa, ea), (pb, eb))| pa == p && pb == p && ea == eb)
        })
        .collect();
    for (k, c) in cands.iter().enumerate() {
        let named = named_centre(c);
        if used[k]
            || centred[c.piece]
            || !c.pair
            || !(named || c.with_grain && c.side != Side::Neither)
        {
            continue;
        }
        // Only the piece's outermost straight edge across the grain not
        // sewn already: the centre, not a side.
        let others_outside = cands.iter().enumerate().any(|(m, o)| {
            !used[m]
                && o.piece == c.piece
                && o.edge != c.edge
                && o.with_grain
                && o.length > c.length
        });
        if others_outside && !named {
            continue;
        }
        let centre = seam(whole(c, false), whole(c, true), false);
        let parts = garment.parts(&centre, &place);
        if !garment.sew(&parts) {
            continue;
        }
        used[k] = true;
        centred[c.piece] = true;
        out.push(Suggestion {
            seam: Seam {
                name: sentence(&c.name),
                ..centre
            },
            confidence: if named { 0.6 } else { 0.4 },
            reasons: vec![Reason::CentreOfPair],
        });
    }
    // Trousers: a front and a back sewn to each other twice, at the
    // outseam and the inseam, are a leg; the edge left over from the waist
    // to the inseam's top (the crotch curve) is sewn to its other half.
    for (k, c) in cands.iter().enumerate() {
        if used[k] || !c.pair {
            continue;
        }
        // The piece's edges sewn to one other piece, by the seams taken.
        let sewn_to = |other: usize| -> Vec<usize> {
            accepted
                .iter()
                .filter_map(|t| {
                    let (_, i, j, _, _) = t.pair;
                    let (a, b) = (&cands[i], &cands[j]);
                    if a.piece == c.piece && b.piece == other {
                        Some(i)
                    } else if b.piece == c.piece && a.piece == other {
                        Some(j)
                    } else {
                        None
                    }
                })
                .collect()
        };
        let Some(tube) = cands
            .iter()
            .map(|o| o.piece)
            .filter(|&o| o != c.piece)
            .map(sewn_to)
            .find(|edges| edges.len() >= 2)
        else {
            continue;
        };
        // How high (down the page) an edge's end is where it meets `c`, and
        // its other end.
        let meeting = |e: &Candidate| -> Option<(f64, f64)> {
            let (first, last) = (e.points[0], e.points[e.points.len() - 1]);
            if (c.edge + 1) % c.edges == e.edge {
                Some((first.y, last.y))
            } else if (e.edge + 1) % c.edges == c.edge {
                Some((last.y, first.y))
            } else {
                None
            }
        };
        // The tube's seam that `c` meets at its top end, the lower of the
        // tube's tops (an inseam's, at the crotch, not an outseam's, at the
        // waist).
        let tops: Vec<f64> = tube
            .iter()
            .map(|&e| {
                let p = &cands[e].points;
                p[0].y.min(p[p.len() - 1].y)
            })
            .collect();
        let at_inseam = tube.iter().any(|&e| {
            meeting(&cands[e]).is_some_and(|(here, there)| {
                here < there
                    && tops.iter().all(|&t| here >= t - 1.0)
                    && tops.iter().any(|&t| here > t + 1.0)
            })
        });
        let chord = c.points[c.points.len() - 1] - c.points[0];
        let down = chord.y.abs() / chord.hypot().max(1e-9);
        if !at_inseam || down < CROTCH_DOWN {
            continue;
        }
        let centre = seam(whole(c, false), whole(c, true), false);
        let parts = garment.parts(&centre, &place);
        if !garment.sew(&parts) {
            continue;
        }
        used[k] = true;
        out.push(Suggestion {
            seam: Seam {
                name: sentence(&c.name),
                ..centre
            },
            confidence: 0.5,
            reasons: vec![Reason::CentreOfPair],
        });
    }
    out.extend(round_openings(&cands, pieces, &mut garment));
    out.extend(along_openings(&cands, pieces, &mut garment, &used));
    out
}

/// A trouser's crotch curve runs at least this much down the page (the
/// cosine, over its chord): more down than across.
const CROTCH_DOWN: f64 = 0.8;

/// An edge this much longer or shorter than a run of an opening's edges
/// is not sewn along them, mm.
const ALONG: f64 = 1.5;

/// Pieces sewn by one edge along part of an opening of the garment, across
/// the centre seam there: a crotch piece along the lower edges of a pair of
/// fronts, joined at their centre; a band along a neckline. Of the edges
/// not sewn, each along the run of the opening's edges it is as long as
/// (taking in both halves of a pair), the closest first; only where one run
/// fits.
fn along_openings<P: Borrow<Piece>>(
    cands: &[Candidate],
    pieces: &[P],
    garment: &mut Assembly,
    used: &[bool],
) -> Vec<Suggestion> {
    // The garment's free edges in runs, closed or not (a run stops where an
    // edge is sewn along part of it).
    // And how many copies each one's garment is made of.
    let (mut openings, mut closed, mut sizes) = (Vec::new(), Vec::new(), Vec::new());
    for (run, round) in garment.runs() {
        let opening = Opening::new(run, cands, garment);
        sizes.push(garment.joined_to(garment.copy(opening.edges[0].0)));
        openings.push(opening);
        closed.push(round);
    }
    // Every run as long as an edge: (misfit, edge, opening, first edge,
    // edges in the run).
    let mut fits: Vec<(f64, usize, usize, usize, usize)> = Vec::new();
    for (k, c) in cands.iter().enumerate() {
        if used[k] || c.strip || c.length < SHORTEST {
            continue;
        }
        let mine = garment.copy(CopyEdge {
            piece: c.piece,
            edge: c.edge,
            mirror: false,
        });
        let size = garment.joined_to(mine);
        let mut found: Vec<(f64, usize, usize, usize, usize)> = Vec::new();
        for (o, opening) in openings.iter().enumerate() {
            let n = opening.edges.len();
            // An opening of a garment larger than what the piece is sewn
            // to so far: a crotch piece onto a corset, not the other way.
            if sizes[o] <= size || garment.joined(mine, garment.copy(opening.edges[0].0)) {
                continue;
            }
            for first in 0..n {
                // Runs from each edge on, one way: the other way is the
                // same runs. An open run does not go round.
                let mut walked = 0.0;
                let most = if closed[o] { n - 1 } else { n - first };
                for m in 1..=most {
                    walked += opening.lengths[(first + m - 1) % n];
                    if walked > c.length + ALONG {
                        break;
                    }
                    // Across a centre seam: both halves of a pair.
                    let across = (0..m).any(|i| {
                        let (x, _) = opening.edges[(first + i) % n];
                        x.mirror
                            && (0..m).any(|j| {
                                let (y, _) = opening.edges[(first + j) % n];
                                !y.mirror && y.piece == x.piece
                            })
                    });
                    if m >= 2 && across && (walked - c.length).abs() <= ALONG {
                        found.push(((walked - c.length).abs(), k, o, first, m));
                    }
                }
            }
        }
        // Only where one run fits.
        if let [fit] = found.as_slice() {
            fits.push(*fit);
        }
    }
    let mut taken: Vec<CopyEdge> = Vec::new();
    let mut parts: Vec<(Part, f64)> = Vec::new();
    // The closest first (fits are few: one an edge at most).
    let mut tried = vec![false; fits.len()];
    while let Some(x) = (0..fits.len())
        .filter(|&x| !tried[x])
        .min_by(|&x, &y| fits[x].0.total_cmp(&fits[y].0))
    {
        tried[x] = true;
        let (_, k, o, first, m) = fits[x];
        let c = &cands[k];
        let opening = &openings[o];
        // The run's edges: the opening's from `first` on.
        let run = |i: usize| {
            let at = (first + i) % opening.edges.len();
            let (e, forwards) = opening.edges[at];
            (e, forwards, opening.lengths[at])
        };
        let edge = CopyEdge {
            piece: c.piece,
            edge: c.edge,
            mirror: false,
        };
        if taken.contains(&edge) || (0..m).any(|i| taken.contains(&run(i).0)) {
            continue;
        }
        let total: f64 = (0..m).map(|i| run(i).2).sum();
        // Either way along the run, as the garment will have it.
        for along in [true, false] {
            let mut walked = 0.0;
            let mut these: Vec<Part> = Vec::new();
            for i in 0..m {
                let (e, forwards, length) = run(i);
                // A speck of an edge (where two corners nearly meet) is
                // sewn to nothing.
                if length < 1.0 {
                    continue;
                }
                let (lo, hi) = (walked, walked + length * c.length / total);
                walked = hi;
                let (from, to) = if along {
                    (lo, hi)
                } else {
                    (c.length - hi, c.length - lo)
                };
                let stretch = (opened_along(&c.darts, from), opened_along(&c.darts, to));
                these.push((edge, stretch, e, (0.0, length), along != forwards));
            }
            if garment.sew(&these) {
                taken.push(edge);
                for i in 0..m {
                    taken.push(run(i).0);
                }
                for part in these {
                    parts.push((part, 0.6));
                }
                break;
            }
        }
    }
    as_seams(&parts, pieces, cands, garment, Reason::FitsAlong)
}

/// Whether sewing `a` to `b` (that way round) goes on from a seam already
/// sewn: an end of each already meets.
fn continues(garment: &mut Assembly, a: &Candidate, b: &Candidate, reversed: bool) -> bool {
    let edge = |c: &Candidate| CopyEdge {
        piece: c.piece,
        edge: c.edge,
        mirror: false,
    };
    let (ea, eb) = (edge(a), edge(b));
    if !garment.has(ea) || !garment.has(eb) {
        return false;
    }
    let (sa, ta) = (garment.start(ea), garment.end(ea));
    let (sb, tb) = (garment.start(eb), garment.end(eb));
    if reversed {
        garment.meet(sa, tb) || garment.meet(ta, sb)
    } else {
        garment.meet(sa, sb) || garment.meet(ta, tb)
    }
}

/// An opening of the garment, or a tube's open end: its copy edges in
/// order round it (each run forwards or not), each one's length, and its
/// notches' places round it from where it starts.
struct Opening {
    edges: Vec<(CopyEdge, bool)>,
    lengths: Vec<f64>,
    notches: Vec<f64>,
}

impl Opening {
    fn new(edges: Vec<(CopyEdge, bool)>, cands: &[Candidate], garment: &Assembly) -> Opening {
        let lengths: Vec<f64> = edges.iter().map(|(e, _)| garment.length(*e)).collect();
        let mut notches = Vec::new();
        let mut walked = 0.0;
        for (&(e, forwards), &length) in edges.iter().zip(&lengths) {
            if let Some(c) = cands
                .iter()
                .find(|c| c.piece == e.piece && c.edge == e.edge)
            {
                for &(_, at) in &c.notches {
                    notches.push(walked + if forwards { at } else { length - at });
                }
            }
            walked += length;
        }
        Opening {
            edges,
            lengths,
            notches,
        }
    }

    fn length(&self) -> f64 {
        self.lengths.iter().sum()
    }

    /// The same opening, from its `k`th edge on, or round the other way.
    fn from(&self, k: usize, backwards: bool, cands: &[Candidate], garment: &Assembly) -> Opening {
        let n = self.edges.len();
        let mut edges: Vec<(CopyEdge, bool)> = (0..n).map(|i| self.edges[(k + i) % n]).collect();
        if backwards {
            // Round the other way, from the same place: the edge before it
            // first, each run the other way.
            edges = (0..n)
                .map(|i| {
                    let (e, f) = self.edges[(k + n - 1 - i) % n];
                    (e, !f)
                })
                .collect();
        }
        Opening::new(edges, cands, garment)
    }
}

/// How unlike a tube's end and an opening are when sewn round it, each
/// from its start: its notches against the opening's notches and the
/// places its pieces meet, scaled to its length; mm, on average.
fn misfit(tube: &Opening, opening: &Opening) -> f64 {
    let scale = tube.length() / opening.length().max(1e-9);
    let mut marks: Vec<f64> = opening.notches.iter().map(|x| x * scale).collect();
    let mut walked = 0.0;
    for &l in &opening.lengths[..opening.lengths.len().saturating_sub(1)] {
        walked += l;
        marks.push(walked * scale);
    }
    let nearest =
        |x: f64, among: &[f64]| among.iter().map(|y| (x - y).abs()).fold(20.0f64, f64::min);
    let mut cost = 0.0;
    let mut count = 0.0;
    for &t in &tube.notches {
        cost += nearest(t, &marks);
        count += 1.0;
    }
    for &o in &opening.notches {
        cost += nearest(o * scale, &tube.notches);
        count += 1.0;
    }
    if count > 0.0 { cost / count } else { 0.0 }
}

/// Each tube's open end (a sleeve's head) sewn round an opening of the
/// rest of the garment as long as it (an armhole, perhaps the edges of a
/// front, a yoke and a back): split where the opening's edges meet, or at
/// a notch of its own just there, and started where its notches and the
/// opening's best agree (its underarm at the side seam, where none say).
fn round_openings<P: Borrow<Piece>>(
    cands: &[Candidate],
    pieces: &[P],
    garment: &mut Assembly,
) -> Vec<Suggestion> {
    let loops = garment.loops();
    let copy_of = |l: &[(CopyEdge, bool)], g: &Assembly| g.copy(l[0].0);
    let mut tubes = Vec::new();
    let mut openings = Vec::new();
    for l in loops {
        let copy = copy_of(&l, garment);
        let one = l.iter().all(|(e, _)| garment.copy(*e) == copy);
        if one && garment.tube(copy) {
            tubes.push(l);
        } else {
            openings.push(l);
        }
    }
    // Every way round each opening for each tube end, the best first.
    let mut fits: Vec<(f64, usize, usize, usize, bool)> = Vec::new();
    for (t, tube) in tubes.iter().enumerate() {
        let tube_end = Opening::new(tube.clone(), cands, garment);
        for (o, opening) in openings.iter().enumerate() {
            let (tc, oc) = (copy_of(tube, garment), copy_of(opening, garment));
            if garment.joined(tc, oc) {
                continue;
            }
            let round = Opening::new(opening.clone(), cands, garment);
            let (lt, lo) = (tube_end.length(), round.length());
            if (lt - lo).abs() > EASED * lo {
                continue;
            }
            for k in 0..opening.len() {
                for backwards in [false, true] {
                    let from = round.from(k, backwards, cands, garment);
                    let start = garment.first(from.edges[0].0, from.edges[0].1);
                    // Where nothing says otherwise, the underarm at the side
                    // seam: the longest seam round the opening.
                    let side = garment.longest_at(start);
                    let cost = misfit(&tube_end, &from) - 1e-4 * side;
                    fits.push((cost, t, o, k, backwards));
                }
            }
        }
    }
    fits.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut tube_used = vec![false; tubes.len()];
    let mut opening_used = vec![false; openings.len()];
    let mut parts: Vec<(Part, f64)> = Vec::new();
    for (cost, t, o, k, backwards) in fits {
        if tube_used[t] || opening_used[o] {
            continue;
        }
        let tube_end = Opening::new(tubes[t].clone(), cands, garment);
        let round =
            Opening::new(openings[o].clone(), cands, garment).from(k, backwards, cands, garment);
        let these = sewn_round(&tube_end, &round, cands);
        if these.is_empty() || !garment.sew(&these) {
            continue;
        }
        tube_used[t] = true;
        opening_used[o] = true;
        let confidence = (0.9 - cost.max(0.0) / 20.0).clamp(0.3, 0.9);
        parts.extend(these.into_iter().map(|p| (p, confidence)));
    }
    as_seams(&parts, pieces, cands, garment, Reason::FitsOpening)
}

/// A tube's end sewn round an opening, each from its start: the parts of
/// the tube's edges sewn to each of the opening's edges.
fn sewn_round(tube: &Opening, round: &Opening, cands: &[Candidate]) -> Vec<Part> {
    let scale = tube.length() / round.length().max(1e-9);
    // Where each of the opening's edges ends, round the tube: at a notch of
    // the tube's just there.
    let mut cuts = vec![0.0];
    let mut walked = 0.0;
    for &l in &round.lengths[..round.lengths.len() - 1] {
        walked += l;
        let at = walked * scale;
        let snapped = tube
            .notches
            .iter()
            .copied()
            .filter(|n| (n - at).abs() <= NOTCH_SNAP)
            .min_by(|x, y| (x - at).abs().total_cmp(&(y - at).abs()))
            .unwrap_or(at);
        cuts.push(snapped);
    }
    cuts.push(tube.length());
    // Each stretch of the tube, on its own edges.
    let mut out = Vec::new();
    for (k, &(edge, forwards)) in round.edges.iter().enumerate() {
        let (from, to) = (cuts[k], cuts[k + 1]);
        if to - from <= 0.5 {
            return Vec::new();
        }
        let mut start = 0.0;
        for (i, &(te, tf)) in tube.edges.iter().enumerate() {
            let len = tube.lengths[i];
            let (lo, hi) = (from.max(start), to.min(start + len));
            start += len;
            if hi - lo <= 0.5 {
                continue;
            }
            // Along the tube's edge, as sewn: run backwards, from its end.
            let (a, b) = if tf {
                (lo - (start - len), hi - (start - len))
            } else {
                (start - hi, start - lo)
            };
            // On the stitching line, its darts open.
            let darts = cands
                .iter()
                .find(|c| c.piece == te.piece && c.edge == te.edge)
                .map(|c| c.darts.clone())
                .unwrap_or_default();
            let stretch = (opened_along(&darts, a), opened_along(&darts, b));
            let round_darts = cands
                .iter()
                .find(|c| c.piece == edge.piece && c.edge == edge.edge)
                .map(|c| c.darts.clone())
                .unwrap_or_default();
            let round_len = opened_along(&round_darts, round.lengths[k]);
            // Sewn start to start where both run the same way round.
            out.push((te, stretch, edge, (0.0, round_len), tf != forwards));
        }
    }
    out
}

/// Parts sewn on copies, as seams (for `reason`): a part and its mirror
/// image (on the other copies of pairs) as one seam, which is sewn on
/// both; another naming the mirror copies it is on.
fn as_seams<P: Borrow<Piece>>(
    parts: &[(Part, f64)],
    pieces: &[P],
    cands: &[Candidate],
    garment: &Assembly,
    reason: Reason,
) -> Vec<Suggestion> {
    // Whether a part's stretch is all of its edge.
    let whole = |e: CopyEdge, (from, to): (f64, f64)| {
        let darts = cands
            .iter()
            .find(|c| c.piece == e.piece && c.edge == e.edge)
            .map(|c| c.darts.clone())
            .unwrap_or_default();
        from <= 0.5 && to >= opened_along(&darts, garment.length(e)) - 0.5
    };
    let side = |e: CopyEdge, stretch: (f64, f64)| {
        let piece = pieces[e.piece].borrow();
        let all = whole(e, stretch);
        SeamSide {
            piece: piece.id.clone(),
            edge: piece.outline.edges[e.edge].id.clone(),
            from: (!all).then_some(stretch.0),
            to: (!all).then_some(stretch.1),
            mirror: e.mirror,
        }
    };
    let close = |x: (f64, f64), y: (f64, f64)| (x.0 - y.0).abs() < 0.01 && (x.1 - y.1).abs() < 0.01;
    // `q` is `p` on the first copies of the same pieces.
    let first_of = |q: &Part, p: &Part| {
        q.0.piece == p.0.piece
            && q.0.edge == p.0.edge
            && q.2.piece == p.2.piece
            && q.2.edge == p.2.edge
            && close(q.1, p.1)
            && close(q.3, p.3)
            && q.4 == p.4
            && !q.0.mirror
            && !q.2.mirror
    };
    let mut out = Vec::new();
    for (p, confidence) in parts {
        let (a, sa, b, sb, reversed) = *p;
        // Sewn on both copies by the first copies' seam.
        if a.mirror && b.mirror && parts.iter().any(|(q, _)| first_of(q, p)) {
            continue;
        }
        out.push(Suggestion {
            seam: seam(side(a, sa), side(b, sb), reversed),
            confidence: *confidence,
            reasons: vec![reason],
        });
    }
    out
}

/// An edge named for the centre: "centre back", "CF".
fn named_centre(c: &Candidate) -> bool {
    ["centre", "center", "cb", "cf"]
        .iter()
        .any(|w| c.name.split_whitespace().any(|x| x == *w) || c.name.starts_with(w))
}

/// Pieces drawn for one side of the body and cut once, as patterns that
/// draw each panel once often leave them (saying to cut two only in their
/// instructions, if at all): a band of two or more pieces, sewn side by
/// side by `seams` and the seams suggested for them (edges running with
/// the grain, not tops or hems), from a centre front left open to a centre
/// back left open. Each is to be cut as a mirrored pair, a left and a
/// right.
pub fn drawn_for_one_side<P: Borrow<Piece>>(pieces: &[P], seams: &[Seam]) -> Vec<Id> {
    let cands = candidates(pieces, &[], true);
    let find = |s: &SeamSide| {
        cands
            .iter()
            .find(|c| c.piece_id == s.piece && c.edge_id == s.edge)
    };
    // The seams sewn and suggested, but for a front's centre suggested to
    // a back's: the two halves of a pattern drawn for one side, put
    // together as if they were the whole of it.
    let mut sewn: Vec<Seam> = seams.to_vec();
    for s in suggest(pieces, seams) {
        let centres = match (find(&s.seam.a), find(&s.seam.b)) {
            (Some(a), Some(b)) => {
                a.only_straight
                    && b.only_straight
                    && a.side != b.side
                    && a.side != Side::Neither
                    && b.side != Side::Neither
            }
            _ => false,
        };
        if !centres {
            sewn.push(s.seam);
        }
    }
    // Up and down its piece, within 45° of the grain (or it has none).
    let upright = |c: &Candidate| {
        let chord = c.points[c.points.len() - 1] - c.points[0];
        pieces[c.piece].borrow().grainline.as_ref().is_none_or(|g| {
            let g = Point::from(g.to) - Point::from(g.from);
            chord.dot(g).abs() >= 0.7 * chord.hypot() * g.hypot()
        })
    };
    let is_sewn = |piece: &str, edge: &str| {
        sewn.iter().any(|s| {
            [&s.a, &s.b]
                .iter()
                .any(|x| x.piece == piece && x.edge == edge)
        })
    };
    // The pieces joined by seams, each group by its first piece.
    let mut group: Vec<usize> = (0..pieces.len()).collect();
    let root = |group: &[usize], mut i: usize| {
        while group[i] != i {
            i = group[i];
        }
        i
    };
    for s in &sewn {
        if let (Some(a), Some(b)) = (find(&s.a), find(&s.b))
            && upright(a)
            && upright(b)
        {
            let (ra, rb) = (root(&group, a.piece), root(&group, b.piece));
            group[ra.max(rb)] = ra.min(rb);
        }
    }
    // Each piece's centre left open (its outermost straight edge with the
    // grain, or one named for the centre), if it has one.
    let free: Vec<bool> = cands
        .iter()
        .map(|c| !is_sewn(&c.piece_id, &c.edge_id))
        .collect();
    let mut open_centre: Vec<Option<Side>> = vec![None; pieces.len()];
    for (k, c) in cands.iter().enumerate() {
        let named = named_centre(c);
        if !free[k] || c.side == Side::Neither || !(named || c.with_grain) {
            continue;
        }
        let others_outside = cands.iter().enumerate().any(|(m, o)| {
            free[m] && o.piece == c.piece && o.edge != c.edge && o.with_grain && o.length > c.length
        });
        if named || !others_outside {
            open_centre[c.piece] = Some(c.side);
        }
    }
    let once = |p: &Piece| !p.has_fold() && p.cut.iter().all(|c| c.quantity == 1);
    let mut out = Vec::new();
    for first in 0..pieces.len() {
        let members: Vec<usize> = (0..pieces.len())
            .filter(|&p| root(&group, p) == first)
            .collect();
        let has = |side: Side| members.iter().any(|&p| open_centre[p] == Some(side));
        if members.len() >= 2
            && members.iter().all(|&p| once(pieces[p].borrow()))
            && has(Side::Front)
            && has(Side::Back)
        {
            out.extend(members.iter().map(|&p| pieces[p].borrow().id.clone()));
        }
    }
    out
}

/// Whether two edges chosen to be sewn together go the other way round
/// (`a`'s start to `b`'s end), judged as suggestions are: hems meeting
/// hems, notches meeting notches, curves alike.
pub fn sewn_reversed<P: Borrow<Piece>>(a: &SeamSide, b: &SeamSide, pieces: &[P]) -> bool {
    let all = candidates(pieces, &[], true);
    let find = |s: &SeamSide| {
        all.iter()
            .find(|c| c.piece_id == s.piece && c.edge_id == s.edge)
    };
    match (find(a), find(b)) {
        (Some(x), Some(y)) => score(x, y, true).0 > score(x, y, false).0,
        _ => false,
    }
}

/// "side seam" → "Side seam".
fn sentence(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
