# Soom

*(working name)* An open-source garment pattern studio for home sewists that
runs in the browser. Open the patterns you already own, fit them to your
measurements, see them draped in 3D, adjust them, and print them at exactly
the right size.

**Status:** Phase 1 (true-scale foundation) is complete for review: open
SVG and DXF patterns, view them at true size, set seam allowance edge by
edge, mark notches and grainlines, and print a tiled PDF at exactly 100 %.
Projects are saved on the device and can be backed up as `.soom` files.

## Documents

* [Engineering proposal](docs/proposal.md): architecture, layout, input
  design, Phase 1 plan
* [Project file format](docs/file-format.md): the open `.soom` format, with
  its [JSON Schema](docs/schema/project.schema.json)
* [Open questions](docs/open-questions.md): sewing and product decisions
  waiting for an answer
* [Licences](docs/licences.md): every dependency and asset
* [Demo notes](docs/demo-notes.md): what you can try after each step

## Development

Requirements: Rust (the toolchain in `rust-toolchain.toml` installs
itself), `wasm-pack`, Node 22+, pnpm 9+.

```bash
pnpm install
pnpm wasm      # build the Rust engine to WebAssembly (and shrink it with wasm-opt)
pnpm dev       # http://localhost:5173 (served cross-origin isolated)
```

Tests:

```bash
cargo test --workspace   # Rust: unit, property and golden tests
pnpm test                # TypeScript unit tests
pnpm e2e                 # Playwright, Chromium + WebKit
```

## Licence

Not chosen yet; see [open questions](docs/open-questions.md#licence).
