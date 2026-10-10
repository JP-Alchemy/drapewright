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
