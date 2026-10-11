//! A drape, from flat panels to cloth settled on the body: sewn, the
//! stitches tightening as the seams close round the body while the cloth
//! hangs, then left to settle until it is still.

use crate::cloth::{Cloth, ClothBuilder, V3};
use crate::collide::BodyField;
use crate::fabric::Fabric;
use crate::garment::Garment;

/// Seconds spent sewing, in three stages, and how loosely the stitches hold
/// in each (m/N), tightening as the seams close. The cloth hangs all the
/// while, so that it settles where it would be worn.
const SEWING: [(f32, f32); 3] = [(0.4, 1e-3), (0.4, 1e-5), (0.4, 1e-8)];
/// Steps a frame: small enough for stiff cloth.
const SUBSTEPS: u32 = 30;
/// Settled: the cloth's mean speed below this, m/s.
const STILL: f32 = 0.004;
/// No point moves faster than this, m/s: the placing leaves the cloth
/// strained where it is stretched round the body, and let go all at once
/// that strain would fling it.
const MAX_SPEED: f32 = 1.0;

/// A drape in progress.
#[derive(Debug)]
pub struct Drape {
    pub cloth: Cloth,
    body: BodyField,
    /// Seconds simulated.
    pub time: f32,
    /// Where each panel's points start in the cloth.
    pub firsts: Vec<u32>,
}

impl Drape {
    /// Starts a drape of `garment`, its panels placed at `starts` (mm), in
    /// `fabric`, on the body `body` (points mm, triangles facing out).
    pub fn new(
        garment: &Garment,
        starts: &[Vec<[f64; 3]>],
        fabric: Fabric,
        body: (&[[f64; 3]], &[[u32; 3]]),
    ) -> Drape {
        let mut builder = ClothBuilder::new(fabric);
        let mut firsts = Vec::new();
        for (panel, start) in garment.panels.iter().zip(starts) {
            // Flat in the fabric's own directions: the grain along +y.
            let (g0, g1) = (panel.grain[0], panel.grain[1]);
            let rest: Vec<[f32; 2]> = panel
                .mesh
                .points
                .iter()
                .map(|p| {
                    let (u, v) = (p[0] * g1 - p[1] * g0, p[0] * g0 + p[1] * g1);
                    [(u / 1000.0) as f32, (v / 1000.0) as f32]
                })
                .collect();
            let start: Vec<V3> = start
                .iter()
                .map(|p| p.map(|c| (c / 1000.0) as f32))
                .collect();
            firsts.push(builder.panel(&rest, &start, &panel.mesh.triangles));
        }
        for [(pa, a), (pb, b)] in &garment.stitches {
            builder.stitch(firsts[*pa] + a, firsts[*pb] + b);
        }
        let mut cloth = builder.build();
        cloth.damping = 6.0;
        cloth.max_speed = MAX_SPEED;
        cloth.stitch_compliance = SEWING[0].1;
        let points: Vec<V3> = body
            .0
            .iter()
            .map(|p| p.map(|c| (c / 1000.0) as f32))
            .collect();
        let field = BodyField::new(&points, body.1, 0.008, 0.06);
        cloth.push_out(&field);
        Drape {
            cloth,
            body: field,
            time: 0.0,
            firsts,
        }
    }

    /// Whether the seams are still being sewn.
    pub fn sewing(&self) -> bool {
        self.time < SEWING.iter().map(|s| s.0).sum::<f32>()
    }

    /// Moves on a frame of `dt` seconds.
    pub fn step(&mut self, dt: f32) {
        let mut until = 0.0;
        let mut stage = None;
        for &(seconds, compliance) in &SEWING {
            until += seconds;
            if self.time < until {
                stage = Some(compliance);
                break;
            }
        }
        match stage {
            Some(compliance) => self.cloth.stitch_compliance = compliance,
            None => {
                self.cloth.damping = 3.0;
                self.cloth.stitch_compliance = 1e-8;
            }
        }
        self.cloth.step(dt, SUBSTEPS, &self.body);
        self.time += dt;
    }

    /// Sewn, and hanging still after a second at least: just sewn, before
    /// gravity has pulled at it, it is still too.
    pub fn settled(&self) -> bool {
        let sewn: f32 = SEWING.iter().map(|s| s.0).sum();
        self.time > sewn + 1.0 && self.cloth.mean_speed() < STILL
    }

    /// The body's distance at a point (m): for checking.
    pub fn clearance(&self, p: V3) -> f32 {
        crate::cloth::Collider::distance(&self.body, p).0
    }
}
