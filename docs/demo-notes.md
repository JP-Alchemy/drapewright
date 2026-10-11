# Demo notes

What you can try after each step. Newest first.

To run the app locally:

```bash
pnpm install
pnpm wasm
pnpm dev        # then open http://localhost:5173
```

---

## Step 10 — Projects that stay put: autosave, backup, offline (Phase 1, day 10)

**Try it:**

1. Open a pattern and change something. Next to the project name (top
   left), **Saving…** turns into **Saved** a moment later. Close the tab or
   reload: the same project opens, where you left it.
2. **Tap the project name** to see **Projects**:
   * Rename the project.
   * Start a **New project**.
   * Open another project from the list.
   * **Delete** one. You are asked first, and the project that is open
     cannot be deleted.
3. **Back up** (top bar) saves a `.drapewright` file holding the project and the
   original SVG/DXF its pieces came from. Keep it in Files or a cloud
   drive.
4. **Restore a backup…** (in Projects), or just *Open pattern* and pick a
   `.drapewright`. If that project is already on the device, the backup opens as
   a copy named "… (restored)". Nothing is overwritten.
5. **Offline:** after one visit the app loads and works with no internet,
   import and printing included. Nothing is ever uploaded.
6. **This device** (ⓘ) now says whether the browser has promised to keep
   saved projects ("Kept safe"). Safari on iPad does this for apps added
   to the Home Screen; otherwise, back up the projects you care about.

**Where it is kept:** each project is a folder in the browser's private
file system (OPFS), with a list in IndexedDB. Where OPFS cannot be written,
as in Safari's private browsing, the same files go into IndexedDB. Each
save writes a complete copy before replacing the old one, so a crash
mid-save never loses the project.

**The format is published:** `docs/schema/project.schema.json` is generated
from the same Rust types that read and write projects. A test checks real
saved projects against it, and CI fails if it changes without being
committed.

**Checked on every push:**
* A walk-through test: open a pattern, change the allowance, mark a hem, a
  notch and a grainline, print the PDF, reload, and check everything is
  still there.
* An offline reload.
* Edit speed: changing a piece's allowance and redrawing its lines takes a
  median of 0.1 ms (worst under 1 ms) on Chromium and WebKit. The budget is
  one frame, 16 ms.
* The engine is now shrunk with `wasm-opt`: 1.8 MB (690 KB compressed),
  down from 2.5 MB.

**Not yet:** a thumbnail picture for each project in the list. Nudges to
back up after many edits come later, with the Home Screen install prompt.

---

## Step 9 — Printing at true size, and measuring the screen (Phase 1, day 9)

**Try it:** open `fixtures/dxf/aama-skirt.dxf` and
`fixtures/svg/flared-skirt.svg`, then press **Print** (top bar).

1. **The print dialog** shows the pages as they will come out of the
   printer, with the pieces laid out on them and the page letters (A1, A2…
   B1…), plus the page count.
   * Choose **A4, US Letter, A3, Tabloid or A0** (copy shop) and portrait
     or landscape. The preview and count update as you go.
   * Choose how pages join: **Trim and butt** (cut the margin off two sides
     and lay pages edge to edge) or **Overlap** (lay pages over each other
     by a strip, 1.5 cm by default).
   * You can also set the margin, the notch style (slit or V), whether the
     dashed stitching line prints, the test square, the page map, and
     whether empty pages are left out.
2. **Save PDF.** The file is made on your device and nothing is uploaded.
   * Page 1 has the instructions and two test squares, **10 cm** and
     **4 in**, plus a map of the pages.
   * Every other page has a frame to trim along and its letter in the
     margin, in faint large type and next to each neighbour's edge.
   * Trimmed pages have half-diamonds that make a whole diamond when butted
     together. Overlapping pages have a dashed line where the next page's
     edge goes and circles to line up.
   * Pieces have their cutting line (solid), stitching line (dashed),
     notches, grainline, "Place on fold" brackets, drill holes and other
     marks, plus their name and "Cut 1 · Main fabric".
3. **Print it at 100%** ("Actual size"). The PDF asks readers not to
   scale it and to print one-sided, but check the print dialog anyway.
   **Measure the test square: it must be 10 cm.** Print settings are part
   of the project and can be undone.
4. **Measure your screen:** the ruler button (bottom left, with an orange
   dot until it is done). Hold a bank card against the screen and slide
   until the outline matches it, or check the 5 cm line with a ruler.
   After that, **100%** shows pieces at their real size on this device. It
   is remembered on this device only and is not part of any project.

Drill holes and lines inside pieces now show on the table too, as they
print.

**How we know it is exact:** the tests read the PDF back and measure it:
* Every page has the exact paper size, and print scaling is off.
* The test squares measure 100.000 mm and 101.600 mm.
* Known edges keep their lengths to 0.001 mm: 200 and 300 mm stitching
  lines, and 230 and 330 mm cutting lines.
* Every point of every cutting line falls on a printed page.
* The same project always gives the same file.

**Not yet:**
* Pieces keep their table rotation and are laid out on shelves, without
  rotating or nesting to save paper. Moving pieces on the print sheet by
  hand comes later; the file format already holds such a layout.
* A piece's name can be split across two pages.
* The PDF uses the standard Helvetica font, which has Western European
  letters only. The dialog lists any name that would print with "?".
* New projects start on A4. Choosing Letter is one tap and is saved with
  the project.

---

## Step 8b — DXF import, and asking about seam allowance (Phase 1, day 8)

**Try it:** *Open pattern* now takes `.dxf` as well as `.svg`.

1. Open `fixtures/dxf/aama-skirt.dxf`, a garment CAD (AAMA/ASTM) file.
   * **Size:** the file says *metric*, which nearly always means
     millimetres, but not always, so you confirm it. Each reading shows how
     big the skirt front would be.
   * **Skirt front:** the file has both its cutting line and its stitching
     line, so Drapewright keeps the stitching line as the outline and measures the
     allowance edge by edge: 1.5 cm seams, 4 cm hem, and nothing on the
     centre front, which the file marks as a fold. The notch, grainline,
     drill hole, "Quantity: 1" and "Size: 38" come across too.
   * **Waistband:** only its cutting line is in the file. That is what
     garment CAD files mean by their cutting lines, so *Already included*
     is chosen for you. Set the width it should show (try 1 cm). The
     cutting line is kept exactly as drawn, with the stitching line dashed
     inside it.
2. Open `fixtures/svg/inkscape-mm.svg` (or any SVG with one outline per
   piece). The new question **"Is seam allowance included in these
   pieces?"** has **nothing chosen**, and *Add* waits until you answer:
   * *Not included*: the lines are stitching lines, and the allowance is
     added around them (as before).
   * *Already included*: the lines are cutting lines, kept as they are.

   Getting this wrong makes a garment too big or too small by two
   allowances at every seam, and the file cannot say which it is, so Drapewright
   asks rather than guessing. Files with both lines (like
   `fixtures/svg/cut-and-seam.svg`) are not asked about: their allowance is
   measured.
3. Open `fixtures/dxf/generic-mm.dxf`, an ordinary CAD drawing. The pocket's
   rounded corners stay curved, within 0.001 mm of true arcs. The flap was drawn as four separate
   lines and still becomes one piece, named by the text inside it. The
   hatch Drapewright cannot read yet is reported, not silently dropped.

**What it reads:**
* ASCII DXF from any version: lines, polylines (with bulges), arcs,
  circles, splines, points, text and blocks.
* Units come from `$INSUNITS` when the drawing has them, and from the AAMA
  units text otherwise. With neither, you are asked.
* Curves stay curves. Splines of degree 3 or less become exact Béziers,
  and arcs are within 0.001 mm. A spline saved only as the points it passes
  through is joined with straight lines, and the import says so.
* **Not yet:** binary DXF (you are asked to save it as ASCII), ellipses,
  hatches and 3D entities (reported).

**Research-grade, with a fallback:**
* AAMA/ASTM files from real CAD systems vary between vendors. The reader
  follows the published layer numbers and is tested only on files drawn for
  these tests. The fallback is that anything it cannot place is reported,
  and a file that is not recognised as AAMA is read as an ordinary drawing.
  A few real exports would show what needs adjusting (open question 17).

**Found by testing:** measuring the allowance at the nearest point of the
cutting line gave a 4 cm hem as 1.5 cm. Near the fold corner, the fold's
cutting line is closer than the hem's. The allowance is now measured
straight out from the stitching line, as you would with a ruler.

---

## Step 8a — Notches, grainline and cutting instructions (Phase 1, day 8)

**Try it** with `fixtures/svg/flared-skirt.svg`:

* **Toolbar** (top left): *Select* (V), *Notches* (N), *Grainline* (G), with
  a one-line hint for the active tool.
* **Notches:** with the notch tool, tap an edge. A notch appears exactly
  there, drawn as a slit into the allowance with a faint line across it. It
  is measured from the nearer corner, so it stays put if the far end
  changes. **Tap the notch** (on the slit itself, out at the cutting line)
  to cycle single → double → triple. **Drag it** to slide it along the
  edge; the distance from the corner shows as you go, and the slide is one
  undo step. The panel shows the type and the exact distance, which corner
  it is measured from, and *Delete notch*. Right-click an edge for *Add a
  notch here*.
* **Grainline:** with the grainline tool, drag inside a piece. It snaps to
  15° steps of the piece's own axes; hold Shift for any angle. Or use
  *Add a straight grainline* in the panel, and switch on *One way* for
  napped fabric (single arrowhead).
* **Cutting:** *How many* and which fabric; *As mirrored pairs* appears from
  2 up, *On the fold* when the piece has a fold edge; *Also cut in another
  fabric* adds lining or interfacing. The first line shows on the piece
  ("Cut 1 on the fold · Main fabric").

**Found by testing:** you could only pick a notch at its point on the
stitching line, 1.5 cm inside the slit you can see. Now the whole mark
(slit and line) can be tapped.

---

## Step 7 — Seam allowance, edge by edge and corner by corner (Phase 1, day 7)

**Try it:** open `fixtures/svg/flared-skirt.svg` (new: a panel flaring 20°
with a straight hem).

1. **Tap the bottom edge.** The panel now has an *Edge* section: its length
   along the stitching line (also shown on the table next to the edge), a
   name, and **This edge is a Seam / Hem / Fold**. Choose **Hem**: the hem
   gets the piece's hem allowance, and the corners next to it fold back.
2. Under *Piece*, set **Hem allowance** to 4 cm. Watch the bottom-right
   corner: the hem allowance angles back in, exactly as it must to lie flat
   when turned up along a flared side seam.
3. **Tap that corner** (the point where the side seam meets the hem). The
   *Corner* section shows **six live pictures of this very corner**:
   automatic, pointed, squared and folded back, on either side. Each one is
   computed from the real piece, so what you see is what you will cut. Where
   two shapes come out the same (common at right angles) it says so; where
   one is impossible it says that too. **These pictures are what to show the
   seamstress for open questions 3 and 4.**
4. Tap the left edge and choose **Fold**: no allowance, and the cutting line
   meets the fold squarely. Try **Fold** on a curved edge (the hem of
   `inkscape-mm.svg`): Drapewright refuses and explains that a fold must be
   straight.
5. Any edge can have **its own allowance**; *Use the piece's allowance*
   puts it back.
6. Right-click (long-press) an edge for **Add a corner here**, a corner for
   **Remove this corner**, and either for changing the edge type.
7. **Allowance is already included**: for patterns printed with allowance,
   the outline becomes the cutting line and the dashed stitching line is
   worked out inside it.

Undo (⌘Z, two-finger tap) covers all of it; stepping an allowance up and
down is a single undo step.

**Fixed along the way** (found by the new end-to-end tests): ⌘Z did nothing
right after picking a corner shape, because focus on a radio button counted
as typing; and tapping an edge of a piece that was part of a group selection
kept the whole group selected, hiding the edge's settings.

---

## Step 6 — Input layer: Pencil, fingers, mouse and keys (Phase 1, day 6)

**Try it** (`pnpm dev`, open a fixture as in step 5):

* **Move pieces:** drag a piece. While you drag it follows the pointer; on
  release it is one undo step ("Undo move"). Arrow keys nudge the selection
  by 1 mm, Shift+arrow by 1 cm; a run of nudges undoes in one go.
* **Select:** tap or click a piece; Shift-click adds to the selection. Tap
  an *edge* (on the dashed stitching line) and it lights up; the panel shows
  its length along the stitching line.
* **Inspector:** rename the piece, and step the seam allowance up and down.
  The cutting line follows immediately. Stepping is one undo step.
* **Menu:** right-click (or **long-press** on iPad) a piece for *Turn 90°*
  and *Delete piece*; on the empty table for *Open pattern* and *Show all*.
* **Undo/redo gestures on iPad:** two-finger tap undoes, three-finger tap
  redoes, with a short "Undo move" confirmation at the top.
* **Navigate:** two fingers pan and pinch; space + drag pans with a mouse.
* **Keys:** ⌘Z / ⇧⌘Z (Ctrl+Z / Ctrl+Y), ⌫ delete, 0 show all, 1 actual
  size, + and − zoom, Esc clear selection, ⌘A select all.
* **Pencil:** once the Pencil has been used, fingers only navigate and
  select (palm rejection), as in Procreate. Holding the Pencil still on an
  edge shows its length.

**Worth trying on a real iPad** (please report back): palm resting on the
glass while drawing with the Pencil; two-finger tap undo; long-press menu.
These are unit-tested with recorded event sequences, but real hardware is
the real test.

**Under the hood:** `@drapewright/input` is framework-free (it does not even see
the DOM). It turns raw pen, touch and mouse events into pan/zoom, undo/redo,
context menus and a single stream of tool events, with palm rejection and
gesture recognition, all unit-tested by replaying event sequences against a
fake clock. Tools are small state machines that only ever preview or
dispatch commands, so a cancelled gesture can never leave half an edit.

**Fixed along the way:** if the app starts while its window is hidden, the
browser gives it no size, and "show all" used to zoom out to 2%. The canvas
is now measured when it appears, and "show all" waits until the table has a
size.

---

## Step 5 — Open an SVG and see it on the table (Phase 1, day 5)

**Try it:** `pnpm wasm && pnpm dev`, open http://localhost:5173.

1. **Open pattern** and pick `fixtures/svg/inkscape-mm.svg`. Drapewright reads it in
   the background and tells you what it found: two pieces, plus a small
   shape and a stray line it left out. Choose **Add 2 pieces**.
2. The pieces lie on a green cutting mat with a real 1 cm grid, bolder every
   5 cm. Each piece is drawn as pattern paper: solid **cutting line**,
   dashed **stitching line** 1.5 cm inside (the provisional default
   allowance), the **grainline** arrow, and the name.
3. Open `fixtures/svg/illustrator-pt.svg`. This file does not say how big it
   is, so Drapewright **asks**, showing what each reading would mean ("Pocket would
   be 10.0 cm × 5.0 cm"). Pick one; new pieces land beside the existing ones.
4. `fixtures/svg/cut-and-seam.svg` has both lines; Drapewright measures the
   allowance from the file (1.5 cm, with a 3 cm hem) instead of adding its
   own.
5. Navigate: two-finger scroll pans, pinch (or ⌘/Ctrl + scroll) zooms
   around the pointer, drag the empty mat to pan. The zoom buttons are
   bottom left; the percentage button jumps to **actual size**. It is
   approximate until the screen is measured in step 9.
6. Click a piece (on the table or in the list) to select it. **Undo/Redo**
   (toolbar, ⌘Z / ⇧⌘Z) step back and forth through what you added.
7. Switch your system between light and dark: the app and the table follow.

**Not yet:** nothing is saved between reloads (step 10), pieces cannot be
moved or edited yet (steps 6–8), and touch gestures on iPad come with the
input layer (step 6).

**Under the hood:** the `project` crate is the document engine. Every change
is a command, the engine refuses any edit that would leave a piece broken,
and unlimited undo shares unchanged pieces between versions. Random
editing sessions (property test) never produce duplicate ids or broken
pieces, and undoing everything restores the start exactly. Imports run in a
worker on a second copy of the WebAssembly engine. Rust types generate the
TypeScript types, so the two sides cannot drift.

---

## Step 4 — SVG import and export (Phase 1, day 4)

Still nothing new in the browser: the importer gets its screen in step 5.

**Try it:** `cargo test -p drapewright-io`. The fixtures in `fixtures/svg/` cover
what real files throw at an importer:

* **Inkscape, in millimetres** (`inkscape-mm.svg`): two pieces found and
  named from their labels ("Skirt front", "Waistband"), a layer transform
  undone, the grainline recognised from its id, a drill hole kept as a
  mark. A notch drawn as a triangle, a title and a stray line are counted
  and reported, never silently dropped.
* **Illustrator** (`illustrator-pt.svg`): sizes in points written as
  pixels. The importer reads it as points (Illustrator's habit) but marks
  the scale **unconfirmed**; choosing another reading rescales everything.
* **No units** (`unitless.svg`): the scale is a guess and must be
  confirmed or measured. Imports never pretend to know.
* **Cutting and stitching lines both present** (`cut-and-seam.svg`): paired
  automatically. The stitching line becomes the outline and the allowance
  is measured edge by edge (15 mm, with a 30 mm hem). The importer then
  checks its own cutting line against the file's and would report any
  difference.
* **Transforms and `<use>`** (`transforms.svg`): rotated and scaled shapes
  come out at the right size.
* **Round trip**: pieces exported to SVG and imported again come back
  within 0.001 mm, with the same allowances.

**Scale rules:** a file is only trusted when its width and height state
physical units. Otherwise the importer suggests 96, 72 or 90 per inch from
clues (Illustrator's generator comment, old Inkscape versions, a drawing
exactly the size of A4 or Letter) and the app will ask.

---

## Step 3 — Seam allowance engine (Phase 1, day 3)

**Try it:** open `crates/pattern-core/tests/gallery/index.html` in your
browser (double-click it in Finder). It shows eleven hard cases, each
with the stitching line dashed and the cutting line solid:

* a half bodice with curved neckline and armhole, 15 mm seams, a 30 mm
  waist hem folded back at the side seam, and centre front on the fold;
* a flared skirt whose 40 mm hem angles back in at the side seam, so it
  lies flat when turned up;
* a trouser leg narrowing to the hem, where the hem allowance flares out
  instead;
* a 20° flare with a 50 mm hem, where the folded-back end is narrower than
  the hem and the corner is trimmed;
* a 10° point cut square at 40 mm, a squared corner, a circle, and an 8 mm
  curve under a 15 mm allowance that the cutting line bridges cleanly;
* a pattern with allowance *included*, with the stitching line derived
  inside it.

These are the pictures to show the seamstress for open questions 3 and 4
(corner shapes).

`cargo test -p drapewright-pattern-core` also runs the exact cases and four
properties over hundreds of random pieces: the cut piece always contains
the whole stitched piece and never crosses itself, away from corners the
cutting line is exactly the allowance away (within 0.005 mm), and so on.

**Speed:** under 1 ms per piece for everything in the gallery (release
build). Curves tighter than the allowance first took 43 ms; they are now
thinned where they cannot affect the result (0.4 ms, identical output).

**How it works:** the allowance region is a union of simple pieces: a
rectangle per flattened segment, pointed joins at kinks, and one polygon
per corner shaped by its corner style. The union uses i_overlay. This
avoids the classic failure of offsetting a curve and repairing its loops.
The trade-off is a cutting line that is a polyline within 0.002 mm of the
true curve.

---

## Step 2 — Geometry kernel (Phase 1, day 2)

Nothing new in the browser yet; this step is the foundation the next ones
draw with.

**Try it:** `cargo test -p drapewright-pattern-core`. Seven property tests run
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
