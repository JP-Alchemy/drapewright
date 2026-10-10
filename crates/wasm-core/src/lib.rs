//! WebAssembly facade for Soom's 2D engine.
//!
//! This crate only converts between JavaScript and Rust types. All logic
//! lives in the platform-independent crates it wraps, so a native shell can
//! use them without going through here.

use wasm_bindgen::prelude::*;

/// Called once by the host after the module is instantiated.
#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

/// Engine version, shown in the app's About panel.
#[wasm_bindgen(js_name = engineVersion)]
pub fn engine_version() -> String {
    soom_pattern_core::VERSION.to_string()
}
