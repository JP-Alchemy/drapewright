//! Pictures of a garment worn on the avatar, for an AI assistant to look
//! at: the figure and the cloth on it from the front, the side and the
//! back, side by side, lit from the front.

use schemars::JsonSchema;
use serde::Deserialize;
use tiny_skia::Pixmap;

use crate::render::{Picture, label};

/// Blank border around the figures, and between them, mm.
const BORDER: f32 = 60.0;
/// Room under each figure for its name, pixels.
const CAPTION: f32 = 30.0;
/// Drawn this many times larger, then shrunk: smooth edges.
const OVER: u32 = 2;
const BACKGROUND: [u8; 3] = [250, 248, 244];
/// The figure's skin (sRGB, 0 to 1).
const SKIN: [f32; 3] = [0.80, 0.78, 0.75];
/// Light from in front, above and to the left (towards the light).
const LIGHT: [f32; 3] = [-0.3, 0.45, 0.84];
/// How dark the cloth's inner side looks, seen through an opening.
const INNER: f32 = 0.75;

/// Which way the figure is seen from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum View {
    Front,
    /// From the figure's right, facing right.
    Side,
    Back,
}

impl View {
    pub const ALL: [View; 3] = [View::Front, View::Side, View::Back];

    pub fn name(self) -> &'static str {
        match self {
            View::Front => "Front",
            View::Side => "Side",
            View::Back => "Back",
        }
    }

    /// A point (mm) as seen: across to the right, up, and towards the eye.
    fn see(self, [x, y, z]: [f32; 3]) -> [f32; 3] {
        match self {
            View::Front => [x, y, z],
            View::Side => [z, y, -x],
            View::Back => [-x, y, -z],
        }
    }
}

/// What to draw: the figure's skin and the cloth (mm), each a list of
/// points and triangles, and each of the cloth's points' colour (sRGB, 0
/// to 1).
#[derive(Clone, Copy, Debug)]
pub struct Scene<'a> {
    pub skin: (&'a [[f32; 3]], &'a [[u32; 3]]),
    pub cloth: (&'a [[f32; 3]], &'a [[u32; 3]]),
    pub colours: &'a [[f32; 3]],
}

/// Cloth is drawn over skin up to this far behind it, mm: cloth lying on
/// the body, its flat triangles cutting a little into the body's curves.
const ON_SKIN: f32 = 3.0;

fn cross(a: [f32; 3], b: [f32; 3], c: [f32; 3]) -> [f32; 3] {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ]
}

fn unit(n: [f32; 3]) -> [f32; 3] {
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-12);
    n.map(|c| c / l)
}

/// Each point's normal: its triangles', by their area.
fn normals(points: &[[f32; 3]], triangles: &[[u32; 3]]) -> Vec<[f32; 3]> {
    let mut sum = vec![[0.0f32; 3]; points.len()];
    for t in triangles {
        let n = cross(
            points[t[0] as usize],
            points[t[1] as usize],
            points[t[2] as usize],
        );
        for &i in t {
            for k in 0..3 {
                sum[i as usize][k] += n[k];
            }
        }
    }
    sum.into_iter().map(unit).collect()
}

/// `colour` lit where the surface faces `n` (a unit vector, as seen).
fn lit(colour: [f32; 3], n: [f32; 3]) -> [f32; 3] {
    let l = unit(LIGHT);
    let towards = n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
    let light = 0.4 + 0.6 * towards.max(0.0);
    colour.map(|c| c * light)
}

/// Pixels with how near what is drawn in each is.
struct Canvas {
    width: usize,
    height: usize,
    colour: Vec<[f32; 3]>,
    near: Vec<f32>,
}

impl Canvas {
    fn new(width: usize, height: usize, background: [f32; 3]) -> Self {
        Canvas {
            width,
            height,
            colour: vec![background; width * height],
            near: vec![f32::MIN; width * height],
        }
    }

    /// Fills a triangle, its corners (x, y in pixels, and how near) each
    /// with a colour, where nothing nearer is drawn.
    fn triangle(&mut self, corners: [[f32; 3]; 3], colours: [[f32; 3]; 3]) {
        let [a, b, c] = corners;
        let edge = |p: [f32; 3], q: [f32; 3], x: f32, y: f32| {
            (q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0])
        };
        let area = edge(a, b, c[0], c[1]);
        if area.abs() < 1e-6 {
            return;
        }
        let clamp = |v: f32, most: usize| (v.max(0.0) as usize).min(most);
        let (x0, x1) = (
            clamp(a[0].min(b[0]).min(c[0]).floor(), self.width),
            clamp(a[0].max(b[0]).max(c[0]).ceil(), self.width),
        );
        let (y0, y1) = (
            clamp(a[1].min(b[1]).min(c[1]).floor(), self.height),
            clamp(a[1].max(b[1]).max(c[1]).ceil(), self.height),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w = [
                    edge(b, c, px, py) / area,
                    edge(c, a, px, py) / area,
                    edge(a, b, px, py) / area,
                ];
                if w.iter().any(|&w| w < 0.0) {
                    continue;
                }
                let near = w[0] * a[2] + w[1] * b[2] + w[2] * c[2];
                let at = y * self.width + x;
                if near > self.near[at] {
                    self.near[at] = near;
                    self.colour[at] = [0, 1, 2].map(|k| {
                        w[0] * colours[0][k] + w[1] * colours[1][k] + w[2] * colours[2][k]
                    });
                }
            }
        }
    }

    fn pixmap(&self) -> Option<Pixmap> {
        let mut pixmap = Pixmap::new(self.width as u32, self.height as u32)?;
        for (out, c) in pixmap.data_mut().chunks_exact_mut(4).zip(&self.colour) {
            for k in 0..3 {
                out[k] = (c[k] * 255.0).round().clamp(0.0, 255.0) as u8;
            }
            out[3] = 255;
        }
        Some(pixmap)
    }
}

/// Draws `scene` seen from each of `views` in turn, left to right, fitted to
/// at most `max_px` pixels on the longer side.
pub fn draw(scene: &Scene, views: &[View], max_px: u32) -> Result<Picture, String> {
    let (skin, skin_triangles) = scene.skin;
    let (cloth, cloth_triangles) = scene.cloth;
    if views.is_empty() || cloth.is_empty() {
        return Err("There is nothing to draw.".into());
    }
    let all = || skin.iter().chain(cloth);
    let (low, high) = all().fold((f32::MAX, f32::MIN), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    // Each view's width, mm: the figure from side to side as seen.
    let spans: Vec<(f32, f32)> = views
        .iter()
        .map(|&v| {
            all().fold((f32::MAX, f32::MIN), |(l, h), &p| {
                let x = v.see(p)[0];
                (l.min(x), h.max(x))
            })
        })
        .collect();
    let wide = spans.iter().map(|(l, h)| h - l).sum::<f32>() + BORDER * (views.len() + 1) as f32;
    let tall = high - low + 2.0 * BORDER;
    let longest = max_px.clamp(200, 4000) as f32;
    let scale = (longest / wide).min((longest - CAPTION) / tall);
    let (width, height) = (
        (wide * scale).ceil() as u32,
        (tall * scale + CAPTION).ceil() as u32,
    );
    if u64::from(width) * u64::from(height) > 4_000_000 {
        return Err("The picture would be too large.".into());
    }
    let over = scale * OVER as f32;
    let mut canvas = Canvas::new(
        (width * OVER) as usize,
        (height * OVER) as usize,
        BACKGROUND.map(|c| f32::from(c) / 255.0),
    );
    let skin_normals = normals(skin, skin_triangles);
    let cloth_normals = normals(cloth, cloth_triangles);

    let mut left = BORDER;
    let mut middles = Vec::new();
    for (&view, &(from, to)) in views.iter().zip(&spans) {
        let px = |p: [f32; 3], nearer: f32| {
            let [x, y, z] = view.see(p);
            [
                (left + x - from) * over,
                (high + BORDER - y) * over,
                z + nearer,
            ]
        };
        for t in skin_triangles {
            let [a, b, c] = t.map(|i| view.see(skin[i as usize]));
            // The skin facing away is hidden by the skin in front.
            if cross(a, b, c)[2] <= 0.0 {
                continue;
            }
            let colours = t.map(|i| lit(SKIN, view.see(skin_normals[i as usize])));
            canvas.triangle(t.map(|i| px(skin[i as usize], 0.0)), colours);
        }
        for t in cloth_triangles {
            let [a, b, c] = t.map(|i| view.see(cloth[i as usize]));
            // Its inner side, seen through an opening: darker, lit the
            // other way.
            let (side, shade) = if cross(a, b, c)[2] >= 0.0 {
                (1.0, 1.0)
            } else {
                (-1.0, INNER)
            };
            let colours = t.map(|i| {
                let n = view.see(cloth_normals[i as usize]).map(|c| c * side);
                lit(scene.colours[i as usize].map(|c| c * shade), n)
            });
            canvas.triangle(t.map(|i| px(cloth[i as usize], ON_SKIN)), colours);
        }
        middles.push((left + (to - from) / 2.0) * scale);
        left += to - from + BORDER;
    }

    let big = canvas.pixmap().ok_or("The picture would be too large.")?;
    let mut pixmap = shrunk(&big, OVER).ok_or("The picture would be too large.")?;
    for (&view, x) in views.iter().zip(middles) {
        let y = height as f32 - CAPTION * 0.75;
        label(&mut pixmap, view.name(), x, y, 14.0, (33, 33, 33));
    }
    let png = pixmap.encode_png().map_err(|e| e.to_string())?;
    Ok(Picture {
        png,
        width,
        height,
        mm_per_px: 1.0 / f64::from(scale),
    })
}

/// `big` made `by` times smaller, each pixel the average of those it
/// covered.
fn shrunk(big: &Pixmap, by: u32) -> Option<Pixmap> {
    let (w, h) = (big.width() / by, big.height() / by);
    let mut small = Pixmap::new(w, h)?;
    let from = big.data();
    let row = big.width() as usize * 4;
    let n = by * by;
    for (i, out) in small.data_mut().chunks_exact_mut(4).enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let mut sum = [0u32; 4];
        for dy in 0..by {
            for dx in 0..by {
                let at = ((y * by + dy) as usize) * row + ((x * by + dx) as usize) * 4;
                for (s, v) in sum.iter_mut().zip(&from[at..at + 4]) {
                    *s += u32::from(*v);
                }
            }
        }
        for (o, s) in out.iter_mut().zip(sum) {
            *o = ((s + n / 2) / n) as u8;
        }
    }
    Some(small)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A flat square of cloth facing the front, 200 mm across, in front of
    /// a square of skin facing the front too.
    fn squares() -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
        let square = |z: f32, size: f32| {
            vec![
                [-size, 1000.0 - size, z],
                [size, 1000.0 - size, z],
                [size, 1000.0 + size, z],
                [-size, 1000.0 + size, z],
            ]
        };
        (square(0.0, 300.0), square(50.0, 100.0))
    }

    const TRIANGLES: [[u32; 3]; 2] = [[0, 1, 2], [0, 2, 3]];

    fn pixel(picture: &Picture, x: u32, y: u32) -> [u8; 3] {
        let pixmap = Pixmap::decode_png(&picture.png).unwrap();
        let p = pixmap.pixel(x, y).unwrap();
        [p.red(), p.green(), p.blue()]
    }

    #[test]
    fn the_cloth_hides_the_skin_behind_it_and_shows_its_inner_side_from_behind() {
        let (skin, cloth) = squares();
        let red = vec![[1.0, 0.0, 0.0]; 4];
        let scene = Scene {
            skin: (&skin, &TRIANGLES),
            cloth: (&cloth, &TRIANGLES),
            colours: &red,
        };
        let front = draw(&scene, &[View::Front], 400).unwrap();
        let (cx, cy) = (front.width / 2, (front.height - CAPTION as u32) / 2);
        let [r, g, b] = pixel(&front, cx, cy);
        assert!(r > 100 && g == 0 && b == 0, "{:?}", [r, g, b]);
        // Beside the cloth, the skin.
        let skin_px = (250.0 / front.mm_per_px) as u32;
        let [r, g, b] = pixel(&front, cx - skin_px, cy);
        assert!(
            r > 100 && g > 100 && b > 100 && r.abs_diff(b) < 30,
            "{:?}",
            [r, g, b]
        );
        // From behind, the skin faces away and is unseen; the cloth shows
        // its inner side, darker.
        let back = draw(&scene, &[View::Back], 400).unwrap();
        let [r, g, b] = pixel(&back, back.width / 2, (back.height - CAPTION as u32) / 2);
        assert!(
            g < 10 && b < 10 && r > 50,
            "the cloth's inner side: {:?}",
            [r, g, b]
        );
    }

    #[test]
    fn views_sit_side_by_side_within_the_size_asked_for() {
        let (skin, cloth) = squares();
        let grey = vec![[0.5; 3]; 4];
        let scene = Scene {
            skin: (&skin, &TRIANGLES),
            cloth: (&cloth, &TRIANGLES),
            colours: &grey,
        };
        let one = draw(&scene, &[View::Front], 600).unwrap();
        let three = draw(&scene, &View::ALL, 600).unwrap();
        assert!(one.width.max(one.height) <= 600);
        assert!(three.width.max(three.height) <= 600);
        assert!(three.width > one.width, "{} {}", three.width, one.width);
        assert!(draw(&scene, &[], 600).is_err());
    }
}
