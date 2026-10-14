# Licences of dependencies and assets

Every third-party dependency and asset Drapewright uses, with its licence. CI
checks every Rust crate in the build with cargo-deny (`deny.toml`) and
every production npm package with `pnpm licences`
(`scripts/check-licences.mjs`), both against the allow-list below. The
tables are maintained by hand and list the direct dependencies and why we
use them.

**Allow-list** (enforced in CI): MIT, Apache-2.0 (also WITH
LLVM-exception), BSD-2/3-Clause, ISC, Zlib, Unicode-3.0, CC0-1.0, 0BSD,
BSL-1.0. MPL-2.0 only by explicit exception recorded here. Nothing
GPL/LGPL/AGPL.

Project licence: **not yet chosen** (see
[open-questions.md](open-questions.md#licence)).

## Direct Rust dependencies

| Crate | Licence | Used for | Used in |
|---|---|---|---|
| kurbo | Apache-2.0 OR MIT | Bézier maths, arc length, nearest point, offsets | pattern-core |
| i_overlay | MIT OR Apache-2.0 | Robust polygon clean-up for cutting lines | pattern-core |
| serde, serde_json | MIT OR Apache-2.0 | Project file (de)serialisation | all |
| thiserror | MIT OR Apache-2.0 | Error types | all |
| usvg | Apache-2.0 OR MIT | SVG parsing (CSS, transforms, units) | io |
| roxmltree | MIT OR Apache-2.0 | Reading SVG root attributes and text | io |
| sha2 | MIT OR Apache-2.0 | Hashing imported files for provenance | io |
| pdf-writer | MIT OR Apache-2.0 | Tiled PDF export | io |
| flate2 (with miniz_oxide: MIT OR Zlib OR Apache-2.0) | MIT OR Apache-2.0 | Compressing PDF page content; the body file | io, body |
| zip | MIT | `.drapewright` container (deflate only, through flate2) | project |
| schemars | MIT | JSON Schema generation for the public format | pattern-core, project |
| ts-rs | MIT | TypeScript types generated from Rust | project, wasm-core |
| wasm-bindgen | MIT OR Apache-2.0 | JS bindings (facade crate only) | wasm-core |
| tiny-skia (with png) | BSD-3-Clause | Pictures of pieces for AI assistants; pictures of figures and drapes to check by eye (examples) | mcp, body, sim |
| md-5, aes, sha2 | MIT OR Apache-2.0 | Opening "protected" PDF patterns (encrypted with an empty user password), as viewers do | pdf-import |
| serde-wasm-bindgen | MIT | Passing data across the boundary | wasm-core, wasm-3d |
| wgpu | MIT OR Apache-2.0 | Drawing the 3D view: WebGPU, or WebGL2 where there is none | render, wasm-3d |
| bytemuck | Zlib OR Apache-2.0 OR MIT | The 3D view's vertices and settings as bytes | render |
| spade | MIT OR Apache-2.0 | Meshing pattern pieces for the drape (constrained Delaunay triangulation) | sim |
| web-sys, wasm-bindgen-futures | MIT OR Apache-2.0 | The canvas handed to the sim worker; waiting for the GPU | wasm-3d |
| proptest *(dev)* | MIT OR Apache-2.0 | Property-based tests | tests |
| insta *(dev)* | Apache-2.0 | Golden-file snapshot tests | tests |
| lopdf *(dev)* | MIT | Reading generated PDFs back in tests | tests |
| naga *(dev)* | MIT OR Apache-2.0 | Checking the 3D view's shader, and its WebGL2 translation, without a GPU | render |

## Direct npm dependencies

| Package | Licence | Used for |
|---|---|---|
| react, react-dom | MIT | UI |
| react-aria-components | Apache-2.0 | Accessible, touch- and pen-correct components |
| lucide-react | ISC | Icons |
| idb | ISC | IndexedDB helper (project list; project files where OPFS is unavailable) |
| vite, @vitejs/plugin-react | MIT | Build and dev server |
| vite-plugin-pwa (Workbox) | MIT | Service worker and manifest |
| typescript | Apache-2.0 | Type checking |
| vitest | MIT | Unit tests |
| @playwright/test | Apache-2.0 | End-to-end tests |
| ajv *(dev)* | MIT | Checking saved projects against the published JSON Schema in tests |
| binaryen *(dev)* | Apache-2.0 | `wasm-opt`, shrinking the WebAssembly build |
| eslint, typescript-eslint, prettier | MIT | Linting and formatting |
| @freesewing/core, its plugins and designs (teagan, sasha, penelope, sandy, cathrin, bella, simone, diana, and those they build on), @freesewing/models | MIT | Drafting FreeSewing's designs to someone's measurements, in the browser, loaded only when one is drafted; © Joost De Cock and contributors, credited in the dialog with the licence at `/licences/freesewing.txt`. `hooks` (MIT, by Brian Noguchi), which FreeSewing's core uses, says so where pnpm doesn't read it, so the licence check knows it by hand. |

## Assets

| Asset | Licence | Notes |
|---|---|---|
| UI font | — | System font stack; nothing bundled. |
| PDF font | — | Helvetica, one of the 14 standard PDF fonts, referenced and not embedded. |
| Test fixtures in `fixtures/` | CC0-1.0 unless noted | Drawn for this project; each folder has a `LICENCE` note. |
| Sample patterns in the app | CC0-1.0, and MIT for the dress | The test blouse and skirt (`fixtures/drapewright/`), and FreeSewing's Sasha dress as a PDF (`fixtures/pdf/freesewing-sasha-example-a0.pdf`, drafted without its pockets; MIT, © Joost De Cock), shipped with its licence at `/licences/freesewing.txt` and credited beside the samples. |
| Body model, `assets/body/adult.dwbody` | CC0-1.0 data; Apache-2.0 method | MakeHuman's mesh, shape targets and skin weights (via MPFB2, CC0-1.0), prepared as in Anny (Copyright 2025 NAVER Corp., Apache-2.0): the adult shape blending and mesh edits. Built by `crates/body/examples/build_body.rs`; see `assets/body/README.md`. Never Anny's non-commercial `smplx` topology, never SMPL. |
| Fabric textures (Phase 3) | to choose | Prefer CC0 sources. |
