//! A paper pattern piece photographed lying on a table beside a sheet of
//! paper of known size. The sheet's corners, tapped on the photo, say how
//! the table lies in it: the photo is read as the table seen from above, a
//! pixel a millimetre. The piece is the paper round a point tapped on it,
//! as far as its colour goes (lines printed on it and holes in it are
//! part of it); its outline is traced, smoothed, split at its corners into
//! edges and drawn with curves, as a piece in millimetres.

use drapewright_pattern_core::{
    Edge, Outline, Piece, Pt, ScaleBasis, Segment, Source, SourceFormat, SourceScale,
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::import::{
    AllowanceQuestion, ImportError, ImportResult, ScaleInfo, ScaleOption, ScaleReason, sha256_hex,
};

type P2 = [f64; 2];

/// The furthest a piece reaches from the point tapped on it, mm.
const REACH: f64 = 1300.0;
/// Half the square round the point tapped whose colour is the piece's, mm.
const SAMPLE: i64 = 6;
/// Lightness counts for this much of a difference in colour as hue does:
/// shade falls across paper unevenly.
const LIGHTNESS: f64 = 0.5;
/// Holes and notches in the outline narrower than this are closed, mm.
const CLOSE: usize = 2;
/// The traced outline is smoothed over this many millimetres either side.
const SMOOTH: usize = 2;
/// A corner: the outline turning more than this (degrees) between the
/// directions it comes from and goes on in, over `CORNER_SPAN` mm.
const CORNER_TURN: f64 = 35.0;
const CORNER_SPAN: usize = 8;
/// Curves are drawn within this of the outline traced, mm.
const FIT: f64 = 0.8;

/// What the photo shows, and where the user tapped.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PhotoRequest {
    /// The sheet of paper's corners in the photo (px from its top left),
    /// in any order.
    pub corners: [[f64; 2]; 4],
    /// The sheet's size, mm, either way round.
    pub sheet: [f64; 2],
    /// A point on the piece in the photo, px.
    pub on: [f64; 2],
    /// How far the paper's colour may vary before its edge is reached: 0
    /// to 1, 0.25 suiting most.
    pub tolerance: f64,
    /// What to call the piece.
    pub name: String,
    /// The photo's file name.
    pub file: String,
}

/// A piece found in a photo.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Traced {
    /// Its outline in the photo (px), to show over it.
    pub outline: Vec<[f64; 2]>,
    /// It reaches the edge of the photo, or further than a piece goes:
    /// part of it may be missing, or the table taken for it.
    pub cut_off: bool,
    /// The piece, as an import.
    pub result: ImportResult,
}

fn fail(why: &str) -> ImportError {
    ImportError::Photo(why.to_string())
}

/// A projective map of the plane: the table as the photo shows it.
#[derive(Clone, Copy, Debug)]
struct Homography([f64; 9]);

impl Homography {
    /// The map taking each of `from` to the matching one of `to`; none if
    /// three of either lie on a line.
    fn between(from: [P2; 4], to: [P2; 4]) -> Option<Homography> {
        let mut rows = [[0.0; 9]; 8];
        for i in 0..4 {
            let ([x, y], [u, v]) = (from[i], to[i]);
            rows[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
            rows[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
        }
        for c in 0..8 {
            let pivot = (c..8).max_by(|&a, &b| rows[a][c].abs().total_cmp(&rows[b][c].abs()))?;
            if rows[pivot][c].abs() < 1e-12 {
                return None;
            }
            rows.swap(c, pivot);
            for r in 0..8 {
                if r != c {
                    let k = rows[r][c] / rows[c][c];
                    let pivot_row = rows[c];
                    for (x, p) in rows[r].iter_mut().zip(pivot_row) {
                        *x -= k * p;
                    }
                }
            }
        }
        let h: Vec<f64> = (0..8).map(|r| rows[r][8] / rows[r][r]).collect();
        Some(Homography([
            h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0,
        ]))
    }

    /// Where `p` goes; none if behind the eye (beyond the horizon).
    fn apply(&self, [x, y]: P2) -> Option<P2> {
        let m = &self.0;
        let w = m[6] * x + m[7] * y + m[8];
        (w > 1e-12).then(|| {
            [
                (m[0] * x + m[1] * y + m[2]) / w,
                (m[3] * x + m[4] * y + m[5]) / w,
            ]
        })
    }
}

/// The photo's pixels, RGBA, row by row.
struct Image<'a> {
    rgba: &'a [u8],
    width: usize,
    height: usize,
}

impl Image<'_> {
    /// The colour at `p` (px), blended between pixels, as CIE Lab; none off
    /// the photo.
    fn lab(&self, [x, y]: P2) -> Option<[f64; 3]> {
        let (fx, fy) = (x - 0.5, y - 0.5);
        if fx < 0.0 || fy < 0.0 || fx > (self.width - 1) as f64 || fy > (self.height - 1) as f64 {
            return None;
        }
        let (x0, y0) = (fx.floor() as usize, fy.floor() as usize);
        let (x1, y1) = ((x0 + 1).min(self.width - 1), (y0 + 1).min(self.height - 1));
        let (tx, ty) = (fx - x0 as f64, fy - y0 as f64);
        let px = |x: usize, y: usize, k: usize| f64::from(self.rgba[4 * (y * self.width + x) + k]);
        let rgb = [0, 1, 2].map(|k| {
            let top = px(x0, y0, k) * (1.0 - tx) + px(x1, y0, k) * tx;
            let bottom = px(x0, y1, k) * (1.0 - tx) + px(x1, y1, k) * tx;
            (top * (1.0 - ty) + bottom * ty) / 255.0
        });
        Some(lab(rgb))
    }
}

/// sRGB (0 to 1) as CIE Lab (D65).
fn lab(rgb: [f64; 3]) -> [f64; 3] {
    let linear = rgb.map(|c| {
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    let [r, g, b] = linear;
    let xyz = [
        (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047,
        0.2126 * r + 0.7152 * g + 0.0722 * b,
        (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883,
    ];
    let f = xyz.map(|t| {
        if t > 0.008856 {
            t.cbrt()
        } else {
            7.787 * t + 16.0 / 116.0
        }
    });
    [
        116.0 * f[1] - 16.0,
        500.0 * (f[0] - f[1]),
        200.0 * (f[1] - f[2]),
    ]
}

/// How unlike two colours are, as the tolerance reads it (about 0 to 1).
fn unlike(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dl = (a[0] - b[0]) * LIGHTNESS;
    (dl * dl + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt() / 100.0
}

/// The four points in order round them, the same way round as `like`.
fn in_order(points: [P2; 4]) -> [P2; 4] {
    let middle = [
        points.iter().map(|p| p[0]).sum::<f64>() / 4.0,
        points.iter().map(|p| p[1]).sum::<f64>() / 4.0,
    ];
    let mut sorted = points;
    sorted.sort_by(|a, b| {
        let angle = |p: &P2| (p[1] - middle[1]).atan2(p[0] - middle[0]);
        angle(a).total_cmp(&angle(b))
    });
    sorted
}

fn distance(a: P2, b: P2) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// What each cell of the table (a millimetre square) is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Cell {
    Unseen,
    Piece,
    Not,
    /// Off the photo, or the sheet.
    Out,
}

/// The table round the point tapped, a cell a millimetre: `size` cells
/// across, its first cell at `origin` (mm).
struct Table {
    size: usize,
    cells: Vec<Cell>,
}

impl Table {
    fn at(&self, i: usize, j: usize) -> Cell {
        self.cells[j * self.size + i]
    }
}

/// Traces the piece in a photo: `rgba` its pixels (`width` by `height`),
/// `file` the photo's bytes as read (for its fingerprint).
pub fn trace(
    rgba: &[u8],
    (width, height): (u32, u32),
    file: &[u8],
    request: &PhotoRequest,
) -> Result<Traced, ImportError> {
    let (width, height) = (width as usize, height as usize);
    if width < 2 || height < 2 || rgba.len() < width * height * 4 {
        return Err(fail("the photo has no pixels"));
    }
    let image = Image {
        rgba,
        width,
        height,
    };
    // The sheet: its corners in the photo, the same way round as its
    // corners on the table, its longer sides the longer.
    let corners = in_order(request.corners);
    let sides = [
        distance(corners[0], corners[1]) + distance(corners[2], corners[3]),
        distance(corners[1], corners[2]) + distance(corners[3], corners[0]),
    ];
    let (long, short) = (
        request.sheet[0].max(request.sheet[1]),
        request.sheet[0].min(request.sheet[1]),
    );
    if short <= 0.0 {
        return Err(fail("the sheet has no size"));
    }
    let (w, h) = if sides[0] >= sides[1] {
        (long, short)
    } else {
        (short, long)
    };
    let sheet = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
    let (Some(to_table), Some(to_photo)) = (
        Homography::between(corners, sheet),
        Homography::between(sheet, corners),
    ) else {
        return Err(fail("the sheet’s corners don’t make a rectangle"));
    };
    let Some(seed) = to_table.apply(request.on) else {
        return Err(fail("the point tapped isn’t on the table"));
    };
    let in_sheet = |p: P2| p[0] >= -3.0 && p[1] >= -3.0 && p[0] <= w + 3.0 && p[1] <= h + 3.0;
    if in_sheet(seed) {
        return Err(fail(
            "the point tapped is on the sheet of paper: tap the pattern piece",
        ));
    }
    let size = (2.0 * REACH) as usize + 1;
    let origin = [seed[0] - REACH, seed[1] - REACH];
    let centre = |i: usize, j: usize| [origin[0] + i as f64 + 0.5, origin[1] + j as f64 + 0.5];
    let colour = |i: usize, j: usize| -> Option<[f64; 3]> {
        let p = centre(i, j);
        if in_sheet(p) {
            return None;
        }
        image.lab(to_photo.apply(p)?)
    };
    // The paper's colour: the middle of each part of it round the point.
    let (si, sj) = (REACH as usize, REACH as usize);
    let mut samples: Vec<[f64; 3]> = Vec::new();
    for dj in -SAMPLE..=SAMPLE {
        for di in -SAMPLE..=SAMPLE {
            if let Some(c) = colour((si as i64 + di) as usize, (sj as i64 + dj) as usize) {
                samples.push(c);
            }
        }
    }
    if samples.is_empty() {
        return Err(fail("the point tapped isn’t on the photo"));
    }
    let paper = [0, 1, 2].map(|k| {
        let mut v: Vec<f64> = samples.iter().map(|c| c[k]).collect();
        v.sort_by(f64::total_cmp);
        v[v.len() / 2]
    });
    // The paper, as far as its colour goes.
    let mut table = Table {
        size,
        cells: vec![Cell::Unseen; size * size],
    };
    let mut stack = vec![(si, sj)];
    let mut cut_off = false;
    while let Some((i, j)) = stack.pop() {
        let k = j * size + i;
        if table.cells[k] != Cell::Unseen {
            continue;
        }
        table.cells[k] = match colour(i, j) {
            None => Cell::Out,
            Some(c) if unlike(c, paper) > request.tolerance => Cell::Not,
            Some(_) => Cell::Piece,
        };
        if table.cells[k] == Cell::Out && !in_sheet(centre(i, j)) {
            cut_off = true;
        }
        if table.cells[k] != Cell::Piece {
            continue;
        }
        if i == 0 || j == 0 || i == size - 1 || j == size - 1 {
            cut_off = true;
            continue;
        }
        stack.extend([(i + 1, j), (i - 1, j), (i, j + 1), (i, j - 1)]);
    }
    let mask = filled(&table);
    let outline = smoothed(&traced(&mask, (si - mask.from[0], sj - mask.from[1]))?);
    if outline.len() < 8 {
        return Err(fail(
            "no piece was found round the point tapped: its colour is too like what it lies on",
        ));
    }
    let shift = [
        origin[0] + mask.from[0] as f64,
        origin[1] + mask.from[1] as f64,
    ];
    let on_table: Vec<P2> = outline
        .iter()
        .map(|p| [p[0] + shift[0], p[1] + shift[1]])
        .collect();
    let in_photo: Vec<[f64; 2]> = on_table
        .iter()
        .step_by(4)
        .filter_map(|&p| to_photo.apply(p))
        .collect();
    let sha256 = sha256_hex(file);
    let piece = piece_of(&on_table, request, &sha256);
    let pieces = vec![piece];
    let allowance = AllowanceQuestion::for_unmeasured(&pieces, &[], None);
    Ok(Traced {
        outline: in_photo,
        cut_off,
        result: ImportResult {
            pieces,
            scale: ScaleInfo {
                chosen: ScaleOption {
                    mm_per_unit: 1.0,
                    basis: ScaleBasis::Measured,
                },
                certain: true,
                reason: ScaleReason::ReferenceSheet,
                alternatives: Vec::new(),
            },
            allowance,
            sizes: Default::default(),
            notes: Vec::new(),
            sha256,
            pages: Vec::new(),
        },
    })
}

/// The piece's cells, within the box round them (and a cell's margin):
/// its holes filled and its narrowest gaps closed.
struct Mask {
    /// The box's first cell on the table, and its size.
    from: [usize; 2],
    size: [usize; 2],
    on: Vec<bool>,
}

impl Mask {
    fn at(&self, i: i64, j: i64) -> bool {
        i >= 0
            && j >= 0
            && (i as usize) < self.size[0]
            && (j as usize) < self.size[1]
            && self.on[j as usize * self.size[0] + i as usize]
    }
}

fn filled(table: &Table) -> Mask {
    let n = table.size;
    let (mut low, mut high) = ([n, n], [0, 0]);
    for j in 0..n {
        for i in 0..n {
            if table.at(i, j) == Cell::Piece {
                low = [low[0].min(i), low[1].min(j)];
                high = [high[0].max(i), high[1].max(j)];
            }
        }
    }
    let margin = CLOSE + 2;
    let from = [low[0].saturating_sub(margin), low[1].saturating_sub(margin)];
    let to = [(high[0] + margin).min(n - 1), (high[1] + margin).min(n - 1)];
    let size = [to[0] - from[0] + 1, to[1] - from[1] + 1];
    let mut on: Vec<bool> = (0..size[0] * size[1])
        .map(|k| table.at(from[0] + k % size[0], from[1] + k / size[0]) == Cell::Piece)
        .collect();
    // Closed: grown by `CLOSE` and shrunk back, square by square.
    let grown = |on: &[bool]| -> Vec<bool> {
        let at = |v: &[bool], i: usize, j: usize| v[j * size[0] + i];
        let mut across = vec![false; on.len()];
        for j in 0..size[1] {
            for i in 0..size[0] {
                let (lo, hi) = (i.saturating_sub(CLOSE), (i + CLOSE).min(size[0] - 1));
                across[j * size[0] + i] = (lo..=hi).any(|x| at(on, x, j));
            }
        }
        let mut out = vec![false; on.len()];
        for j in 0..size[1] {
            for i in 0..size[0] {
                let (lo, hi) = (j.saturating_sub(CLOSE), (j + CLOSE).min(size[1] - 1));
                out[j * size[0] + i] = (lo..=hi).any(|y| at(&across, i, y));
            }
        }
        out
    };
    let not = |v: Vec<bool>| -> Vec<bool> { v.iter().map(|b| !b).collect() };
    on = not(grown(&not(grown(&on))));
    // Holes filled: whatever isn't reached from the box's edge.
    let mut outside = vec![false; on.len()];
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for i in 0..size[0] {
        stack.extend([(i, 0), (i, size[1] - 1)]);
    }
    for j in 0..size[1] {
        stack.extend([(0, j), (size[0] - 1, j)]);
    }
    while let Some((i, j)) = stack.pop() {
        let k = j * size[0] + i;
        if on[k] || outside[k] {
            continue;
        }
        outside[k] = true;
        if i > 0 {
            stack.push((i - 1, j));
        }
        if j > 0 {
            stack.push((i, j - 1));
        }
        if i + 1 < size[0] {
            stack.push((i + 1, j));
        }
        if j + 1 < size[1] {
            stack.push((i, j + 1));
        }
    }
    Mask {
        from,
        size,
        on: outside.iter().map(|o| !o).collect(),
    }
}

/// The outline round the piece's cells that `seed` (a cell in the mask) is
/// in: the corners of the cells along its edge, the piece on the left
/// going round (anticlockwise as y goes down the page… clockwise on it).
fn traced(mask: &Mask, seed: (usize, usize)) -> Result<Vec<P2>, ImportError> {
    let (si, sj) = (seed.0 as i64, seed.1 as i64);
    if !mask.at(si, sj) {
        return Err(fail(
            "no piece was found round the point tapped: its colour is too like what it lies on",
        ));
    }
    // From the seed rightwards to the piece's edge.
    let mut i = si;
    while mask.at(i + 1, sj) {
        i += 1;
    }
    // Walk the cracks between cells: at a corner (x, y), heading `d`, the
    // piece on the right.
    let dirs: [(i64, i64); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    // The cells either side ahead of corner (x, y) going `d`: left, right.
    let ahead = |x: i64, y: i64, d: usize| -> (bool, bool) {
        let (dx, dy) = dirs[d];
        // The cell ahead-right of the corner and ahead-left.
        let (rx, ry, lx, ly) = match (dx, dy) {
            (1, 0) => (x, y, x, y - 1),
            (0, 1) => (x - 1, y, x, y),
            (-1, 0) => (x - 1, y - 1, x - 1, y),
            _ => (x, y - 1, x - 1, y - 1),
        };
        (mask.at(lx, ly), mask.at(rx, ry))
    };
    // Start on the right side of cell (i, sj), going down: the piece on
    // the right (west of the crack).
    let start = (i + 1, sj, 1usize);
    let (mut x, mut y, mut d) = start;
    let mut points = Vec::new();
    let limit = 4 * mask.on.len() + 16;
    loop {
        points.push([x as f64, y as f64]);
        let (dx, dy) = dirs[d];
        x += dx;
        y += dy;
        // Turn right where the cell ahead-right is the piece, left where
        // the cell ahead-left isn't... keeping the piece on the right.
        let (left, right) = ahead(x, y, d);
        d = match (left, right) {
            (_, false) => (d + 1) % 4,
            (false, true) => d,
            (true, true) => (d + 3) % 4,
        };
        if (x, y, d) == start || points.len() > limit {
            break;
        }
    }
    Ok(points)
}

/// The outline evened out: each point the mean of those within `SMOOTH`
/// along it, then one every millimetre.
fn smoothed(points: &[P2]) -> Vec<P2> {
    let n = points.len();
    if n < 3 {
        return points.to_vec();
    }
    let k = SMOOTH as i64;
    let mean: Vec<P2> = (0..n as i64)
        .map(|i| {
            let mut s = [0.0, 0.0];
            for o in -k..=k {
                let p = points[(i + o).rem_euclid(n as i64) as usize];
                s = [s[0] + p[0], s[1] + p[1]];
            }
            [s[0] / (2 * k + 1) as f64, s[1] / (2 * k + 1) as f64]
        })
        .collect();
    // Evenly, a millimetre apart.
    let mut out = vec![mean[0]];
    let mut carried = 0.0;
    for i in 0..n {
        let (a, b) = (mean[i], mean[(i + 1) % n]);
        let l = distance(a, b);
        let mut s = 1.0 - carried;
        while s <= l {
            let t = s / l;
            out.push([a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]);
            s += 1.0;
        }
        carried = l - (s - 1.0);
    }
    if out.len() > 1 && distance(out[0], out[out.len() - 1]) < 0.5 {
        out.pop();
    }
    out
}

/// The outline's corners: where it turns most, more than `CORNER_TURN`.
fn corners(points: &[P2]) -> Vec<usize> {
    let n = points.len();
    let span = CORNER_SPAN.min(n / 4).max(1);
    let turn: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b, c) = (
                points[(i + n - span) % n],
                points[i],
                points[(i + span) % n],
            );
            let (u, v) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            let cross = u[0] * v[1] - u[1] * v[0];
            let dot = u[0] * v[0] + u[1] * v[1];
            cross.atan2(dot).abs().to_degrees()
        })
        .collect();
    (0..n)
        .filter(|&i| {
            turn[i] > CORNER_TURN
                && (1..=span)
                    .all(|o| turn[i] >= turn[(i + o) % n] && turn[i] > turn[(i + n - o) % n])
        })
        .collect()
}

/// A cubic through `points`, or a line where they lie along one, within
/// `FIT`: Schneider's algorithm (Graphics Gems, 1990).
fn fitted(points: &[P2], out: &mut Vec<Segment>) {
    let n = points.len();
    let (first, last) = (points[0], points[n - 1]);
    let chord = distance(first, last);
    let off_line = points
        .iter()
        .map(|&p| {
            if chord < 1e-9 {
                return distance(p, first);
            }
            ((last[0] - first[0]) * (first[1] - p[1]) - (first[0] - p[0]) * (last[1] - first[1]))
                .abs()
                / chord
        })
        .fold(0.0, f64::max);
    if off_line <= FIT || n < 4 {
        out.push(Segment::line(last));
        return;
    }
    let tangent = |a: P2, b: P2| {
        let l = distance(a, b).max(1e-9);
        [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
    };
    let reach = 3.min(n - 1);
    let t1 = tangent(first, points[reach]);
    let t2 = tangent(last, points[n - 1 - reach]);
    split_fit(points, t1, t2, out, 0);
}

fn split_fit(points: &[P2], t1: P2, t2: P2, out: &mut Vec<Segment>, depth: usize) {
    let n = points.len();
    let (first, last) = (points[0], points[n - 1]);
    // Chord-length parameters.
    let mut u = vec![0.0; n];
    for i in 1..n {
        u[i] = u[i - 1] + distance(points[i - 1], points[i]);
    }
    let total = u[n - 1].max(1e-9);
    for v in &mut u {
        *v /= total;
    }
    let bezier = |c: [P2; 4], t: f64| -> P2 {
        let s = 1.0 - t;
        let w = [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t];
        [
            w.iter().zip(&c).map(|(w, p)| w * p[0]).sum(),
            w.iter().zip(&c).map(|(w, p)| w * p[1]).sum(),
        ]
    };
    let solve = |u: &[f64]| -> [P2; 4] {
        // Least squares for how far along each tangent the controls go.
        let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
        for (p, &t) in points.iter().zip(u) {
            let s = 1.0 - t;
            let (b1, b2) = (3.0 * s * s * t, 3.0 * s * t * t);
            let a1 = [t1[0] * b1, t1[1] * b1];
            let a2 = [t2[0] * b2, t2[1] * b2];
            let base = bezier([first, first, last, last], t);
            let r = [p[0] - base[0], p[1] - base[1]];
            c00 += a1[0] * a1[0] + a1[1] * a1[1];
            c01 += a1[0] * a2[0] + a1[1] * a2[1];
            c11 += a2[0] * a2[0] + a2[1] * a2[1];
            x0 += a1[0] * r[0] + a1[1] * r[1];
            x1 += a2[0] * r[0] + a2[1] * r[1];
        }
        let det = c00 * c11 - c01 * c01;
        let chord = distance(first, last);
        let (mut l1, mut l2) = if det.abs() > 1e-12 {
            ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det)
        } else {
            (chord / 3.0, chord / 3.0)
        };
        if l1 < chord * 1e-3 || l2 < chord * 1e-3 {
            l1 = chord / 3.0;
            l2 = chord / 3.0;
        }
        [
            first,
            [first[0] + t1[0] * l1, first[1] + t1[1] * l1],
            [last[0] + t2[0] * l2, last[1] + t2[1] * l2],
            last,
        ]
    };
    let worst = |c: [P2; 4], u: &[f64]| -> (f64, usize) {
        points
            .iter()
            .zip(u)
            .enumerate()
            .map(|(i, (p, &t))| (distance(*p, bezier(c, t)), i))
            .fold((0.0, 0), |a, b| if b.0 > a.0 { b } else { a })
    };
    let mut c = solve(&u);
    let (mut error, mut at) = worst(c, &u);
    // A few rounds of finding each point's nearest place on the curve.
    for _ in 0..4 {
        if error <= FIT {
            break;
        }
        for (t, p) in u.iter_mut().zip(points) {
            let s = 1.0 - *t;
            let q = bezier(c, *t);
            let d1 = [
                3.0 * (s * s * (c[1][0] - c[0][0])
                    + 2.0 * s * *t * (c[2][0] - c[1][0])
                    + *t * *t * (c[3][0] - c[2][0])),
                3.0 * (s * s * (c[1][1] - c[0][1])
                    + 2.0 * s * *t * (c[2][1] - c[1][1])
                    + *t * *t * (c[3][1] - c[2][1])),
            ];
            let d2 = [
                6.0 * (s * (c[2][0] - 2.0 * c[1][0] + c[0][0])
                    + *t * (c[3][0] - 2.0 * c[2][0] + c[1][0])),
                6.0 * (s * (c[2][1] - 2.0 * c[1][1] + c[0][1])
                    + *t * (c[3][1] - 2.0 * c[2][1] + c[1][1])),
            ];
            let r = [q[0] - p[0], q[1] - p[1]];
            let num = r[0] * d1[0] + r[1] * d1[1];
            let den = d1[0] * d1[0] + d1[1] * d1[1] + r[0] * d2[0] + r[1] * d2[1];
            if den.abs() > 1e-12 {
                *t = (*t - num / den).clamp(0.0, 1.0);
            }
        }
        c = solve(&u);
        (error, at) = worst(c, &u);
    }
    if error <= FIT || n < 6 || depth > 12 {
        out.push(Segment::cubic(c[1], c[2], c[3]));
        return;
    }
    // Split where it is worst, the tangent there shared.
    let at = at.clamp(2, n - 3);
    let middle = {
        let (a, b) = (points[at - 1], points[at + 1]);
        let l = distance(a, b).max(1e-9);
        [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
    };
    split_fit(&points[..=at], t1, [-middle[0], -middle[1]], out, depth + 1);
    split_fit(&points[at..], middle, t2, out, depth + 1);
}

/// The piece drawn round `outline` (mm on the table), its edges from
/// corner to corner, set at the page's top left.
fn piece_of(outline: &[P2], request: &PhotoRequest, sha256: &str) -> Piece {
    let n = outline.len();
    let mut cut = corners(outline);
    if cut.is_empty() {
        // Round all the way: four edges, evenly.
        cut = (0..4).map(|k| k * n / 4).collect();
    }
    let low = [
        outline.iter().map(|p| p[0]).fold(f64::MAX, f64::min),
        outline.iter().map(|p| p[1]).fold(f64::MAX, f64::min),
    ];
    let at = |i: usize| {
        let p = outline[i % n];
        [p[0] - low[0], p[1] - low[1]]
    };
    let mut edges = Vec::new();
    for (k, &from) in cut.iter().enumerate() {
        let to = if k + 1 < cut.len() {
            cut[k + 1]
        } else {
            cut[0] + n
        };
        let points: Vec<P2> = (from..=to).map(at).collect();
        let mut segments = Vec::new();
        fitted(&points, &mut segments);
        edges.push(Edge::new(format!("e{}", k + 1), segments));
    }
    let mut piece = Piece::new(
        "photo",
        if request.name.trim().is_empty() {
            "Photographed piece"
        } else {
            request.name.trim()
        },
        Outline {
            start: Pt::from(at(cut[0])),
            edges,
        },
    );
    piece.source = Some(Source {
        format: SourceFormat::Photo,
        file: Some(request.file.clone()),
        sha256: Some(sha256.to_string()),
        reference: None,
        scale: Some(SourceScale {
            mm_per_unit: 1.0,
            basis: ScaleBasis::Measured,
            confirmed: true,
        }),
        words: Vec::new(),
    });
    piece
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_homography_takes_four_points_where_they_go() {
        let from = [[10.0, 20.0], [300.0, 40.0], [280.0, 260.0], [30.0, 240.0]];
        let to = [[0.0, 0.0], [210.0, 0.0], [210.0, 297.0], [0.0, 297.0]];
        let h = Homography::between(from, to).unwrap();
        for (f, t) in from.iter().zip(&to) {
            let p = h.apply(*f).unwrap();
            assert!(distance(p, *t) < 1e-6, "{p:?} {t:?}");
        }
        assert!(
            Homography::between([[0.0, 0.0], [1.0, 1.0], [2.0, 2.0], [3.0, 0.0]], to).is_none()
        );
    }

    #[test]
    fn corners_are_found_where_an_outline_turns() {
        // A square 100 mm across, a point a millimetre.
        let mut square = Vec::new();
        for k in 0..400 {
            let s = f64::from(k % 100);
            square.push(match k / 100 {
                0 => [s, 0.0],
                1 => [100.0, s],
                2 => [100.0 - s, 100.0],
                _ => [0.0, 100.0 - s],
            });
        }
        assert_eq!(corners(&square), [0, 100, 200, 300]);
        // A circle has none.
        let circle: Vec<P2> = (0..628)
            .map(|k| {
                let a = f64::from(k) / 100.0;
                [100.0 * a.cos(), 100.0 * a.sin()]
            })
            .collect();
        assert!(corners(&circle).is_empty());
    }

    #[test]
    fn a_curve_is_fitted_within_a_millimetre() {
        let arc: Vec<P2> = (0..=150)
            .map(|k| {
                let a = f64::from(k) / 100.0;
                [200.0 * a.cos(), 200.0 * a.sin()]
            })
            .collect();
        let mut segments = Vec::new();
        fitted(&arc, &mut segments);
        assert!(segments.len() <= 3, "{} segments", segments.len());
        let line: Vec<P2> = (0..=100)
            .map(|k| [f64::from(k), 0.3 * f64::from(k % 2)])
            .collect();
        let mut segments = Vec::new();
        fitted(&line, &mut segments);
        assert_eq!(segments.len(), 1);
        assert!(matches!(segments[0], Segment::Line(_)));
    }
}
