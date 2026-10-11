//! A drape seen from the front and the side, to check by eye: when the
//! pieces are placed, when they are sewn, and when they have settled.
//!
//!     cargo run --release -p drapewright-sim --example drape_picture -- \
//!         assets/body/adult.dwbody out [fabric]
//!
//! writes out-placed.png, out-sewn.png and out-settled.png, draping a
//! skirt cut for the standard figure.

#[path = "../tests/common/mod.rs"]
mod common;

use drapewright_body::measure::trunk_sections;
use drapewright_body::{BodyModel, Shape, measure};
use drapewright_sim::drape::Drape;
use drapewright_sim::fabric::preset;
use drapewright_sim::garment::assemble;
use drapewright_sim::place::{Figure, Section, place};
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

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [asset, out, rest @ ..] = args.as_slice() else {
        eprintln!("usage: drape_picture <body file> <out> [fabric]");
        std::process::exit(2);
    };
    let fabric = preset(rest.first().map_or("poplin", String::as_str)).expect("a fabric preset");
    let model =
        BodyModel::from_bytes(&std::fs::read(asset).expect("body file")).expect("a body file");
    let body = model.shape(&Shape::default());
    let (_, marks) = measure(&model, &body);
    let shoulders = marks.points["shoulderPoint"][1];
    let figure = Figure {
        waist: marks.levels["waist"],
        shoulders,
        sections: trunk_sections(&model, &body, marks.levels["hip"] - 400.0, shoulders, 20.0)
            .into_iter()
            .map(|(y, middle, half)| Section { y, middle, half })
            .collect(),
    };
    let shown = model.asset.shown;
    let triangles: Vec<[u32; 3]> = model
        .asset
        .triangles
        .iter()
        .filter(|t| t.iter().all(|&v| v < shown))
        .copied()
        .collect();
    let (pieces, seams) = common::skirt();
    let garment = assemble(&pieces, &seams, 20.0).expect("a garment");
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
        "settled after {:.1} s; mean speed {:.4} m/s",
        drape.time,
        drape.cloth.mean_speed()
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
    draw("settled", &drape);
}
