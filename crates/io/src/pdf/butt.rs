//! Tiles printed to be trimmed at a frame and butted together: each page
//! holds only its own part of the drawing, cut off at the frame drawn round
//! it, with no overlap (plotter drivers print large patterns this way). No
//! shape appears on two pages, but a line cut off at one page's frame
//! carries on from the next page's frame at the same height. The page grid
//! (how many pages across, which way they run) is the one under which the
//! most lines carry on across the joins.

use drapewright_pattern_core::geom::flatten_segs;
use kurbo::{ParamCurve, PathSeg, Point, Rect, Vec2};

/// Ends this close to a frame's side are on it, mm.
const ON: f64 = 0.3;
/// Lines carrying on across a join meet within this, mm (more for lines
/// meeting it at a slant, where frames a hair apart move the meeting point).
const MEET: f64 = 0.3;
/// … and run within this of the same way, radians (10°).
const SAME_WAY: f64 = 0.175;
/// Lines shorter than this (lettering) are no help, mm.
const MIN_LINE: f64 = 3.0;
/// A drawn frame keeps within this of the page's edges, mm.
const MAX_MARGIN: f64 = 25.0;

const LEFT: usize = 0;
const RIGHT: usize = 1;
const TOP: usize = 2;
const BOTTOM: usize = 3;

/// Where a line meets one side of a frame: how far along the side, which
/// way it runs (an angle from 0 to π), and the sine of the angle it meets
/// the side at.
#[derive(Clone, Copy, Debug)]
struct Cut {
    along: f64,
    angle: f64,
    steep: f64,
    /// Where it is on the page, and which way it goes into the page.
    at: Point,
    way: Vec2,
}

/// The lines cut off at each side of one page's frame.
pub(crate) struct Edges {
    sides: [Vec<Cut>; 4],
}

impl Edges {
    /// Where `paths` (in page millimetres, joined end to end) end on the
    /// sides of `frame`.
    pub(crate) fn of<'a>(paths: impl Iterator<Item = &'a [PathSeg]>, frame: Rect) -> Self {
        let mut sides: [Vec<Cut>; 4] = Default::default();
        let corner = 2.0 * ON;
        for segs in paths.filter(|s| !s.is_empty()) {
            let pts = flatten_segs(segs, 0.05);
            let length: f64 = pts.windows(2).map(|w| w[0].distance(w[1])).sum();
            if length < MIN_LINE {
                continue;
            }
            let reversed: Vec<Point> = pts.iter().rev().copied().collect();
            for line in [&pts[..], &reversed[..]] {
                let end = line[0];
                // Which way the line leaves its end, over its last 2 mm.
                let mut walked = 0.0;
                let mut inward = line[line.len() - 1];
                for w in line.windows(2) {
                    walked += w[0].distance(w[1]);
                    if walked >= 2.0 {
                        inward = w[1];
                        break;
                    }
                }
                let way = inward - end;
                if way.hypot() < 1e-6 {
                    continue;
                }
                let way = way / way.hypot();
                let angle = way.y.atan2(way.x).rem_euclid(std::f64::consts::PI);
                // (side, how far inside, how fast the line goes in, where
                // along the side, how fast it goes along, the side's ends)
                let on_side = [
                    (
                        LEFT,
                        end.x - frame.x0,
                        way.x,
                        end.y,
                        way.y,
                        frame.y0,
                        frame.y1,
                    ),
                    (
                        RIGHT,
                        frame.x1 - end.x,
                        -way.x,
                        end.y,
                        way.y,
                        frame.y0,
                        frame.y1,
                    ),
                    (
                        TOP,
                        end.y - frame.y0,
                        way.y,
                        end.x,
                        way.x,
                        frame.x0,
                        frame.x1,
                    ),
                    (
                        BOTTOM,
                        frame.y1 - end.y,
                        -way.y,
                        end.x,
                        way.x,
                        frame.x0,
                        frame.x1,
                    ),
                ];
                for (side, off, inwards, along, sliding, lo, hi) in on_side {
                    if off.abs() <= ON
                        && inwards > 0.0
                        && along > lo + corner
                        && along < hi - corner
                    {
                        // Where the line meets the side itself: a line
                        // stopping a hair short of it (or past it) at a
                        // slant meets it further along.
                        let at_side = along - off / inwards * sliding;
                        sides[side].push(Cut {
                            along: at_side - lo,
                            angle,
                            steep: inwards,
                            at: end,
                            way,
                        });
                    }
                }
            }
        }
        for side in &mut sides {
            side.sort_by(|a, b| a.along.total_cmp(&b.along));
        }
        Self { sides }
    }

    /// Cuts at the same place on most pages (a footer touching the frame)
    /// are page furniture: left out.
    pub(crate) fn without_furniture(mut pages: Vec<Self>) -> Vec<Self> {
        let n = pages.len();
        if n < 4 {
            return pages;
        }
        let furniture: Vec<Vec<Cut>> = (0..4)
            .map(|side| {
                pages
                    .iter()
                    .flat_map(|p| &p.sides[side])
                    .filter(|cut| {
                        let on = pages
                            .iter()
                            .filter(|p| p.sides[side].iter().any(|c| same(cut, c)))
                            .count();
                        2 * on >= n
                    })
                    .copied()
                    .collect()
            })
            .collect();
        for page in &mut pages {
            for (cuts, furniture) in page.sides.iter_mut().zip(&furniture) {
                cuts.retain(|c| !furniture.iter().any(|f| same(c, f)));
            }
        }
        pages
    }

    fn is_empty(&self) -> bool {
        self.sides.iter().all(Vec::is_empty)
    }
}

/// Two cuts at one place, the same way.
fn same(a: &Cut, b: &Cut) -> bool {
    let turn = (a.angle - b.angle).abs();
    (a.along - b.along).abs() <= MEET / a.steep.min(b.steep).max(0.25)
        && turn.min(std::f64::consts::PI - turn) <= SAME_WAY
}

/// The cuts on two sides that meet which carry on across the join, as
/// pairs.
fn partners(a: &[Cut], b: &[Cut]) -> Vec<(Cut, Cut)> {
    let mut used = vec![false; b.len()];
    let mut pairs = Vec::new();
    for cut in a {
        let partner = (0..b.len()).find(|&j| !used[j] && same(cut, &b[j]));
        if let Some(j) = partner {
            used[j] = true;
            pairs.push((*cut, b[j]));
        }
    }
    pairs
}

/// How many cuts on two sides that meet carry on across the join, and how
/// many don't.
fn carry_on(a: &[Cut], b: &[Cut]) -> (usize, usize) {
    let matched = partners(a, b).len();
    (matched, a.len() + b.len() - 2 * matched)
}

/// How far apart pages are along x (`across` true) or along y, from the
/// lines that cross their joins at a slant: a line cut off at one page's
/// frame and at the next's is one straight line for only one step (a line
/// straight across says nothing about it). The median of what each line
/// says, or `fallback` when too few cross at a slant.
fn step(pairs: &[(Cut, Cut)], across: bool, fallback: f64) -> f64 {
    // u across the join, v along it.
    let uv = |p: Vec2| if across { (p.x, p.y) } else { (p.y, p.x) };
    let mut steps: Vec<f64> = pairs
        .iter()
        .filter_map(|(a, b)| {
            let (au, av) = uv(a.at.to_vec2());
            let (bu, bv) = uv(b.at.to_vec2());
            // The line's way from the first page into the next.
            let (du, dv) = uv(-a.way);
            // With the next page `s` on, b at (bu + s, bv) is on the line
            // through a: (bv - av) * du = (bu + s - au) * dv.
            (dv.abs() >= 0.2).then(|| au - bu + (bv - av) * du / dv)
        })
        .collect();
    if steps.len() < 3 {
        return fallback;
    }
    steps.sort_by(f64::total_cmp);
    steps[steps.len() / 2]
}

/// A line cut off at two pages' frames: where it ends on each page (page,
/// point on it).
pub(crate) type Stitch = ((usize, Point), (usize, Point));

/// The pages' places on the sheet (offsets from each page's own
/// millimetres) if one page grid clearly explains where lines are cut off,
/// with the lines that carry on across its joins. `edges` and `frames` are
/// per page; the frames are all one size.
pub(crate) fn butt_grid(edges: &[Edges], frames: &[Rect]) -> Option<(Vec<Vec2>, Vec<Stitch>)> {
    let n = edges.len();
    if n < 2 || edges.iter().all(Edges::is_empty) {
        return None;
    }
    let (fw, fh) = (
        frames.iter().map(Rect::width).sum::<f64>() / n as f64,
        frames.iter().map(Rect::height).sum::<f64>() / n as f64,
    );
    // (score, matched, unmatched, the place of each page)
    type Layout = (i64, usize, usize, Vec<(usize, usize)>);
    let mut layouts: Vec<Layout> = Vec::new();
    for cols in 1..=n {
        let rows = n.div_ceil(cols);
        for column_major in [false, true] {
            for bottom_up in [false, true] {
                for right_to_left in [false, true] {
                    let place = |i: usize| {
                        let (c, r) = if column_major {
                            (i / rows, i % rows)
                        } else {
                            (i % cols, i / cols)
                        };
                        let (cols, rows) = if column_major {
                            (n.div_ceil(rows), rows)
                        } else {
                            (cols, rows)
                        };
                        let c = if right_to_left { cols - 1 - c } else { c };
                        let r = if bottom_up { rows - 1 - r } else { r };
                        (c, r)
                    };
                    let places: Vec<(usize, usize)> = (0..n).map(place).collect();
                    let at = |c: usize, r: usize| places.iter().position(|&p| p == (c, r));
                    let (mut matched, mut unmatched) = (0, 0);
                    for (i, &(c, r)) in places.iter().enumerate() {
                        // A line cut off where no page follows runs off the sheet.
                        let mut pair =
                            |side: usize, other: Option<usize>, facing: usize| match other {
                                Some(j) => {
                                    let (m, u) =
                                        carry_on(&edges[i].sides[side], &edges[j].sides[facing]);
                                    matched += m;
                                    unmatched += u;
                                }
                                None => unmatched += edges[i].sides[side].len(),
                            };
                        pair(RIGHT, at(c + 1, r), LEFT);
                        pair(BOTTOM, at(c, r + 1), TOP);
                        if c == 0 || at(c - 1, r).is_none() {
                            unmatched += edges[i].sides[LEFT].len();
                        }
                        if r == 0 || at(c, r - 1).is_none() {
                            unmatched += edges[i].sides[TOP].len();
                        }
                    }
                    let score = matched as i64 - unmatched as i64;
                    layouts.push((score, matched, unmatched, places));
                }
            }
        }
    }
    layouts.sort_by(|a, b| b.0.cmp(&a.0));
    let (score, matched, unmatched, places) = layouts.first()?.clone();
    // The next different grid, for how clear the choice is.
    let runner_up = layouts
        .iter()
        .find(|l| l.3 != places)
        .map_or(i64::MIN, |l| l.0);
    if matched < 4 || matched < 2 * unmatched || score < runner_up.saturating_add(4) {
        return None;
    }
    // Every page draws its part of the sheet from one origin; the frame
    // lines drawn round it are a hair off here and there. The steps between
    // pages come from the lines crossing the joins.
    let at = |c: usize, r: usize| places.iter().position(|&p| p == (c, r));
    let (mut sideways, mut downways) = (Vec::new(), Vec::new());
    let mut stitches: Vec<Stitch> = Vec::new();
    for (i, &(c, r)) in places.iter().enumerate() {
        if let Some(j) = at(c + 1, r) {
            let pairs = partners(&edges[i].sides[RIGHT], &edges[j].sides[LEFT]);
            stitches.extend(pairs.iter().map(|(a, b)| ((i, a.at), (j, b.at))));
            sideways.extend(pairs);
        }
        if let Some(j) = at(c, r + 1) {
            let pairs = partners(&edges[i].sides[BOTTOM], &edges[j].sides[TOP]);
            stitches.extend(pairs.iter().map(|(a, b)| ((i, a.at), (j, b.at))));
            downways.extend(pairs);
        }
    }
    let (fw, fh) = (step(&sideways, true, fw), step(&downways, false, fh));
    let middle = |mut v: Vec<f64>| {
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    };
    let origin = Vec2::new(
        middle(frames.iter().map(|f| f.x0).collect()),
        middle(frames.iter().map(|f| f.y0).collect()),
    );
    let placed = places
        .iter()
        .map(|&(c, r)| Vec2::new(c as f64 * fw, r as f64 * fh) - origin)
        .collect();
    Some((placed, stitches))
}

/// A frame drawn round a page's drawing: straight lines along all four
/// sides of a large rectangle with the rest of the drawing inside it.
/// `lines` are a page's paths, in millimetres on a page of `size`.
pub(crate) fn drawn_frame<'a>(
    lines: impl Iterator<Item = &'a [PathSeg]> + Clone,
    size: Vec2,
) -> Option<Rect> {
    // Straight stretches along x and along y, by where they lie.
    let mut across: Vec<(f64, f64, f64)> = Vec::new(); // (y, x0, x1)
    let mut down: Vec<(f64, f64, f64)> = Vec::new(); // (x, y0, y1)
    for segs in lines.clone() {
        for seg in segs {
            let PathSeg::Line(l) = seg else { continue };
            let (a, b) = (l.p0, l.p1);
            if (a.y - b.y).abs() < 0.05 && (a.x - b.x).abs() > 1.0 {
                across.push((a.y, a.x.min(b.x), a.x.max(b.x)));
            } else if (a.x - b.x).abs() < 0.05 && (a.y - b.y).abs() > 1.0 {
                down.push((a.x, a.y.min(b.y), a.y.max(b.y)));
            }
        }
    }
    // Lines (stretches lying together) at least 40 % of the page across.
    let long = |stretches: &mut Vec<(f64, f64, f64)>, page: f64| -> Vec<(f64, f64, f64)> {
        stretches.sort_by(|a, b| a.0.total_cmp(&b.0));
        // Stretches lying together, then how much of its extent each
        // group covers.
        let mut groups: Vec<Vec<(f64, f64, f64)>> = Vec::new();
        for &s in stretches.iter() {
            match groups.last_mut() {
                Some(g) if (g[g.len() - 1].0 - s.0).abs() <= 0.1 => g.push(s),
                _ => groups.push(vec![s]),
            }
        }
        groups
            .into_iter()
            .filter_map(|mut g| {
                g.sort_by(|a, b| a.1.total_cmp(&b.1));
                let (mut covered, mut reach) = (0.0, f64::MIN);
                for &(_, from, to) in &g {
                    covered += (to - from.max(reach)).max(0.0);
                    reach = reach.max(to);
                }
                let from = g.iter().map(|s| s.1).fold(f64::MAX, f64::min);
                // Where the group's longest stretch lies stands for it.
                let at = g
                    .iter()
                    .max_by(|a, b| (a.2 - a.1).total_cmp(&(b.2 - b.1)))
                    .map_or(0.0, |s| s.0);
                (covered >= 0.4 * page && covered >= 0.9 * (reach - from))
                    .then_some((at, from, reach))
            })
            .collect()
    };
    let across = long(&mut across, size.x);
    let down = long(&mut down, size.y);
    let (top, bottom) = (across.first()?, across.last()?);
    let (left, right) = (down.first()?, down.last()?);
    let frame = Rect::new(left.0, top.0, right.0, bottom.0);
    let fits =
        |l: &(f64, f64, f64), lo: f64, hi: f64| (l.1 - lo).abs() <= 1.0 && (l.2 - hi).abs() <= 1.0;
    // A frame keeps to the page's margins: a large rectangular piece with
    // everything else inside it is not one.
    let margins = [frame.x0, frame.y0, size.x - frame.x1, size.y - frame.y1];
    if margins.iter().any(|m| !(-0.5..=MAX_MARGIN).contains(m))
        || frame.width() < 0.5 * size.x
        || frame.height() < 0.5 * size.y
        || !fits(top, frame.x0, frame.x1)
        || !fits(bottom, frame.x0, frame.x1)
        || !fits(left, frame.y0, frame.y1)
        || !fits(right, frame.y0, frame.y1)
    {
        return None;
    }
    // Everything drawn is inside it.
    let around = frame.inflate(0.5, 0.5);
    lines
        .flat_map(|segs| segs.iter().flat_map(|s| [s.start(), s.end()]))
        .all(|p: Point| around.contains(p))
        .then_some(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::Line;

    fn line(a: (f64, f64), b: (f64, f64)) -> Vec<PathSeg> {
        vec![PathSeg::Line(Line::new(a, b))]
    }

    #[test]
    fn finds_the_grid_lines_carry_on_across() {
        // A 3 × 2 sheet of 100 × 150 mm frames, pages numbered along rows
        // from the bottom. Slanted lines run across every join.
        let frame = Rect::new(10.0, 10.0, 110.0, 160.0);
        let (cols, rows) = (3, 2);
        let mut edges = Vec::new();
        for i in 0..cols * rows {
            let (c, r) = (i % cols, rows - 1 - i / cols);
            let sheet = Vec2::new(c as f64 * 100.0, r as f64 * 150.0);
            // Lines on the sheet, cut to this page's frame.
            let mut paths: Vec<Vec<PathSeg>> = Vec::new();
            for k in 0..7 {
                let y = 17.0 + 37.0 * k as f64;
                let x = 23.0 + 41.0 * k as f64;
                // Across the sheet, slightly slanted; and down it.
                for (a, b) in [((5.0, y), (295.0, y + 9.0)), ((x, 5.0), (x + 7.0, 295.0))] {
                    let a = Point::new(a.0, a.1) - sheet + frame.origin().to_vec2();
                    let b = Point::new(b.0, b.1) - sheet + frame.origin().to_vec2();
                    if let Some(l) = clip(Line::new(a, b), frame) {
                        paths.push(vec![PathSeg::Line(l)]);
                    }
                }
            }
            edges.push(Edges::of(paths.iter().map(Vec::as_slice), frame));
        }
        let frames = vec![frame; cols * rows];
        let (at, stitches) = butt_grid(&edges, &frames).expect("a grid");
        // Each line crosses its joins: 7 across × 2 joins, 7 down × 1.
        assert_eq!(stitches.len(), 7 * 2 + 7, "{stitches:?}");
        // Page 0 sits bottom left, page 5 top right.
        assert!(
            (at[0] - Vec2::new(-10.0, 150.0 - 10.0)).hypot() < 1e-9,
            "{:?}",
            at[0]
        );
        assert!(
            (at[5] - Vec2::new(200.0 - 10.0, -10.0)).hypot() < 1e-9,
            "{:?}",
            at[5]
        );
    }

    /// Liang–Barsky: the part of `l` inside `r`.
    fn clip(l: Line, r: Rect) -> Option<Line> {
        let (mut t0, mut t1) = (0.0f64, 1.0f64);
        let d = l.p1 - l.p0;
        for (p, q) in [
            (-d.x, l.p0.x - r.x0),
            (d.x, r.x1 - l.p0.x),
            (-d.y, l.p0.y - r.y0),
            (d.y, r.y1 - l.p0.y),
        ] {
            if p == 0.0 {
                if q < 0.0 {
                    return None;
                }
            } else {
                let t = q / p;
                if p < 0.0 {
                    t0 = t0.max(t);
                } else {
                    t1 = t1.min(t);
                }
            }
        }
        (t0 < t1).then(|| Line::new(l.p0 + d * t0, l.p0 + d * t1))
    }

    #[test]
    fn a_drawn_frame_round_the_drawing() {
        let mut paths = vec![
            line((10.0, 10.0), (199.9, 10.0)),
            line((199.9, 10.0), (199.9, 286.0)),
            line((199.9, 286.0), (10.0, 286.0)),
            line((10.0, 286.0), (10.0, 10.0)),
            line((50.0, 60.0), (120.0, 200.0)),
        ];
        let size = Vec2::new(210.0, 297.0);
        let frame = drawn_frame(paths.iter().map(Vec::as_slice), size).unwrap();
        assert!((frame.x0 - 10.0).abs() < 1e-9 && (frame.y1 - 286.0).abs() < 1e-9);
        // Something outside it: not a frame round the drawing.
        paths.push(line((20.0, 290.0), (40.0, 290.0)));
        assert!(drawn_frame(paths.iter().map(Vec::as_slice), size).is_none());
    }
}
