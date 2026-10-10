# Test fixtures

Pattern files used by Soom's tests. Unless a folder says otherwise, every
file here was drawn for Soom's tests and is dedicated to the public domain
under [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).

Never add a commercial pattern here. Real patterns used for private testing
belong outside the repository (see `docs/open-questions.md`).

| File | What it exercises |
|---|---|
| `svg/inkscape-mm.svg` | Inkscape-style document in millimetres: a layer with a transform, a curved skirt piece, a waistband `<rect>`, a grainline, a drill hole, a notch drawn as a triangle, a title and a loose line |
| `svg/illustrator-pt.svg` | Illustrator export: DOCTYPE, CSS classes, sizes in points written as pixels |
| `svg/unitless.svg` | No units at all: the scale must be confirmed |
| `svg/cut-and-seam.svg` | Both cutting and stitching lines for one piece, with a deeper hem |
| `svg/transforms.svg` | Nested rotate/scale transforms, `<use>` and `<polygon>` |
| `svg/flared-skirt.svg` | A panel flaring 20° with a straight hem, where the corner shapes differ visibly |
| `dxf/aama-skirt.dxf` | AAMA/ASTM-style garment CAD file, `UNITS: METRIC`: a skirt front in a block with cutting line (layer 1), stitching line (14) with a 40 mm hem and 15 mm seams, turn points, a notch, grainline, fold (mirror) line, drill hole and piece texts; and a waistband with only its cutting line, so allowance is included |
| `dxf/generic-mm.dxf` | Ordinary CAD drawing in millimetres (`$INSUNITS` 4): a pocket with rounded corners from polyline bulges, a flap drawn as four separate lines (one reversed) named by a text inside it, a grainline layer, a drill circle and an unsupported hatch |
| `dxf/inches.dxf` | `$INSUNITS` 1 (inches): a 4″ × 2″ cuff |
| `dxf/unitless.dxf` | No header at all: the scale must be confirmed |

The DXF files are written by `dxf/make_fixtures.py` (run it from the
repository root after changing it).
