//! The project's garment draped on the avatar, as the app asks for it.

use drapewright_body::{BodyModel, Shape};
use drapewright_project::{Engine, container};
use drapewright_sim::fabric::PRESETS;
use drapewright_wasm_3d::drape::{DrapeError, Draping};
use drapewright_wasm_3d::{DrapeRequest, DrapeStage, FabricPreset};

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
        settings: Shape::default().into(),
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
fn every_fabric_preset_is_the_simulators() {
    let names: Vec<&str> = FabricPreset::ALL.iter().map(|f| f.name()).collect();
    let presets: Vec<&str> = PRESETS.iter().map(|(n, _)| *n).collect();
    assert_eq!(names, presets);
    // Named in JavaScript as in the simulator.
    for f in FabricPreset::ALL {
        assert_eq!(
            serde_json::to_string(&f).unwrap(),
            format!("\"{}\"", f.name())
        );
    }
}
