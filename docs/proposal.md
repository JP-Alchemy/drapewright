# Soom — engineering proposal

Version 0.1 · 2026-10-10 · status: **for review**

This answers the four things asked for before code: the repository layout,
the project file schema, the input-layer design, and a two-week plan for
Phase 1. Sewing questions I could not settle are collected in
[open-questions.md](open-questions.md) with the provisional default I use
until they are answered. Items marked **[you]** need a decision from you.

---

## 0. The decisions in one screen

| Area | Decision | Why |
|---|---|---|
| Source of truth | The Rust **document engine** owns the project. Every change is a serialisable `Command`; undo/redo step between immutable document versions that share unchanged pieces. | One place enforces "never silently alter geometry"; undo works the same for 2D, 3D and import steps; a native shell reuses it unchanged. |
| Threads | 2D engine on the **main thread** (synchronous calls, < 16 ms). Import, export, zip and OPFS writes in an **IO worker** running the same WASM. Simulation and rendering (Phase 3) in a **sim worker** with a separately loaded WASM. | Dragging needs no round-trip; heavy jobs never block the UI; the 3D code costs nothing at cold load. |
| Geometry | `kurbo` Béziers as the exact model. Seam allowance is built per edge on a 0.001 mm flattening, with sewing corner rules, then cleaned with a robust polygon engine (`i_overlay`). | Exact where it matters (the stitching line she edits), boring and robust where it can be (the derived cutting line). Error < 0.001 mm, far below what a printer can show. |
| 2D view | Canvas 2D in the TS host, drawing render data from the engine. | Fast, crisp at any zoom, works everywhere incl. Safari; WebGPU is kept for 3D. |
| UI framework | **React 19 + React Aria Components** on Vite. | See §1.5 — accessible, touch/pen-correct press and long-press handling out of the box; biggest contributor pool. |
| File format | `.soom` = zip of `project.json` (`schemaVersion: 1`, millimetres, y-down) + sidecar files. JSON Schema generated from the Rust types and published in `docs/schema/`. | Open, diffable, documented, and impossible to drift from the code. |
| Hosting | Static files + `_headers` (COOP/COEP) on any host that can set headers. | Cross-origin isolation from day one. **[you]** pick the host (Cloudflare Pages or Netlify are free and support `_headers`). |
| Licence | **[you]** — not chosen. All dependencies picked so far are MIT/Apache/ISC, compatible with any choice. | See [open-questions.md](open-questions.md#licence). |

---

## 1. Repository layout

### 1.1 Rust workspace

```
soom/
├── Cargo.toml              workspace (edition 2024, resolver 3)
├── rust-toolchain.toml     pinned stable + wasm32-unknown-unknown
├── crates/
│   ├── pattern-core/   P1  geometry kernel and piece model
│   ├── project/        P1  project document: schema, migrations, commands, undo, .soom container
│   ├── io/             P1  SVG + DXF import; tiled PDF + SVG export  (DXF-AAMA export P2)
│   ├── wasm-core/      P1  wasm-bindgen facade for everything 2D (no logic)
│   ├── sizing/         P2  size tables, measurement sets, size nests, blending, alteration stack (P4)
│   ├── pdf-import/     P2  vector extraction, tile reassembly, size lines, piece detection
│   ├── seams/          P3  seam pairs, ease distribution, length reports, auto-suggest
│   ├── body/           P3  ANNY avatar + fitting to measurements
│   ├── sim/            P3  XPBD cloth in WGSL via wgpu
│   ├── render/         P3  wgpu renderer: fabric shading, heatmaps
│   ├── wasm-3d/        P3  wasm-bindgen facade for body + sim + render (lazy-loaded)
│   └── pattern-param/  P5  parametric construction graph
├── tools/
│   └── viewer/         P3  dev-only native wgpu viewer (runs the same `sim`)
├── xtask/              build helpers: wasm build, codegen, size budgets, licence report
└── fixtures/           test patterns, each with a LICENCE note
```

Crates are created when their phase starts, so the tree stays honest.
Package names are prefixed (`soom-pattern-core`, `soom-io`, …) so they never
clash on crates.io or with `std::io`.

| Crate | Owns | Depends on |
|---|---|---|
| `pattern-core` | `Piece`, `Edge`, segments, notches, marks, grainline; arc length; hit testing; seam allowance engine; edge split/join; mirror on fold; slash-and-spread (P4) | `kurbo`, `i_overlay`, `serde` |
| `project` | `Project` aggregate; `Command` enum; `History` (undo); derived-geometry cache; schema versioning + migrations; `.soom` zip read/write | `pattern-core`, later `sizing`, `seams`; `zip`, `serde_json`, `schemars` |
| `io` | Format adapters. Import returns plain pieces + an `ImportReport` (scale confidence, warnings); export takes pieces + options and returns bytes | `pattern-core`; `usvg`, `pdf-writer` |
| `wasm-core` | Exposes `Engine` (project + history) and `Jobs` (import/export) to JS. Type conversion only | `project`, `io`, `wasm-bindgen`, `serde-wasm-bindgen` |

Rules enforced in review and CI:

1. **Only `wasm-*` crates may depend on `wasm-bindgen`/`js-sys`/`web-sys`.**
   `sim`/`render` use `wgpu`, which reaches WebGPU on the web, but they receive
   their surface from the facade (web) or the viewer (native).
2. **Millimetres, `f64`, everywhere.** `f32` only at the GPU and PDF
   boundaries.
3. **Deterministic core.** No clocks or randomness inside crates; time and IDs
   are passed in. The same inputs always produce the same bytes. This is what
   makes golden-file tests possible.
4. **No panics across the boundary.** Every public operation returns
   `Result<_, SoomError>` with a plain-language message and a machine code.

### 1.2 TypeScript workspace (pnpm)

```
soom/
├── package.json            root scripts: dev, build, test, e2e, wasm
├── pnpm-workspace.yaml
├── packages/
│   ├── engine/             @soom/engine — the WASM package (built from crates/wasm-core),
│   │                       generated TS types, typed RPC to the IO worker
│   └── input/              @soom/input — framework-free pointer, gesture, shortcut layer
└── web/                    @soom/web — the PWA
    ├── index.html
    ├── vite.config.ts      COOP/COEP headers in dev and preview; PWA plugin
    ├── public/_headers     COOP/COEP for production hosts
    └── src/
        ├── app/            React shell: layout, toolbar, panels, dialogs (sewing vocabulary)
        ├── canvas/         2D view: viewport maths, Canvas 2D renderer, cutting-mat grid
        ├── tools/          tools as small state machines (select, notch, grainline, measure…)
        ├── storage/        OPFS + IndexedDB, autosave, backup export/import
        ├── platform/       capability checks (WebGPU, OPFS, isolation), screen calibration
        ├── workers/        io.worker.ts (and sim.worker.ts in P3)
        └── strings/        all user-facing text in one place (plain sewing language)
```

Internal packages ship TypeScript source (no build step); Vite and Vitest
consume them directly. Everything that would differ in a native shell
(storage, files, devices) sits behind interfaces in `web/src/platform` and
`web/src/storage`.

### 1.3 Runtime architecture

```
 MAIN THREAD                                   IO WORKER                     SIM WORKER (P3)
 ┌──────────────────────────────────────┐     ┌─────────────────────────┐   ┌──────────────────────┐
 │ React UI (panels, dialogs)           │     │ wasm-core instance #2   │   │ wasm-3d (lazy)       │
 │ Input layer → active tool            │     │  • import SVG/DXF/PDF   │   │  • body, sim, render │
 │   │ preview (transient)              │     │  • export PDF/SVG/DXF   │   │  • OffscreenCanvas   │
 │   ▼                                  │ RPC │  • .soom zip pack/unpack│   │    + WebGPU          │
 │ Engine (wasm-core instance #1)       │◄───►│  • OPFS sync writes     │   └──────────────────────┘
 │   dispatch(Command) → new revision   │     └─────────────────────────┘
 │   derived geometry (cached per piece)│
 │   hit testing                        │
 │   ▼                                  │
 │ Canvas 2D renderer                   │
 └──────────────────────────────────────┘
```

**An edit, end to end:** pointer event → input router (palm rejection,
gesture recognition) → active tool → while dragging, the tool draws a
transient preview and asks the engine for live measurements; on release it
dispatches one `Command` → the engine validates it, creates a new document
version and recomputes derived geometry only for changed pieces → the
renderer redraws → autosave (debounced) sends the serialised project to the
IO worker, which writes it to OPFS.

**Undo** is a list of document versions. Pieces are `Arc`-shared, so a
version costs only the pieces that changed. Commands carry an optional
*coalesce key* so that, say, ten arrow-key nudges become one undo step.
Import confirmations and (later) 3D placement changes are commands too.
That is how undo covers "2D, 3D and import steps".

### 1.4 Build, test, CI

* `pnpm wasm` → `wasm-pack build crates/wasm-core --target web` into
  `packages/engine/pkg`. `pnpm dev` serves with COOP/COEP.
* CI (GitHub Actions, Linux runners): `cargo fmt --check`, `clippy -D
  warnings`, `cargo test` (incl. property and golden tests), wasm build + size
  budget, `tsc --noEmit`, ESLint, Vitest, Playwright on Chromium and WebKit
  (WebKit with iPad touch emulation), licence check (`cargo-deny` + pnpm
  licence allow-list).
* Generated TypeScript types (`ts-rs`) and the JSON Schema (`schemars`) are
  committed; CI fails if they are stale.

### 1.5 React or Svelte → React

Both would work. The deciding factors for *this* app:

1. **Touch and Pencil correctness.** React Aria's press, long-press and
   focus handling already behaves correctly for mouse, touch and pen on iPad
   Safari (no ghost clicks, no hover-only states, proper long-press menus).
   That covers "no hover-only affordances", 44 pt targets and
   keyboard-navigable UI without us rebuilding it.
2. **Accessibility** comes with the components (ARIA patterns, focus
   management, reduced motion), not as an afterthought.
3. **Contributors.** For an open-source project, React has the largest pool
   of people who can help.
4. Svelte's advantages (smaller runtime, fine-grained updates) matter little
   here: the hot paths (canvas drawing, pointer moves) bypass the framework
   entirely, and React's runtime fits easily in the 3 s cold-load budget.

### 1.6 Dependencies and licences

Every dependency and asset is recorded in [licences.md](licences.md),
regenerated by `cargo xtask licences`. CI rejects anything outside the
allow-list (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode, CC0; MPL-2.0 by
exception). Nothing GPL is used, so every licence option stays open.

---

## 2. Project file format

Full specification with field-by-field rules: [file-format.md](file-format.md).
In short:

* A `.soom` file is a zip: `project.json`, `sources/` (the original imported
  files, so a re-import is always possible), `thumbnails/`, and later
  `meshes/` and `textures/`.
* `project.json` has `format: "soom-project"` and an integer
  `schemaVersion`. Any change to the schema bumps it. The app migrates older
  files step by step and refuses newer ones with a clear message. Third-party
  data lives under `extensions` and is preserved untouched.
* Units are **millimetres** (`f64`); coordinates are x → right, y ↓ down (as
  in SVG). Outlines run **clockwise as seen on screen**.
* A **piece** has an outline made of **edges** (corner to corner). Each edge
  is a run of straight or cubic Bézier segments and carries what sewing
  attaches to an edge: its kind (`seam`, `hem`, `fold`), an allowance
  override, the corner style at its end, and its notches.
* Seam allowance is stored as *intent* (widths, corner styles, mode `add` or
  `included`), never as baked geometry. The cutting line is always derived.
* Sizing (size nests and blending), the alteration stack, seams,
  measurements and fabrics are specified as drafts now and become
  normative in the phase that implements them. Each phase that changes the
  schema ships a migration and a golden fixture of the previous version.

A minimal Phase 1 piece:

```json
{
  "id": "p1",
  "name": "Skirt front",
  "outline": {
    "start": [0, 0],
    "edges": [
      { "id": "e1", "name": "Waist",        "segments": [{ "c1": [80, 4], "c2": [160, 8], "to": [240, 12] }] },
      { "id": "e2", "name": "Side seam",    "segments": [{ "to": [300, 600] }] },
      { "id": "e3", "name": "Hem",          "kind": "hem", "segments": [{ "c1": [200, 610], "c2": [100, 615], "to": [0, 615] }] },
      { "id": "e4", "name": "Centre front", "kind": "fold", "segments": [{ "to": [0, 0] }] }
    ]
  },
  "allowance": { "mode": "add", "seam": 15, "hem": 40 },
  "cut": [{ "material": "main", "quantity": 1, "onFold": true }],
  "grainline": { "from": [120, 150], "to": [120, 450] },
  "notches": [{ "id": "n1", "edge": "e2", "at": 180, "from": "start", "kind": "single" }]
}
```

---

## 3. Input-layer design

### 3.1 Principles

* **One input path.** Everything arrives as Pointer Events on the canvas
  (`touch-action: none`) plus keyboard events on the document. No separate
  mouse/touch code paths.
* **Pencil edits, fingers navigate.** Pen pointers draw and edit. Touch
  pointers pan, zoom, select and run gestures. A mouse acts like a pen plus
  scroll-wheel navigation.
* **Every action three ways:** toolbar button, keyboard shortcut, and gesture
  or long-press menu. No action exists only behind hover.

### 3.2 Pipeline

```
PointerEvent / KeyboardEvent / Safari GestureEvent
  └► InputRouter            tracks pointers by id and type; palm rejection;
       │                    coalesced events for smooth pen strokes
       ├► GestureRecognizers  (touch)  pan, pinch, two-finger tap, three-finger tap,
       │                               long-press, quick-pinch-to-fit
       ├► ShortcutMap         (keys)   platform-aware (⌘ on Apple, Ctrl elsewhere)
       └► ToolEvents          (pen, mouse, and finger when allowed)
             └► Active tool (state machine) ──► preview  (transient, never in undo)
                                           └──► Command  (to engine; one undo step)
```

`@soom/input` contains the router, recognisers and shortcut map with **no
DOM rendering and no React**. It is unit-tested by replaying recorded
pointer-event sequences (including real iPad traces we capture).

### 3.3 Rules

| Situation | Behaviour |
|---|---|
| Pen goes down | All touch pointers are cancelled and new touches are ignored until 300 ms after pen-up (palm rejection). Touches with a large contact area are always ignored while a pen has been seen recently. |
| Finger editing | Until a Pencil is first seen, one finger can drag handles (iPads without Pencil, phones). After the first pen event, fingers only navigate and select, as in Procreate. A setting can switch it back. |
| Two-finger tap | Undo. Three-finger tap: redo. Both fingers must lift within 250 ms with less than 10 pt of movement, otherwise it is a pan or pinch. |
| Pinch / two-finger drag | Zoom about the gesture centre / pan. Trackpad pinch (`ctrl+wheel`, Safari `gesturechange`) maps to the same zoom. |
| Long-press (500 ms, < 8 pt movement) or right-click | Context menu for whatever is under the pointer (edge, corner, notch, piece, empty table). |
| Pencil hold on an edge (350 ms) | Shows the edge length and, later, the matching seam's length difference. Selecting an edge shows the same inline. Hover adds it on laptops but is never required. |
| Hit targets | Hit radius is in screen space: pen 10 pt, touch 22 pt (44 pt target), mouse 6 px. Curve handles get the touch radius whatever the pointer. The engine does the hit test in millimetres after converting the radius by the current zoom. |

### 3.4 Gesture and shortcut map (Phase 1)

| Action | Laptop | iPad |
|---|---|---|
| Pan | Space+drag, middle-drag, two-finger scroll | Two-finger drag |
| Zoom | ⌘/Ctrl + scroll, trackpad pinch, `+` / `−` | Pinch |
| Fit all / actual size (100 %) | `0` / `1` | Quick pinch / toolbar |
| Undo / redo | ⌘Z / ⇧⌘Z (Ctrl+Z / Ctrl+Y) | Two-finger tap / three-finger tap; toolbar buttons |
| Select | Click | Tap (finger or Pencil) |
| Move a piece, edit a point | Drag | Pencil drag (finger drag before a Pencil is seen) |
| Context menu | Right-click | Long-press |
| Tools | `V` select, `N` notch, `G` grainline, `M` measure, `A` allowance | Toolbar |
| Delete | ⌫ | Context menu |

### 3.5 Tools are state machines

Each tool implements `down / move / up / cancel / key` and returns either
nothing, a preview, or a command. They never mutate the document. That makes
every tool unit-testable without a browser, and means a cancelled gesture
(palm, incoming call, rotation) can never leave half an edit behind.

### 3.6 True scale on screen

Browsers do not report physical pixel density, so "100 %" needs a one-time
calibration per device: hold a bank card (85.60 × 53.98 mm, a size every
card shares) or a ruler against the screen and drag until it matches. The
result is stored per device. Until calibrated, the zoom label says "≈100 %
(not calibrated)". The same calibration later drives projector mode.

---

## 4. Phase 1 plan — two weeks

Goal: *open an SVG or DXF, see it at true scale, add and edit seam
allowance, mark grainline and notches, and print a tiled PDF that measures
exactly right.*

### Week 1 — the kernel and the first visible pattern

| Day | Tasks | Tests | Demo |
|---|---|---|---|
| 1 | Repo, workspaces, toolchains, CI; Vite + React shell served with COOP/COEP; PWA manifest + service worker; licence ledger | CI green; `crossOriginIsolated === true` asserted in Playwright | App opens offline and reports isolation |
| 2 | `pattern-core`: segments, edges, pieces; orientation; arc length and point-at-distance; nearest point; bounds; validation | Property tests: arc length additivity, split/join round-trip, orientation invariants | — |
| 3 | Seam allowance engine: per-edge widths, `hem` and `fold` kinds, corners (pointed, squared, fold-back mirror), self-intersection clean-up, `included` mode | Property tests on random pieces: cutting line encloses the stitching line, distance = allowance ± 0.001 mm away from corners, no self-intersections; golden SVG snapshots of hard cases | — |
| 4 | SVG import: units and scale detection with a confidence level; closed paths → pieces; automatic corner detection → edges; open paths → internal lines / grainline candidates; provenance | Golden tests: Inkscape (mm), Illustrator (pt-as-px), unitless, FreeSewing output, transforms and `<use>` | — |
| 5 | `project`: schema v1, commands, history, migrations scaffold, `.soom` container; `wasm-core` facade; `@soom/engine`; canvas with cutting-mat grid; open SVG → pieces on screen; pan/zoom with mouse and trackpad | Round-trip golden tests for `project.json`; engine command tests | **Open an SVG and see the pieces** |

### Week 2 — sewing marks, true scale, printing

| Day | Tasks | Tests | Demo |
|---|---|---|---|
| 6 | `@soom/input`: router, palm rejection, pinch/pan, two/three-finger tap undo/redo, long-press; select/move tool; inspector panel; undo/redo buttons | Recogniser tests from recorded event sequences; tool state-machine tests | Navigate with fingers on iPad; undo with two fingers |
| 7 | Seam allowance UI: piece default, per-edge override, edge kind (seam/hem/fold), corner chooser with pictures; split edge here / join edges; inline edge lengths | Command tests; derived-geometry snapshots | **Add and edit seam allowance** |
| 8 | Grainline tool, notch tool (single/double/triple), cut instructions, piece name labels; DXF import (generic DXF and AAMA blocks/layers, `$INSUNITS` and AAMA units) | DXF golden tests incl. AAMA layers 1/2/4/7/8/11/14 | **Mark grainline and notches; open a DXF** |
| 9 | Tiled PDF export: automatic layout, A4 / Letter / A0, margins, trim or overlap assembly, page grid labels, assembly marks, page map, 10 cm + 4 in test square, `PrintScaling=None`; screen calibration UI for true scale | PDF parsed back in tests: page sizes, test square = 100.000 mm, a known edge has its exact length, every piece is fully covered by pages | **Print a tiled PDF and measure the test square** |
| 10 | OPFS autosave + IndexedDB project list; one-tap backup (`.soom` download) and restore; offline check; Playwright end-to-end (open SVG → allowance → notches → export → verify PDF); performance check; demo notes | E2E on Chromium + WebKit; 2D edit latency measured in CI | Full Phase 1 walk-through |

**Phase 1 is done when** the seamstress can open an SVG or DXF of one of her
pieces, add or adjust the allowance, mark grain and notches, print on A4 or
Letter, and the test square and a known seam measure exactly right with a
ruler.

**Not in Phase 1** (deliberately): editing curve shapes beyond splitting and
joining edges, PDF import, sizes, seams between pieces, 3D.

---

## 5. Risks and research-grade items

| Item | Risk | Approach | Fallback |
|---|---|---|---|
| Seam allowance on tight curves and sharp corners (P1) | Offset loops, spikes, near-degenerate corners | Fine flattening + robust polygon clean-up; mitre limit; property tests over random shapes | Per-corner manual style override; round join for anything the rules cannot resolve, flagged to the user |
| SVG scale (P1) | Files without physical units (Illustrator writes points as pixels) | Detect units; use paper-size heuristics; show confidence; never import silently when unsure | "Measure a known line" calibration at import |
| DXF variants (P1) | AAMA files differ by CAD vendor (units, block layout) | Lenient reader; report what was ignored; fixtures from several exporters | Generic "closed polylines become pieces" mode |
| Printing at 100 % (P1) | Viewers default to "fit to page", iOS especially | `PrintScaling=None`, test square on page 1, plain guidance at export | A0 / copy-shop output; projector mode (P4) |
| iPad storage (P1) | Safari can evict site data | `navigator.storage.persist()`, nudge to Add to Home Screen, backup button always visible | Backup reminders after N edits without export |
| **PDF piece detection (P2)** | Research-grade | Vector-first; every detection is a suggestion she confirms | Manual "trace this outline" with snapping to the PDF's own vectors |
| **Matching size lines across sizes (P2)** | Corners must correspond between sizes to blend | Match by order along the outline + corner angle | She taps corresponding corners |
| **Body fitting to measurements (P3)** | Residuals on unusual figures | Optimise ANNY shape parameters; report per-measurement residual | Show residuals; allow manual sliders |
| **Sim at 20k particles on iPad (P3)** | GPU budget, Safari WebGPU maturity | XPBD with graph colouring; adaptive resolution | Lower resolution; static drape preview |

---

## 6. What I need from you

1. **Licence** — MIT/Apache-2.0, MPL-2.0, or GPLv3. Trade-offs in
   [open-questions.md](open-questions.md#licence).
2. **The garments for the acceptance test** (the `[FILL IN]`), and a few of
   her real pattern files to use as private fixtures. They stay out of the
   public repo unless their licence allows.
3. **Production host** for the PWA (Cloudflare Pages / Netlify / other) when
   we want her to try it on her iPad. Until then I can serve it on your
   laptop.
4. The sewing questions in [open-questions.md](open-questions.md). None of
   them block Phase 1. Each has a provisional default that is easy to change.
