//! Placing a garment's panels round the body before it is sewn: panels
//! joined by upright seams (side seams, princess seams, a centre back) go
//! round the body as a ring, in the order their seams join them, wrapped
//! on an oval a little larger than the body there, the centre front in
//! front. A ring with shoulder seams (a bodice) hangs from above the
//! shoulders; another from the waist, or from the ring it is sewn under.

use std::collections::HashMap;
use std::f64::consts::TAU;

use drapewright_body::measure::trunk_sections;
use drapewright_body::{Body, BodyModel, measure};

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
        let (_, marks) = measure(model, body);
        let shoulders = marks.points["shoulderPoint"][1];
        let hip = marks.levels["hip"];
        let sections = trunk_sections(model, body, hip - 400.0, shoulders, 20.0)
            .into_iter()
            .map(|(y, middle, half)| Section { y, middle, half })
            .collect();
        Figure {
            waist: marks.levels["waist"],
            shoulders,
            sections,
        }
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
    fn section(&self, y: f64) -> Section {
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

/// A ring at one height: each panel's span across there (or none), and
/// the oval they go round.
type Row = (Vec<Option<(f64, f64)>>, Oval);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Side {
    Left,
    Right,
}

/// Where each panel's points start, mm.
pub fn place(g: &Garment, figure: &Figure) -> Vec<Vec<[f64; 3]>> {
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
    let mut shoulders = vec![false; n];
    let mut under: Vec<Option<usize>> = vec![None; n];
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
            // Near the top of both: over the shoulders. One's top to the
            // other's bottom: hung under it.
            let near_top =
                |p: usize, v: f64| v > boxes[p].1[1] - 0.25 * (boxes[p].1[1] - boxes[p].0[1]);
            let near_bottom =
                |p: usize, v: f64| v < boxes[p].0[1] + 0.25 * (boxes[p].1[1] - boxes[p].0[1]);
            if near_top(*pa, sa.1) && near_top(*pb, sb.1) {
                shoulders[*pa] = true;
                shoulders[*pb] = true;
            } else if near_top(*pa, sa.1) && near_bottom(*pb, sb.1) {
                under[*pa] = Some(*pb);
            } else if near_top(*pb, sb.1) && near_bottom(*pa, sa.1) {
                under[*pb] = Some(*pa);
            }
        }
    }
    // Rings: panels joined side by side, in order round the body, each
    // turned over if its seams run the other way.
    let mut ring_of = vec![usize::MAX; n];
    let mut rings: Vec<Vec<(usize, bool)>> = Vec::new();
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
        // Rightwards from the start, then leftwards for an open ring.
        for going in [Side::Right, Side::Left] {
            let (mut at, mut out) = (start, going);
            loop {
                let next = beside[at]
                    .iter()
                    .find(|(s, q, _, _)| *s == out && (ring_of[*q] == usize::MAX || *q == start));
                let Some(&(_, q, enters, _)) = next else {
                    break;
                };
                if q == start {
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
    let mut out: Vec<Vec<[f64; 3]>> = vec![Vec::new(); n];
    for (r, ring) in rings.iter().enumerate() {
        let top = tops[r].unwrap_or(figure.waist + 20.0);
        // The front's middle in front, a back's behind; otherwise the
        // first panel's middle in front.
        let front = ring
            .iter()
            .position(|&(p, _)| names[p].contains("front"))
            .unwrap_or(0);
        let behind = names[ring[front].0].contains("back");
        // The ring at each height (every 5 mm): each panel's span there (or
        // none), and the oval it goes round.
        let key = |y: f64| (y / 5.0).round() as i64;
        let mut rows: HashMap<i64, Row> = HashMap::new();
        for &(p, _) in ring {
            for q in &g.panels[p].mesh.points {
                let y = top - (boxes[p].1[1] - q[1]);
                rows.entry(key(y)).or_insert_with(|| {
                    let spans: Vec<Option<(f64, f64)>> = ring
                        .iter()
                        .map(|&(o, _)| between_seams(o, boxes[o].1[1] - (top - y)))
                        .collect();
                    let around: f64 = spans.iter().flatten().map(|(a, b)| b - a).sum();
                    (spans, Oval::round(figure.smoothed(y), around))
                });
            }
        }
        for (k, &(p, turned)) in ring.iter().enumerate() {
            let (_, high) = boxes[p];
            let mut points = Vec::new();
            for q in &g.panels[p].mesh.points {
                let y = top - (high[1] - q[1]);
                let (spans, oval) = &rows[&key(y)];
                let perimeter = oval.perimeter();
                let width = |j: usize| spans[j].map_or(0.0, |(a, b)| b - a);
                let offset: f64 = (0..k).map(width).sum();
                let (left, right) = spans[k].unwrap_or((q[0], q[0]));
                let u = if turned { right - q[0] } else { q[0] - left };
                let middle: f64 = (0..front).map(width).sum::<f64>() + width(front) / 2.0;
                // Round the whole oval: a garment smaller than it starts
                // stretched, to close onto the body.
                let around: f64 = (0..ring.len()).map(width).sum();
                let stretch = (perimeter / around.max(1.0)).max(1.0);
                let s =
                    (offset + u - middle) * stretch + if behind { perimeter / 2.0 } else { 0.0 };
                let [x, z] = oval.at(s);
                let middle = figure.smoothed(y).middle;
                points.push([x + middle[0], y, z + middle[1]]);
            }
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
