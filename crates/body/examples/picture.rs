//! A picture of a shaped figure, from the front and the side, to check by
//! eye:
//!
//!     cargo run --release -p drapewright-body --example picture -- \
//!         assets/body/adult.dwbody out.png [woman=1 weight=0.7 … pose=sitting]

use drapewright_body::{BodyModel, Pose, Rig, Shape};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, Transform};

/// Pixels per millimetre.
const SCALE: f32 = 0.3;

/// A triangle to draw: its depth, corners, shade and body region.
type Drawn = (f32, [(f32, f32); 3], f32, u8);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [asset, out, settings @ ..] = args.as_slice() else {
        eprintln!("usage: picture <body file> <out.png> [setting=value …]");
        std::process::exit(2);
    };
    let model =
        BodyModel::from_bytes(&std::fs::read(asset).expect("body file")).expect("a body file");
    let mut shape = Shape::default();
    let mut pose = Pose::Standing;
    for s in settings {
        let (key, value) = s.split_once('=').expect("setting=value");
        if key == "pose" {
            pose = match value {
                "arms-down" => Pose::ArmsDown,
                "reaching" => Pose::Reaching,
                "arms-up" => Pose::ArmsUp,
                "stepping" => Pose::Stepping,
                "sitting" => Pose::Sitting,
                _ => Pose::Standing,
            };
            continue;
        }
        let v: f64 = value.parse().expect("a number");
        match key {
            "woman" => shape.woman = v,
            "age" => shape.age = v,
            "muscle" => shape.muscle = v,
            "weight" => shape.weight = v,
            "height" => shape.height = v,
            "proportions" => shape.proportions = v,
            "cup" => shape.cup = v,
            "firmness" => shape.firmness = v,
            other => {
                shape.local.insert(other.to_string(), v);
            }
        }
    }
    let body = model.shape(&shape);
    let body = Rig::new(&model, &body)
        .expect("joints")
        .pose(&body, pose, 1.0);
    let shown = model.asset.shown as usize;
    let height = body.vertices[..shown]
        .iter()
        .map(|p| p[1])
        .fold(0.0, f64::max) as f32;
    let (w, h) = ((2400.0 * SCALE) as u32, ((height + 100.0) * SCALE) as u32);
    let mut pixmap = Pixmap::new(w, h).expect("a picture");
    pixmap.fill(Color::from_rgba8(250, 248, 244, 255));
    // Front view on the left (looking at the figure), side view on the right
    // (the figure facing right).
    for (view, offset) in [(0, 600.0f32), (1, 1700.0f32)] {
        let project = |p: &[f64; 3]| -> (f32, f32, f32) {
            let (x, depth) = if view == 0 {
                (p[0] as f32, p[2] as f32)
            } else {
                (p[2] as f32, -p[0] as f32)
            };
            (
                (offset + x) * SCALE,
                (height + 50.0 - p[1] as f32) * SCALE,
                depth,
            )
        };
        let mut tris: Vec<Drawn> = model
            .asset
            .triangles
            .iter()
            .filter_map(|t| {
                let [a, b, c] = t.map(|i| body.vertices[i as usize]);
                let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
                let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
                let n = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-9);
                let towards = if view == 0 { n[2] } else { -n[0] } / len;
                if towards <= 0.0 {
                    return None;
                }
                let light = (0.35 + 0.65 * towards) as f32;
                let (pa, pb, pc) = (project(&a), project(&b), project(&c));
                let region = model.asset.regions[t[0] as usize];
                Some((
                    (pa.2 + pb.2 + pc.2) / 3.0,
                    [(pa.0, pa.1), (pb.0, pb.1), (pc.0, pc.1)],
                    light,
                    region,
                ))
            })
            .collect();
        tris.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (_, pts, light, region) in tris {
            let mut pb = PathBuilder::new();
            pb.move_to(pts[0].0, pts[0].1);
            pb.line_to(pts[1].0, pts[1].1);
            pb.line_to(pts[2].0, pts[2].1);
            pb.close();
            let Some(path) = pb.finish() else { continue };
            let mut paint = Paint::default();
            let shade = |c: f32| (c * light).clamp(0.0, 255.0) as u8;
            if std::env::var_os("REGIONS").is_some() {
                // Each region its own colour, to check the regions by eye.
                let [r, g, b] = [
                    [200, 200, 200],
                    [230, 150, 40],
                    [80, 160, 220],
                    [220, 60, 60],
                    [220, 60, 60],
                    [90, 200, 90],
                    [90, 200, 90],
                    [160, 90, 200],
                    [160, 90, 200],
                    [230, 220, 60],
                    [230, 220, 60],
                    [120, 120, 120],
                    [120, 120, 120],
                    [0, 0, 0],
                ][region as usize];
                paint.set_color_rgba8(shade(r as f32), shade(g as f32), shade(b as f32), 255);
            } else {
                paint.set_color_rgba8(shade(222.0), shade(190.0), shade(170.0), 255);
            }
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
    // Where the measurements were taken: levels as lines, points as dots,
    // on the front view.
    let (measures, marks) = drapewright_body::measure(&model, &body);
    let to_px = |x: f64, y: f64| {
        (
            (600.0 + x as f32) * SCALE,
            (height + 50.0 - y as f32) * SCALE,
        )
    };
    for (name, y) in &marks.levels {
        let (x0, py) = to_px(-300.0, *y);
        let (x1, _) = to_px(300.0, *y);
        let mut pb = PathBuilder::new();
        pb.move_to(x0, py);
        pb.line_to(x1, py);
        if let Some(path) = pb.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(40, 90, 220, 255);
            let stroke = tiny_skia::Stroke {
                width: 1.5,
                ..Default::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
        println!("  level {name}: {y:.0} mm");
    }
    for (name, p) in &marks.points {
        let (x, y) = to_px(p[0], p[1]);
        if let Some(path) = PathBuilder::from_circle(x, y, 4.0) {
            let mut paint = Paint::default();
            paint.set_color_rgba8(220, 40, 40, 255);
            pixmap.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
        println!("  point {name}: {:.0}, {:.0}, {:.0}", p[0], p[1], p[2]);
    }
    for (name, v) in &measures {
        println!("  {name}: {:.1} cm", v / 10.0);
    }
    pixmap.save_png(out).expect("writing the picture");
    println!("{out}: {:.0} mm tall", height);
}
