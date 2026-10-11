# The Drapewright project format

Status: **schema version 1** (Phase 1). Sections marked *draft* describe
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
  "schemaVersion": 1,
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
  "view": { /* ViewState */ },
  "extensions": {}
}
```

| Field | Type | Notes |
|---|---|---|
| `format` | `"drapewright-project"` | Identifies the file type. |
| `schemaVersion` | integer | `1` for this document. See §8. |
| `generator` | string | Free text naming the program that wrote the file. |
| `meta.id` | string | Stable project identifier. |
| `meta.name` | string | Shown to the user. |
| `meta.created`, `meta.modified` | RFC 3339 string | UTC. |
| `meta.notes` | string | Free text. Default `""`. |
| `settings.displayUnits` | `"mm"` \| `"cm"` \| `"in"` | Default `"cm"`. |
| `pieces` | array of Piece | Order is the order shown in piece lists. |
| `print` | PrintSettings | Optional; defaults in §6. |
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
  "marks": [ /* Mark */ ],
  "lines": [ /* InternalLine */ ],
  "labels": [ /* Label */ ],
  "placement": { "x": 0, "y": 0, "rotation": 0 },
  "source": { /* Source */ }
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
| `marks` | array of Mark | `[]` | §4.8. |
| `lines` | array of InternalLine | `[]` | §4.9. |
| `labels` | array of Label | `[]` | §4.10. |
| `placement` | Placement | `{ "x": 0, "y": 0, "rotation": 0 }` | §4.11. |
| `source` | Source \| null | `null` | §4.12. |

All piece geometry (outline, notches, marks, lines, labels, grainline) is
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
| `format` | `svg`, `dxf`, `pdf`, `project` (taken from another project), `drafted`. |
| `file`, `sha256` | Original file name and hash; the file itself is in `sources/`. |
| `ref` | Element id, layer or block name inside the source. |
| `scale.mmPerUnit` | Millimetres per source unit used at import. |
| `scale.basis` | How the scale was established: `declaredUnits` (file states physical units), `assumed96dpi`, `assumed72dpi`, `assumed90dpi` (drawings without units), `assumedMm`, `assumedCm`, `assumedInches` (CAD drawings without units), `measured` (the user measured a known length). |
| `scale.confirmed` | Whether the user confirmed the scale. Imports never proceed with an unconfirmed guess. |

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

## 9. Validation

A file is valid when it matches the JSON Schema and:

1. identifiers are unique across the whole project;
2. every outline is closed and every edge has positive length;
3. every outline is clockwise on screen (positive signed area with y down);
4. `fold` edges are straight;
5. every notch refers to an edge of its own piece and `0 ≤ at ≤` the edge
   length;
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

### 10.2 Measurement sets (Phase 3)

```json
"measurementSets": [{
  "id": "ms1", "name": "Me", "revision": 3, "takenOn": "2026-10-01",
  "values": { "height": 1650, "bust": 960, "highBust": 880, "underbust": 800, "waist": 780,
              "hip": 1040, "shoulderWidth": 390, "backWaistLength": 410, "armLength": 590, "inseam": 770 },
  "previous": [{ "revision": 2, "takenOn": "2026-06-12", "values": { "bust": 950 } }]
}]
```

Keys come from a documented vocabulary (each with how-to-measure text),
with a mapping table to Seamly2D/Valentina and FreeSewing names. Custom keys
start with `x-`.

### 10.3 Alteration stack (Phase 4)

```json
"alterations": [
  { "id": "a1", "kind": "lengthenShorten", "enabled": true, "pieces": ["p1", "p2"], "line": "l1", "amount": 25 },
  { "id": "a2", "kind": "fullBust", "enabled": true, "piece": "p1", "amount": 15, "apex": [210, 260] }
]
```

Applied in order on top of the sized outline. The piece then also carries
`base` (the outline before alterations); `outline` is the result. Kinds:
`lengthenShorten`, `fullBust`, `smallBust`, `swayback`, `broadShoulder`,
`narrowShoulder`, `waist`, `hip`, `slashSpread`, `pivot`, `ease`, `trueSeam`.

### 10.4 Seams (Phase 3)

```json
"seams": [{
  "id": "s1", "name": "Side seam",
  "a": { "piece": "p1", "edge": "e2" },
  "b": { "piece": "p2", "edge": "e6", "reversed": true },
  "ease": "even",
  "confirmed": true
}]
```

Either side may be a range of an edge (`"from": mm, "to": mm`). Lengths and
mismatches are always derived.

### 10.5 Fabrics (Phase 3)

```json
"fabrics": [{ "id": "f1", "name": "Mid-weight linen", "preset": "linen", "weight": 185,
              "stretch": { "lengthwise": 2, "crosswise": 3 }, "texture": "textures/f1.jpg" }]
```

Pieces reference a fabric with `"fabric": "f1"`. `weight` is g/m²;
`stretch` is percent under the calibration test load.

### 10.6 Parametric construction (Phase 5)

`"parametric": { "steps": [ … ] }`: named points and curves defined by
expressions over measurement keys and earlier steps, Seamly2D-style.
