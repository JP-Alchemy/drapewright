//! Pages that are tiles of one sheet. Home-print patterns split a large
//! sheet over many pages; most tiling tools draw each page as the whole
//! sheet's drawing, moved and clipped to the page. Shapes that appear on two
//! pages, identical but moved, say exactly how those pages fit together:
//! no page grid or labels need guessing.

use std::collections::HashMap;

use kurbo::{ParamCurve, PathSeg, Point, Rect, Vec2};

use crate::assemble::ascending;

/// Shapes shorter than this say too little to match pages by, mm.
const MIN_LENGTH: f64 = 20.0;
/// Shapes match when their points agree to within this, mm.
const SAME: f64 = 0.3;
/// Offsets are counted to the nearest millimetre.
const GRID: f64 = 1.0;
/// A shape matching more shapes than this on another page is a repeated
/// motif (a diamond, a logo) and is not used to match pages.
const MAX_REPEATS: usize = 3;
/// Points compared per shape.
const SAMPLES: usize = 24;

/// A shape, described wherever it is on its page.
#[derive(Clone, Debug)]
pub(crate) struct Print {
    closed: bool,
    segments: usize,
    length: f64,
    /// Where it starts on its page.
    first: Point,
    /// Its extent on its page.
    bounds: Rect,
    /// Some of its points, relative to the first.
    samples: Vec<Vec2>,
}

impl Print {
    pub(crate) fn of(segs: &[PathSeg], closed: bool) -> Option<Print> {
        let first = segs.first()?.start();
        let length: f64 = segs.iter().map(|s| s.start().distance(s.end())).sum();
        if length < MIN_LENGTH {
            return None;
        }
        let step = segs.len().div_ceil(SAMPLES).max(1);
        let samples = segs.iter().step_by(step).map(|s| s.end() - first).collect();
        let bounds = segs.iter().fold(Rect::from_points(first, first), |r, s| {
            r.union_pt(s.start()).union_pt(s.end())
        });
        Some(Print {
            closed,
            segments: segs.len(),
            length,
            first,
            bounds,
            samples,
        })
    }

    /// Bucket for quick lookup: kind, segment count, length to 2 mm.
    fn bucket(&self) -> (bool, usize, i64) {
        (
            self.closed,
            self.segments,
            (self.length / 2.0).floor() as i64,
        )
    }

    fn same_shape(&self, other: &Print) -> bool {
        self.closed == other.closed
            && self.segments == other.segments
            && (self.length - other.length).abs() <= SAME * 2.0
            && self.samples.len() == other.samples.len()
            && self
                .samples
                .iter()
                .zip(&other.samples)
                .all(|(a, b)| (*a - *b).hypot() <= SAME)
    }
}

struct Index<'a> {
    buckets: HashMap<(bool, usize, i64), Vec<&'a Print>>,
}

impl<'a> Index<'a> {
    fn new(prints: impl IntoIterator<Item = &'a Print>) -> Self {
        let mut buckets: HashMap<(bool, usize, i64), Vec<&Print>> = HashMap::new();
        for p in prints {
            buckets.entry(p.bucket()).or_default().push(p);
        }
        Index { buckets }
    }

    /// Shapes the same as `p`, wherever they are.
    fn matches<'b>(&'b self, p: &'b Print) -> impl Iterator<Item = &'a Print> + 'b {
        let (closed, segments, length) = p.bucket();
        (length - 1..=length + 1)
            .filter_map(move |l| self.buckets.get(&(closed, segments, l)))
            .flatten()
            .copied()
            .filter(move |q| q.same_shape(p))
    }
}

/// Labelled neighbours may be out of line by this much, mm (drawings
/// shifted a little on their pages).
const GUIDED: f64 = 30.0;

/// Groups of pages that are tiles of one sheet, each page with its offset
/// on the sheet (mm). Pages that share no shapes are groups of one.
/// `prints[i]` are page i's shapes; `sizes[i]` its width and height, mm;
/// `grid[i]` where page i's label puts it (column, row), if it is labelled.
///
/// Pages are joined by their strongest links first: where most shapes
/// agree, a shape the same as several on the other page (a piece drawn
/// twice on the sheet) counting as a part of one for each. A page whose every shape is drawn on every page (some tiling
/// tools draw the whole sheet on each) shares many with its true
/// neighbours, and only a few, by chance, with others (a page-edge mark, a
/// motif). Labelled pages join only their label's neighbours, on the side
/// the labels say, as far off as the shapes say (tiles overlap, or are
/// trimmed of their margins); one that shares no shape with them (a piece
/// on a page of its own) still goes where its label says, as far from its
/// neighbour as the other pages are from theirs.
pub(crate) fn arrange(
    prints: &[Vec<Print>],
    sizes: &[Vec2],
    grid: &[Option<(i64, i64)>],
) -> Vec<Vec<(usize, Vec2)>> {
    let n = prints.len();
    let indexes: Vec<Index> = prints.iter().map(Index::new).collect();
    // Page furniture (a frame, a footer rule) sits at the same place on
    // many pages; it says nothing about where pages go on the sheet.
    let furniture = |page: usize, p: &Print| {
        let pages = (0..n)
            .filter(|&o| {
                o != page
                    && indexes[o]
                        .matches(p)
                        .any(|q| q.first.distance(p.first) <= 1.0)
            })
            .count();
        pages + 1 >= 3.max(n / 2)
    };
    let useful: Vec<Vec<&Print>> = prints
        .iter()
        .enumerate()
        .map(|(i, page)| page.iter().filter(|p| !furniture(i, p)).collect())
        .collect();
    let useful_indexes: Vec<Index> = useful
        .iter()
        .map(|p| Index::new(p.iter().copied()))
        .collect();

    // Where two pages overlap at an offset, they show the same drawing:
    // most shapes wholly inside the overlap on either page are on the
    // other too. (Without this, two sheets that merely share a piece end
    // of the same length would be laid on top of each other.)
    let agree_where_they_overlap = |i: usize, j: usize, d: Vec2| {
        let (pi, pj) = (sizes[i], sizes[j]);
        let overlap = Rect::new(0.0, 0.0, pi.x, pi.y)
            .intersect(Rect::new(d.x, d.y, d.x + pj.x, d.y + pj.y))
            .inset(-2.0);
        if overlap.width() <= 0.0 || overlap.height() <= 0.0 {
            return true;
        }
        let side = |from: usize, to: usize, area: Rect, shift: Vec2| {
            let inside: Vec<&&Print> = useful[from]
                .iter()
                .filter(|p| {
                    area.contains(p.bounds.origin())
                        && area.contains(Point::new(p.bounds.x1, p.bounds.y1))
                })
                .collect();
            let agree = inside
                .iter()
                .filter(|a| {
                    useful_indexes[to]
                        .matches(a)
                        .any(|b| (a.first - b.first - shift).hypot() <= 1.0)
                })
                .count();
            inside.len() < 3 || agree * 2 >= inside.len()
        };
        side(i, j, overlap, d) && side(j, i, overlap - d, -d)
    };
    // How far a labelled page's labelled neighbour is on the sheet,
    // (columns, rows), from page i.
    let apart = |i: usize, j: usize| -> Option<(i64, i64)> {
        let ((ci, ri), (cj, rj)) = (grid[i]?, grid[j]?);
        Some((cj - ci, rj - ri))
    };

    // Links between pages: page j sits at page i's offset plus d, as so
    // many shapes say.
    let mut links: Vec<(f64, usize, usize, Vec2)> = Vec::new();
    for i in 0..n {
        for (j, theirs) in useful_indexes.iter().enumerate().skip(i + 1) {
            let guide = apart(i, j);
            // Labelled pages join only their neighbours.
            if guide.is_some_and(|(c, r)| c.abs() + r.abs() != 1) {
                continue;
            }
            // (shapes, their weight, offsets summed) for each offset. A
            // shape the same as two or three on the other page (a piece
            // drawn twice on the sheet) votes for each by halves or thirds.
            let mut votes: HashMap<(i64, i64), (usize, f64, Vec2)> = HashMap::new();
            for a in &useful[i] {
                let same: Vec<&Print> = theirs.matches(a).collect();
                if same.is_empty() || same.len() > MAX_REPEATS {
                    continue;
                }
                let weight = 1.0 / same.len() as f64;
                for b in same {
                    // Page j's origin on page i: a shape at `b.first` on
                    // page j is at `a.first` on page i.
                    let d = a.first - b.first;
                    let page = sizes[i];
                    let near = match guide {
                        // On the side the labels say, at least half a page
                        // off and at most a page, and nearly in line.
                        Some((c, r)) => {
                            let (along, across, size) = if c != 0 {
                                (d.x * c as f64, d.y, page.x)
                            } else {
                                (d.y * r as f64, d.x, page.y)
                            };
                            (0.5 * size..=size + 1.0).contains(&along) && across.abs() <= GUIDED
                        }
                        // Two tiles are never in the same place, and
                        // neighbours touch or overlap: they are less than
                        // a page apart.
                        None => {
                            d.hypot() >= 1.0
                                && d.x.abs() <= page.x + 1.0
                                && d.y.abs() <= page.y + 1.0
                        }
                    };
                    if !near {
                        continue;
                    }
                    let key = ((d.x / GRID).round() as i64, (d.y / GRID).round() as i64);
                    let v = votes.entry(key).or_insert((0, 0.0, Vec2::ZERO));
                    v.0 += 1;
                    v.1 += weight;
                    v.2 += d;
                }
            }
            let best = votes
                .into_iter()
                .max_by(|a, b| a.1.1.total_cmp(&b.1.1).then(b.0.cmp(&a.0)));
            if let Some((_, (count, weight, sum))) = best
                && count >= 2
            {
                let d = sum / count as f64;
                if agree_where_they_overlap(i, j, d) {
                    links.push((weight, i, j, d));
                }
            }
        }
    }
    // The strongest links first (then as found), each joining two groups
    // not yet joined.
    let order = ascending(&links.iter().map(|l| -l.0).collect::<Vec<_>>());
    let links: Vec<(f64, usize, usize, Vec2)> = order.into_iter().map(|k| links[k]).collect();
    let mut group: Vec<usize> = (0..n).collect();
    let mut offset: Vec<Vec2> = vec![Vec2::ZERO; n];
    let join = |group: &mut Vec<usize>, offset: &mut Vec<Vec2>, i: usize, j: usize, d: Vec2| {
        let (gi, gj) = (group[i], group[j]);
        if gi == gj {
            return;
        }
        // Page j to page i's offset plus d, its group with it.
        let shift = offset[i] + d - offset[j];
        for k in 0..n {
            if group[k] == gj {
                group[k] = gi;
                offset[k] += shift;
            }
        }
    };
    for &(_, i, j, d) in &links {
        join(&mut group, &mut offset, i, j, d);
    }
    // How far apart labelled neighbours are, across and down, as most
    // shapes say; a page apart if none do.
    let step = |across: bool| {
        let mut steps: Vec<f64> = links
            .iter()
            .filter_map(|&(_, i, j, d)| match apart(i, j)? {
                (c, 0) if across => Some(d.x / c as f64),
                (0, r) if !across => Some(d.y / r as f64),
                _ => None,
            })
            .collect();
        steps.sort_by(f64::total_cmp);
        steps.get(steps.len() / 2).copied()
    };
    let (across, down) = (step(true), step(false));
    // Labelled pages joined to nothing by shapes: beside the labelled page
    // nearest them on the sheet, where their labels say.
    loop {
        let mut best: Option<(i64, usize, usize)> = None;
        for i in 0..n {
            for j in 0..n {
                let (Some((ci, ri)), Some((cj, rj))) = (grid[i], grid[j]) else {
                    continue;
                };
                if group[i] == group[j] {
                    continue;
                }
                let apart = (ci - cj).abs() + (ri - rj).abs();
                if best.is_none_or(|b| (apart, i, j) < (b.0, b.1, b.2)) {
                    best = Some((apart, i, j));
                }
            }
        }
        let Some((_, i, j)) = best else { break };
        let (c, r) = apart(i, j).expect("both labelled");
        let page = sizes[i];
        let d = Vec2::new(
            c as f64 * across.unwrap_or(page.x),
            r as f64 * down.unwrap_or(page.y),
        );
        join(&mut group, &mut offset, i, j, d);
    }
    // Each group, from its first page.
    let mut groups: Vec<Vec<(usize, Vec2)>> = Vec::new();
    let mut seen: Vec<usize> = Vec::new();
    for i in 0..n {
        if seen.contains(&group[i]) {
            continue;
        }
        seen.push(group[i]);
        let members: Vec<usize> = (0..n).filter(|&k| group[k] == group[i]).collect();
        let origin = offset[members[0]];
        groups.push(members.iter().map(|&k| (k, offset[k] - origin)).collect());
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use kurbo::{Affine, Line};

    fn square(x: f64, y: f64, side: f64) -> Vec<PathSeg> {
        let p = |a: f64, b: f64| Point::new(x + a, y + b);
        vec![
            PathSeg::Line(Line::new(p(0.0, 0.0), p(side, 0.0))),
            PathSeg::Line(Line::new(p(side, 0.0), p(side, side))),
            PathSeg::Line(Line::new(p(side, side), p(0.0, side))),
            PathSeg::Line(Line::new(p(0.0, side), p(0.0, 0.0))),
        ]
    }

    #[test]
    fn pages_sharing_shapes_fit_together() {
        // A sheet with two shapes, seen by three pages 180 mm apart (with a
        // little rounding noise), each with its own frame; and a cover page.
        let sheet = [square(100.0, 50.0, 120.0), square(250.0, 60.0, 90.0)];
        let page = |dx: f64, noise: f64| -> Vec<Print> {
            let mut shapes: Vec<Print> = sheet
                .iter()
                .map(|s| {
                    let moved: Vec<PathSeg> = s
                        .iter()
                        .map(|seg| Affine::translate((-dx + noise, noise)) * *seg)
                        .collect();
                    Print::of(&moved, true).unwrap()
                })
                .collect();
            shapes.push(Print::of(&square(10.0, 10.0, 190.0), true).unwrap());
            shapes
        };
        let cover = vec![Print::of(&square(30.0, 30.0, 100.0), true).unwrap()];
        let a4 = Vec2::new(210.0, 297.0);
        let groups = arrange(
            &[
                cover,
                page(0.0, 0.0),
                page(180.0, 0.0001),
                page(360.0, -0.0001),
            ],
            &[a4; 4],
            &[None; 4],
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![(0, Vec2::ZERO)]);
        let sheet = &groups[1];
        assert_eq!(sheet.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [1, 2, 3]);
        assert!((sheet[1].1 - Vec2::new(180.0, 0.0)).hypot() < 0.01);
        assert!((sheet[2].1 - Vec2::new(360.0, 0.0)).hypot() < 0.01);
    }

    #[test]
    fn a_piece_drawn_twice_on_the_sheet_does_not_outvote_a_neighbour() {
        // Pages 0, 1 and 2: page 1 beside page 0 (three shapes say), page 2
        // below page 1 (three shapes say). Page 2 also shows two copies of
        // four shapes page 0 shows one of, at an offset no tile is at: four
        // shapes against three, but each the same as two.
        let moved = |segs: Vec<PathSeg>, by: (f64, f64)| -> Vec<PathSeg> {
            segs.iter().map(|s| Affine::translate(by) * *s).collect()
        };
        let print = |segs: Vec<PathSeg>| Print::of(&segs, true).unwrap();
        let beside = [(20.0, 20.0, 25.0), (60.0, 30.0, 30.0), (100.0, 40.0, 35.0)];
        let below = [(20.0, 20.0, 40.0), (70.0, 30.0, 45.0), (120.0, 40.0, 50.0)];
        let twice = [
            (10.0, 150.0, 21.0),
            (40.0, 150.0, 22.0),
            (70.0, 150.0, 23.0),
            (100.0, 150.0, 24.0),
        ];
        let page0: Vec<Print> = beside
            .iter()
            .map(|&(x, y, s)| print(square(x + 150.0, y, s)))
            .chain(twice.iter().map(|&(x, y, s)| print(square(x, y, s))))
            .collect();
        let page1: Vec<Print> = beside
            .iter()
            .map(|&(x, y, s)| print(square(x, y, s)))
            .chain(
                below
                    .iter()
                    .map(|&(x, y, s)| print(square(x, y + 200.0, s))),
            )
            .collect();
        let page2: Vec<Print> = below
            .iter()
            .map(|&(x, y, s)| print(square(x, y, s)))
            .chain(twice.iter().flat_map(|&(x, y, s)| {
                [
                    print(moved(square(x, y, s), (200.0, -280.0))),
                    print(moved(square(x, y, s), (195.0, -250.0))),
                ]
            }))
            .collect();
        let a4 = Vec2::new(210.0, 297.0);
        let groups = arrange(&[page0, page1, page2], &[a4; 3], &[None; 3]);
        assert_eq!(groups.len(), 1);
        let at: Vec<Vec2> = groups[0].iter().map(|(_, d)| *d).collect();
        assert!((at[1] - Vec2::new(150.0, 0.0)).hypot() < 0.01, "{at:?}");
        assert!((at[2] - Vec2::new(150.0, 200.0)).hypot() < 0.01, "{at:?}");
    }
}
