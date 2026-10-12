//! Fabrics of one's own in a project (schema version 6): measured at home,
//! kept with the project, changed and removed like anything else.

use drapewright_fabric::{FabricPreset, Measured, OwnFabric};
use drapewright_project::{Command, CommandError, Engine, SCHEMA_VERSION};

fn project() -> Engine {
    Engine::new("prj", "Test", "2026-10-12T00:00:00Z", "test")
}

fn linen(weight: f64) -> OwnFabric {
    OwnFabric {
        id: String::new(),
        name: "Blue linen".into(),
        like: FabricPreset::Linen,
        measured: Measured {
            weight: Some(weight),
            ..Default::default()
        },
    }
}

#[test]
fn a_fabric_is_added_changed_and_removed() {
    let mut e = project();
    e.dispatch(
        Command::AddFabric {
            fabric: linen(185.0),
        },
        Default::default(),
    )
    .unwrap();
    let fabric = e.document().fabrics[0].clone();
    assert!(!fabric.id.is_empty());
    assert!((fabric.fabric().density - 0.185).abs() < 1e-6);

    let mut heavier = fabric.clone();
    heavier.measured.weight = Some(240.0);
    e.dispatch(
        Command::SetFabric {
            fabric: heavier.clone(),
        },
        Default::default(),
    )
    .unwrap();
    assert_eq!(e.document().fabrics, [heavier]);
    e.undo().unwrap();
    assert_eq!(e.document().fabrics, std::slice::from_ref(&fabric));

    e.dispatch(
        Command::DeleteFabric {
            id: fabric.id.clone(),
        },
        Default::default(),
    )
    .unwrap();
    assert!(e.document().fabrics.is_empty());
    assert!(matches!(
        e.dispatch(Command::DeleteFabric { id: fabric.id }, Default::default()),
        Err(CommandError::UnknownFabric { .. })
    ));
}

#[test]
fn a_fabric_needs_a_name_and_measurements_that_can_be() {
    let mut e = project();
    let refused = |e: &mut Engine, fabric: OwnFabric| {
        matches!(
            e.dispatch(Command::AddFabric { fabric }, Default::default()),
            Err(CommandError::Rejected { .. })
        )
    };
    assert!(refused(
        &mut e,
        OwnFabric {
            name: " ".into(),
            ..linen(185.0)
        }
    ));
    assert!(refused(&mut e, linen(3.0)));
    assert!(e.document().fabrics.is_empty());
}

#[test]
fn fabrics_are_kept_with_the_project() {
    let mut e = project();
    e.dispatch(
        Command::AddFabric {
            fabric: linen(185.0),
        },
        Default::default(),
    )
    .unwrap();
    let json = e.to_json("2026-10-12T10:00:00Z");
    assert!(json.contains(&format!("\"schemaVersion\": {SCHEMA_VERSION}")));
    let (again, issues) = Engine::open(&json, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert_eq!(again.document().fabrics, e.document().fabrics);
}

#[test]
fn a_version_5_project_opens_with_no_fabrics() {
    let v5 = include_str!("fixtures/project-v5.json");
    assert!(v5.contains("\"schemaVersion\": 5"));
    let (e, issues) = Engine::open(v5, "test").unwrap();
    assert!(issues.is_empty(), "{issues:?}");
    assert!(e.document().fabrics.is_empty());
    assert_eq!(e.document().pieces.len(), 2);
}
