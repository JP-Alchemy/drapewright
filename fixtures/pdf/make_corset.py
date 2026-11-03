#!/usr/bin/env python3
"""Writes corset-boning.pdf, a corset pattern's boning and lacing as indie
patterns print them. CC0: drawn for Drapewright's tests.

Run from the repository root: python3 fixtures/pdf/make_corset.py

One size, drawn for one side (each panel cut as a pair): a centre front, a
side panel and a centre back, their seams curved in 10 mm at the waist,
their centres straight, their tops sloping up towards the back, each with
its waist line across it. The
centre front has a boning channel down its middle: two dashed lines, the
word "Boning" printed between them. The side panel's bone runs down its
right seam to a short line marked "Stop / Boning / Here" (each word its own
line of print). The centre back has eyelets down it, 15 mm in from its
centre, a "Back Lacing Bone" channel either side of them. A 10 cm test
square.
"""
import math
import os

HERE = os.path.dirname(os.path.abspath(__file__))

PT = 72 / 25.4
W, H = 540.0, 400.0  # page, mm


def pt(x, y):
    """Millimetres from the top-left corner to PDF points (y up)."""
    return f"{x * PT:.3f} {(H - y) * PT:.3f}"


def text(s, x, y, size, rotate=False):
    """Text at (x, y) mm (baseline start); rotated 90° to read upwards."""
    tx, ty = (x * PT, (H - y) * PT)
    m = f"0 1 -1 0 {tx:.3f} {ty:.3f}" if rotate else f"1 0 0 1 {tx:.3f} {ty:.3f}"
    return f"BT /F1 {size} Tf {m} Tm ({s}) Tj ET"


def side(x, y0, y1, inwards):
    """A side from (x, y0) to (x, y1): straight, or a seam curved in at the
    waist by `inwards` (towards larger x where positive)."""
    if inwards == 0:
        return f"{pt(x, y1)} l"
    k = inwards * 4 / 3
    third = (y1 - y0) / 3
    return f"{pt(x + k, y0 + third)} {pt(x + k, y0 + 2 * third)} {pt(x, y1)} c"


def panel(x0, x1, tops, bottoms, left, right):
    """A panel's outline, clockwise: its top (sloping from `tops[0]` at its
    left to `tops[1]`), its right side, its bottom and its left side, each
    side shaped in by `left` and `right` (0: straight, a centre)."""
    return " ".join(
        [
            f"{pt(x0, tops[0])} m {pt(x1, tops[1])} l",
            side(x1, tops[1], bottoms[1], -right),
            f"{pt(x0, bottoms[0])} l",
            side(x0, bottoms[0], tops[0], left),
            "h S",
        ]
    )


def dashed(x, y0, y1):
    return f"[2 1.5] 0 d {pt(x, y0)} m {pt(x, y1)} l S [] 0 d"


def grainline(x, y0, y1):
    head = 4.0
    return "\n".join(
        [
            f"{pt(x, y0)} m {pt(x, y1)} l S",
            f"{pt(x, y0)} m {pt(x - 1.6, y0 + head)} l {pt(x + 1.6, y0 + head)} l h f",
            f"{pt(x, y1)} m {pt(x - 1.6, y1 - head)} l {pt(x + 1.6, y1 - head)} l h f",
        ]
    )


def circle(cx, cy, r):
    """A circle of four Bézier quarters."""
    k = 0.5523 * r
    return (
        f"{pt(cx + r, cy)} m "
        f"{pt(cx + r, cy + k)} {pt(cx + k, cy + r)} {pt(cx, cy + r)} c "
        f"{pt(cx - k, cy + r)} {pt(cx - r, cy + k)} {pt(cx - r, cy)} c "
        f"{pt(cx - r, cy - k)} {pt(cx - k, cy - r)} {pt(cx, cy - r)} c "
        f"{pt(cx + k, cy - r)} {pt(cx + r, cy - k)} {pt(cx + r, cy)} c S"
    )


def waist(x0, x1, y):
    return f"{pt(x0, y)} m {pt(x1, y)} l S\n" + text("WAIST", (x0 + x1) / 2 - 6.0, y - 2.0, 8)


def content():
    parts = ["0 0 0 RG 0 0 0 rg 0.6 w"]
    # The centre front: its left side the centre, a channel down its middle.
    parts += [
        panel(40.0, 110.0, (50.0, 40.0), (350.0, 340.0), 0.0, 10.0),
        waist(40.0, 110.0, 200.0),
        dashed(66.0, 55.0, 325.0),
        dashed(74.0, 55.0, 325.0),
        text("Boning", 71.5, 120.0, 9, rotate=True),
        grainline(95.0, 80.0, 300.0),
        text("FRONT CENTER", 44.0, 260.0, 9),
        text("Cut 2", 44.0, 266.0, 8),
    ]
    # The side panel: its bone down its right seam, stopping at the mark.
    parts += [
        panel(160.0, 240.0, (40.0, 30.0), (340.0, 330.0), 10.0, 10.0),
        waist(160.0, 240.0, 200.0),
        f"{pt(200.0, 300.0)} m {pt(237.0, 300.0)} l S",
        text("Stop", 214.0, 284.0, 9),
        text("Boning", 212.0, 289.0, 9),
        text("Here", 214.0, 294.0, 9),
        grainline(180.0, 80.0, 300.0),
        text("FRONT SIDE", 164.0, 150.0, 9),
        text("Cut 2", 164.0, 156.0, 8),
    ]
    # The centre back: its right side the centre, eyelets down it between
    # two lacing bones.
    parts += [
        panel(290.0, 360.0, (30.0, 20.0), (330.0, 320.0), 10.0, 0.0),
        waist(290.0, 340.0, 200.0),
        dashed(333.0, 50.0, 330.0),
        dashed(339.0, 50.0, 330.0),
        dashed(351.0, 50.0, 330.0),
        dashed(357.0, 50.0, 330.0),
        text("Back Lacing Bone", 337.5, 160.0, 7, rotate=True),
        text("Back Lacing Bone", 355.5, 160.0, 7, rotate=True),
        grainline(305.0, 80.0, 300.0),
        text("BACK CENTER", 292.0, 260.0, 9),
        text("Cut 2", 292.0, 266.0, 8),
    ]
    parts += [circle(345.0, 60.0 + 26.0 * k, 2.5) for k in range(11)]
    # The test square, its size inside it.
    parts += [
        f"{pt(420.0, 140.0)} {100 * PT:.3f} {100 * PT:.3f} re S",
        text("10 cm", 460.0, 92.0, 10),
    ]
    return "\n".join(parts)


def pdf(objects):
    out = bytearray(b"%PDF-1.5\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += f"{i} 0 obj\n".encode() + body + b"\nendobj\n"
    xref = len(out)
    out += f"xref\n0 {len(objects) + 1}\n0000000000 65535 f \n".encode()
    for o in offsets:
        out += f"{o:010d} 00000 n \n".encode()
    out += f"trailer\n<< /Size {len(objects) + 1} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    return bytes(out)


def stream(data):
    data = data.encode("latin-1")
    return b"<< /Length " + str(len(data)).encode() + b" >>\nstream\n" + data + b"\nendstream"


if __name__ == "__main__":
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W * PT:.3f} {H * PT:.3f}] "
        f"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".encode(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        stream(content()),
    ]
    data = pdf(objects)
    with open(os.path.join(HERE, "corset-boning.pdf"), "wb") as f:
        f.write(data)
    print("wrote corset-boning.pdf", len(data), "bytes")
