//! The drawing itself: a pipeline for the figure, one for the cloth draped
//! on it and one for the shadow under them, with a depth buffer and (where
//! the device allows) four samples a pixel, so edges are smooth.

use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

use crate::camera::{Matrix, Orbit};
use crate::mesh::Mesh;

/// Colours the view is drawn in, as on the page (sRGB, 0 to 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    pub background: [f32; 3],
    pub figure: [f32; 3],
}

impl Default for Look {
    fn default() -> Self {
        Look {
            background: [0.937, 0.914, 0.875],
            figure: [0.80, 0.74, 0.68],
        }
    }
}

/// A colour as the GPU blends it (linear light).
fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
}

/// The cloth's points are coloured one by one (a heatmap), in linear light.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ClothVertex {
    position: [f32; 3],
    normal: [f32; 3],
    colour: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Uniforms {
    view_projection: Matrix,
    eye: [f32; 4],
    light: [f32; 4],
    figure: [f32; 4],
    sky: [f32; 4],
    ground: [f32; 4],
    floor: [f32; 4],
}

const DEPTH: wgpu::TextureFormat = wgpu::TextureFormat::Depth24Plus;

/// The textures a frame is drawn into before it reaches the page.
#[derive(Debug)]
struct Targets {
    size: (u32, u32),
    /// Four samples a pixel, resolved into the page's texture.
    samples: Option<wgpu::TextureView>,
    depth: wgpu::TextureView,
}

/// The figure's buffers.
#[derive(Debug)]
struct Figure {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    count: u32,
    /// The shadow's radius, mm.
    reach: f32,
}

/// The cloth's buffers, written again as it moves.
#[derive(Debug)]
struct Cloth {
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    points: usize,
    count: u32,
}

fn vertices(mesh: &Mesh) -> Vec<Vertex> {
    mesh.positions
        .iter()
        .zip(&mesh.normals)
        .map(|(&position, &normal)| Vertex { position, normal })
        .collect()
}

/// Draws the 3D view on a device, into textures of one format.
#[derive(Debug)]
pub struct Renderer {
    format: wgpu::TextureFormat,
    sample_count: u32,
    figure_pipeline: wgpu::RenderPipeline,
    cloth_pipeline: wgpu::RenderPipeline,
    floor_pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    floor: wgpu::Buffer,
    figure: Option<Figure>,
    cloth: Option<Cloth>,
    /// Something the figure uses, in a colour of its own: a seat.
    prop: Option<Cloth>,
    targets: Option<Targets>,
}

impl Renderer {
    /// Ready to draw into textures of `format`, with `sample_count` samples
    /// a pixel (1 or 4).
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, sample_count: u32) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("3D view"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shader.wgsl").into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("3D view uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("3D view uniforms"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("3D view uniforms"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("3D view"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let multisample = wgpu::MultisampleState {
            count: sample_count,
            ..Default::default()
        };
        // The figure and the cloth: lit surfaces of points and normals (the
        // cloth's coloured point by point). The figure's back faces are
        // inside it; either side of the cloth may be seen.
        let figure_attributes = wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3];
        let cloth_attributes =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x3, 2 => Float32x3];
        let lit = |label: &str,
                   (vertex, fragment): (&str, &str),
                   layout: wgpu::VertexBufferLayout,
                   cull_mode: Option<wgpu::Face>| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some(vertex),
                    compilation_options: Default::default(),
                    buffers: &[Some(layout)],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: DEPTH,
                    depth_write_enabled: Some(true),
                    depth_compare: Some(wgpu::CompareFunction::Less),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample,
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: Default::default(),
                    targets: &[Some(format.into())],
                }),
                multiview_mask: None,
                cache: None,
            })
        };
        let figure_pipeline = lit(
            "figure",
            ("figure_vertex", "figure_fragment"),
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<Vertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &figure_attributes,
            },
            Some(wgpu::Face::Back),
        );
        let cloth_pipeline = lit(
            "cloth",
            ("cloth_vertex", "cloth_fragment"),
            wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<ClothVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &cloth_attributes,
            },
            None,
        );
        let floor_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("floor"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("floor_vertex"),
                compilation_options: Default::default(),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 8,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2],
                })],
            },
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DEPTH,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample,
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("floor_fragment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        let corners: [[f32; 2]; 6] = [
            [-1.0, -1.0],
            [1.0, -1.0],
            [1.0, 1.0],
            [-1.0, -1.0],
            [1.0, 1.0],
            [-1.0, 1.0],
        ];
        let floor = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("floor"),
            contents: bytemuck::cast_slice(&corners),
            usage: wgpu::BufferUsages::VERTEX,
        });
        Renderer {
            format,
            sample_count,
            figure_pipeline,
            cloth_pipeline,
            floor_pipeline,
            uniforms,
            bindings,
            floor,
            figure: None,
            cloth: None,
            prop: None,
            targets: None,
        }
    }

    /// Something the figure uses, drawn in `colour` (sRGB, 0 to 1): a seat
    /// it sits on. None, nothing.
    pub fn set_prop(&mut self, device: &wgpu::Device, prop: Option<(&Mesh, [f32; 3])>) {
        self.prop = prop.map(|(mesh, colour)| {
            let vertices: Vec<ClothVertex> = mesh
                .positions
                .iter()
                .zip(&mesh.normals)
                .map(|(&position, &normal)| ClothVertex {
                    position,
                    normal,
                    colour: colour.map(linear),
                })
                .collect();
            Cloth {
                vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("prop"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("prop"),
                    contents: bytemuck::cast_slice(&mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                points: vertices.len(),
                count: mesh.indices.len() as u32,
            }
        });
    }

    /// The figure to draw from now on.
    pub fn set_mesh(&mut self, device: &wgpu::Device, mesh: &Mesh) {
        let vertices = vertices(mesh);
        let (low, high) = mesh.bounds();
        self.figure = Some(Figure {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("figure"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("figure"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            count: mesh.indices.len() as u32,
            reach: (high[0] - low[0]).max(high[2] - low[2]) * 0.75,
        });
    }

    /// The figure moved: `mesh` the same figure (as many points, the same
    /// triangles) somewhere else, written into its buffers. False, and
    /// nothing done, if there's no such figure.
    pub fn move_figure(&mut self, queue: &wgpu::Queue, mesh: &Mesh) -> bool {
        let Some(figure) = &self.figure else {
            return false;
        };
        if figure.count != mesh.indices.len() as u32 {
            return false;
        }
        queue.write_buffer(&figure.vertices, 0, bytemuck::cast_slice(&vertices(mesh)));
        true
    }

    /// The cloth to draw from now on, draped on the figure, each point in
    /// its colour (sRGB, 0 to 1). Cloth of as many points and triangles as
    /// the last is written into its buffers: a drape moves its cloth every
    /// frame.
    pub fn set_cloth(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        mesh: &Mesh,
        colours: &[[f32; 3]],
    ) {
        let vertices: Vec<ClothVertex> = mesh
            .positions
            .iter()
            .zip(&mesh.normals)
            .zip(colours.iter().chain(std::iter::repeat(&[0.5; 3])))
            .map(|((&position, &normal), colour)| ClothVertex {
                position,
                normal,
                colour: colour.map(linear),
            })
            .collect();
        let count = mesh.indices.len() as u32;
        if let Some(cloth) = &self.cloth
            && cloth.points == vertices.len()
            && cloth.count == count
        {
            queue.write_buffer(&cloth.vertices, 0, bytemuck::cast_slice(&vertices));
            queue.write_buffer(&cloth.indices, 0, bytemuck::cast_slice(&mesh.indices));
            return;
        }
        let usage = wgpu::BufferUsages::COPY_DST;
        self.cloth = Some(Cloth {
            vertices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cloth"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX | usage,
            }),
            indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("cloth"),
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX | usage,
            }),
            points: vertices.len(),
            count,
        });
    }

    /// No cloth from now on.
    pub fn clear_cloth(&mut self) {
        self.cloth = None;
    }

    fn targets(&mut self, device: &wgpu::Device, size: (u32, u32)) -> &Targets {
        if self.targets.as_ref().is_none_or(|t| t.size != size) {
            let texture = |format, label| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width: size.0.max(1),
                            height: size.1.max(1),
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: self.sample_count,
                        dimension: wgpu::TextureDimension::D2,
                        format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            self.targets = Some(Targets {
                size,
                samples: (self.sample_count > 1).then(|| texture(self.format, "samples")),
                depth: texture(DEPTH, "depth"),
            });
        }
        self.targets.as_ref().expect("made above")
    }

    /// Draws a frame into `target` (`size` pixels), seen from `orbit`.
    pub fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        size: (u32, u32),
        orbit: &Orbit,
        look: &Look,
    ) {
        let aspect = size.0.max(1) as f32 / size.1.max(1) as f32;
        let colour = |c: [f32; 3]| [linear(c[0]), linear(c[1]), linear(c[2]), 1.0];
        let figure = colour(look.figure);
        let uniforms = Uniforms {
            view_projection: orbit.view_projection(aspect),
            eye: {
                let e = orbit.eye();
                [e[0], e[1], e[2], 1.0]
            },
            // From above, in front and a little to the figure's right.
            light: [-0.35, 0.8, 0.5, 0.0],
            figure,
            sky: [0.42, 0.42, 0.44, 1.0],
            ground: [0.16, 0.15, 0.15, 1.0],
            floor: [
                self.figure.as_ref().map_or(400.0, |f| f.reach),
                0.28,
                0.0,
                0.0,
            ],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&uniforms));
        let background = colour(look.background);
        self.targets(device, size);
        let targets = self.targets.as_ref().expect("made above");
        let (view, resolve) = match &targets.samples {
            Some(samples) => (samples, Some(target)),
            None => (target, None),
        };
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("3D view"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: resolve,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(background[0]),
                            g: f64::from(background[1]),
                            b: f64::from(background[2]),
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &targets.depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Discard,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, &self.bindings, &[]);
            if let Some(figure) = &self.figure {
                pass.set_pipeline(&self.floor_pipeline);
                pass.set_vertex_buffer(0, self.floor.slice(..));
                pass.draw(0..6, 0..1);
                pass.set_pipeline(&self.figure_pipeline);
                pass.set_vertex_buffer(0, figure.vertices.slice(..));
                pass.set_index_buffer(figure.indices.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..figure.count, 0, 0..1);
                for cloth in [&self.cloth, &self.prop].into_iter().flatten() {
                    pass.set_pipeline(&self.cloth_pipeline);
                    pass.set_vertex_buffer(0, cloth.vertices.slice(..));
                    pass.set_index_buffer(cloth.indices.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..cloth.count, 0, 0..1);
                }
            }
        }
        queue.submit([encoder.finish()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use naga::back::glsl;

    #[test]
    fn the_shader_is_valid_and_translates_for_webgl2() {
        let module = naga::front::wgsl::parse_str(include_str!("shader.wgsl")).expect("WGSL");
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("a valid shader");
        // The uniforms laid out as the shader reads them.
        let uniforms = module
            .types
            .iter()
            .find(|(_, t)| t.name.as_deref() == Some("Uniforms"))
            .expect("the uniforms");
        assert_eq!(
            uniforms.1.inner.size(module.to_ctx()) as usize,
            std::mem::size_of::<Uniforms>()
        );
        // WebGL2 reads GLSL ES 3.0, translated from it.
        for (stage, entry) in [
            (naga::ShaderStage::Vertex, "figure_vertex"),
            (naga::ShaderStage::Fragment, "figure_fragment"),
            (naga::ShaderStage::Vertex, "cloth_vertex"),
            (naga::ShaderStage::Fragment, "cloth_fragment"),
            (naga::ShaderStage::Vertex, "floor_vertex"),
            (naga::ShaderStage::Fragment, "floor_fragment"),
        ] {
            let options = glsl::Options {
                version: glsl::Version::new_gles(300),
                ..Default::default()
            };
            let pipeline = glsl::PipelineOptions {
                shader_stage: stage,
                entry_point: entry.into(),
                multiview: None,
            };
            let mut out = String::new();
            glsl::Writer::new(
                &mut out,
                &module,
                &info,
                &options,
                &pipeline,
                naga::proc::BoundsCheckPolicies::default(),
            )
            .and_then(|mut w| w.write())
            .unwrap_or_else(|e| panic!("{entry}: {e}"));
            assert!(out.contains("void main()"), "{entry}");
        }
    }

    #[test]
    fn page_colours_are_blended_in_linear_light() {
        assert!((linear(0.5) - 0.214).abs() < 1e-3);
        assert!(linear(0.0).abs() < 1e-9);
        assert!((linear(1.0) - 1.0).abs() < 1e-6);
    }
}
