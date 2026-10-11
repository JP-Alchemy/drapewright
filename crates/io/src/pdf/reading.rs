//! Reading lettering that is drawn, not written: plotter drivers print
//! words as pen strokes, so a pattern's piece names and cutting counts are
//! lines like any other. A block of lettering is sorted into letters
//! (strokes that touch), the letters into lines, the block turned upright
//! and sized (whichever of four ways, and of capitals or small letters
//! alone, reads best), and each letter matched against a stroke font of
//! technical lettering (`strokefont`): the letter whose strokes lie
//! closest, both ways, wins. Letters then take their word's case, and
//! words are checked against the words patterns use.

use std::sync::OnceLock;

use kurbo::{Point, Vec2};

use super::strokefont::{CHARS, glyph, without_dot};

/// Strokes this close are one letter's, mm.
const TOUCH: f64 = 0.25;
/// Grid the distance maps are kept on: font units per cell, and the area
/// they cover (font units).
const CELL: f64 = 0.25;
const MAP_X: (f64, f64) = (-8.0, 8.0);
const MAP_Y: (f64, f64) = (-6.0, 14.0);
/// Points along strokes, font units apart.
const SPACING: f64 = 0.5;
/// How wide the font's wide letters (o, n, e, A, H…) are, font units.
const WIDE_LETTER: f64 = 6.6;
/// How tall the font's small letters (x, a, o…) are, in capitals.
const X_HEIGHT: f64 = 0.67;
/// Lettering is read as capitals and small letters unless reading it as
/// small letters alone (x-height the tallest) fits this much better.
const ALL_SMALL: f64 = 0.8;
/// Letters enough to tell which way round a block reads.
const SAMPLE: usize = 24;
/// Lines whose letters fit the font this closely (font units) read surely:
/// logos and hatching read as letters too, but further off.
pub(crate) const SURE: f64 = 0.7;
/// Lines whose letters fit this closely read fairly well: letters in other
/// hands than the font's are about a unit off.
pub(crate) const FAIR: f64 = 1.5;

/// A line of lettering, read.
#[derive(Clone, Debug)]
pub(crate) struct ReadLine {
    pub text: String,
    /// The middle of the line, mm.
    pub at: Point,
    /// Capital height, mm.
    pub size: f64,
    /// Reading direction (unit).
    pub dir: Vec2,
    /// How far its letters are from the font's, on average (font units;
    /// letters read well are under a unit off).
    pub fit: f64,
}

/// Distances to the nearest stroke on a grid (a distance map).
struct Map {
    cells: Vec<f32>,
    width: usize,
    height: usize,
}

impl Map {
    fn of(strokes: &[Vec<(f64, f64)>]) -> Self {
        let width = ((MAP_X.1 - MAP_X.0) / CELL).ceil() as usize;
        let height = ((MAP_Y.1 - MAP_Y.0) / CELL).ceil() as usize;
        let mut cells = vec![f32::MAX; width * height];
        for s in strokes {
            for p in along(s, CELL / 2.0) {
                let (cx, cy) = (
                    ((p.0 - MAP_X.0) / CELL).floor(),
                    ((p.1 - MAP_Y.0) / CELL).floor(),
                );
                if cx >= 0.0 && cy >= 0.0 && (cx as usize) < width && (cy as usize) < height {
                    cells[cy as usize * width + cx as usize] = 0.0;
                }
            }
        }
        // Two passes of a 3-4 chamfer: near enough to true distances.
        let (a, b) = (1.0f32, 1.4f32);
        for y in 0..height {
            for x in 0..width {
                let mut d = cells[y * width + x];
                if x > 0 {
                    d = d.min(cells[y * width + x - 1] + a);
                }
                if y > 0 {
                    d = d.min(cells[(y - 1) * width + x] + a);
                    if x > 0 {
                        d = d.min(cells[(y - 1) * width + x - 1] + b);
                    }
                    if x + 1 < width {
                        d = d.min(cells[(y - 1) * width + x + 1] + b);
                    }
                }
                cells[y * width + x] = d;
            }
        }
        for y in (0..height).rev() {
            for x in (0..width).rev() {
                let mut d = cells[y * width + x];
                if x + 1 < width {
                    d = d.min(cells[y * width + x + 1] + a);
                }
                if y + 1 < height {
                    d = d.min(cells[(y + 1) * width + x] + a);
                    if x + 1 < width {
                        d = d.min(cells[(y + 1) * width + x + 1] + b);
                    }
                    if x > 0 {
                        d = d.min(cells[(y + 1) * width + x - 1] + b);
                    }
                }
                cells[y * width + x] = d;
            }
        }
        Self {
            cells,
            width,
            height,
        }
    }

    /// Distance from `p` (font units) to the nearest stroke.
    fn at(&self, p: (f64, f64)) -> f64 {
        let cx = ((p.0 - MAP_X.0) / CELL).floor();
        let cy = ((p.1 - MAP_Y.0) / CELL).floor();
        // Outside the map: the edge's distance plus the way out to it.
        let (x, y) = (
            cx.clamp(0.0, (self.width - 1) as f64),
            cy.clamp(0.0, (self.height - 1) as f64),
        );
        let outside = (cx - x).abs() + (cy - y).abs();
        f64::from(self.cells[y as usize * self.width + x as usize]) * CELL + outside * CELL
    }
}

/// Points along a polyline about `step` apart, ends included.
fn along(points: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let mut out = vec![points[0]];
    for w in points.windows(2) {
        let (a, b) = (w[0], w[1]);
        // Letters are tens of units across; anything far longer is not one.
        let n = ((b.0 - a.0).hypot(b.1 - a.1) / step)
            .ceil()
            .clamp(1.0, 400.0) as usize;
        for k in 1..=n {
            let t = k as f64 / n as f64;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

/// A letter of the font, ready to compare with.
struct Template {
    c: char,
    points: Vec<(f64, f64)>,
    map: Map,
}

/// The font's letters, centred on their extent across.
fn templates() -> &'static [Template] {
    static FONT: OnceLock<Vec<Template>> = OnceLock::new();
    FONT.get_or_init(|| {
        CHARS
            .chars()
            .filter_map(|c| Some((c, glyph(c)?)))
            .chain("ij".chars().filter_map(|c| Some((c, without_dot(c)?))))
            .map(|(c, glyph)| {
                let strokes: Vec<Vec<(f64, f64)>> = glyph.iter().map(|s| s.to_vec()).collect();
                let (lo, hi) = strokes
                    .iter()
                    .flatten()
                    .fold((f64::MAX, f64::MIN), |(lo, hi), p| {
                        (lo.min(p.0), hi.max(p.0))
                    });
                let mid = (lo + hi) / 2.0;
                let centred: Vec<Vec<(f64, f64)>> = strokes
                    .iter()
                    .map(|s| s.iter().map(|p| (p.0 - mid, p.1)).collect())
                    .collect();
                let points = centred.iter().flat_map(|s| along(s, SPACING)).collect();
                Template {
                    c,
                    map: Map::of(&centred),
                    points,
                }
            })
            .collect()
    })
}

/// The root mean square of some distances.
fn rms(distances: impl Iterator<Item = f64>) -> f64 {
    let (sum, n) = distances.fold((0.0, 0usize), |(sum, n), d| (sum + d * d, n + 1));
    if n == 0 { 0.0 } else { (sum / n as f64).sqrt() }
}

/// The characters a letter's strokes (font units, centred across) look
/// most like, best first, with how far off each is (root mean square, so
/// a stroke missing, as the side of an O is from a C, counts for more than
/// a slightly different hand all round).
fn recognise(strokes: &[Vec<(f64, f64)>]) -> Vec<(char, f64)> {
    let points: Vec<(f64, f64)> = strokes.iter().flat_map(|s| along(s, SPACING)).collect();
    if points.is_empty() {
        return Vec::new();
    }
    let map = Map::of(strokes);
    let mut scores: Vec<(char, f64)> = templates()
        .iter()
        .map(|t| {
            let there = rms(points.iter().map(|p| t.map.at(*p)));
            let back = rms(t.points.iter().map(|p| map.at(*p)));
            (t.c, there + back)
        })
        .collect();
    scores.sort_by(|a, b| a.1.total_cmp(&b.1));
    scores
}

/// Strokes that touch, together: letters (an i's dot comes later).
fn letters(strokes: &[Vec<Point>]) -> Vec<Vec<usize>> {
    let n = strokes.len();
    let bounds: Vec<kurbo::Rect> = strokes
        .iter()
        .map(|s| {
            s.iter()
                .fold(
                    kurbo::Rect::new(f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                    |r, p| r.union_pt(*p),
                )
                .inflate(TOUCH, TOUCH)
        })
        .collect();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (bounds[i], bounds[j]);
            if a.x0 > b.x1 || b.x0 > a.x1 || a.y0 > b.y1 || b.y0 > a.y1 {
                continue;
            }
            let near = strokes[i].windows(2).any(|a| {
                strokes[j]
                    .windows(2)
                    .any(|b| segments_apart(a[0], a[1], b[0], b[1]) <= TOUCH)
            }) || (strokes[i].len() == 1 || strokes[j].len() == 1)
                && strokes[i]
                    .iter()
                    .any(|p| strokes[j].iter().any(|q| p.distance(*q) <= TOUCH));
            if near {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    for i in 0..n {
        let r = root(&mut parent, i);
        groups.entry(r).or_default().push(i);
    }
    groups.into_values().collect()
}

/// How far apart two segments are: none where they cross.
fn segments_apart(a: Point, b: Point, c: Point, d: Point) -> f64 {
    let cross = |o: Point, p: Point, q: Point| (p - o).cross(q - o);
    let (d1, d2) = (cross(c, d, a), cross(c, d, b));
    let (d3, d4) = (cross(a, b, c), cross(a, b, d));
    if d1 * d2 < 0.0 && d3 * d4 < 0.0 {
        return 0.0;
    }
    segment_distance(a, c, d)
        .min(segment_distance(b, c, d))
        .min(segment_distance(c, a, b))
        .min(segment_distance(d, a, b))
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let ab = b - a;
    let len2 = ab.hypot2();
    if len2 < 1e-18 {
        return p.distance(a);
    }
    let t = ((p - a).dot(ab) / len2).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// The way the block's strokes mostly run, as an angle from 0 to π/2: its
/// letters are upright one of four ways from it.
fn main_angle(strokes: &[Vec<Point>]) -> f64 {
    let mut bins = [0.0f64; 90];
    for s in strokes {
        for w in s.windows(2) {
            let d = w[1] - w[0];
            let len = d.hypot();
            if len < 1e-9 {
                continue;
            }
            let a = d.y.atan2(d.x).rem_euclid(std::f64::consts::FRAC_PI_2);
            bins[(a.to_degrees().round() as usize) % 90] += len;
        }
    }
    // Smoothed over neighbouring degrees.
    let best = (0..90)
        .max_by(|&a, &b| {
            let around = |k: usize| bins[(k + 89) % 90] + 2.0 * bins[k] + bins[(k + 1) % 90];
            around(a).total_cmp(&around(b))
        })
        .unwrap_or(0);
    // Then finer: the mean way of the strokes within a degree and a half of
    // it (a long line a degree off drifts a whole line up or down).
    let coarse = (best as f64).to_radians();
    let quarter = std::f64::consts::FRAC_PI_2;
    let (mut sum, mut weight) = (0.0, 0.0);
    for s in strokes {
        for w in s.windows(2) {
            let d = w[1] - w[0];
            let len = d.hypot();
            if len < 1e-9 {
                continue;
            }
            let a = d.y.atan2(d.x).rem_euclid(quarter);
            // Nearest to the coarse angle, across the wrap at 90°.
            let off = (a - coarse + quarter / 2.0).rem_euclid(quarter) - quarter / 2.0;
            if off.abs() <= 1.5f64.to_radians() {
                sum += off * len;
                weight += len;
            }
        }
    }
    if weight > 0.0 {
        coarse + sum / weight
    } else {
        coarse
    }
}

/// A letter on its line: its strokes in the line's frame (x along the line,
/// y up), and its extent.
struct Placed {
    strokes: Vec<Vec<(f64, f64)>>,
    x: (f64, f64),
    y: (f64, f64),
}

impl Placed {
    fn width(&self) -> f64 {
        self.x.1 - self.x.0
    }
    fn height(&self) -> f64 {
        self.y.1 - self.y.0
    }
    fn mid_x(&self) -> f64 {
        (self.x.0 + self.x.1) / 2.0
    }
}

/// The line letters stand on: height at x = 0 and rise per unit along.
struct Baseline {
    level: f64,
    rise: f64,
}

impl Baseline {
    /// Through the feet of a line's letters, robustly: the median of the
    /// slopes between pairs of letters (Theil-Sen), then the median level.
    fn of(row: &[Placed], tallest: f64) -> Self {
        let mut feet: Vec<(f64, f64)> = row
            .iter()
            .filter(|p| p.height() >= 0.4 * tallest)
            .map(|p| (p.mid_x(), p.y.0))
            .collect();
        if feet.is_empty() {
            feet = row.iter().map(|p| (p.mid_x(), p.y.0)).collect();
        }
        let mut slopes: Vec<f64> = Vec::new();
        for (i, a) in feet.iter().enumerate() {
            for b in &feet[i + 1..] {
                if (b.0 - a.0).abs() > 0.5 * tallest {
                    slopes.push((b.1 - a.1) / (b.0 - a.0));
                }
            }
        }
        slopes.sort_by(f64::total_cmp);
        let rise = slopes
            .get(slopes.len() / 2)
            .copied()
            .unwrap_or(0.0)
            .clamp(-0.05, 0.05);
        let mut levels: Vec<f64> = feet.iter().map(|f| f.1 - rise * f.0).collect();
        levels.sort_by(f64::total_cmp);
        Self {
            level: levels[levels.len() / 2],
            rise,
        }
    }

    fn at(&self, x: f64) -> f64 {
        self.level + self.rise * x
    }
}

/// A block's letters laid out one way round: its lines (top first, letters
/// along) on their baselines, and how big and wide the lettering is.
struct Layout {
    dir: Vec2,
    rows: Vec<(Vec<Placed>, Baseline)>,
    /// Capital height, mm.
    cap: f64,
    /// Width against the font's.
    stretch: f64,
    /// Letters' spacing, centre to centre, mm.
    pitch: f64,
}

impl Layout {
    /// The letters (`groups` of `strokes`) laid out reading along `dir`,
    /// sized as capitals and small letters, or as small letters alone.
    fn of(
        strokes: &[Vec<Point>],
        groups: &[Vec<usize>],
        dir: Vec2,
        all_small: bool,
    ) -> Option<Self> {
        let up = Vec2::new(dir.y, -dir.x);
        let frame = |p: &Point| (p.to_vec2().dot(dir), p.to_vec2().dot(up));
        let mut placed: Vec<Placed> = groups
            .iter()
            .map(|g| {
                let strokes: Vec<Vec<(f64, f64)>> = g
                    .iter()
                    .map(|&i| strokes[i].iter().map(frame).collect())
                    .collect();
                let (x, y) = strokes
                    .iter()
                    .flatten()
                    .fold(((f64::MAX, f64::MIN), (f64::MAX, f64::MIN)), |(x, y), p| {
                        ((x.0.min(p.0), x.1.max(p.0)), (y.0.min(p.1), y.1.max(p.1)))
                    });
                Placed { strokes, x, y }
            })
            .collect();
        let mut heights: Vec<f64> = placed.iter().map(Placed::height).collect();
        heights.sort_by(f64::total_cmp);
        let tallest = heights.last().copied().unwrap_or(0.0);
        if tallest <= 0.0 {
            return None;
        }
        // Dots (of i and j, full stops) are small both ways; they join the
        // letter under or over them later.
        let small = |p: &Placed| p.height() < 0.25 * tallest && p.width() < 0.25 * tallest;
        let (marks, bodies): (Vec<Placed>, Vec<Placed>) = placed.drain(..).partition(small);

        // Lines: letters side by side whose heights overlap, and so on along
        // (a line drifting a little stays one line); top line first.
        let n = bodies.len();
        let mut line_of: Vec<usize> = (0..n).collect();
        fn root(line_of: &mut [usize], mut i: usize) -> usize {
            while line_of[i] != i {
                line_of[i] = line_of[line_of[i]];
                i = line_of[i];
            }
            i
        }
        for i in 0..n {
            for j in i + 1..n {
                let (a, b) = (&bodies[i], &bodies[j]);
                let apart = (b.x.0 - a.x.1).max(a.x.0 - b.x.1).max(0.0);
                let overlap = a.y.1.min(b.y.1) - a.y.0.max(b.y.0);
                if apart <= 2.5 * tallest && overlap >= 0.5 * a.height().min(b.height()) {
                    let (ri, rj) = (root(&mut line_of, i), root(&mut line_of, j));
                    line_of[ri] = rj;
                }
            }
        }
        let mut grouped: std::collections::BTreeMap<usize, Vec<Placed>> = Default::default();
        for (i, p) in bodies.into_iter().enumerate() {
            let r = root(&mut line_of, i);
            grouped.entry(r).or_default().push(p);
        }
        let mut rows: Vec<Vec<Placed>> = grouped.into_values().collect();
        // Words a few spaces apart on one line: side by side, at one height.
        let span = |r: &Vec<Placed>| {
            r.iter()
                .fold((f64::MAX, f64::MIN, f64::MAX, f64::MIN), |s, p| {
                    (
                        s.0.min(p.x.0),
                        s.1.max(p.x.1),
                        s.2.min(p.y.0),
                        s.3.max(p.y.1),
                    )
                })
        };
        let mut merged = true;
        while merged {
            merged = false;
            'pairs: for i in 0..rows.len() {
                for j in i + 1..rows.len() {
                    let (a, b) = (span(&rows[i]), span(&rows[j]));
                    let side_by_side = a.1 <= b.0 || b.1 <= a.0;
                    let overlap = a.3.min(b.3) - a.2.max(b.2);
                    if side_by_side && overlap >= 0.6 * (a.3 - a.2).min(b.3 - b.2) {
                        let row = rows.remove(j);
                        rows[i].extend(row);
                        merged = true;
                        break 'pairs;
                    }
                }
            }
        }
        let middle =
            |r: &Vec<Placed>| r.iter().map(|p| p.y.0 + p.y.1).sum::<f64>() / r.len() as f64;
        rows.sort_by(|a, b| middle(b).total_cmp(&middle(a)));
        // Each dot to the letter it is over (or under), else on its own.
        for m in marks {
            let mid = m.mid_x();
            // The letter just under (or over) it: the smallest gap up or down.
            let gap = |p: &Placed| (m.y.0 - p.y.1).max(p.y.0 - m.y.1).max(0.0);
            let home = rows
                .iter_mut()
                .flat_map(|r| r.iter_mut())
                .filter(|p| p.x.0 - 0.2 * tallest <= mid && mid <= p.x.1 + 0.2 * tallest)
                .filter(|p| gap(p) <= 0.4 * tallest)
                .min_by(|a, b| {
                    gap(a)
                        .total_cmp(&gap(b))
                        .then((a.mid_x() - mid).abs().total_cmp(&(b.mid_x() - mid).abs()))
                });
            match home {
                Some(p) => {
                    p.strokes.extend(m.strokes);
                    p.x = (p.x.0.min(m.x.0), p.x.1.max(m.x.1));
                    p.y = (p.y.0.min(m.y.0), p.y.1.max(m.y.1));
                }
                None => {
                    let row = rows.iter_mut().min_by(|a, b| {
                        let gap = |r: &Vec<Placed>| {
                            let (lo, hi) = r.iter().fold((f64::MAX, f64::MIN), |(lo, hi), q| {
                                (lo.min(q.y.0), hi.max(q.y.1))
                            });
                            (lo - m.y.1).max(m.y.0 - hi).max(0.0)
                        };
                        gap(a).total_cmp(&gap(b))
                    });
                    match row {
                        Some(r) => r.push(m),
                        None => rows.push(vec![m]),
                    }
                }
            }
        }

        // Each line's baseline (a line, as the block may be a hair off the way
        // its strokes run), and the block's capital height.
        let rows: Vec<(Vec<Placed>, Baseline)> = rows
            .into_iter()
            .map(|mut r| {
                r.sort_by(|a, b| a.mid_x().total_cmp(&b.mid_x()));
                let baseline = Baseline::of(&r, tallest);
                (r, baseline)
            })
            .collect();
        let mut tops: Vec<f64> = rows
            .iter()
            .flat_map(|(r, b)| r.iter().map(move |p| p.y.1 - b.at(p.mid_x())))
            .filter(|t| *t > 0.0)
            .collect();
        tops.sort_by(f64::total_cmp);
        // Capitals are near the tallest letters (a line of dashes says little),
        // unless the lettering is all small letters, as "seam" is.
        let top = tops
            .get((tops.len() * 85 / 100).min(tops.len().saturating_sub(1)))
            .copied()
            .unwrap_or(tallest)
            .max(0.5 * tallest);
        // Small letters alone are all of a height (an i's dot aside).
        if all_small
            && tops
                .get(tops.len() * 15 / 100)
                .is_some_and(|low| top > 1.3 * low)
        {
            return None;
        }
        let cap = if all_small { top / X_HEIGHT } else { top };
        let scale = 10.0 / cap;
        // Lettering can be drawn wider or narrower than the font: the wide
        // letters (not i, l, 1 or stops) say by how much.
        let mut widths: Vec<f64> = rows
            .iter()
            .flat_map(|(r, _)| r)
            .map(|p| p.width() * scale)
            .filter(|w| *w >= 4.5)
            .collect();
        widths.sort_by(f64::total_cmp);
        let stretch = if widths.len() >= 3 {
            (widths[widths.len() / 2] / WIDE_LETTER).clamp(0.6, 1.8)
        } else {
            1.0
        };
        // Letters are this far apart, centre to centre.
        let mut steps: Vec<f64> = rows
            .iter()
            .flat_map(|(r, _)| r.windows(2).map(|w| w[1].mid_x() - w[0].mid_x()))
            .filter(|d| *d > 0.4 * cap)
            .collect();
        steps.sort_by(f64::total_cmp);
        let pitch = steps
            .get(steps.len() * 3 / 10)
            .copied()
            .unwrap_or(1.15 * cap);
        Some(Self {
            dir,
            rows,
            cap,
            stretch,
            pitch,
        })
    }

    /// A letter's strokes in font units, centred across, on its baseline.
    fn in_font_units(&self, p: &Placed, baseline: &Baseline) -> Vec<Vec<(f64, f64)>> {
        let (scale, mid) = (10.0 / self.cap, p.mid_x());
        p.strokes
            .iter()
            .map(|s| {
                s.iter()
                    .map(|q| {
                        (
                            (q.0 - mid) * scale / self.stretch,
                            (q.1 - baseline.at(mid)) * scale,
                        )
                    })
                    .collect()
            })
            .collect()
    }

    /// How far every `step`th letter is from its likeliest in the font, on
    /// average, mm (lower is better).
    fn fit(&self, step: usize) -> f64 {
        let (total, count) = self
            .rows
            .iter()
            .flat_map(|(row, baseline)| row.iter().map(move |p| (p, baseline)))
            .step_by(step)
            .filter_map(|(p, baseline)| {
                recognise(&self.in_font_units(p, baseline))
                    .first()
                    .map(|g| g.1)
            })
            .fold((0.0, 0usize), |(total, count), d| (total + d, count + 1));
        if count == 0 {
            f64::MAX
        } else {
            total / count as f64 * self.cap / 10.0
        }
    }

    /// The lines, read.
    fn read(&self) -> Vec<ReadLine> {
        let up = Vec2::new(self.dir.y, -self.dir.x);
        let mut lines = Vec::new();
        for (row, baseline) in &self.rows {
            let mut text = String::new();
            let (mut off, mut letters) = (0.0, 0usize);
            let mut last: Option<f64> = None;
            for p in row {
                let mid = p.mid_x();
                if let Some(l) = last {
                    let gap = ((mid - l) / self.pitch).round() as i64 - 1;
                    for _ in 0..gap.clamp(0, 8) {
                        text.push(' ');
                    }
                }
                last = Some(mid);
                if let Some(&(c, d)) = recognise(&self.in_font_units(p, baseline)).first() {
                    text.push(c);
                    off += d;
                    letters += 1;
                }
            }
            let (x0, x1) = row.iter().fold((f64::MAX, f64::MIN), |(a, b), p| {
                (a.min(p.x.0), b.max(p.x.1))
            });
            let middle = (x0 + x1) / 2.0;
            let mid = Vec2::new(middle, baseline.at(middle) + self.cap / 2.0);
            lines.push(ReadLine {
                text,
                at: (self.dir * mid.x + up * mid.y).to_point(),
                size: self.cap,
                dir: self.dir,
                fit: if letters == 0 {
                    f64::MAX
                } else {
                    off / letters as f64
                },
            });
        }
        lines
    }
}

/// Reads a block of lettering from its strokes (polylines, mm): its lines
/// in reading order, top first.
pub(crate) fn read_block(strokes: &[Vec<Point>]) -> Vec<ReadLine> {
    let strokes: Vec<Vec<Point>> = strokes.iter().filter(|s| !s.is_empty()).cloned().collect();
    if strokes.is_empty() {
        return Vec::new();
    }
    let groups = letters(&strokes);
    let angle = main_angle(&strokes);
    // Each way round, as capitals and small letters or small letters alone:
    // whichever a sample of the letters fits the font best is read.
    let step = groups.len().div_ceil(SAMPLE).max(1);
    let mut best: Option<(Layout, f64)> = None;
    for quarter in 0..4 {
        let a = angle + f64::from(quarter) * std::f64::consts::FRAC_PI_2;
        let dir = Vec2::new(a.cos(), a.sin());
        for all_small in [false, true] {
            let Some(layout) = Layout::of(&strokes, &groups, dir, all_small) else {
                continue;
            };
            let fit = layout.fit(step) / if all_small { ALL_SMALL } else { 1.0 };
            if best.as_ref().is_none_or(|b| fit < b.1) {
                best = Some((layout, fit));
            }
        }
    }
    let mut lines = best.map(|(layout, _)| layout.read()).unwrap_or_default();
    for line in &mut lines {
        line.text = correct(line.text.trim());
    }
    lines
}

/// Words patterns use: a word read with one letter off one of these is
/// taken to be it.
const WORDS: &[&str] = &[
    "allowance",
    "and",
    "armhole",
    "back",
    "band",
    "belt",
    "bias",
    "binding",
    "blouse",
    "bodice",
    "bottom",
    "button",
    "buttonhole",
    "buttons",
    "casing",
    "center",
    "centre",
    "coat",
    "collar",
    "contrast",
    "cuff",
    "cuffs",
    "cut",
    "dart",
    "dress",
    "ease",
    "edge",
    "elastic",
    "extension",
    "face",
    "fabric",
    "facing",
    "facings",
    "flap",
    "fly",
    "fold",
    "foldline",
    "from",
    "front",
    "fusible",
    "gather",
    "grain",
    "grainline",
    "gusset",
    "height",
    "hem",
    "hood",
    "inner",
    "interfacing",
    "jacket",
    "lapel",
    "left",
    "leg",
    "length",
    "line",
    "lining",
    "loop",
    "loops",
    "lower",
    "main",
    "middle",
    "neck",
    "neckline",
    "notch",
    "outer",
    "pair",
    "pairs",
    "panel",
    "pants",
    "piece",
    "placket",
    "pleat",
    "pocket",
    "pockets",
    "right",
    "rouleau",
    "ruffle",
    "seam",
    "shield",
    "shirt",
    "shorts",
    "shoulder",
    "side",
    "size",
    "skirt",
    "sleeve",
    "sleeves",
    "stand",
    "strap",
    "straps",
    "strip",
    "the",
    "tie",
    "ties",
    "top",
    "topstitching",
    "trousers",
    "under",
    "upper",
    "waist",
    "waistband",
    "welt",
    "with",
    "yoke",
    "zip",
    "zipper",
];

/// Letters drawn alike but for size (c and C, o and O, s and S…) in the
/// case of the word they are in, and O and 0, l and 1 as letters in words
/// and digits in numbers. A word's first letter may be a capital.
fn settle_case(word: &str) -> String {
    let count = |f: fn(&char) -> bool| word.chars().filter(f).count();
    let (upper, lower, digits) = (
        count(char::is_ascii_uppercase),
        count(char::is_ascii_lowercase),
        count(char::is_ascii_digit),
    );
    let in_letters = word.chars().all(|c| c.is_ascii_alphabetic() || c == '0');
    let capitals = upper >= 2 && upper > lower;
    let small = lower >= 2 && lower > upper;
    let number = digits > 0 && digits >= upper + lower;
    let first = word.find(|c: char| c.is_ascii_alphabetic());
    word.char_indices()
        .map(|(k, c)| match c {
            'c' | 'o' | 's' | 'u' | 'v' | 'w' | 'x' | 'z' if capitals => c.to_ascii_uppercase(),
            'l' if capitals => 'I',
            '0' if capitals && in_letters => 'O',
            'C' | 'O' | 'S' | 'U' | 'V' | 'W' | 'X' | 'Z' if small && Some(k) != first => {
                c.to_ascii_lowercase()
            }
            '0' if small && in_letters => 'o',
            'O' | 'o' if number => '0',
            'l' | 'I' if number => '1',
            c => c,
        })
        .collect()
}

/// Words read with one wrong letter, set right where only one pattern word
/// is that close.
fn correct(text: &str) -> String {
    text.split(' ')
        .map(settle_case)
        .map(|word| {
            let letters: Vec<char> = word.chars().collect();
            if letters.len() < 3 || !letters.iter().all(|c| c.is_alphabetic()) {
                return word.to_string();
            }
            let lower = word.to_lowercase();
            if WORDS.contains(&lower.as_str()) {
                return word.to_string();
            }
            let close: Vec<&str> = WORDS
                .iter()
                .copied()
                .filter(|w| {
                    w.len() == lower.len()
                        && w.chars().zip(lower.chars()).filter(|(a, b)| a != b).count() == 1
                })
                .collect();
            match close.as_slice() {
                [only] => {
                    let upper =
                        letters.iter().filter(|c| c.is_uppercase()).count() * 2 > letters.len();
                    if upper {
                        only.to_uppercase()
                    } else {
                        (*only).to_string()
                    }
                }
                _ => word.to_string(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` drawn in the font: `size` mm capitals, starting at `at`,
    /// reading along `dir`, letters 1.15 capitals apart.
    pub(crate) fn drawn(text: &str, at: Point, size: f64, dir: Vec2) -> Vec<Vec<Point>> {
        let up = Vec2::new(dir.y, -dir.x);
        let mut out = Vec::new();
        for (k, c) in text.chars().enumerate() {
            let Some(strokes) = glyph(c) else { continue };
            let origin = at + dir * (k as f64 * 1.15 * size);
            for s in strokes {
                out.push(
                    s.iter()
                        .map(|p| origin + dir * (p.0 * size / 10.0) + up * (p.1 * size / 10.0))
                        .collect(),
                );
            }
        }
        out
    }

    #[test]
    fn every_letter_reads_as_itself() {
        for c in CHARS.chars() {
            let strokes: Vec<Vec<(f64, f64)>> =
                glyph(c).unwrap().iter().map(|s| s.to_vec()).collect();
            let (lo, hi) = strokes
                .iter()
                .flatten()
                .fold((f64::MAX, f64::MIN), |(l, h), p| (l.min(p.0), h.max(p.0)));
            let centred: Vec<Vec<(f64, f64)>> = strokes
                .iter()
                .map(|s| s.iter().map(|p| (p.0 - (lo + hi) / 2.0, p.1)).collect())
                .collect();
            assert_eq!(recognise(&centred)[0].0, c);
        }
    }

    #[test]
    fn blocks_read_any_way_round() {
        for angle in [0.0f64, 90.0, 180.0, 270.0, 7.0, 123.0] {
            let dir = Vec2::new(angle.to_radians().cos(), angle.to_radians().sin());
            let up = Vec2::new(dir.y, -dir.x);
            let mut strokes = drawn("RIGHT FRONT", Point::new(100.0, 100.0), 4.4, dir);
            strokes.extend(drawn(
                "cut 2 fabric",
                Point::new(100.0, 100.0) - up * 6.6,
                4.4,
                dir,
            ));
            let lines = read_block(&strokes);
            let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
            assert_eq!(texts, ["RIGHT FRONT", "cut 2 fabric"], "at {angle}°");
            assert!((lines[0].size - 4.4).abs() < 0.2, "{}", lines[0].size);
        }
    }

    #[test]
    fn small_letters_alone_read_at_their_size() {
        // "seam" has no capital or tall letter to tell its size by.
        for angle in [0.0f64, 90.0, 180.0, 270.0] {
            let dir = Vec2::new(angle.to_radians().cos(), angle.to_radians().sin());
            let lines = read_block(&drawn("seam", Point::new(50.0, 50.0), 4.0, dir));
            assert_eq!(lines[0].text, "seam", "at {angle}°");
            assert!((lines[0].size - 4.0).abs() < 0.3, "{}", lines[0].size);
        }
    }

    #[test]
    fn another_hands_o_is_no_c() {
        // A narrower O with longer straight sides than the font's: its
        // right side, missing from a C, tells them apart.
        let o = vec![vec![
            (-3.2, 7.5),
            (-3.2, 2.5),
            (-2.4, 0.8),
            (-0.8, 0.0),
            (0.8, 0.0),
            (2.4, 0.8),
            (3.2, 2.5),
            (3.2, 7.5),
            (2.4, 9.2),
            (0.8, 10.0),
            (-0.8, 10.0),
            (-2.4, 9.2),
            (-3.2, 7.5),
        ]];
        assert_eq!(recognise(&o)[0].0, 'O');
    }

    #[test]
    fn an_i_without_its_dot() {
        // The dots are lost; the stems read as i still.
        let strokes: Vec<Vec<Point>> =
            drawn("fabric", Point::new(0.0, 0.0), 3.0, Vec2::new(1.0, 0.0))
                .into_iter()
                .filter(|s| s.iter().any(|p| p.distance(s[0]) > 0.5))
                .collect();
        assert_eq!(read_block(&strokes)[0].text, "fabric");
    }

    #[test]
    fn one_wrong_letter_is_set_right() {
        assert_eq!(correct("SLEEUE cut 2"), "SLEEVE cut 2");
        assert_eq!(correct("fabrie"), "fabric");
        assert_eq!(correct("ODILE"), "ODILE");
        assert_eq!(correct("RCULEAU"), "ROULEAU");
    }

    #[test]
    fn letters_take_the_case_of_their_word() {
        assert_eq!(correct("ODlLE blouse"), "ODILE blouse");
        assert_eq!(correct("LOOPs"), "LOOPS");
        assert_eq!(correct("poCket"), "pocket");
        assert_eq!(correct("Size 4O"), "Size 40");
        assert_eq!(correct("SGC0"), "SGCO");
    }
}
