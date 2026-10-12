# Phases 3 and 4 — see it on her, fit and edit

Version 0.2 · 2026-10-12 · status: **done**, but for step 23 (the WebGPU
drape), held back until the iPad shows it is needed. What each step does,
and what it doesn't do yet, is in [demo-notes.md](demo-notes.md); the
sewing choices waiting for the seamstress are in
[open-questions.md](open-questions.md) (18 to 29).

**Phase 3, see it on her:** enter the wearer's measurements and see an
avatar with that figure; pair the pattern's seams (Drapewright suggests,
the seamstress confirms); drape the pattern on the avatar in a chosen
fabric; see where it pulls and where it stands away.

**Phase 4, fit and edit:** alter the pattern (lengthen or shorten, full or
small bust, swayback, shoulders, waist and hip, slash and spread, true up
seams), each alteration a step that can be switched off. Every 2D edit re-drapes
live. Calibrate a fabric's stretch, and project the pattern at true size
onto fabric.

---

## Decisions

| Area | Decision | Why |
|---|---|---|
| Body | **Anny** (NAVER LABS Europe, code Apache-2.0) on MakeHuman's body data (via MPFB2, **CC0**). Only its default topology; never its non-commercial `smplx` topology, never SMPL. | Freely licensed, lifelike for any age and figure, and it carries MakeHuman's *measure* shapes (bust, underbust, waist, hips, shoulders, nape to waist, arm and leg lengths…), made for fitting measurements. |
| Body data | A tool in the repo reads a local copy of Anny's data and writes a compact body file: the base mesh, only the shapes used, body-part labels. The file is committed with its licence note; the app loads it when 3D is first opened and keeps it for offline use. | Reproducible, small, no network at run time. |
| Fitting | Overall shape from a few settings (woman or man, age, weight, muscle, height, proportions, cup size), then the measure shapes solved by least squares to the measurements. Each measurement shows how close the avatar comes. | Fast, explainable, and honest about what the avatar can't match. |
| Measurements | A vocabulary of measurements, each with how-to-measure text. Measurement sets are kept in the project (schema v2, with a migration), and FreeSewing names are mapped. | The data the avatar, ease checks and alterations share. |
| Seams | A `seams` crate. Seams pair edges or parts of edges, including a piece with its mirror copy (left and right). It gives lengths, the difference between the two sides and how the notches line up, and suggests pairs by name, length, notch type and front or back. Each pair is confirmed by hand. | "Walking the seams" is useful on its own in 2D, and the drape needs it. |
| Cloth | A `sim` crate: XPBD cloth with triangle strain and bending. Fabrics stretch differently along the grain, across it and on the bias. Seams are stitches that close in the first moments, and the body is a distance field to collide with. A CPU version comes first (deterministic and tested); WebGPU compute follows, with the same model. | Stable at large steps; strain comes straight out of it for the heatmap. |
| 3D view | A `render` crate on wgpu (WebGPU, with WebGL2 drawing as the fallback), running in the sim worker on an OffscreenCanvas. Orbit uses the same input layer as the 2D view. | As in the proposal. Her iPad runs iPadOS 26, so it has WebGPU. |
| Fabrics | Presets (cotton lawn, poplin, linen, chambray, denim, twill, crepe, satin, jersey, rib knit, wool suiting, fleece) with weight, stretch along and across the grain, and stiffness. Phase 4 adds calibrating a real fabric. | A good start without measuring anything. |
| Alterations | Operations on the stitching line, in `pattern-core`. The stack lives in the project and is applied in order to the kept base outline; each step can be switched off; truing blends the curves. | Nothing destructive; geometry never changes silently. |
| Projector | A full-screen true-scale view, calibrated by measuring a projected square on the table, with mirroring, high contrast and thick lines. | Cutting straight from the fabric. |

---

## Steps

Each step is one or a few commits, with tests and a demo note in
[demo-notes.md](demo-notes.md).

### Phase 3

| Step | What | Tests | Demo |
|---|---|---|---|
| 17 | **Measurements.** Vocabulary with how-to-measure text; measurement sets in the project (schema v2 with migration); a form in cm or inches; FreeSewing names mapped. | Schema golden files and migration; vocabulary complete; unit conversion | Enter measurements and keep them with the project |
| 18 | **Seams.** Pair edges (and mirror copies); lengths and mismatch; notches matched; suggestions to confirm; shown on the 2D view. | Pairing on FreeSewing fixtures; suggestion precision on the test patterns | Tap a seam and see both sides' lengths |
| 19 | **Avatar.** Body data tool; the `body` crate (shape, measure, fit); how close each measurement comes. | Measurements on the mesh against known values; fitting reaches its targets on sample figures | A report of the fitted avatar's measurements |
| 20 | **3D view.** `render` and `wasm-3d` in the sim worker; see the avatar and turn it. | Renderer smoke test (WebGL2 in CI); worker protocol | See the avatar |
| 21 | **Drape.** Meshing, placing pieces round the body, stitching seams, the CPU cloth, fabric presets. | Sim invariants (no stretch beyond limits, seams closed, nothing inside the body); a drape that settles | Drape a skirt and a blouse on the avatar |
| 22 | **Heatmaps.** Strain (pulling) and ease (room between cloth and body), with numbers where tapped. | Strain on known stretched cases; ease against the distance field | See where it pulls |
| 23 | **WebGPU drape.** The same cloth in compute shaders, for speed. Moved to just before step 28: the CPU drape already takes a second or two for a skirt or a blouse in the browser, and it is live re-draping that will need more speed. | CPU and GPU agree within tolerance | Drape in a second or two |

### Phase 4

| Step | What |
|---|---|
| 24 | Alteration stack; lengthen and shorten along a line |
| 25 | Full and small bust adjustment; slash and spread; pivot |
| 26 | Swayback; broad and narrow shoulders; waist and hip |
| 27 | True up seams (walk and blend) |
| (23) | WebGPU drape, if live re-draping needs the speed |
| 28 | Live re-drape after each 2D edit |
| 29 | Fabric calibration |
| 30 | Projector mode |

---

## Risks

| Item | Risk | Approach | Fallback |
|---|---|---|---|
| Fitting | Unusual figures leave residuals | Least squares over the measure shapes; show each residual | Manual sliders for the shapes |
| Placing pieces | Pieces must start round the right part of the body | Fronts, backs and sleeves placed by name and seams; they can be dragged on the avatar | Each piece placed by hand |
| Cloth speed on iPad | Too slow to feel live | Coarser mesh while editing, finer at rest; WebGPU compute | Drape on request, not live |
| Body file size | Too many shapes to load quickly | Only the shapes used, quantised, loaded with 3D | Fewer age and proportion shapes |

---

## Backlog (not scheduled)

* **Plotter PDFs: lines drawn dash by dash.** Dashed lines come in as many
  tiny lines, and a word crossed by a dash-dot line reads as nonsense
  (step 16). Waiting for more patterns that need it.
