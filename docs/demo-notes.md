# Demo notes

What you can try after each step. Newest first.

To run the app locally:

```bash
pnpm install
pnpm wasm
pnpm dev        # then open http://localhost:5173
```

---

## Step 2 — Geometry kernel (Phase 1, day 2)

Nothing new in the browser yet; this step is the foundation the next ones
draw with.

**Try it:** `cargo test -p soom-pattern-core`. Seven property tests run
256 random pieces each:

* edge lengths add up to the outline length;
* a point found by distance along an edge is found again by "nearest
  point" (what tapping an edge will use);
* splitting an edge and joining it back changes nothing, and notches stay
  exactly where they were (to 0.000001 mm);
* reversing an outline twice gives back the identical piece;
* every piece survives a save/load round trip **bit for bit**. This test
  caught that the JSON library rounds the last digit of some numbers by
  default, which would have been a silent geometry change on every save. It
  now uses exact round-tripping.

**What's under the hood:** the piece model in
`crates/pattern-core/src/model.rs` *is* the file format (serde types, with
TypeScript types generated into `packages/engine/src/generated`), plus
measuring (`geom.rs`), edge split/join/reverse that keep notches and corner
styles in place (`ops.rs`), and the validation rules from the format spec
(`validate.rs`).

---

## Step 1 — Scaffold (Phase 1, day 1)

**Try it:**

1. `pnpm dev` and open http://localhost:5173. You should see the green
   cutting mat and a **This device** card.
2. All four checks should be green in current Chrome and Safari 26:
   *Saves on this device*, *Works without internet*, *3D draping*, *Fast
   mode*. Firefox shows *3D draping* as unavailable unless WebGPU is enabled.
3. The card ends with "Pattern engine 0.1.0". That text comes from the Rust
   engine compiled to WebAssembly, which proves the Rust ↔ browser pipeline.
4. Offline check: `pnpm build && pnpm preview`, open
   http://localhost:4173, reload once, then stop the server and reload
   again. The app still opens.

**What's under the hood:** Cargo workspace (`pattern-core`, `wasm-core`),
pnpm workspace (`web`, `packages/engine`), COOP/COEP headers in dev, preview
and production (`web/public/_headers`, which also sets a CSP that blocks
requests to other sites), PWA manifest and service worker, CI with Rust,
licence, type, lint, unit and Playwright (Chromium + iPad WebKit) jobs.
