//! Placing a garment's panels round the body before it is sewn: panels
//! joined by upright seams (side seams, princess seams, a centre back) go
//! round the body as a ring, in the order their seams join them, wrapped
//! on an oval a little larger than the body there, the centre front in
//! front. A ring with shoulder seams (a bodice) hangs from above the
//! shoulders; another from the waist, or from the ring it is sewn under.
//! A ring sewn to cloth by a shoulder (a sleeve, its underarm seam closing
//! it) goes round that arm, its top over the shoulder.

use std::collections::HashMap;
use std::f64::consts::TAU;

use drapewright_body::measure::trunk_sections;
use drapewright_body::{Body, BodyModel, Region, measure};

use crate::garment::{Copy, Garment, Half};

/// What placing needs to know of the figure, mm (y up, z to the front).
#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub waist: f64,
    /// The top of the shoulders.
    pub shoulders: f64,
    /// Round the trunk at a height: its middle (x, z) and half its width
    /// and depth, from the body's slices from the shoulders to the hip.
    pub sections: Vec<Section>,
    /// The arms, left and right, for sleeves.
    pub arms: Vec<Arm>,
    /// How high the crotch is: where a bodysuit's crotch piece goes under.
    pub crotch: f64,
    /// Where the body is measured round (bust, waist, hip), and how big it
    /// is there.
    pub levels: Vec<Level>,
}

/// A height the body is measured round.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Level {
    /// `bust`, `waist` or `hip`.
    pub name: &'static str,
    pub y: f64,
    /// The body's girth there, mm.
    pub girth: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Section {
    pub y: f64,
    pub middle: [f64; 2],
    pub half: [f64; 2],
}

impl Figure {
    /// The avatar's figure: its waist and shoulders where it is measured,
    /// and its trunk's sections every 20 mm from 400 mm below the hip to
    /// the shoulders.
    pub fn of(model: &BodyModel, body: &Body) -> Figure {
        let (measured, marks) = measure(model, body);
        let shoulders = marks.points["shoulderPoint"][1];
        let hip = marks.levels["hip"];
        let sections = trunk_sections(model, body, hip - 400.0, shoulders, 20.0)
            .into_iter()
            .map(|(y, middle, half)| Section { y, middle, half })
            .collect();
        let arms = [
            (
                ["l-shoulder", "l-elbow", "l-hand"],
                [Region::ArmLeft, Region::ShoulderLeft],
            ),
            (
                ["r-shoulder", "r-elbow", "r-hand"],
                [Region::ArmRight, Region::ShoulderRight],
            ),
        ]
        .into_iter()
        .filter_map(|(joints, regions)| Arm::of(model, body, joints, regions))
        .collect();
        let levels = ["bust", "waist", "hip"]
            .into_iter()
            .filter_map(|name| {
                Some(Level {
                    name,
                    y: *marks.levels.get(name)?,
                    girth: *measured.get(name)?,
                })
            })
            .collect();
        Figure {
            waist: marks.levels["waist"],
            shoulders,
            sections,
            arms,
            crotch: marks.points["crotch"][1],
            levels,
        }
    }

    /// The trunk's middle at height `y` (x, z).
    pub fn middle(&self, y: f64) -> [f64; 2] {
        self.smoothed(y).middle
    }

    /// The trunk's section at height `y`, averaged over 60 mm above and
    /// below: its shape changing quickly from one height to the next would
    /// twist the cloth placed round it.
    fn smoothed(&self, y: f64) -> Section {
        let near: Vec<&Section> = self
            .sections
            .iter()
            .filter(|s| (s.y - y).abs() <= 60.0)
            .collect();
        if near.is_empty() {
            return self.section(y);
        }
        let n = near.len() as f64;
        let mean = |f: &dyn Fn(&Section) -> f64| near.iter().map(|s| f(s)).sum::<f64>() / n;
        Section {
            y,
            middle: [mean(&|s| s.middle[0]), mean(&|s| s.middle[1])],
            half: [mean(&|s| s.half[0]), mean(&|s| s.half[1])],
        }
    }

    /// The trunk's section nearest height `y`.
    pub(crate) fn section(&self, y: f64) -> Section {
        self.sections
            .iter()
            .copied()
            .min_by(|a, b| (a.y - y).abs().total_cmp(&(b.y - y).abs()))
            .unwrap_or(Section {
                y,
                middle: [0.0, 0.0],
                half: [170.0, 120.0],
            })
    }
}

type V3 = [f64; 3];

fn sub3(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn add3(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale3(a: V3, k: f64) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}

fn dot3(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross3(a: V3, b: V3) -> V3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit3(a: V3) -> V3 {
    scale3(a, 1.0 / dot3(a, a).sqrt().max(1e-12))
}

fn lerp3(a: V3, b: V3, t: f64) -> V3 {
    add3(a, scale3(sub3(b, a), t))
}

/// Points down an arm are this far apart, mm.
const ARM_STEP: f64 = 10.0;
/// The line down an arm starts this far inside its shoulder joint, mm: a
/// sleeve's cap reaches over the shoulder.
const ARM_BEFORE: f64 = 100.0;

/// An arm, for a sleeve to go round: a smooth line down its middle, from
/// inside the shoulder through the elbow to the wrist, a point every
/// `ARM_STEP` mm; at each, the way to the top of the arm and to its side
/// (turning as little as they can down the arm), and how far the arm
/// reaches from the line.
#[derive(Clone, Debug, PartialEq)]
pub struct Arm {
    pub line: Vec<V3>,
    pub up: Vec<V3>,
    pub side: Vec<V3>,
    pub radius: Vec<f64>,
}

impl Arm {
    /// The arm through joints `[shoulder, elbow, wrist]`, its surface the
    /// body's in `regions`.
    fn of(model: &BodyModel, body: &Body, joints: [&str; 3], regions: [Region; 2]) -> Option<Arm> {
        let [s, e, w] = [
            model.joint(body, joints[0])?,
            model.joint(body, joints[1])?,
            model.joint(body, joints[2])?,
        ];
        // Through the joints smoothly (Catmull-Rom), finely, and on a
        // little past each end.
        let hermite = |a: V3, b: V3, ma: V3, mb: V3, t: f64| {
            let (t2, t3) = (t * t, t * t * t);
            let mut p = scale3(a, 2.0 * t3 - 3.0 * t2 + 1.0);
            p = add3(p, scale3(ma, t3 - 2.0 * t2 + t));
            p = add3(p, scale3(b, -2.0 * t3 + 3.0 * t2));
            add3(p, scale3(mb, t3 - t2))
        };
        let middle = scale3(sub3(w, s), 0.5);
        let mut fine = Vec::new();
        for (a, b, ma, mb) in [(s, e, sub3(e, s), middle), (e, w, middle, sub3(w, e))] {
            for k in 0..200 {
                fine.push(hermite(a, b, ma, mb, f64::from(k) / 200.0));
            }
        }
        fine.push(w);
        let first = unit3(sub3(fine[1], fine[0]));
        let last = unit3(sub3(fine[fine.len() - 1], fine[fine.len() - 2]));
        fine.insert(0, add3(s, scale3(first, -ARM_BEFORE)));
        fine.push(add3(w, scale3(last, 50.0)));
        // A point every ARM_STEP mm along it.
        let mut line = vec![fine[0]];
        let mut walked = 0.0;
        for pair in fine.windows(2) {
            let length = dot3(sub3(pair[1], pair[0]), sub3(pair[1], pair[0])).sqrt();
            while walked + length >= ARM_STEP * line.len() as f64 && length > 0.0 {
                let t = (ARM_STEP * line.len() as f64 - walked) / length;
                line.push(lerp3(pair[0], pair[1], t));
            }
            walked += length;
        }
        if line.len() < 3 {
            return None;
        }
        let n = line.len();
        let tangent = |k: usize| unit3(sub3(line[(k + 1).min(n - 1)], line[k.saturating_sub(1)]));
        let mut up = Vec::with_capacity(n);
        let mut side = Vec::with_capacity(n);
        let mut last_up = [0.0, 1.0, 0.0];
        for k in 0..n {
            let t = tangent(k);
            last_up = unit3(sub3(last_up, scale3(t, dot3(last_up, t))));
            up.push(last_up);
            side.push(cross3(t, last_up));
        }
        // How far the arm's surface reaches from the line, near each point.
        let mut radius = vec![0.0f64; n];
        for (i, p) in body.vertices[..model.asset.shown as usize]
            .iter()
            .enumerate()
        {
            if !model.region(i).is_some_and(|r| regions.contains(&r)) {
                continue;
            }
            let k = (0..n)
                .min_by(|&a, &b| {
                    let (da, db) = (sub3(*p, line[a]), sub3(*p, line[b]));
                    dot3(da, da).total_cmp(&dot3(db, db))
                })
                .expect("a line");
            let d = sub3(*p, line[k]);
            let across = sub3(d, scale3(tangent(k), dot3(d, tangent(k))));
            radius[k] = radius[k].max(dot3(across, across).sqrt());
        }
        // Where no surface was nearest, as near the line's nearest that was.
        for k in 1..n {
            if radius[k] == 0.0 {
                radius[k] = radius[k - 1];
            }
        }
        for k in (0..n - 1).rev() {
            if radius[k] == 0.0 {
                radius[k] = radius[k + 1];
            }
        }
        Some(Arm {
            line,
            up,
            side,
            radius,
        })
    }

    /// `along` mm down the line: the point, the ways to the top of the arm
    /// and its side, and the arm's radius.
    fn at(&self, along: f64) -> (V3, V3, V3, f64) {
        let last = self.line.len() - 1;
        let f = (along / ARM_STEP).clamp(0.0, last as f64);
        let k = (f.floor() as usize).min(last - 1);
        let t = f - k as f64;
        (
            lerp3(self.line[k], self.line[k + 1], t),
            unit3(lerp3(self.up[k], self.up[k + 1], t)),
            unit3(lerp3(self.side[k], self.side[k + 1], t)),
            self.radius[k] + (self.radius[k + 1] - self.radius[k]) * t,
        )
    }
}

/// A garment no larger than the body is placed this much larger round,
/// to start clear of it (stretched, it draws in onto the body).
const CLEAR: f64 = 1.04;

/// An oval, measured along its rim: x = a·sin t, z = b·cos t.
struct Oval {
    a: f64,
    b: f64,
    /// Arc length at evenly spaced angles round the rim.
    table: Vec<f64>,
}

impl Oval {
    fn new(a: f64, b: f64) -> Oval {
        let n = 720;
        let mut table = vec![0.0];
        for k in 1..=n {
            let (t0, t1) = (TAU * (k - 1) as f64 / n as f64, TAU * k as f64 / n as f64);
            let (p0, p1) = ([a * t0.sin(), b * t0.cos()], [a * t1.sin(), b * t1.cos()]);
            let last = table[table.len() - 1];
            table.push(last + (p1[0] - p0[0]).hypot(p1[1] - p0[1]));
        }
        Oval { a, b, table }
    }

    fn perimeter(&self) -> f64 {
        self.table[self.table.len() - 1]
    }

    /// The point `s` mm round the rim from the front, towards the figure's
    /// left (+x), as (x, z).
    fn at(&self, s: f64) -> [f64; 2] {
        let s = s.rem_euclid(self.perimeter());
        let n = self.table.len() - 1;
        let k = self.table.partition_point(|&v| v < s).clamp(1, n);
        let (s0, s1) = (self.table[k - 1], self.table[k]);
        let f = if s1 > s0 { (s - s0) / (s1 - s0) } else { 0.0 };
        let t = TAU * ((k - 1) as f64 + f) / n as f64;
        [self.a * t.sin(), self.b * t.cos()]
    }

    /// An oval as round as the body's section, as long round as the
    /// garment there (`perimeter`) if that is larger, and at least `CLEAR`
    /// times the body's.
    fn round(section: Section, perimeter: f64) -> Oval {
        // Garments hang rounder than the body's flattest sections (the two
        // thighs side by side): a flat oval turns too sharply at its sides.
        let (a, b) = (section.half[0], section.half[1].max(0.65 * section.half[0]));
        let body = Oval::new(a, b);
        let scale = (perimeter / body.perimeter()).max(CLEAR);
        Oval::new(a * scale, b * scale)
    }
}

fn panel_box(g: &Garment, p: usize) -> ([f64; 2], [f64; 2]) {
    let mut low = [f64::MAX; 2];
    let mut high = [f64::MIN; 2];
    for q in &g.panels[p].mesh.points {
        for k in 0..2 {
            low[k] = low[k].min(q[k]);
            high[k] = high[k].max(q[k]);
        }
    }
    (low, high)
}

/// An edge's run flat: (mean u, mean v, how much it rises over its run).
fn edge_shape(g: &Garment, p: usize, edge: &str, half: Half) -> Option<(f64, f64, f64, f64)> {
    let e = g.panels[p].edge(edge, half)?;
    let pts: Vec<[f64; 2]> = e
        .points
        .iter()
        .map(|&i| g.panels[p].mesh.points[i as usize])
        .collect();
    let n = pts.len() as f64;
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    Some((
        pts.iter().map(|q| q[0]).sum::<f64>() / n,
        pts.iter().map(|q| q[1]).sum::<f64>() / n,
        (last[0] - first[0]).abs(),
        (last[1] - first[1]).abs(),
    ))
}

/// Where edge `e` of panel `p` is across at height `v`: read off the edge,
/// or its nearer end beyond it.
fn edge_u(g: &Garment, p: usize, e: usize, v: f64) -> f64 {
    let panel = &g.panels[p];
    let pts: Vec<[f64; 2]> = panel.edges[e]
        .points
        .iter()
        .map(|&i| panel.mesh.points[i as usize])
        .collect();
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        if (a[1] - v) * (b[1] - v) <= 0.0 && (a[1] - b[1]).abs() > 1e-12 {
            return a[0] + (v - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
        }
    }
    // Beyond the edge: its end nearer that height.
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    if (first[1] - v).abs() < (last[1] - v).abs() {
        first[0]
    } else {
        last[0]
    }
}

/// A panel's outline flat, once round.
fn outline(g: &Garment, p: usize) -> Vec<[f64; 2]> {
    let panel = &g.panels[p];
    let mut out = Vec::new();
    for chain in &panel.mesh.chains {
        for &i in &chain[..chain.len() - 1] {
            out.push(panel.mesh.points[i as usize]);
        }
    }
    out
}

/// Where a horizontal line at height `v` crosses an outline: its leftmost
/// and rightmost crossings.
fn span(outline: &[[f64; 2]], v: f64) -> Option<(f64, f64)> {
    let mut low = f64::MAX;
    let mut high = f64::MIN;
    for i in 0..outline.len() {
        let (a, b) = (outline[i], outline[(i + 1) % outline.len()]);
        if (a[1] - v) * (b[1] - v) <= 0.0 && (a[1] - b[1]).abs() > 1e-12 {
            let u = a[0] + (v - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            low = low.min(u);
            high = high.max(u);
        } else if (a[1] - v).abs() < 1e-12 {
            low = low.min(a[0]);
            high = high.max(a[0]);
        }
    }
    (low < high).then_some((low, high))
}

/// Each panel of a ring's span across, at one height (or none).
type Spans = Vec<Option<(f64, f64)>>;

/// A ring at one height: its panels' spans, and the oval they go round.
type Row = (Spans, Oval);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// Where each panel's points start, mm.
pub fn place(g: &Garment, figure: &Figure) -> Vec<Vec<[f64; 3]>> {
    match bands_turned(g) {
        Some(turned) => place_upright(&turned, figure),
        None => place_upright(g, figure),
    }
}

/// Whether a height `v` on panel `p` is in its top or bottom quarter.
fn near_end(g: &Garment, p: usize, v: f64) -> (bool, bool) {
    let (low, high) = panel_box(g, p);
    let quarter = 0.25 * (high[1] - low[1]);
    (v > high[1] - quarter, v < low[1] + quarter)
}

/// The garment with each band turned to hang from what it is sewn to, or
/// to stand on it: pattern CAD draws a cuff or a waistband either way up.
/// A band is sewn by one level edge and is not joined side by side to
/// anything: sewn to another's hem, that edge goes at its top; to another's
/// top, at its bottom. None when no band needs turning.
fn bands_turned(g: &Garment) -> Option<Garment> {
    let n = g.panels.len();
    // Each panel's links: (its edge, upright, its edge's mean height, the
    // other panel, the other's edge's mean height).
    type Sewn = (String, bool, f64, usize, f64);
    let mut links: Vec<Vec<Sewn>> = vec![Vec::new(); n];
    for link in &g.links {
        let (pa, ea, ha) = (link.a.0, &link.a.1, link.a.2);
        let (pb, eb, hb) = (link.b.0, &link.b.1, link.b.2);
        let (Some(sa), Some(sb)) = (edge_shape(g, pa, ea, ha), edge_shape(g, pb, eb, hb)) else {
            continue;
        };
        let upright = sa.3 > sa.2 && sb.3 > sb.2;
        links[pa].push((format!("{ea}{ha:?}"), upright, sa.1, pb, sb.1));
        links[pb].push((format!("{eb}{hb:?}"), upright, sb.1, pa, sa.1));
    }
    let turn: Vec<bool> = (0..n)
        .map(|p| {
            let mine = &links[p];
            let Some(first) = mine.first() else {
                return false;
            };
            if mine.iter().any(|l| l.1 || l.0 != first.0 || l.3 == p) {
                return false;
            }
            let (top, bottom) = near_end(g, p, first.2);
            let (under, over) = mine.iter().fold((0, 0), |(u, o), l| {
                let (other_top, other_bottom) = near_end(g, l.3, l.4);
                (u + usize::from(other_bottom), o + usize::from(other_top))
            });
            (under > over && bottom) || (over > under && top)
        })
        .collect();
    if !turn.contains(&true) {
        return None;
    }
    let mut turned = g.clone();
    for (p, panel) in turned.panels.iter_mut().enumerate() {
        if !turn[p] {
            continue;
        }
        let (low, high) = panel_box(g, p);
        let (cx, cy) = (low[0] + high[0], low[1] + high[1]);
        for q in &mut panel.mesh.points {
            *q = [cx - q[0], cy - q[1]];
        }
        panel.grain = [-panel.grain[0], -panel.grain[1]];
    }
    Some(turned)
}

/// A panel sewn between cloth already placed (a yoke, between a front and
/// a back): each of its points stitched to placed cloth goes where that
/// is, and the rest span the gap between, each the mean of its neighbours,
/// as a soap film spans a wire frame. `partners`: its points and where what
/// they are sewn to is.
fn spanned(g: &Garment, p: usize, partners: &[(usize, u32, [f64; 3])]) -> Vec<[f64; 3]> {
    let panel = &g.panels[p];
    let n = panel.mesh.points.len();
    let mut neighbours: Vec<Vec<usize>> = vec![Vec::new(); n];
    for t in &panel.mesh.triangles {
        for k in 0..3 {
            let (a, b) = (t[k] as usize, t[(k + 1) % 3] as usize);
            if !neighbours[a].contains(&b) {
                neighbours[a].push(b);
            }
            if !neighbours[b].contains(&a) {
                neighbours[b].push(a);
            }
        }
    }
    let mut sums = vec![([0.0; 3], 0.0f64); n];
    for &(q, i, at) in partners {
        if q == p {
            let s = &mut sums[i as usize];
            s.0 = add3(s.0, at);
            s.1 += 1.0;
        }
    }
    let fixed: Vec<Option<[f64; 3]>> = sums
        .iter()
        .map(|(sum, count)| (*count > 0.0).then(|| scale3(*sum, 1.0 / count)))
        .collect();
    let held: Vec<[f64; 3]> = fixed.iter().flatten().copied().collect();
    let middle = held.iter().fold([0.0; 3], |m, &x| {
        add3(m, scale3(x, 1.0 / held.len().max(1) as f64))
    });
    let mut x: Vec<[f64; 3]> = fixed.iter().map(|f| f.unwrap_or(middle)).collect();
    for _ in 0..400 {
        for i in 0..n {
            if fixed[i].is_some() || neighbours[i].is_empty() {
                continue;
            }
            let k = neighbours[i].len() as f64;
            x[i] = neighbours[i]
                .iter()
                .fold([0.0; 3], |m, &j| add3(m, scale3(x[j], 1.0 / k)));
        }
    }
    x
}

/// Whether a panel goes between the legs (a bodysuit's crotch, a gusset),
/// by its name.
fn between_legs(name: &str) -> bool {
    let name = name.to_lowercase();
    name.contains("crotch") || name.contains("gusset")
}

/// How far below the crotch a crotch piece starts, and how far above it
/// its ends reach at the front and back, mm.
const UNDER: f64 = 15.0;
const UP: f64 = 70.0;

/// A crotch piece's points, curved under the crotch from the front to the
/// back between the legs: its end sewn to a front at the front, to a back
/// at the back. A crotch in two pieces, a front and a back, has the front
/// half of the curve and the back half. Sewn, it draws up to them.
fn under_the_crotch(
    g: &Garment,
    p: usize,
    placed: &[Vec<[f64; 3]>],
    figure: &Figure,
) -> Vec<[f64; 3]> {
    let panel = &g.panels[p];
    let points = &panel.mesh.points;
    if points.is_empty() {
        return Vec::new();
    }
    let n = points.len() as f64;
    let centre = points
        .iter()
        .fold([0.0, 0.0], |c, q| [c[0] + q[0] / n, c[1] + q[1] / n]);
    // Which way on the panel is towards the front: to its points sewn to a
    // front (or away from those sewn to a back), else up the panel.
    let mut towards = [0.0, 0.0];
    for &[a, b] in &g.stitches {
        for ((q, i), (o, _)) in [(a, b), (b, a)] {
            if q != p || o == p || placed[o].is_empty() {
                continue;
            }
            let name = g.panels[o].name.to_lowercase();
            let sign = if name.contains("front") {
                1.0
            } else if name.contains("back") {
                -1.0
            } else {
                continue;
            };
            let at = points[i as usize];
            towards[0] += sign * (at[0] - centre[0]);
            towards[1] += sign * (at[1] - centre[1]);
        }
    }
    let length = towards[0].hypot(towards[1]);
    let front = if length > 1e-9 {
        [towards[0] / length, towards[1] / length]
    } else {
        [0.0, 1.0]
    };
    let across = [front[1], -front[0]];
    // Along the curve from the front's top (0) to the back's (1): this
    // piece's share of it.
    let name = panel.name.to_lowercase();
    let (from, to) = if name.contains("front") {
        (0.0, 0.5)
    } else if name.contains("back") {
        (0.5, 1.0)
    } else {
        (0.0, 1.0)
    };
    let along: Vec<f64> = points
        .iter()
        .map(|q| (q[0] - centre[0]) * front[0] + (q[1] - centre[1]) * front[1])
        .collect();
    let (low, high) = along
        .iter()
        .fold((f64::MAX, f64::MIN), |(l, h), &v| (l.min(v), h.max(v)));
    // The curve, in the body's middle: a half ellipse under the crotch,
    // from just in front of the body to just behind it.
    let top = figure.crotch + UP;
    let section = figure.section(top);
    let (middle, depth) = (section.middle, section.half[1] * CLEAR);
    let bottom = figure.crotch - UNDER;
    let curve = |t: f64| {
        // From the front (angle 0) under (π/2) to the back (π).
        let angle = t * std::f64::consts::PI;
        [
            top - (top - bottom) * angle.sin(),
            middle[1] + depth * angle.cos(),
        ]
    };
    points
        .iter()
        .zip(&along)
        .map(|(q, &v)| {
            let share = if high > low {
                (high - v) / (high - low)
            } else {
                0.5
            };
            let [y, z] = curve(from + (to - from) * share);
            let x = middle[0] + (q[0] - centre[0]) * across[0] + (q[1] - centre[1]) * across[1];
            [x, y, z]
        })
        .collect()
}

/// Who is beside whom in a garment: its panels' boxes, the panels joined
/// side by side to each (this side, the other panel, its side, this
/// panel's edge there), those joined above and below (each with its edge's
/// mean height there), and its rings: panels joined side by side in order
/// round the body (each turned over or not), whether each closes, and each
/// panel's ring.
struct Traced {
    boxes: Vec<([f64; 2], [f64; 2])>,
    beside: Vec<Vec<(Side, usize, Side, usize)>>,
    level: Vec<(usize, f64, usize, f64)>,
    rings: Vec<Vec<(usize, bool)>>,
    closed: Vec<bool>,
    ring_of: Vec<usize>,
    names: Vec<String>,
}

fn trace(g: &Garment) -> Traced {
    let n = g.panels.len();
    let boxes: Vec<_> = (0..n).map(|p| panel_box(g, p)).collect();
    // Upright links join panels side by side; level ones join them above
    // and below (or over the shoulders).
    // Side by side: (this side, the other panel, its side, this panel's
    // edge there).
    let mut beside: Vec<Vec<(Side, usize, Side, usize)>> = vec![Vec::new(); n];
    let edge_index = |p: usize, id: &str, half: Half| {
        g.panels[p]
            .edges
            .iter()
            .position(|e| e.edge == id && e.half == half)
    };
    // Level links: (panel, its edge's mean height there) for each side.
    let mut level: Vec<(usize, f64, usize, f64)> = Vec::new();
    for link in &g.links {
        let (pa, ea, ha) = (&link.a.0, &link.a.1, link.a.2);
        let (pb, eb, hb) = (&link.b.0, &link.b.1, link.b.2);
        let (Some(sa), Some(sb)) = (edge_shape(g, *pa, ea, ha), edge_shape(g, *pb, eb, hb)) else {
            continue;
        };
        let side = |p: usize, u: f64| {
            let (low, high) = boxes[p];
            if u < (low[0] + high[0]) / 2.0 {
                Side::Left
            } else {
                Side::Right
            }
        };
        let upright = sa.3 > sa.2 && sb.3 > sb.2;
        if upright {
            if (pa != pb || side(*pa, sa.0) != side(*pb, sb.0))
                && let (Some(ia), Some(ib)) = (edge_index(*pa, ea, ha), edge_index(*pb, eb, hb))
            {
                beside[*pa].push((side(*pa, sa.0), *pb, side(*pb, sb.0), ia));
                beside[*pb].push((side(*pb, sb.0), *pa, side(*pa, sa.0), ib));
            }
        } else {
            level.push((*pa, sa.1, *pb, sb.1));
        }
    }
    // Rings: panels joined side by side, in order round the body, each
    // turned over if its seams run the other way.
    let mut ring_of = vec![usize::MAX; n];
    let mut rings: Vec<Vec<(usize, bool)>> = Vec::new();
    // Each ring joined all the way round (a skirt), or open (a blouse
    // buttoned down the front).
    let mut closed: Vec<bool> = Vec::new();
    let names: Vec<String> = g.panels.iter().map(|p| p.name.to_lowercase()).collect();
    let starts: Vec<usize> = {
        // Begin with a front cut on the fold, else any front, else anything.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by_key(|&p| {
            let front = names[p].contains("front");
            match (g.panels[p].copy, front) {
                (Copy::Unfolded, true) => 0,
                (Copy::Unfolded, false) => 1,
                (_, true) => 2,
                _ => 3,
            }
        });
        order
    };
    for &start in &starts {
        if ring_of[start] != usize::MAX {
            continue;
        }
        let r = rings.len();
        ring_of[start] = r;
        let mut ring = vec![(start, false)];
        let mut round = false;
        // Rightwards from the start, then leftwards for an open ring.
        for going in [Side::Right, Side::Left] {
            if round {
                break;
            }
            let (mut at, mut out) = (start, going);
            loop {
                let next = beside[at]
                    .iter()
                    .find(|(s, q, _, _)| *s == out && (ring_of[*q] == usize::MAX || *q == start));
                let Some(&(_, q, enters, _)) = next else {
                    break;
                };
                if q == start {
                    round = true;
                    break;
                }
                ring_of[q] = r;
                // Entered on its right going right: turned over.
                let turned = (enters == Side::Right) == (going == Side::Right);
                if going == Side::Right {
                    ring.push((q, turned));
                } else {
                    ring.insert(0, (q, turned));
                }
                out = if enters == Side::Left {
                    Side::Right
                } else {
                    Side::Left
                };
                at = q;
            }
        }
        rings.push(ring);
        closed.push(round);
    }
    Traced {
        boxes,
        beside,
        level,
        rings,
        closed,
        ring_of,
        names,
    }
}

/// Whether a garment goes only half way round the body: its largest ring
/// runs from a front to a back without closing (a pattern drawn for one
/// side, its pieces cut once where they should be cut as pairs).
pub fn half_round(g: &Garment) -> bool {
    let t = trace(g);
    let Some(r) = (0..t.rings.len()).max_by_key(|&r| t.rings[r].len()) else {
        return false;
    };
    let ring = &t.rings[r];
    let (first, last) = (&t.names[ring[0].0], &t.names[ring[ring.len() - 1].0]);
    let front_and_back = (first.contains("front") && last.contains("back"))
        || (first.contains("back") && last.contains("front"));
    ring.len() >= 2 && !t.closed[r] && front_and_back
}

/// Where each panel's points start, the panels upright (see `place`).
fn place_upright(g: &Garment, figure: &Figure) -> Vec<Vec<[f64; 3]>> {
    let Traced {
        boxes,
        beside,
        level,
        rings,
        closed,
        ring_of,
        names,
    } = trace(g);
    let n = g.panels.len();
    // Bridges: a panel joined to nothing side by side but sewn by two or
    // more of its edges to others (a yoke, between a front and a back over
    // the shoulder) goes between them once they are placed.
    let mut sewn_by: Vec<Vec<(String, usize, f64)>> = vec![Vec::new(); n];
    for link in &g.links {
        let (pa, pb) = (link.a.0, link.b.0);
        if pa == pb {
            continue;
        }
        let (Some(sa), Some(sb)) = (
            edge_shape(g, pa, &link.a.1, link.a.2),
            edge_shape(g, pb, &link.b.1, link.b.2),
        ) else {
            continue;
        };
        sewn_by[pa].push((format!("{}{:?}", link.a.1, link.a.2), pb, sb.1));
        sewn_by[pb].push((format!("{}{:?}", link.b.1, link.b.2), pa, sa.1));
    }
    let bridge: Vec<bool> = (0..n)
        .map(|p| {
            beside[p].is_empty() && {
                let mut edges: Vec<&String> = sewn_by[p].iter().map(|s| &s.0).collect();
                edges.sort();
                edges.dedup();
                edges.len() >= 2
            }
        })
        .collect();
    // Near the top of two panels of a ring: over the shoulders (two rings
    // sewn at their tops are a sleeve in an armhole). One's top to the
    // other's bottom: hung under it.
    let near_top = |p: usize, v: f64| v > boxes[p].1[1] - 0.25 * (boxes[p].1[1] - boxes[p].0[1]);
    let near_bottom = |p: usize, v: f64| v < boxes[p].0[1] + 0.25 * (boxes[p].1[1] - boxes[p].0[1]);
    let mut shoulders = vec![false; n];
    let mut under: Vec<Option<usize>> = vec![None; n];
    for &(pa, va, pb, vb) in &level {
        if bridge[pa] || bridge[pb] {
            continue;
        }
        if near_top(pa, va) && near_top(pb, vb) {
            if ring_of[pa] == ring_of[pb] {
                shoulders[pa] = true;
                shoulders[pb] = true;
            }
        } else if near_top(pa, va) && near_bottom(pb, vb) {
            under[pa] = Some(pb);
        } else if near_top(pb, vb) && near_bottom(pa, va) {
            under[pb] = Some(pa);
        }
    }
    // Two panels of a ring sewn near their tops to the same bridge: over
    // the shoulders too, the bridge between them.
    for p in (0..n).filter(|&p| bridge[p]) {
        let tops: Vec<usize> = sewn_by[p]
            .iter()
            .filter(|&&(_, q, v)| !bridge[q] && near_top(q, v))
            .map(|&(_, q, _)| q)
            .collect();
        for &a in &tops {
            if tops.iter().any(|&b| b != a && ring_of[b] == ring_of[a]) {
                shoulders[a] = true;
            }
        }
    }
    // Each ring's height: from above the shoulders, under another ring, or
    // from the waist.
    let tall = |r: &[(usize, bool)]| {
        r.iter()
            .map(|&(p, _)| boxes[p].1[1] - boxes[p].0[1])
            .fold(0.0, f64::max)
    };
    let mut tops: Vec<Option<f64>> = vec![None; rings.len()];
    for (r, ring) in rings.iter().enumerate() {
        if ring.iter().any(|&(p, _)| shoulders[p]) {
            tops[r] = Some(figure.shoulders + 30.0);
        }
    }
    for _ in 0..rings.len() {
        for (r, ring) in rings.iter().enumerate() {
            if tops[r].is_some() {
                continue;
            }
            if let Some(over) = ring.iter().find_map(|&(p, _)| under[p])
                && let Some(top) = tops[ring_of[over]]
            {
                tops[r] = Some(top - tall(&rings[ring_of[over]]));
            }
        }
    }
    // Each ring wrapped row by row: at each height, every panel as wide as
    // it is there, side by side, so that seams meet at every height (an
    // A-line skirt goes on as a cone, not a cylinder).
    let outlines: Vec<Vec<[f64; 2]>> = (0..n).map(|p| outline(g, p)).collect();
    // A panel's width at height v: between its seams to either side, read
    // off those edges (so that the two sides of a seam meet at every
    // height); where a side has no seam, the outline's.
    let between_seams = |p: usize, v: f64| -> Option<(f64, f64)> {
        let outline_span = span(&outlines[p], v);
        let seam = |side: Side| {
            beside[p]
                .iter()
                .find(|b| b.0 == side)
                .map(|b| edge_u(g, p, b.3, v))
        };
        let left = seam(Side::Left).or(outline_span.map(|s| s.0))?;
        let right = seam(Side::Right).or(outline_span.map(|s| s.1))?;
        (left < right).then_some((left, right))
    };
    // A ring round the trunk, hanging from `top`: each panel's points.
    // How far up each panel goes from the panel's flat y, for a ring
    // whose top is at `top`: each by its waist line where every panel of a
    // ring has one and it is not hung from something placed (a corset at
    // the waist, bust and hips where they go), else by its top.
    let raise = |ring: &[(usize, bool)], top: f64, by_waist: bool| -> Vec<f64> {
        let waists: Option<Vec<f64>> = ring.iter().map(|&(p, _)| g.panels[p].waist).collect();
        match waists {
            Some(waists) if by_waist => waists.iter().map(|w| figure.waist - w).collect(),
            _ => ring.iter().map(|&(p, _)| top - boxes[p].1[1]).collect(),
        }
    };
    let wrap_trunk =
        |ring: &[(usize, bool)], round: bool, top: f64, by_waist: bool| -> Vec<Vec<[f64; 3]>> {
            let raised = raise(ring, top, by_waist);
            // In front: the seam between a front's two halves, where they
            // are sewn at its centre (a pair's copies side by side), else a
            // front's middle (cut on the fold, say); failing a front, a
            // back's centre or middle goes behind; failing both, the first
            // panel's middle in front. A ring open between two fronts
            // (buttoned down the front) meets itself in front.
            let n = ring.len();
            let named = |k: usize, word: &str| names[ring[k].0].contains(word);
            // Whether the seam before `ring[k]` joins one piece's halves.
            let halves = |k: usize| {
                n >= 2
                    && (k > 0 || round)
                    && g.panels[ring[(k + n - 1) % n].0].piece == g.panels[ring[k].0].piece
            };
            let centre = |word: &str| {
                (0..n)
                    .find(|&k| halves(k) && named(k, word))
                    .map(|k| (k, true))
                    .or_else(|| (0..n).find(|&k| named(k, word)).map(|k| (k, false)))
            };
            let ((front, at_seam), behind) = match (centre("front"), centre("back")) {
                (Some(c), _) => (c, false),
                (None, Some(c)) => (c, true),
                (None, None) => ((0, false), false),
            };
            let open_front = !round
                && ring.len() >= 2
                && names[ring[0].0].contains("front")
                && names[ring[ring.len() - 1].0].contains("front");
            // The ring at each height (every 5 mm): each panel's span there (or
            // none), and the oval it goes round.
            let key = |y: f64| (y / 5.0).round() as i64;
            let mut rows: HashMap<i64, Row> = HashMap::new();
            for (k, &(p, _)) in ring.iter().enumerate() {
                for q in &g.panels[p].mesh.points {
                    let y = q[1] + raised[k];
                    rows.entry(key(y)).or_insert_with(|| {
                        let spans: Spans = ring
                            .iter()
                            .enumerate()
                            .map(|(j, &(o, _))| between_seams(o, y - raised[j]))
                            .collect();
                        let around: f64 = spans.iter().flatten().map(|(a, b)| b - a).sum();
                        (spans, Oval::round(figure.smoothed(y), around))
                    });
                }
            }
            let mut placed = Vec::new();
            for (k, &(p, turned)) in ring.iter().enumerate() {
                let mut points = Vec::new();
                for q in &g.panels[p].mesh.points {
                    let y = q[1] + raised[k];
                    let (spans, oval) = &rows[&key(y)];
                    let perimeter = oval.perimeter();
                    let width = |j: usize| spans[j].map_or(0.0, |(a, b)| b - a);
                    let offset: f64 = (0..k).map(width).sum();
                    let (left, right) = spans[k].unwrap_or((q[0], q[0]));
                    let u = if turned { right - q[0] } else { q[0] - left };
                    let middle: f64 = if open_front {
                        0.0
                    } else if at_seam {
                        (0..front).map(width).sum()
                    } else {
                        (0..front).map(width).sum::<f64>() + width(front) / 2.0
                    };
                    // Round the whole oval: a garment smaller than it starts
                    // stretched, to close onto the body.
                    let around: f64 = (0..ring.len()).map(width).sum();
                    let stretch = (perimeter / around.max(1.0)).max(1.0);
                    let s = (offset + u - middle) * stretch
                        + if behind { perimeter / 2.0 } else { 0.0 };
                    let [x, z] = oval.at(s);
                    let middle = figure.smoothed(y).middle;
                    points.push([x + middle[0], y, z + middle[1]]);
                }
                placed.push(points);
            }
            placed
        };
    // A ring round an arm (a sleeve), sewn to points already placed
    // (`partners`: its panel, point, and where what it is sewn to is):
    // each panel's points.
    let wrap_arm = |ring: &[(usize, bool)],
                    arm: &Arm,
                    partners: &[(usize, u32, [f64; 3])]|
     -> Vec<Vec<[f64; 3]>> {
        // Down the ring from its top: each panel's span between its seams
        // at that depth, and the width round, every 5 mm.
        let depth = |p: usize, q: [f64; 2]| boxes[p].1[1] - q[1];
        let key = |d: f64| (d / 5.0).round() as i64;
        let mut rows: HashMap<i64, (Spans, f64)> = HashMap::new();
        for &(p, _) in ring {
            for q in &g.panels[p].mesh.points {
                let d = depth(p, *q);
                rows.entry(key(d)).or_insert_with(|| {
                    let spans: Spans = ring
                        .iter()
                        .map(|&(o, _)| between_seams(o, boxes[o].1[1] - d))
                        .collect();
                    let around = spans.iter().flatten().map(|(a, b)| b - a).sum();
                    (spans, around)
                });
            }
        }
        // A point's way round the ring from its first seam, its depth, and
        // the width round there.
        let round = |k: usize, turned: bool, q: [f64; 2]| -> (f64, f64, f64) {
            let p = ring[k].0;
            let d = depth(p, q);
            let (spans, around) = &rows[&key(d)];
            let width = |j: usize| spans[j].map_or(0.0, |(a, b)| b - a);
            let offset: f64 = (0..k).map(width).sum();
            let (left, right) = spans[k].unwrap_or((q[0], q[0]));
            let u = if turned { right - q[0] } else { q[0] - left };
            (offset + u, d, *around)
        };
        // The ring's top goes over the top of the arm.
        let (top, _) = ring
            .iter()
            .enumerate()
            .flat_map(|(k, &(p, turned))| {
                g.panels[p]
                    .mesh
                    .points
                    .iter()
                    .map(move |&q| (round(k, turned, q), depth(p, q)))
            })
            .map(|((s, _, _), d)| (s, d))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .expect("a ring of points");
        // Round the arm `along` mm down it, `sign` the way round, turned
        // `turn` radians round from its top over the top of the arm.
        let at = |(s, d, around): (f64, f64, f64), sign: f64, along: f64, turn: f64| {
            let (middle, up, side, radius) = arm.at(along + d);
            let r = (around / TAU).max(radius * CLEAR);
            let angle = sign * TAU * (s - top) / around.max(1.0) + turn;
            add3(
                middle,
                add3(scale3(up, r * angle.cos()), scale3(side, r * angle.sin())),
            )
        };
        // Turned and slid down the arm to bring its seams nearest what
        // they are sewn to.
        let index: HashMap<usize, (usize, bool)> = ring
            .iter()
            .enumerate()
            .map(|(k, &(p, turned))| (p, (k, turned)))
            .collect();
        // Slid as far as the wrist (a cuff), and turned round the arm in
        // steps of 15°.
        let mut best = (f64::MAX, 1.0, ARM_BEFORE, 0.0);
        let reach = ARM_STEP * (arm.line.len() - 1) as f64;
        for sign in [1.0, -1.0] {
            for turn in (0..24).map(|k| TAU * f64::from(k) / 24.0) {
                for step in 0..=(reach / 10.0) as u32 {
                    let along = 10.0 * f64::from(step);
                    let cost: f64 = partners
                        .iter()
                        .map(|&(p, i, partner)| {
                            let (k, turned) = index[&p];
                            let q = g.panels[p].mesh.points[i as usize];
                            let d = sub3(at(round(k, turned, q), sign, along, turn), partner);
                            dot3(d, d)
                        })
                        .sum();
                    if cost < best.0 {
                        best = (cost, sign, along, turn);
                    }
                }
            }
        }
        let (_, sign, along, turn) = best;
        ring.iter()
            .enumerate()
            .map(|(k, &(p, turned))| {
                g.panels[p]
                    .mesh
                    .points
                    .iter()
                    .map(|&q| at(round(k, turned, q), sign, along, turn))
                    .collect()
            })
            .collect()
    };
    let mut out: Vec<Vec<[f64; 3]>> = vec![Vec::new(); n];
    // The trunk's rings first: those hanging from the shoulders, or under
    // another.
    for (r, ring) in rings.iter().enumerate() {
        if let Some(top) = tops[r] {
            for (&(p, _), points) in ring.iter().zip(wrap_trunk(ring, closed[r], top, false)) {
                out[p] = points;
            }
        }
    }
    // Then the rest, each once what it is sewn to is placed: a bridge
    // between what it joins; a ring sewn to cloth by a shoulder or a wrist
    // round that arm; another from the waist.
    let mut waiting: Vec<usize> = (0..rings.len()).filter(|&r| tops[r].is_none()).collect();
    while !waiting.is_empty() {
        // Each waiting ring's points sewn to cloth placed: (its panel, its
        // point, where what it is sewn to is).
        let partners = |r: usize, out: &[Vec<[f64; 3]>]| -> Vec<(usize, u32, [f64; 3])> {
            g.stitches
                .iter()
                .flat_map(|&[a, b]| [(a, b), (b, a)])
                .filter(|&((p, _), (q, j))| {
                    ring_of[p] == r
                        && ring_of[q] != r
                        && !out[q].is_empty()
                        && (j as usize) < out[q].len()
                })
                .map(|((p, i), (q, j))| (p, i, out[q][j as usize]))
                .collect()
        };
        // A bridge waits for cloth placed on two of its edges at least.
        let spans = |r: usize, partners: &[(usize, u32, [f64; 3])]| {
            let p = rings[r][0].0;
            let mut edges: Vec<usize> = partners
                .iter()
                .filter_map(|&(_, i, _)| {
                    g.panels[p].edges.iter().position(|e| e.points.contains(&i))
                })
                .collect();
            edges.sort_unstable();
            edges.dedup();
            edges.len() >= 2
        };
        let ready = waiting.iter().position(|&r| {
            let found = partners(r, &out);
            !found.is_empty() && (!bridge[rings[r][0].0] || spans(r, &found))
        });
        // None ready: the largest from the waist (or, a bridge, between
        // what is placed of what it joins).
        let r = match ready {
            Some(k) => waiting.remove(k),
            None => {
                let k = (0..waiting.len())
                    .max_by(|&a, &b| {
                        let size = |r: usize| {
                            rings[r]
                                .iter()
                                .map(|&(p, _)| g.panels[p].mesh.points.len())
                                .sum::<usize>()
                        };
                        size(waiting[a]).cmp(&size(waiting[b])).then(b.cmp(&a))
                    })
                    .expect("one waiting");
                waiting.remove(k)
            }
        };
        let ring = &rings[r];
        let found = partners(r, &out);
        // A crotch piece: under the crotch, between the legs.
        if ring.len() == 1 && between_legs(&g.panels[ring[0].0].name) {
            let p = ring[0].0;
            out[p] = under_the_crotch(g, p, &out, figure);
            continue;
        }
        if bridge[ring[0].0] && !found.is_empty() {
            out[ring[0].0] = spanned(g, ring[0].0, &found);
            continue;
        }
        let arm = (!found.is_empty())
            .then(|| {
                let count = found.len() as f64;
                let middle = found
                    .iter()
                    .fold([0.0; 3], |m, &(_, _, x)| add3(m, scale3(x, 1.0 / count)));
                figure
                    .arms
                    .iter()
                    .map(|arm| {
                        // Nearest the arm's line, from the shoulder to the
                        // wrist.
                        let d = (0..arm.line.len())
                            .map(|k| {
                                let d = sub3(arm.line[k], middle);
                                dot3(d, d).sqrt() - arm.radius[k]
                            })
                            .fold(f64::MAX, f64::min);
                        (d, arm)
                    })
                    .min_by(|a, b| a.0.total_cmp(&b.0))
                    .filter(|(distance, _)| *distance < 150.0)
                    .map(|(_, arm)| arm)
            })
            .flatten();
        let placed = match arm {
            Some(arm) => wrap_arm(ring, arm, &found),
            None => wrap_trunk(ring, closed[r], figure.waist + 20.0, true),
        };
        for (&(p, _), points) in ring.iter().zip(placed) {
            out[p] = points;
        }
    }
    // Anything not placed (no seams at all): in front, at the waist.
    for p in 0..n {
        if out[p].is_empty() {
            let (low, high) = boxes[p];
            let s = figure.section(figure.waist);
            out[p] = g.panels[p]
                .mesh
                .points
                .iter()
                .map(|q| {
                    [
                        q[0] - (low[0] + high[0]) / 2.0,
                        figure.waist - (high[1] - q[1]),
                        s.middle[1] + s.half[1] + 80.0,
                    ]
                })
                .collect();
        }
    }
    out
}
