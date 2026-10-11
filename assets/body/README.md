# The body file

`adult.dwbody` is the avatar Drapewright shapes and fits to a person's
measurements (`crates/body`). It holds MakeHuman's body mesh and the shapes
that change it, prepared as in Anny:

* **MakeHuman data via MPFB2: CC0 1.0** (public domain dedication): the
  base mesh, the macro and local shape targets, and the skin weights
  (published in Anny's repository, `src/anny/data/mpfb2`).
* **Anny, Copyright 2025 NAVER Corp., Apache License 2.0**: how the adult
  shapes are blended (each shape weighted by the product of its
  variations' weights), and the mesh edits (two holes capped, unattached
  vertices left out). Drapewright's builder follows that code; no Anny code
  runs in Drapewright.

What the file keeps: the mesh drawn (13 492 vertices, 26 972 triangles)
and 22 joint markers; the adult macro shapes (ages 25 to 90, every muscle,
weight, height, proportions, cup size and firmness; the ethnic shapes in
equal parts, as Anny does by default), stored as the 60 principal
components of figures sampled across those settings (95 in 100 test
figures rebuilt to within 2 mm); 25 local shapes for fitting
(MakeHuman's measure shapes and a few more); and each vertex's body region
from the skin weights. Never Anny's non-commercial `smplx` topology, never
SMPL.

To build it again, from a copy of Anny's repository (github.com/naver/anny):

```bash
cargo run --release -p drapewright-body --example build_body -- ~/Source/third-party/anny/src/anny/data assets/body/adult.dwbody
```

The builder only reads files. To look at a figure, or fit one:

```bash
cargo run --release -p drapewright-body --example picture -- assets/body/adult.dwbody figure.png weight=0.7
```

```bash
cargo run --release -p drapewright-body --example fit -- assets/body/adult.dwbody woman height=168 bust=88 waist=70 hip=96
```
