//! Patch pockets: a piece sewn by its sides and bottom onto the face of
//! another, where that one's pattern marks its placement (a line drawn
//! round where the pocket goes, or along the three sides sewn), its top left
//! open. Find seams fits each piece not sewn yet to each placement drawn on
//! another piece; one of its shape is sewn there, one seam for each edge of
//! the pocket sewn. Only pieces shaped as patch pockets are tried: a few
//! centimetres across at least, sewn by most of their outline, which turns
//! (a band's long edge fits too many lines drawn inside pieces). A pocket
//! can also be sewn onto a line chosen by hand, which fits less closely.

use std::borrow::Borrow;

use drapewright_pattern_core::stitching::{
    line_closed, line_points, polyline_length, stitching_edges,
};
use drapewright_pattern_core::upright::up_on_page;
use drapewright_pattern_core::{EdgeKind, Id, InternalLine, LineKind, Piece};
use kurbo::{Point, Vec2};

use crate::model::{Ease, Seam, SeamSide};
use crate::suggest::{Reason, Suggestion, deeper, inside, strip};

/// Points compared along a pocket's sewn run and its placement.
const SAMPLES: usize = 64;
/// Starting points tried round a closed placement, before the best is
/// narrowed down.
const OFFSETS: usize = 128;
/// A placement is at least this long, mm.
const SHORTEST: f64 = 60.0;
/// A pocket's top is turned down by at most this, mm: its sides run past
/// the placement's corners by that much.
const TURNED_DOWN: f64 = 30.0;
/// A pocket is at most this share of the piece it is sewn onto.
const LARGEST: f64 = 0.5;
/// A pocket is at least this wide its narrowest way, mm: smaller pieces
/// fitting lines drawn on others are tabs, slits' facings and marks.
const SMALLEST: f64 = 35.0;
/// A pocket's opening is at most this share of its way round: it is sewn
/// on by most of its outline.
const WIDEST_OPENING: f64 = 0.4;
/// A pocket's sewn run (its sides and bottom) reaches at least this share
/// of its opening's width away from it: it turns, as a U does. A straight
/// run (a band's long edge) fits too many lines drawn inside pieces.
const DEEP: f64 = 0.2;
/// The grain of a pocket sewn on lies along the panel's, or across it,
/// within this, degrees.
const GRAIN_OFF: f64 = 10.0;
/// Fits this close to the best (mm on average) are as good as it, and the
/// pocket's opening is chosen among them by which way up it is.
const AS_GOOD: f64 = 1.0;

/// How closely a pocket must fit its placement: on average, and at worst,
/// mm; how much longer than the pocket's sewn run its placement may be
/// (drawn a little apart), mm; and whether the pocket's grain must lie
/// along the panel's.
#[derive(Clone, Copy)]
struct Limits {
    mean: f64,
    worst: f64,
    slack: f64,
    grain: bool,
}

const FOUND: Limits = Limits {
    mean: 3.0,
    worst: 8.0,
    slack: 10.0,
    grain: true,
};
const BY_HAND: Limits = Limits {
    mean: 6.0,
    worst: 12.0,
    slack: 25.0,
    grain: false,
};

/// How a pocket lies on a placement.
#[derive(Clone, Copy, Debug)]
struct Fit {
    /// The pocket's edge left open.
    mouth: usize,
    /// Taken off each end of its sewn run: the turn-down, mm.
    trim: f64,
    /// Where along the placement the run (trimmed) starts, mm, and whether
    /// it runs backwards along it.
    start: f64,
    backwards: bool,
    /// The placement's length for each millimetre of the run.
    scale: f64,
    /// The pocket's mirror image fits (its other copy, of a pair).
    mirrored: bool,
    /// The turn laying the pocket on the placement, radians.
    turn: f64,
    /// How far apart they are then, mm: on average (root mean square), and
    /// at worst.
    mean: f64,
    worst: f64,
}

/// The point `at` mm along an open polyline.
fn along(points: &[Point], at: f64) -> Point {
    let mut walked = 0.0;
    for w in points.windows(2) {
        let d = w[0].distance(w[1]);
        if walked + d >= at && d > 0.0 {
            return w[0].lerp(w[1], ((at - walked) / d).clamp(0.0, 1.0));
        }
        walked += d;
    }
    points[points.len() - 1]
}

/// The turn (the pocket's points flipped first, if `mirrored`) and shift
/// best laying `a` on `b`, point for point; how far apart they then are, on
/// average and at worst.
fn laid(a: &[Point], b: &[Point], mirrored: bool) -> (f64, f64, f64) {
    let flip = |p: Point| if mirrored { Point::new(p.x, -p.y) } else { p };
    let n = a.len() as f64;
    let mean_of = |ps: &mut dyn Iterator<Item = Point>| {
        let s = ps.fold(Vec2::ZERO, |s, p| s + p.to_vec2());
        (s / n).to_point()
    };
    let ca = mean_of(&mut a.iter().map(|&p| flip(p)));
    let cb = mean_of(&mut b.iter().copied());
    let (mut cross, mut dot) = (0.0, 0.0);
    for (&p, &q) in a.iter().zip(b) {
        let (u, v) = (flip(p) - ca, q - cb);
        cross += u.cross(v);
        dot += u.dot(v);
    }
    let turn = cross.atan2(dot);
    let (s, c) = turn.sin_cos();
    let (mut sum, mut worst) = (0.0, 0.0f64);
    for (&p, &q) in a.iter().zip(b) {
        let u = flip(p) - ca;
        let moved = cb + Vec2::new(c * u.x - s * u.y, s * u.x + c * u.y);
        let d = moved.distance(q);
        sum += d * d;
        worst = worst.max(d);
    }
    (turn, (sum / n).sqrt(), worst)
}

/// `v` turned by `turn` radians, flipped first if `mirrored`.
fn turned(v: [f64; 2], turn: f64, mirrored: bool) -> [f64; 2] {
    let v = if mirrored { [v[0], -v[1]] } else { v };
    let (s, c) = turn.sin_cos();
    [c * v[0] - s * v[1], s * v[0] + c * v[1]]
}

fn grain_of(piece: &Piece) -> Option<[f64; 2]> {
    let g = piece.grainline.as_ref()?;
    let d = [g.to.x - g.from.x, g.to.y - g.from.y];
    let l = d[0].hypot(d[1]);
    (l > 0.0).then(|| [d[0] / l, d[1] / l])
}

/// A pocket's sewn run with `mouth` left open: its other edges' stitching
/// lines end to end, from the mouth's end round to its start, and where
/// each edge starts along it.
fn run(stitching: &[Vec<Point>], mouth: usize) -> (Vec<Point>, Vec<(usize, f64, f64)>) {
    let n = stitching.len();
    let mut points: Vec<Point> = Vec::new();
    let mut spans = Vec::new();
    let mut walked = 0.0;
    for k in 1..n {
        let e = (mouth + k) % n;
        let line = &stitching[e];
        let length = polyline_length(line);
        spans.push((e, walked, walked + length));
        walked += length;
        let skip = usize::from(!points.is_empty());
        points.extend(line.iter().skip(skip));
    }
    (points, spans)
}

/// Whether a run turns away from the chord between its ends (the
/// pocket's opening) by at least `DEEP` of its width.
fn turns(points: &[Point]) -> bool {
    let (a, b) = (points[0], points[points.len() - 1]);
    let chord = b - a;
    let width = chord.hypot();
    if width <= 0.0 {
        return true;
    }
    let deepest = points
        .iter()
        .map(|&p| (p - a).cross(chord).abs() / width)
        .fold(0.0, f64::max);
    deepest >= DEEP * width
}

/// The ways the pocket's run, with `mouth` open, lies on a placement
/// (`line`, closed or not) closest: along an open placement either way; and
/// round a closed one, each starting point that fits better than those
/// either side of it (a rectangle fits at each corner, turned), each way
/// round. None if the run is not about as long as the placement, within
/// `slack` mm, or `near` mm on average.
fn fit_run(
    stitching: &[Vec<Point>],
    mouth: usize,
    (line, closed): (&[Point], bool),
    mirrored: bool,
    (slack, near): (f64, f64),
) -> Vec<Fit> {
    let perimeter: f64 = stitching.iter().map(|e| polyline_length(e)).sum();
    let (points, _) = run(stitching, mouth);
    let length = polyline_length(&points);
    if !turns(&points) || perimeter - length > WIDEST_OPENING * perimeter {
        return Vec::new();
    }
    let total = polyline_length(line);
    // The placement is the pocket's finished outline (all round, or along
    // the sewn run): what the pocket has over is its top turned down.
    let over = if closed {
        (perimeter - total) / 2.0
    } else {
        (length - total) / 2.0
    };
    if !(-slack..=TURNED_DOWN + slack).contains(&over) {
        return Vec::new();
    }
    let trim = over.clamp(0.0, TURNED_DOWN);
    let sewn = length - 2.0 * trim;
    if sewn <= 0.0 {
        return Vec::new();
    }
    let a: Vec<Point> = (0..SAMPLES)
        .map(|k| along(&points, trim + sewn * k as f64 / (SAMPLES - 1) as f64))
        .collect();
    let on = |start: f64, backwards: bool, scale: f64| -> Vec<Point> {
        (0..SAMPLES)
            .map(|k| {
                let u = sewn * scale * k as f64 / (SAMPLES - 1) as f64;
                let at = if backwards { start - u } else { start + u };
                along(line, if closed { at.rem_euclid(total) } else { at })
            })
            .collect()
    };
    let try_at = |start: f64, backwards: bool, scale: f64| {
        let (turn, mean, worst) = laid(&a, &on(start, backwards, scale), mirrored);
        Fit {
            mouth,
            trim,
            start,
            backwards,
            scale,
            mirrored,
            turn,
            mean,
            worst,
        }
    };
    if !closed {
        let scale = total / sewn;
        return vec![try_at(0.0, false, scale), try_at(total, true, scale)];
    }
    // Round a closed placement: every starting point either way, then each
    // that fits better than its neighbours narrowed down.
    let step = total / OFFSETS as f64;
    let mut out = Vec::new();
    for backwards in [false, true] {
        let coarse: Vec<Fit> = (0..OFFSETS)
            .map(|k| try_at(step * k as f64, backwards, 1.0))
            .collect();
        for k in 0..OFFSETS {
            let here = coarse[k].mean;
            let (before, after) = (
                coarse[(k + OFFSETS - 1) % OFFSETS].mean,
                coarse[(k + 1) % OFFSETS].mean,
            );
            if here > before || here >= after || here > near + step {
                continue;
            }
            let mut best = coarse[k];
            let mut by = step;
            for _ in 0..8 {
                by /= 2.0;
                for start in [best.start - by, best.start + by] {
                    let f = try_at(start, backwards, 1.0);
                    if f.mean < best.mean {
                        best = f;
                    }
                }
            }
            best.start = best.start.rem_euclid(total);
            out.push(best);
        }
    }
    out
}

/// Cut as a pair: the pocket has a mirror copy.
fn pair(piece: &Piece) -> bool {
    piece
        .cut
        .iter()
        .any(|c| c.quantity >= 2 && c.mirrored.unwrap_or(true))
}

/// The copies of a pocket to fit, each on its own: as drawn, and a pair's
/// mirror copy; only the one some of its seams sew already, if any (a
/// pocket sewn on in part).
fn copies_of(piece: &Piece, existing: &[Seam]) -> &'static [&'static [bool]] {
    match sewn_copy(piece, existing) {
        Some(true) => &[&[true]],
        Some(false) => &[&[false]],
        None if pair(piece) => &[&[false], &[true]],
        None => &[&[false]],
    }
}

/// The copy of a pocket its seams sew already (true: a pair's mirror copy),
/// if any do.
fn sewn_copy(piece: &Piece, existing: &[Seam]) -> Option<bool> {
    existing
        .iter()
        .flat_map(|s| [&s.a, &s.b])
        .find(|side| side.piece == piece.id && !side.is_line())
        .map(|side| side.mirror)
}

/// Worn on both sides of the body: cut as a pair, or on the fold.
fn both_sides(piece: &Piece) -> bool {
    let on_fold = piece.has_fold() && piece.cut.iter().all(|c| c.on_fold.unwrap_or(true));
    pair(piece) || on_fold
}

/// The edge left open is the pocket's hem, if it has one: marked so, or
/// given a hem's allowance.
fn hemmed(piece: &Piece, edge: usize) -> bool {
    piece.outline.edges[edge].kind == EdgeKind::Hem || deeper(piece, edge)
}

/// How a pocket best lies on a placement drawn on `host`, within `limits`:
/// its run fitted with each edge left open in turn, as drawn or mirrored
/// (`mirrors`: its copies to try), or only at its hems if it has any. Of
/// the fits as good as the best: the one standing the way the panel
/// stands; then the one open at its top; then the closest.
fn best_fit(
    (pocket, stitching): (&Piece, &[Vec<Point>]),
    host: &Piece,
    (points, closed): (&[Point], bool),
    limits: Limits,
    mirrors: &[bool],
) -> Option<Fit> {
    if stitching.len() < 2 || points.len() < 2 {
        return None;
    }
    let grains = (grain_of(pocket), grain_of(host));
    let (cos_off, sin_off) = (GRAIN_OFF.to_radians().cos(), GRAIN_OFF.to_radians().sin());
    let grain_ok = |f: &Fit| match grains {
        (Some(g), Some(h)) if limits.grain => {
            let g = turned(g, f.turn, f.mirrored);
            let c = (g[0] * h[0] + g[1] * h[1]).abs();
            c >= cos_off || c <= sin_off
        }
        _ => true,
    };
    // A pocket with a hem is open there (at its top, or at its side).
    let hems: Vec<usize> = (0..stitching.len())
        .filter(|&e| hemmed(pocket, e))
        .collect();
    let mouths: Vec<usize> = if hems.is_empty() {
        (0..stitching.len()).collect()
    } else {
        hems
    };
    let fits: Vec<Fit> = mouths
        .into_iter()
        .flat_map(|mouth| {
            mirrors
                .iter()
                .flat_map(|&m| {
                    fit_run(
                        stitching,
                        mouth,
                        (points, closed),
                        m,
                        (limits.slack, limits.mean),
                    )
                })
                .collect::<Vec<_>>()
        })
        .filter(|f| f.mean <= limits.mean && f.worst <= limits.worst && grain_ok(f))
        .collect();
    let least = fits.iter().map(|f| f.mean).fold(f64::INFINITY, f64::min);
    let (up, host_up) = (up_on_page(pocket), up_on_page(host));
    // How far the pocket is turned from standing as the panel stands, in
    // steps of about 10°.
    let off_upright = |f: &Fit| {
        let u = turned(up, f.turn, f.mirrored);
        let off = (u[0] * host_up[0] + u[1] * host_up[1])
            .clamp(-1.0, 1.0)
            .acos();
        (off / 0.2).round() as i32
    };
    // How high up the panel the opening's middle is, from the pocket's.
    let middle = |points: &[Point]| {
        let s = points.iter().fold(Vec2::ZERO, |s, p| s + p.to_vec2());
        s / points.len().max(1) as f64
    };
    let centre = middle(&stitching.concat());
    let opening_height = |f: &Fit| {
        let m = middle(&stitching[f.mouth]) - centre;
        let m = turned([m.x, m.y], f.turn, f.mirrored);
        m[0] * host_up[0] + m[1] * host_up[1]
    };
    fits.into_iter()
        .filter(|f| f.mean <= least + AS_GOOD)
        .min_by(|x, y| {
            off_upright(x)
                .cmp(&off_upright(y))
                .then(
                    opening_height(y)
                        .round()
                        .total_cmp(&opening_height(x).round()),
                )
                .then(x.mirrored.cmp(&y.mirrored))
                .then(x.mean.total_cmp(&y.mean))
        })
}

/// "patch pocket" → "Patch pocket".
fn sentence(name: &str) -> String {
    let mut chars = name.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

/// The seams sewing a pocket onto a line on `host` as `fit` lays it: one
/// for each edge of its run, or two where an edge runs past a closed
/// line's start.
fn seams_of(
    pocket: &Piece,
    host: &Piece,
    line: &InternalLine,
    fit: &Fit,
    confirmed: bool,
) -> Vec<Seam> {
    let stitching = stitching_edges(pocket);
    let (points, spans) = run(&stitching, fit.mouth);
    let length = polyline_length(&points);
    let total = polyline_length(&line_points(line));
    let closed = line_closed(line);
    let sewn = length - 2.0 * fit.trim;
    // Where a point of the run (trimmed) is along the line, unwrapped.
    let on_line = |u: f64| {
        let d = u * fit.scale;
        if fit.backwards {
            fit.start - d
        } else {
            fit.start + d
        }
    };
    let mut out = Vec::new();
    let mut add = |edge: usize, from: f64, to: f64, a: f64, b: f64| {
        if to - from < 0.5 || (b - a).abs() < 0.5 {
            return;
        }
        let full = polyline_length(&stitching[edge]);
        let whole = from < 0.05 && to > full - 0.05;
        let (lo, hi) = (a.min(b), a.max(b));
        out.push(Seam {
            id: String::new(),
            name: sentence(&pocket.name),
            a: SeamSide {
                from: (!whole).then_some(from.max(0.0)),
                to: (!whole).then_some(to.min(full)),
                mirror: fit.mirrored,
                ..SeamSide::whole(&pocket.id, &pocket.outline.edges[edge].id)
            },
            b: SeamSide::on_line(&host.id, &line.id, lo.max(0.0), hi.min(total)),
            reversed: fit.backwards,
            ease: Ease::Even,
            confirmed,
        });
    };
    for (edge, start, end) in spans {
        // Its part of the run, trimmed.
        let (u0, u1) = (
            start.max(fit.trim) - fit.trim,
            end.min(length - fit.trim) - fit.trim,
        );
        if u1 - u0 < 1.0 || u0 >= sewn {
            continue;
        }
        let (from, to) = (u0 + fit.trim - start, u1 + fit.trim - start);
        let (p0, p1) = (on_line(u0), on_line(u1));
        if !closed {
            add(edge, from, to, p0, p1);
            continue;
        }
        // Round a closed line, past its start: split where it passes it.
        let lap = p0.div_euclid(total) * total;
        let (q0, q1) = (p0 - lap, p1 - lap);
        let past = if q1 > total + 0.05 {
            Some(total)
        } else if q1 < -0.05 {
            Some(0.0)
        } else {
            None
        };
        match past {
            None => add(edge, from, to, q0, q1),
            Some(at) => {
                let share = (at - q0) / (q1 - q0);
                let split = from + share * (to - from);
                add(edge, from, split, q0, at);
                let wrap = if at == 0.0 { total } else { -total };
                add(edge, split, to, at + wrap, q1 + wrap);
            }
        }
    }
    out
}

/// How wide a piece is on its stitching line, its narrowest way across
/// (turned whichever way), mm.
fn sewn_size(piece: &Piece) -> f64 {
    let points: Vec<Point> = stitching_edges(piece).concat();
    (0..36)
        .map(|k| {
            let (s, c) = (f64::from(k) * 5.0).to_radians().sin_cos();
            let along: Vec<f64> = points.iter().map(|p| p.x * c + p.y * s).collect();
            let low = along.iter().copied().fold(f64::MAX, f64::min);
            let high = along.iter().copied().fold(f64::MIN, f64::max);
            (high - low).max(0.0)
        })
        .fold(f64::MAX, f64::min)
}

/// Cut from lining or interfacing only.
fn underneath(piece: &Piece) -> bool {
    !piece.cut.is_empty()
        && piece.cut.iter().all(|c| {
            let m = c.material.to_lowercase();
            m.contains("lining") || m.contains("interfacing")
        })
}

/// The piece and line a piece is sewn onto already, if it is sewn so (in
/// part: some of a pocket's seams sewn, the rest still to suggest) and by
/// nothing else.
fn sewn_onto<'a>(piece: &Piece, existing: &'a [Seam]) -> Option<(&'a Id, &'a Id)> {
    let mut onto = None;
    for s in existing.iter().filter(|s| s.touches(&piece.id)) {
        let (mine, other) = if s.a.piece == piece.id {
            (&s.a, &s.b)
        } else {
            (&s.b, &s.a)
        };
        let line = other
            .line
            .as_ref()
            .filter(|_| !mine.is_line() && mine.piece == piece.id)?;
        if other.piece == piece.id || onto.is_some_and(|o| o != (&other.piece, line)) {
            return None;
        }
        onto = Some((&other.piece, line));
    }
    onto
}

/// Pieces that might be pockets sewn on, and the piece and line each is
/// partly sewn onto already, if it is: not sewn otherwise, not facings,
/// linings, interfacing or strips, and not too small.
fn loose<'a, P: Borrow<Piece>>(
    pieces: &[P],
    existing: &'a [Seam],
) -> Vec<(usize, Option<(&'a Id, &'a Id)>)> {
    (0..pieces.len())
        .filter_map(|p| {
            let piece = pieces[p].borrow();
            let sewn = existing.iter().any(|s| s.touches(&piece.id));
            let onto = sewn_onto(piece, existing);
            (sewn_size(piece) >= SMALLEST
                && (!sewn || onto.is_some())
                && !inside(piece)
                && !strip(piece)
                && !underneath(piece))
            .then_some((p, onto))
        })
        .collect()
}

/// The lines drawn inside a piece that could be a pocket's placement, and
/// whether each is sewn onto already.
fn placements<'a>(piece: &'a Piece, existing: &[Seam]) -> Vec<(&'a InternalLine, bool)> {
    piece
        .lines
        .iter()
        .filter(|l| {
            matches!(
                l.kind,
                LineKind::Line | LineKind::Placement | LineKind::Stitch
            )
        })
        .filter(|l| polyline_length(&line_points(l)) >= SHORTEST)
        .map(|l| {
            let sewn = existing.iter().any(|s| {
                [&s.a, &s.b]
                    .iter()
                    .any(|side| side.piece == piece.id && side.line.as_ref() == Some(&l.id))
            });
            (l, sewn)
        })
        .collect()
}

/// How big a piece is: its outline's bounding box's area, mm².
fn area(piece: &Piece) -> f64 {
    piece.outline.bounds().area()
}

/// Pockets onto placements of their shape, drawn on other pieces, as
/// suggestions: each pocket's copies and each placement used once, closest
/// fits first.
pub fn suggest<P: Borrow<Piece>>(pieces: &[P], existing: &[Seam]) -> Vec<Suggestion> {
    // Each piece that might be a pocket, its stitching line edge by edge,
    // and how long each edge and all of it are.
    type Pocket<'a> = (
        usize,
        Option<(&'a Id, &'a Id)>,
        Vec<Vec<Point>>,
        Vec<f64>,
        f64,
    );
    let pockets: Vec<Pocket<'_>> = loose(pieces, existing)
        .into_iter()
        .map(|(p, onto)| {
            let stitching = stitching_edges(pieces[p].borrow());
            let lengths: Vec<f64> = stitching.iter().map(|e| polyline_length(e)).collect();
            let perimeter = lengths.iter().sum();
            (p, onto, stitching, lengths, perimeter)
        })
        .collect();
    // Each piece's placements, once: (line, its points, closed, length,
    // sewn onto already). An open one that runs straight on is no pocket's.
    type Placement = (usize, Vec<Point>, bool, f64, bool);
    let lines: Vec<Vec<Placement>> = pieces
        .iter()
        .map(Borrow::borrow)
        .map(|host| {
            if inside(host) || strip(host) || underneath(host) {
                return Vec::new();
            }
            placements(host, existing)
                .into_iter()
                .filter_map(|(line, sewn)| {
                    let l = host.lines.iter().position(|x| x.id == line.id)?;
                    let points = line_points(line);
                    let closed = line_closed(line);
                    let length = polyline_length(&points);
                    (closed || turns(&points)).then_some((l, points, closed, length, sewn))
                })
                .collect()
        })
        .collect();
    let near = |over: f64| (-FOUND.slack..=TURNED_DOWN + FOUND.slack).contains(&over);
    // Every fit good enough: (pocket, host, line, fit).
    let mut fits: Vec<(usize, usize, usize, Fit)> = Vec::new();
    for (p, onto, stitching, lengths, perimeter) in &pockets {
        let pocket = pieces[*p].borrow();
        for (h, host) in pieces.iter().map(Borrow::borrow).enumerate() {
            if h == *p || lines[h].is_empty() || area(pocket) > LARGEST * area(host) {
                continue;
            }
            for (l, points, closed, length, sewn) in &lines[h] {
                // A pocket sewn in part goes on where it is sewn; a
                // placement sewn onto takes no other.
                let its = onto
                    .is_some_and(|(piece, line)| *piece == host.id && **line == host.lines[*l].id);
                if onto.is_some() != its || (*sewn && !its) {
                    continue;
                }
                // About as long, all round or less one edge (its opening).
                let about = if *closed {
                    near((perimeter - length) / 2.0)
                } else {
                    lengths.iter().any(|e| near((perimeter - e - length) / 2.0))
                };
                if !about {
                    continue;
                }
                // Each copy apart: a pair's two pockets may go on two
                // placements, as drawn and mirrored.
                for copies in copies_of(pocket, existing) {
                    let placed = (points.as_slice(), *closed);
                    if let Some(fit) = best_fit((pocket, stitching), host, placed, FOUND, copies) {
                        fits.push((*p, h, *l, fit));
                    }
                }
            }
        }
    }
    // Closest first, pockets as drawn before mirror copies: a pair's
    // mirror copy goes where the one as drawn can't.
    fits.sort_by(|x, y| {
        x.3.mirrored
            .cmp(&y.3.mirrored)
            .then(x.3.mean.total_cmp(&y.3.mean))
    });
    // Each copy of a pocket sewn once: the one as drawn, and a pair's
    // mirror copy. A pocket sewn as drawn onto a panel worn on both sides
    // of the body is sewn on both, its copies one on each.
    let mut used_copies: Vec<(usize, bool)> = Vec::new();
    let mut used_lines: Vec<(usize, usize)> = Vec::new();
    let mut out = Vec::new();
    for (p, h, l, fit) in fits {
        let pocket = pieces[p].borrow();
        let host = pieces[h].borrow();
        let copies: Vec<(usize, bool)> = if fit.mirrored {
            vec![(p, true)]
        } else if both_sides(host) && pair(pocket) {
            vec![(p, false), (p, true)]
        } else {
            vec![(p, false)]
        };
        if used_lines.contains(&(h, l)) || copies.iter().any(|c| used_copies.contains(c)) {
            continue;
        }
        used_lines.push((h, l));
        used_copies.extend(copies);
        let confidence =
            (0.95 - 0.35 * ((fit.mean - 1.0) / (FOUND.mean - 1.0)).max(0.0)).clamp(0.5, 0.95);
        // Less the pocket's edges sewn already (the copy's, of a pair).
        let sewn_already = |seam: &Seam| {
            existing.iter().any(|s| {
                [&s.a, &s.b].iter().any(|side| {
                    side.sews_edge(&pocket.id, &seam.a.edge) && side.mirror == seam.a.mirror
                })
            })
        };
        out.extend(
            seams_of(pocket, host, &host.lines[l], &fit, false)
                .into_iter()
                .filter(|seam| !sewn_already(seam))
                .map(|seam| Suggestion {
                    seam,
                    confidence,
                    reasons: vec![Reason::OnPlacement],
                }),
        );
    }
    out
}

/// Sews `pocket` onto the line `line` inside `host`, as it best fits there:
/// its seams, confirmed; none if it is not about the line's shape. A pair's
/// copy sewn on already by `existing` seams (on another placement) leaves
/// the other.
pub fn sew_onto<P: Borrow<Piece>>(
    pieces: &[P],
    existing: &[Seam],
    pocket: &str,
    host: &str,
    line: &str,
) -> Option<Vec<Seam>> {
    let find = |id: &str| pieces.iter().map(Borrow::borrow).find(|p| p.id == id);
    let (pocket, host) = (find(pocket)?, find(host)?);
    if pocket.id == host.id {
        return None;
    }
    let line = host.lines.iter().find(|l| l.id == line)?;
    let copies: &[bool] = match (pair(pocket), sewn_copy(pocket, existing)) {
        (true, Some(used)) => {
            if used {
                &[false]
            } else {
                &[true]
            }
        }
        (true, None) => &[false, true],
        (false, _) => &[false],
    };
    let stitching = stitching_edges(pocket);
    let points = line_points(line);
    let fit = best_fit(
        (pocket, &stitching),
        host,
        (&points, line_closed(line)),
        BY_HAND,
        copies,
    )?;
    Some(seams_of(pocket, host, line, &fit, true))
}
