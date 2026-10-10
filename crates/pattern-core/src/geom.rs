//! Measuring outlines: lengths, positions along edges, tangents and normals,
//! nearest points, areas and bounds.

use kurbo::{
    BezPath, CubicBez, Line, ParamCurve, ParamCurveArclen, ParamCurveArea, ParamCurveDeriv,
    ParamCurveExtrema, ParamCurveNearest, PathEl, PathSeg, Point, Rect, Vec2,
};

use crate::Pt;
use crate::model::{Edge, Outline, Segment};

/// Accuracy of arc-length computations, mm.
pub const ARCLEN_ACCURACY: f64 = 1e-7;

/// Points closer than this are the same point, mm.
pub const SAME_POINT: f64 = 1e-6;

impl Segment {
    /// The segment as a kurbo curve starting at `from`.
    pub fn path_seg(&self, from: Pt) -> PathSeg {
        match *self {
            Segment::Line(l) => PathSeg::Line(Line::new(from, l.to)),
            Segment::Cubic(c) => PathSeg::Cubic(CubicBez::new(from, c.c1, c.c2, c.to)),
        }
    }

    /// The segment model for a kurbo curve (its start point is implied).
    pub fn from_path_seg(seg: PathSeg) -> Self {
        match seg {
            PathSeg::Line(l) => Segment::line(l.p1),
            PathSeg::Quad(q) => {
                let c = q.raise();
                Segment::cubic(c.p1, c.p2, c.p3)
            }
            PathSeg::Cubic(c) => Segment::cubic(c.p1, c.p2, c.p3),
        }
    }
}

/// Absolute kurbo segments for a run of segments starting at `start`.
pub fn segs_of(start: Pt, segments: &[Segment]) -> Vec<PathSeg> {
    let mut from = start;
    segments
        .iter()
        .map(|s| {
            let seg = s.path_seg(from);
            from = s.to();
            seg
        })
        .collect()
}

/// Unit tangent of `seg` at `t`, well defined even where a cubic's control
/// point coincides with its end point. Zero only for a zero-length segment.
pub fn unit_tangent(seg: &PathSeg, t: f64) -> Vec2 {
    const TINY: f64 = 1e-12;
    let first_nonzero = |candidates: &[Vec2]| {
        candidates
            .iter()
            .copied()
            .find(|v| v.hypot2() > TINY)
            .map_or(Vec2::ZERO, Vec2::normalize)
    };
    match seg {
        PathSeg::Line(l) => first_nonzero(&[l.p1 - l.p0]),
        PathSeg::Quad(q) => unit_tangent(&PathSeg::Cubic(q.raise()), t),
        PathSeg::Cubic(c) => {
            if t <= 1e-9 {
                first_nonzero(&[c.p1 - c.p0, c.p2 - c.p0, c.p3 - c.p0])
            } else if t >= 1.0 - 1e-9 {
                first_nonzero(&[c.p3 - c.p2, c.p3 - c.p1, c.p3 - c.p0])
            } else {
                let d = c.deriv().eval(t).to_vec2();
                if d.hypot2() > TINY {
                    d.normalize()
                } else {
                    // Cusp: use the chord across a small neighbourhood.
                    let h = 1e-4;
                    first_nonzero(&[c.eval((t + h).min(1.0)) - c.eval((t - h).max(0.0))])
                }
            }
        }
    }
}

/// The outward normal for a tangent of an outline that runs clockwise on
/// screen (positive signed area with y down).
pub fn outward_normal(tangent: Vec2) -> Vec2 {
    Vec2::new(tangent.y, -tangent.x)
}

/// Flattens segments into a polyline that stays within `tolerance` of the
/// curves. The first point is the start of the first segment and every
/// segment's exact end point is included.
pub fn flatten_segs(segs: &[PathSeg], tolerance: f64) -> Vec<Point> {
    let mut points = Vec::new();
    for seg in segs {
        match seg {
            PathSeg::Line(l) => {
                if points.is_empty() {
                    points.push(l.p0);
                }
                points.push(l.p1);
            }
            _ => {
                let start = seg.start();
                if points.is_empty() {
                    points.push(start);
                }
                let elements = [PathEl::MoveTo(start), seg.as_path_el()];
                kurbo::flatten(elements, tolerance, |el| {
                    if let PathEl::LineTo(p) = el {
                        points.push(p);
                    }
                });
                // Make sure the exact end point is present.
                let end = seg.end();
                match points.last_mut() {
                    Some(last) if last.distance(end) < SAME_POINT => *last = end,
                    _ => points.push(end),
                }
            }
        }
    }
    points
}

impl Outline {
    /// Start point of edge `i`.
    pub fn edge_start(&self, i: usize) -> Pt {
        if i == 0 {
            return self.start;
        }
        // An empty edge has no end; fall back to the last edge before it that has one.
        self.edges[..i]
            .iter()
            .rev()
            .find_map(Edge::end)
            .unwrap_or(self.start)
    }

    /// End point of edge `i`.
    pub fn edge_end(&self, i: usize) -> Pt {
        self.edges[i].end().unwrap_or_else(|| self.edge_start(i))
    }

    pub fn edge_index(&self, id: &str) -> Option<usize> {
        self.edges.iter().position(|e| e.id == id)
    }

    /// Geometry of edge `i`, for measuring along it.
    pub fn edge_geom(&self, i: usize) -> EdgeGeom {
        EdgeGeom::new(segs_of(self.edge_start(i), &self.edges[i].segments))
    }

    /// Every segment of the outline as an absolute kurbo curve.
    pub fn path_segs(&self) -> Vec<PathSeg> {
        let mut segs = Vec::new();
        let mut from = self.start;
        for edge in &self.edges {
            for s in &edge.segments {
                segs.push(s.path_seg(from));
                from = s.to();
            }
        }
        segs
    }

    pub fn to_bezpath(&self) -> BezPath {
        let mut path = BezPath::new();
        path.move_to(self.start);
        for seg in self.path_segs() {
            path.push(seg.as_path_el());
        }
        path.close_path();
        path
    }

    /// Signed area in mm². Positive for the expected orientation
    /// (clockwise on screen, y down).
    pub fn signed_area(&self) -> f64 {
        self.path_segs()
            .iter()
            .map(ParamCurveArea::signed_area)
            .sum()
    }

    pub fn bounds(&self) -> Rect {
        self.path_segs()
            .iter()
            .map(ParamCurveExtrema::bounding_box)
            .reduce(|a, b| a.union(b))
            .unwrap_or_else(|| Rect::from_points(self.start, self.start))
    }

    /// Distance between the end of the last edge and `start`.
    pub fn closure_gap(&self) -> f64 {
        match self.edges.last().and_then(Edge::end) {
            Some(end) => end.distance(self.start),
            None => f64::INFINITY,
        }
    }

    /// Total length of the outline, mm.
    pub fn length(&self) -> f64 {
        self.path_segs()
            .iter()
            .map(|s| s.arclen(ARCLEN_ACCURACY))
            .sum()
    }
}

/// The nearest point on an edge to some query point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeNearest {
    /// Distance from the query point, mm.
    pub distance: f64,
    /// Distance along the edge from its start, mm.
    pub along: f64,
    pub point: Point,
}

/// An edge prepared for measuring along it.
#[derive(Clone, Debug)]
pub struct EdgeGeom {
    segs: Vec<PathSeg>,
    /// Cumulative length at the end of each segment.
    cum: Vec<f64>,
}

impl EdgeGeom {
    pub fn new(segs: Vec<PathSeg>) -> Self {
        let mut total = 0.0;
        let cum = segs
            .iter()
            .map(|s| {
                total += s.arclen(ARCLEN_ACCURACY);
                total
            })
            .collect();
        EdgeGeom { segs, cum }
    }

    pub fn segs(&self) -> &[PathSeg] {
        &self.segs
    }

    pub fn length(&self) -> f64 {
        self.cum.last().copied().unwrap_or(0.0)
    }

    pub fn start(&self) -> Point {
        self.segs.first().map_or(Point::ZERO, ParamCurve::start)
    }

    pub fn end(&self) -> Point {
        self.segs.last().map_or(Point::ZERO, ParamCurve::end)
    }

    /// Segment index and parameter at distance `along` from the start
    /// (clamped to the edge).
    pub fn locate(&self, along: f64) -> (usize, f64) {
        if self.segs.is_empty() {
            return (0, 0.0);
        }
        let along = along.clamp(0.0, self.length());
        let k = self
            .cum
            .partition_point(|&c| c < along)
            .min(self.segs.len() - 1);
        let before = if k == 0 { 0.0 } else { self.cum[k - 1] };
        let seg_len = self.cum[k] - before;
        let t = if seg_len <= 0.0 {
            0.0
        } else {
            self.segs[k].inv_arclen((along - before).min(seg_len), ARCLEN_ACCURACY)
        };
        (k, t.clamp(0.0, 1.0))
    }

    pub fn point_at(&self, along: f64) -> Point {
        let (k, t) = self.locate(along);
        self.segs.get(k).map_or(Point::ZERO, |s| s.eval(t))
    }

    /// Unit tangent at distance `along` (direction of travel).
    pub fn tangent_at(&self, along: f64) -> Vec2 {
        let (k, t) = self.locate(along);
        self.segs.get(k).map_or(Vec2::ZERO, |s| unit_tangent(s, t))
    }

    /// Unit outward normal at distance `along`.
    pub fn normal_at(&self, along: f64) -> Vec2 {
        outward_normal(self.tangent_at(along))
    }

    /// Unit tangent leaving the start of the edge.
    pub fn start_tangent(&self) -> Vec2 {
        self.segs
            .iter()
            .map(|s| unit_tangent(s, 0.0))
            .find(|v| *v != Vec2::ZERO)
            .unwrap_or(Vec2::ZERO)
    }

    /// Unit tangent arriving at the end of the edge.
    pub fn end_tangent(&self) -> Vec2 {
        self.segs
            .iter()
            .rev()
            .map(|s| unit_tangent(s, 1.0))
            .find(|v| *v != Vec2::ZERO)
            .unwrap_or(Vec2::ZERO)
    }

    /// Distance along the edge of parameter `t` on segment `k`.
    pub fn along_at(&self, k: usize, t: f64) -> f64 {
        let before = if k == 0 { 0.0 } else { self.cum[k - 1] };
        before + self.segs[k].subsegment(0.0..t).arclen(ARCLEN_ACCURACY)
    }

    pub fn nearest(&self, p: Point) -> Option<EdgeNearest> {
        let (k, n) = self
            .segs
            .iter()
            .enumerate()
            .map(|(k, s)| (k, s.nearest(p, 1e-9)))
            .min_by(|a, b| a.1.distance_sq.total_cmp(&b.1.distance_sq))?;
        Some(EdgeNearest {
            distance: n.distance_sq.sqrt(),
            along: self.along_at(k, n.t),
            point: self.segs[k].eval(n.t),
        })
    }

    /// Polyline within `tolerance` of the edge.
    pub fn flatten(&self, tolerance: f64) -> Vec<Point> {
        flatten_segs(&self.segs, tolerance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Edge;

    fn square(size: f64) -> Outline {
        Outline {
            start: Pt::new(0.0, 0.0),
            edges: vec![
                Edge::new("e1", vec![Segment::line([size, 0.0])]),
                Edge::new("e2", vec![Segment::line([size, size])]),
                Edge::new("e3", vec![Segment::line([0.0, size])]),
                Edge::new("e4", vec![Segment::line([0.0, 0.0])]),
            ],
        }
    }

    #[test]
    fn clockwise_on_screen_has_positive_area() {
        // (0,0) → (100,0) → (100,100) → (0,100) runs clockwise with y down.
        let o = square(100.0);
        assert!((o.signed_area() - 10_000.0).abs() < 1e-9);
        assert!(o.closure_gap() < 1e-12);
    }

    #[test]
    fn outward_normal_points_away_from_the_piece() {
        let o = square(100.0);
        // Top edge runs right; outside is up (negative y).
        let n = o.edge_geom(0).normal_at(50.0);
        assert!((n.y + 1.0).abs() < 1e-12 && n.x.abs() < 1e-12);
        // Right edge runs down; outside is to the right.
        let n = o.edge_geom(1).normal_at(50.0);
        assert!((n.x - 1.0).abs() < 1e-12 && n.y.abs() < 1e-12);
    }

    #[test]
    fn measures_along_an_edge() {
        let geom = EdgeGeom::new(segs_of(
            Pt::new(0.0, 0.0),
            &[Segment::line([30.0, 0.0]), Segment::line([30.0, 40.0])],
        ));
        assert!((geom.length() - 70.0).abs() < 1e-9);
        let p = geom.point_at(50.0);
        assert!((p.x - 30.0).abs() < 1e-9 && (p.y - 20.0).abs() < 1e-9);
        let near = geom.nearest(Point::new(35.0, 20.0)).unwrap();
        assert!((near.along - 50.0).abs() < 1e-9);
        assert!((near.distance - 5.0).abs() < 1e-9);
    }

    #[test]
    fn quarter_circle_arc_length() {
        // Standard cubic approximation of a quarter circle of radius 100.
        let k = 0.552_284_749_830_793_4 * 100.0;
        let geom = EdgeGeom::new(segs_of(
            Pt::new(100.0, 0.0),
            &[Segment::cubic([100.0, k], [k, 100.0], [0.0, 100.0])],
        ));
        let exact = std::f64::consts::FRAC_PI_2 * 100.0;
        // The cubic approximation is within 0.03% of the true circle.
        assert!((geom.length() - exact).abs() / exact < 3e-4);
        // Distances along the curve round-trip through `nearest`.
        for d in [0.0, 10.0, 77.7, 150.0, geom.length()] {
            let near = geom.nearest(geom.point_at(d)).unwrap();
            assert!((near.along - d).abs() < 1e-6, "{d} vs {}", near.along);
        }
    }

    #[test]
    fn tangent_is_defined_at_degenerate_cubic_ends() {
        // c1 coincides with the start point, a common export artefact.
        let seg = PathSeg::Cubic(CubicBez::new(
            (0.0, 0.0),
            (0.0, 0.0),
            (50.0, 0.0),
            (100.0, 0.0),
        ));
        let t0 = unit_tangent(&seg, 0.0);
        assert!((t0.x - 1.0).abs() < 1e-12);
        let seg = PathSeg::Cubic(CubicBez::new(
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
            (0.0, 0.0),
        ));
        assert_eq!(unit_tangent(&seg, 0.5), Vec2::ZERO);
    }

    #[test]
    fn flattening_keeps_exact_end_points() {
        let segs = segs_of(
            Pt::new(0.0, 0.0),
            &[
                Segment::cubic([0.0, 50.0], [100.0, 50.0], [100.0, 0.0]),
                Segment::line([200.0, 0.0]),
            ],
        );
        let pts = flatten_segs(&segs, 0.01);
        assert_eq!(pts.first(), Some(&Point::new(0.0, 0.0)));
        assert!(pts.contains(&Point::new(100.0, 0.0)));
        assert_eq!(pts.last(), Some(&Point::new(200.0, 0.0)));
    }
}
