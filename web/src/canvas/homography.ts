// A projective map of the plane (a homography): what a projector does to the
// cutting mat when it is not square on to it. Four points and where they
// land fix it.

export type Pt = readonly [number, number]

/** The map, row by row; the last entry is 1. */
export type Homography = readonly [
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
  number,
]

/**
 * The map taking each of `from` to the matching one of `to`; null if three
 * of either lie on a line.
 */
export function solveHomography(
  from: readonly [Pt, Pt, Pt, Pt],
  to: readonly [Pt, Pt, Pt, Pt],
): Homography | null {
  // x' = (a x + b y + c) / (g x + h y + 1), y' = (d x + e y + f) / (g x + h y + 1):
  // two equations from each point, eight unknowns.
  const rows: number[][] = []
  for (let i = 0; i < 4; i++) {
    const [x, y] = from[i]!
    const [u, v] = to[i]!
    rows.push([x, y, 1, 0, 0, 0, -u * x, -u * y, u])
    rows.push([0, 0, 0, x, y, 1, -v * x, -v * y, v])
  }
  // Gaussian elimination with partial pivoting.
  for (let c = 0; c < 8; c++) {
    let pivot = c
    for (let r = c + 1; r < 8; r++) {
      if (Math.abs(rows[r]![c]!) > Math.abs(rows[pivot]![c]!)) pivot = r
    }
    if (Math.abs(rows[pivot]![c]!) < 1e-12) return null
    ;[rows[c], rows[pivot]] = [rows[pivot]!, rows[c]!]
    const row = rows[c]!
    for (let r = 0; r < 8; r++) {
      if (r === c) continue
      const f = rows[r]![c]! / row[c]!
      if (f === 0) continue
      for (let k = c; k < 9; k++) rows[r]![k]! -= f * row[k]!
    }
  }
  const h = rows.map((row, c) => row[8]! / row[c]!)
  const map = [h[0]!, h[1]!, h[2]!, h[3]!, h[4]!, h[5]!, h[6]!, h[7]!, 1] as const
  return map.every(Number.isFinite) ? map : null
}

/** Where `p` lands. */
export function mapPoint(m: Homography, [x, y]: Pt): [number, number] {
  const w = m[6] * x + m[7] * y + m[8]
  return [(m[0] * x + m[1] * y + m[2]) / w, (m[3] * x + m[4] * y + m[5]) / w]
}

/** The map undone; null if it cannot be. */
export function invert(m: Homography): Homography | null {
  const [a, b, c, d, e, f, g, h, i] = m
  const A = e * i - f * h
  const B = -(d * i - f * g)
  const C = d * h - e * g
  const det = a * A + b * B + c * C
  if (Math.abs(det) < 1e-12) return null
  const inv = [
    A,
    -(b * i - c * h),
    b * f - c * e,
    B,
    a * i - c * g,
    -(a * f - c * d),
    C,
    -(a * h - b * g),
    a * e - b * d,
  ].map((v) => v / det)
  const last = inv[8]!
  return inv.map((v) => v / last) as unknown as Homography
}
