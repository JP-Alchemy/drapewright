//! Pages that are tiles of one sheet. Home-print patterns split a large
//! sheet over many pages; most tiling tools draw each page as the whole
//! sheet's drawing, moved and clipped to the page. Shapes that appear on two
//! pages, identical but moved, say exactly how those pages fit together:
//! no page grid or labels need guessing.

use std::collections::{HashMap, VecDeque};

use kurbo::{ParamCurve, PathSeg, Point, Vec2};

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
        Some(Print {
            closed,
            segments: segs.len(),
            length,
            first,
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

/// Groups of pages that are tiles of one sheet, each page with its offset
/// on the sheet (mm). Pages that share no shapes are groups of one.
/// `prints[i]` are page i's shapes.
pub(crate) fn arrange(prints: &[Vec<Print>]) -> Vec<Vec<(usize, Vec2)>> {
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

    // Links between pages: page j sits at page i's offset plus d.
    let mut links: Vec<Vec<(usize, Vec2)>> = vec![Vec::new(); n];
    for i in 0..n {
        for j in i + 1..n {
            let mut votes: HashMap<(i64, i64), (usize, Vec2)> = HashMap::new();
            for a in &useful[i] {
                let same: Vec<&Print> = useful_indexes[j].matches(a).collect();
                if same.is_empty() || same.len() > MAX_REPEATS {
                    continue;
                }
                for b in same {
                    let d = a.first - b.first;
                    // Two tiles are never in the same place.
                    if d.hypot() < 1.0 {
                        continue;
                    }
                    let key = ((d.x / GRID).round() as i64, (d.y / GRID).round() as i64);
                    let v = votes.entry(key).or_insert((0, Vec2::ZERO));
                    v.0 += 1;
                    v.1 += d;
                }
            }
            let best = votes
                .into_iter()
                .max_by(|a, b| a.1.0.cmp(&b.1.0).then(b.0.cmp(&a.0)));
            if let Some((_, (count, sum))) = best
                && count >= 2
            {
                let d = sum / count as f64;
                links[i].push((j, d));
                links[j].push((i, -d));
            }
        }
    }
    // Each connected group, placed from its first page.
    let mut placed: Vec<Option<Vec2>> = vec![None; n];
    let mut groups = Vec::new();
    for root in 0..n {
        if placed[root].is_some() {
            continue;
        }
        placed[root] = Some(Vec2::ZERO);
        let mut group = vec![(root, Vec2::ZERO)];
        let mut queue = VecDeque::from([root]);
        while let Some(i) = queue.pop_front() {
            let at = placed[i].expect("placed before queued");
            for &(j, d) in &links[i] {
                if placed[j].is_none() {
                    placed[j] = Some(at + d);
                    group.push((j, at + d));
                    queue.push_back(j);
                }
            }
        }
        group.sort_by_key(|(i, _)| *i);
        groups.push(group);
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
        let groups = arrange(&[
            cover,
            page(0.0, 0.0),
            page(180.0, 0.0001),
            page(360.0, -0.0001),
        ]);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], vec![(0, Vec2::ZERO)]);
        let sheet = &groups[1];
        assert_eq!(sheet.iter().map(|(i, _)| *i).collect::<Vec<_>>(), [1, 2, 3]);
        assert!((sheet[1].1 - Vec2::new(180.0, 0.0)).hypot() < 0.01);
        assert!((sheet[2].1 - Vec2::new(360.0, 0.0)).hypot() < 0.01);
    }
}
