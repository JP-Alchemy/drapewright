//! The 3D view: drawn by `drapewright-render` on a canvas the page hands to
//! the sim worker, through WebGPU where there is one and WebGL2 where not.

use drapewright_body::Shape;
use drapewright_render::{Look, Mesh, Orbit, Renderer};
use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::drape::ClothState;
use crate::{Avatar, AvatarSettings, ClothShows, Draping};

fn error(e: impl std::fmt::Display) -> JsError {
    JsError::new(&e.to_string())
}

/// Turning, radians for each CSS pixel dragged.
const TURN: f32 = 0.008;

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
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("3D view"),
                required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                    .using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(error)?;
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

    /// Shows `avatar` shaped by `settings` (an `AvatarSettings`), framed
    /// whole the first time.
    pub fn show(&mut self, avatar: &Avatar, settings: JsValue) -> Result<(), JsError> {
        let settings: AvatarSettings = serde_wasm_bindgen::from_value(settings).map_err(error)?;
        let model = &avatar.model;
        let body = model.shape(&Shape::from(settings));
        let positions: Vec<[f32; 3]> = body
            .vertices
            .iter()
            .map(|p| [p[0] as f32, p[1] as f32, p[2] as f32])
            .collect();
        let mesh = Mesh::new(positions, &model.skin());
        let (low, high) = mesh.bounds();
        self.renderer.set_mesh(&self.device, &mesh);
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

    /// Shows the cloth of a drape as it is now.
    #[wasm_bindgen(js_name = showCloth)]
    pub fn show_cloth(&mut self, draping: &Draping) {
        self.cloth = Some(draping.0.state());
        self.colour_cloth();
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

    /// Turns round the figure for a drag of `dx`, `dy` CSS pixels.
    pub fn turn(&mut self, dx: f32, dy: f32) {
        if let Some(orbit) = &mut self.orbit {
            orbit.turn(-dx * TURN, dy * TURN);
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
