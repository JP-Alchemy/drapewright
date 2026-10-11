//! Drawing Drapewright's 3D view with wgpu: the avatar now, the drape to
//! come. The drawing ([`Renderer`]) needs only a device, so it runs wherever
//! wgpu does; the page's canvas is attached in `drapewright-wasm-3d`.
//! Millimetres throughout, Y up, the figure facing +Z.

pub mod camera;
pub mod mesh;
mod renderer;

pub use camera::{Matrix, Orbit};
pub use mesh::Mesh;
pub use renderer::{Look, Renderer};
