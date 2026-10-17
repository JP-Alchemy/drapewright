//! The 3D view: drawn by `drapewright-render` on a canvas the page hands to
//! the sim worker, through WebGPU where there is one and WebGL2 where not.

use drapewright_body::{Pose, Rig, Shape};
use drapewright_render::{Look, Mesh, Orbit, Renderer};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::drape::ClothState;
use crate::{Avatar, AvatarPose, AvatarSettings, ClothShows, Draping};

fn error(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Turning, radians for each CSS pixel dragged.
const TURN: f32 = 0.008;

/// A figure: its points (mm), and its skin's triangles.
type Figure = (Vec<[f32; 3]>, Vec<[u32; 3]>);

/// A seat's colour (sRGB, 0 to 1): a plain grey.
const SEAT: [f32; 3] = [0.6, 0.6, 0.6];

/// A box (its lowest and highest corners), its faces flat: four corners
/// each, two triangles facing out.
fn box_mesh((low, high): ([f32; 3], [f32; 3])) -> Mesh {
    let corner = |i: usize| {
        [
            if i & 1 == 0 { low[0] } else { high[0] },
            if i & 2 == 0 { low[1] } else { high[1] },
            if i & 4 == 0 { low[2] } else { high[2] },
        ]
    };
    // Each face's corners, anticlockwise seen from outside.
    let faces: [[usize; 4]; 6] = [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ];
    let mut positions = Vec::new();
    let mut triangles = Vec::new();
    for face in faces {
        let first = positions.len() as u32;
        positions.extend(face.map(corner));
        triangles.push([first, first + 1, first + 2]);
        triangles.push([first, first + 2, first + 3]);
    }
    Mesh::new(positions, &triangles)
}

/// The page's colours for the view (sRGB, 0 to 1).
#[derive(Deserialize)]
struct Colours {
    background: [f32; 3],
    figure: [f32; 3],
    cloth: [f32; 3],
}

/// The 3D view, drawing on a canvas.
#[wasm_bindgen]
#[derive(Debug)]
pub struct View {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    /// The format drawn in (sRGB, perhaps viewing a plain surface).
    format: wgpu::TextureFormat,
    renderer: Renderer,
    orbit: Option<Orbit>,
    bounds: Option<([f32; 3], [f32; 3])>,
    look: Look,
    /// The fabric's colour on the page (sRGB, 0 to 1).
    fabric: [f32; 3],
    /// The cloth shown, as last seen, and what it is shown as.
    cloth: Option<ClothState>,
    shows: ClothShows,
    backend: String,
    /// The figure standing as measured, and which move of it is shown
    /// instead: a drape's (by number), 0 for a pose of its own, none for
    /// standing.
    standing: Option<Figure>,
    moved: Option<u32>,
    /// The seat shown, if the figure sits.
    seat: Option<([f32; 3], [f32; 3])>,
}

#[wasm_bindgen]
impl View {
    /// Starts drawing on `canvas`, handed over by the page at its size in
    /// device pixels.
    pub async fn create(canvas: web_sys::OffscreenCanvas) -> Result<View, JsError> {
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let mut instance_desc = wgpu::InstanceDescriptor::new_without_display_handle();
        instance_desc.backends = wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL;
        let instance = wgpu::util::new_instance_with_webgpu_detection(instance_desc).await;
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas))
            .map_err(error)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(error)?;
        let backend = match adapter.get_info().backend {
            wgpu::Backend::BrowserWebGpu => "WebGPU",
            _ => "WebGL2",
        };
        // With WebGPU, enough to compute with: the drape is moved on on the
        // graphics card too.
        let gpu = backend == "WebGPU";
        let limits = if gpu {
            wgpu::Limits::default()
        } else {
            wgpu::Limits::downlevel_webgl2_defaults()
        };
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("3D view"),
                required_limits: limits.using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(error)?;
        crate::drape::use_card(gpu.then(|| (device.clone(), queue.clone())));
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| JsError::new("the canvas cannot be drawn on"))?;
        // Drawn in sRGB, through a view of the surface if it is plain.
        let format = config.format.add_srgb_suffix();
        if format != config.format {
            config.view_formats.push(format);
        }
        config.alpha_mode = wgpu::CompositeAlphaMode::Opaque;
        surface.configure(&device, &config);
        let samples = if adapter
            .get_texture_format_features(format)
            .flags
            .sample_count_supported(4)
        {
            4
        } else {
            1
        };
        let renderer = Renderer::new(&device, format, samples);
        Ok(View {
            surface,
            device,
            queue,
            config,
            format,
            renderer,
            orbit: None,
            bounds: None,
            look: Look::default(),
            fabric: [0.36, 0.47, 0.59],
            cloth: None,
            shows: ClothShows::default(),
            backend: backend.into(),
            standing: None,
            moved: None,
            seat: None,
        })
    }

    /// How the view is drawn: "WebGPU" or "WebGL2".
    #[wasm_bindgen(getter)]
    pub fn backend(&self) -> String {
        self.backend.clone()
    }

    fn aspect(&self) -> f32 {
        self.config.width as f32 / self.config.height.max(1) as f32
    }

    /// The canvas is now `width` by `height` device pixels.
    pub fn resize(&mut self, width: u32, height: u32) {
        let (width, height) = (width.max(1), height.max(1));
        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    /// The page's colours: `{ background, figure, cloth }`, each
    /// `[r, g, b]` from 0 to 1.
    #[wasm_bindgen(js_name = setLook)]
    pub fn set_look(&mut self, colours: JsValue) -> Result<(), JsError> {
        let c: Colours = serde_wasm_bindgen::from_value(colours).map_err(error)?;
        self.look = Look {
            background: c.background,
            figure: c.figure,
        };
        self.fabric = c.cloth;
        self.colour_cloth();
        Ok(())
    }

    /// Shows `avatar` shaped by `settings` (an `AvatarSettings`) in `pose`
    /// (an `AvatarPose`), framed whole the first time.
    pub fn show(
        &mut self,
        avatar: &Avatar,
        settings: JsValue,
        pose: JsValue,
    ) -> Result<(), JsError> {
        let settings: AvatarSettings = serde_wasm_bindgen::from_value(settings).map_err(error)?;
        let pose: AvatarPose = if pose.is_undefined() {
            AvatarPose::Standing
        } else {
            serde_wasm_bindgen::from_value(pose).map_err(error)?
        };
        let model = &avatar.model;
        let body = model.shape(&Shape::from(settings));
        let to_f32 = |vertices: &[[f64; 3]]| -> Vec<[f32; 3]> {
            vertices
                .iter()
                .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
                .collect()
        };
        let skin = model.skin();
        let standing = to_f32(&body.vertices);
        let mut seat = None;
        let posed = match Rig::new(model, &body) {
            Some(rig) if pose != AvatarPose::Standing => {
                let stance = rig.stance(&body, Pose::from(pose));
                if pose == AvatarPose::Sitting {
                    let (low, high) = rig.seat(&body, &stance);
                    seat = Some((low.map(|c| c as f32), high.map(|c| c as f32)));
                }
                Some(to_f32(&rig.place(&body, &stance).vertices))
            }
            _ => None,
        };
        self.show_seat(seat);
        // Framed by the figure standing, however it's posed.
        let (low, high) = Mesh::new(standing.clone(), &skin).bounds();
        self.moved = posed.as_ref().map(|_| 0);
        let mesh = Mesh::new(posed.unwrap_or_else(|| standing.clone()), &skin);
        self.renderer.set_mesh(&self.device, &mesh);
        self.standing = Some((standing, skin));
        match &mut self.orbit {
            // A new figure for the same view: keep where it is seen from.
            Some(orbit) => {
                orbit.target[1] = (low[1] + high[1]) / 2.0;
            }
            None => self.orbit = Some(Orbit::framing(low, high, self.aspect())),
        }
        self.bounds = Some((low, high));
        Ok(())
    }

    /// Shows the cloth of a drape as it is now, and its body as it has
    /// moved.
    #[wasm_bindgen(js_name = showCloth)]
    pub fn show_cloth(&mut self, draping: &Draping) {
        self.cloth = Some(draping.0.state());
        self.colour_cloth();
        self.show_seat(draping.0.seat());
        let Some((standing, skin)) = &self.standing else {
            return;
        };
        match draping.0.body() {
            Some((n, points)) if self.moved != Some(n) => {
                let mesh = Mesh::new(points.to_vec(), skin);
                if !self.renderer.move_figure(&self.queue, &mesh) {
                    self.renderer.set_mesh(&self.device, &mesh);
                }
                self.moved = Some(n);
            }
            None if self.moved.is_some() => {
                let mesh = Mesh::new(standing.clone(), skin);
                self.renderer.move_figure(&self.queue, &mesh);
                self.moved = None;
            }
            _ => {}
        }
    }

    /// Shows `seat`, if not shown already.
    fn show_seat(&mut self, seat: Option<([f32; 3], [f32; 3])>) {
        if seat != self.seat {
            let mesh = seat.map(box_mesh);
            self.renderer
                .set_prop(&self.device, mesh.as_ref().map(|m| (m, SEAT)));
            self.seat = seat;
        }
    }

    /// The cloth, shown as it is to be shown.
    fn colour_cloth(&mut self) {
        if let Some(cloth) = &self.cloth {
            let mesh = Mesh::new(cloth.points.clone(), &cloth.triangles);
            let colours = cloth.colours(self.shows, self.fabric);
            self.renderer
                .set_cloth(&self.device, &self.queue, &mesh, &colours);
        }
    }

    /// Takes the cloth off.
    #[wasm_bindgen(js_name = hideCloth)]
    pub fn hide_cloth(&mut self) {
        self.cloth = None;
        self.renderer.clear_cloth();
    }

    /// Shows the cloth as a `ClothShows`: its fabric, or a heatmap.
    #[wasm_bindgen(js_name = setShows)]
    pub fn set_shows(&mut self, shows: JsValue) -> Result<(), JsError> {
        self.shows = serde_wasm_bindgen::from_value(shows).map_err(error)?;
        self.colour_cloth();
        Ok(())
    }

    /// The cloth seen at `x`, `y` CSS pixels from the view's top left
    /// (each `pixel_ratio` device pixels): a `ClothReading`, or null where
    /// there is none.
    pub fn pick(&self, x: f32, y: f32, pixel_ratio: f32) -> Result<JsValue, JsError> {
        let (Some(cloth), Some(orbit)) = (&self.cloth, &self.orbit) else {
            return Ok(JsValue::NULL);
        };
        let (width, height) = (self.config.width as f32, self.config.height as f32);
        let (eye, way) = orbit.ray(x * pixel_ratio, y * pixel_ratio, width, height);
        match cloth.pick(eye, way) {
            Some(reading) => reading
                .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
                .map_err(error),
            None => Ok(JsValue::NULL),
        }
    }

    /// The pieces' edges on the cloth as seen now (`SeenEdge`s), at
    /// `pixel_ratio` device pixels to a CSS pixel; null with no cloth on.
    pub fn outline(&self, pixel_ratio: f32) -> Result<JsValue, JsError> {
        let (Some(cloth), Some(orbit)) = (&self.cloth, &self.orbit) else {
            return Ok(JsValue::NULL);
        };
        let (width, height) = (self.config.width as f32, self.config.height as f32);
        let m = orbit.view_projection(self.aspect());
        let ratio = pixel_ratio.max(0.1);
        let project = |p: [f32; 3]| {
            let v = [p[0], p[1], p[2], 1.0];
            let c = [0, 1, 2, 3].map(|r| (0..4).map(|k| m[k][r] * v[k]).sum::<f32>());
            (c[3] > 1e-6).then(|| {
                [
                    (c[0] / c[3] + 1.0) / 2.0 * width / ratio,
                    (1.0 - c[1] / c[3]) / 2.0 * height / ratio,
                ]
            })
        };
        cloth
            .seen(orbit.eye(), project)
            .serialize(&serde_wasm_bindgen::Serializer::json_compatible())
            .map_err(error)
    }

    /// Turns round the figure for a drag of `dx`, `dy` CSS pixels.
    pub fn turn(&mut self, dx: f32, dy: f32) {
        if let Some(orbit) = &mut self.orbit {
            orbit.turn(-dx * TURN, dy * TURN);
        }
    }

    /// Turns round the figure by `radians`, as on a turntable.
    pub fn spin(&mut self, radians: f32) {
        if let Some(orbit) = &mut self.orbit {
            orbit.turn(radians, 0.0);
        }
    }

    /// Comes `factor` times nearer.
    pub fn zoom(&mut self, factor: f32) {
        if let Some(orbit) = &mut self.orbit {
            orbit.zoom(factor);
        }
    }

    /// Slides the view for a drag of `dx`, `dy` CSS pixels, each
    /// `pixel_ratio` device pixels.
    pub fn slide(&mut self, dx: f32, dy: f32, pixel_ratio: f32) {
        let height = self.config.height as f32;
        if let Some(orbit) = &mut self.orbit {
            let mm = orbit.mm_per_pixel(height) * pixel_ratio;
            orbit.slide(-dx * mm, dy * mm);
        }
    }

    /// Back to the figure seen whole from the front.
    pub fn reset(&mut self) {
        if let Some((low, high)) = self.bounds {
            self.orbit = Some(Orbit::framing(low, high, self.aspect()));
        }
    }

    /// Draws a frame.
    pub fn draw(&mut self) -> Result<(), JsError> {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            _ => return Ok(()),
        };
        let target = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.format),
            ..Default::default()
        });
        let orbit = self.orbit.unwrap_or_else(|| {
            Orbit::framing([-300.0, 0.0, -200.0], [300.0, 1700.0, 200.0], self.aspect())
        });
        self.renderer.draw(
            &self.device,
            &self.queue,
            &target,
            (self.config.width, self.config.height),
            &orbit,
            &self.look,
        );
        self.queue.present(frame);
        Ok(())
    }
}
