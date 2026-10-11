//! Fits a figure to measurements given in centimetres and prints how close
//! it comes:
//!
//!     cargo run --release -p drapewright-body --example fit -- \
//!         assets/body/adult.dwbody woman height=168 bust=88 waist=70 hip=96

use std::collections::BTreeMap;
use std::time::Instant;

use drapewright_body::{BodyModel, fit};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [asset, figure, values @ ..] = args.as_slice() else {
        eprintln!("usage: fit <body file> woman|man [name=cm …]");
        std::process::exit(2);
    };
    let model =
        BodyModel::from_bytes(&std::fs::read(asset).expect("body file")).expect("a body file");
    let targets: BTreeMap<String, f64> = values
        .iter()
        .map(|v| {
            let (k, cm) = v.split_once('=').expect("name=cm");
            (
                k.to_string(),
                cm.parse::<f64>().expect("centimetres") * 10.0,
            )
        })
        .collect();
    let start = Instant::now();
    let result = fit(&model, &targets, figure == "woman", 35.0);
    println!("fitted in {:.0} ms", start.elapsed().as_secs_f64() * 1000.0);
    let s = &result.shape;
    println!(
        "settings: height {:.2} weight {:.2} muscle {:.2} cup {:.2}",
        s.height, s.weight, s.muscle, s.cup
    );
    for (k, v) in &s.local {
        println!("  {k}: {v:+.2}");
    }
    for (k, t) in &targets {
        let r = result.residuals.get(k).copied();
        println!(
            "  {k:16} wanted {:6.1} cm, figure {}",
            t / 10.0,
            r.map_or("—".into(), |r| format!(
                "{:6.1} cm ({:+.1})",
                (t + r) / 10.0,
                r / 10.0
            ))
        );
    }
}
