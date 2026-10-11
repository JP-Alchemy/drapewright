# Test fixtures

Pattern files used by Drapewright's tests. Unless a folder says otherwise, every
file here was drawn for Drapewright's tests and is dedicated to the public domain
under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).

Never add a commercial pattern here. Real patterns used for private testing
belong outside the repository (see `docs/open-questions.md`). To see how a
folder of them imports (pieces, scale, notes, timing) without opening the app:

```bash
cargo run --release -p drapewright-io --example import_report -- ~/Source/drapewright-test-patterns
```

| File | What it exercises |
|---|---|
| `svg/inkscape-mm.svg` | Inkscape-style document in millimetres: a layer with a transform, a curved skirt piece, a waistband `<rect>`, a grainline, a drill hole, a notch drawn as a triangle, a title and a loose line |
| `svg/illustrator-pt.svg` | Illustrator export: DOCTYPE, CSS classes, sizes in points written as pixels |
| `svg/unitless.svg` | No units at all: the scale must be confirmed |
| `svg/cut-and-seam.svg` | Both cutting and stitching lines for one piece, with a deeper hem |
| `svg/transforms.svg` | Nested rotate/scale transforms, `<use>` and `<polygon>` |
| `svg/flared-skirt.svg` | A panel flaring 20° with a straight hem, where the corner shapes differ visibly |
| `svg/freesewing-sasha-sa10.svg` | **Generated with FreeSewing** (MIT; see below): the Sasha princess-seam skater dress at FreeSewing's standard size 36 (`cisFemaleAdult36`), with 10 mm seam allowance. Piece groups named `fs-stack-…`; open cutting lines along the fold (front) and at a corner (the others); `#notch`/`#bnotch` symbols; a cut-on-fold-and-grainline bracket; cutting texts |
| `svg/freesewing-sasha-no-allowance.svg` | The same without seam allowance: the fold comes from the cut-on-fold bracket alone |
| `dxf/aama-skirt.dxf` | AAMA/ASTM-style garment CAD file, `UNITS: METRIC`: a skirt front in a block with cutting line (layer 1), stitching line (14) with a 40 mm hem and 15 mm seams, turn points, a notch, grainline, fold (mirror) line, drill hole and piece texts; and a waistband with only its cutting line, so allowance is included |
| `dxf/generic-mm.dxf` | Ordinary CAD drawing in millimetres (`$INSUNITS` 4): a pocket with rounded corners from polyline bulges, a flap drawn as four separate lines (one reversed) named by a text inside it, a grainline layer, a drill circle and an unsupported hatch |
| `dxf/inches.dxf` | `$INSUNITS` 1 (inches): a 4″ × 2″ cuff |
| `dxf/unitless.dxf` | No header at all: the scale must be confirmed |

The DXF files are written by `dxf/make_fixtures.py` (run it from the
repository root after changing it).

The two `freesewing-*` files were made with FreeSewing 4.10.2
(`@freesewing/sasha`, `sa: 10` and `sa: 0`). FreeSewing is MIT licensed,
copyright (c) 2022 Joost De Cock. Its licence is in
`svg/LICENSE-freesewing`, copied from codeberg.org/freesewing/freesewing.
