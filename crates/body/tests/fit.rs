//! The body file in the repository: figures shaped, measured as a tape
//! would, and fitted to measurements.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use drapewright_body::{BodyModel, Shape, fit, measure};

fn model() -> &'static BodyModel {
    static MODEL: OnceLock<BodyModel> = OnceLock::new();
    MODEL.get_or_init(|| {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../assets/body/adult.dwbody"
        );
        BodyModel::from_bytes(&std::fs::read(path).expect("the body file")).expect("a body file")
    })
}

fn cm(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v * 10.0))
        .collect()
}

#[test]
fn figures_measure_as_people_do() {
    let m = model();
    for (shape, figure) in [
        (Shape::default(), "woman"),
        (
            Shape {
                woman: 0.0,
                muscle: 0.6,
                ..Shape::default()
            },
            "man",
        ),
    ] {
        let (got, _) = measure(m, &m.shape(&shape));
        let get = |k: &str| {
            got.get(k)
                .copied()
                .unwrap_or_else(|| panic!("{figure}: no {k}"))
        };
        // Every measurement is taken, within its usual adult range (mm).
        for (k, lo, hi) in [
            ("height", 1500.0, 2000.0),
            ("bust", 750.0, 1200.0),
            ("waist", 550.0, 1000.0),
            ("hip", 800.0, 1200.0),
            ("neck", 300.0, 450.0),
            ("shoulderWidth", 320.0, 460.0),
            ("backWaistLength", 340.0, 500.0),
            ("armLength", 500.0, 700.0),
            ("inseam", 700.0, 950.0),
            ("wrist", 130.0, 190.0),
        ] {
            let v = get(k);
            assert!((lo..=hi).contains(&v), "{figure}: {k} {v:.0} mm");
        }
        assert!(
            get("waist") < get("bust") && get("waist") < get("hip"),
            "{figure}"
        );
    }
}

#[test]
fn round_the_bust_as_sewists_measure() {
    let m = model();
    let at = |weight: f64, cup: f64| {
        let shape = Shape {
            weight,
            cup,
            ..Shape::default()
        };
        measure(m, &m.shape(&shape))
    };
    let (average, marks) = at(0.5, 0.5);
    let (full, _) = at(0.5, 1.0);
    // The high bust passes above the breasts: a few cm under the bust for
    // an average cup, a good deal more for a full one.
    let gap = |g: &BTreeMap<&str, f64>| g["bust"] - g["highBust"];
    assert!(
        (10.0..=60.0).contains(&gap(&average)),
        "average cup: {:.0} mm",
        gap(&average)
    );
    assert!(
        gap(&full) > gap(&average) + 40.0,
        "full cup: {:.0} mm",
        gap(&full)
    );
    // The underbust at the fold under the breasts, the waist well below.
    let level = |k: &str| marks.levels[k];
    let fold = level("bust") - level("underbust");
    assert!((25.0..=80.0).contains(&fold), "fold {fold:.0} mm under");
    assert!(level("underbust") - level("waist") > 50.0);
    assert!(average["waist"] < average["underbust"] && average["underbust"] < average["bust"]);
    // A fuller middle that only widens from under the bust down: the waist
    // is still well below the fold.
    let (_, fuller) = at(1.0, 1.0);
    let below = fuller.levels["underbust"] - fuller.levels["waist"];
    assert!(below > 30.0, "waist {below:.0} mm under the fold");
}

#[test]
fn a_figure_is_found_again_from_its_measurements() {
    let m = model();
    let original = Shape {
        weight: 0.75,
        height: 0.3,
        cup: 0.7,
        local: BTreeMap::from([("measure-waist-circ-incr".to_string(), 0.4)]),
        ..Shape::default()
    };
    let (taken, _) = measure(m, &m.shape(&original));
    let targets: BTreeMap<String, f64> = ["height", "bust", "waist", "hip"]
        .iter()
        .map(|k| (k.to_string(), taken[k]))
        .collect();
    let fitted = fit(m, &targets, true, 35.0);
    for (k, r) in &fitted.residuals {
        assert!(r.abs() < 10.0, "{k} off by {r:.1} mm");
    }
}

/// Body measurements from a commercial pattern size chart (inches).
fn chart(height_cm: f64, bust: f64, waist: f64, hip: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("height".to_string(), height_cm * 10.0),
        ("bust".to_string(), bust * 25.4),
        ("waist".to_string(), waist * 25.4),
        ("hip".to_string(), hip * 25.4),
    ])
}

#[test]
fn a_pattern_size_chart_fits_from_small_to_large() {
    let m = model();
    for (size, bust, waist, hip) in [
        ("6", 30.5, 23.0, 32.5),
        ("14", 36.0, 28.0, 38.0),
        ("22", 44.0, 37.0, 46.0),
        ("30W", 52.0, 45.0, 54.0),
    ] {
        let fitted = fit(m, &chart(165.0, bust, waist, hip), true, 35.0);
        for (k, r) in &fitted.residuals {
            assert!(r.abs() < 10.0, "size {size}: {k} off by {r:.1} mm");
        }
    }
    let man = fit(m, &chart(178.0, 40.0, 34.0, 40.0), false, 35.0);
    for (k, r) in &man.residuals {
        assert!(r.abs() < 10.0, "man: {k} off by {r:.1} mm");
    }
}

#[test]
fn a_high_bust_out_of_reach_leaves_the_rest_fitted() {
    let m = model();
    // A B cup (the high bust 5 cm under the bust) at size 14 is reached.
    let mut targets = chart(165.0, 36.0, 28.0, 38.0);
    targets.insert("highBust".into(), 36.0 * 25.4 - 50.0);
    for (k, r) in &fit(m, &targets, true, 35.0).residuals {
        assert!(r.abs() < 10.0, "size 14: {k} off by {r:.1} mm");
    }
    // At 30W the figure's frame cannot be that broad: the high bust is
    // left short, and said to be, but the rest still fit.
    let mut targets = chart(165.0, 52.0, 45.0, 54.0);
    targets.insert("highBust".into(), 52.0 * 25.4 - 50.0);
    let fitted = fit(m, &targets, true, 35.0);
    for (k, r) in &fitted.residuals {
        if k == "highBust" {
            assert!(*r < -30.0, "the high bust is out of reach: {r:.1} mm");
        } else {
            assert!(r.abs() < 10.0, "size 30W: {k} off by {r:.1} mm");
        }
    }
}

#[test]
fn a_fuller_figure_with_many_measurements_fits() {
    let m = model();
    let targets = cm(&[
        ("height", 162.0),
        ("highBust", 94.0),
        ("bust", 104.0),
        ("underbust", 86.0),
        ("waist", 88.0),
        ("hip", 112.0),
        ("neck", 36.0),
        ("shoulderWidth", 39.0),
        ("backWaistLength", 40.0),
        ("armLength", 58.0),
        ("inseam", 74.0),
        ("thigh", 64.0),
    ]);
    // So many measurements pull against one another a little: the main ones
    // within a centimetre, the others within one and a half.
    let fitted = fit(m, &targets, true, 45.0);
    for (k, r) in &fitted.residuals {
        let main = ["height", "bust", "waist", "hip"].contains(&k.as_str());
        let allowed = if main { 10.0 } else { 15.0 };
        assert!(r.abs() < allowed, "{k} off by {r:.1} mm");
    }
}
