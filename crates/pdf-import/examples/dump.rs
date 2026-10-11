//! What the reader finds in PDFs: pages, layers, and per page the shapes
//! (stroked, filled, with which colours) and the text.
//!
//!     cargo run -p drapewright-pdf-import --example dump -- [-v] <file.pdf>…

use std::collections::BTreeMap;

fn main() {
    let mut verbose = false;
    for arg in std::env::args().skip(1) {
        if arg == "-v" {
            verbose = true;
            continue;
        }
        let data = match std::fs::read(&arg) {
            Ok(d) => d,
            Err(e) => {
                println!("{arg}: {e}");
                continue;
            }
        };
        let start = std::time::Instant::now();
        let doc = match drapewright_pdf_import::Document::load(&data) {
            Ok(d) => d,
            Err(e) => {
                println!("{arg}: {e}");
                continue;
            }
        };
        println!(
            "{arg}: {} pages, {} layers",
            doc.page_count(),
            doc.layers().len()
        );
        for l in doc.layers() {
            println!(
                "  layer {} “{}”{}",
                l.id,
                l.name,
                if l.visible { "" } else { " (hidden)" }
            );
        }
        for i in 0..doc.page_count() {
            match doc.page(i) {
                Err(e) => println!("  page {}: {e}", i + 1),
                Ok(p) => {
                    let mut colours: BTreeMap<String, usize> = BTreeMap::new();
                    for s in &p.drawing.shapes {
                        let key = match (s.stroke, s.fill) {
                            (Some(c), _) => format!(
                                "stroke #{:02x}{:02x}{:02x}",
                                (c[0] * 255.0) as u8,
                                (c[1] * 255.0) as u8,
                                (c[2] * 255.0) as u8
                            ),
                            (None, Some(c)) => format!(
                                "fill #{:02x}{:02x}{:02x}",
                                (c[0] * 255.0) as u8,
                                (c[1] * 255.0) as u8,
                                (c[2] * 255.0) as u8
                            ),
                            _ => "?".into(),
                        };
                        *colours.entry(key).or_default() += 1;
                    }
                    let mm = |v: f64| (v * 25.4 / 72.0).round();
                    println!(
                        "  page {}: {} × {} mm, rotate {}, {} shapes, {} texts",
                        i + 1,
                        mm(p.crop_box.width()),
                        mm(p.crop_box.height()),
                        p.rotate,
                        p.drawing.shapes.len(),
                        p.drawing.texts.len()
                    );
                    let top: Vec<String> =
                        colours.iter().map(|(k, v)| format!("{k} ×{v}")).collect();
                    println!("    {}", top.join(", "));
                    let words: Vec<&str> = p
                        .drawing
                        .texts
                        .iter()
                        .take(if verbose { 400 } else { 12 })
                        .map(|t| t.text.as_str())
                        .collect();
                    println!("    text: {}", words.join(" | "));
                }
            }
        }
        println!("  ({} ms)", start.elapsed().as_millis());
    }
}
