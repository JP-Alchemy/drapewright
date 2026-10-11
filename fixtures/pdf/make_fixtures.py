#!/usr/bin/env python3
"""Writes the multi-size PDF test fixtures in this folder. CC0: drawn for
Drapewright's tests.

Run from the repository root: python3 fixtures/pdf/make_fixtures.py

Both files hold one pattern in sizes 8, 10 and 12, nested as printed
patterns are: a front bodice cut on the fold, and a sleeve. Each size's
cutting line is drawn whole. Grainlines (with arrowheads), names, cutting
instructions, "place on fold" and a 10 cm test square are shared by all
sizes.

- sizes-layers.pdf: each size on its own layer (optional content group),
  all in black, as layered indie patterns are.
- sizes-colours.pdf: no layers; each size in its own colour, with a legend.
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))

PT = 72 / 25.4
W, H = 600.0, 500.0  # page, mm
SIZES = [("8", 0.95), ("10", 1.0), ("12", 1.05)]
COLOURS = {"8": (0.85, 0.1, 0.1), "10": (0.1, 0.55, 0.15), "12": (0.1, 0.25, 0.85)}


def pt(x, y):
    """Millimetres from the top-left corner to PDF points (y up)."""
    return f"{x * PT:.3f} {(H - y) * PT:.3f}"


def scale_about(points, anchor, k):
    ax, ay = anchor
    return [(ax + (x - ax) * k, ay + (y - ay) * k) for x, y in points]


# Outlines as moves: ("L", p) or ("C", c1, c2, p), from a start point; size 10.
BODICE = {
    "anchor": (40.0, 40.0),
    "start": (40.0, 60.0),
    "path": [
        ("C", (70.0, 60.0), (100.0, 50.0), (110.0, 40.0)),  # neckline
        ("L", (190.0, 60.0)),  # shoulder
        ("C", (185.0, 110.0), (180.0, 160.0), (200.0, 170.0)),  # armhole
        ("L", (195.0, 330.0)),  # side seam
        ("C", (150.0, 340.0), (90.0, 342.0), (40.0, 340.0)),  # hem
        ("L", (40.0, 60.0)),  # centre front, on the fold
    ],
}
SLEEVE = {
    "anchor": (340.0, 40.0),
    "start": (260.0, 120.0),
    "path": [
        ("C", (290.0, 110.0), (300.0, 40.0), (340.0, 40.0)),
        ("C", (380.0, 40.0), (390.0, 110.0), (420.0, 120.0)),
        ("L", (405.0, 330.0)),
        ("L", (275.0, 330.0)),
        ("L", (260.0, 120.0)),
    ],
}


def outline(shape, k):
    """Path operators for a shape at grade k."""
    pts_of = lambda ps: scale_about(ps, shape["anchor"], k)
    (sx, sy), = pts_of([shape["start"]])
    ops = [f"{pt(sx, sy)} m"]
    for move in shape["path"]:
        ps = pts_of(list(move[1:]))
        if move[0] == "L":
            ops.append(f"{pt(*ps[0])} l")
        else:
            ops.append(f"{pt(*ps[0])} {pt(*ps[1])} {pt(*ps[2])} c")
    ops.append("h S")
    return "\n".join(ops)


def grainline(x, y0, y1):
    """A vertical grainline with filled arrowheads at both ends."""
    head = 4.0
    return "\n".join(
        [
            f"{pt(x, y0)} m {pt(x, y1)} l S",
            f"{pt(x, y0)} m {pt(x - 1.6, y0 + head)} l {pt(x + 1.6, y0 + head)} l h f",
            f"{pt(x, y1)} m {pt(x - 1.6, y1 - head)} l {pt(x + 1.6, y1 - head)} l h f",
        ]
    )


def text(s, x, y, size, rotate=False):
    """Text at (x, y) mm (baseline start); rotated 90° to read upwards."""
    tx, ty = (x * PT, (H - y) * PT)
    m = f"0 1 -1 0 {tx:.3f} {ty:.3f}" if rotate else f"1 0 0 1 {tx:.3f} {ty:.3f}"
    s = s.replace("\\", "\\\\").replace("(", "\\(").replace(")", "\\)")
    return f"BT /F1 {size} Tf {m} Tm ({s}) Tj ET"


def shared():
    """Everything all sizes share."""
    return "\n".join(
        [
            "0 0 0 RG 0 0 0 rg 0.6 w",
            grainline(110.0, 140.0, 280.0),
            grainline(340.0, 150.0, 300.0),
            text("FRONT BODICE", 60.0, 200.0, 14),
            text("Cut 1 on fold", 60.0, 210.0, 10),
            text("SLEEVE", 312.0, 215.0, 14),
            text("Cut 2", 318.0, 225.0, 10),
            text("PLACE ON FOLD", 48.0, 250.0, 9, rotate=True),
            # The test square, with its size inside.
            f"{pt(470.0, 370.0)} {100 * PT:.3f} {-100 * PT:.3f} re S",
            text("10 cm", 510.0, 422.0, 10),
            text("Test pattern - sizes 8, 10, 12", 20.0, 480.0, 12),
        ]
    )


def pdf(objects):
    """A PDF from numbered object bodies (1-based), catalog first."""
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


def stream(content):
    data = content.encode("latin-1")
    return b"<< /Length " + str(len(data)).encode() + b" >>\nstream\n" + data + b"\nendstream"


def write(name, data):
    with open(os.path.join(HERE, name), "wb") as f:
        f.write(data)
    print("wrote", name, len(data), "bytes")


def layered():
    # 1 catalog, 2 pages, 3 page, 4 font, 5–7 size layers, 8 content
    groups = []
    for label, k in SIZES:
        groups.append(f"/OC /S{label} BDC\n0 0 0 RG 0.8 w\n{outline(BODICE, k)}\n{outline(SLEEVE, k)}\nEMC")
    content = "\n".join(groups + [shared()])
    ocgs = " ".join(f"{5 + i} 0 R" for i in range(len(SIZES)))
    props = " ".join(f"/S{label} {5 + i} 0 R" for i, (label, _) in enumerate(SIZES))
    objects = [
        f"<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [{ocgs}] /D << /Order [{ocgs}] >> >> >>".encode(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W * PT:.3f} {H * PT:.3f}] "
        f"/Resources << /Font << /F1 4 0 R >> /Properties << {props} >> >> /Contents 8 0 R >>".encode(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    ]
    for label, _ in SIZES:
        objects.append(f"<< /Type /OCG /Name (Size {label}) >>".encode())
    objects.append(stream(content))
    write("sizes-layers.pdf", pdf(objects))


def coloured():
    parts = []
    for label, k in SIZES:
        r, g, b = COLOURS[label]
        parts.append(f"{r} {g} {b} RG 0.8 w\n{outline(BODICE, k)}\n{outline(SLEEVE, k)}")
    # The legend: a sample of each line, then its size.
    legend = [text("Sizes", 20.0, 395.0, 11)]
    for i, (label, _) in enumerate(SIZES):
        y = 405.0 + i * 10.0
        r, g, b = COLOURS[label]
        legend.append(f"{r} {g} {b} RG 0.8 w {pt(20.0, y)} m {pt(45.0, y)} l S")
        legend.append(f"0 0 0 rg " + text(label, 50.0, y + 1.5, 10))
    content = "\n".join(parts + [shared()] + legend)
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        f"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {W * PT:.3f} {H * PT:.3f}] "
        f"/Resources << /Font << /F1 4 0 R >> >> /Contents 5 0 R >>".encode(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        stream(content),
    ]
    write("sizes-colours.pdf", pdf(objects))


if __name__ == "__main__":
    layered()
    coloured()
