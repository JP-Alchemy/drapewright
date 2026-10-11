//! The avatar as the app uses it: fitted to a measurement set.

use std::collections::BTreeMap;

use drapewright_body::BodyModel;
use drapewright_project::measurements::MeasurementKey;
use drapewright_wasm_3d::{AvatarRequest, fit_avatar};

fn model() -> BodyModel {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    BodyModel::from_bytes(&std::fs::read(path).expect("the body file")).expect("a body file")
}

#[test]
fn the_avatar_takes_every_measurement_in_the_vocabulary() {
    let request: AvatarRequest = serde_json::from_str(
        r#"{"woman": true, "age": 40, "values": {"height": 1650, "bust": 920, "waist": 740, "hip": 990, "x-cuff": 240}}"#,
    )
    .expect("a request as the app sends it");
    let fitted = fit_avatar(&model(), &request);
    for key in MeasurementKey::ALL {
        assert!(
            fitted.measured.contains_key(&key.name()),
            "the avatar does not take {}",
            key.name()
        );
    }
    // Every measurement given that the avatar takes is reported on; a
    // measurement of the seamstress's own is not.
    let reported: Vec<&str> = fitted.residuals.keys().map(String::as_str).collect();
    assert_eq!(reported, ["bust", "height", "hip", "waist"]);
    for (k, r) in &fitted.residuals {
        assert!(r.abs() < 10.0, "{k} off by {r:.1} mm");
    }
}

#[test]
fn ages_outside_the_avatars_are_taken_as_its_nearest() {
    let request = AvatarRequest {
        woman: false,
        age: 18.0,
        values: BTreeMap::from([("height".to_string(), 1800.0)]),
    };
    let fitted = fit_avatar(&model(), &request);
    assert!((fitted.settings.age - 25.0).abs() < 1e-9);
    assert!(fitted.settings.woman.abs() < 1e-9);
}
