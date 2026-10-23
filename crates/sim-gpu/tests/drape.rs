//! The test garments draped on the graphics card, as on the processor.

use std::future::Future;
use std::time::Instant;

use drapewright_body::{BodyModel, Pose, Shape};
use drapewright_project::container::unpack;
use drapewright_project::migrate::load;
use drapewright_seams::Seam;
use drapewright_sim::ClothBuilder;
use drapewright_sim::cloth::V3;
use drapewright_sim::collide::{BodyField, Seat};
use drapewright_sim::drape::{Drape, Felt, Posable, Stepper};
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::{draped_pieces, shell};
use drapewright_sim::inside::with_inside;
use drapewright_sim::mesh::{PanelMesh, mesh};
use drapewright_sim::place::{Figure, place};
use drapewright_sim_gpu::GpuCloth;

/// Runs a future to its end, here and now.
fn block_on<F: Future>(f: F) -> F::Output {
    let waker = std::task::Waker::noop();
    let mut cx = std::task::Context::from_waker(waker);
    let mut f = std::pin::pin!(f);
    loop {
        if let std::task::Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// A graphics card to compute on, if there is one.
fn card() -> Option<(wgpu::Device, wgpu::Queue)> {
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends = wgpu::Backends::PRIMARY;
    let instance = wgpu::Instance::new(desc);
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    eprintln!("on {:?}", adapter.get_info().name);
    block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("tests"),
        required_limits: wgpu::Limits::default().using_resolution(adapter.limits()),
        ..Default::default()
    }))
    .ok()
}

fn avatar() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).unwrap()).unwrap()
}

/// A test project's pieces and confirmed seams; one with none sewn (the
/// pocket skirt) sewn by the seams Find seams suggests.
fn project(name: &str) -> (Vec<drapewright_pattern_core::Piece>, Vec<Seam>) {
    let path = format!(
        "{}/../../fixtures/drapewright/{name}.drapewright",
        env!("CARGO_MANIFEST_DIR")
    );
    let file = load(&unpack(&std::fs::read(path).unwrap()).unwrap().project_json).unwrap();
    let pieces: Vec<drapewright_pattern_core::Piece> =
        file.document.pieces.iter().map(|p| (**p).clone()).collect();
    let mut seams: Vec<Seam> = file
        .document
        .seams
        .iter()
        .filter(|s| s.confirmed)
        .cloned()
        .collect();
    if seams.is_empty() {
        seams = drapewright_seams::suggest(&pieces, &[])
            .into_iter()
            .enumerate()
            .map(|(k, s)| Seam {
                id: format!("s{k}"),
                confirmed: true,
                ..s.seam
            })
            .collect();
    }
    (pieces, seams)
}

/// A drape of `name` on the standard figure, its cloth `spacing` mm apart,
/// moved on by `card` if given; its points nudged by `nudge` if given.
fn drape(
    model: &BodyModel,
    name: &str,
    spacing: f64,
    card: Option<&(wgpu::Device, wgpu::Queue)>,
    nudge: Option<u64>,
) -> Drape {
    let (pieces, seams) = project(name);
    let body = model.shape(&Shape::default());
    let figure = Figure::of(model, &body);
    let draped = draped_pieces(&pieces, &seams);
    let shell = shell(&draped, &seams, spacing).unwrap();
    let starts = place(&shell, &figure);
    let (garment, starts) =
        with_inside(&shell, &starts, &draped, &seams, spacing).unwrap_or((shell, starts));
    let mut drape = Drape::new(
        &garment,
        &starts,
        preset("poplin").unwrap(),
        (&body.vertices, &model.skin()),
    );
    drape.posable(Posable::of(model, &body).unwrap());
    if let Some(seed) = nudge {
        nudged(&mut drape, seed);
    }
    if let Some((device, queue)) = card {
        let gpu = GpuCloth::new(device, queue, &drape.cloth).unwrap();
        eprintln!("colours (edges, bends, stitches): {:?}", gpu.colours());
        drape.use_stepper(Box::new(gpu));
    }
    drape
}

/// Each point moved up to a millimetre each way, differently for each
/// `seed`: a start as near the same as makes no difference to how the
/// garment should hang, but a different path there.
fn nudged(drape: &mut Drape, seed: u64) {
    let mut h = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    for p in &mut drape.cloth.x {
        for c in p.iter_mut() {
            // Xorshift.
            h ^= h << 13;
            h ^= h >> 7;
            h ^= h << 17;
            *c += ((h % 2001) as f32 - 1000.0) * 1e-6;
        }
    }
}

/// Steps until settled, at most `seconds` of the cloth's time; how long
/// that took here.
fn settle(drape: &mut Drape, seconds: f32) -> f64 {
    let start = Instant::now();
    let until = drape.time + seconds;
    while (drape.moving() || !drape.settled()) && drape.time < until {
        drape.step(1.0 / 60.0);
        drape.catch_up();
    }
    start.elapsed().as_secs_f64()
}

fn lowest(drape: &Drape) -> f32 {
    drape.cloth.x.iter().map(|p| p[1]).fold(f32::MAX, f32::min)
}

fn deepest(drape: &Drape) -> f32 {
    drape
        .cloth
        .x
        .iter()
        .map(|&p| drape.clearance(p))
        .fold(f32::MAX, f32::min)
}

#[test]
fn the_skirt_hangs_alike_on_the_card_and_the_processor() {
    let Some(card) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    let model = avatar();
    let mut cpu = drape(&model, "skirt", 20.0, None, None);
    let cpu_took = settle(&mut cpu, 10.0);
    let mut gpu = drape(&model, "skirt", 20.0, Some(&card), None);
    let gpu_took = settle(&mut gpu, 10.0);
    eprintln!(
        "processor: {:.1} s of cloth in {cpu_took:.2} s, hem {:.3} m; card: {:.1} s in {gpu_took:.2} s, hem {:.3} m, widest stitch {:.1} mm, nearest the body {:.1} mm",
        cpu.time,
        lowest(&cpu),
        gpu.time,
        lowest(&gpu),
        gpu.cloth.widest_stitch() * 1000.0,
        deepest(&gpu) * 1000.0,
    );
    assert!(gpu.settled(), "still moving on the card");
    assert!(gpu.cloth.widest_stitch() < 0.005);
    assert!(deepest(&gpu) > 0.0);
    assert!((lowest(&gpu) - lowest(&cpu)).abs() < 0.015);
}

#[test]
fn a_lined_skirt_and_a_shirt_drape_on_the_card_and_pose() {
    let Some(card) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    let model = avatar();
    for (name, pose) in [("lined-skirt", Pose::Stepping), ("shirt", Pose::Reaching)] {
        let mut gpu = drape(&model, name, 12.0, Some(&card), None);
        let took = settle(&mut gpu, 10.0);
        let points = gpu.cloth.x.len();
        gpu.pose(pose);
        let posed = settle(&mut gpu, 10.0);
        eprintln!(
            "{name}: {points} points; settled in {took:.2} s, posed in {posed:.2} s; widest stitch {:.1} mm; nearest the body {:.1} mm",
            gpu.cloth.widest_stitch() * 1000.0,
            deepest(&gpu) * 1000.0
        );
        assert!(gpu.settled() && !gpu.moving(), "{name} still moving");
        assert!(gpu.cloth.widest_stitch() < 0.005, "{name}");
        assert!(deepest(&gpu) > -0.002, "{name}");
    }
}

#[test]
fn the_card_moves_the_cloth_the_same_way_every_time() {
    let Some(card) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    // A lining's cloth too, half a second in: still moving fast, and
    // pressed against the body and the skirt over it.
    let model = avatar();
    let half_a_second = || {
        let mut gpu = drape(&model, "lined-skirt", 12.0, Some(&card), None);
        for _ in 0..30 {
            gpu.step(1.0 / 60.0);
            gpu.catch_up();
        }
        gpu.cloth.x
    };
    let (first, again) = (half_a_second(), half_a_second());
    // Bit for bit.
    let differ = first
        .iter()
        .zip(&again)
        .filter(|(a, b)| a.map(f32::to_bits) != b.map(f32::to_bits))
        .count();
    assert_eq!(differ, 0, "{differ} of {} points elsewhere", first.len());
}

/// How a drape on the card is waited for.
#[derive(Clone, Copy, Debug)]
enum Waiting {
    /// After every frame, as the tests here do.
    EveryFrame,
    /// Not at all: asked as often as it can be whether the next frame can
    /// be taken.
    Eagerly,
    /// Not at all: as many frames as can be taken, then a pause of up to
    /// 6 ms, as a page's frames go by in the browser.
    NowAndThen,
}

/// The garment `name` settled on the card, moved into a pose and settled
/// again, waited for so.
fn waited(
    model: &BodyModel,
    card: &(wgpu::Device, wgpu::Queue),
    name: &str,
    waiting: Waiting,
) -> Drape {
    let mut gpu = drape(model, name, 12.0, Some(card), None);
    let mut h = 0x9E37_79B9_7F4A_7C15u64;
    for pose in [None, Some(Pose::Stepping)] {
        if let Some(pose) = pose {
            gpu.pose(pose);
        }
        let until = gpu.time + 6.0;
        // As in the browser: done as soon as a frame leaves it settled.
        let going = |gpu: &Drape| (gpu.moving() || !gpu.settled()) && gpu.time < until;
        while going(&gpu) {
            match waiting {
                Waiting::EveryFrame => {
                    gpu.step(1.0 / 60.0);
                    gpu.catch_up();
                }
                Waiting::Eagerly => {
                    if gpu.ready() {
                        gpu.step(1.0 / 60.0);
                    }
                }
                Waiting::NowAndThen => {
                    while gpu.ready() && going(&gpu) {
                        gpu.step(1.0 / 60.0);
                    }
                    // Xorshift.
                    h ^= h << 13;
                    h ^= h >> 7;
                    h ^= h << 17;
                    std::thread::sleep(std::time::Duration::from_micros(h % 6000));
                }
            }
        }
    }
    gpu
}

#[test]
fn the_card_drapes_alike_however_long_its_cloth_takes_to_come_back() {
    let Some(card) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    // In the browser nothing can be waited for: the cloth's points come
    // back as they do, and the next frame is taken when it can be. What is
    // worked out from them (which of the lining's points are kept behind
    // which of the skirt's, which of a pocket's in front of which, when it
    // has settled, the body's field as it moves) comes from the same frames
    // however long that takes.
    let model = avatar();
    for name in ["lined-skirt", "pocket-skirt"] {
        let mut drapes = [Waiting::EveryFrame, Waiting::Eagerly, Waiting::NowAndThen]
            .map(|waiting| {
                let start = Instant::now();
                let gpu = waited(&model, &card, name, waiting);
                eprintln!(
                    "{name}, {waiting:?}: {:.2} s of cloth in {:.2} s",
                    gpu.time,
                    start.elapsed().as_secs_f64()
                );
                gpu
            })
            .into_iter();
        let first = drapes.next().unwrap();
        assert!(first.settled(), "{name} still moving");
        for again in drapes {
            assert_eq!(
                again.time.to_bits(),
                first.time.to_bits(),
                "{name} settled at another frame"
            );
            // Bit for bit.
            let differ = first
                .cloth
                .x
                .iter()
                .zip(&again.cloth.x)
                .filter(|(a, b)| a.map(f32::to_bits) != b.map(f32::to_bits))
                .count();
            assert_eq!(
                differ,
                0,
                "{name}: {differ} of {} points elsewhere",
                first.cloth.x.len()
            );
        }
    }
}

/// The share of the points sewn on (pockets) not in front of the cloth
/// they lie on (the panel before them): no further from the body than its
/// nearest point, where they are on it.
fn behind(drape: &Drape, under: std::ops::Range<usize>) -> f32 {
    let x = &drape.cloth.x;
    let d = |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
    let near: Vec<usize> = (0..x.len())
        .filter(|&i| drape.cloth.sewn_on(i))
        .filter(|&i| under.clone().any(|j| d(x[i], x[j]) < 0.01))
        .collect();
    let wrong = near
        .iter()
        .filter(|&&i| {
            let j = under
                .clone()
                .min_by(|&a, &b| d(x[i], x[a]).total_cmp(&d(x[i], x[b])))
                .expect("cloth under");
            drape.clearance(x[i]) < drape.clearance(x[j]) - 0.0005
        })
        .count();
    wrong as f32 / near.len().max(1) as f32
}

#[test]
fn the_pockets_drape_over_the_skirt_on_the_card_as_on_the_processor() {
    let Some(card) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    let model = avatar();
    let mut cpu = drape(&model, "pocket-skirt", 20.0, None, None);
    settle(&mut cpu, 10.0);
    let mut gpu = drape(&model, "pocket-skirt", 20.0, Some(&card), None);
    let took = settle(&mut gpu, 10.0);
    // The front first, then the back's two copies, then the pockets.
    let front = 0..gpu.firsts[1] as usize;
    let pockets = gpu.firsts[3] as usize..gpu.cloth.x.len();
    let middle = |drape: &Drape| {
        let n = pockets.len() as f32;
        pockets.clone().fold([0.0f32; 3], |m, k| {
            let p = drape.cloth.x[k];
            [m[0] + p[0] / n, m[1] + p[1] / n, m[2] + p[2] / n]
        })
    };
    let (a, b) = (middle(&cpu), middle(&gpu));
    eprintln!(
        "card: settled in {took:.2} s; widest stitch {:.1} mm; {:.4} of the pockets behind the front; pockets' middle {b:?} (processor {a:?})",
        gpu.cloth.widest_stitch() * 1000.0,
        behind(&gpu, front.clone()),
    );
    assert!(pockets.clone().all(|k| gpu.cloth.sewn_on(k)));
    assert!(gpu.settled(), "still moving on the card");
    assert!(gpu.cloth.widest_stitch() < 0.005);
    assert!(behind(&gpu, front) < 0.02);
    assert!((0..3).all(|k| (a[k] - b[k]).abs() < 0.01), "{a:?} {b:?}");
    // Posed, still over the front.
    gpu.pose(Pose::Stepping);
    settle(&mut gpu, 10.0);
    assert!(gpu.settled() && !gpu.moving(), "still moving, stepping");
    assert!(gpu.cloth.widest_stitch() < 0.005);
    assert!(behind(&gpu, 0..gpu.firsts[1] as usize) < 0.02);
}

#[test]
fn the_lined_skirt_settles_on_the_card_however_it_starts() {
    let Some(card) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    // Each start nudged a little: another path to the same skirt. Before,
    // about one start in twenty-five folded a hem corner over by a side
    // seam as it first swung, the lining was pushed out through it, and
    // the two layers tangled, never still and pressed into the body.
    let model = avatar();
    for seed in 1..=6 {
        let mut gpu = drape(&model, "lined-skirt", 12.0, Some(&card), Some(seed));
        let took = settle(&mut gpu, 10.0);
        eprintln!(
            "start {seed}: {:.2} s of cloth in {took:.2} s; nearest the body {:.1} mm",
            gpu.time,
            deepest(&gpu) * 1000.0
        );
        assert!(gpu.settled(), "start {seed}: still moving");
        assert!(deepest(&gpu) > 0.0, "start {seed}: into the body");
    }
}

/// A rectangle `w` by `h` m, meshed with points about `spacing` m apart,
/// laid flat.
fn rectangle(w: f64, h: f64, spacing: f64) -> (PanelMesh, Vec<[f32; 2]>) {
    let side = |a: [f64; 2], b: [f64; 2]| {
        let n = ((b[0] - a[0]).hypot(b[1] - a[1]) / spacing)
            .round()
            .max(1.0) as usize;
        (0..=n)
            .map(|k| {
                let t = k as f64 / n as f64;
                [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])]
            })
            .collect::<Vec<_>>()
    };
    let m = mesh(
        &[
            side([0.0, 0.0], [w, 0.0]),
            side([w, 0.0], [w, h]),
            side([w, h], [0.0, h]),
            side([0.0, h], [0.0, 0.0]),
        ],
        spacing,
    )
    .unwrap();
    let rest = m
        .points
        .iter()
        .map(|p| [p[0] as f32, p[1] as f32])
        .collect();
    (m, rest)
}

#[test]
fn a_lining_is_not_pushed_through_cloth_folded_over_it_on_the_card() {
    let Some((device, queue)) = card() else {
        eprintln!("no graphics card: skipped");
        return;
    };
    // As on the processor (crates/sim/tests/cloth.rs): cloth 9 cm across
    // in front of a body (a box whose face is 1 cm behind it), a lining
    // 2 cm across 2 mm behind it, and the cloth's right-hand 3 cm folded
    // back over in front of the lining, 4 mm out.
    let (outside, rest) = rectangle(0.09, 0.06, 0.01);
    let (inside, lining) = rectangle(0.02, 0.02, 0.01);
    let mut builder = ClothBuilder::new(preset("poplin").unwrap());
    let start: Vec<V3> = rest.iter().map(|p| [p[0], p[1], 0.0]).collect();
    let a = builder.panel(&rest, &start, &outside.triangles);
    let behind: Vec<V3> = lining
        .iter()
        .map(|p| [p[0] + 0.035, p[1] + 0.02, -0.002])
        .collect();
    let b = builder.panel(&lining, &behind, &inside.triangles);
    builder.inside_from(b);
    builder.lies_on(b..b + lining.len() as u32, a..b);
    let mut cloth = builder.build();
    cloth.gravity = [0.0; 3];
    let (corners, faces) = Seat {
        low: [-0.05, -0.05, -0.06],
        high: [0.14, 0.11, -0.01],
    }
    .mesh();
    let body = BodyField::new(&corners, &faces, 0.005, 0.03);
    cloth.face_out(&body);
    for p in &mut cloth.x[..b as usize] {
        if p[0] > 0.06 + 1e-6 {
            *p = [0.12 - p[0], p[1], 0.004];
        }
    }
    let mut gpu = GpuCloth::new(&device, &queue, &cloth).unwrap();
    let felt = Felt {
        body: &body,
        towards: None,
        seat: None,
    };
    gpu.step(&mut cloth, 1.0 / 1800.0, 1, &felt);
    let front = gpu.points_now()[b as usize..]
        .iter()
        .map(|p| p[2])
        .fold(f32::MIN, f32::max);
    assert!(front < 0.0, "the lining {:.1} mm in front", front * 1000.0);
}
