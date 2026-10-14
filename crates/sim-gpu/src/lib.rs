//! The drape's cloth moved on on the graphics card: the same small XPBD
//! steps as `drapewright-sim` takes on the processor, in compute shaders
//! (`cloth.wgsl`). Edges, bends and stitches are coloured, no two of a
//! colour sharing a point, and each colour solved at once. The cloth's
//! points come back to the processor a frame or two behind, for showing it
//! and telling when it has settled.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use bytemuck::{Pod, Zeroable};
use drapewright_sim::cloth::{Cloth, Parts, V3};
use drapewright_sim::collide::Grid;
use drapewright_sim::drape::{Felt, Stepper};
use wgpu::util::DeviceExt;

/// Points a workgroup moves on (as in the shaders).
const WORKGROUP: u32 = 64;
/// Frames sent to the graphics card ahead of those it has done.
const AHEAD: u64 = 8;
/// A range's entry in its uniform buffer, bytes: as uniform buffer offsets
/// must be aligned.
const RANGE_STRIDE: u64 = 256;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct Params {
    gravity: [f32; 4],
    seat_low: [f32; 4],
    seat_high: [f32; 4],
    shift: [f32; 4],
    a: [f32; 4],
    b: [f32; 4],
    counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
struct GridUniform {
    low: [f32; 4],
    size: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct EdgeGpu {
    i: u32,
    j: u32,
    rest: f32,
    compliance: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BendGpu {
    i: [u32; 4],
    k: [f32; 4],
    compliance: [f32; 4],
}

/// Why the cloth can't be moved on on the graphics card.
#[derive(Debug)]
pub struct Unsupported(pub String);

impl std::fmt::Display for Unsupported {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unsupported {}

/// Constraints in colours: each colour's constraints share no point. Gives
/// the order to lay them out in, and each colour's start and length.
fn coloured(points: impl Iterator<Item = Vec<u32>>, n: usize) -> (Vec<usize>, Vec<(u32, u32)>) {
    let mut used: Vec<Vec<u16>> = vec![Vec::new(); n];
    let mut colour_of = Vec::new();
    let mut colours = 0u16;
    for corners in points {
        let mut c = 0u16;
        while corners.iter().any(|&p| used[p as usize].contains(&c)) {
            c += 1;
        }
        for &p in &corners {
            used[p as usize].push(c);
        }
        colours = colours.max(c + 1);
        colour_of.push(c);
    }
    let mut order: Vec<usize> = (0..colour_of.len()).collect();
    order.sort_by_key(|&k| colour_of[k]);
    let mut ranges = Vec::new();
    let mut start = 0u32;
    for c in 0..colours {
        let count = colour_of.iter().filter(|&&x| x == c).count() as u32;
        ranges.push((start, count));
        start += count;
    }
    (order, ranges)
}

/// Each point's list of things (triangles, or points near it), as the
/// shaders read it: where each point's list starts (one more than the
/// points), then the lists, as indices into the one array.
fn lists(n: usize, items: impl Iterator<Item = (usize, u32)>) -> Vec<u32> {
    let mut per: Vec<Vec<u32>> = vec![Vec::new(); n];
    for (k, item) in items {
        per[k].push(item);
    }
    let mut out = Vec::with_capacity(n + 1 + per.iter().map(Vec::len).sum::<usize>());
    let mut at = (n + 1) as u32;
    for list in &per {
        out.push(at);
        at += list.len() as u32;
    }
    out.push(at);
    for list in per {
        out.extend(list);
    }
    out
}

/// A buffer at least four bytes long (empty bindings are not allowed).
fn storage(
    device: &wgpu::Device,
    label: &str,
    bytes: &[u8],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    let padded;
    let contents = if bytes.len() < 16 {
        padded = vec![0u8; 16];
        &padded[..]
    } else {
        bytes
    };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents,
        usage: wgpu::BufferUsages::STORAGE | usage,
    })
}

/// What each entry point binds in group 0.
const PREDICT: &[u32] = &[0, 1, 2, 3, 4, 15];
const SHIFT: &[u32] = &[0, 1];
const EDGES: &[u32] = &[0, 1, 5];
const BENDS: &[u32] = &[0, 1, 6];
const STITCHES: &[u32] = &[0, 1, 7];
const COLLIDE: &[u32] = &[0, 1, 2, 4, 8, 9, 10, 11];
const FACE_OUT: &[u32] = &[0, 1, 8, 9, 12];
const LAYERS: &[u32] = &[0, 1, 2, 12, 13, 15];
const LAYERS_OUT: &[u32] = &[0, 1, 2, 12, 14, 15];
const VELOCITY: &[u32] = &[0, 1, 2, 3, 4, 15];

struct Pass {
    pipeline: wgpu::ComputePipeline,
    group: wgpu::BindGroup,
    /// For constraints: each colour's range, bound as group 1.
    colours: Vec<(wgpu::BindGroup, u32)>,
}

/// The cloth on the graphics card.
pub struct GpuCloth {
    device: wgpu::Device,
    queue: wgpu::Queue,
    n: u32,
    inside_from: u32,
    params: wgpu::Buffer,
    grids: wgpu::Buffer,
    x: wgpu::Buffer,
    staging: wgpu::Buffer,
    fields: wgpu::Buffer,
    /// The fields in `fields` now: their numbers.
    fields_in: (u64, Option<u64>),
    layers: wgpu::Buffer,
    layers_out: wgpu::Buffer,
    /// Which finding of the layers' pairs is on the card.
    layers_in: u64,
    passes: Passes,
    /// Bound buffers by binding, to bind again when one is made anew.
    bound: Vec<Option<wgpu::Buffer>>,
    gap: f32,
    friction: f32,
    apart: f32,
    reach: f32,
    /// Moved by this before the next step.
    shift: V3,
    /// Frames sent, and done.
    sent: u64,
    done: Arc<AtomicU64>,
    /// Points being read back (from the frame after which they were
    /// copied), and whether they are here.
    reading: Option<(u64, Arc<AtomicBool>)>,
    /// The frame the cloth here was last brought up to date with.
    read: u64,
    /// The step (s) of the last frame.
    dt: f32,
}

struct Passes {
    predict: Pass,
    shift: Pass,
    edges: Pass,
    bends: Pass,
    stitches: Pass,
    collide: Pass,
    face_out: Pass,
    layers: Pass,
    layers_out: Pass,
    velocity: Pass,
}

impl std::fmt::Debug for GpuCloth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GpuCloth")
            .field("points", &self.n)
            .field("sent", &self.sent)
            .finish_non_exhaustive()
    }
}

impl GpuCloth {
    /// The cloth `cloth`, as it is now, put on the graphics card `device`;
    /// an error if it can't do the work (no compute shaders, or too few
    /// storage buffers).
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        cloth: &Cloth,
    ) -> Result<GpuCloth, Unsupported> {
        let limits = device.limits();
        if limits.max_storage_buffers_per_shader_stage < 8
            || limits.max_compute_invocations_per_workgroup < WORKGROUP
        {
            return Err(Unsupported("no compute shaders".into()));
        }
        let parts: Parts = cloth.parts();
        let n = parts.x.len();
        let vec4 = |v: &[V3], w: &[f32]| -> Vec<[f32; 4]> {
            v.iter()
                .enumerate()
                .map(|(k, p)| [p[0], p[1], p[2], w.get(k).copied().unwrap_or(0.0)])
                .collect()
        };
        let x = vec4(&parts.x, &parts.w);
        let v = vec4(&parts.v, &[]);
        let copy = wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC;
        let buf = |label: &str, data: &[u8]| storage(device, label, data, copy);
        let x_buf = buf("cloth x", bytemuck::cast_slice(&x));
        let p_buf = buf("cloth p", bytemuck::cast_slice(&x));
        let v_buf = buf("cloth v", bytemuck::cast_slice(&v));
        let contact = buf("cloth contact", &vec![0u8; n * 16]);
        let layered = buf("cloth layered", &vec![0u8; n * 16]);
        // Constraints, in colours.
        let (edge_order, edge_ranges) = coloured(parts.edges.iter().map(|e| vec![e.0, e.1]), n);
        let edges: Vec<EdgeGpu> = edge_order
            .iter()
            .map(|&k| {
                let (i, j, rest, compliance) = parts.edges[k];
                EdgeGpu {
                    i,
                    j,
                    rest,
                    compliance,
                }
            })
            .collect();
        let (bend_order, bend_ranges) = coloured(parts.bends.iter().map(|b| b.0.to_vec()), n);
        let bends: Vec<BendGpu> = bend_order
            .iter()
            .map(|&k| {
                let (i, kk, compliance) = parts.bends[k];
                BendGpu {
                    i,
                    k: kk,
                    compliance: [compliance, 0.0, 0.0, 0.0],
                }
            })
            .collect();
        let (stitch_order, stitch_ranges) = coloured(parts.stitches.iter().map(|s| s.to_vec()), n);
        let stitches: Vec<[u32; 2]> = stitch_order.iter().map(|&k| parts.stitches[k]).collect();
        let tris: Vec<[u32; 4]> = parts
            .triangles
            .iter()
            .zip(parts.facing.iter().chain(std::iter::repeat(&1.0)))
            .map(|(t, &f)| [t[0], t[1], t[2], u32::from(f > 0.0)])
            .collect();
        let around = lists(
            n,
            parts
                .triangles
                .iter()
                .enumerate()
                .flat_map(|(k, t)| t.iter().map(move |&c| (c as usize, k as u32))),
        );
        let edges_buf = buf("cloth edges", bytemuck::cast_slice(&edges));
        let bends_buf = buf("cloth bends", bytemuck::cast_slice(&bends));
        let stitches_buf = buf("cloth stitches", bytemuck::cast_slice(&stitches));
        let tris_buf = buf("cloth triangles", bytemuck::cast_slice(&tris));
        let around_buf = buf("cloth around", bytemuck::cast_slice(&around));
        let outward = buf("cloth outward", &vec![0u8; n * 16]);
        let layers = buf("cloth layers", &[0u8; 16]);
        let layers_out = buf("cloth layers out", &[0u8; 16]);
        let fields = buf("body fields", &[0u8; 16]);
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cloth params"),
            size: std::mem::size_of::<Params>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let grids = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("body grids"),
            size: 2 * std::mem::size_of::<GridUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let staging = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cloth read back"),
            size: (n.max(1) * 16) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cloth"),
            source: wgpu::ShaderSource::Wgsl(include_str!("cloth.wgsl").into()),
        });
        let mut bound: Vec<Option<wgpu::Buffer>> = vec![None; 16];
        for (k, b) in [
            (0, &params),
            (1, &x_buf),
            (2, &p_buf),
            (3, &v_buf),
            (4, &contact),
            (5, &edges_buf),
            (6, &bends_buf),
            (7, &stitches_buf),
            (8, &tris_buf),
            (9, &around_buf),
            (10, &grids),
            (11, &fields),
            (12, &outward),
            (13, &layers),
            (14, &layers_out),
            (15, &layered),
        ] {
            bound[k] = Some(b.clone());
        }
        // Each colour's range, in one buffer.
        let all_ranges: Vec<(u32, u32)> = edge_ranges
            .iter()
            .chain(&bend_ranges)
            .chain(&stitch_ranges)
            .copied()
            .collect();
        let mut range_bytes = vec![0u8; (all_ranges.len().max(1) as u64 * RANGE_STRIDE) as usize];
        for (k, &(start, count)) in all_ranges.iter().enumerate() {
            let at = k * RANGE_STRIDE as usize;
            range_bytes[at..at + 8].copy_from_slice(bytemuck::cast_slice(&[start, count]));
        }
        let ranges = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("cloth colours"),
            contents: &range_bytes,
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let make = |entry: &str, bindings: &[u32], colours: &[(u32, u32)], first: usize| -> Pass {
            let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            });
            let group = group_for(device, &pipeline, bindings, &bound);
            let layout = if colours.is_empty() {
                None
            } else {
                Some(pipeline.get_bind_group_layout(1))
            };
            let colours = colours
                .iter()
                .enumerate()
                .map(|(c, &(_, count))| {
                    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("colour"),
                        layout: layout.as_ref().expect("a constraint pass"),
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                                buffer: &ranges,
                                offset: (first + c) as u64 * RANGE_STRIDE,
                                size: std::num::NonZeroU64::new(16),
                            }),
                        }],
                    });
                    (group, count)
                })
                .collect();
            Pass {
                pipeline,
                group,
                colours,
            }
        };
        let passes = Passes {
            predict: make("predict", PREDICT, &[], 0),
            shift: make("shift", SHIFT, &[], 0),
            edges: make("solve_edges", EDGES, &edge_ranges, 0),
            bends: make("solve_bends", BENDS, &bend_ranges, edge_ranges.len()),
            stitches: make(
                "solve_stitches",
                STITCHES,
                &stitch_ranges,
                edge_ranges.len() + bend_ranges.len(),
            ),
            collide: make("collide", COLLIDE, &[], 0),
            face_out: make("face_out", FACE_OUT, &[], 0),
            layers: make("order_layers", LAYERS, &[], 0),
            layers_out: make("order_layers_out", LAYERS_OUT, &[], 0),
            velocity: make("velocity", VELOCITY, &[], 0),
        };
        Ok(GpuCloth {
            device: device.clone(),
            queue: queue.clone(),
            n: n as u32,
            inside_from: parts.inside_from.min(n) as u32,
            params,
            grids,
            x: x_buf,
            staging,
            fields,
            fields_in: (0, None),
            layers,
            layers_out,
            layers_in: 0,
            passes,
            bound,
            gap: parts.gap,
            friction: parts.friction,
            apart: parts.apart,
            reach: parts.reach,
            shift: [0.0; 3],
            sent: 0,
            done: Arc::new(AtomicU64::new(0)),
            reading: None,
            read: 0,
            dt: 0.0,
        })
    }

    /// How many colours each kind of constraint is solved in: edges,
    /// bends, stitches.
    pub fn colours(&self) -> [usize; 3] {
        [
            self.passes.edges.colours.len(),
            self.passes.bends.colours.len(),
            self.passes.stitches.colours.len(),
        ]
    }

    /// Binds a buffer made anew, everywhere it is bound.
    fn rebind(&mut self, binding: u32, buffer: wgpu::Buffer) {
        self.bound[binding as usize] = Some(buffer);
        let device = &self.device;
        let bound = &self.bound;
        for (pass, bindings) in [
            (&mut self.passes.collide, COLLIDE),
            (&mut self.passes.layers, LAYERS),
            (&mut self.passes.layers_out, LAYERS_OUT),
        ] {
            if bindings.contains(&binding) {
                pass.group = group_for(device, &pass.pipeline, bindings, bound);
            }
        }
    }

    /// The fields felt, on the card: written again only when they change.
    fn put_fields(&mut self, felt: &Felt<'_>) {
        let a = felt.body.grid();
        let b = felt.towards.map(|(f, _)| f.grid());
        let ids = (a.id, b.map(|g| g.id));
        if ids == self.fields_in {
            return;
        }
        let total = a.values.len() + b.map_or(0, |g| g.values.len());
        let bytes = (total * 4) as u64;
        if bytes > self.fields.size() {
            let fields = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("body fields"),
                size: bytes.next_power_of_two(),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.fields = fields.clone();
            self.rebind(11, fields);
        }
        let uniform = |g: &Grid<'_>, base: usize| GridUniform {
            low: [g.low[0], g.low[1], g.low[2], g.cell],
            size: [
                g.size[0] as u32,
                g.size[1] as u32,
                g.size[2] as u32,
                base as u32,
            ],
        };
        self.queue
            .write_buffer(&self.fields, 0, bytemuck::cast_slice(a.values));
        let mut grids = [uniform(&a, 0), GridUniform::default()];
        if let Some(b) = &b {
            self.queue.write_buffer(
                &self.fields,
                (a.values.len() * 4) as u64,
                bytemuck::cast_slice(b.values),
            );
            grids[1] = uniform(b, a.values.len());
        }
        self.queue
            .write_buffer(&self.grids, 0, bytemuck::cast_slice(&grids));
        self.fields_in = ids;
    }

    /// Each inside point's outside points near it, as found here.
    fn put_layers(&mut self, cloth: &mut Cloth) {
        if self.inside_from >= self.n {
            return;
        }
        let first = self.inside_from as usize;
        let inside = self.n as usize - first;
        let (found, pairs) = cloth.near_layers();
        if found == self.layers_in {
            return;
        }
        self.layers_in = found;
        let pairs = pairs.to_vec();
        let ins = lists(inside, pairs.iter().map(|&(i, j)| (i as usize - first, j)));
        let outs = lists(first, pairs.iter().map(|&(i, j)| (j as usize, i)));
        for (binding, list) in [(13, ins), (14, outs)] {
            let bytes = (list.len() * 4) as u64;
            let buffer = if binding == 13 {
                &self.layers
            } else {
                &self.layers_out
            };
            if bytes > buffer.size() {
                let made = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("cloth layers"),
                    size: bytes.next_power_of_two(),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                if binding == 13 {
                    self.layers = made.clone();
                } else {
                    self.layers_out = made.clone();
                }
                self.rebind(binding, made);
            }
            let buffer = if binding == 13 {
                &self.layers
            } else {
                &self.layers_out
            };
            self.queue
                .write_buffer(buffer, 0, bytemuck::cast_slice(&list));
        }
    }

    /// Brings the cloth here up to date if its points have come back.
    fn collect(&mut self, cloth: &mut Cloth) {
        let Some((frame, here)) = &self.reading else {
            return;
        };
        if !here.load(Ordering::Acquire) {
            return;
        }
        let frame = *frame;
        let x: Vec<V3> = {
            let Ok(data) = self.staging.slice(..).get_mapped_range() else {
                return;
            };
            let all: &[[f32; 4]] = bytemuck::cast_slice(&data);
            all.iter()
                .take(self.n as usize)
                .map(|p| [p[0], p[1], p[2]])
                .collect()
        };
        self.staging.unmap();
        cloth.moved_to(x, (frame - self.read) as f32 * self.dt);
        self.read = frame;
        self.reading = None;
    }

    fn poll(&self, wait: bool) {
        let _ = self.device.poll(if wait {
            wgpu::PollType::wait_indefinitely()
        } else {
            wgpu::PollType::Poll
        });
    }
}

/// A bind group for group 0 of `pipeline`, binding `bindings`.
fn group_for(
    device: &wgpu::Device,
    pipeline: &wgpu::ComputePipeline,
    bindings: &[u32],
    bound: &[Option<wgpu::Buffer>],
) -> wgpu::BindGroup {
    let entries: Vec<wgpu::BindGroupEntry<'_>> = bindings
        .iter()
        .map(|&b| wgpu::BindGroupEntry {
            binding: b,
            resource: bound[b as usize]
                .as_ref()
                .expect("every binding has a buffer")
                .as_entire_binding(),
        })
        .collect();
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &entries,
    })
}

impl Stepper for GpuCloth {
    fn ready(&mut self) -> bool {
        self.poll(false);
        self.sent < self.done.load(Ordering::Acquire) + AHEAD
    }

    fn step(&mut self, cloth: &mut Cloth, dt: f32, substeps: u32, felt: &Felt<'_>) {
        self.poll(false);
        self.collect(cloth);
        self.dt = dt;
        self.put_fields(felt);
        self.put_layers(cloth);
        let h = dt / substeps.max(1) as f32;
        let (seat_low, seat_high) = felt.seat.map_or(([0.0; 3], [0.0; 3]), |s| (s.low, s.high));
        let params = Params {
            gravity: [cloth.gravity[0], cloth.gravity[1], cloth.gravity[2], h],
            seat_low: [
                seat_low[0],
                seat_low[1],
                seat_low[2],
                f32::from(u8::from(felt.seat.is_some())),
            ],
            seat_high: [
                seat_high[0],
                seat_high[1],
                seat_high[2],
                felt.towards.map_or(0.0, |(_, s)| s),
            ],
            shift: [
                self.shift[0],
                self.shift[1],
                self.shift[2],
                cloth.max_speed.min(f32::MAX),
            ],
            a: [
                (-cloth.damping * h).exp(),
                self.gap,
                self.friction,
                cloth.stitch_compliance / (h * h),
            ],
            b: [self.apart, self.reach, 0.0, 0.0],
            counts: [
                self.n,
                self.inside_from,
                u32::from(felt.towards.is_some()),
                0,
            ],
        };
        self.queue
            .write_buffer(&self.params, 0, bytemuck::bytes_of(&params));
        let groups = |count: u32| count.div_ceil(WORKGROUP);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cloth"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("cloth steps"),
                timestamp_writes: None,
            });
            let run = |pass: &mut wgpu::ComputePass<'_>, p: &Pass, count: u32| {
                pass.set_pipeline(&p.pipeline);
                pass.set_bind_group(0, &p.group, &[]);
                pass.dispatch_workgroups(groups(count), 1, 1);
            };
            let run_colours = |pass: &mut wgpu::ComputePass<'_>, p: &Pass| {
                pass.set_pipeline(&p.pipeline);
                pass.set_bind_group(0, &p.group, &[]);
                for (group, count) in &p.colours {
                    pass.set_bind_group(1, group, &[]);
                    pass.dispatch_workgroups(groups(*count), 1, 1);
                }
            };
            if self.shift.iter().any(|&c| c != 0.0) {
                run(&mut pass, &self.passes.shift, self.n);
            }
            let layered = self.inside_from < self.n;
            for _ in 0..substeps.max(1) {
                run(&mut pass, &self.passes.predict, self.n);
                run_colours(&mut pass, &self.passes.edges);
                run_colours(&mut pass, &self.passes.bends);
                run_colours(&mut pass, &self.passes.stitches);
                run(&mut pass, &self.passes.collide, self.n);
                if layered {
                    run(&mut pass, &self.passes.face_out, self.inside_from);
                    run(&mut pass, &self.passes.layers_out, self.inside_from);
                    run(&mut pass, &self.passes.layers, self.n - self.inside_from);
                }
                run(&mut pass, &self.passes.velocity, self.n);
            }
        }
        self.shift = [0.0; 3];
        self.sent += 1;
        let read = self.reading.is_none();
        if read {
            encoder.copy_buffer_to_buffer(&self.x, 0, &self.staging, 0, self.staging.size());
        }
        self.queue.submit([encoder.finish()]);
        let done = self.done.clone();
        let frame = self.sent;
        self.queue.on_submitted_work_done(move || {
            done.fetch_max(frame, Ordering::AcqRel);
        });
        if read {
            let here = Arc::new(AtomicBool::new(false));
            let flag = here.clone();
            self.staging
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if result.is_ok() {
                        flag.store(true, Ordering::Release);
                    }
                });
            self.reading = Some((frame, here));
        }
    }

    fn shift(&mut self, cloth: &mut Cloth, by: V3) {
        for (s, b) in self.shift.iter_mut().zip(by) {
            *s += b;
        }
        cloth.shift(by);
    }

    fn catch_up(&mut self, cloth: &mut Cloth) {
        // In the browser nothing can be waited for: the cloth comes back
        // as it does.
        if cfg!(target_arch = "wasm32") {
            return;
        }
        // What is being read, then the cloth as it is now.
        while self.reading.is_some() {
            self.poll(true);
            self.collect(cloth);
        }
        if self.sent == self.read {
            return;
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("catch up"),
            });
        encoder.copy_buffer_to_buffer(&self.x, 0, &self.staging, 0, self.staging.size());
        self.queue.submit([encoder.finish()]);
        let here = Arc::new(AtomicBool::new(false));
        let flag = here.clone();
        self.staging
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                if result.is_ok() {
                    flag.store(true, Ordering::Release);
                }
            });
        self.reading = Some((self.sent, here));
        while self.reading.is_some() {
            self.poll(true);
            self.collect(cloth);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shaders_are_valid() {
        let module = naga::front::wgsl::parse_str(include_str!("cloth.wgsl")).expect("WGSL");
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .expect("valid shaders");
        // The parameters laid out as the shaders read them.
        let size = |name: &str| {
            let (_, ty) = module
                .types
                .iter()
                .find(|(_, t)| t.name.as_deref() == Some(name))
                .expect(name);
            ty.inner.size(module.to_ctx()) as usize
        };
        assert_eq!(size("Params"), std::mem::size_of::<Params>());
        assert_eq!(size("Grid"), std::mem::size_of::<GridUniform>());
        assert_eq!(size("Edge"), std::mem::size_of::<EdgeGpu>());
        assert_eq!(size("Bend"), std::mem::size_of::<BendGpu>());
    }

    #[test]
    fn no_two_constraints_of_a_colour_share_a_point() {
        let edges = [[0, 1], [1, 2], [2, 3], [3, 0], [0, 2]];
        let (order, ranges) = coloured(edges.iter().map(|e| e.to_vec()), 4);
        assert_eq!(order.len(), 5);
        for &(start, count) in &ranges {
            let mut seen = Vec::new();
            for k in start..start + count {
                for p in edges[order[k as usize]] {
                    assert!(!seen.contains(&p), "point {p} twice in a colour");
                    seen.push(p);
                }
            }
        }
        assert_eq!(ranges.iter().map(|r| r.1).sum::<u32>(), 5);
    }

    #[test]
    fn lists_say_where_each_points_starts() {
        let list = lists(3, [(0, 7), (2, 8), (0, 9)].into_iter());
        assert_eq!(list, [4, 6, 6, 7, 7, 9, 8]);
    }
}
