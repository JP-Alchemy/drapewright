//! The test garments draped on the graphics card, as on the processor.

use std::future::Future;
use std::time::Instant;

use drapewright_body::{BodyModel, Pose, Shape};
use drapewright_project::container::unpack;
use drapewright_project::migrate::load;
use drapewright_seams::Seam;
use drapewright_sim::drape::{Drape, Posable};
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::{assemble, draped_pieces};
use drapewright_sim::inside::with_inside;
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

/// A test project's pieces and confirmed seams.
fn project(name: &str) -> (Vec<drapewright_pattern_core::Piece>, Vec<Seam>) {
    let path = format!(
        "{}/../../fixtures/drapewright/{name}.drapewright",
        env!("CARGO_MANIFEST_DIR")
    );
    let file = load(&unpack(&std::fs::read(path).unwrap()).unwrap().project_json).unwrap();
    let pieces = file.document.pieces.iter().map(|p| (**p).clone()).collect();
    let seams = file
        .document
        .seams
        .iter()
        .filter(|s| s.confirmed)
        .cloned()
        .collect();
    (pieces, seams)
}

/// A drape of `name` on the standard figure, its cloth `spacing` mm apart,
/// moved on by `card` if given.
fn drape(
    model: &BodyModel,
    name: &str,
    spacing: f64,
    card: Option<&(wgpu::Device, wgpu::Queue)>,
) -> Drape {
    let (pieces, seams) = project(name);
    let body = model.shape(&Shape::default());
    let figure = Figure::of(model, &body);
    let draped = draped_pieces(&pieces, &seams);
    let shell = assemble(&draped.sewn, &seams, spacing).unwrap();
    let starts = place(&shell, &figure);
    let (garment, starts) = with_inside(
        &shell,
        &starts,
        (&draped.sewn, &draped.lined),
        &seams,
        spacing,
    )
    .unwrap_or((shell, starts));
    let mut drape = Drape::new(
        &garment,
        &starts,
        preset("poplin").unwrap(),
        (&body.vertices, &model.skin()),
    );
    drape.posable(Posable::of(model, &body).unwrap());
    if let Some((device, queue)) = card {
        let gpu = GpuCloth::new(device, queue, &drape.cloth).unwrap();
        eprintln!("colours (edges, bends, stitches): {:?}", gpu.colours());
        drape.use_stepper(Box::new(gpu));
    }
    drape
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
    let mut cpu = drape(&model, "skirt", 20.0, None);
    let cpu_took = settle(&mut cpu, 10.0);
    let mut gpu = drape(&model, "skirt", 20.0, Some(&card));
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
        let mut gpu = drape(&model, name, 12.0, Some(&card));
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
        assert!(gpu.cloth.widest_stitch() < 0.008, "{name}");
        assert!(deepest(&gpu) > -0.002, "{name}");
    }
}
