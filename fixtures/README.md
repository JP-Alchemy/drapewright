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
| `pdf/freesewing-sasha-a0.pdf` | The FreeSewing dress above, printed by Drapewright on A0 for a copy shop: a cover page (test squares, page map) and the sheet on two pages, each drawing every piece it touches, clipped. Stitching lines dashed, notches cut into the cutting lines, grainlines and fold brackets with arrowheads, names and cutting printed on each piece |
| `pdf/freesewing-sasha-example-a0.pdf` | The app's sample dress: FreeSewing's Sasha drafted by Drapewright (**A FreeSewing design, made to measure**) to the example figure's measurements, without its pockets and with its back neckline widened as its front's is, then printed on A0 for a copy shop as above: five pieces, uniform 1 cm allowance, no hems marked. Find seams sews it whole (`crates/io/tests/sample_dress.rs`) |
| `pdf/sizes-layers.pdf` | A front bodice (cut on the fold) and a sleeve in sizes 8, 10 and 12, nested, each size on its own layer ("Size 8"…), all in black; shared grainlines with arrowheads, names, cutting, "PLACE ON FOLD" and a 10 cm test square |
| `pdf/sizes-colours.pdf` | The same with no layers: each size in its own colour, with a legend (a sample of each line beside its size) |
| `pdf/sizes-layers-aes128.pdf`, `pdf/sizes-layers-aes256.pdf` | `sizes-layers.pdf` "protected" as Acrobat does it: AES-128 (security handler revision 4) and AES-256 (revision 6), owner password only, so they open without one |
| `pdf/sizes-layers-password.pdf` | The same with a password needed to open it ("open sesame"): refused, with a plain message |
| `pdf/freesewing-sasha-a0-protected.pdf` | The same, rewritten by Ghostscript 10 as a PDF 1.7 with object streams and a cross-reference stream, and "protected" (RC4 128-bit, owner password only: it opens without one; printing allowed, copying not), as many bought patterns are |
| `drapewright/skirt.drapewright` | A Drapewright project: the skirt the drape tests use, a front cut on the fold and a back cut as a pair, with its side seams and centre back seam sewn. Written by `cargo test -p drapewright-sim --test fixture` |
| `drapewright/blouse.drapewright` | The blouse the drape tests use: a loose bodice, front and back each cut on the fold, and a short sleeve cut as a pair, sewn at the shoulders, sides and underarms and into the armholes (each half of the sleeve cap sewn to one armhole, eased in). Written by the same test |
| `drapewright/lined-skirt.drapewright` | The skirt lined: a front and a back lining 3 cm shorter, cut from lining, sewn at their sides and centre back and to the skirt at the waist. Written by the same test |
| `photo/piece-and-a4.png` | A photo, drawn: a skirt panel (27 × 55.5 cm, a grainline printed on it) and a sheet of A4 lying on a green table, seen at a slant and lit unevenly. The sheet's corners are at (205, 140), (70, 150), (55, 352) and (222, 335) px. Written by `cargo test -p drapewright-io --test photo` |
| `drapewright/shirt.drapewright` | A yoked shirt the drape tests use: a back gathered to a yoke, fronts buttoned down the front, gathered sleeves with cuffs, and a back neck facing, sewn by the seams Find seams suggests. Written by the same test |

The DXF files are written by `dxf/make_fixtures.py`, the `sizes-*` PDFs by
`pdf/make_fixtures.py` and their encrypted copies by `pdf/protect.py`, which
needs the `openssl` command (run them from the repository root after
changing them; poppler's `pdfinfo` and `pdftotext` confirm the encrypted
files are valid).

The `freesewing-*` files were made with FreeSewing 4.10.2
(`@freesewing/sasha`, `sa: 10` and `sa: 0`); the PDFs are Drapewright's
prints of the SVG with 10 mm allowance (the protected one by `gs -sDEVICE=pdfwrite
-dCompatibilityLevel=1.7 -sOwnerPassword=… -sUserPassword= -dEncryptionR=3
-dKeyLength=128 -dPermissions=-3900`). FreeSewing is MIT licensed,
copyright (c) 2022 Joost De Cock. Its licence is in
`svg/LICENSE-freesewing`, copied from codeberg.org/freesewing/freesewing.
