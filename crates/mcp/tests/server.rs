//! The MCP server end to end: the protocol, then a session as an assistant
//! would run it, on the committed fixtures.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use drapewright_mcp::{Host, Server};
use drapewright_project::container;
use serde_json::{Value, json};

fn server() -> Server {
    Server::new(Host {
        now: || "2026-10-11T12:00:00Z".into(),
        new_id: || "test-project".into(),
    })
}

fn fixture(path: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(path)
        .display()
        .to_string()
}

/// A fresh, empty folder for one test's files.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("drapewright-mcp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn request(s: &mut Server, method: &str, params: Value) -> Value {
    s.handle(json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params }))
        .expect("a request gets a reply")
}

/// Calls a tool: whether it failed, and its content.
fn call(s: &mut Server, name: &str, args: Value) -> (bool, Vec<Value>) {
    let reply = request(s, "tools/call", json!({ "name": name, "arguments": args }));
    let result = &reply["result"];
    (
        result["isError"].as_bool().expect("isError"),
        result["content"].as_array().expect("content").clone(),
    )
}

fn text(content: &[Value]) -> String {
    content
        .iter()
        .filter_map(|c| c["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn ok(s: &mut Server, name: &str, args: Value) -> String {
    let (failed, content) = call(s, name, args);
    assert!(!failed, "{name} failed: {}", text(&content));
    text(&content)
}

fn ok_json(s: &mut Server, name: &str, args: Value) -> Value {
    serde_json::from_str(&ok(s, name, args)).expect("JSON")
}

fn fails(s: &mut Server, name: &str, args: Value) -> String {
    let (failed, content) = call(s, name, args);
    assert!(failed, "{name} should have failed: {}", text(&content));
    text(&content)
}

fn piece_id(summary: &Value, name: &str) -> String {
    summary["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == name)
        .unwrap_or_else(|| panic!("no piece {name}"))["id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[test]
fn speaks_the_protocol() {
    let mut s = server();
    let init = request(
        &mut s,
        "initialize",
        json!({ "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "test", "version": "1" } }),
    );
    let result = &init["result"];
    assert_eq!(init["id"], 7);
    assert_eq!(result["protocolVersion"], "2025-06-18");
    assert_eq!(result["serverInfo"]["name"], "drapewright");
    assert!(result["capabilities"]["tools"].is_object());
    assert!(
        result["instructions"]
            .as_str()
            .unwrap()
            .contains("millimetres")
    );
    // An unknown version gets the newest this server speaks.
    let newer = request(
        &mut s,
        "initialize",
        json!({ "protocolVersion": "2099-01-01" }),
    );
    assert_eq!(
        newer["result"]["protocolVersion"],
        drapewright_mcp::PROTOCOL_VERSIONS[0]
    );

    // Notifications get no reply.
    assert!(
        s.handle(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .is_none()
    );
    assert_eq!(request(&mut s, "ping", json!({}))["result"], json!({}));

    let list = request(&mut s, "tools/list", json!({}));
    let tools = list["result"]["tools"].as_array().unwrap();
    assert!(tools.len() >= 20);
    for t in tools {
        assert_eq!(t["inputSchema"]["type"], "object", "{}", t["name"]);
        assert!(!t["description"].as_str().unwrap().is_empty());
    }
    let import = tools
        .iter()
        .find(|t| t["name"] == "import_pattern")
        .unwrap();
    let schema = &import["inputSchema"];
    assert_eq!(schema["required"], json!(["path"]));
    for field in ["path", "sizes", "scale", "seam_allowance"] {
        assert!(schema["properties"][field].is_object(), "{field}");
    }
    assert_eq!(import["annotations"]["readOnlyHint"], false);
    let inspect = tools
        .iter()
        .find(|t| t["name"] == "inspect_pattern_file")
        .unwrap();
    assert_eq!(inspect["annotations"]["readOnlyHint"], true);

    assert_eq!(request(&mut s, "nope", json!({}))["error"]["code"], -32601);
    assert_eq!(
        request(&mut s, "tools/call", json!({ "name": "nope" }))["error"]["code"],
        -32602
    );
    let parse = s.handle_line("{not json").unwrap();
    assert!(parse.contains("-32700"), "{parse}");
    // Arguments that don't fit come back as a tool error the assistant can read.
    let wrong = fails(&mut s, "project_summary", json!({ "colour": "red" }));
    assert!(wrong.contains("don't fit"), "{wrong}");
}

#[test]
fn an_assistant_imports_asks_changes_and_saves() {
    let dir = scratch("session");
    let mut s = server();
    let none = fails(&mut s, "project_summary", json!({}));
    assert!(none.contains("No project is open"), "{none}");

    // A file with every size: what it holds, and what must be asked.
    let graded = fixture("dxf/aama-graded.dxf");
    let report = ok_json(&mut s, "inspect_pattern_file", json!({ "path": graded }));
    assert_eq!(report["sizes"]["sizes"], json!(["S", "M", "L"]));
    assert_eq!(report["sizes"]["made_in"], "M");
    assert_eq!(
        report["pieces"].as_array().unwrap().len(),
        2,
        "one size shown"
    );
    assert_eq!(report["scale"]["certain"], false);
    let reading = report["scale"]["reading"].clone();
    assert!(report["scale"]["readings"].as_array().unwrap().len() > 1);
    assert!(report["seam_allowance_question"].is_object());
    let next = report["to_import"].as_str().unwrap();
    assert!(next.contains("sizes") && next.contains("scale") && next.contains("seam_allowance"));

    // Nothing is guessed: each question must be answered.
    let e = fails(&mut s, "import_pattern", json!({ "path": graded }));
    assert!(e.contains("3 sizes") && e.contains("S, M, L"), "{e}");
    let e = fails(
        &mut s,
        "import_pattern",
        json!({ "path": graded, "sizes": ["XL"] }),
    );
    assert!(e.contains("no size \"XL\""), "{e}");
    let e = fails(
        &mut s,
        "import_pattern",
        json!({ "path": graded, "sizes": ["M"] }),
    );
    assert!(e.contains("doesn't say how big"), "{e}");
    let e = fails(
        &mut s,
        "import_pattern",
        json!({ "path": graded, "sizes": ["M"], "scale": reading }),
    );
    assert!(e.contains("seam allowance"), "{e}");
    let added = ok_json(
        &mut s,
        "import_pattern",
        json!({
            "path": graded,
            "sizes": ["M"],
            "scale": reading,
            "seam_allowance": { "included": true, "width_mm": 10 },
        }),
    );
    let names: Vec<&str> = added["added"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Front", "Back"]);
    assert_eq!(added["project"], "aama-graded");

    // A FreeSewing pattern needs no questions.
    let dress = fixture("svg/freesewing-sasha-sa10.svg");
    let added = ok_json(&mut s, "import_pattern", json!({ "path": dress }));
    assert_eq!(added["added"].as_array().unwrap().len(), 7);

    let summary = ok_json(&mut s, "project_summary", json!({}));
    assert_eq!(summary["pieces"].as_array().unwrap().len(), 9);
    assert_eq!(summary["unsaved_changes"], true);

    // A picture, numbered, with edge ids.
    let front = piece_id(&summary, "Front inside");
    let (failed, content) = call(
        &mut s,
        "render_preview",
        json!({ "pieces": [front], "labels": "edges", "max_px": 600 }),
    );
    assert!(!failed);
    let image = content.iter().find(|c| c["type"] == "image").unwrap();
    assert_eq!(image["mimeType"], "image/png");
    assert!(image["data"].as_str().unwrap().starts_with("iVBORw0KGgo"));
    let legend = text(&content);
    assert!(legend.contains("1. ") && legend.contains("Front inside") && legend.contains("fold"));

    // Rename, cut, edges: each one undo step.
    let pocket = piece_id(&summary, "Pocket");
    let detail = ok_json(
        &mut s,
        "update_piece",
        json!({ "piece": pocket, "name": "Patch pocket", "cut": [{ "quantity": 2, "material": "lining" }] }),
    );
    assert_eq!(detail["name"], "Patch pocket");
    assert_eq!(detail["cut"], json!(["Cut 2 mirrored · lining"]));
    let piece = ok_json(&mut s, "get_piece", json!({ "piece": front }));
    let fold = piece["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "fold")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let changed = ok_json(
        &mut s,
        "update_edge",
        json!({ "piece": front, "edge": fold, "kind": "seam", "name": "centre front" }),
    );
    let edge = changed["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == fold.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        (edge["kind"].as_str(), edge["name"].as_str()),
        (Some("seam"), Some("centre front"))
    );
    assert!(ok(&mut s, "undo", json!({})).contains("edge type"));
    let back = ok_json(&mut s, "get_piece", json!({ "piece": front }));
    let edge = back["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["id"] == fold.as_str())
        .unwrap()
        .clone();
    assert_eq!(
        (edge["kind"].as_str(), edge["name"].as_str()),
        (Some("fold"), None)
    );
    let e = fails(
        &mut s,
        "update_edge",
        json!({ "piece": front, "edge": "e999999", "kind": "hem" }),
    );
    assert!(e.contains("edge"), "{e}");

    // A drafted piece, and a notch on it.
    let band = ok_json(
        &mut s,
        "create_piece",
        json!({ "name": "Waistband", "rectangle": { "width_mm": 700, "height_mm": 80 }, "cut": [{ "quantity": 1 }] }),
    );
    assert_eq!(
        (band["width_mm"].as_f64(), band["height_mm"].as_f64()),
        (Some(700.0), Some(80.0))
    );
    let band_id = band["id"].as_str().unwrap().to_string();
    let top = band["edges"][0]["id"].as_str().unwrap().to_string();
    let notched = ok(
        &mut s,
        "add_notch",
        json!({ "piece": band_id, "edge": top, "at_mm": 350, "kind": "double" }),
    );
    assert!(notched.contains("Added notch"), "{notched}");
    let e = fails(
        &mut s,
        "create_piece",
        json!({ "name": "Open", "path": "M0,0 L10,0 L10,10" }),
    );
    assert!(
        e.contains("closed") || e.contains("end where it starts"),
        "{e}"
    );

    assert!(
        ok(&mut s, "arrange_pieces", json!({ "width_mm": 1500 })).contains("Arranged 10 pieces")
    );
    let plan = ok_json(&mut s, "print_settings", json!({ "paper": "a0" }));
    assert_eq!(plan["paper"], "A0 (copy shop)");

    // Saved with the files the pieces came from, and opened again.
    let file = dir.join("dress");
    let saved = ok(
        &mut s,
        "save_project",
        json!({ "path": file.display().to_string() }),
    );
    assert!(saved.contains("dress.drapewright"), "{saved}");
    let bytes = std::fs::read(dir.join("dress.drapewright")).unwrap();
    let unpacked = container::unpack(&bytes).unwrap();
    let sources: Vec<&str> = unpacked.sidecars.iter().map(|s| s.path.as_str()).collect();
    assert_eq!(sources.len(), 2, "{sources:?}");
    assert!(
        sources
            .iter()
            .any(|p| p.ends_with("-freesewing-sasha-sa10.svg"))
    );
    assert!(sources.iter().any(|p| p.ends_with("-aama-graded.dxf")));
    assert_eq!(
        ok_json(&mut s, "project_summary", json!({}))["unsaved_changes"],
        false
    );

    // Unsaved work is not dropped without saying so.
    ok(
        &mut s,
        "update_piece",
        json!({ "piece": band_id, "name": "Belt" }),
    );
    let e = fails(&mut s, "new_project", json!({}));
    assert!(e.contains("unsaved changes"), "{e}");
    let fresh = ok_json(
        &mut s,
        "new_project",
        json!({ "name": "Next", "discard_unsaved": true }),
    );
    assert_eq!(fresh["pieces"].as_array().unwrap().len(), 0);

    let opened = ok_json(
        &mut s,
        "open_project",
        json!({ "path": dir.join("dress.drapewright").display().to_string() }),
    );
    let names: Vec<&str> = opened["pieces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(names.len(), 10);
    assert!(names.contains(&"Patch pocket") && names.contains(&"Waistband"));

    // Exports, which never replace a file unless asked.
    let pdf = dir.join("dress.pdf");
    ok(
        &mut s,
        "export_pdf",
        json!({ "path": pdf.display().to_string() }),
    );
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF"));
    let e = fails(
        &mut s,
        "export_pdf",
        json!({ "path": pdf.display().to_string() }),
    );
    assert!(e.contains("already exists"), "{e}");
    ok(
        &mut s,
        "export_pdf",
        json!({ "path": pdf.display().to_string(), "overwrite": true }),
    );
    let svg = dir.join("dress.svg");
    ok(
        &mut s,
        "export_svg",
        json!({ "path": svg.display().to_string() }),
    );
    assert!(std::fs::read_to_string(&svg).unwrap().contains("<svg"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn runs_over_stdio() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_drapewright-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    for line in [
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": { "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": { "name": "t", "version": "1" } } }),
        json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
        json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": { "name": "new_project", "arguments": { "name": "Over stdio" } } }),
    ] {
        writeln!(stdin, "{line}").unwrap();
    }
    drop(stdin);
    let replies: Vec<Value> = BufReader::new(child.stdout.take().unwrap())
        .lines()
        .map(|l| serde_json::from_str(&l.unwrap()).unwrap())
        .collect();
    assert!(child.wait().unwrap().success());
    let ids: Vec<&Value> = replies.iter().map(|r| &r["id"]).collect();
    assert_eq!(ids, [&json!(1), &json!(2), &json!(3)]);
    assert!(
        replies[2]["result"]["content"][0]["text"]
            .as_str()
            .unwrap()
            .contains("Over stdio")
    );
}

#[test]
fn an_assistant_keeps_a_persons_measurements() {
    let mut s = server();
    ok(&mut s, "new_project", json!({ "name": "Blouse for Anna" }));
    let added = ok(
        &mut s,
        "set_measurements",
        json!({ "name": "Anna", "taken_on": "2026-10-01",
                "values_mm": { "bust": 960, "waist": 7800, "x-elbow": 260 } }),
    );
    // A waist given in tenths of a millimetre, say, is pointed out.
    assert!(added.contains("Added Anna's measurements"), "{added}");
    assert!(added.contains("waist 7800 mm"), "{added}");
    let got = ok_json(&mut s, "get_measurements", json!({}));
    let anna = &got["people"][0];
    let id = anna["id"].as_str().unwrap().to_string();
    assert_eq!(anna["valuesMm"]["bust"], 960.0);
    assert!(
        got["measurements"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["key"] == "hip" && m["freesewing"] == "seat")
    );

    // Measured again a month later: the first measurement is kept.
    ok(
        &mut s,
        "set_measurements",
        json!({ "person": id, "measure_again": true, "taken_on": "2026-11-01",
                "values_mm": { "waist": 780 }, "remove": ["x-elbow"] }),
    );
    let anna = &ok_json(&mut s, "get_measurements", json!({}))["people"][0];
    assert_eq!(anna["measurement"], 2);
    assert_eq!(anna["earlierMeasurements"], 1);
    assert_eq!(anna["valuesMm"], json!({ "bust": 960.0, "waist": 780.0 }));

    let wrong = fails(
        &mut s,
        "set_measurements",
        json!({ "values_mm": { "wiast": 780 } }),
    );
    assert!(wrong.contains("highBust"), "{wrong}");
    let gone = fails(
        &mut s,
        "set_measurements",
        json!({ "person": "m99", "name": "X" }),
    );
    assert!(gone.contains("get_measurements"), "{gone}");
}
