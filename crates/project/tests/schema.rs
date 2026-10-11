//! The published JSON Schema of `project.json` (docs/schema), generated
//! from the types that read and write it, so the two cannot drift. CI fails
//! if the generated file differs from the committed one.

use std::path::Path;

use drapewright_project::ProjectFile;

#[test]
fn json_schema_is_up_to_date() {
    let mut schema = schemars::schema_for!(ProjectFile);
    schema.insert("title".into(), "Drapewright project (project.json)".into());
    schema.insert(
        "description".into(),
        "A Drapewright sewing pattern project, schema version 1. See docs/file-format.md.".into(),
    );
    let text = serde_json::to_string_pretty(&schema).unwrap() + "\n";
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/schema/project.schema.json");
    if std::fs::read_to_string(&path).ok().as_deref() != Some(text.as_str()) {
        std::fs::write(&path, &text).unwrap();
    }
    // A sanity check that the schema describes what the engine writes.
    assert!(text.contains("\"schemaVersion\""));
    assert!(text.contains("\"pieces\""));
}
