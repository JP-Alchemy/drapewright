//! The cloth on its own: hanging, falling on a ball, sewn into a tube.

use drapewright_sim::cloth::{Collider, V3};
use drapewright_sim::fabric::preset;
use drapewright_sim::mesh::{PanelMesh, mesh};
use drapewright_sim::{Cloth, ClothBuilder};

/// A rectangle `w` by `h` m, meshed with points about `spacing` m apart.
fn rectangle(w: f32, h: f32, spacing: f32) -> PanelMesh {
    let side = |a: [f64; 2], b: [f64; 2]| {
        let n = ((b[0] - a[0]).hypot(b[1] - a[1]) / f64::from(spacing))
            .round()
            .max(1.0) as usize;
        (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
            })
            .collect::<Vec<_>>()
    };
    let (w, h) = (f64::from(w), f64::from(h));
    mesh(
        &[
            side([0.0, 0.0], [w, 0.0]),
            side([w, 0.0], [w, h]),
            side([w, h], [0.0, h]),
            side([0.0, h], [0.0, 0.0]),
        ],
        f64::from(spacing),
    )
    .unwrap()
}

fn flat(m: &PanelMesh) -> Vec<[f32; 2]> {
    m.points
        .iter()
        .map(|p| [p[0] as f32, p[1] as f32])
        .collect()
}

struct Ball {
    centre: V3,
    radius: f32,
}

impl Collider for Ball {
    fn distance(&self, p: V3) -> (f32, V3) {
        let d = [
            p[0] - self.centre[0],
            p[1] - self.centre[1],
            p[2] - self.centre[2],
        ];
        let l = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt().max(1e-9);
        (l - self.radius, d.map(|c| c / l))
    }
}

/// A post standing on the floor, round the y axis.
struct Post {
    radius: f32,
}

impl Collider for Post {
    fn distance(&self, p: V3) -> (f32, V3) {
        let l = p[0].hypot(p[2]).max(1e-9);
        (l - self.radius, [p[0] / l, 0.0, p[2] / l])
    }
}

/// Steps a second takes, in small steps of 1/1800 s: small enough for
/// stiff cloth to settle without stretching.
fn run(cloth: &mut Cloth, seconds: f32, body: &dyn Collider) {
    for _ in 0..(seconds * 60.0) as usize {
        cloth.step(1.0 / 60.0, 30, body);
    }
}

#[test]
fn a_cloth_hung_by_its_top_edge_settles_without_stretching_much() {
    let m = rectangle(1.0, 1.0, 0.04);
    let rest = flat(&m);
    // A curtain: hung in the x-y plane by its top edge, at y = 1.
    let start: Vec<V3> = rest.iter().map(|p| [p[0], p[1], 0.0]).collect();
    let mut builder = ClothBuilder::new(preset("poplin").unwrap());
    builder.panel(&rest, &start, &m.triangles);
    let mut cloth = builder.build();
    for (k, p) in rest.iter().enumerate() {
        if (p[1] - 1.0).abs() < 1e-6 {
            cloth.pin(k as u32);
        }
    }
    run(&mut cloth, 3.0, &drapewright_sim::cloth::Nothing);
    let mean = cloth.mean_stretch();
    assert!(mean < 1.005, "stretched {mean} on average");
    let most = cloth.most_stretched();
    assert!(most < 1.02, "stretched {most} at the most");
    assert!(
        cloth.mean_speed() < 0.01,
        "still moving at {} m/s",
        cloth.mean_speed()
    );
    // It hangs straight down, its own length.
    let lowest = cloth.x.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    assert!((-0.03..0.01).contains(&lowest), "{lowest}");
}

#[test]
fn a_cloth_falls_on_a_ball_and_rests_on_it() {
    let m = rectangle(1.0, 1.0, 0.04);
    let rest = flat(&m);
    // Flat, 10 cm above a ball of radius 30 cm.
    let start: Vec<V3> = rest.iter().map(|p| [p[0] - 0.5, 0.4, p[1] - 0.5]).collect();
    let mut builder = ClothBuilder::new(preset("cottonLawn").unwrap());
    builder.panel(&rest, &start, &m.triangles);
    let mut cloth = builder.build();
    let ball = Ball {
        centre: [0.0, 0.0, 0.0],
        radius: 0.3,
    };
    run(&mut cloth, 2.5, &ball);
    let nearest = cloth
        .x
        .iter()
        .map(|&p| ball.distance(p).0)
        .fold(f32::MAX, f32::min);
    assert!(nearest > -0.002, "a point is {nearest} m inside the ball");
    // Draped over it: the middle on top, the corners hanging down.
    let top = cloth.x.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((0.29..0.33).contains(&top), "{top}");
    let lowest = cloth.x.iter().map(|p| p[1]).fold(f32::MAX, f32::min);
    assert!(lowest < 0.0, "{lowest}");
}

#[test]
fn two_panels_sewn_at_their_sides_close_round_a_post() {
    let (w, h) = (0.5, 0.4);
    let m = rectangle(w, h, 0.03);
    let rest = flat(&m);
    let fabric = preset("poplin").unwrap();
    let mut builder = ClothBuilder::new(fabric);
    // One panel in front of the post, one behind, a little apart.
    let front: Vec<V3> = rest.iter().map(|p| [p[0] - w / 2.0, p[1], 0.2]).collect();
    let back: Vec<V3> = rest.iter().map(|p| [w / 2.0 - p[0], p[1], -0.2]).collect();
    let a = builder.panel(&rest, &front, &m.triangles);
    let b = builder.panel(&rest, &back, &m.triangles);
    // The back is turned round: its x = w side is on the front's x = 0
    // side of the post, and the other way about.
    let at = |x: f32, y: f32| {
        rest.iter()
            .position(|p| (p[0] - x).abs() < 1e-6 && (p[1] - y).abs() < 1e-6)
            .unwrap() as u32
    };
    for p in &rest {
        if p[0].abs() < 1e-6 || (p[0] - w).abs() < 1e-6 {
            builder.stitch(a + at(p[0], p[1]), b + at(w - p[0], p[1]));
        }
    }
    let mut cloth = builder.build();
    let post = Post { radius: 0.12 };
    // Sewn first, without gravity, the stitches tightening.
    cloth.gravity = [0.0; 3];
    cloth.damping = 4.0;
    for (seconds, compliance) in [(0.3, 1e-3), (0.3, 1e-5), (0.4, 1e-8)] {
        cloth.stitch_compliance = compliance;
        run(&mut cloth, seconds, &post);
    }
    let gap = cloth.widest_stitch();
    assert!(gap < 0.002, "a seam is still {gap} m open");
    let nearest = cloth
        .x
        .iter()
        .map(|&p| post.distance(p).0)
        .fold(f32::MAX, f32::min);
    assert!(nearest > -0.002, "{nearest}");
}
