#!/usr/bin/env python3
"""Writes the DXF test fixtures in this folder. CC0: drawn for Soom's tests.

Run from the repository root: python3 fixtures/dxf/make_fixtures.py
DXF stores y pointing up; Soom flips it on import.
"""
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))


def pairs(*items):
    return "".join(f"{code}\n{value}\n" for code, value in items)


def header(insunits=None):
    if insunits is None:
        return ""
    return pairs((0, "SECTION"), (2, "HEADER"), (9, "$INSUNITS"), (70, insunits), (0, "ENDSEC"))


def lwpolyline(layer, vertices, closed=True):
    """vertices: (x, y) or (x, y, bulge)."""
    out = pairs((0, "LWPOLYLINE"), (8, layer), (90, len(vertices)), (70, 1 if closed else 0))
    for v in vertices:
        out += pairs((10, f"{v[0]:.4f}"), (20, f"{v[1]:.4f}"))
        if len(v) > 2:
            out += pairs((42, f"{v[2]:.6f}"))
    return out


def polyline(layer, points, closed=True):
    """Old-style POLYLINE with VERTEX entities, as AAMA files use."""
    out = pairs((0, "POLYLINE"), (8, layer), (66, 1), (70, 1 if closed else 0))
    for x, y in points:
        out += pairs((0, "VERTEX"), (8, layer), (10, f"{x:.4f}"), (20, f"{y:.4f}"))
    return out + pairs((0, "SEQEND"), (8, layer))


def line(layer, a, b):
    return pairs((0, "LINE"), (8, layer), (10, a[0]), (20, a[1]), (11, b[0]), (21, b[1]))


def point(layer, p):
    return pairs((0, "POINT"), (8, layer), (10, p[0]), (20, p[1]))


def text(layer, p, value, height=5):
    return pairs((0, "TEXT"), (8, layer), (10, p[0]), (20, p[1]), (40, height), (1, value))


def circle(layer, c, r):
    return pairs((0, "CIRCLE"), (8, layer), (10, c[0]), (20, c[1]), (40, r))


def section(name, body):
    return pairs((0, "SECTION"), (2, name)) + body + pairs((0, "ENDSEC"))


def write(name, body):
    with open(os.path.join(HERE, name), "w", newline="\n") as f:
        f.write(body + pairs((0, "EOF")))


def intersect(p, d, q, e):
    den = d[0] * e[1] - d[1] * e[0]
    w = (q[0] - p[0], q[1] - p[1])
    s = (w[0] * e[1] - w[1] * e[0]) / den
    return (p[0] + s * d[0], p[1] + s * d[1])


def aama_skirt():
    # Stitching line, y up: centre front on x = 0 (the fold), waist at the top.
    sew = [(0, 600), (200, 600), (250, 0), (0, 0)]
    side_dir = (50, -600)
    n = math.hypot(*side_dir)
    side_out = (600 / n, 50 / n)  # outward normal of the side seam (to the right)
    side_cut = (200 + 15 * side_out[0], 600 + 15 * side_out[1])
    # Cutting line: 15 mm at the waist and side seam, 40 mm hem, none at the fold.
    waist_cut = intersect((0, 615), (1, 0), side_cut, side_dir)
    hem_cut = intersect((0, -40), (1, 0), side_cut, side_dir)
    cut = [(0, 615), waist_cut, hem_cut, (0, -40)]
    mid = (225 + 15 * side_out[0], 300 + 15 * side_out[1])
    block = pairs((0, "BLOCK"), (8, "0"), (2, "FRONT"), (70, 0), (10, 0), (20, 0))
    block += polyline("1", cut)
    block += polyline("14", sew)
    block += "".join(point("2", p) for p in cut)
    block += line("4", mid, (mid[0] - 6 * side_out[0], mid[1] - 6 * side_out[1]))
    block += line("7", (100, 100), (100, 500))
    block += line("6", (0, 0), (0, 600))
    block += point("13", (60, 450))
    block += text("1", (40, 300), "Piece Name: SKIRT FRONT")
    block += text("1", (40, 290), "Quantity: 1")
    block += text("1", (40, 280), "Size: 38")
    block += pairs((0, "ENDBLK"), (8, "0"))
    # A waistband with only its cutting line, as many CAD exports have:
    # allowance included, width not stated. Centre back on the fold.
    band = [(0, 0), (380, 0), (380, 80), (0, 80)]
    block += pairs((0, "BLOCK"), (8, "0"), (2, "WAISTBAND"), (70, 0), (10, 0), (20, 0))
    block += polyline("1", band)
    block += "".join(point("2", p) for p in band)
    block += line("6", (0, 0), (0, 80))
    block += line("7", (40, 40), (340, 40))
    block += text("1", (60, 50), "Piece Name: WAISTBAND")
    block += text("1", (60, 30), "Quantity: 1")
    block += pairs((0, "ENDBLK"), (8, "0"))
    entities = text("0", (0, 700), "ANSI/AAMA VERSION: 1.0.0") + text("0", (0, 690), "UNITS: METRIC")
    entities += pairs((0, "INSERT"), (8, "0"), (2, "FRONT"), (10, 0), (20, 0))
    entities += pairs((0, "INSERT"), (8, "0"), (2, "WAISTBAND"), (10, 350), (20, 0))
    write("aama-skirt.dxf", section("BLOCKS", block) + section("ENTITIES", entities))


def generic_mm():
    b = -math.tan(math.radians(22.5))  # clockwise quarter circles
    pocket = [(0, 160), (140, 160), (140, 20, b), (120, 0), (20, 0, b), (0, 20)]
    ents = lwpolyline("POCKET", pocket)
    ents += line("GRAINLINE", (70, 30), (70, 130))
    ents += circle("0", (70, 140), 2)
    # A flap drawn as four separate lines, one of them backwards.
    ents += line("0", (200, 0), (340, 0)) + line("0", (340, 0), (340, 50))
    ents += line("0", (340, 50), (200, 50)) + line("0", (200, 0), (200, 50))
    ents += text("0", (230, 25), "Pocket flap", height=8)
    ents += pairs((0, "HATCH"), (8, "0"))
    write("generic-mm.dxf", header(4) + section("ENTITIES", ents))


def inches():
    write("inches.dxf", header(1) + section("ENTITIES", lwpolyline("CUFF", [(0, 0), (4, 0), (4, 2), (0, 2)])))


def unitless():
    write("unitless.dxf", section("ENTITIES", lwpolyline("COLLAR", [(0, 0), (100, 0), (100, 50), (0, 50)])))


if __name__ == "__main__":
    aama_skirt()
    generic_mm()
    inches()
    unitless()
