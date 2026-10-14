//! The project's garment draped on the avatar, as the app asks for it.

use drapewright_body::{BodyModel, Shape};
use drapewright_project::{Engine, container};
use drapewright_wasm_3d::drape::Measured;
use drapewright_wasm_3d::drape::{DrapeError, Draping};
use drapewright_wasm_3d::{AvatarPose, ClothShows, DrapeRequest, DrapeStage, FabricPreset};

fn model() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).expect("the body file")).expect("a body file")
}

/// A test garment's project (`fixtures/drapewright/{name}.drapewright`),
/// as the app sends its pieces and seams.
fn garment(name: &str) -> DrapeRequest {
    let path = format!(
        "{}/../../fixtures/drapewright/{name}.drapewright",
        env!("CARGO_MANIFEST_DIR")
    );
    let unpacked = container::unpack(&std::fs::read(path).unwrap()).unwrap();
    let (engine, _) = Engine::open(&unpacked.project_json, "test").unwrap();
    let doc = engine.document();
    DrapeRequest {
        pieces: doc.pieces.iter().map(|p| (**p).clone()).collect(),
        seams: doc.seams.clone(),
        fabric: FabricPreset::Poplin,
        measured: None,
        settings: Shape::default().into(),
        pose: None,
    }
}

/// Drapes `request` until it is done, checking as it goes; its draping.
fn drape_until_done(request: &DrapeRequest) -> Draping {
    let mut draping = Draping::start(&model(), request).unwrap();
    assert!(draping.left_out.is_empty());
    let mut progress = draping.progress();
    assert_eq!(progress.stage, DrapeStage::Sewing);
    let mut frames = 0;
    while !matches!(progress.stage, DrapeStage::Settled | DrapeStage::Stopped) {
        progress = draping.step();
        frames += 1;
        assert!(frames < 60 * 30, "never done");
    }
    assert_eq!(progress.stage, DrapeStage::Settled, "{progress:?}");
    assert!(progress.widest_gap < 5.0, "{progress:?}");
    draping
}

#[test]
fn a_blouse_drapes_with_its_sleeves() {
    let draping = drape_until_done(&garment("blouse"));
    // Out along the arms, both sides.
    let reach = draping
        .points()
        .iter()
        .map(|p| p[0].abs())
        .fold(0.0, f32::max);
    assert!(reach > 250.0, "reaches {reach} mm out");
}

#[test]
fn a_skirt_asked_for_sitting_sits_once_draped_and_stands_again() {
    let request = DrapeRequest {
        pose: Some(AvatarPose::Sitting),
        ..garment("skirt")
    };
    let mut draping = Draping::start(&model(), &request).unwrap();
    let mut stages = Vec::new();
    for _ in 0..60 * 30 {
        let stage = draping.step().stage;
        if stages.last() != Some(&stage) {
            stages.push(stage);
        }
        if matches!(stage, DrapeStage::Settled | DrapeStage::Stopped) {
            break;
        }
    }
    assert_eq!(
        stages,
        [
            DrapeStage::Sewing,
            DrapeStage::Settling,
            DrapeStage::Posing,
            DrapeStage::Settling,
            DrapeStage::Settled
        ]
    );
    // Sitting: the knees forward, well in front of the toes standing (the
    // body below the hands).
    let knee = |d: &Draping| {
        let (_, body) = d.body().unwrap();
        body.iter()
            .filter(|p| p[1] < 700.0)
            .map(|p| p[2])
            .fold(f32::MIN, f32::max)
    };
    assert!(
        knee(&draping) > 400.0,
        "the knees {} mm forward",
        knee(&draping)
    );
    // Standing again, asked once settled.
    draping.pose(AvatarPose::Standing);
    assert_eq!(draping.progress().stage, DrapeStage::Posing);
    while !matches!(
        draping.step().stage,
        DrapeStage::Settled | DrapeStage::Stopped
    ) {}
    assert_eq!(draping.progress().stage, DrapeStage::Settled);
    assert!(
        knee(&draping) < 250.0,
        "the toes {} mm forward",
        knee(&draping)
    );
}

#[test]
fn the_skirt_drapes_until_it_has_settled() {
    let mut draping = drape_until_done(&garment("skirt"));
    let progress = draping.progress();
    // Done: stepping on changes nothing.
    let points = draping.points();
    assert_eq!(draping.step(), progress);
    assert_eq!(draping.points(), points);
    // The cloth in mm, round the avatar's waist.
    let top = points.iter().map(|p| p[1]).fold(f32::MIN, f32::max);
    assert!((1000.0..1300.0).contains(&top), "top {top}");
    assert!(
        draping
            .triangles()
            .iter()
            .flatten()
            .all(|&i| (i as usize) < points.len())
    );
}

#[test]
fn pieces_sewn_to_nothing_are_left_out() {
    let mut request = garment("skirt");
    let mut pocket = request.pieces[0].clone();
    pocket.id = "pocket".into();
    request.pieces.push(pocket);
    let draping = Draping::start(&model(), &request).unwrap();
    assert_eq!(draping.left_out, ["pocket"]);
}

#[test]
fn nothing_is_draped_until_something_is_sewn() {
    let mut request = garment("skirt");
    // Seams suggested but not confirmed are not sewn.
    for seam in &mut request.seams {
        seam.confirmed = false;
    }
    assert!(matches!(
        Draping::start(&model(), &request),
        Err(DrapeError::NothingSewn)
    ));
}

#[test]
fn every_fabric_preset_is_named_alike_in_javascript_and_the_simulator() {
    for f in FabricPreset::ALL {
        assert_eq!(
            serde_json::to_string(&f).unwrap(),
            format!("\"{}\"", f.name())
        );
        assert_eq!(drapewright_sim::fabric::preset(f.name()), Some(f.fabric()));
    }
}

#[test]
fn a_skirt_drapes_in_a_fabric_measured_at_home() {
    // Like poplin, but heavier and stretching like a knit across.
    let request = DrapeRequest {
        measured: Some(Measured {
            weight: Some(260.0),
            crosswise: Some(25.0),
            overhang: Some(5.0),
            ..Default::default()
        }),
        ..garment("skirt")
    };
    let draping = drape_until_done(&request);
    assert_eq!(draping.progress().stage, DrapeStage::Settled);
}

#[test]
fn the_cloth_is_read_where_it_is_looked_at() {
    let state = drape_until_done(&garment("skirt")).state();
    // Looking straight at the front of the skirt, below the waist.
    let front = state
        .pick([0.0, 1000.0, 3000.0], [0.0, 0.0, -1.0])
        .expect("the skirt's front");
    assert!((0.0..40.0).contains(&front.room), "{front:?}");
    assert!(front.pull >= 0.0 && front.stretch >= 0.0, "{front:?}");
    // From behind, the back.
    assert!(
        state
            .pick([0.0, 1000.0, -3000.0], [0.0, 0.0, 1.0])
            .is_some()
    );
    // Above it, at the chest: no cloth there.
    assert!(
        state
            .pick([0.0, 1300.0, 3000.0], [0.0, 0.0, -1.0])
            .is_none()
    );
    // Shown as fabric, all alike; as heatmaps, varying.
    let alike = |a: [f32; 3], b: [f32; 3]| (0..3).all(|k| (a[k] - b[k]).abs() < 1e-6);
    let fabric = [0.3, 0.4, 0.5];
    assert!(
        state
            .colours(ClothShows::Fabric, fabric)
            .iter()
            .all(|&c| alike(c, fabric))
    );
    for shows in [ClothShows::Pull, ClothShows::Room] {
        let colours = state.colours(shows, fabric);
        assert_eq!(colours.len(), state.points.len());
        assert!(colours.iter().any(|&c| !alike(c, colours[0])), "{shows:?}");
    }
}

#[test]
fn a_blouse_settles_after_each_pose() {
    let mut draping = drape_until_done(&garment("blouse"));
    for pose in [
        AvatarPose::Reaching,
        AvatarPose::ArmsDown,
        AvatarPose::ArmsUp,
    ] {
        draping.pose(pose);
        let mut progress = draping.progress();
        let mut frames = 0;
        while !matches!(progress.stage, DrapeStage::Settled | DrapeStage::Stopped) {
            progress = draping.step();
            frames += 1;
        }
        eprintln!("{pose:?}: {:?} after {frames} frames", progress.stage);
        assert_eq!(progress.stage, DrapeStage::Settled, "{pose:?}");
    }
}
