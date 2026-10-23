# Sewing a piece onto another: patch pockets

*Design, 2026-10-23. Checked by JP on 2026-10-23 and built as Step 80;
§9 lists where the build differs from it.*

A patch pocket is sewn by its sides and bottom onto the face of a panel,
where the panel's pattern marks its placement, with its top left open. Today
a seam can only join an edge to an edge. So a pocket can't be sewn: it is
left off the drape ("One piece isn't sewn to anything"), though it imports
and prints like any piece.

This design lets one side of a seam be a line drawn inside a piece instead
of an edge. Find seams suggests a pocket onto a placement of its shape. It
can be sewn on by hand. The drape shows it lying over the panel.

## 1. The file: a seam side on a line

A seam side gains one field, `line`:

```json
{ "piece": "front", "edge": "", "line": "pocket-placement", "from": 0, "to": 160 }
```

- `line` names one of the piece's `lines` (an `InternalLine`). `edge` is
  then empty. An edge side is as before, with `"line": null`.
- `from` and `to` are millimetres along the line as drawn, as along an
  edge. They always run from lower to higher. A closed line (one that ends
  where it starts) is measured once round from its start.
- **Each pocket edge is its own seam.** The pocket's left side is sewn to
  the placement from 0 to 160 mm, its bottom from 160 to 290, and its right
  side from 290 to 450. That reuses everything a seam already has:
  measuring, the two lengths compared, Turn round, Unpick, the seams list,
  and the assistant's tools. If a pocket edge would run past a closed
  line's start, it is sewn as two seams, so that `from` < `to` holds
  everywhere.
- Lines have no notches, so a line side reports none. A line that has gone
  is a new problem, `missingLine`, next to `missingEdge`.
- **Schema version 8**, with a migration from 7 that changes nothing
  (version 7 files have no line sides). `docs/file-format.md` §8 and §10.4
  and the JSON Schema are updated. Older builds then refuse version 8
  files, as they do for every format step.

I considered a separate record, "pocket P sewn onto line L, open at edge
E", with the stitching worked out from shapes each time. It would be one
row in the seams list instead of three. But it would be a second kind of
join beside seams, and the measuring, the turning round and the assistant's
tools would all need doing twice. Seam sides are smaller, and they are what
the brief asked for.

## 2. Find seams: a pocket onto a placement of its shape

A new pass runs before the edge-to-edge suggestions (in
`crates/seams/src/pockets.rs`). The pockets it places then take no part in
edge seams, so a pocket's 15 cm side can't be suggested onto someone's
15 cm cuff.

- **Placements:** lines inside other pieces drawn as `placement` (the DXF
  importer's AAMA layers 9/10) or as plain `line` (what the SVG and PDF
  importers make of a dashed outline). Each must be at least 6 cm long, and
  closed or open.
- **Pockets:** pieces not sewn yet, of any name, cut in the main fabric.
  "Pocket" in the name only breaks ties.
- **Shape match.** Each edge of the pocket is tried as its opening. The
  rest of its stitching line (its "sewn run": side, bottom, side, or
  however many edges) is fitted to the placement: resampled by length on
  both, turned and shifted (never mirrored) for the closest fit, as
  `sim::inside::fit` already does. An open placement is fitted whole,
  either way along. A closed placement is fitted at every starting point
  round it, either way. The fit is accepted if it is within 3 mm on
  average and 8 mm at worst, and the pocket's grain lies along the panel's
  (within 10°, or across it: some pockets are cut on the cross grain).
- **A deeper top.** A patch pocket's top is turned down, so its sides on
  the piece run past the placement's corners by that turn-down. If the
  sewn run is longer than the placement's, the extra is taken off its two
  ends equally, up to 6 cm in all. The side seams then sew part of each
  side (`from` or `to` on the pocket's side edges), up to where the turn
  starts.
- **Which edge is the opening.** Of the fits that pass, the best wins. If
  a rectangle fits equally whichever edge is left open, the opening is the
  edge marked as a hem. Failing that, it is the edge lying highest on the
  panel once the panel is stood upright as the drape stands it (by its
  grain, then its hems: the rule in `sim::garment::upright`, moved to
  `pattern-core` so that both can use it).
- **Pairs.** A pocket cut as a pair goes on a panel cut as a pair, or on
  both halves of a panel cut on the fold (its placement is drawn on the
  half). Each seam is sewn on both halves of the body, as an edge seam is.
  A pocket cut once on a panel cut as a pair goes on the half as drawn.
- **Suggested** with a new reason, `onPlacement` ("where its placement is
  marked"). Confidence comes from the fit: about 0.95 within 1 mm, falling
  towards the 3 mm limit.

Not suggested: placements marked only by dots at the pocket's corners
(open question 40), and a pocket that fits no placement. The second can
still be sewn on by hand, onto a line of another shape.

## 3. By hand

- In a piece's details, a piece with no seams has **Sew onto a
  placement…** next to the edge's **Sew to another edge…**. The status bar
  then says "Tap the line on the panel where Pocket goes", with Cancel, as
  sewing an edge does now.
- Lines become things that can be tapped (a new `Hit::Line` in
  `crates/project/src/hit.rs`, after notches, corners and edges). Tapping
  inside a piece, away from its lines, takes the line nearest the tap.
- The engine fits the pocket to that line, as Find seams does, but with
  its limits relaxed (10 mm on average). It sews all the seams in one undo
  step and picks the first, so the pocket is shown joined to the panel.
  If nothing fits, a toast says "Pocket isn't the shape of that line".
- On the table, a picked or suggested seam on a line draws that part of the
  line strongly, joined to the pocket's edge by the dashed line, as edges
  are now. The seams list names the line by its label: "Pocket, left side
  to Skirt front, pocket placement".
- The assistant (MCP): `sew_edges` takes `line` in place of `edge` for
  either side, and `get_seams` reports it.

## 4. The drape

### Points on the panel where the pocket is sewn

The pocket's sewn edges are stitched to points of the panel, not to points
somewhere inside its triangles.

- **The panel's mesh includes the placement.** `sim::mesh::mesh` takes
  inner chains as well as the boundary: their points are inserted into the
  constrained Delaunay triangulation, joined by constraint edges, and kept
  clear of the lattice as the boundary is.
- Each seam on a line is sampled as a seam on an edge is: as many steps on
  the pocket's edge as on its stretch of the line, so the stitches are
  point to point. The existing stitch constraint then sews them, as it
  does every seam. So `cloth.rs`'s and the card's stitch solvers don't
  change, and neither does their colouring.
- The step count comes from the line's length alone, so the panel's mesh
  is the same whether or not the pocket is in the garment being assembled.
  The garment without its linings and pockets, which is what is placed and
  measured, then matches the whole garment panel for panel. This is the
  same check linings already pass (`inside::with_inside`).
- Only the sewn stretches go into the mesh, not the pocket's open top. A
  panel cut on the fold, or the second of a pair, is still meshed as the
  mirror image of the first, placements included, so a garment cut alike
  on both sides hangs alike. If only one half has a pocket, it is meshed
  whole.
- A placement that crosses the panel's edge or its fold can't be meshed
  inside it. Its pocket is then left off (see the status lines below)
  rather than the whole drape failing.

### Where the pocket starts, and which way out it lies

- **Panel order:** the garment's own panels, then its facings and linings,
  then the pieces sewn on. The cloth's layered points run from
  `inside_from` to the end as now, and a second index, `on_from`, starts
  the pieces sewn on.
- **Placed** as a lining is (`inside::place_inside`): laid flat on the
  panel its stitches join it to, by the turn that fits them, then started
  where that panel is on the body. `Panel.over` is the panel it lies on,
  and a new `Panel.sewn_on` says it lies outside it.
- **Kept outside the panel.** `order_layers` keeps a lining behind the
  cloth it lies on. For a point at or after `on_from`, the same push runs
  the other way: in front of the panel by the cloth's thickness and a
  little air, within the same reach, with the same friction. It is limited
  to the panel it lies on (`lies_on`), so a pocket near a side seam isn't
  pushed off the back. Where the two have folded over each other (their
  normals disagree), both are left be, as linings are. The sewn points
  themselves are one with the panel (`at_seam`) and are not pushed.
- **Which way it faces** comes from the panel it lies on, as for a lining
  (`face_out`).
- **On the card** (`crates/sim-gpu`): the parameters' spare slot
  (`counts.w`) carries `on_from`. `short_of` and the two `order_layers`
  passes take the sign from it. Nothing else changes: the same passes, in
  the same order, each point's list read in the same order. So the card
  stays deterministic (`the_card_drapes_alike_however_long_its_cloth_takes_to_come_back`
  gets the pocket skirt too).
- **Drawn** as the garment's own cloth, not paler as linings are: the 3D
  view, the drape picture and the assistant's drape tool ask
  `cloth.inside(k)`, which becomes `inside_from <= k < on_from`.

### Status lines

- A pocket that is draped needs no line of its own: it is just there.
- "A pocket is sewn onto a panel that isn't draped, so it's left off." (Its
  panel is sewn to nothing, say.)
- "A pocket couldn't be laid on its panel, so it's left off." (Its
  placement runs over the panel's edge, say.)

A piece sewn onto a facing or lining (a coat's inside pocket) faces the
body, the other way. It is left off for now, with the first line above.

## 5. Not covered: pockets drafted into a panel (open question 36)

Sasha's side front comes in two pieces. One runs down past the pocket's
opening as its bag, behind the other, and is the garment's own outside
cloth above it. So one panel would be outside cloth in one part and behind
another panel in another. The layers here go by panel (whole ranges of
points), so this design doesn't cover it.

It would need each point to know its own layer: behind which panel, in
front of which, or neither. That means per-point roles in `order_layers`
and on the card instead of the two ranges, and placing for a piece that is
part shell, part inside. That's a step of its own, and the pockets here
don't need it. Question 36 stays open, and this is noted there.

## 6. Tests and the fixture

- `pattern-core::testing::garments::pocket_skirt()`: the test skirt (front
  cut on the fold, back cut as a pair), with a 13 × 16 cm patch pocket cut
  as a pair. The pocket has rounded bottom corners and a hem at its top,
  turned down 3 cm. Its placement on the front is a closed outline,
  labelled "Pocket placement" and drawn from its top left corner. It is
  saved as `fixtures/drapewright/pocket-skirt.drapewright` by the fixture
  test.
- Seams: Find seams sews it whole (sides, centre back and the pocket on
  both halves of the front), with `onPlacement`. Variants are an open
  placement (three sides), a closed one starting mid-curve (a seam split in
  two), a pocket with its allowance included and the deeper top not marked
  (partial side seams), and a placement of another shape (not suggested).
  By hand: the engine's sew-onto with a line, then Unpick.
- Format: version 7 migrates to 8, and a line side round-trips.
- Mesh: inner chains are kept, point for point, and are mirrored on a fold.
- Drape, on the processor (`crates/sim/tests/drape.rs`) and on the card
  (`crates/sim-gpu/tests/drape.rs`): the pocket skirt settles, every pocket
  stitch closes, and every pocket point ends up outside the front, not
  through it. It also settles from nudged starts. The card's run is the
  same however long its cloth takes to come back.
- Print: the pocket skirt's PDF is the same with and without its seams.
- Web: unit tests for the line labels, and an e2e test that opens the
  fixture, finds the seams, sews them all and drapes it, with the pocket
  shown.

## 7. Open questions to add

40. **Pocket placements marked by dots only.** Many commercial patterns
    mark only the top corners. *Provisional: not suggested; sew it on by
    hand onto a line drawn on the panel.*
41. **A pocket's top turned down by an amount not marked.** *Provisional:
    the turn-down is the difference between the pocket's sides and the
    placement's, up to 6 cm.*

## 8. Order of work

1. Format: `SeamSide.line`, measuring, problems, schema 8 and its
   migration, docs.
2. Find seams' pocket pass and sewing one on by hand in the engine, with
   their tests.
3. Drape: inner chains in the mesh, sampling lines in `assemble`, the
   panel order, placing, layering outside on the processor and the card,
   and the status lines.
4. The fixture, the web UI (tapping lines, Sew onto…, labels, drawing on
   the table), MCP, the demo note, then `pnpm ci:local`.

## 9. As built

Built as above, except:

- **The opening.** A pocket with a hem (an edge marked as one, or given a
  hem's allowance) is only tried open at its hem, not preferred so. A
  rectangle sewn on upside down onto a square fitted within the by-hand
  limits otherwise.
- **Names** play no part: a piece named "pocket" is fitted as any other.
- **By hand, the limits** are 6 mm on average and 12 mm at worst (not 10
  mm), with the placement up to 25 mm longer round than the pocket.
  Placements 5 mm bigger all round are sewn onto; a 12 cm square for a 13
  × 16 cm pocket is refused.
- **A pair's mirror copy** is fitted on its own, so that a front drawn whole
  with a placement either side gets the pocket on one and its mirror copy
  on the other (the seams name the copy, `mirror`).
- **Tapping a line:** there is no `Hit::Line`. While a pocket waits to be
  sewn on, a tap on any piece takes the line on it nearest the tap
  (`Engine::nearest_line`). Hit-testing elsewhere is as before.
- **Left off:** two status lines, as designed, but split by cause: sewn
  onto a piece not draped or onto a lining (`Draping.off`), and not laid on
  its piece (`Draping.not_laid`).
- **What can be a pocket.** Tried on about 300 real patterns (graded
  nests from pattern CAD, bought and free PDFs, museum SVGs), the first
  build suggested 1,109 "pockets": waistbands, plackets and tabs fitting
  straight lines or slits drawn inside other pieces. Find seams now also
  needs a pocket at least 35 mm across its narrowest way (turned any way)
  on its stitching line, its sewn run to turn away from its opening by a
  fifth of the opening's width at least, its opening at most 40% of its
  way round, and not to be cut from interfacing; and an open placement
  must turn too. That leaves 6 across those patterns: a haori jacket's
  patch pocket, what looks like one on a museum nightgown, and four
  matches between legend or information boxes imported as pieces.
- **The garment placed and measured** (`garment::shell`) is assembled with
  its pockets and then has them taken off (`Garment::first`), so that the
  front's mesh is the same in it as in the whole garment.
