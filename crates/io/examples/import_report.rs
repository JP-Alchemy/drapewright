//! How a pile of real pattern files imports: one line per file with the
//! pieces found, how sure the scale is, and what the importer noted. For
//! checking the importers against patterns that can't be committed.
//!
//!     cargo run -p drapewright-io --example import_report -- [-v] <files or folders>…
//!
//! Folders are searched for .svg, .dxf and .pdf files. `-v` also lists each piece:
//! its edges (folds marked), allowance, notches, internal lines, grainline
//! and cutting. Nothing is written.

use std::path::{Path, PathBuf};
use std::time::Instant;

use drapewright_io::{ImportNote, ImportResult};

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(path)
            .map(|d| d.filter_map(|e| e.ok().map(|e| e.path())).collect())
            .unwrap_or_default();
        entries.sort();
        for entry in entries {
            collect(&entry, out);
        }
    } else if matches!(extension(path).as_str(), "svg" | "dxf" | "pdf") {
        out.push(path.to_path_buf());
    }
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn note(n: &ImportNote) -> String {
    match n {
        ImportNote::SeamLineFound { allowance, .. } => format!("seam line ({allowance} mm)"),
        ImportNote::CuttingLineDiffers { max_mm, .. } => format!("cut line differs {max_mm} mm"),
        ImportNote::SmallShapesIgnored { count } => format!("{count} small shapes left out"),
        ImportNote::LooseLinesIgnored { count } => format!("{count} loose lines left out"),
        ImportNote::LinesOnOutlineIgnored { count } => {
            format!("{count} lines on outlines left out")
        }
        ImportNote::NoClosedShapes => "no closed shapes".into(),
        ImportNote::UnsupportedEntities { count } => format!("{count} unsupported entities"),
        ImportNote::CurvesTraced { count } => format!("{count} curves traced"),
        ImportNote::OutlineCrossesItself { .. } => "an outline crosses itself".into(),
        ImportNote::PagesJoined { count } => format!("{count} pages joined"),
        ImportNote::PagesLeftOut { count } => format!("{count} pages left out"),
        ImportNote::TestSquareFound { size } => format!("test square {size} mm"),
    }
}

/// Notes of the same kind, counted ("seam line (15 mm) ×4").
fn summarise(r: &ImportResult) -> String {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for n in &r.notes {
        let text = note(n);
        match counts.iter_mut().find(|(t, _)| *t == text) {
            Some((_, c)) => *c += 1,
            None => counts.push((text, 1)),
        }
    }
    counts
        .into_iter()
        .map(|(t, c)| if c > 1 { format!("{t} ×{c}") } else { t })
        .collect::<Vec<_>>()
        .join("; ")
}

fn main() {
    let mut files = Vec::new();
    let mut verbose = false;
    for arg in std::env::args().skip(1) {
        if arg == "-v" {
            verbose = true;
        } else {
            collect(Path::new(&arg), &mut files);
        }
    }
    if files.is_empty() {
        eprintln!("usage: import_report <files or folders>…");
        std::process::exit(2);
    }
    let (mut ok, mut failed, mut empty) = (0, 0, 0);
    for path in &files {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                println!("ERROR  {name}: {e}");
                failed += 1;
                continue;
            }
        };
        let start = Instant::now();
        let result = match extension(path).as_str() {
            "svg" => drapewright_io::svg::import_svg(&data, Some(name)),
            "pdf" => drapewright_io::pdf::import_pdf(&data, Some(name)),
            _ => drapewright_io::dxf::import_dxf(&data, Some(name)),
        };
        let ms = start.elapsed().as_secs_f64() * 1000.0;
        match result {
            Err(e) => {
                println!("ERROR  {name}: {e}");
                failed += 1;
            }
            Ok(r) => {
                if r.pieces.is_empty() {
                    empty += 1;
                } else {
                    ok += 1;
                }
                let scale = format!(
                    "{:?}{}",
                    r.scale.chosen.basis,
                    if r.scale.certain { "" } else { "?" }
                );
                let sizes = match r.sizes.names.len() {
                    0 | 1 => String::new(),
                    n => format!(
                        ", {n} sizes {}–{}{}",
                        r.sizes.names[0],
                        r.sizes.names[n - 1],
                        r.sizes
                            .sample
                            .as_deref()
                            .map(|s| format!(" (sample {s})"))
                            .unwrap_or_default()
                    ),
                };
                let asked = if r.allowance.pieces.is_empty() {
                    String::new()
                } else {
                    format!(", asks allowance for {}", r.allowance.pieces.len())
                };
                let names: Vec<&str> = r.pieces.iter().take(4).map(|p| p.name.as_str()).collect();
                let more = r.pieces.len().saturating_sub(4);
                println!(
                    "{:>3} pcs  {name} [{scale}{sizes}{asked}, {ms:.0} ms] {}{} | {}",
                    r.pieces.len(),
                    names.join(", "),
                    if more > 0 {
                        format!(" +{more}")
                    } else {
                        String::new()
                    },
                    summarise(&r),
                );
                if verbose {
                    for p in &r.pieces {
                        let folds = p
                            .outline
                            .edges
                            .iter()
                            .filter(|e| e.kind == drapewright_pattern_core::EdgeKind::Fold)
                            .count();
                        let cut = p.cut.first().map_or(String::new(), |c| {
                            format!(
                                "cut {}{}{} {}",
                                c.quantity,
                                if c.mirrored == Some(true) {
                                    " mirrored"
                                } else {
                                    ""
                                },
                                if c.on_fold == Some(true) {
                                    " on fold"
                                } else {
                                    ""
                                },
                                c.material
                            )
                        });
                        let b = p.outline.bounds();
                        println!(
                            "         - {} @{:.0},{:.0}: {} edges ({folds} fold), {:?} {} mm, {} notches, {} lines, grain {}, {cut}",
                            p.name,
                            b.x0,
                            b.y0,
                            p.outline.edges.len(),
                            p.allowance.mode,
                            p.allowance.seam,
                            p.notches.len(),
                            p.lines.len(),
                            if p.grainline.is_some() { "yes" } else { "no" },
                        );
                    }
                }
            }
        }
    }
    println!(
        "== {} files: {ok} with pieces, {empty} with none, {failed} failed",
        files.len()
    );
}
