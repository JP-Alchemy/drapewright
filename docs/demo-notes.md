# Demo notes

What you can try after each step. Newest first.

To run the app locally:

```bash
pnpm install
pnpm wasm
pnpm dev        # then open http://localhost:5173
```

---

## Step 17 — Measurements (Phase 3, part 1)

The first step of Phase 3 ([plan](plan-phase-3-4.md)): people's
measurements, kept with the project. The avatar, the ease checks and the
alterations of the steps to come all start from them.

**Try it:**

1. Press **Measurements** in the top bar, then **Add my measurements**.
2. Fill in what you have; only height, bust, waist and hip are needed to
   start. Each box says how to take that measurement, and boxes are in the
   project's units (cm, mm or inches).
3. Type a waist of 780 while the project is in centimetres: the form says
   it is unusual for a waist and to check the units. Make it 78.
4. Switch the figure to **Man's**: bust becomes chest, and the bust's own
   measurements (high bust, underbust, bust span, shoulder to bust point)
   go away. Values already typed are kept.
5. **Measure again** starts a new measurement dated today; the earlier one
   is kept with the person (shown as "1 earlier measurement kept").
6. **Add a person** for each client. Undo takes back any change. Reload:
   it is all still there, saved with the project.

AI assistants (step 12) get two tools: `get_measurements` and
`set_measurements`.

**What changed in the file:** projects are now schema version 2, with
`measurementSets`. Projects saved before open as before (they have no
measurements); a cached older app refuses version 2 files rather than
dropping the measurements when it saves.

**What's under the hood:** `crates/project/src/measurements.rs` (the 27
measurements, their usual ranges and FreeSewing names, and the sets),
four commands in `command.rs`, the migration in `migrate.rs` with a golden
version 1 file (`crates/project/tests/fixtures/project-v1.json`), and
`web/src/app/MeasurementsDialog.tsx`. The file format is in
[file-format.md §10.2](file-format.md).

**Not yet:** pictures beside the how-to text; importing a FreeSewing or
Seamly2D measurements file.

---

## Step 16 — Reading lettering drawn as strokes (Phase 2, part 4)

PDFs printed through a plotter driver (step 15) draw their words as pen
strokes, so their pieces came in as *Piece 1*, *Piece 2*… The import now
reads that lettering: each piece gets its name and how many to cut from
the label the pattern program printed on it.

**Try it:**

1. The made-up plotter drawing now has names, drawn in a plain hand of its
   own (not the one the importer knows):

   ```bash
   PLOTTED_PDF=plotted.pdf cargo test -p drapewright-io --test pdf_plotter
   ```

   The front comes in as **Left**, cut 4, and the back as **Fly**, cut 2,
   read from lettering turned on its side. The strip, with only a logo on
   it, stays *Piece 3*. The A4 tiles (`PLOTTED_TILES=tiles.pdf`) give the
   same names.
2. On the two real bought patterns (kept out of the repo), every piece is
   named, in the A0 and the A4 files alike:
   * the blouse: Front yoke (cut 4), Sleeve (cut 2), Back, Back neck
     facing, Rouleau button loops (cut 2), Right front, Left front, Cuff
     (cut 4), Right front facing, Left front facing;
   * the trousers: Back leg and Front leg (cut 2 each), Belt loops (5pc),
     Fly shield (cut 1 on the fold, its fold edge marked from the word
     "fold" printed along it), Fly extension (cut 1 in interfacing),
     Waistband, Pocket bag (cut 2).

**How it reads:**
* Letters are strokes that touch, and lines are letters side by side. The
  block is turned whichever of the four ways reads best, and each line
  gets its own baseline, so lettering a hair off straight still reads.
* Each letter is compared with a stroke font of technical lettering, both
  ways round: how far the drawn strokes are from the font's, and the
  font's from the drawn ones. A missing stroke (the side of an O, which a
  C lacks) counts for more than a slightly different hand.
* Lettering in small letters only ("seam") is read at its own size.
* Letters drawn alike but for size (c and C, o and O) take the case of
  their word; O and 0, l and 1 are letters in words and digits in numbers.
  A word one letter off a word patterns use ("RCULEAU") is set right.
* The name is the line above "cut …" in the label, or else a line of
  capitals read closely. No other lettering names a piece: strokes read as
  letters can be anything (a logo, hatching, a page's footer).
* A "fold line" (folded in sewing, as on a cut-on fly) no longer marks the
  edge near it to go on the fabric fold. "Fold" and "on fold" still do.

**Found by testing:**
* Notch cuts beside lettering were taken into it and lost. A stroke from
  one line to another, or straight and long from one, is now a notch cut,
  so the blouse's buttonhole ticks all come in. But a stroke lying along a
  line (an asterisk's, at a dart's point) or a dot sitting on one (an i's)
  stays lettering.
* A single stroke too long to be a letter, scaled up for matching, ran out
  of memory. Strokes are now capped in length for matching.
* The word "foldline" beside the trousers' cut-on fly marked the front
  leg's edge as a fold.

**Not yet:**
* A word crossed by a line drawn dash by dash ("seam" under a centre line)
  reads as nonsense, because the dashes are taken for letters. It names
  nothing. Joining dashed lines is a later step.
* Measurements printed on the pattern (the finished sizes table) are read
  but not used.

**What's under the hood:** `crates/io/src/pdf/strokefont.rs` (the font),
`reading.rs` (letters, lines, which way round, matching, words) and
`drawn_labels` in `import.rs` (which line is the name). Tests are in
`reading.rs` and `crates/io/tests/pdf_plotter.rs`.

---

## Step 15 — PDFs printed from pattern CAD through a plotter driver (Phase 2, part 3)

Some pattern companies draw in pattern CAD (Grafis, for example) and make
their PDFs by "printing" to a plotter driver. Such a PDF has no text and no
shapes, only thousands of short pen strokes: every line, every word and
every logo is strokes. These now import.

**Try it:**

1. Write the made-up plotter drawing the tests use, then open it:

   ```bash
   PLOTTED_PDF=plotted.pdf cargo test -p drapewright-io --test pdf_plotter
   ```

   It has three pieces, each drawn the way such files draw things:
   * **A front with a cut-on fly.** The allowance is 4 cm along the top of
     the fly edge and 1 cm below. The import breaks that edge where the
     allowance changes, so each part keeps its own allowance, and the
     cutting line comes out exactly as drawn.
   * **A back whose stitching line forks** round an extension above the
     hem. The outer way round becomes the stitching line, and the seam's
     way on to the hem stays as a line. Its dart ends in a circle: the
     circle is a mark, and the dart is not mistaken for the grainline.
   * **A bias strip** whose stitching line is drawn on three sides only. The
     fourth edge gets no allowance, as drawn, and the grainline is at 45°.
   * The words and the brand logo on each piece are left out, and the
     import says so. The logo is drawn at three sizes. The round "+"
     outlines become button marks.
2. The same drawing printed on A4 the way plotter drivers tile it:

   ```bash
   PLOTTED_TILES=tiles.pdf cargo test -p drapewright-io --test pdf_plotter
   ```

   Eight pages, each with a frame and only its own part of the drawing,
   to trim at the frame and butt together. The import says **8 pages
   joined** and gives the same three pieces as the one sheet.
3. On two real bought patterns (kept out of the repo), each sold as an A0
   copy-shop file and as A4 pages (36 and 24):
   * the blouse gives 10 pieces, 9 with their stitching lines (the 10th has
     none drawn), all with grainlines, and the 12 buttons;
   * the trousers give 7 pieces, 5 with stitching lines (the other two have
     none), all with grainlines, the fly's end, the dart's point and the
     waistband button;
   * the A4 files give the same pieces as the A0 ones.

**How the strokes become outlines:**
* Strokes join end to end. Where several lines meet, the outline goes on
  the way that turns least, as a pencil would. A notch cut stops where it
  meets the outline it marks.
* Hairline gaps a plotter leaves between strokes (under half a millimetre)
  are bridged.
* Lines that meet end to middle are seen as a network: a seam line running
  on past a corner into the allowance, a fork, notches joining the cutting
  line to the stitching line. Loose ends are trimmed. The outline is the
  network's outer edge, peeled off one after another: the cutting line,
  then the stitching line inside it.
* Lettering is clusters of tiny strokes, including short words a space
  apart ("to") and a dash between words. Logos are the same intricate
  letter shapes repeated on the pieces at any size, with the dots beside
  them.
* **Pages to trim and butt:** each page has a frame drawn round its part
  of the drawing, and no shape is on two pages. A line cut off at one
  page's frame carries on from the next page's frame, so the page grid
  (how many across, rows from the top or the bottom) is the one under
  which the most lines carry on. Then each line's two ends are stitched
  together across the join. Footers touching every frame are ignored.
* Arrowheads drawn as triangle outlines (as big as 3 cm) make grainlines,
  even when "grainline" is written right beside them. A piece's grainline
  is its longest arrow. Shorter ones show which way to press a dart.

**Not yet:**
* **Piece names and cutting counts** (step 16): they are drawn as strokes
  too, so pieces are named *Piece 1*, *Piece 2*… The import says so.
* An allowance that curves from one width into another (the end of the
  real trousers' fly) becomes a square step, and the import gives the
  largest difference from the drawn cutting line (9 mm there).

**Found by testing:** a rectangular piece bigger than 80% of an A4 page,
printed across several tiles, was taken for a page's frame and left out.
A frame must now lie on its page.

**What's under the hood:** `crates/io/src/pdf/butt.rs` (frames, the page
grid, stitches), `loops.rs` (the network of lines and its outer edges),
`lettering.rs` (words, logos, round "+" marks), the stepped allowances and
three-sided stitching lines in `assemble.rs`, and triangle arrowheads in
`marks.rs`. The test `crates/io/tests/pdf_plotter.rs` draws each case as a
plotter would, on one sheet and on butted A4 tiles.

---

## Step 14 — PDF patterns in several sizes (Phase 2, part 2)

**Try it:**

1. Open `fixtures/pdf/sizes-colours.pdf`, a front bodice and a sleeve in
   sizes 8, 10 and 12, nested as printed patterns are, each size in its
   own colour. The import says **Found 2 pieces in 3 sizes** and asks
   **Which size?**. Each choice shows its line as printed (red 8, green
   10, blue 12), named from the pattern's legend, with the middle size
   offered first. Only the chosen size's pieces are added, each with its
   name, grainline, cutting and fold.
2. Open `fixtures/pdf/sizes-layers.pdf`: the same pattern with each size
   on a layer ("Size 8"…), as layered indie patterns are. The sizes come
   from the layer names.
3. Choose two sizes to compare them: each piece says its size, *Front
   bodice (10)*.

**Why it matters:** without this, nested size lines look like a cutting
line with its stitching line inside, and the grade between sizes would
have been "measured" as seam allowance. Now each size is assembled on its
own, with what the sizes share (grainlines, names, cutting, notches).

**How sizes are told apart:**
* Layers named as sizes: "Size 10", "XS", "38", "UK 12", "Taille 38",
  "10/12"…
* Otherwise, line styles (colour, dash, width) that each draw outlines.
  This needs three or more styles, or a legend naming them, because a
  solid line and a dashed one are usually a cutting line and its
  stitching line. The styles' outlines must also nest, so pieces merely
  drawn in different colours stay one size.

**Every piece found is a suggestion:** the list of pieces in the import
now has a tick box for each, with its size. Untick anything that isn't a
pattern piece (a diagram from an instructions page, a title box) and it
isn't added.

**Protected PDFs, as Acrobat makes them:** `fixtures/pdf/sizes-layers-aes128.pdf`
and `…-aes256.pdf` open like the plain file. They use AES-128 and AES-256,
which is what Acrobat and InDesign write. `…-password.pdf` needs a password
to open, and the import says so and what to do.

**Not yet:** sizes whose lines share a stretch (a common centre front
drawn once for all sizes). Each size's line must go all the way round.

---

## Step 13 — PDF patterns (Phase 2, part 1)

**Try it:** *Open pattern* now takes `.pdf`.

1. Open `fixtures/pdf/freesewing-sasha-a0-protected.pdf`. It is the
   FreeSewing dress printed by Drapewright for a copy shop: a cover page
   and the sheet split over two A0 pages. It is also "protected" the way
   many bought patterns are: it opens without a password but says "no
   copying". The import:
   * Puts the 2 pages back together into one sheet, and leaves out the
     cover (test squares and page map).
   * Knows the size exactly: PDF measures in points.
   * Finds the 7 pieces and names them from the words printed on them. It
     reads the cutting from "Cut 2 · Main fabric" and "Cut 1 on the fold",
     and finds the fold edge from the "Place on fold" bracket.
   * Measures the 1 cm allowance from the printed stitching line, so
     nothing needs asking.
   * Turns the notches cut into the cutting line into notches (3 on the
     front, 5 on the front side panel…) and the arrows into grainlines.
     Their arrowheads, and the marks for joining pages, are left out.
2. Print the dress yourself from Drapewright (A4, trim and butt: 31
   pages), then open that PDF. The 31 pages are put back together into
   the same 7 pieces. Letter pages that overlap work too.

**How pages are put back together:** most tiling tools, Drapewright's
included, draw each page as the whole sheet's drawing, moved and clipped
to that page. A shape that appears on two pages, identical but moved,
says exactly where those pages sit on the sheet. So no page grid or
labels need guessing. Furniture that stays put on every page (frames,
footers, page letters) is ignored.

**Not yet:**
* PDFs whose tiles each contain only their own piece of the drawing,
  with no shapes in common. These need the page grid; they come after
  sizes.
* A list to untick pieces you don't want (instruction pages can still add
  a diagram or two).

**Found by testing:**
* The printed piece names sat at the middle of each piece's bounding box,
  which can be outside a curved piece (and printed over another one). The
  PDF now prints names where the piece is widest, as the AI previews do.
* Pages that share no visible shape (a blank band between pattern rows)
  are still joined, through the shapes they draw beyond their edges.

**What's under the hood:** a new crate, `crates/pdf-import`, reads PDFs.
It covers the file structure (damaged files included), streams, fonts and
text, colours, layers, and the standard encryption with an empty user
password. It is pure Rust with no randomness, so it runs in the browser.
`crates/io/src/pdf/import.rs` turns its drawing into pieces, with
`tiles.rs` (putting pages back together) and `marks.rs` (grainlines,
notches, fold brackets, test squares). Round-trip tests print the dress
in each layout and check that importing the PDF gives back the same
pieces.

---

## Step 12 — Working with an AI assistant (MCP server)

**Try it** with Claude Code (Claude Desktop works too; see
[ai-assistants.md](ai-assistants.md)):

```bash
cargo build --release -p drapewright-mcp
claude mcp add drapewright -- "$PWD/target/release/drapewright-mcp"
```

Then start `claude` in the repository folder and ask:

1. *"Import fixtures/dxf/aama-graded.dxf."* The assistant can't import it
   until it asks you three things, because the server refuses to guess
   them: which size (S, M or L, made in M), how big the file is (it only
   says "metric"), and whether seam allowance is included.
2. *"Import fixtures/svg/freesewing-sasha-sa10.svg and show me the
   pieces."* No questions this time. The assistant gets a picture: each
   piece numbered and filled, with folds in blue, the stitching line
   dashed, notches and grainlines.
3. *"Show the front with its edge ids, call the fold edge centre front,
   rename the pocket to Patch pocket, cut 2 in lining, and add a
   70 × 8 cm waistband."* Each request is one undo step (*"undo that"*
   works).
4. *"Lay everything out for 150 cm fabric and save it to
   ~/Desktop/sasha.drapewright."* Open that file in the app with
   **Projects → Restore a backup…**. The project is all there, with the
   original pattern files kept inside it.

**How it behaves:** it reads and writes only the files it is given, makes
no network connections, never overwrites a file or drops unsaved changes
without being told to, and changes projects only through the app's own
commands.

**Found by testing:** piece numbers placed at the middle of a piece's
bounding box fell outside curved pieces, such as the dress front. They now
go at the point farthest inside the piece.

**What's under the hood:** `crates/mcp` speaks the protocol (JSON-RPC over
stdio) in `lib.rs`, and its 25 tools are in `tools.rs`. Each tool's
arguments are a Rust type, so its JSON Schema is generated and can't drift.
Several commands now apply as one all-or-nothing undo step
(`Engine::dispatch_all`). Previews are drawn with tiny-skia, labelled with
a small built-in stroke font so they need no font files (`render.rs`,
`font.rs`).

---

## Step 11 — Real patterns: every size in one file, and FreeSewing (after Phase 1)

**Try it:**

1. **A file with every size.** Garment CAD files (Gerber, CLO, Browzwear)
   carry each piece once per size. Open `fixtures/dxf/aama-graded.dxf`: the
   import says **Found 2 pieces in 3 sizes** and asks **Which size?**, with
   the size the pattern was made in (M) chosen. Choose L as well to compare
   two sizes. Each piece then says which it is, *Front (M)* and *Front (L)*.
   Only the chosen sizes are added.
2. **A FreeSewing pattern.** Open `fixtures/svg/freesewing-sasha-sa10.svg`,
   FreeSewing's Sasha dress in size 36 with 1 cm allowance:
   * The 7 pieces have FreeSewing's names: *Front inside*, *Pocket*,
     *Sleeve*…
   * Both lines are in the file, so the 1 cm allowance is measured, not
     asked.
   * *Front inside* is cut on the fold. FreeSewing leaves its cutting line
     open along the fold, and that edge becomes a **fold** with no
     allowance.
   * The 14 notches land on the seams (the back one is double). Every piece
     has its grainline, and "Cut 2 mirrored" or "Cut 1 on fold" comes
     across.
   * Left out, and listed in the import: FreeSewing's logo on the sleeve,
     and the second copies it draws of the front neckline and back armhole.
     A copy left in would stay behind as a ghost line when you reshape that
     seam.
   * The import says once, for all 7 pieces, that FreeSewing's cutting
     lines differ from Drapewright's by up to 1 cm. That is only at
     corners: FreeSewing cuts its corners off at 45°, while Drapewright
     points them (open question 3). Notes that say the same about several
     pieces are now always said once.
3. `fixtures/svg/freesewing-sasha-no-allowance.svg` is the same dress
   without allowance. Its outlines could be either line, so you are asked
   about allowance. The fold still comes from FreeSewing's cut-on-fold
   bracket.

**Checking against real patterns:** the collection in
`~/Source/drapewright-test-patterns` can't be committed (its README says
where each file comes from), but you can see how all of it imports:

```bash
cargo run --release -p drapewright-io --example import_report -- -v ~/Source/drapewright-test-patterns
```

Every file imports. That covers 44 garment CAD exports from Gerber, Lectra,
Assyst, CLO and Browzwear, museum patterns from MoMu Antwerp, and the
FreeSewing dress.

**Not yet:** Lectra and Assyst exports carry only the size the pattern was
made in. CLO and Browzwear files that come with a `.rul` file hold the base
size plus rules for the others, and those rules are not applied yet.

**Found by testing:** a line inside two overlapping pieces went to either
piece, at random, from one import to the next. It now goes to the smaller
piece, so a file always imports the same way.

**What's under the hood:** FreeSewing's conventions are in
`crates/io/src/svg/freesewing.rs`. Cutting lines left open (at a fold or
a corner) are closed in `close_open_outlines` in `crates/io/src/assemble.rs`.
Sizes are in `ImportResult.sizes`, and `choose_sizes` keeps the chosen ones.

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
