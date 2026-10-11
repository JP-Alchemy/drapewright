//! The camera: it orbits a point on the figure, as one walks round a dress
//! form, and zooms towards it.

use std::f32::consts::TAU;

/// A 4×4 matrix, column by column (as WGSL reads it).
pub type Matrix = [[f32; 4]; 4];

/// Looking at `target` from `distance` away, `yaw` round and `pitch` up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    /// The point looked at, mm.
    pub target: [f32; 3],
    /// Round the figure, radians: 0 in front, positive towards its left.
    pub yaw: f32,
    /// Above the target (positive) or below, radians.
    pub pitch: f32,
    /// From the target, mm.
    pub distance: f32,
    /// The view's height, radians.
    pub fov_y: f32,
}

/// Not quite straight down or up, where turning would flip.
const PITCH_LIMIT: f32 = 1.4;
/// Close enough for a neckline, far enough for a room, mm.
const NEAREST: f32 = 250.0;
const FARTHEST: f32 = 15_000.0;

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn unit(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt().max(f32::EPSILON);
    [a[0] / l, a[1] / l, a[2] / l]
}

/// `a · b`, for column matrices.
pub fn multiply(a: &Matrix, b: &Matrix) -> Matrix {
    let mut out = [[0.0; 4]; 4];
    for (c, column) in out.iter_mut().enumerate() {
        for (r, value) in column.iter_mut().enumerate() {
            *value = (0..4).map(|k| a[k][r] * b[c][k]).sum();
        }
    }
    out
}

impl Orbit {
    /// A figure between the corners `low` and `high`, seen from the front
    /// and a little above, the whole of it in a view `aspect` wide.
    pub fn framing(low: [f32; 3], high: [f32; 3], aspect: f32) -> Orbit {
        let fov_y = 30f32.to_radians();
        let target = [
            (low[0] + high[0]) / 2.0,
            (low[1] + high[1]) / 2.0,
            (low[2] + high[2]) / 2.0,
        ];
        let mut orbit = Orbit {
            target,
            yaw: 0.0,
            pitch: 0.1,
            distance: 1.0,
            fov_y,
        };
        orbit.fit(low, high, aspect);
        orbit
    }

    /// Moves back or in so that the figure between `low` and `high` fills
    /// the view, with a margin.
    pub fn fit(&mut self, low: [f32; 3], high: [f32; 3], aspect: f32) {
        let tall = (high[1] - low[1]) / 2.0;
        let wide = (high[0] - low[0]).max(high[2] - low[2]) / 2.0 / aspect.max(0.1);
        let half = tall.max(wide) * 1.12;
        self.distance = (half / (self.fov_y / 2.0).tan()).clamp(NEAREST, FARTHEST);
    }

    /// Where the camera is, mm.
    pub fn eye(&self) -> [f32; 3] {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [
            self.target[0] + self.distance * sy * cp,
            self.target[1] + self.distance * sp,
            self.target[2] + self.distance * cy * cp,
        ]
    }

    /// Turns round the target by `yaw` and up by `pitch`, radians.
    pub fn turn(&mut self, yaw: f32, pitch: f32) {
        self.yaw = (self.yaw + yaw).rem_euclid(TAU);
        self.pitch = (self.pitch + pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// Comes `factor` times nearer (less than 1: further).
    pub fn zoom(&mut self, factor: f32) {
        if factor > 0.0 && factor.is_finite() {
            self.distance = (self.distance / factor).clamp(NEAREST, FARTHEST);
        }
    }

    /// Slides the target across the view: `right` and `up`, mm.
    pub fn slide(&mut self, right: f32, up: f32) {
        let forward = unit(sub(self.target, self.eye()));
        let r = unit(cross(forward, [0.0, 1.0, 0.0]));
        let u = cross(r, forward);
        for k in 0..3 {
            self.target[k] += right * r[k] + up * u[k];
        }
    }

    /// Millimetres at the target per pixel, for a view `height` pixels tall.
    pub fn mm_per_pixel(&self, height: f32) -> f32 {
        2.0 * self.distance * (self.fov_y / 2.0).tan() / height.max(1.0)
    }

    /// From the figure's space to the view's, for a view `aspect` wide
    /// (depth from 0 near to 1 far).
    pub fn view_projection(&self, aspect: f32) -> Matrix {
        let eye = self.eye();
        let f = unit(sub(self.target, eye));
        let s = unit(cross(f, [0.0, 1.0, 0.0]));
        let u = cross(s, f);
        let view = [
            [s[0], u[0], -f[0], 0.0],
            [s[1], u[1], -f[1], 0.0],
            [s[2], u[2], -f[2], 0.0],
            [-dot(s, eye), -dot(u, eye), dot(f, eye), 1.0],
        ];
        let near = (self.distance * 0.05).max(10.0);
        let far = self.distance * 3.0 + 3_000.0;
        let g = 1.0 / (self.fov_y / 2.0).tan();
        let projection = [
            [g / aspect.max(0.01), 0.0, 0.0, 0.0],
            [0.0, g, 0.0, 0.0],
            [0.0, 0.0, far / (near - far), -1.0],
            [0.0, 0.0, near * far / (near - far), 0.0],
        ];
        multiply(&projection, &view)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(m: &Matrix, p: [f32; 3]) -> [f32; 3] {
        let v = [p[0], p[1], p[2], 1.0];
        let c: Vec<f32> = (0..4)
            .map(|r| (0..4).map(|k| m[k][r] * v[k]).sum())
            .collect();
        [c[0] / c[3], c[1] / c[3], c[2] / c[3]]
    }

    #[test]
    fn a_figure_is_framed_whole_and_centred() {
        let (low, high) = ([-250.0, 0.0, -150.0], [250.0, 1700.0, 150.0]);
        let orbit = Orbit::framing(low, high, 1.0);
        let m = orbit.view_projection(1.0);
        let centre = project(&m, orbit.target);
        assert!(
            centre[0].abs() < 1e-4 && centre[1].abs() < 1e-4,
            "{centre:?}"
        );
        assert!((0.0..1.0).contains(&centre[2]));
        // Head and feet are in view, near its top and bottom.
        let top = project(&m, [0.0, 1700.0, 0.0])[1];
        let bottom = project(&m, [0.0, 0.0, 0.0])[1];
        assert!(top > 0.75 && top < 1.0, "{top}");
        assert!(bottom < -0.75 && bottom > -1.0, "{bottom}");
    }

    #[test]
    fn turning_goes_round_and_stops_short_of_overhead() {
        let mut orbit = Orbit::framing([-1.0, 0.0, -1.0], [1.0, 1000.0, 1.0], 1.0);
        let before = orbit.distance;
        orbit.pitch = 0.0;
        orbit.turn(std::f32::consts::FRAC_PI_2, 0.0);
        let eye = orbit.eye();
        // A quarter turn: looking from the figure's left side.
        assert!(eye[0] > 0.0 && eye[2].abs() < 1e-3, "{eye:?}");
        assert!((before - orbit.distance).abs() < 1e-3);
        orbit.turn(0.0, 10.0);
        assert!((orbit.pitch - PITCH_LIMIT).abs() < 1e-6);
        orbit.zoom(1e9);
        assert!((orbit.distance - NEAREST).abs() < 1e-3);
        orbit.zoom(0.0);
        assert!((orbit.distance - NEAREST).abs() < 1e-3);
    }

    #[test]
    fn sliding_moves_the_target_across_the_view() {
        let mut orbit = Orbit::framing([-1.0, 0.0, -1.0], [1.0, 1000.0, 1.0], 1.0);
        orbit.pitch = 0.0;
        let before = orbit.target;
        orbit.slide(100.0, 50.0);
        // From the front, right is the figure's left (+X) and up is up.
        assert!((orbit.target[0] - before[0] - 100.0).abs() < 1e-3);
        assert!((orbit.target[1] - before[1] - 50.0).abs() < 1e-3);
        assert!((orbit.target[2] - before[2]).abs() < 1e-3);
    }
}
