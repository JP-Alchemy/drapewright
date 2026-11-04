//! FreeSewing's Cathrin corset (MIT) as the app drafts it to the example
//! figure (`fixtures/svg/freesewing-cathrin-example.svg`): its centre
//! front and the panel beside it drawn by one part, and a waist line on
//! every panel. Find seams sews it whole, and the drape puts it on by its
//! waist, from about the underbust down to the high hip.

use drapewright_body::{BodyModel, Shape};
use drapewright_io::svg::import_svg;
use drapewright_pattern_core::Piece;
use drapewright_seams::{Reason, Seam, suggest};
use drapewright_sim::garment::assemble;
use drapewright_sim::girth::girths;
use drapewright_sim::place::{Figure, Level, place};

fn pieces() -> Vec<Piece> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/svg/freesewing-cathrin-example.svg"
    );
    import_svg(&std::fs::read(path).unwrap(), Some("cathrin.svg"))
        .unwrap()
        .pieces
}

fn figure() -> Figure {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../assets/body/adult.dwbody"
    );
    let model = BodyModel::from_bytes(&std::fs::read(path).unwrap()).unwrap();
    let body = model.shape(&Shape::default());
    Figure::of(&model, &body)
}

#[test]
fn each_panel_is_read_with_its_cutting_and_waist_line() {
    let pieces = pieces();
    let read: Vec<(&str, u32, bool)> = pieces
        .iter()
        .map(|p| (p.name.as_str(), p.cut[0].quantity, p.has_fold()))
        .collect();
    assert_eq!(
        read,
        [
            ("Panel1a", 1, true),
            ("Panel1b", 2, false),
            ("Panel2", 2, false),
            ("Panel3", 2, false),
            ("Panel4", 2, false),
            ("Panel5", 2, false),
            ("Panel6", 2, false),
        ]
    );
    for p in &pieces {
        let labels: Vec<Option<&str>> = p.lines.iter().map(|l| l.label.as_deref()).collect();
        assert_eq!(labels, [Some("Waist line")], "{}", p.name);
    }
}

/// A seam suggested: its pieces' names, and why.
type Said = (String, String, Vec<Reason>);

/// The seams Find seams suggests, and what is said of each.
fn suggested(pieces: &[Piece]) -> (Vec<Seam>, Vec<Said>) {
    let name = |id: &str| pieces.iter().find(|p| p.id == id).unwrap().name.clone();
    let found = suggest(pieces, &[]);
    let mut said: Vec<Said> = found
        .iter()
        .map(|s| {
            (
                name(&s.seam.a.piece),
                name(&s.seam.b.piece),
                s.reasons.clone(),
            )
        })
        .map(|(a, b, r)| if a <= b { (a, b, r) } else { (b, a, r) })
        .collect();
    said.sort_by(|x, y| (&x.0, &x.1).cmp(&(&y.0, &y.1)));
    (found.into_iter().map(|s| s.seam).collect(), said)
}

#[test]
fn find_seams_sews_the_panels_side_by_side_and_the_centre_back() {
    let pieces = pieces();
    let (_, said) = suggested(&pieces);
    let pairs: Vec<(&str, &str)> = said.iter().map(|s| (s.0.as_str(), s.1.as_str())).collect();
    assert_eq!(
        pairs,
        [
            ("Panel1a", "Panel1b"),
            ("Panel1b", "Panel2"),
            ("Panel2", "Panel3"),
            ("Panel3", "Panel4"),
            ("Panel4", "Panel5"),
            ("Panel5", "Panel6"),
            ("Panel6", "Panel6"),
        ]
    );
    // The centre front's two parts, straight and as long as each other:
    // their waist lines meet at the seam. The back's centre, to its other
    // half.
    assert!(said[0].2.contains(&Reason::LinesMeet), "{:?}", said[0]);
    assert_eq!(said[6].2, [Reason::CentreOfPair]);
}

#[test]
fn the_corset_goes_on_by_its_waist_from_the_underbust_to_the_high_hip() {
    let pieces = pieces();
    let (seams, _) = suggested(&pieces);
    let figure = figure();
    let g = assemble(&pieces, &seams, 20.0).unwrap();
    let at = place(&g, &figure);
    let level = |name: &str| figure.levels.iter().find(|l| l.name == name).unwrap();
    for (k, panel) in g.panels.iter().enumerate() {
        let ys = at[k].iter().map(|p| p[1]);
        let (low, high) = ys.fold((f64::MAX, f64::MIN), |(l, h), y| (l.min(y), h.max(y)));
        // Its waist line at the waist: 81 mm below its top at the sides,
        // where the underbust is, and 127 mm above its hem at the high hip
        // (FreeSewing's waist to underbust and to hips, estimated).
        let waist = panel.waist.expect("a waist line");
        let flat_high = panel
            .mesh
            .points
            .iter()
            .map(|p| p[1])
            .fold(f64::MIN, f64::max);
        assert!(
            ((high - figure.waist) - (flat_high - waist)).abs() < 1.0,
            "{}: top at {high:.0}, waist {:.0}",
            panel.name,
            figure.waist
        );
        assert!(
            high > figure.waist + 70.0,
            "{}: top at {high:.0}",
            panel.name
        );
        assert!(
            high < level("underbust").y + 20.0,
            "{}: top at {high:.0}",
            panel.name
        );
        assert!(
            low < figure.waist - 110.0 && low > level("hip").y,
            "{}: hem at {low:.0}",
            panel.name
        );
    }
    // Round the waist, a corset's: the figure's 708 mm, less the waist it
    // takes in and the lacing's gap at the back.
    let sizes = girths(&g, &at, &figure);
    let size = |name: &str| sizes.iter().find(|s| s.0 == name).unwrap().1;
    let waist = size("waist").expect("round the waist");
    assert!((580.0..650.0).contains(&waist), "{waist:.0}");
    // It ends above the hip: none there.
    assert_eq!(size("hip"), None);
}

#[test]
fn the_centre_front_dipping_alone_below_the_rest_is_not_its_size_round() {
    let pieces = pieces();
    let (seams, _) = suggested(&pieces);
    let mut figure = figure();
    let g = assemble(&pieces, &seams, 20.0).unwrap();
    let at = place(&g, &figure);
    let bottom = |k: usize| at[k].iter().map(|p| p[1]).fold(f64::MAX, f64::min);
    let front = g.panels.iter().position(|p| p.name == "Panel1a").unwrap();
    let others = (0..g.panels.len())
        .filter(|&k| k != front)
        .map(bottom)
        .fold(f64::MAX, f64::min);
    assert!(bottom(front) < others - 0.5);
    // Just above the front's lowest point, the rest of the corset ends
    // higher: across the front alone, a few centimetres, nothing round.
    figure.levels = vec![Level {
        name: "hip",
        y: (bottom(front) + others) / 2.0,
        girth: 900.0,
    }];
    assert_eq!(girths(&g, &at, &figure), [("hip", None)]);
}
