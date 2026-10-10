//! A gallery of seam allowance cases, written as SVG files to
//! `tests/gallery/` for people to look at, and summarised in a snapshot so
//! any change shows up in review.

use std::fmt::Write as _;
use std::path::Path;
use std::time::Instant;

use kurbo::Point;
use soom_pattern_core::allowance::allowance_lines;
use soom_pattern_core::{AllowanceMode, CornerStyle, Edge, EdgeKind, Outline, Piece, Pt, Segment};

const K: f64 = 0.552_284_749_830_793_4;

fn piece(name: &str, start: [f64; 2], edges: Vec<Edge>, seam: f64, hem: Option<f64>) -> Piece {
    let mut p = Piece::new(
        "p1",
        name,
        Outline {
            start: Pt::from(start),
            edges,
        },
    );
    p.allowance.seam = seam;
    p.allowance.hem = hem;
    p
}

fn edge(id: &str, segments: Vec<Segment>) -> Edge {
    Edge::new(id, segments)
}

fn kind(mut e: Edge, kind: EdgeKind) -> Edge {
    e.kind = kind;
    e
}

fn corner(mut e: Edge, style: CornerStyle) -> Edge {
    e.end_corner = style;
    e
}

/// A half front bodice: curved neckline and armhole, centre front on the
/// fold, waist as a hem.
fn bodice() -> Piece {
    piece(
        "Bodice front",
        [0.0, 85.0],
        vec![
            edge(
                "neckline",
                vec![Segment::cubic([40.0, 85.0], [72.0, 40.0], [75.0, 0.0])],
            ),
            edge("shoulder", vec![Segment::line([195.0, 45.0])]),
            edge(
                "armhole",
                vec![Segment::cubic(
                    [180.0, 110.0],
                    [185.0, 215.0],
                    [230.0, 230.0],
                )],
            ),
            edge("side", vec![Segment::line([215.0, 420.0])]),
            kind(
                edge("waist", vec![Segment::line([0.0, 430.0])]),
                EdgeKind::Hem,
            ),
            kind(edge("cf", vec![Segment::line([0.0, 85.0])]), EdgeKind::Fold),
        ],
        15.0,
        Some(30.0),
    )
}

fn cases() -> Vec<(&'static str, &'static str, Piece)> {
    let rect = |name| {
        piece(
            name,
            [0.0, 0.0],
            vec![
                edge("top", vec![Segment::line([200.0, 0.0])]),
                edge("right", vec![Segment::line([200.0, 100.0])]),
                edge("bottom", vec![Segment::line([0.0, 100.0])]),
                edge("left", vec![Segment::line([0.0, 0.0])]),
            ],
            10.0,
            None,
        )
    };
    let mut mixed = rect("Mixed widths");
    mixed.outline.edges[0].allowance = Some(25.0);
    mixed.outline.edges[1].allowance = Some(5.0);
    mixed.outline.edges[2].allowance = Some(0.0);

    let skirt = piece(
        "Flared skirt",
        [0.0, 0.0],
        vec![
            edge(
                "waist",
                vec![Segment::cubic([70.0, 4.0], [140.0, 8.0], [200.0, 12.0])],
            ),
            edge("side", vec![Segment::line([300.0, 600.0])]),
            kind(
                edge(
                    "hem",
                    vec![Segment::cubic([200.0, 615.0], [100.0, 620.0], [0.0, 620.0])],
                ),
                EdgeKind::Hem,
            ),
            kind(edge("cf", vec![Segment::line([0.0, 0.0])]), EdgeKind::Fold),
        ],
        15.0,
        Some(40.0),
    );

    let flare = 20f64.to_radians();
    let wide = piece(
        "Deep hem, wide flare",
        [0.0, 0.0],
        vec![
            edge("waist", vec![Segment::line([200.0, 0.0])]),
            edge(
                "side",
                vec![Segment::line([200.0 + 400.0 * flare.tan(), 400.0])],
            ),
            kind(
                edge("hem", vec![Segment::line([0.0, 400.0])]),
                EdgeKind::Hem,
            ),
            edge("cf", vec![Segment::line([0.0, 0.0])]),
        ],
        15.0,
        Some(50.0),
    );

    let tapered = piece(
        "Tapered leg hem",
        [0.0, 0.0],
        vec![
            edge("top", vec![Segment::line([260.0, 0.0])]),
            edge("outseam", vec![Segment::line([200.0, 500.0])]),
            kind(
                edge("hem", vec![Segment::line([40.0, 500.0])]),
                EdgeKind::Hem,
            ),
            edge("inseam", vec![Segment::line([0.0, 0.0])]),
        ],
        15.0,
        Some(40.0),
    );

    let half = 5f64.to_radians();
    let point = piece(
        "Sharp point",
        [0.0, 0.0],
        vec![
            edge(
                "a",
                vec![Segment::line([300.0 * half.cos(), -300.0 * half.sin()])],
            ),
            edge(
                "b",
                vec![Segment::line([300.0 * half.cos(), 300.0 * half.sin()])],
            ),
            edge("c", vec![Segment::line([0.0, 0.0])]),
        ],
        10.0,
        None,
    );

    let squared = piece(
        "Squared corner",
        [0.0, 0.0],
        vec![
            edge("a", vec![Segment::line([200.0, 0.0])]),
            corner(
                edge("b", vec![Segment::line([0.0, 115.47])]),
                CornerStyle::SquareThis,
            ),
            edge("c", vec![Segment::line([0.0, 0.0])]),
        ],
        10.0,
        None,
    );

    let circle = piece(
        "Circle",
        [100.0, 0.0],
        vec![edge(
            "round",
            vec![
                Segment::cubic([100.0, K * 100.0], [K * 100.0, 100.0], [0.0, 100.0]),
                Segment::cubic([-K * 100.0, 100.0], [-100.0, K * 100.0], [-100.0, 0.0]),
                Segment::cubic([-100.0, -K * 100.0], [-K * 100.0, -100.0], [0.0, -100.0]),
                Segment::cubic([K * 100.0, -100.0], [100.0, -K * 100.0], [100.0, 0.0]),
            ],
        )],
        15.0,
        None,
    );

    let k = K * 8.0;
    let notch = piece(
        "Tight inward curve",
        [0.0, 0.0],
        vec![
            edge("left", vec![Segment::line([92.0, 0.0])]),
            edge(
                "dip",
                vec![
                    Segment::cubic([92.0, k], [100.0 - k, 8.0], [100.0, 8.0]),
                    Segment::cubic([100.0 + k, 8.0], [108.0, k], [108.0, 0.0]),
                ],
            ),
            edge("right", vec![Segment::line([200.0, 0.0])]),
            edge(
                "rest",
                vec![
                    Segment::line([200.0, 100.0]),
                    Segment::line([0.0, 100.0]),
                    Segment::line([0.0, 0.0]),
                ],
            ),
        ],
        15.0,
        None,
    );

    let mut included = bodice();
    included.name = "Bodice, allowance included".into();
    included.allowance.mode = AllowanceMode::Included;

    vec![
        (
            "rectangle",
            "Even 10 mm allowance, pointed corners.",
            rect("Rectangle"),
        ),
        (
            "mixed-widths",
            "25, 5, 0 and 10 mm on the four sides.",
            mixed,
        ),
        (
            "bodice",
            "Curved neckline and armhole, 15 mm seams, 30 mm folded-back waist hem, centre front on the fold.",
            bodice(),
        ),
        (
            "flared-skirt",
            "40 mm hem folded back to follow the flared side seam; centre front on the fold.",
            skirt,
        ),
        (
            "wide-flare",
            "20° flare with a 50 mm hem: the folded-back end is narrower than the hem, so the corner is trimmed.",
            wide,
        ),
        (
            "tapered-hem",
            "Trouser leg narrowing to the hem: the hem allowance flares out to match the leg above.",
            tapered,
        ),
        (
            "sharp-point",
            "10° point: the pointed corner is cut square 40 mm (4 × 10 mm) out.",
            point,
        ),
        (
            "squared",
            "Acute corner with a squared end on one edge.",
            squared,
        ),
        ("circle", "Allowance on a circle is a circle.", circle),
        (
            "tight-curve",
            "8 mm curve with 15 mm allowance: the cutting line bridges it without looping.",
            notch,
        ),
        (
            "included",
            "Allowance included: the stitching line is derived inside the cutting line.",
            included,
        ),
    ]
}

fn path_d(pts: &[Point]) -> String {
    let mut d = String::new();
    for (i, p) in pts.iter().enumerate() {
        let _ = write!(d, "{}{:.2} {:.2}", if i == 0 { "M" } else { "L" }, p.x, p.y);
    }
    d.push('Z');
    d
}

fn outline_d(piece: &Piece) -> String {
    let mut d = format!("M{:.2} {:.2}", piece.outline.start.x, piece.outline.start.y);
    for e in &piece.outline.edges {
        for s in &e.segments {
            match s {
                Segment::Line(l) => {
                    let _ = write!(d, "L{:.2} {:.2}", l.to.x, l.to.y);
                }
                Segment::Cubic(c) => {
                    let _ = write!(
                        d,
                        "C{:.2} {:.2} {:.2} {:.2} {:.2} {:.2}",
                        c.c1.x, c.c1.y, c.c2.x, c.c2.y, c.to.x, c.to.y
                    );
                }
            }
        }
    }
    d.push('Z');
    d
}

fn polygon_area(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n)
        .map(|i| pts[i].to_vec2().cross(pts[(i + 1) % n].to_vec2()))
        .sum::<f64>()
        / 2.0
}

fn perimeter(pts: &[Point]) -> f64 {
    let n = pts.len();
    (0..n).map(|i| pts[i].distance(pts[(i + 1) % n])).sum()
}

#[test]
fn gallery() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/gallery");
    std::fs::create_dir_all(&dir).unwrap();
    let mut summary = String::new();
    let mut index = String::from(
        "<!doctype html><meta charset=utf-8><title>Seam allowance gallery</title>\
         <style>body{font:15px system-ui;margin:24px;background:#efe9df;color:#1f2a2e}\
         figure{display:inline-block;margin:0 16px 24px 0;vertical-align:top;width:340px}\
         img{width:340px;height:300px;background:#fff;border:1px solid #d8cfc1;border-radius:8px}\
         figcaption{margin-top:6px}</style>\
         <h1>Seam allowance gallery</h1>\
         <p>Dashed: stitching line (or derived stitching line). Solid: cutting line. Generated by \
         <code>cargo test -p soom-pattern-core --test gallery</code>.</p>\n",
    );
    for (slug, caption, piece) in cases() {
        let t = Instant::now();
        let lines = allowance_lines(&piece);
        let ms = t.elapsed().as_secs_f64() * 1e3;
        println!(
            "{slug}: {} cutting points in {ms:.2} ms",
            lines.cutting.len()
        );

        let (min, max) = lines.cutting.iter().fold(
            (
                Point::new(f64::MAX, f64::MAX),
                Point::new(f64::MIN, f64::MIN),
            ),
            |(lo, hi), p| {
                (
                    Point::new(lo.x.min(p.x), lo.y.min(p.y)),
                    Point::new(hi.x.max(p.x), hi.y.max(p.y)),
                )
            },
        );
        let pad = 10.0;
        let (w, h) = (max.x - min.x + 2.0 * pad, max.y - min.y + 2.0 * pad);
        let stitching = if piece.allowance.mode == AllowanceMode::Add {
            format!(
                r##"<path d="{}" fill="none" stroke="#b4532a" stroke-width="{sw}" stroke-dasharray="{da} {db}"/>"##,
                outline_d(&piece),
                sw = w / 400.0,
                da = w / 80.0,
                db = w / 120.0
            )
        } else {
            format!(
                r##"<path d="{}" fill="none" stroke="#b4532a" stroke-width="{sw}" stroke-dasharray="{da} {db}"/>"##,
                path_d(&lines.stitching),
                sw = w / 400.0,
                da = w / 80.0,
                db = w / 120.0
            )
        };
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{:.2} {:.2} {:.2} {:.2}">
<path d="{}" fill="#fbf7f0" stroke="#1f2a2e" stroke-width="{:.3}" stroke-linejoin="round"/>
{stitching}
</svg>
"##,
            min.x - pad,
            min.y - pad,
            w,
            h,
            path_d(&lines.cutting),
            w / 300.0,
        );
        std::fs::write(dir.join(format!("{slug}.svg")), svg).unwrap();
        let _ = writeln!(
            index,
            "<figure><img src=\"{slug}.svg\" alt=\"\"><figcaption><b>{}</b><br>{caption}</figcaption></figure>",
            piece.name
        );
        let _ = writeln!(
            summary,
            "{slug}: area {:.1} mm², perimeter {:.2} mm, bounds ({:.2}, {:.2})–({:.2}, {:.2}), notes {:?}",
            polygon_area(&lines.cutting),
            perimeter(&lines.cutting),
            min.x,
            min.y,
            max.x,
            max.y,
            lines.notes
        );
    }
    std::fs::write(dir.join("index.html"), index).unwrap();
    insta::assert_snapshot!(summary);
}
