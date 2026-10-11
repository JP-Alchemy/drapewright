//! People's measurements in a project (schema version 2), and opening the
//! projects saved before them.

use std::collections::BTreeMap;

use drapewright_project::measurements::{EarlierMeasurements, vocabulary};
use drapewright_project::{
    Command, CommandError, Engine, Figure, MeasurementKey, MeasurementSet, SCHEMA_VERSION,
};

fn engine() -> Engine {
    Engine::new("prj", "Test", "2026-10-11T00:00:00Z", "test")
}

fn me(values: &[(&str, f64)]) -> MeasurementSet {
    MeasurementSet {
        id: String::new(),
        name: "Me".into(),
        figure: Figure::Woman,
        revision: 1,
        taken_on: Some("2026-10-01".into()),
        age: Some(40),
        values: values.iter().map(|(k, v)| (k.to_string(), *v)).collect(),
        previous: Vec::new(),
    }
}

#[test]
fn a_version_3_project_opens_with_no_ages() {
    let v3 = include_str!("fixtures/project-v3.json");
    assert!(v3.contains("\"schemaVersion\": 3"));
    let (e, issues) = Engine::open(v3, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    let me = &e.document().measurement_sets[0];
    assert_eq!((me.name.as_str(), me.age), ("Me", None));
    let saved = e.to_json("2026-10-12T10:00:00Z");
    assert!(saved.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
    assert!(saved.contains("\"age\": null"));
}

#[test]
fn a_version_1_project_opens_with_no_measurements() {
    let v1 = include_str!("fixtures/project-v1.json");
    assert!(v1.contains("\"schemaVersion\": 1"));
    let (e, issues) = Engine::open(v1, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(e.document().pieces.len(), 1);
    assert!(e.document().measurement_sets.is_empty());
    let saved = e.to_json("2026-10-11T10:00:00Z");
    assert!(saved.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
    assert!(saved.contains("\"measurementSets\": []"));
}

#[test]
fn measurements_are_added_changed_kept_and_undone() {
    let mut e = engine();
    let set = e
        .dispatch(
            Command::AddMeasurementSet {
                set: me(&[("bust", 960.0), ("waist", 780.0)]),
            },
            Default::default(),
        )
        .unwrap();
    assert!(set.document);
    let added = e.document().measurement_sets[0].clone();
    assert!(added.id.starts_with('m'), "{}", added.id);
    assert_eq!(added.get(MeasurementKey::Bust), Some(960.0));

    // Changed in place: same revision.
    let mut changed = added.clone();
    changed.values.insert("hip".into(), 1040.0);
    e.dispatch(
        Command::SetMeasurementSet { set: changed },
        Default::default(),
    )
    .unwrap();
    assert_eq!(e.document().measurement_sets[0].values.len(), 3);

    // Measured again: the old values are kept, dated, newest first.
    e.dispatch(
        Command::RemeasureSet {
            id: added.id.clone(),
            taken_on: Some("2026-12-01".into()),
        },
        Default::default(),
    )
    .unwrap();
    let again = &e.document().measurement_sets[0];
    assert_eq!(again.revision, 2);
    assert_eq!(again.taken_on.as_deref(), Some("2026-12-01"));
    assert_eq!(
        again.previous,
        vec![EarlierMeasurements {
            revision: 1,
            taken_on: Some("2026-10-01".into()),
            values: BTreeMap::from([
                ("bust".into(), 960.0),
                ("hip".into(), 1040.0),
                ("waist".into(), 780.0)
            ]),
        }]
    );

    // They are saved and opened again.
    let (opened, _) = Engine::open(&e.to_json("now"), "test").unwrap();
    assert_eq!(
        opened.document().measurement_sets,
        e.document().measurement_sets
    );

    e.undo();
    assert_eq!(e.document().measurement_sets[0].revision, 1);
    let delete = || Command::DeleteMeasurementSet {
        id: added.id.clone(),
    };
    e.dispatch(delete(), Default::default()).unwrap();
    assert!(e.document().measurement_sets.is_empty());
    assert_eq!(
        e.dispatch(delete(), Default::default()).unwrap_err(),
        CommandError::UnknownMeasurementSet {
            id: added.id.clone()
        }
    );
}

#[test]
fn impossible_measurements_are_refused() {
    let mut e = engine();
    for bad in [
        me(&[("waist", 0.0)]),
        me(&[("waist", f64::NAN)]),
        me(&[("wiast", 780.0)]),
        MeasurementSet {
            name: "  ".into(),
            ..me(&[])
        },
        MeasurementSet {
            age: Some(200),
            ..me(&[])
        },
    ] {
        assert!(matches!(
            e.dispatch(Command::AddMeasurementSet { set: bad }, Default::default()),
            Err(CommandError::Rejected { .. })
        ));
    }
    assert!(e.document().measurement_sets.is_empty());
    // Custom measurements are fine.
    e.dispatch(
        Command::AddMeasurementSet {
            set: me(&[("x-elbow", 260.0)]),
        },
        Default::default(),
    )
    .unwrap();
}

#[test]
fn ids_never_collide_with_pieces() {
    let mut e = engine();
    e.dispatch(
        Command::AddMeasurementSet { set: me(&[]) },
        Default::default(),
    )
    .unwrap();
    let json = e.to_json("now");
    let (mut opened, _) = Engine::open(&json, "test").unwrap();
    opened
        .dispatch(
            Command::AddMeasurementSet { set: me(&[]) },
            Default::default(),
        )
        .unwrap();
    let ids: Vec<&str> = opened
        .document()
        .measurement_sets
        .iter()
        .map(|m| m.id.as_str())
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
}

#[test]
fn the_vocabulary_maps_to_freesewing() {
    let names: Vec<(String, String)> = vocabulary()
        .into_iter()
        .filter_map(|m| m.freesewing.map(|f| (m.key.name(), f)))
        .collect();
    assert!(names.contains(&("bust".into(), "chest".into())));
    assert!(names.contains(&("hip".into(), "seat".into())));
    assert!(names.contains(&("highHip".into(), "hips".into())));
    assert_eq!(names.len(), 21);
}
