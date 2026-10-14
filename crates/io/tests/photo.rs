//! A paper pattern piece photographed beside a sheet of A4, at a slant:
//! found at its true size.

use drapewright_io::photo::{PhotoRequest, trace};

type P2 = [f64; 2];

/// The 3 × 3 map taking each of `from` to `to` (a homography).
fn homography(from: [P2; 4], to: [P2; 4]) -> [f64; 9] {
    let mut rows = [[0.0; 9]; 8];
    for i in 0..4 {
        let ([x, y], [u, v]) = (from[i], to[i]);
        rows[2 * i] = [x, y, 1.0, 0.0, 0.0, 0.0, -u * x, -u * y, u];
        rows[2 * i + 1] = [0.0, 0.0, 0.0, x, y, 1.0, -v * x, -v * y, v];
    }
    for c in 0..8 {
        let p = (c..8)
            .max_by(|&a, &b| rows[a][c].abs().total_cmp(&rows[b][c].abs()))
            .unwrap();
        rows.swap(c, p);
        for r in 0..8 {
            if r != c {
                let k = rows[r][c] / rows[c][c];
                let pivot = rows[c];
                for (x, q) in rows[r].iter_mut().zip(pivot) {
                    *x -= k * q;
                }
            }
        }
    }
    let h: Vec<f64> = (0..8).map(|r| rows[r][8] / rows[r][r]).collect();
    [h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], 1.0]
}

fn apply(m: &[f64; 9], [x, y]: P2) -> P2 {
    let w = m[6] * x + m[7] * y + m[8];
    [
        (m[0] * x + m[1] * y + m[2]) / w,
        (m[3] * x + m[4] * y + m[5]) / w,
    ]
}

/// The piece on the table (mm): a skirt panel, its side curved.
fn panel() -> Vec<P2> {
    let mut p = vec![[300.0, 60.0], [480.0, 70.0]];
    for k in 1..=20 {
        let t = f64::from(k) / 20.0;
        p.push([480.0 + 70.0 * t * t, 70.0 + 540.0 * t]);
    }
    p.push([280.0, 615.0]);
    p
}

fn inside(poly: &[P2], [x, y]: P2) -> bool {
    let mut odd = false;
    for i in 0..poly.len() {
        let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
        if (a[1] > y) != (b[1] > y) && x < a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
            odd = !odd;
        }
    }
    odd
}

fn area(poly: &[P2]) -> f64 {
    (0..poly.len())
        .map(|i| {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

/// The photo: the table (green), the sheet and the piece, seen at a
/// slant, lit unevenly, with a line printed across the piece; and where
/// the sheet's corners are in it.
fn photo() -> (Vec<u8>, (u32, u32), [P2; 4], [f64; 9]) {
    let corners = [
        [150.0, 260.0],
        [400.0, 240.0],
        [440.0, 600.0],
        [120.0, 640.0],
    ];
    photo_of((1400, 1300), corners, true)
}

/// The photo `size` pixels across, the sheet's corners at `corners`;
/// speckled, as a camera's are, if `grainy`.
fn photo_of(
    (w, h): (u32, u32),
    corners: [P2; 4],
    grainy: bool,
) -> (Vec<u8>, (u32, u32), [P2; 4], [f64; 9]) {
    let sheet = [[0.0, 0.0], [210.0, 0.0], [210.0, 297.0], [0.0, 297.0]];
    let to_table = homography(corners, sheet);
    let to_photo = homography(sheet, corners);
    let piece = panel();
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    let mut noise = 12345u32;
    for y in 0..h {
        for x in 0..w {
            let p = apply(&to_table, [f64::from(x) + 0.5, f64::from(y) + 0.5]);
            // Brighter to the left.
            let light = 1.0 - 0.25 * f64::from(x) / f64::from(w);
            let mut c = if (0.0..=210.0).contains(&p[0]) && (0.0..=297.0).contains(&p[1]) {
                [250.0, 250.0, 248.0]
            } else if inside(&piece, p) {
                // A grainline printed down it.
                if (p[0] - 400.0).abs() < 0.6 && (150.0..500.0).contains(&p[1]) {
                    [30.0, 30.0, 30.0]
                } else {
                    [238.0, 230.0, 210.0]
                }
            } else {
                [60.0, 125.0, 85.0]
            };
            noise = noise.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let jitter = if grainy {
                f64::from((noise >> 16) % 13) - 6.0
            } else {
                0.0
            };
            for v in &mut c {
                *v = (*v * light + jitter).clamp(0.0, 255.0);
            }
            let k = ((y * w + x) * 4) as usize;
            rgba[k..k + 4].copy_from_slice(&[c[0] as u8, c[1] as u8, c[2] as u8, 255]);
        }
    }
    (rgba, (w, h), corners, to_photo)
}

#[test]
fn a_photographed_piece_is_found_at_its_true_size() {
    let (rgba, size, corners, to_photo) = photo();
    let request = PhotoRequest {
        // Tapped in any order, a few pixels out.
        corners: [
            [corners[2][0] + 2.0, corners[2][1] - 1.0],
            [corners[0][0] - 1.0, corners[0][1] + 2.0],
            [corners[3][0], corners[3][1] + 1.0],
            [corners[1][0] + 1.0, corners[1][1]],
        ],
        sheet: [297.0, 210.0],
        on: apply(&to_photo, [380.0, 300.0]),
        tolerance: 0.25,
        name: "Skirt front".into(),
        file: "front.jpg".into(),
    };
    let traced = trace(&rgba, size, b"photo bytes", &request).unwrap();
    assert!(!traced.cut_off);
    let piece = &traced.result.pieces[0];
    assert_eq!(piece.name, "Skirt front");
    let bounds = piece.outline.bounds();
    let truth = panel();
    let (tw, th) = (550.0 - 280.0, 615.0 - 60.0);
    eprintln!(
        "{:.1} × {:.1} mm (truly {tw} × {th}); area {:.0} (truly {:.0}); {} edges",
        bounds.width(),
        bounds.height(),
        piece.outline.signed_area(),
        area(&truth),
        piece.outline.edges.len()
    );
    assert!((bounds.width() - tw).abs() < 6.0, "{}", bounds.width());
    assert!((bounds.height() - th).abs() < 6.0, "{}", bounds.height());
    // Clockwise, as outlines run, and as big as the piece.
    let a = piece.outline.signed_area();
    assert!((a / area(&truth) - 1.0).abs() < 0.03, "{a}");
    // Its four corners, as edges.
    assert_eq!(piece.outline.edges.len(), 4);
    assert!(traced.result.scale.certain);
    assert_eq!(traced.result.allowance.pieces.len(), 1);
    // Shown on the photo where it is: on the piece's edge, seen from above.
    let to_table = homography(
        corners,
        [[0.0, 0.0], [210.0, 0.0], [210.0, 297.0], [0.0, 297.0]],
    );
    let off_edge = |q: P2| {
        (0..truth.len())
            .map(|i| {
                let (a, b) = (truth[i], truth[(i + 1) % truth.len()]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let t = (((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1])
                    / (d[0] * d[0] + d[1] * d[1]))
                    .clamp(0.0, 1.0);
                (q[0] - a[0] - t * d[0]).hypot(q[1] - a[1] - t * d[1])
            })
            .fold(f64::MAX, f64::min)
    };
    let worst = traced
        .outline
        .iter()
        .map(|&p| off_edge(apply(&to_table, p)))
        .fold(0.0, f64::max);
    assert!(traced.outline.len() > 100);
    assert!(worst < 5.0, "shown {worst} mm off the piece's edge");
    let _ = to_photo;
}

#[test]
fn tapping_the_sheet_or_a_slanted_quad_is_explained() {
    let (rgba, size, corners, _) = photo();
    let request = PhotoRequest {
        corners,
        sheet: [210.0, 297.0],
        on: [270.0, 440.0],
        tolerance: 0.25,
        name: String::new(),
        file: "front.jpg".into(),
    };
    let err = trace(&rgba, size, b"", &request).unwrap_err().to_string();
    assert!(err.contains("on the sheet of paper"), "{err}");
    let flat = PhotoRequest {
        corners: [[0.0, 0.0], [10.0, 10.0], [20.0, 20.0], [30.0, 30.0]],
        ..request
    };
    let err = trace(&rgba, size, b"", &flat).unwrap_err().to_string();
    assert!(err.contains("rectangle"), "{err}");
}

/// Where the sheet's corners are in the app's test photo
/// (`fixtures/photo/piece-and-a4.png`), px; and a point on the piece.
const FIXTURE_CORNERS: [P2; 4] = [[70.0, 150.0], [205.0, 140.0], [222.0, 335.0], [55.0, 352.0]];

#[test]
fn the_photo_fixture_is_up_to_date() {
    let (rgba, (w, h), corners, to_photo) = photo_of((720, 900), FIXTURE_CORNERS, false);
    let on = apply(&to_photo, [380.0, 300.0]);
    // The app's test taps here (as a share of the photo across and down).
    eprintln!(
        "on the piece at {:.1}, {:.1} px ({:.4}, {:.4})",
        on[0],
        on[1],
        on[0] / f64::from(w),
        on[1] / f64::from(h)
    );
    let mut pixmap = tiny_skia::Pixmap::new(w, h).unwrap();
    pixmap.data_mut().copy_from_slice(&rgba);
    let png = pixmap.encode_png().unwrap();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/photo/piece-and-a4.png");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if std::fs::read(&path).ok().as_deref() != Some(&png[..]) {
        std::fs::write(&path, &png).unwrap();
    }
    // It is found there, the size it is.
    let request = PhotoRequest {
        corners,
        sheet: [210.0, 297.0],
        on,
        tolerance: 0.25,
        name: String::new(),
        file: "piece-and-a4.png".into(),
    };
    let traced = trace(&rgba, (w, h), &png, &request).unwrap();
    let bounds = traced.result.pieces[0].outline.bounds();
    eprintln!("{:.1} × {:.1} mm", bounds.width(), bounds.height());
    assert!((bounds.width() - 270.0).abs() < 8.0);
    assert!((bounds.height() - 555.0).abs() < 8.0);
}
