//! Builds the body file from a local copy of Anny's data (its MakeHuman /
//! MPFB2 assets, CC0):
//!
//!     cargo run --release -p drapewright-body --example build_body -- \
//!         <anny>/src/anny/data assets/body/adult.dwbody
//!
//! It keeps MakeHuman's body mesh with Anny's edits (two holes capped,
//! unattached vertices left out, quads cut into triangles), the eyes, and
//! the joint markers the measurements use. The adult macro shapes (young
//! to old, every muscle, weight, height, proportions, cup size and
//! firmness, the ethnic shapes averaged as Anny does by default) are
//! stored as principal components, as many as it takes to rebuild each
//! shape to within `TOLERANCE`. The local shapes the fitting uses are
//! stored as they are, sparse. Only files are read; nothing is run.

use std::collections::{BTreeMap, HashMap};
use std::io::Read;
use std::path::{Path, PathBuf};

use drapewright_body::asset::{
    BodyAsset, Component, Joint, LOCAL_UNIT, LocalShape, MacroShape, Region, Sparse,
};
use drapewright_body::shape::{
    AGE, CUP, FIRMNESS, GENDER, HEIGHT, MUSCLE, OLD, PROPORTIONS, Shape, VARIATIONS, WEIGHT, YOUNG,
};
use flate2::read::GzDecoder;

/// How close rebuilt test figures must be to the real ones: 95 in 100 of
/// them nowhere further than this, mm.
const TOLERANCE: f64 = 2.0;
/// Figures sampled for the components, and drawn afresh to test them.
const SAMPLES: usize = 4000;
const TESTS: usize = 300;
/// Never more components than this.
const MOST_COMPONENTS: usize = 120;

/// A small, fixed random sequence (xorshift), so the file is the same each
/// time it is built.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> f64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A woman's or a man's figure, anywhere in the other settings (the app
    /// offers no figure in between).
    fn figure(&mut self) -> Shape {
        Shape {
            woman: if self.next() < 0.5 { 1.0 } else { 0.0 },
            age: YOUNG + (OLD - YOUNG) * self.next(),
            muscle: self.next(),
            weight: self.next(),
            height: self.next(),
            proportions: self.next(),
            cup: self.next(),
            firmness: self.next(),
            local: Default::default(),
        }
    }
}
/// MakeHuman's units are decimetres.
const MM: f64 = 100.0;

/// Vertices whose faces Anny drops (`get_edited_mesh_faces`), and the
/// faces it closes the holes with.
const DROPPED: [std::ops::RangeInclusive<usize>; 2] = [1778..=1793, 8450..=8465];
const CAPS: [[usize; 4]; 14] = [
    [8437, 8438, 8439, 8440],
    [8436, 8437, 8440, 8441],
    [8435, 8436, 8441, 8442],
    [8434, 8435, 8442, 8443],
    [8449, 8434, 8443, 8444],
    [8448, 8449, 8444, 8445],
    [8447, 8448, 8445, 8446],
    [1762, 1771, 1770, 1763],
    [1763, 1770, 1769, 1764],
    [1764, 1769, 1768, 1765],
    [1765, 1768, 1767, 1766],
    [1762, 1777, 1772, 1771],
    [1777, 1776, 1773, 1772],
    [1776, 1775, 1774, 1773],
];

/// The joint markers measurements use.
const JOINTS: [&str; 22] = [
    "joint-neck",
    "joint-head",
    "joint-pelvis",
    "joint-spine-1",
    "joint-spine-2",
    "joint-spine-3",
    "joint-spine-4",
    "joint-l-clavicle",
    "joint-r-clavicle",
    "joint-l-shoulder",
    "joint-r-shoulder",
    "joint-l-elbow",
    "joint-r-elbow",
    "joint-l-hand",
    "joint-r-hand",
    "joint-l-upper-leg",
    "joint-r-upper-leg",
    "joint-l-knee",
    "joint-r-knee",
    "joint-l-ankle",
    "joint-r-ankle",
    "joint-ground",
];

/// The local shapes the fitting can use: MakeHuman's measure shapes, and a
/// few for shapes a tape doesn't see (a belly, a seat, a bust's height).
const LOCALS: [&str; 25] = [
    "measure-bust-circ-incr",
    "measure-underbust-circ-incr",
    "measure-waist-circ-incr",
    "measure-hips-circ-incr",
    "measure-neck-circ-incr",
    "measure-neck-height-incr",
    "measure-shoulder-dist-incr",
    "measure-frontchest-dist-incr",
    "measure-napetowaist-dist-incr",
    "measure-waisttohip-dist-incr",
    "measure-upperarm-circ-incr",
    "measure-upperarm-length-incr",
    "measure-lowerarm-length-incr",
    "measure-wrist-circ-incr",
    "measure-thigh-circ-incr",
    "measure-knee-circ-incr",
    "measure-calf-circ-incr",
    "measure-ankle-circ-incr",
    "measure-upperleg-height-incr",
    "measure-lowerleg-height-incr",
    "stomach-pregnant-incr",
    "buttocks-volume-incr",
    "breast-dist-incr",
    "breast-trans-up",
    "hip-scale-depth-incr",
];

const NOTICE: &str = "Drapewright body model. Mesh, shapes and skin weights from MakeHuman via \
MPFB2 (CC0 1.0), as prepared in Anny (Copyright 2025 NAVER Corp., Apache License 2.0): the \
adult shape blending and the mesh edits follow Anny. Built by Drapewright's build_body example.";

/// A macro shape: the variations that weigh it (feature, variation), and its
/// offsets (mm, x, y, z in turn).
type Recipe = (Vec<(u8, u8)>, Vec<f64>);

struct Obj {
    vertices: Vec<[f64; 3]>,
    groups: HashMap<String, Vec<Vec<usize>>>,
}

fn read_obj(path: &Path) -> Obj {
    let text = std::fs::read_to_string(path).expect("base.obj");
    let mut obj = Obj {
        vertices: Vec::new(),
        groups: HashMap::new(),
    };
    let mut group = String::new();
    for line in text.lines() {
        let mut parts = line.split_whitespace();
        match parts.next() {
            Some("v") => {
                let v: Vec<f64> = parts.map(|p| p.parse().expect("a number")).collect();
                obj.vertices.push([v[0], v[1], v[2]]);
            }
            Some("g") => group = parts.next().unwrap_or_default().to_string(),
            Some("f") => {
                let face = parts
                    .map(|p| p.split('/').next().unwrap().parse::<usize>().unwrap() - 1)
                    .collect();
                obj.groups.entry(group.clone()).or_default().push(face);
            }
            _ => {}
        }
    }
    obj
}

/// A target file: vertex (in the base mesh) → offset, decimetres.
fn read_target(path: &Path) -> Vec<(usize, [f64; 3])> {
    let mut text = String::new();
    GzDecoder::new(std::fs::File::open(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())))
        .read_to_string(&mut text)
        .expect("a gzipped target");
    text.lines()
        .filter_map(|line| {
            let p: Vec<&str> = line.split_whitespace().collect();
            (p.len() == 4).then(|| {
                let f = |s: &str| s.parse::<f64>().expect("a number");
                (p[0].parse().expect("a vertex"), [f(p[1]), f(p[2]), f(p[3])])
            })
        })
        .collect()
}

/// Eigenvalues and eigenvectors (columns of the second) of a symmetric
/// matrix, by cyclic Jacobi rotations.
#[allow(clippy::needless_range_loop)] // Row and column indices read as the maths does.
fn eigen(mut a: Vec<Vec<f64>>) -> (Vec<f64>, Vec<Vec<f64>>) {
    let n = a.len();
    let mut v: Vec<Vec<f64>> = (0..n)
        .map(|i| (0..n).map(|j| if i == j { 1.0 } else { 0.0 }).collect())
        .collect();
    let scale: f64 = (0..n).map(|i| a[i][i].abs()).sum::<f64>().max(1e-300);
    for _ in 0..60 {
        let off: f64 = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .map(|(i, j)| a[i][j] * a[i][j])
            .sum();
        if off.sqrt() < 1e-14 * scale {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = if theta >= 0.0 { 1.0 } else { -1.0 }
                    / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for row in a.iter_mut() {
                    let (kp, kq) = (row[p], row[q]);
                    row[p] = c * kp - s * kq;
                    row[q] = s * kp + c * kq;
                }
                for k in 0..n {
                    let (pk, qk) = (a[p][k], a[q][k]);
                    a[p][k] = c * pk - s * qk;
                    a[q][k] = s * pk + c * qk;
                }
                for row in v.iter_mut() {
                    let (kp, kq) = (row[p], row[q]);
                    row[p] = c * kp - s * kq;
                    row[q] = s * kp + c * kq;
                }
            }
        }
    }
    ((0..n).map(|i| a[i][i]).collect(), v)
}

/// The region a bone moves.
fn region_of(bone: &str) -> Region {
    let side = |l: Region, r: Region| if bone.ends_with(".L") { l } else { r };
    if bone.starts_with("neck") {
        Region::Neck
    } else if bone.starts_with("shoulder") {
        side(Region::ShoulderLeft, Region::ShoulderRight)
    } else if bone.starts_with("upperarm") || bone.starts_with("lowerarm") {
        side(Region::ArmLeft, Region::ArmRight)
    } else if ["wrist", "finger", "metacarpal"]
        .iter()
        .any(|p| bone.starts_with(p))
    {
        side(Region::HandLeft, Region::HandRight)
    } else if bone.starts_with("upperleg") || bone.starts_with("lowerleg") {
        side(Region::LegLeft, Region::LegRight)
    } else if bone.starts_with("foot") || bone.starts_with("toe") {
        side(Region::FootLeft, Region::FootRight)
    } else if ["spine", "pelvis", "breast", "clavicle", "root"]
        .iter()
        .any(|p| bone.starts_with(p))
    {
        Region::Torso
    } else {
        Region::Head
    }
}

fn main() {
    let args: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let [data, out] = args.as_slice() else {
        eprintln!("usage: build_body <anny>/src/anny/data <out.dwbody>");
        std::process::exit(2);
    };
    let mpfb = data.join("mpfb2");
    let obj = read_obj(&mpfb.join("3dobjs/base.obj"));

    // The faces drawn: the body with Anny's edits, and the eyes.
    let dropped = |v: usize| DROPPED.iter().any(|r| r.contains(&v));
    let mut faces: Vec<Vec<usize>> = obj.groups["body"]
        .iter()
        .filter(|f| !f.iter().any(|&v| dropped(v)))
        .cloned()
        .collect();
    faces.extend(CAPS.iter().map(|c| c.to_vec()));
    for eye in ["helper-l-eye", "helper-r-eye"] {
        faces.extend(obj.groups[eye].iter().cloned());
    }
    let mut shown: Vec<usize> = faces.iter().flatten().copied().collect();
    shown.sort_unstable();
    shown.dedup();
    let mut order = shown.clone();
    let mut joints = Vec::new();
    for name in JOINTS {
        let mut cube: Vec<usize> = obj.groups[name].iter().flatten().copied().collect();
        cube.sort_unstable();
        cube.dedup();
        assert_eq!(cube.len(), 8, "{name}");
        joints.push(Joint {
            name: name.trim_start_matches("joint-").to_string(),
            first: order.len() as u32,
        });
        order.extend(cube);
    }
    let index: HashMap<usize, u32> = order
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i as u32))
        .collect();
    let n = order.len();
    let base: Vec<[f64; 3]> = order
        .iter()
        .map(|&v| obj.vertices[v].map(|c| c * MM))
        .collect();
    let mut triangles: Vec<[u32; 3]> = faces
        .iter()
        .flat_map(|f| {
            let f: Vec<u32> = f.iter().map(|v| index[v]).collect();
            (1..f.len() - 1).map(move |k| [f[0], f[k], f[k + 1]])
        })
        .collect();
    // Outward facing: the body's signed volume is positive.
    let volume: f64 = triangles
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| base[i as usize]);
            (a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                + a[2] * (b[0] * c[1] - b[1] * c[0]))
                / 6.0
        })
        .sum();
    if volume < 0.0 {
        triangles.iter_mut().for_each(|t| t.swap(1, 2));
    }
    println!(
        "mesh: {} vertices drawn, {} joint markers, {} triangles, volume {:.1} L",
        shown.len(),
        joints.len(),
        triangles.len(),
        volume.abs() / 1e6
    );

    // Regions, from the bone that moves each vertex most.
    let weights: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(mpfb.join("rigs/standard/weights.default.json")).expect("weights"),
    )
    .expect("JSON");
    let mut best: Vec<(f64, Region)> = vec![(0.0, Region::Torso); n];
    for (bone, list) in weights["weights"].as_object().expect("weights by bone") {
        let region = region_of(bone);
        for pair in list.as_array().expect("pairs") {
            let v = pair[0].as_u64().expect("vertex") as usize;
            let w = pair[1].as_f64().expect("weight");
            if let Some(&i) = index.get(&v)
                && w > best[i as usize].0
            {
                best[i as usize] = (w, region);
            }
        }
    }
    let mut regions: Vec<u8> = best.iter().map(|(_, r)| *r as u8).collect();
    for r in &mut regions[shown.len()..] {
        *r = Region::Joint as u8;
    }
    let mut counts: BTreeMap<u8, usize> = BTreeMap::new();
    regions
        .iter()
        .for_each(|r| *counts.entry(*r).or_default() += 1);
    println!("regions: {counts:?}");

    // The adult macro shapes, each with the variations that weigh it.
    let dir = mpfb.join("targets");
    let name = |f: u8, v: usize| VARIATIONS[f as usize][v];
    let dense = |entries: &[(usize, [f64; 3])], scale: f64| {
        let mut d = vec![0.0f64; 3 * n];
        for (v, o) in entries {
            if let Some(&i) = index.get(v) {
                for k in 0..3 {
                    d[3 * i as usize + k] += o[k] * MM * scale;
                }
            }
        }
        d
    };
    let mut shapes: Vec<Recipe> = Vec::new();
    for g in 0..2 {
        for a in 0..2 {
            let (gn, an) = (name(GENDER, g), name(AGE, a));
            // The three ethnic shapes in equal parts.
            let mut avg = vec![0.0; 3 * n];
            for race in ["african", "asian", "caucasian"] {
                let t = dense(
                    &read_target(&dir.join(format!("macrodetails/{race}-{gn}-{an}.target.gz"))),
                    1.0 / 3.0,
                );
                avg.iter_mut().zip(t).for_each(|(x, y)| *x += y);
            }
            shapes.push((vec![(GENDER, g as u8), (AGE, a as u8)], avg));
            for m in 0..3 {
                for w in 0..3 {
                    let (mn, wn) = (name(MUSCLE, m), name(WEIGHT, w));
                    let base_factors = vec![
                        (GENDER, g as u8),
                        (AGE, a as u8),
                        (MUSCLE, m as u8),
                        (WEIGHT, w as u8),
                    ];
                    let path = dir.join(format!(
                        "macrodetails/universal-{gn}-{an}-{mn}-{wn}.target.gz"
                    ));
                    shapes.push((base_factors.clone(), dense(&read_target(&path), 1.0)));
                    for h in 0..2 {
                        let path = dir.join(format!(
                            "macrodetails/height/{gn}-{an}-{mn}-{wn}-{}.target.gz",
                            name(HEIGHT, h)
                        ));
                        let mut f = base_factors.clone();
                        f.push((HEIGHT, h as u8));
                        shapes.push((f, dense(&read_target(&path), 1.0)));
                    }
                    for p in 0..2 {
                        let path = dir.join(format!(
                            "macrodetails/proportions/{gn}-{an}-{mn}-{wn}-{}.target.gz",
                            name(PROPORTIONS, p)
                        ));
                        let mut f = base_factors.clone();
                        f.push((PROPORTIONS, p as u8));
                        shapes.push((f, dense(&read_target(&path), 1.0)));
                    }
                    if g == 1 {
                        for c in 0..3 {
                            for fi in 0..3 {
                                let path = dir.join(format!(
                                    "breast/female-{an}-{mn}-{wn}-{}-{}.target.gz",
                                    name(CUP, c),
                                    name(FIRMNESS, fi)
                                ));
                                if path.exists() {
                                    let mut f = base_factors.clone();
                                    f.push((CUP, c as u8));
                                    f.push((FIRMNESS, fi as u8));
                                    shapes.push((f, dense(&read_target(&path), 1.0)));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let count = shapes.len();
    println!("macro shapes: {count}");

    // How far a rebuilt figure strays: anywhere, and on the parts clothes
    // cover.
    let worst = |residual: &[Vec<f64>]| {
        residual
            .iter()
            .flat_map(|r| {
                r.chunks_exact(3)
                    .map(|d| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
            })
            .fold(0.0f64, f64::max)
    };
    let clothed: Vec<bool> = regions
        .iter()
        .map(|&r| {
            matches!(
                Region::from_u8(r),
                Some(
                    Region::Torso
                        | Region::Neck
                        | Region::ShoulderLeft
                        | Region::ShoulderRight
                        | Region::ArmLeft
                        | Region::ArmRight
                        | Region::LegLeft
                        | Region::LegRight
                )
            )
        })
        .collect();
    let worst_clothed = |residual: &[Vec<f64>]| {
        residual
            .iter()
            .flat_map(|r| {
                r.chunks_exact(3)
                    .zip(&clothed)
                    .filter(|(_, c)| **c)
                    .map(|(d, _)| (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt())
            })
            .fold(0.0f64, f64::max)
    };
    // Principal components of figures, not of shapes: figures are blends
    // of shapes, and far fewer components describe them. Figures are
    // sampled across the settings; their spread in shape weights (C) and
    // the shapes' overlaps (G) give the components without building any
    // figure: they are the eigenvectors of C½ G C½.
    let weights_of = |s: &Shape| {
        let w = s.variation_weights();
        shapes
            .iter()
            .map(|(factors, _)| {
                factors
                    .iter()
                    .map(|&(f, v)| w[f as usize][v as usize])
                    .product::<f64>()
            })
            .collect::<Vec<f64>>()
    };
    let mut rng = Rng(0x5eed_d0a9e);
    let samples: Vec<Vec<f64>> = (0..SAMPLES).map(|_| weights_of(&rng.figure())).collect();
    let mean_w: Vec<f64> = (0..count)
        .map(|i| samples.iter().map(|w| w[i]).sum::<f64>() / SAMPLES as f64)
        .collect();
    let mut spread = vec![vec![0.0f64; count]; count];
    for w in &samples {
        for i in 0..count {
            let di = w[i] - mean_w[i];
            if di == 0.0 {
                continue;
            }
            for j in 0..count {
                spread[i][j] += di * (w[j] - mean_w[j]) / SAMPLES as f64;
            }
        }
    }
    let (sv, svec) = eigen(spread);
    // C½ = V √Λ Vᵀ.
    let half: Vec<Vec<f64>> = (0..count)
        .map(|i| {
            (0..count)
                .map(|j| {
                    (0..count)
                        .map(|k| svec[i][k] * sv[k].max(0.0).sqrt() * svec[j][k])
                        .sum()
                })
                .collect()
        })
        .collect();
    let gram: Vec<Vec<f64>> = (0..count)
        .map(|i| {
            (0..count)
                .map(|j| {
                    shapes[i]
                        .1
                        .iter()
                        .zip(&shapes[j].1)
                        .map(|(x, y)| x * y)
                        .sum()
                })
                .collect()
        })
        .collect();
    let product = |a: &Vec<Vec<f64>>, b: &Vec<Vec<f64>>| -> Vec<Vec<f64>> {
        (0..count)
            .map(|i| {
                (0..count)
                    .map(|j| (0..count).map(|k| a[i][k] * b[k][j]).sum())
                    .collect()
            })
            .collect()
    };
    let (values, vectors) = eigen(product(&product(&half, &gram), &half));
    let mut ranked: Vec<usize> = (0..count).collect();
    ranked.sort_by(|&a, &b| values[b].total_cmp(&values[a]));
    // The mean figure's offsets.
    let mut mean = vec![0.0f64; 3 * n];
    for (w, s) in mean_w.iter().zip(&shapes) {
        mean.iter_mut().zip(&s.1).for_each(|(m, x)| *m += w * x);
    }
    // Test figures, drawn afresh, to judge how many components it takes.
    let tests: Vec<Vec<f64>> = (0..TESTS)
        .map(|_| {
            let w = weights_of(&rng.figure());
            let mut f = vec![0.0f64; 3 * n];
            for (wi, s) in w.iter().zip(&shapes) {
                if *wi != 0.0 {
                    f.iter_mut().zip(&s.1).for_each(|(x, y)| *x += wi * y);
                }
            }
            f.iter_mut().zip(&mean).for_each(|(x, m)| *x -= m);
            f
        })
        .collect();
    let mut residual = tests.clone();
    let mut components = Vec::new();
    let mut coefficients: Vec<Vec<f32>> = vec![Vec::new(); count];
    let mut mean_mix = Vec::new();
    // Each test figure's largest error: the 95th percentile.
    let typical = |residual: &[Vec<f64>]| {
        let mut each: Vec<f64> = residual
            .iter()
            .map(|r| worst(std::slice::from_ref(r)))
            .collect();
        each.sort_by(f64::total_cmp);
        each[each.len() * 95 / 100]
    };
    for &k in &ranked {
        let error = typical(&residual);
        if std::env::var_os("CURVE").is_some() && components.len() % 5 == 0 {
            // Each test figure's largest error, sorted: the median and the
            // 95th percentile figure.
            let mut each: Vec<f64> = residual
                .iter()
                .map(|r| worst(std::slice::from_ref(r)))
                .collect();
            each.sort_by(f64::total_cmp);
            println!(
                "  {} components: worst {error:.2} mm, median figure {:.2} mm, 95% {:.2} mm",
                components.len(),
                each[each.len() / 2],
                each[each.len() * 95 / 100]
            );
        }
        if error <= TOLERANCE || values[k] <= 0.0 || components.len() == MOST_COMPONENTS {
            break;
        }
        // The component: Tᵀ C½ y, made unit length.
        let a: Vec<f64> = (0..count)
            .map(|i| (0..count).map(|j| half[i][j] * vectors[j][k]).sum())
            .collect();
        let mut basis = vec![0.0f64; 3 * n];
        for (ai, s) in a.iter().zip(&shapes) {
            basis.iter_mut().zip(&s.1).for_each(|(b, x)| *b += ai * x);
        }
        let len = basis.iter().map(|x| x * x).sum::<f64>().sqrt();
        basis.iter_mut().for_each(|b| *b /= len);
        let largest = basis.iter().fold(0.0f64, |m, x| m.max(x.abs())).max(1e-12);
        let scale = largest / f64::from(i16::MAX);
        let values16: Vec<i16> = basis.iter().map(|x| (x / scale).round() as i16).collect();
        let stored: Vec<f64> = values16.iter().map(|&q| f64::from(q) * scale).collect();
        let norm2: f64 = stored.iter().map(|y| y * y).sum();
        // Each shape's part in this component, and the mean figure's.
        let mut mixed = 0.0;
        for (i, s) in shapes.iter().enumerate() {
            let c = s.1.iter().zip(&stored).map(|(x, y)| x * y).sum::<f64>() / norm2;
            coefficients[i].push(c as f32);
            mixed += mean_w[i] * c;
        }
        mean_mix.push(mixed as f32);
        for r in &mut residual {
            let c = r.iter().zip(&stored).map(|(x, y)| x * y).sum::<f64>() / norm2;
            r.iter_mut().zip(&stored).for_each(|(x, y)| *x -= c * y);
        }
        components.push(Component {
            scale: scale as f32,
            values: values16,
        });
    }
    println!(
        "components: {} (95% of test figures within {:.2} mm; the worst {:.2} mm, on clothed parts {:.2} mm)",
        components.len(),
        typical(&residual),
        worst(&residual),
        worst_clothed(&residual)
    );
    let macros: Vec<MacroShape> = shapes
        .iter()
        .zip(coefficients)
        .map(|((factors, _), coefficients)| MacroShape {
            factors: factors.clone(),
            coefficients,
        })
        .collect();

    // Local shapes, sparse, with their opposites from target.json.
    let catalogue: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join("target.json")).expect("target.json"),
    )
    .expect("JSON");
    let mut locals = Vec::new();
    for wanted in LOCALS {
        let mut found = None;
        for (folder, meta) in catalogue.as_object().expect("by folder") {
            for category in meta["categories"].as_array().into_iter().flatten() {
                let o = &category["opposites"];
                if o["positive-unsided"].as_str() == Some(wanted) {
                    found = Some((
                        folder.clone(),
                        o["negative-unsided"]
                            .as_str()
                            .expect("opposite")
                            .to_string(),
                    ));
                }
            }
        }
        let (folder, opposite) = found.unwrap_or_else(|| panic!("no local shape {wanted}"));
        let sparse = |file: &str| {
            let mut s = Sparse::default();
            let mut entries: Vec<(u32, [i16; 3])> =
                read_target(&dir.join(&folder).join(format!("{file}.target.gz")))
                    .into_iter()
                    .filter_map(|(v, o)| {
                        let i = *index.get(&v)?;
                        let q = o.map(|c| (c * MM / f64::from(LOCAL_UNIT)).round() as i16);
                        (q != [0, 0, 0]).then_some((i, q))
                    })
                    .collect();
            entries.sort_by_key(|e| e.0);
            for (i, q) in entries {
                s.vertices.push(i);
                s.offsets.push(q);
            }
            s
        };
        locals.push(LocalShape {
            name: wanted.to_string(),
            increase: sparse(wanted),
            decrease: sparse(&opposite),
        });
    }
    println!("local shapes: {}", locals.len());

    let asset = BodyAsset {
        notice: NOTICE.to_string(),
        base: base.iter().map(|p| p.map(|c| c as f32)).collect(),
        triangles,
        shown: shown.len() as u32,
        regions,
        joints,
        mean: mean.iter().map(|&m| m as f32).collect(),
        mean_mix,
        components,
        macros,
        locals,
    };
    let bytes = asset.to_bytes();
    std::fs::write(out, &bytes).expect("writing the body file");
    println!(
        "wrote {} ({:.1} MB)",
        out.display(),
        bytes.len() as f64 / 1e6
    );
}
