//! Outlines drawn as lines that meet end to middle. Pattern CAD printed
//! through a plotter driver draws a seam line in pieces: at a corner one
//! seam line runs on past it into the allowance and the next ends on it; a
//! seam line forks round an extension of the piece; notches join the
//! cutting line to the stitching line. Seen as a network of lines, the
//! outlines are its outer edge once loose ends are trimmed, peeled off one
//! after another (the cutting line, then the stitching line inside it).

use std::collections::HashMap;

use drapewright_pattern_core::geom::{flatten_segs, polygon_area};
use kurbo::{
    ParamCurve, ParamCurveArclen, ParamCurveExtrema, ParamCurveNearest, PathSeg, Point, Vec2,
};

/// What became of a stretch of line.
#[derive(Clone, Copy, PartialEq)]
enum Fate {
    Open,
    Loose,
    Loop,
}

/// One stretch of line between two meeting points.
struct Edge {
    from: usize,
    to: usize,
    segs: Vec<PathSeg>,
    fate: Fate,
    /// The path it was cut from.
    path: usize,
}

/// Open `paths`, all drawn alike, as the loops they make and the lines left
/// over (closed and open). Points within `tolerance` mm meet.
pub(crate) fn find_loops(paths: Vec<Vec<PathSeg>>, tolerance: f64) -> Vec<(Vec<PathSeg>, bool)> {
    let paths: Vec<Vec<PathSeg>> = paths.into_iter().filter(|p| !p.is_empty()).collect();
    let cuts = landing_points(&paths, tolerance);
    let mut nodes = Nodes::new(tolerance);
    let mut edges: Vec<Edge> = Vec::new();
    for (index, (path, at)) in paths.iter().zip(cuts).enumerate() {
        for segs in cut(path, at) {
            let (from, to) = (
                nodes.at(segs[0].start()),
                nodes.at(segs[segs.len() - 1].end()),
            );
            // A speck going round nothing is no part of the network.
            let length: f64 = segs.iter().map(|s| s.arclen(0.01)).sum();
            let fate = if from == to && length <= 4.0 * tolerance {
                Fate::Loose
            } else {
                Fate::Open
            };
            edges.push(Edge {
                from,
                to,
                segs,
                fate,
                path: index,
            });
        }
    }

    let mut loops: Vec<Vec<PathSeg>> = Vec::new();
    loop {
        trim_loose_ends(&mut edges, nodes.points.len());
        if !edges.iter().any(|e| e.fate == Fate::Open) {
            break;
        }
        let peeled = peel(&mut edges);
        if peeled.is_empty() {
            break;
        }
        loops.extend(peeled);
    }

    // Paths no loop took come back as they were; of the others, what is
    // left of them, in runs.
    let mut out: Vec<(Vec<PathSeg>, bool)> = loops.into_iter().map(|segs| (segs, true)).collect();
    let used: Vec<bool> = {
        let mut used = vec![false; paths.len()];
        for e in edges.iter().filter(|e| e.fate == Fate::Loop) {
            used[e.path] = true;
        }
        used
    };
    let mut run: Vec<PathSeg> = Vec::new();
    let mut last = usize::MAX;
    for e in edges {
        if !used[e.path] {
            continue;
        }
        if e.fate == Fate::Loop || e.path != last {
            if !run.is_empty() {
                out.push((std::mem::take(&mut run), false));
            }
            last = e.path;
            if e.fate == Fate::Loop {
                continue;
            }
        }
        run.extend(e.segs);
    }
    if !run.is_empty() {
        out.push((run, false));
    }
    out.extend(
        paths
            .into_iter()
            .zip(used)
            .filter(|(_, used)| !used)
            .map(|(segs, _)| (segs, false)),
    );
    out
}

/// A loop that comes back through a point it passed (two sizes of a nest
/// traced as one, through a corner they share) as the loops it makes, each
/// round some room: a mere sliver (a line gone along and back) is not cut
/// off.
pub(crate) fn split_touching(segs: Vec<PathSeg>, tolerance: f64) -> Vec<Vec<PathSeg>> {
    let roomy = |segs: &[PathSeg]| {
        let pts = flatten_segs(segs, 0.5);
        let around: f64 = segs.iter().map(|s| s.arclen(0.01)).sum();
        polygon_area(&pts).abs() >= MIN_WIDTH / 2.0 * around
    };
    let cell = |p: Point| {
        (
            (p.x / tolerance).floor() as i64,
            (p.y / tolerance).floor() as i64,
        )
    };
    // Where each segment starts, by cell.
    let mut starts: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, s) in segs.iter().enumerate() {
        starts.entry(cell(s.start())).or_default().push(i);
    }
    for (k, s) in segs.iter().enumerate() {
        let end = s.end();
        let (cx, cy) = cell(end);
        let mut back: Vec<usize> = (cx - 1..=cx + 1)
            .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
            .filter_map(|c| starts.get(&c))
            .flatten()
            .copied()
            .filter(|&i| i > 0 && i <= k && segs[i].start().distance(end) <= tolerance)
            .collect();
        back.sort_unstable();
        for i in back {
            let inner: Vec<PathSeg> = segs[i..=k].to_vec();
            let outer: Vec<PathSeg> = segs[..i].iter().chain(&segs[k + 1..]).copied().collect();
            if outer.is_empty() || !roomy(&inner) || !roomy(&outer) {
                continue;
            }
            let mut out = split_touching(inner, tolerance);
            out.extend(split_touching(outer, tolerance));
            return out;
        }
    }
    vec![segs]
}

/// A piece is at least this wide, mm: narrower loops are slivers.
const MIN_WIDTH: f64 = 1.0;

/// Where an end of one path lands on the middle of a path: (segment,
/// parameter) on each path.
fn landing_points(paths: &[Vec<PathSeg>], tolerance: f64) -> Vec<Vec<(usize, f64)>> {
    const CELL: f64 = 5.0;
    let key = |x: f64, y: f64| ((x / CELL).floor() as i64, (y / CELL).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<(usize, usize)>> = HashMap::new();
    for (i, path) in paths.iter().enumerate() {
        for (k, seg) in path.iter().enumerate() {
            let b = seg.bounding_box().inflate(tolerance, tolerance);
            let (x0, y0) = key(b.x0, b.y0);
            let (x1, y1) = key(b.x1, b.y1);
            for x in x0..=x1 {
                for y in y0..=y1 {
                    grid.entry((x, y)).or_default().push((i, k));
                }
            }
        }
    }
    // How far along each path each segment starts, mm.
    let along: Vec<Vec<f64>> = paths
        .iter()
        .map(|p| {
            let mut total = 0.0;
            let mut starts = Vec::with_capacity(p.len() + 1);
            for seg in p {
                starts.push(total);
                total += seg.arclen(0.01);
            }
            starts.push(total);
            starts
        })
        .collect();
    let mut cuts = vec![Vec::new(); paths.len()];
    for (j, path) in paths.iter().enumerate() {
        let length = along[j][path.len()];
        for (at_start, end) in [(true, path[0].start()), (false, path[path.len() - 1].end())] {
            for &(i, k) in grid.get(&key(end.x, end.y)).map_or(&[][..], Vec::as_slice) {
                let other = &paths[i];
                // Meeting another path's end is for the nodes to see to; so is
                // a path coming back to its own other end.
                let (first, last) = (other[0].start(), other[other.len() - 1].end());
                let meets_end = if i == j {
                    (if at_start { last } else { first }).distance(end) <= tolerance
                } else {
                    first.distance(end) <= tolerance || last.distance(end) <= tolerance
                };
                if meets_end {
                    continue;
                }
                let near = other[k].nearest(end, 1e-6);
                if near.distance_sq.sqrt() > tolerance {
                    continue;
                }
                // A path's end lies on its own last stretch.
                if i == j {
                    let from_start = along[i][k] + other[k].subsegment(0.0..near.t).arclen(0.01);
                    let from_end = if at_start {
                        from_start
                    } else {
                        length - from_start
                    };
                    if from_end < 1.0 + 10.0 * tolerance {
                        continue;
                    }
                }
                cuts[i].push((k, near.t));
            }
        }
    }
    cuts
}

/// `path` cut at the given places: (segment, parameter).
fn cut(path: &[PathSeg], at: Vec<(usize, f64)>) -> Vec<Vec<PathSeg>> {
    const EPS: f64 = 1e-6;
    // A cut at the very end of a segment is one at the start of the next.
    let mut cuts: Vec<(usize, f64)> = at
        .into_iter()
        .map(|(k, t)| {
            if t >= 1.0 - EPS {
                (k + 1, 0.0)
            } else {
                (k, t.max(0.0))
            }
        })
        .collect();
    cuts.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    cuts.dedup_by(|a, b| a.0 == b.0 && (a.1 - b.1).abs() < EPS);
    let mut pieces = Vec::new();
    let mut current: Vec<PathSeg> = Vec::new();
    let mut c = 0;
    for (k, seg) in path.iter().enumerate() {
        let mut from = 0.0;
        while c < cuts.len() && cuts[c].0 == k {
            let t = cuts[c].1;
            c += 1;
            if t > EPS {
                current.push(seg.subsegment(from..t));
                from = t;
            }
            if !current.is_empty() {
                pieces.push(std::mem::take(&mut current));
            }
        }
        current.push(if from > EPS {
            seg.subsegment(from..1.0)
        } else {
            *seg
        });
    }
    if !current.is_empty() {
        pieces.push(current);
    }
    pieces
}

/// Meeting points: points within the tolerance are one.
struct Nodes {
    points: Vec<Point>,
    grid: HashMap<(i64, i64), Vec<usize>>,
    tolerance: f64,
}

impl Nodes {
    fn new(tolerance: f64) -> Self {
        Self {
            points: Vec::new(),
            grid: HashMap::new(),
            tolerance: tolerance.max(1e-9),
        }
    }

    fn key(&self, p: Point) -> (i64, i64) {
        let cell = 2.0 * self.tolerance;
        ((p.x / cell).floor() as i64, (p.y / cell).floor() as i64)
    }

    fn at(&mut self, p: Point) -> usize {
        let (kx, ky) = self.key(p);
        for dx in -1..=1 {
            for dy in -1..=1 {
                for &n in self
                    .grid
                    .get(&(kx + dx, ky + dy))
                    .map_or(&[][..], Vec::as_slice)
                {
                    if self.points[n].distance(p) <= self.tolerance {
                        return n;
                    }
                }
            }
        }
        self.points.push(p);
        let n = self.points.len() - 1;
        self.grid.entry((kx, ky)).or_default().push(n);
        n
    }
}

/// Lines that end where nothing else meets them (and what then dangles in
/// turn) are loose: stubs past a corner, notches, lines on a piece.
fn trim_loose_ends(edges: &mut [Edge], nodes: usize) {
    let mut at: Vec<Vec<usize>> = vec![Vec::new(); nodes];
    for (e, edge) in edges
        .iter()
        .enumerate()
        .filter(|(_, e)| e.fate == Fate::Open)
    {
        at[edge.from].push(e);
        at[edge.to].push(e);
    }
    let degree = |at: &Vec<Vec<usize>>, edges: &[Edge], n: usize| {
        at[n]
            .iter()
            .filter(|&&e| edges[e].fate == Fate::Open)
            .count()
    };
    let mut stack: Vec<usize> = (0..nodes).filter(|&n| degree(&at, edges, n) == 1).collect();
    while let Some(n) = stack.pop() {
        let Some(&e) = at[n].iter().find(|&&e| edges[e].fate == Fate::Open) else {
            continue;
        };
        if degree(&at, edges, n) != 1 {
            continue;
        }
        edges[e].fate = Fate::Loose;
        let other = if edges[e].from == n {
            edges[e].to
        } else {
            edges[e].from
        };
        if degree(&at, edges, other) == 1 {
            stack.push(other);
        }
    }
}

/// The outline of each network of lines still open: the way round its
/// outside. Those lines become loops.
fn peel(edges: &mut [Edge]) -> Vec<Vec<PathSeg>> {
    // Half-edge 2e runs from the edge's start to its end; 2e + 1 back.
    let open: Vec<usize> = (0..edges.len())
        .filter(|&e| edges[e].fate == Fate::Open)
        .collect();
    let head = |h: usize| {
        if h.is_multiple_of(2) {
            edges[h / 2].to
        } else {
            edges[h / 2].from
        }
    };
    let tail = |h: usize| {
        if h.is_multiple_of(2) {
            edges[h / 2].from
        } else {
            edges[h / 2].to
        }
    };
    let oriented = |h: usize| -> Vec<PathSeg> {
        let segs = &edges[h / 2].segs;
        if h.is_multiple_of(2) {
            segs.clone()
        } else {
            segs.iter().rev().map(PathSeg::reverse).collect()
        }
    };
    // Around each meeting point, the ways out in order of direction.
    let mut around: HashMap<usize, Vec<(f64, usize)>> = HashMap::new();
    for &e in &open {
        for h in [2 * e, 2 * e + 1] {
            let d = leaving(&oriented(h));
            around.entry(tail(h)).or_default().push((d.y.atan2(d.x), h));
        }
    }
    let mut place: HashMap<usize, usize> = HashMap::new();
    for ways in around.values_mut() {
        ways.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (i, (_, h)) in ways.iter().enumerate() {
            place.insert(*h, i);
        }
    }
    // Each face: arriving somewhere, leave by the way just before the way
    // back. Faces inside come round one way, the outside the other.
    let next = |h: usize| {
        let ways = &around[&head(h)];
        let i = place[&(h ^ 1)];
        ways[(i + ways.len() - 1) % ways.len()].1
    };
    let mut seen = std::collections::HashSet::new();
    let mut faces: Vec<(f64, Vec<usize>)> = Vec::new();
    for &e in &open {
        for start in [2 * e, 2 * e + 1] {
            if seen.contains(&start) {
                continue;
            }
            let mut face = Vec::new();
            let mut h = start;
            while seen.insert(h) {
                face.push(h);
                h = next(h);
            }
            let pts: Vec<Point> = face
                .iter()
                .flat_map(|&h| flatten_segs(&oriented(h), 0.5))
                .collect();
            faces.push((polygon_area(&pts), face));
        }
    }
    // Networks: the outside of each is its face that comes round the
    // other way, the largest.
    let mut parent: HashMap<usize, usize> = HashMap::new();
    fn root(parent: &HashMap<usize, usize>, mut n: usize) -> usize {
        while let Some(&p) = parent.get(&n) {
            n = p;
        }
        n
    }
    for &e in &open {
        let (a, b) = (root(&parent, edges[e].from), root(&parent, edges[e].to));
        if a != b {
            parent.insert(b, a);
        }
    }
    let mut outside: HashMap<usize, (f64, usize)> = HashMap::new();
    for (k, (area, face)) in faces.iter().enumerate() {
        if *area >= 0.0 {
            continue;
        }
        let net = root(&parent, tail(face[0]));
        let best = outside.entry(net).or_insert((*area, k));
        if *area < best.0 {
            *best = (*area, k);
        }
    }
    // A line crossed both ways joins two loops: each is a loop of its own.
    // In the order the faces were found, the same every time.
    let mut outsides: Vec<usize> = outside.values().map(|&(_, k)| k).collect();
    outsides.sort_unstable();
    let rings: Vec<(Vec<PathSeg>, Vec<usize>)> = outsides
        .into_iter()
        .flat_map(|k| split_at_bridges(faces[k].1.clone()))
        .map(|ring| (ring.iter().flat_map(|&h| oriented(h)).collect(), ring))
        .collect();
    let mut loops = Vec::new();
    for (segs, ring) in rings {
        if polygon_area(&flatten_segs(&segs, 0.5)).abs() > 1e-6 {
            loops.push(segs);
        }
        for h in ring {
            edges[h / 2].fate = Fate::Loop;
        }
    }
    loops
}

/// Direction a path leaves its start, over its first millimetre or so.
fn leaving(segs: &[PathSeg]) -> Vec2 {
    let pts = flatten_segs(segs, 0.05);
    let total: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
    let reach = total.min(3.0) / 3.0;
    let mut walked = 0.0;
    for w in pts.windows(2) {
        walked += w[0].distance(w[1]);
        if walked >= reach {
            let d = w[1] - pts[0];
            return d / d.hypot().max(1e-12);
        }
    }
    let d = pts[pts.len() - 1] - pts[0];
    d / d.hypot().max(1e-12)
}

/// A face's ways round split where it crosses a line both ways (a line
/// joining two loops): the loops on either side.
fn split_at_bridges(face: Vec<usize>) -> Vec<Vec<usize>> {
    for i in 0..face.len() {
        if let Some(j) = (i + 1..face.len()).find(|&j| face[j] == face[i] ^ 1) {
            let inner: Vec<usize> = face[i + 1..j].to_vec();
            let outer: Vec<usize> = face[j + 1..].iter().chain(&face[..i]).copied().collect();
            return [inner, outer]
                .into_iter()
                .filter(|r| !r.is_empty())
                .flat_map(split_at_bridges)
                .collect();
        }
    }
    vec![face]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Line;

    /// A polyline as separate one-segment paths, a stroke per side.
    fn strokes(points: &[(f64, f64)]) -> Vec<Vec<PathSeg>> {
        points
            .windows(2)
            .map(|w| vec![PathSeg::Line(Line::new(w[0], w[1]))])
            .collect()
    }

    fn joined(points: &[(f64, f64)]) -> Vec<PathSeg> {
        points
            .windows(2)
            .map(|w| PathSeg::Line(Line::new(w[0], w[1])))
            .collect()
    }

    fn area(segs: &[PathSeg]) -> f64 {
        polygon_area(&flatten_segs(segs, 0.1)).abs()
    }

    #[test]
    fn corners_where_one_line_runs_on_past_the_other() {
        // A waist line running on 10 mm past both corners, the side seams
        // ending on it.
        let mut paths = vec![joined(&[(-10.0, 0.0), (110.0, 0.0)])];
        paths.push(joined(&[
            (0.0, 0.0),
            (0.0, 200.0),
            (100.0, 200.0),
            (100.0, 0.0),
        ]));
        let out = find_loops(paths, 0.1);
        let loops: Vec<_> = out.iter().filter(|(_, c)| *c).collect();
        assert_eq!(loops.len(), 1);
        assert!((area(&loops[0].0) - 20_000.0).abs() < 1.0);
        // The stubs are left as lines.
        assert_eq!(out.iter().filter(|(_, c)| !*c).count(), 2);
    }

    #[test]
    fn a_fork_keeps_the_larger_loop() {
        // A seam line round an extension, and its way on straight to the hem.
        let mut paths = vec![joined(&[
            (30.0, 100.0),
            (0.0, 100.0),
            (0.0, 200.0),
            (130.0, 200.0),
            (130.0, 0.0),
            (30.0, 0.0),
        ])];
        paths.push(joined(&[(30.0, 0.0), (30.0, 200.0)]));
        let out = find_loops(paths, 0.1);
        let loops: Vec<_> = out.iter().filter(|(_, c)| *c).collect();
        assert_eq!(loops.len(), 1);
        assert!((area(&loops[0].0) - (100.0 * 200.0 + 30.0 * 100.0)).abs() < 1.0);
        assert_eq!(out.iter().filter(|(_, c)| !*c).count(), 1);
    }

    #[test]
    fn notches_between_two_outlines_come_away() {
        // Cutting line, stitching line 10 mm inside, and notches across.
        let mut paths = strokes(&[
            (0.0, 0.0),
            (100.0, 0.0),
            (100.0, 100.0),
            (0.0, 100.0),
            (0.0, 0.0),
        ]);
        paths.extend(strokes(&[
            (10.0, 10.0),
            (90.0, 10.0),
            (90.0, 90.0),
            (10.0, 90.0),
            (10.0, 10.0),
        ]));
        paths.push(joined(&[(0.0, 50.0), (10.0, 50.0)]));
        paths.push(joined(&[(50.0, 100.0), (50.0, 90.0)]));
        let out = find_loops(paths, 0.1);
        let mut areas: Vec<f64> = out
            .iter()
            .filter(|(_, c)| *c)
            .map(|(s, _)| area(s))
            .collect();
        areas.sort_by(f64::total_cmp);
        assert_eq!(areas.len(), 2);
        assert!((areas[0] - 6400.0).abs() < 1.0 && (areas[1] - 10_000.0).abs() < 1.0);
        assert_eq!(out.iter().filter(|(_, c)| !*c).count(), 2);
    }

    #[test]
    fn lines_across_a_piece_are_not_its_outline() {
        // An outline with a line from side to side.
        let paths = vec![
            joined(&[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0)]),
            joined(&[(100.0, 100.0), (0.0, 100.0), (0.0, 0.0)]),
            joined(&[(0.0, 50.0), (100.0, 50.0)]),
        ];
        let out = find_loops(paths, 0.1);
        let loops: Vec<_> = out.iter().filter(|(_, c)| *c).collect();
        assert_eq!(loops.len(), 1);
        assert!((area(&loops[0].0) - 10_000.0).abs() < 1.0);
        assert_eq!(out.iter().filter(|(_, c)| !*c).count(), 1);
    }

    #[test]
    fn lines_alone_stay_lines() {
        let paths = vec![
            joined(&[(0.0, 0.0), (50.0, 0.0)]),
            joined(&[(0.0, 10.0), (50.0, 30.0), (90.0, 10.0)]),
        ];
        let out = find_loops(paths, 0.1);
        assert_eq!(out.len(), 2);
        assert!(out.iter().all(|(_, c)| !*c));
    }
}
