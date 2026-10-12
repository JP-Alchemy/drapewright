//! A garment put together as far as its seams go: each piece's copies (a
//! pair as its two mirror images, a piece cut on the fold as one, a half
//! either side of the fold), which of their corners meet, which way out
//! each copy is beside those it is sewn to, and the edges left free round
//! the holes the seams leave (an armhole, a neck). Suggestions are tried
//! on it first: one that would pinch a piece (bring two of its corners
//! together through other pieces) or sew a copy both right side and wrong
//! side out is not made.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

use drapewright_pattern_core::{EdgeKind, Piece};

use crate::model::{Seam, SeamSide};

/// Ends of edges this near meet, mm.
const END: f64 = 0.5;

/// Corners further apart than this round the garment are as far as any,
/// mm: a garment's whole edge is rarely longer.
const FARTHEST: f64 = 6000.0;

/// A meeting place reached at a distance, nearest first in a heap.
#[derive(PartialEq)]
struct Reached(f64, usize);

impl Eq for Reached {}

impl Ord for Reached {
    fn cmp(&self, other: &Self) -> Ordering {
        // The nearest is the greatest, at the top of the heap.
        other.0.total_cmp(&self.0).then(self.1.cmp(&other.1))
    }
}

impl PartialOrd for Reached {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// An edge of one of a piece's copies: the piece and edge (by index), and
/// whether it is on the mirror copy of a pair (or, unfolded, the far side
/// of the fold).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(crate) struct CopyEdge {
    pub piece: usize,
    pub edge: usize,
    pub mirror: bool,
}

/// How a piece is cut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cut {
    Once,
    Pair,
    /// On the fold: one copy, the fold edge (by index) inside it.
    Unfolded(usize),
}

/// Sets that can be joined (union-find), each member knowing whether it is
/// flipped from its set's root: for which way out each copy is.
#[derive(Clone, Debug)]
struct Sets {
    parent: Vec<usize>,
    flipped: Vec<bool>,
}

impl Sets {
    fn new(n: usize) -> Sets {
        Sets {
            parent: (0..n).collect(),
            flipped: vec![false; n],
        }
    }

    /// The root of `x`'s set, and whether `x` is flipped from it.
    fn find(&mut self, x: usize) -> (usize, bool) {
        let p = self.parent[x];
        if p == x {
            return (x, false);
        }
        let (root, f) = self.find(p);
        self.parent[x] = root;
        self.flipped[x] ^= f;
        (root, self.flipped[x])
    }

    /// Joins `x` and `y`, `y` flipped from `x` or not. False where that
    /// contradicts what is known.
    fn join(&mut self, x: usize, y: usize, flip: bool) -> bool {
        let ((rx, fx), (ry, fy)) = (self.find(x), self.find(y));
        if rx == ry {
            return (fx ^ fy) == flip;
        }
        self.parent[ry] = rx;
        self.flipped[ry] = fx ^ fy ^ flip;
        true
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Assembly {
    cuts: Vec<Cut>,
    /// Each piece's edges' lengths as sewn (darts closed), mm.
    lengths: Vec<Vec<f64>>,
    /// Each piece's first corner, and first copy.
    corner_base: Vec<usize>,
    copy_base: Vec<usize>,
    /// Which corners meet.
    corners: Sets,
    /// Which corners of the garment's shell meet: seams to facings and
    /// linings left out (layered inside, they bring nothing of it
    /// together).
    shell: Sets,
    /// Each piece is a facing or a lining.
    inside: Vec<bool>,
    /// Which corners a copy's own seams bring together (a sleeve's
    /// underarm brings its head's ends together).
    own: Sets,
    /// Which way out each copy is, beside those it is sewn to.
    faces: Sets,
    /// Each copy edge's sewn stretches, mm along it.
    sewn: HashMap<CopyEdge, Vec<(f64, f64)>>,
    /// Copies sewn to themselves: tubes (a sleeve, by its underarm).
    tubes: Vec<bool>,
    /// Edges sewn from end to end: their ends may meet (a cuff sewn round
    /// a sleeve's hem wraps it, its ends together).
    wrapped: Vec<CopyEdge>,
    /// Each meeting place's neighbours along the copies' edges, and how
    /// far: made when first needed after a change.
    graph: Option<HashMap<usize, Vec<(usize, f64)>>>,
}

/// One seam on two copies: each side's copy edge and the stretch of it
/// sewn, and whether the first's start meets the second's end.
pub(crate) type Part = (CopyEdge, (f64, f64), CopyEdge, (f64, f64), bool);

impl Assembly {
    /// The pieces, each edge `lengths[piece][edge]` mm long as sewn.
    pub fn new(pieces: &[&Piece], lengths: Vec<Vec<f64>>) -> Assembly {
        let mut cuts = Vec::new();
        let (mut corner_base, mut copy_base) = (Vec::new(), Vec::new());
        let (mut corners, mut copies) = (0, 0);
        for piece in pieces {
            let cut = piece
                .cut
                .iter()
                .find(|c| c.material == "main")
                .or(piece.cut.first())
                .cloned()
                .unwrap_or_default();
            let fold = piece
                .outline
                .edges
                .iter()
                .position(|e| e.kind == EdgeKind::Fold);
            let cut = match fold {
                Some(f) if cut.on_fold.unwrap_or(true) => Cut::Unfolded(f),
                _ if cut.quantity >= 2 && cut.mirrored.unwrap_or(true) => Cut::Pair,
                _ => Cut::Once,
            };
            corner_base.push(corners);
            copy_base.push(copies);
            corners += 2 * piece.outline.edges.len();
            copies += if cut == Cut::Pair { 2 } else { 1 };
            cuts.push(cut);
        }
        Assembly {
            cuts,
            lengths,
            corner_base,
            copy_base,
            corners: Sets::new(corners),
            shell: Sets::new(corners),
            inside: pieces.iter().map(|p| crate::suggest::inside(p)).collect(),
            own: Sets::new(corners),
            faces: Sets::new(copies),
            sewn: HashMap::new(),
            tubes: vec![false; copies],
            wrapped: Vec::new(),
            graph: None,
        }
    }

    fn edges(&self, piece: usize) -> usize {
        self.lengths[piece].len()
    }

    /// Whether the copy edge is there to sew: a whole piece has no mirror
    /// copy, and a piece unfolded no fold edge.
    pub fn has(&self, e: CopyEdge) -> bool {
        e.piece < self.cuts.len()
            && e.edge < self.edges(e.piece)
            && match self.cuts[e.piece] {
                Cut::Once => !e.mirror,
                Cut::Pair => true,
                Cut::Unfolded(fold) => e.edge != fold,
            }
    }

    pub fn length(&self, e: CopyEdge) -> f64 {
        self.lengths[e.piece][e.edge]
    }

    /// Corner `i` (where edge `i` starts) of a piece's copy or half.
    fn corner(&self, piece: usize, mirror: bool, i: usize) -> usize {
        let n = self.edges(piece);
        let i = i % n;
        let half_a = match self.cuts[piece] {
            Cut::Once => true,
            Cut::Pair => !mirror,
            // The fold's ends are on both halves.
            Cut::Unfolded(fold) => !mirror || i == fold || i == (fold + 1) % n,
        };
        self.corner_base[piece] + if half_a { i } else { n + i }
    }

    pub fn start(&self, e: CopyEdge) -> usize {
        self.corner(e.piece, e.mirror, e.edge)
    }

    pub fn end(&self, e: CopyEdge) -> usize {
        self.corner(e.piece, e.mirror, e.edge + 1)
    }

    /// The copy an edge is on.
    pub fn copy(&self, e: CopyEdge) -> usize {
        self.copy_base[e.piece] + usize::from(e.mirror && self.cuts[e.piece] == Cut::Pair)
    }

    /// The copy a corner is on.
    fn corner_copy(&self, c: usize) -> usize {
        let piece = self.corner_base.partition_point(|&b| b <= c) - 1;
        let mirror = c - self.corner_base[piece] >= self.edges(piece);
        self.copy(CopyEdge {
            piece,
            edge: 0,
            mirror,
        })
    }

    /// Whether two corners meet already.
    pub fn meet(&mut self, a: usize, b: usize) -> bool {
        self.corners.find(a).0 == self.corners.find(b).0
    }

    /// Whether two copies are joined by seams, directly or through others.
    pub fn joined(&mut self, a: usize, b: usize) -> bool {
        self.faces.find(a).0 == self.faces.find(b).0
    }

    /// How many copies are joined to a copy by seams, itself among them.
    pub fn joined_to(&mut self, a: usize) -> usize {
        let root = self.faces.find(a).0;
        (0..self.tubes.len())
            .filter(|&c| self.faces.find(c).0 == root)
            .count()
    }

    /// A seam as its parts on copies: on both copies of pairs, or on those
    /// it names where it names a mirror copy (as it is draped). `place`
    /// finds a side's piece and edge (by index).
    pub fn parts(
        &self,
        seam: &Seam,
        place: impl Fn(&SeamSide) -> Option<(usize, usize)>,
    ) -> Vec<Part> {
        let (Some((pa, ea)), Some((pb, eb))) = (place(&seam.a), place(&seam.b)) else {
            return Vec::new();
        };
        let explicit = seam.a.mirror || seam.b.mirror;
        let halves: &[(bool, bool)] = if explicit {
            &[(seam.a.mirror, seam.b.mirror)]
        } else {
            &[(false, false), (true, true)]
        };
        halves
            .iter()
            .filter_map(|&(ma, mb)| {
                let a = CopyEdge {
                    piece: pa,
                    edge: ea,
                    mirror: ma,
                };
                let b = CopyEdge {
                    piece: pb,
                    edge: eb,
                    mirror: mb,
                };
                if !self.has(a) || !self.has(b) {
                    return None;
                }
                let stretch = |s: &SeamSide, e: CopyEdge| {
                    let len = self.length(e);
                    (
                        s.from.unwrap_or(0.0).clamp(0.0, len),
                        s.to.unwrap_or(len).clamp(0.0, len),
                    )
                };
                Some((
                    a,
                    stretch(&seam.a, a),
                    b,
                    stretch(&seam.b, b),
                    seam.reversed,
                ))
            })
            .collect()
    }

    /// A copy to try seams on, without what is worked out from it.
    pub fn trial(&mut self) -> Assembly {
        let graph = self.graph.take();
        let trial = self.clone();
        self.graph = graph;
        trial
    }

    /// Sews the parts, if that keeps every copy whole (none pinched) and
    /// each one way out. False, and nothing sewn, where it would not.
    pub fn sew(&mut self, parts: &[Part]) -> bool {
        let mut next = self.trial();
        let mut touched = Vec::new();
        for &(a, sa, b, sb, reversed) in parts {
            // Which of each side's ends are sewn: a part of an edge may
            // start or stop short of them.
            let ends = |e: CopyEdge, s: (f64, f64)| {
                (s.0.min(s.1) <= END, s.0.max(s.1) >= next.length(e) - END)
            };
            let ((a_start, a_end), (b_start, b_end)) = (ends(a, sa), ends(b, sb));
            let meetings = if reversed {
                [
                    (a_start && b_end).then(|| (next.start(a), next.end(b))),
                    (a_end && b_start).then(|| (next.end(a), next.start(b))),
                ]
            } else {
                [
                    (a_start && b_start).then(|| (next.start(a), next.start(b))),
                    (a_end && b_end).then(|| (next.end(a), next.end(b))),
                ]
            };
            let (ca, cb) = (next.copy(a), next.copy(b));
            if ca == cb {
                next.tubes[ca] = true;
            }
            let layered = next.inside[a.piece] || next.inside[b.piece];
            for (x, y) in meetings.into_iter().flatten() {
                next.corners.join(x, y, false);
                if !layered {
                    next.shell.join(x, y, false);
                    touched.push(x);
                }
                if ca == cb {
                    next.own.join(x, y, false);
                }
            }
            for (e, (start, end)) in [(a, (a_start, a_end)), (b, (b_start, b_end))] {
                if start && end {
                    next.wrapped.push(e);
                }
            }
            // Laid out flat, right sides up, two pieces drawn the same way
            // round are sewn end to start; one of them mirrored, start to
            // start.
            let same_way = reversed == (a.mirror == b.mirror);
            if !next.faces.join(ca, cb, !same_way) {
                return false;
            }
            next.sewn.entry(a).or_default().push(sa);
            next.sewn.entry(b).or_default().push(sb);
        }
        if !next.whole(&touched) {
            return false;
        }
        next.graph = None;
        *self = next;
        true
    }

    /// No copy of the shell pinched where the corners in `touched` now
    /// meet others: those of one copy meet only through its own seams, or
    /// are the ends of an edge of it sewn from end to end round a loop.
    fn whole(&mut self, touched: &[usize]) -> bool {
        let mut roots: Vec<usize> = touched.iter().map(|&c| self.shell.find(c).0).collect();
        roots.sort_unstable();
        roots.dedup();
        if roots.is_empty() {
            return true;
        }
        let n = self.corners.parent.len();
        let mut meeting: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
        for c in 0..n {
            let root = self.shell.find(c).0;
            if roots.binary_search(&root).is_err() {
                continue;
            }
            let piece = self.corner_base.partition_point(|&b| b <= c) - 1;
            if self.inside[piece] {
                continue;
            }
            meeting
                .entry((root, self.corner_copy(c)))
                .or_default()
                .push(c);
        }
        for corners in meeting.values() {
            let mut own: Vec<usize> = corners.iter().map(|&c| self.own.find(c).0).collect();
            own.sort_unstable();
            own.dedup();
            if own.len() <= 1 {
                continue;
            }
            let round = corners.len() == 2
                && self.wrapped.iter().any(|&e| {
                    let (s, t) = (self.start(e), self.end(e));
                    (s == corners[0] && t == corners[1]) || (s == corners[1] && t == corners[0])
                });
            if !round {
                return false;
            }
        }
        true
    }

    /// Whether any of a copy edge is sewn.
    pub fn is_sewn(&self, e: CopyEdge) -> bool {
        self.sewn.get(&e).is_some_and(|s| !s.is_empty())
    }

    /// The free edges' loops: each a run of copy edges left unsewn, end to
    /// start where their corners meet, round a hole in the garment (each
    /// edge run forwards or not). Runs that do not close are left out.
    pub fn loops(&mut self) -> Vec<Vec<(CopyEdge, bool)>> {
        self.runs()
            .into_iter()
            .filter_map(|(run, closed)| closed.then_some(run))
            .collect()
    }

    /// The free edges' runs, each as far as it goes either way: the loops
    /// of `loops`, and runs that stop where the boundary is not plain (an
    /// edge sewn along part of it): each run's copy edges in order (each
    /// run forwards or not), and whether it closes.
    pub fn runs(&mut self) -> Vec<(Vec<(CopyEdge, bool)>, bool)> {
        let mut free = Vec::new();
        for piece in 0..self.cuts.len() {
            for edge in 0..self.edges(piece) {
                for mirror in [false, true] {
                    let e = CopyEdge {
                        piece,
                        edge,
                        mirror,
                    };
                    if self.has(e) && !self.is_sewn(e) {
                        free.push(e);
                    }
                }
            }
        }
        // Each free edge's ends, as meeting places.
        let mut tips: Vec<(usize, usize)> = Vec::new();
        for &e in &free {
            let (s, t) = (self.start(e), self.end(e));
            tips.push((self.corners.find(s).0, self.corners.find(t).0));
        }
        let mut at: HashMap<usize, Vec<(usize, bool)>> = HashMap::new();
        for (k, &(s, t)) in tips.iter().enumerate() {
            at.entry(s).or_default().push((k, true));
            at.entry(t).or_default().push((k, false));
        }
        // On from a meeting place: the edges taken (each from its start or
        // not), and where the walk got to.
        let walk = |here: usize, begin: usize, used: &mut Vec<bool>| {
            let mut run: Vec<(usize, bool)> = Vec::new();
            let mut here = here;
            loop {
                let ends = at.get(&here).cloned().unwrap_or_default();
                // Only where the boundary is plain: two free ends meet.
                if ends.len() != 2 {
                    break;
                }
                let Some(&(k, from_start)) = ends.iter().find(|(k, _)| !used[*k]) else {
                    break;
                };
                used[k] = true;
                run.push((k, from_start));
                here = if from_start { tips[k].1 } else { tips[k].0 };
                if here == begin {
                    break;
                }
            }
            (run, here)
        };
        let mut used = vec![false; free.len()];
        let mut out = Vec::new();
        for first in 0..free.len() {
            if used[first] {
                continue;
            }
            used[first] = true;
            let (begin, end) = tips[first];
            let (ahead, reached) = walk(end, begin, &mut used);
            let closed = reached == begin;
            let behind = if closed {
                Vec::new()
            } else {
                walk(begin, end, &mut used).0
            };
            // Behind it, run back the other way; then it, then ahead.
            let mut run: Vec<(CopyEdge, bool)> = Vec::new();
            for &(k, from_start) in behind.iter().rev() {
                run.push((free[k], !from_start));
            }
            run.push((free[first], true));
            for &(k, from_start) in &ahead {
                run.push((free[k], from_start));
            }
            out.push((run, closed));
        }
        out
    }

    /// Where a loop's run of copy edges starts (a corner).
    pub fn first(&self, e: CopyEdge, forwards: bool) -> usize {
        if forwards { self.start(e) } else { self.end(e) }
    }

    /// How far apart two corners are round the garment as sewn so far,
    /// along its pieces' edges, mm; none where they are not joined, or are
    /// further apart than `FARTHEST`.
    pub fn distance(&mut self, a: usize, b: usize) -> Option<f64> {
        let (from, to) = (self.corners.find(a).0, self.corners.find(b).0);
        if from == to {
            return Some(0.0);
        }
        let (ca, cb) = (self.corner_copy(a), self.corner_copy(b));
        if self.faces.find(ca).0 != self.faces.find(cb).0 {
            return None;
        }
        if self.graph.is_none() {
            self.graph = Some(self.meeting_places());
        }
        let graph = self.graph.as_ref().expect("made");
        let mut best: HashMap<usize, f64> = HashMap::from([(from, 0.0)]);
        let mut heap = BinaryHeap::from([Reached(0.0, from)]);
        while let Some(Reached(d, here)) = heap.pop() {
            if here == to {
                return Some(d);
            }
            if d > FARTHEST {
                return None;
            }
            if best.get(&here).is_some_and(|&b| d > b) {
                continue;
            }
            for &(next, length) in graph.get(&here).into_iter().flatten() {
                let reach = d + length;
                if best.get(&next).is_none_or(|&b| reach < b) {
                    best.insert(next, reach);
                    heap.push(Reached(reach, next));
                }
            }
        }
        None
    }

    /// Each meeting place's neighbours along the copies' edges, and how far.
    fn meeting_places(&mut self) -> HashMap<usize, Vec<(usize, f64)>> {
        let mut out: HashMap<usize, Vec<(usize, f64)>> = HashMap::new();
        for piece in 0..self.cuts.len() {
            for edge in 0..self.edges(piece) {
                for mirror in [false, true] {
                    let e = CopyEdge {
                        piece,
                        edge,
                        mirror,
                    };
                    if !self.has(e) {
                        continue;
                    }
                    let (s, t) = (self.start(e), self.end(e));
                    let (s, t) = (self.corners.find(s).0, self.corners.find(t).0);
                    let length = self.length(e);
                    out.entry(s).or_default().push((t, length));
                    out.entry(t).or_default().push((s, length));
                }
            }
        }
        out
    }

    /// The corners a part brings together: each end sewn whole, to where
    /// it meets the other side.
    pub fn joins(&self, part: &Part) -> Vec<(usize, usize)> {
        let &(a, sa, b, sb, reversed) = part;
        let ends = |e: CopyEdge, s: (f64, f64)| {
            (s.0.min(s.1) <= END, s.0.max(s.1) >= self.length(e) - END)
        };
        let ((a_start, a_end), (b_start, b_end)) = (ends(a, sa), ends(b, sb));
        let joins = if reversed {
            [
                (a_start && b_end).then(|| (self.start(a), self.end(b))),
                (a_end && b_start).then(|| (self.end(a), self.start(b))),
            ]
        } else {
            [
                (a_start && b_start).then(|| (self.start(a), self.start(b))),
                (a_end && b_end).then(|| (self.end(a), self.end(b))),
            ]
        };
        joins.into_iter().flatten().collect()
    }

    /// Whether a copy is sewn to itself: a tube.
    pub fn tube(&self, copy: usize) -> bool {
        self.tubes[copy]
    }

    /// The longest seam with an end where corner `c` is: a side seam, where
    /// a sleeve's underarm goes.
    pub fn longest_at(&mut self, c: usize) -> f64 {
        let here = self.corners.find(c).0;
        let sewn: Vec<CopyEdge> = self.sewn.keys().copied().collect();
        let mut longest = 0.0f64;
        for e in sewn {
            let (s, t) = (self.start(e), self.end(e));
            if self.corners.find(s).0 == here || self.corners.find(t).0 == here {
                longest = longest.max(self.length(e));
            }
        }
        longest
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drapewright_pattern_core::{CutInstruction, Edge, Outline, Pt, Segment};

    /// A rectangle `w` by `h` mm, cut `quantity` times.
    fn panel(id: &str, w: f64, h: f64, quantity: u32) -> Piece {
        let mut p = Piece::new(
            id,
            id,
            Outline {
                start: Pt::new(0.0, 0.0),
                edges: vec![
                    Edge::new("top", vec![Segment::line([w, 0.0])]),
                    Edge::new("right", vec![Segment::line([w, h])]),
                    Edge::new("bottom", vec![Segment::line([0.0, h])]),
                    Edge::new("left", vec![Segment::line([0.0, 0.0])]),
                ],
            },
        );
        p.cut = vec![CutInstruction {
            quantity,
            ..Default::default()
        }];
        p
    }

    fn assembly(pieces: &[Piece]) -> Assembly {
        let lengths = pieces
            .iter()
            .map(|p| {
                (0..p.outline.edges.len())
                    .map(|i| p.outline.edge_geom(i).length())
                    .collect()
            })
            .collect();
        Assembly::new(&pieces.iter().collect::<Vec<_>>(), lengths)
    }

    fn whole(piece: usize, edge: usize, mirror: bool, a: &Assembly) -> (CopyEdge, (f64, f64)) {
        let e = CopyEdge {
            piece,
            edge,
            mirror,
        };
        (e, (0.0, a.length(e)))
    }

    #[test]
    fn a_tube_is_its_own_and_has_two_ends() {
        let mut a = assembly(&[panel("sleeve", 300.0, 500.0, 1)]);
        // Its left side to its right, top to top.
        let (l, sl) = whole(0, 3, false, &a);
        let (r, sr) = whole(0, 1, false, &a);
        assert!(a.sew(&[(r, sr, l, sl, true)]));
        let loops = a.loops();
        assert_eq!(loops.len(), 2, "{loops:?}");
        assert!(loops.iter().all(|l| l.len() == 1));
    }

    #[test]
    fn a_piece_is_not_pinched_through_others() {
        // Two rectangles sewn at both sides make a ring; sewing the first's
        // top to the second's as well would bring the second's top
        // corners together through the first: pinched.
        let mut a = assembly(&[
            panel("front", 300.0, 500.0, 1),
            panel("back", 300.0, 500.0, 1),
        ]);
        let (fr, sfr) = whole(0, 1, false, &a);
        let (bl, sbl) = whole(1, 3, false, &a);
        assert!(a.sew(&[(fr, sfr, bl, sbl, true)]));
        let (fl, sfl) = whole(0, 3, false, &a);
        let (br, sbr) = whole(1, 1, false, &a);
        assert!(a.sew(&[(fl, sfl, br, sbr, true)]));
        // The tops' loop and the bottoms'.
        assert_eq!(a.loops().len(), 2);
        let (ft, sft) = whole(0, 0, false, &a);
        let (bb, sbb) = whole(1, 2, false, &a);
        // The front's top to the back's bottom joins the back's top-left
        // corner to its bottom-left one through the front.
        assert!(!a.sew(&[(ft, sft, bb, sbb, true)]));
    }

    #[test]
    fn a_copy_is_one_way_out() {
        // Sewn end to start, two pieces drawn alike lie right side up side
        // by side; their tops sewn start to start as well, one would have
        // to be wrong side out too.
        let mut a = assembly(&[panel("a", 300.0, 500.0, 1), panel("b", 300.0, 500.0, 1)]);
        let (ar, sar) = whole(0, 1, false, &a);
        let (bl, sbl) = whole(1, 3, false, &a);
        assert!(a.sew(&[(ar, sar, bl, sbl, true)]));
        let (at, sat) = whole(0, 0, false, &a);
        let (bt, sbt) = whole(1, 0, false, &a);
        assert!(!a.sew(&[(at, sat, bt, sbt, false)]));
        // Refused, nothing of it was sewn.
        assert!(!a.is_sewn(at));
    }

    #[test]
    fn sets_know_which_way_each_member_is() {
        let mut s = Sets::new(4);
        assert!(s.join(0, 1, true));
        assert!(s.join(1, 2, true));
        // 0 and 2 are flipped twice from each other: the same way.
        assert!(s.join(0, 2, false));
        assert!(!s.join(2, 0, true));
        assert!(s.join(3, 0, false));
        assert!(s.find(3).1 ^ s.find(1).1);
    }
}
