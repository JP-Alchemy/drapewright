//! Suggesting seams for a pattern that has none yet. Edges that are about
//! as long as each other, shaped alike, notched alike or named alike are
//! likely sewn together; so is a hem end with a hem end. A centre edge of a
//! piece cut as a pair may be sewn to its other half. Each suggestion says
//! why, and waits to be confirmed.

use std::borrow::Borrow;

use drapewright_pattern_core::stitching::{
    closed_along, closed_polyline, dart_spans, polyline_length, stitching_edges,
};
use drapewright_pattern_core::{EdgeKind, Id, NotchFrom, NotchKind, Piece};
use kurbo::{Point, Vec2};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::{Ease, Seam, SeamSide};

/// Why a pair of edges looks like a seam.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, TS, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Reason {
    /// Within a few millimetres of each other's length.
    SameLength,
    /// Curved alike.
    SameShape,
    /// The same notches, in the same places.
    NotchesMatch,
    /// Named alike.
    SameName,
    /// Hems, or folds, meet at the same end.
    EndsMatch,
    /// One edge of a front and one of a back.
    FrontAndBack,
    /// A centre edge of a piece cut as a pair, sewn to its other half.
    CentreOfPair,
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
/// Curves are compared at this many points.
const SAMPLES: usize = 21;
/// An edge whose furthest point from its chord is under this is straight, mm.
const STRAIGHT: f64 = 2.0;
/// A pair needs at least this score to be suggested.
const THRESHOLD: f64 = 1.4;

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
    /// Straight and running with the grain.
    with_grain: bool,
    /// Furthest from its chord, mm.
    depth: f64,
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

fn end_of(kind: EdgeKind) -> End {
    match kind {
        EdgeKind::Hem => End::Hem,
        EdgeKind::Fold => End::Fold,
        EdgeKind::Seam => End::Other,
    }
}

/// The edges that could be seams (`every`: all edges, for judging a pair
/// chosen by hand), leaving out those in `taken`.
fn candidates<P: Borrow<Piece>>(pieces: &[P], taken: &[(Id, Id)], every: bool) -> Vec<Candidate> {
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
            let mut notches: Vec<(NotchKind, f64)> = piece
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
                .collect();
            notches.sort_by(|a, b| a.1.total_cmp(&b.1));
            let chord = points[points.len() - 1] - points[0];
            let depth = profile(points).iter().fold(0.0f64, |m, y| m.max(y.abs()));
            let with_grain = depth < STRAIGHT
                && grain.is_some_and(|g| (chord / chord.hypot().max(1e-9)).dot(g).abs() > 0.985);
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
                ends: (
                    end_of(edges[(ei + n - 1) % n].kind),
                    end_of(edges[(ei + 1) % n].kind),
                ),
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
            });
        }
    }
    out
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
    if apart <= SAME_LENGTH {
        reasons.push(Reason::SameLength);
        // Pattern CAD trues seams to the millimetre.
        score += if apart <= 1.0 { 0.3 } else { 0.15 };
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
    for (x, y) in [(a.ends.0, b_ends.0), (a.ends.1, b_ends.1)] {
        match (x, y) {
            (End::Other, End::Other) => {}
            (x, y) if x == y => {
                score += 0.35;
                matched = true;
            }
            _ => score -= 0.6,
        }
    }
    if matched {
        reasons.push(Reason::EndsMatch);
    }
    match (a.side, b.side) {
        (Side::Front, Side::Back) | (Side::Back, Side::Front) => {
            // Both at a fold: a front's and a back's waist or neckline,
            // each sewn to something else.
            let at_fold = |c: &Candidate| c.ends.0 == End::Fold || c.ends.1 == End::Fold;
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
        score -= 1.0;
    }
    (score, reasons)
}

/// A pair being considered: its score, the two candidates, which way
/// round, and why.
type Pair = (f64, usize, usize, bool, Vec<Reason>);

/// An edge by piece and edge index.
type Place = (usize, usize);

/// Two edges sewn together.
type Sewn = (Place, Place);

/// The best pairs, each candidate in one at most, leaving out `banned`.
fn greedy(pairs: &[Pair], banned: &[(usize, usize)], count: usize) -> Vec<Pair> {
    let mut used = vec![false; count];
    let mut out = Vec::new();
    for pair in pairs {
        let (_, i, j, ..) = *pair;
        if used[i] || used[j] || banned.contains(&(i, j)) {
            continue;
        }
        used[i] = true;
        used[j] = true;
        out.push(pair.clone());
    }
    out
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
    // The seams already sewn, by piece and edge index.
    let place = |side: &SeamSide| {
        let pi = pieces.iter().position(|p| p.borrow().id == side.piece)?;
        let ei = pieces[pi].borrow().outline.edge_index(&side.edge)?;
        Some((pi, ei))
    };
    let already: Vec<Sewn> = existing
        .iter()
        .filter_map(|s| Some((place(&s.a)?, place(&s.b)?)))
        .collect();
    let mut pairs: Vec<(f64, usize, usize, bool, Vec<Reason>)> = Vec::new();
    for i in 0..cands.len() {
        for j in i + 1..cands.len() {
            let (a, b) = (&cands[i], &cands[j]);
            if (a.length - b.length).abs() / a.length.max(b.length) > MOST_APART {
                continue;
            }
            // Only a tube (a sleeve) is sewn to itself; a piece's left and
            // right sides match but are not sewn together.
            if a.piece == b.piece && !a.tube {
                continue;
            }
            let ((s1, r1), (s2, r2)) = (score(a, b, false), score(a, b, true));
            let (s, reversed, reasons) = if s2 > s1 {
                (s2, true, r2)
            } else {
                (s1, false, r1)
            };
            if s >= THRESHOLD {
                pairs.push((s, i, j, reversed, reasons));
            }
        }
    }
    pairs.sort_by(|x, y| y.0.total_cmp(&x.0));
    // The best pairs, then again without any that run alongside the others.
    let mut banned = Vec::new();
    let mut accepted = greedy(&pairs, &banned, cands.len());
    for _ in 0..4 {
        let along = alongside(&accepted, &cands, &already);
        if along.is_empty() {
            break;
        }
        banned.extend(along);
        accepted = greedy(&pairs, &banned, cands.len());
    }
    let mut used = vec![false; cands.len()];
    for (_, i, j, ..) in &accepted {
        used[*i] = true;
        used[*j] = true;
    }
    let index_of = |piece: usize, edge: usize| {
        cands
            .iter()
            .position(|c| c.piece == piece && c.edge == edge)
    };
    let mut out = Vec::new();
    for (s, i, j, reversed, reasons) in accepted {
        let (a, b) = (&cands[i], &cands[j]);
        let name = if a.name == b.name {
            a.name.clone()
        } else {
            String::new()
        };
        let suggestion = |a: SeamSide, b: SeamSide, reversed: bool| Suggestion {
            seam: Seam {
                id: String::new(),
                name: sentence(&name),
                a,
                b,
                reversed,
                ease: Ease::Even,
                confirmed: false,
            },
            confidence: (s / 3.0).clamp(0.0, 1.0),
            reasons: reasons.clone(),
        };
        out.push(suggestion(
            SeamSide::whole(&a.piece_id, &a.edge_id),
            SeamSide::whole(&b.piece_id, &b.edge_id),
            reversed,
        ));
        // A piece cut as a pair sewn to one drawn whole: its other half is
        // sewn to the mirror image of the edge (a yoke to both shoulders).
        for (pair, whole, pair_is_a) in [(a, b, true), (b, a, false)] {
            let Some(k) = (pair.pair && whole.counterpart.is_some())
                .then(|| whole.counterpart.and_then(|g| index_of(whole.piece, g)))
                .flatten()
            else {
                continue;
            };
            if used[k] {
                continue;
            }
            used[k] = true;
            let half = SeamSide {
                mirror: true,
                ..SeamSide::whole(&pair.piece_id, &pair.edge_id)
            };
            let other = SeamSide::whole(&cands[k].piece_id, &cands[k].edge_id);
            out.push(if pair_is_a {
                suggestion(half, other, !reversed)
            } else {
                suggestion(other, half, !reversed)
            });
        }
    }
    // A centre edge of a piece cut as a pair, left over: sewn to its other
    // half (a centre back seam).
    for (k, c) in cands.iter().enumerate() {
        let named = ["centre", "center", "cb", "cf"]
            .iter()
            .any(|w| c.name.split_whitespace().any(|x| x == *w) || c.name.starts_with(w));
        if used[k] || !c.pair || !(named || c.with_grain && c.side != Side::Neither) {
            continue;
        }
        // Only the piece's outermost straight edge across the grain: the
        // centre, not a side.
        let others_outside = cands
            .iter()
            .any(|o| o.piece == c.piece && o.edge != c.edge && o.with_grain && o.length > c.length);
        if others_outside && !named {
            continue;
        }
        used[k] = true;
        out.push(Suggestion {
            seam: Seam {
                id: String::new(),
                name: sentence(&c.name),
                a: SeamSide::whole(&c.piece_id, &c.edge_id),
                b: SeamSide {
                    mirror: true,
                    ..SeamSide::whole(&c.piece_id, &c.edge_id)
                },
                reversed: false,
                ease: Ease::Even,
                confirmed: false,
            },
            confidence: if named { 0.6 } else { 0.4 },
            reasons: vec![Reason::CentreOfPair],
        });
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
