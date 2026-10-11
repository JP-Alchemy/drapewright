//! Writes a shaped figure as an OBJ file, to look at:
//!
//!     cargo run --release -p drapewright-body --example export_obj -- \
//!         assets/body/adult.dwbody out.obj [woman=1 weight=0.7 …]

use drapewright_body::{BodyModel, Shape};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [asset, out, settings @ ..] = args.as_slice() else {
        eprintln!("usage: export_obj <body file> <out.obj> [setting=value …]");
        std::process::exit(2);
    };
    let model =
        BodyModel::from_bytes(&std::fs::read(asset).expect("body file")).expect("a body file");
    let mut shape = Shape::default();
    for s in settings {
        let (key, value) = s.split_once('=').expect("setting=value");
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
    let mut text = String::new();
    let shown = model.asset.shown as usize;
    for p in &body.vertices[..shown] {
        text.push_str(&format!("v {:.2} {:.2} {:.2}\n", p[0], p[1], p[2]));
    }
    for t in &model.asset.triangles {
        text.push_str(&format!("f {} {} {}\n", t[0] + 1, t[1] + 1, t[2] + 1));
    }
    std::fs::write(out, text).expect("writing");
    let top = body.vertices[..shown]
        .iter()
        .map(|p| p[1])
        .fold(f64::MIN, f64::max);
    println!("{out}: {} vertices, {:.0} mm tall", shown, top);
}
