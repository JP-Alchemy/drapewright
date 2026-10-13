//! A drape seen from the front and the side, to check by eye: when the
//! pieces are placed, when they are sewn, and when they have settled.
//!
//!     cargo run --release -p drapewright-sim --example drape_picture -- \
//!         assets/body/adult.dwbody out [fabric] \
//!         [skirt|blouse|long|bust|armhole|yoked|trousers|<project file> [suggest]]
//!
//! writes out-placed.png, out-sewn.png and out-settled.png, draping a
//! skirt, a blouse, a blouse with long sleeves, or one with a full bust
//! adjustment (its dart in the side seam, or pivoted into the armhole),
//! cut for the standard figure, or a long blouse with yokes, gathers and
//! cuffs laid out on its side, or trousers, sewn by the seams suggested
//! for them; or a
//! project (a `.drapewright` file or its
//! `project.json`) sewn by its confirmed seams, or with `suggest` by the
//! seams suggested for it.

#[path = "../tests/common/mod.rs"]
mod common;

use drapewright_body::{BodyModel, Shape};
use drapewright_pattern_core::Piece;
use drapewright_project::container::unpack;
use drapewright_project::migrate::load;
use drapewright_seams::Seam;
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::{assemble, draped_pieces};
use drapewright_sim::place::{Figure, place};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Pixels per millimetre.
const SCALE: f32 = 0.35;

/// A triangle to draw: its depth, corners and colour.
type Drawn = (f32, [(f32, f32); 3], [u8; 3]);

fn picture(
    path: &str,
    body: &[[f64; 3]],
    body_triangles: &[[u32; 3]],
    cloth: &[[f32; 3]],
    cloth_triangles: &[[u32; 3]],
) {
    let top = body.iter().map(|p| p[1]).fold(0.0, f64::max) as f32 + 60.0;
    let (w, h) = ((2000.0 * SCALE) as u32, (top * SCALE) as u32);
    let mut pixmap = Pixmap::new(w, h).expect("a picture");
    pixmap.fill(Color::from_rgba8(250, 248, 244, 255));
    for (view, offset) in [(0, 500.0f32), (1, 1450.0f32)] {
        let project = |p: [f32; 3]| -> (f32, f32, f32) {
            let (x, depth) = if view == 0 {
                (p[0], p[2])
            } else {
                (p[2], -p[0])
            };
            ((offset + x) * SCALE, (top - p[1]) * SCALE, depth)
        };
        let shade = |a: [f32; 3], b: [f32; 3], c: [f32; 3], two_sided: bool| -> Option<f32> {
            let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
            let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
            let towards = if view == 0 { n[2] } else { -n[0] } / l;
            let towards = if two_sided { towards.abs() } else { towards };
            (towards > 0.0).then_some(0.35 + 0.65 * towards)
        };
        let mut tris: Vec<Drawn> = Vec::new();
        let no_body: &[[u32; 3]] = &[];
        let body_triangles = if std::env::var_os("NO_BODY").is_some() {
            no_body
        } else {
            body_triangles
        };
        for t in body_triangles {
            let [a, b, c] = t.map(|i| body[i as usize].map(|v| v as f32));
            if let Some(light) = shade(a, b, c, false) {
                let (pa, pb, pc) = (project(a), project(b), project(c));
                let g = (205.0 * light) as u8;
                tris.push((
                    (pa.2 + pb.2 + pc.2) / 3.0,
                    [(pa.0, pa.1), (pb.0, pb.1), (pc.0, pc.1)],
                    [g, g, g],
                ));
            }
        }
        for t in cloth_triangles {
            let [a, b, c] = t.map(|i| cloth[i as usize].map(|v| v * 1000.0));
            if let Some(light) = shade(a, b, c, true) {
                let (pa, pb, pc) = (project(a), project(b), project(c));
                let colour = [
                    (60.0 * light) as u8,
                    (110.0 * light) as u8,
                    (190.0 * light) as u8,
                ];
                tris.push((
                    (pa.2 + pb.2 + pc.2) / 3.0,
                    [(pa.0, pa.1), (pb.0, pb.1), (pc.0, pc.1)],
                    colour,
                ));
            }
        }
        tris.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (_, pts, [r, g, b]) in tris {
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            pb.line_to(pts[1].0, pts[1].1);
            pb.line_to(pts[2].0, pts[2].1);
            pb.close();
            let Some(path) = pb.finish() else { continue };
            let mut paint = Paint::default();
            paint.set_color_rgba8(r, g, b, 255);
            paint.anti_alias = false;
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }
    pixmap.save_png(path).expect("written");
    eprintln!("wrote {path}");
}

/// A project's pieces and confirmed seams, from a `.drapewright` file or its
/// `project.json`: the pieces sewn to nothing left out, as the app leaves
/// them. With `suggest`, sewn as suggested instead of by its own seams.
fn project(path: &str, suggest: bool) -> (Vec<Piece>, Vec<Seam>) {
    let bytes = std::fs::read(path).expect("a project file");
    let json = if path.ends_with(".json") {
        String::from_utf8(bytes).expect("text")
    } else {
        unpack(&bytes).expect("a .drapewright file").project_json
    };
    let file = load(&json).expect("a project");
    let pieces: Vec<Piece> = file.document.pieces.iter().map(|p| (**p).clone()).collect();
    if suggest {
        return sewn_as_found(pieces);
    }
    let seams: Vec<Seam> = file
        .document
        .seams
        .iter()
        .filter(|s| s.confirmed)
        .cloned()
        .collect();
    sewn(pieces, seams)
}

/// Pieces sewn by the seams suggested for them, those drawn for one side
/// cut as pairs first, as Find seams does.
fn sewn_as_found(mut pieces: Vec<Piece>) -> (Vec<Piece>, Vec<Seam>) {
    for id in drapewright_seams::drawn_for_one_side(&pieces, &[]) {
        if let Some(piece) = pieces.iter_mut().find(|p| p.id == id) {
            for cut in &mut piece.cut {
                cut.quantity = 2;
                cut.mirrored = Some(true);
            }
        }
    }
    let seams = drapewright_seams::suggest(&pieces, &[])
        .into_iter()
        .enumerate()
        .map(|(k, s)| Seam {
            id: format!("s{k}"),
            confirmed: true,
            ..s.seam
        })
        .collect();
    sewn(pieces, seams)
}

/// The pieces the app drapes, as their seams sew them: those sewn to
/// nothing and those turned inside left out.
fn sewn(pieces: Vec<Piece>, seams: Vec<Seam>) -> (Vec<Piece>, Vec<Seam>) {
    let name = |id: &str| {
        pieces
            .iter()
            .find(|p| p.id == id)
            .map_or(id.to_string(), |p| p.name.clone())
    };
    let part = |side: &drapewright_seams::SeamSide| match (side.from, side.to) {
        (None, None) => String::new(),
        (from, to) => format!(
            " [{:.1}..{}]",
            from.unwrap_or(0.0),
            to.map_or("end".to_string(), |t| format!("{t:.1}"))
        ),
    };
    for s in &seams {
        eprintln!(
            "seam {}: {} {}{}{} to {} {}{}{}{}",
            s.id,
            name(&s.a.piece),
            s.a.edge,
            part(&s.a),
            if s.a.mirror { " (mirror)" } else { "" },
            name(&s.b.piece),
            s.b.edge,
            part(&s.b),
            if s.b.mirror { " (mirror)" } else { "" },
            if s.reversed { ", reversed" } else { "" },
        );
    }
    let draped = draped_pieces(&pieces, &seams);
    eprintln!(
        "left out: {} sewn to nothing, {} inside",
        draped.alone.len(),
        draped.inside.len()
    );
    (draped.sewn, seams)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [asset, out, rest @ ..] = args.as_slice() else {
        eprintln!(
            "usage: drape_picture <body file> <out> [fabric] \
             [skirt|blouse|long|bust|armhole|yoked|<project file> [suggest]]"
        );
        std::process::exit(2);
    };
    let fabric = preset(rest.first().map_or("poplin", String::as_str)).expect("a fabric preset");
    let model =
        BodyModel::from_bytes(&std::fs::read(asset).expect("body file")).expect("a body file");
    let body = model.shape(&Shape::default());
    let figure = Figure::of(&model, &body);
    let triangles = model.skin();
    let (pieces, seams) = match rest.get(1).map_or("skirt", String::as_str) {
        "blouse" => common::blouse(),
        "long" => common::long_sleeved_blouse(),
        "bust" => common::full_bust_blouse(),
        "armhole" => common::armhole_dart_blouse(),
        "skirt" => common::skirt(),
        "yoked" => {
            let pieces = drapewright_pattern_core::testing::garments::yoked_blouse(true);
            sewn_as_found(pieces)
        }
        "trousers" => sewn_as_found(drapewright_pattern_core::testing::garments::trousers()),
        path => project(path, rest.get(2).is_some_and(|s| s == "suggest")),
    };
    let garment = assemble(&pieces, &seams, 20.0).expect("a garment");
    if drapewright_sim::place::half_round(&garment) {
        eprintln!("only half way round: cut as pairs?");
    }
    let starts = place(&garment, &figure);
    let mut drape = Drape::new(&garment, &starts, fabric, (&body.vertices, &triangles));
    let draw = |name: &str, drape: &Drape| {
        picture(
            &format!("{out}-{name}.png"),
            &body.vertices,
            &triangles,
            &drape.cloth.x,
            &drape.cloth.triangles,
        );
    };
    draw("placed", &drape);
    while drape.sewing() {
        drape.step(1.0 / 60.0);
    }
    draw("sewn", &drape);
    let mut frames = 0;
    while !drape.settled() && frames < 60 * 10 {
        drape.step(1.0 / 60.0);
        frames += 1;
    }
    eprintln!(
        "settled after {:.1} s; mean speed {:.4} m/s; widest stitch {:.1} mm",
        drape.time,
        drape.cloth.mean_speed(),
        drape.cloth.widest_stitch() * 1000.0
    );
    let deepest = drape
        .cloth
        .x
        .iter()
        .map(|&p| drape.clearance(p))
        .fold(f32::MAX, f32::min);
    let middles = drape
        .cloth
        .triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| drape.cloth.x[i as usize]);
            drape.clearance([
                (a[0] + b[0] + c[0]) / 3.0,
                (a[1] + b[1] + c[1]) / 3.0,
                (a[2] + b[2] + c[2]) / 3.0,
            ])
        })
        .fold(f32::MAX, f32::min);
    eprintln!(
        "nearest the body: points {:.1} mm, middles of triangles {:.1} mm",
        deepest * 1000.0,
        middles * 1000.0
    );
    // Hanging evenly: its middle, and its lowest point either side.
    let x = &drape.cloth.x;
    let middle = x.iter().map(|p| p[0]).sum::<f32>() / x.len() as f32 * 1000.0;
    let lowest = |left: bool| {
        x.iter()
            .filter(|p| (p[0] > 0.0) == left)
            .map(|p| p[1])
            .fold(f32::MAX, f32::min)
            * 1000.0
    };
    eprintln!(
        "middle {middle:.1} mm across; lowest {:.0} mm on the left, {:.0} mm on the right",
        lowest(true),
        lowest(false)
    );
    draw("settled", &drape);
}
