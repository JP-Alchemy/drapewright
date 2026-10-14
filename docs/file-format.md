# The Drapewright project format

Status: **schema version 6** (Phase 4). Sections marked *draft* describe
how later phases will extend the format; they become normative, with a
version bump and a migration, when they are implemented.

The format is open. Anyone may read and write it. The machine-readable
JSON Schema, generated from the reference implementation, is in
[`schema/`](schema/).

---

## 1. Container

A project is saved as a single file with the extension **`.drapewright`**. It is a
standard zip archive:

| Path | Required | Contents |
|---|---|---|
| `project.json` | yes | The project document described below (UTF-8). |
| `sources/` | no | Original imported files, named `<sha256>-<original name>`. Kept so a piece can always be traced to, and re-imported from, its source. |
| `thumbnails/cover.png` | no | Preview image for project lists. |
| `meshes/`, `textures/` | no | *draft (Phase 3)*: binary sidecars referenced from `project.json` by relative path. |

Readers must ignore files they do not know. Writers must not put anything
needed to reconstruct the pattern anywhere except `project.json`, with the
single exception of sidecars it references explicitly.

## 2. Conventions

* **Units:** all lengths are **millimetres** as JSON numbers (64-bit
  floats). Angles are **degrees**. Display units (cm, inches) are a
  preference and never change stored values.
* **Coordinates:** x points right, **y points down**, as in SVG and on
  screen.
* **Points** are `[x, y]` arrays.
* **Orientation:** a piece outline runs **clockwise as seen on screen**,
  i.e. its signed area computed with y down is positive. The outside of an
  edge is therefore on the left of its direction of travel, as seen on
  screen.
* **Identifiers** are non-empty strings, unique within the whole project.
  Their form is otherwise opaque. The reference implementation writes short
  ones such as `p3` or `e12`.
* **Keys** are camelCase. Optional fields may be omitted; the documented
  default applies.

## 3. Top level

```json
{
  "format": "drapewright-project",
  "schemaVersion": 6,
  "generator": "Drapewright 0.1.0",
  "meta": {
    "id": "7f3c9a2e5b1d4e8f",
    "name": "A-line skirt",
    "created": "2026-10-10T20:15:00Z",
    "modified": "2026-10-10T20:40:12Z",
    "notes": ""
  },
  "settings": { "displayUnits": "cm" },
  "pieces": [ /* Piece */ ],
  "print": { /* PrintSettings */ },
  "measurementSets": [ /* MeasurementSet, §10.2 */ ],
  "seams": [ /* Seam, §10.4 */ ],
  "fabrics": [ /* OwnFabric, §10.5 */ ],
  "view": { /* ViewState */ },
  "extensions": {}
}
```

| Field | Type | Notes |
|---|---|---|
| `format` | `"drapewright-project"` | Identifies the file type. |
| `schemaVersion` | integer | `6` for this document. See §8. |
| `generator` | string | Free text naming the program that wrote the file. |
| `meta.id` | string | Stable project identifier. |
| `meta.name` | string | Shown to the user. |
| `meta.created`, `meta.modified` | RFC 3339 string | UTC. |
| `meta.notes` | string | Free text. Default `""`. |
| `settings.displayUnits` | `"mm"` \| `"cm"` \| `"in"` | Default `"cm"`. |
| `pieces` | array of Piece | Order is the order shown in piece lists. |
| `print` | PrintSettings | Optional; defaults in §6. |
| `measurementSets` | array of MeasurementSet | People's measurements (§10.2). Default `[]`. Added in version 2. |
| `seams` | array of Seam | Edges sewn together (§10.4). Default `[]`. Added in version 3. |
| `fabrics` | array of OwnFabric | Fabrics of one's own, measured at home (§10.5). Default `[]`. Added in version 6. |
| `view` | ViewState | Optional; non-semantic (§7). |
| `extensions` | object | Data owned by other programs, keyed by a reverse-DNS or URL namespace (e.g. `"org.example.cutter"`). Readers must preserve it unchanged. |

## 4. Piece

```json
{
  "id": "p1",
  "name": "Skirt front",
  "notes": "",
  "outline": { "start": [0, 0], "edges": [ /* Edge */ ] },
  "allowance": { "mode": "add", "seam": 15, "hem": 40 },
  "cut": [{ "material": "main", "quantity": 1, "onFold": true }],
  "grainline": { "from": [120, 150], "to": [120, 450] },
  "notches": [ /* Notch */ ],
  "darts": [ /* Dart */ ],
  "marks": [ /* Mark */ ],
  "lines": [ /* InternalLine */ ],
  "labels": [ /* Label */ ],
  "placement": { "x": 0, "y": 0, "rotation": 0 },
  "source": { /* Source */ },
  "alterations": [ /* Alteration */ ],
  "drawn": null
}
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | string | — | |
| `name` | string | — | e.g. "Front bodice". |
| `notes` | string | `""` | |
| `outline` | Outline | — | §4.1. |
| `allowance` | Allowance | `{ "mode": "add", "seam": 15, "hem": 15 }` | §4.3. |
| `cut` | array of CutInstruction | `[{ "material": "main", "quantity": 1 }]` | §4.5. |
| `grainline` | Grainline \| null | `null` | §4.6. |
| `notches` | array of Notch | `[]` | §4.7. |
| `darts` | array of Dart | `[]` | §4.14 (version 5). |
| `marks` | array of Mark | `[]` | §4.8. |
| `lines` | array of InternalLine | `[]` | §4.9. |
| `labels` | array of Label | `[]` | §4.10. |
| `placement` | Placement | `{ "x": 0, "y": 0, "rotation": 0 }` | §4.11. |
| `source` | Source \| null | `null` | §4.12. |
| `alterations` | array of Alteration | `[]` | §4.13 (version 5). |
| `drawn` | Drawn \| null | `null` | The piece as drawn, before its alterations; §4.13. |

All piece geometry (outline, notches, darts, marks, lines, labels, grainline) is
in the piece's own coordinate system. `placement` positions the piece on the
work table without changing its shape.

### 4.1 Outline and edges

An outline is a closed path made of **edges**. An edge runs from one corner
of the piece to the next (for example "side seam" or "armhole") and may
consist of any number of segments.

```json
"outline": {
  "start": [0, 0],
  "edges": [
    { "id": "e1", "name": "Waist", "segments": [{ "c1": [80, 4], "c2": [160, 8], "to": [240, 12] }] },
    { "id": "e2", "name": "Side seam", "segments": [{ "to": [300, 600] }] },
    { "id": "e3", "name": "Hem", "kind": "hem",
      "segments": [{ "c1": [200, 610], "c2": [100, 615], "to": [0, 615] }] },
    { "id": "e4", "name": "Centre front", "kind": "fold", "segments": [{ "to": [0, 0] }] }
  ]
}
```

* `start` is the first corner. Each segment continues from the end of the
  previous one; the first edge starts at `start`.
* The last segment of the last edge must end exactly at `start`
  (tolerance 1 × 10⁻⁶ mm).
* Every edge has at least one segment and a length greater than zero.

**Edge fields**

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | string | — | |
| `name` | string | `""` | Optional sewing name. |
| `kind` | `"seam"` \| `"hem"` \| `"fold"` | `"seam"` | `hem`: takes the piece's hem allowance and shapes neighbouring corners for folding up (see §4.4). `fold`: placed on the fabric fold; no allowance; must be straight. |
| `segments` | array of Segment | — | §4.2. |
| `allowance` | number \| null | `null` | Overrides the piece's allowance for this edge, in mm. `0` means no allowance. Ignored for `fold`. |
| `endCorner` | CornerStyle | `"auto"` | How the cutting line is shaped at the corner where this edge ends and the next begins (§4.4). |

### 4.2 Segments

| Form | Meaning |
|---|---|
| `{ "to": [x, y] }` | Straight line to `to`. Conceptually the cubic whose control points lie on the line. |
| `{ "c1": [x, y], "c2": [x, y], "to": [x, y] }` | Cubic Bézier with control points `c1`, `c2`, ending at `to`. |

Internal lines (§4.9) use the same encoding: a `start` point followed by
segments.

### 4.3 Allowance

| Field | Type | Default | Notes |
|---|---|---|---|
| `mode` | `"add"` \| `"included"` | `"add"` | `add`: the outline is the **stitching line**; the cutting line is the outline grown by the allowance. `included`: the outline is the **cutting line** as printed on the original pattern; the stitching line is the outline shrunk by the allowance. |
| `seam` | number | `15` | Default allowance for `seam` edges, mm. |
| `hem` | number | same as `seam` | Default allowance for `hem` edges, mm. |

The cutting line and stitching line are always **derived**. They are never
stored, so they can never disagree with the outline.

### 4.4 Corner styles

Each corner joins the edge before it (*this* edge, which owns the
`endCorner` field) to the edge after it (*next*).

| Value | Name in the app | Construction of the cutting line |
|---|---|---|
| `"auto"` | Automatic | `mirrorNext` if the next edge is a hem, `mirrorThis` if this edge is a hem, otherwise `mitre`. |
| `"mitre"` | Pointed | Both cutting lines are extended until they meet. If the point would reach further than 4 × the larger allowance from the corner, it is cut square across at that distance. |
| `"squareThis"`, `"squareNext"` | Squared | The allowance of the named edge ends with a cut at right angles to that edge, placed where the other edge's cutting line crosses the named edge's stitching line (extended). Applies where the corner is a right angle or sharper; at wider corners there is no point to cut off and the corner is pointed. |
| `"mirrorThis"`, `"mirrorNext"` | Folded back | The end of the named edge's allowance is the mirror image, reflected in that edge's stitching line, of the other edge's cutting line. When the allowance is folded back (a hem turned up, a seam pressed open) its end lies exactly on the other edge's cutting line. Where that end is narrower than the allowance (a deep hem on a strongly flared skirt), the allowance is trimmed at the corner. |

Corners next to a `fold` edge are constructed as `mitre` against the fold
line, so the cutting line meets the fold exactly. Shapes are built from
the tangents at the corner. A shaped corner never reaches further than
12 × the larger allowance, and never trims more than 4 × the larger
allowance from the corner. Where it would, the corner is pointed and the app
says so. Where an edge turns sharply between two of its own segments
(a kink that is not a corner), the allowance is pointed there too.

The reference implementation's gallery of corner cases is in
`crates/pattern-core/tests/gallery/`.

### 4.5 Cut instructions

```json
"cut": [
  { "material": "main", "quantity": 2, "mirrored": true },
  { "material": "interfacing", "quantity": 1, "onFold": true }
]
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `material` | string | `"main"` | `main`, `lining`, `interfacing`, `contrast`, or any free text. |
| `quantity` | integer ≥ 1 | `1` | |
| `mirrored` | boolean | `true` when `quantity` ≥ 2 | Cut as mirrored pairs (fabric folded, or flip the pattern). |
| `onFold` | boolean | `true` if the piece has a `fold` edge | Place the `fold` edge on the fabric fold. |

### 4.6 Grainline

```json
"grainline": { "from": [120, 150], "to": [120, 450], "oneWay": false }
```

The segment from `from` to `to` lies along the fabric's lengthwise grain.
`oneWay: true` (fabric with nap or a one-way print) means "top of the fabric
towards `from`". It is drawn with a single arrowhead at `to`. A bias-cut
piece simply has its grainline at 45° to the piece.

### 4.7 Notches

```json
{ "id": "n1", "edge": "e2", "at": 180, "from": "start", "kind": "single" }
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `edge` | edge id | — | The edge the notch sits on. |
| `at` | number | — | Distance in mm **along** the edge, measured from `from`. |
| `from` | `"start"` \| `"end"` | `"start"` | Which end of the edge `at` is measured from. When the edge changes length, the notch keeps its distance from this end. |
| `kind` | `"single"` \| `"double"` \| `"triple"` | `"single"` | Double and triple notches are spaced by the print setting `notchSpacing`. |
| `angle` | number | `0` | Degrees away from perpendicular to the edge. |

### 4.8 Marks

```json
{ "id": "m1", "kind": "drill", "at": [140, 220] }
{ "id": "m2", "kind": "buttonhole", "at": [30, 80], "length": 18, "angle": 90 }
```

`kind` is one of `drill`, `dot`, `circle`, `cross`, `button`, `buttonhole`.
Optional: `size` (diameter, mm), `length` (buttonholes, mm), `angle`
(degrees), `label` (string).

### 4.9 Internal lines

```json
{ "id": "l1", "kind": "lengthenShorten", "start": [0, 300], "segments": [{ "to": [300, 300] }],
  "label": "Lengthen or shorten here" }
```

`kind` is one of `line`, `fold` (pleat or tuck fold), `placement` (pocket or
trim position), `stitch` (topstitching guide), `cut` (slit), and
`lengthenShorten`. Optional `label`.

### 4.10 Labels

```json
{ "id": "t1", "text": "Pocket", "at": [100, 300], "size": 6, "angle": 0 }
```

Free text drawn on the piece. `size` is the letter height in mm (default
5). The piece name and cutting instructions are drawn automatically and are
not stored as labels.

### 4.11 Placement

`{ "x": mm, "y": mm, "rotation": degrees }` maps piece coordinates to the
work table: rotate about the piece origin, then translate. Placement never
changes a piece's shape or its printed result.

### 4.12 Source

```json
"source": {
  "format": "svg",
  "file": "skirt.svg",
  "sha256": "9b2f…",
  "ref": "path#front-outline",
  "scale": { "mmPerUnit": 0.264583, "basis": "declaredUnits", "confirmed": true }
}
```

| Field | Notes |
|---|---|
| `format` | `svg`, `dxf`, `pdf`, `project` (taken from another project), `drafted`, `photo` (a paper pattern piece photographed, measured against a sheet of paper of known size beside it). |
| `file`, `sha256` | Original file name and hash; the file itself is in `sources/`. |
| `ref` | Element id, layer or block name inside the source. |
| `scale.mmPerUnit` | Millimetres per source unit used at import. |
| `scale.basis` | How the scale was established: `declaredUnits` (file states physical units), `assumed96dpi`, `assumed72dpi`, `assumed90dpi` (drawings without units), `assumedMm`, `assumedCm`, `assumedInches` (CAD drawings without units), `measured` (the user measured a known length). |
| `scale.confirmed` | Whether the user confirmed the scale. Imports never proceed with an unconfirmed guess. |

### 4.13 Alterations (version 5)

Changes to a piece's shape are kept as steps, with the piece as drawn
kept beside them, so that nothing is changed for good.

```json
"alterations": [
  {
    "id": "a1",
    "on": true,
    "change": {
      "type": "lengthen",
      "line": { "type": "own", "line": "l3" },
      "by": 25
    }
  }
],
"drawn": { "outline": { /* Outline */ }, "grainline": null, "notches": [], "darts": [], "marks": [], "lines": [], "labels": [] }
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `id` | string | — | |
| `on` | boolean | `true` | Switched off, the step is kept but not applied. |
| `change` | Change | — | What it does (below). |

`drawn` holds the piece's outline, grainline, notches, darts, marks,
internal lines and labels as drawn. A piece with any alterations must have
it, and its own outline, grainline, notches, darts, marks, lines and labels
are then exactly `drawn` with the steps that are on applied in order.
Readers may recompute them; writers must keep them so. With no
alterations, `drawn` is `null` and the piece is as drawn. Ids of edges,
notches, darts, marks, lines and labels are the same in the piece and in
`drawn`.

**Change** `lengthen` — `{ "type": "lengthen", "line": AlterLine, "by":
mm }`: longer by `by` across the line (shorter where `by` is less than
0). The part of the piece beyond the line moves: below it, or right of an
upright line, whichever way the line is given. Lengthened, the piece is
spread apart along the line and an edge crossing it is bridged straight
across the gap. Shortened, a tuck as deep as `|by|` is folded out beyond
the line: what lay in the tuck lies along the line (a jog in a slanted
edge), and what lay beyond it moves back. Notches stay where they were
on their edges' cloth, and so do darts' legs; marks, labels, internal
lines and the grainline move with the part they are on.

**Change** `spread` — `{ "type": "spread", "from": [x, y], "hinge": [x,
y], "by": mm, "turns": "right" | "left" }`: slash and spread. The piece is
slashed straight from `from` to `hinge`, both on its edge, and the part on
the `turns` side of the slash (going from `from` to `hinge`, as the
pattern is seen) turns about the hinge until the slash is `by` mm open at
`from`; less than 0 closes it, the parts overlapping and meeting where
they cross. The slash must run inside the piece.

**Change** `fullBust` — `{ "type": "fullBust", "apex": [x, y], "armhole":
[x, y], "side": [x, y], "by": mm }`: a full bust adjustment by the Y
method (a small one where `by` is less than 0) on a bodice front. Slashes
run from the bust point `apex` straight down the grain to the waist, from
it to `armhole` (on the armhole, where it hinges), from it to `side` (on
the side seam), and from it straight across to the centre front. The side
turns about the hinge until the bust point has moved `by` mm out; the part
below the side seam slash moves with it, opening a dart in the side seam
whose tip is 25 mm short of the bust point (added to the piece's `darts`,
its id the step's with `.dart` after it); the centre front below the bust
drops to meet the side at the waist. Closing (a small bust) overlaps
instead, with no dart.

**Change** `pivot` — `{ "type": "pivot", "dart": id, "to": [x, y],
"share": 0 to 1 }`: one of the piece's darts (§4.14) pivoted to `to`, on
its edge. The piece is slashed straight from `to` to the dart's tip (the
point between its legs furthest from its mouth), and the smaller of the
two parts the slash makes turns about the tip until the dart is closed by
`share` (1 all of it, 0.5 half). The slash opens as much, a new dart with
its tip at the same place. Moved whole, the dart keeps its id, its legs
are taken out of the edge it was in (joined straight where they were not
quite as long as each other), and notches and darts beyond it on that
edge come back as far; in part, the dart stays where it was, narrower, and
the new one is the step's id with `.dart` after it. The slash must run
inside the piece, and `to` must be away from the dart.

**Change** `swayback` — `{ "type": "swayback", "centre": [x, y], "side":
[x, y], "by": mm }`: a swayback adjustment on a back. A wedge `by` mm deep
at `centre` (on the centre back) is folded out, tapering to nothing at
`side` (on the side seam): the piece is slashed straight between them and
the smaller part turns about `side` (let out instead where `by` is less
than 0). The centre back must be a seam: a piece cut on the fold there is
refused.

**Change** `shoulder` — `{ "type": "shoulder", "shoulder": [x, y],
"armhole": [x, y], "by": mm }`: broader shoulders (narrower where `by` is
less than 0). The piece is slashed down the grain from `shoulder` (on the
shoulder seam) to the level of `armhole` (on the armhole), then across to
it, and the part with the shoulder's end turns about `armhole` until that
end has moved `by` mm along the shoulder seam. The seam jogs where it was
slashed, to be trued.

**Change** `bustSide`, `waist` and `hip` — `{ "type": "waist", "at": [x,
y], "to": [x, y], "by": mm }`: more room at a side seam (less where `by`
is less than 0). Both points are on the side seam; it moves out square to
the grain by `by` mm at `at` and beyond it (away from `to`), tapering to
nothing at `to`, and the edges meeting it stretch to meet it. For the
bust, `at` is at the bust (the seam moves out from there up to the
underarm) and `to` at the waist; for the waist, `at` is at the waist and
`to` at the hip; for the hip, `at` is at the hip (the seam moves out from
there down to the hem) and `to` at the waist. The three kinds differ only
in what they are called.

**Change** `stretch` — `{ "type": "stretch", "pattern": percent,
"fabric": percent, "bothWays": boolean }`: a piece drawn for a fabric that
stretches `pattern` % (across the grain), cut from one that stretches
`fabric` %, both 0 to 300. The piece is scaled across the grain by (1 +
`pattern`/100) / (1 + `fabric`/100), and down it too where `bothWays`,
about the start of its fold edge, or else the start of its grainline, or
else its middle. A fabric that stretches more makes the piece smaller; its
notches, marks and lines move with it. `bothWays` defaults to false.

**Change** `trueUp` — `{ "type": "trueUp", "edge": id, "straight":
boolean }`: an edge trued, after other steps left jogs or corners in it.
Straight, each part of it between its darts is redrawn as a straight line
between that part's ends. Otherwise each jog or corner along it (where
one curve meets the next at more than 1°, but not at its ends or its
darts) is blended into a curve running 25 mm either side of it (less
where jogs are close together), leaving and arriving as the edge does
there, and the rest of the edge is kept. Its ends and its darts stay as
they are; its notches stay where they were on it.

**AlterLine** — `{ "type": "own", "line": id }`: one of the piece's
internal lines (normally its lengthen or shorten line), straight from its
start to its end; or `{ "type": "through", "from": [x, y], "to": [x, y] }`:
the line through two points in the piece's coordinates.

More kinds of change follow in Phase 4 (§10.3).

### 4.14 Darts (version 5)

```json
{ "id": "d1", "edge": "e4", "from": 120, "to": 196 }
```

| Field | Type | Default | Notes |
|---|---|---|---|
| `edge` | edge id | — | The edge the dart opens in. |
| `from`, `to` | number | — | Where its legs meet the edge: mm **along** the edge from its start, `0 ≤ from < to ≤` the edge length. |

A dart is a wedge folded out of the piece and sewn closed. The outline
draws it open: between `from` and `to` the edge runs in along one leg to
the dart's tip and back out along the other. Sewn, the dart is closed, its
two legs stitched together from `from` and `to` to the tip, so the two
points meet. A seam along the edge sews past it: its length, the
difference between its sides and where its notches meet are measured on
the stitching line with the edge's darts closed (a seam side's `from` and
`to` are still measured along the edge as drawn, darts open).

## 5. Derived geometry (not stored)

These are always computed by the reader and never stored, so they cannot go
stale:

* the stitching line and the cutting line;
* edge and seam lengths;
* notch positions on the cutting line;
* piece area and bounds.

The reference implementation flattens curves to a tolerance of 0.001 mm
when building cutting lines.

## 6. Print settings

```json
"print": {
  "paper": "a4",
  "orientation": "portrait",
  "margin": 10,
  "assembly": "trim",
  "overlap": 15,
  "stitchLine": true,
  "notchStyle": "slit",
  "notchSpacing": 6,
  "testSquare": true,
  "pageMap": true,
  "skipEmptyPages": true,
  "pieces": null,
  "layout": null
}
```

| Field | Default | Notes |
|---|---|---|
| `paper` | `"a4"` | `a4`, `letter`, `a3`, `tabloid`, `a0`, or `{ "width": mm, "height": mm }`. |
| `orientation` | `"portrait"` | |
| `margin` | `10` | Unprinted border on each page, mm. |
| `assembly` | `"trim"` | `trim`: cut away the border on two sides and butt pages together. `overlap`: lay pages over each other by `overlap` mm and match the marks. |
| `stitchLine` | `true` | Also print the stitching line (dashed). |
| `notchStyle` | `"slit"` | `slit` (a short cut into the allowance) or `v` (a small triangle outside the cutting line). |
| `notchSpacing` | `6` | Gap between the marks of double and triple notches, mm. |
| `testSquare`, `pageMap`, `skipEmptyPages` | `true` | |
| `pieces` | `null` | `null` = all pieces, or a list of piece ids to print. |
| `layout` | `null` | `null` = automatic, or `[{ "piece": "p1", "x": 0, "y": 0, "rotation": 90 }]`: each piece turned about its own origin by `rotation` degrees, then moved by `x`, `y` mm on the printed sheet (origin at the top-left of page A1's printed area, y down). Pieces not listed are laid out automatically below the listed ones. |

## 7. View state

`{ "center": [x, y], "zoom": 1.0, "selection": ["p1"] }`. `zoom` is a
multiple of true physical size (`1.0` = actual size on a calibrated screen).
Non-semantic: readers may ignore it.

## 8. Versions and migrations

* `schemaVersion` is an integer. **Every** change to the schema increases
  it.
* A reader supporting version *N* opens files of versions 1…*N* by applying
  migrations one version at a time, and must refuse files newer than *N*
  with a message asking the user to update.
* `extensions` is copied through every migration untouched.
* The reference implementation keeps a fixture file of every past version
  and tests that it migrates to the current one.

| Version | Change |
|---|---|
| 1 | The first. |
| 2 | `measurementSets` (§10.2). Version 1 files have none. |
| 3 | `seams` (§10.4). Earlier files have none. |
| 4 | A measurement set's `age` (§10.2). Earlier files leave it unknown. |
| 5 | A piece's `alterations` and `drawn` (§4.13), and its `darts` (§4.14). Earlier pieces have none. |
| 6 | `fabrics` (§10.5): fabrics of one's own. Earlier files have none. |

## 9. Validation

A file is valid when it matches the JSON Schema and:

1. identifiers are unique across the whole project;
2. every outline is closed and every edge has positive length;
3. every outline is clockwise on screen (positive signed area with y down);
4. `fold` edges are straight;
5. every notch refers to an edge of its own piece and `0 ≤ at ≤` the edge
   length, and every dart to an edge of its own piece with `0 ≤ from < to
   ≤` the edge length;
6. allowances and quantities are non-negative.

Readers should **warn**, not refuse, when an outline crosses itself, since
imported patterns sometimes do; the reference implementation shows such
pieces with a warning and does not alter them.

---

## 10. Planned extensions (*draft*)

### 10.1 Size nests (Phase 2)

```json
"sizeNests": [{
  "id": "sn1",
  "name": "Sizes 34–46",
  "sizes": ["34", "36", "38", "40", "42", "44", "46"],
  "pieces": [{
    "piece": "p1",
    "outlines": { "34": { "start": [0, 0], "edges": [] }, "36": { "start": [0, 0], "edges": [] } }
  }]
}]
```

Every size's outline of a piece has the same edges in the same order (edge
ids match the working piece), so corners correspond across sizes. A piece
generated from a nest records its selection:

```json
"sizing": { "nest": "sn1", "stops": [
  { "edge": "e2", "at": 0,   "size": "38" },
  { "edge": "e2", "at": 380, "size": "42" }
] }
```

One stop means a single size. Several stops blend between sizes along the
outline (e.g. 38 at the bust to 42 at the hip), followed by truing.
`outline` always holds the resulting geometry, so a reader that does not
understand sizing still sees the selected pattern.

### 10.2 Measurement sets (version 2)

No longer a draft: written since schema version 2.

```json
"measurementSets": [{
  "id": "m7", "name": "Me", "figure": "woman", "revision": 2, "takenOn": "2026-10-01", "age": 41,
  "values": { "height": 1650, "bust": 960, "highBust": 880, "underbust": 800, "waist": 780,
              "hip": 1040, "shoulderWidth": 390, "backWaistLength": 410, "armLength": 590, "inseam": 770 },
  "previous": [{ "revision": 1, "takenOn": "2026-06-12", "values": { "bust": 950 } }]
}]
```

| Field | Type | Notes |
|---|---|---|
| `id` | string | Unique in the project. |
| `name` | string | Whose measurements; not empty. |
| `figure` | `"woman"` \| `"man"` | Which measurements apply, and the avatar's shape. Default `"woman"`. |
| `revision` | integer | Counts up each time the person is measured again. Default `1`. |
| `takenOn` | `YYYY-MM-DD` \| null | The day they were taken. |
| `age` | integer \| null | Their age in years, 1 to 120, for the avatar's shape (version 4). |
| `values` | object | Key → millimetres, each more than zero. |
| `previous` | array | Earlier revisions (`revision`, `takenOn`, `values`), newest first. |

Keys are camelCase from this vocabulary, or custom ones starting with `x-`.
"Around" measurements go round the body; the others along it. The
FreeSewing column maps keys to FreeSewing's measurement names.

| Key | Measured | FreeSewing |
|---|---|---|
| `height` | along | — |
| `bust` (chest on a man's figure) | around | `chest` |
| `highBust` | around | `highBust` |
| `underbust` | around | `underbust` |
| `waist` | around | `waist` |
| `highHip` | around | `hips` |
| `hip` | around | `seat` |
| `neck` | around | `neck` |
| `shoulderWidth` | along | `shoulderToShoulder` |
| `shoulderLength` | along | — |
| `acrossFront` | along | — |
| `acrossBack` | along | — |
| `backWaistLength` | along | — |
| `frontWaistLength` | along | `hpsToWaistFront` |
| `bustSpan` | along | `bustSpan` |
| `shoulderToBust` | along | `hpsToBust` |
| `waistToHip` | along | `waistToSeat` |
| `armLength` | along | `shoulderToWrist` |
| `upperArm` | around | `biceps` |
| `wrist` | around | `wrist` |
| `inseam` | along | `inseam` |
| `outseam` | along | `waistToFloor` |
| `crotchDepth` | along | `crotchDepth` |
| `thigh` | around | `upperLeg` |
| `knee` | around | `knee` |
| `calf` | around | — |
| `ankle` | around | `ankle` |

How to take each one is shown in the app beside its box.

### 10.3 More alterations (Phase 4)

Alterations are kept on each piece (§4.13). More kinds may follow, each
a new `change` type with the same rules.

### 10.4 Seams (version 3)

No longer a draft: written since schema version 3.

```json
"seams": [{
  "id": "s12", "name": "Side seam",
  "a": { "piece": "p1", "edge": "e2", "from": null, "to": null, "mirror": false },
  "b": { "piece": "p2", "edge": "e6", "from": null, "to": null, "mirror": false },
  "reversed": false,
  "ease": "even",
  "confirmed": true
}]
```

| Field | Type | Notes |
|---|---|---|
| `id` | string | Unique in the project. |
| `name` | string | Default `""`. |
| `a`, `b` | SeamSide | The two sides sewn together. |
| `a.piece`, `a.edge` | string | A piece and one of its edges. |
| `a.from`, `a.to` | number \| null | The part of the edge sewn, mm along its stitching line from its start. `null`: from its start, to its end. |
| `a.mirror` | boolean | The piece's mirror copy, its other half (a back cut as a pair, sewn to itself at the centre back). Default `false`. |
| `reversed` | boolean | `a`'s start is sewn to `b`'s end, not its start. Default `false`. |
| `ease` | `"even"` | How a difference in length is spread: evenly. Default `"even"`. |
| `confirmed` | boolean | Confirmed by the user. Default `true`. |

Lengths, the difference between the sides and how their notches meet are
always derived, on the stitching line with darts closed (§4.14): where an
outline is the cutting line (allowance included), each edge is first moved
in by its allowance.

A seam must not sew a side to itself, and every side must lie on its edge.
The reference implementation removes a piece's seams with the piece, and
refuses to add or remove a corner on a sewn edge until its seam is unpicked.

### 10.5 Fabrics (version 6)

No longer a draft: written since schema version 6. Fabrics of one's own,
measured at home, to drape the garment in.

```json
"fabrics": [{
  "id": "f1", "name": "Blue linen", "like": "linen",
  "measured": { "weight": 185, "lengthwise": 1.5, "overhang": 6 }
}]
```

| Field | Type | Notes |
|---|---|---|
| `id` | string | Unique in the project. |
| `name` | string | Not empty. |
| `like` | preset | The ready-made fabric it is most like: what was not measured is taken from it. One of `cottonLawn`, `poplin`, `linen`, `chambray`, `denim`, `twill`, `crepe`, `satin`, `jersey`, `ribKnit`, `woolSuiting`, `fleece`. |
| `measured.weight` | number, optional | g/m²: a 10 cm square's weight in grams, times 100. 10 to 2000. |
| `measured.lengthwise` | number, optional | How much longer 10 cm of a strip 5 cm wide, cut along the grain, grows with 500 g hanging from it, %. 0 to 300. |
| `measured.crosswise` | number, optional | The same of a strip cut across the grain, %. |
| `measured.overhang` | number, optional | How far a strip 2.5 cm wide, cut along the grain, slides out over a table's edge before its end droops to 45° below it, cm. 0.5 to 60. |

The reference implementation works out the drape from these as follows.
The weight is the cloth's mass. A strip 5 cm wide pulled by 500 g is
under 98.1 N/m, so the stretch stiffness along (and across) the grain is
98.1 N/m divided by the stretch as a fraction (at least 0.5%: a ruler
can't tell less). The bias's shear stiffness is the preset's, scaled by
the geometric mean of how much stiffer along and across the fabric is
than the preset. The bending stiffness follows Peirce's cantilever: the
bending length is 0.487 times the overhang, and the stiffness is the
weight (N/m²) times its cube. Thickness and friction are the preset's.

Pieces do not yet name a fabric: the garment is draped in the one chosen
on the avatar.

### 10.6 Parametric construction (Phase 5)

`"parametric": { "steps": [ … ] }`: named points and curves defined by
expressions over measurement keys and earlier steps, Seamly2D-style.
