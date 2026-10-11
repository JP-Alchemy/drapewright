//! PDF patterns read back. The app's own printouts of the FreeSewing dress
//! (fixtures/svg), in every layout, give back the pieces they were printed
//! from: names, cutting, folds, notches, grainlines and the seam allowance
//! measured from the printed stitching line.

// Measured allowances snap to common widths, so they compare exactly.
#![allow(clippy::float_cmp)]

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use drapewright_io::pdf::{PdfTexts, export_pdf, import_pdf, plan_print};
use drapewright_io::svg::import_svg;
use drapewright_io::{ImportNote, ImportResult};
use drapewright_pattern_core::{EdgeKind, Piece, ScaleBasis};
use drapewright_project::Document;
use drapewright_project::document::{Assembly, Meta, Paper, PaperName, PrintSettings};

fn fixture(path: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(path),
    )
    .unwrap()
}

/// The FreeSewing dress, as imported from its SVG.
fn dress() -> Vec<Piece> {
    let data = fixture("svg/freesewing-sasha-sa10.svg");
    import_svg(&data, Some("freesewing-sasha-sa10.svg"))
        .unwrap()
        .pieces
}

fn document(pieces: &[Piece], paper: PaperName, assembly: Assembly) -> Document {
    Document {
        meta: Meta {
            id: "dress".into(),
            name: "Sasha".into(),
            created: "2026-10-11T00:00:00Z".into(),
            modified: "2026-10-11T00:00:00Z".into(),
            notes: String::new(),
        },
        settings: Default::default(),
        pieces: pieces.iter().cloned().map(Arc::new).collect(),
        print: PrintSettings {
            paper: Paper::Named(paper),
            assembly,
            ..Default::default()
        },
        extensions: Default::default(),
    }
}

/// The words the app prints: a cutting line under each name.
fn texts(doc: &Document) -> PdfTexts {
    PdfTexts {
        title: doc.meta.name.clone(),
        subtitle: "Test print".into(),
        instructions: vec!["Print at 100%.".into()],
        square_metric: "10 cm".into(),
        square_imperial: "4 in".into(),
        page_map: "Page map".into(),
        not_printed: "Dashed: not printed".into(),
        place_on_fold: "Place on fold".into(),
        piece_lines: doc
            .pieces
            .iter()
            .map(|p| {
                let fold = p.outline.edges.iter().any(|e| e.kind == EdgeKind::Fold);
                let lines = p
                    .cut
                    .first()
                    .map(|c| {
                        let on_fold = if c.on_fold.unwrap_or(fold) {
                            " on the fold"
                        } else {
                            ""
                        };
                        vec![format!("Cut {}{on_fold} · Main fabric", c.quantity)]
                    })
                    .unwrap_or_default();
                (p.id.clone(), lines)
            })
            .collect::<HashMap<_, _>>(),
    }
}

/// The imported pieces match the ones printed.
fn matches_printed(result: &ImportResult, printed: &[Piece]) {
    assert_eq!(
        result.pieces.len(),
        printed.len(),
        "one piece each, no extras"
    );
    assert!(result.scale.certain);
    assert_eq!(result.scale.chosen.basis, ScaleBasis::DeclaredUnits);
    // Both lines are printed, so the allowance is measured, never asked.
    assert!(result.allowance.pieces.is_empty());
    for original in printed {
        let p = result
            .pieces
            .iter()
            .find(|p| p.name == original.name)
            .unwrap_or_else(|| {
                panic!(
                    "no {} in {:?}",
                    original.name,
                    result.pieces.iter().map(|p| &p.name).collect::<Vec<_>>()
                )
            });
        let name = &original.name;
        assert_eq!(p.notches.len(), original.notches.len(), "{name}: notches");
        assert_eq!(
            p.notches.iter().map(|n| n.kind).collect::<Vec<_>>().len(),
            original.notches.len()
        );
        assert!(p.grainline.is_some(), "{name}: grainline");
        assert_eq!(
            p.allowance.seam, original.allowance.seam,
            "{name}: allowance"
        );
        let folds = |p: &Piece| {
            p.outline
                .edges
                .iter()
                .filter(|e| e.kind == EdgeKind::Fold)
                .count()
        };
        assert_eq!(folds(p), folds(original), "{name}: folds");
        assert_eq!(
            p.cut[0].quantity, original.cut[0].quantity,
            "{name}: cutting"
        );
        let (a, b) = (p.outline.bounds(), original.outline.bounds());
        assert!(
            (a.width() - b.width()).abs() < 0.5 && (a.height() - b.height()).abs() < 0.5,
            "{name}: size"
        );
        // Only the pocket opening is a line inside a piece.
        let lines = usize::from(name == "Front outside above");
        assert_eq!(p.lines.len(), lines, "{name}: internal lines");
    }
}

#[test]
fn printouts_read_back_as_the_pieces_they_print() {
    let pieces = dress();
    for (paper, assembly, pages) in [
        (PaperName::A4, Assembly::Trim, 31),
        (PaperName::Letter, Assembly::Overlap, 37),
        (PaperName::A0, Assembly::Trim, 2),
    ] {
        let doc = document(&pieces, paper, assembly);
        let plan = plan_print(&doc).unwrap();
        let pdf = export_pdf(&doc, &texts(&doc)).unwrap();
        let result = import_pdf(&pdf, Some("sasha.pdf")).unwrap();
        let printed = plan.pages.iter().filter(|p| p.printed).count();
        assert_eq!(printed, pages, "{paper:?}");
        assert!(
            result.notes.contains(&ImportNote::PagesJoined {
                count: pages as u32
            }),
            "{paper:?}: {:?}",
            result.notes
        );
        assert!(
            result
                .notes
                .contains(&ImportNote::PagesLeftOut { count: 1 }),
            "the cover"
        );
        matches_printed(
            &result,
            &doc.pieces.iter().map(|p| (**p).clone()).collect::<Vec<_>>(),
        );
    }
}

#[test]
fn copy_shop_pdfs_and_protected_ones() {
    let pieces = dress();
    for file in [
        "pdf/freesewing-sasha-a0.pdf",
        "pdf/freesewing-sasha-a0-protected.pdf",
    ] {
        let result = import_pdf(&fixture(file), Some(file)).unwrap();
        matches_printed(&result, &pieces);
        let front = result
            .pieces
            .iter()
            .find(|p| p.name == "Front inside")
            .unwrap();
        assert_eq!(front.cut[0].on_fold, Some(true), "{file}");
    }
}

#[test]
fn not_a_pdf_says_so() {
    let e = import_pdf(b"<svg/>", Some("x.pdf")).unwrap_err();
    assert!(e.to_string().contains("not a PDF"), "{e}");
}
