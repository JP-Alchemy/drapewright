// Points along a piece's edge, and stretches of it: for marking part of an
// edge (a seam sewing only part of it).

import type { Piece } from '@drapewright/engine'

type P = [number, number]

/** Steps a curved segment is followed in. */
const CURVE_STEPS = 24

/** One edge of a piece's outline as points along it, in piece coordinates. */
export function edgePoints(piece: Piece, edgeId: string): P[] | null {
  const edges = piece.outline.edges
  const i = edges.findIndex((e) => e.id === edgeId)
  if (i < 0) return null
  const prev = i === 0 ? null : edges[i - 1]?.segments.at(-1)
  let at: P = prev ? [prev.to[0], prev.to[1]] : [piece.outline.start[0], piece.outline.start[1]]
  const points: P[] = [at]
  for (const seg of edges[i]!.segments) {
    if ('c1' in seg) {
      const [a, b, c, d] = [at, seg.c1, seg.c2, seg.to]
      for (let k = 1; k <= CURVE_STEPS; k++) {
        const t = k / CURVE_STEPS
        const u = 1 - t
        const w = [u * u * u, 3 * u * u * t, 3 * u * t * t, t * t * t] as const
        points.push([
          w[0] * a[0] + w[1] * b[0] + w[2] * c[0] + w[3] * d[0],
          w[0] * a[1] + w[1] * b[1] + w[2] * c[1] + w[3] * d[1],
        ])
      }
    } else {
      points.push([seg.to[0], seg.to[1]])
    }
    at = [seg.to[0], seg.to[1]]
  }
  return points
}

/**
 * The stretch of a run of points from `from` to `to` of the way along it
 * (0 to 1): its points, its middle, and the way out from there (to the
 * left of the run: out of a piece, whose outline runs clockwise on screen).
 */
export function stretch(points: P[], from: number, to: number): { points: P[]; middle: P; out: P } {
  const lengths = [0]
  for (let k = 1; k < points.length; k++) {
    const [a, b] = [points[k - 1]!, points[k]!]
    lengths.push(lengths[k - 1]! + Math.hypot(b[0] - a[0], b[1] - a[1]))
  }
  const total = lengths.at(-1) ?? 0
  const at = (f: number): { p: P; k: number } => {
    const s = Math.min(Math.max(f, 0), 1) * total
    let k = 1
    while (k < points.length - 1 && lengths[k]! < s) k++
    const [a, b] = [points[k - 1]!, points[k]!]
    const span = lengths[k]! - lengths[k - 1]!
    const t = span > 0 ? (s - lengths[k - 1]!) / span : 0
    return { p: [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t], k }
  }
  const [start, end] = [at(Math.min(from, to)), at(Math.max(from, to))]
  const run: P[] = [start.p, ...points.slice(start.k, end.k), end.p]
  const mid = at((from + to) / 2)
  const [a, b] = [points[mid.k - 1]!, points[mid.k]!]
  const length = Math.hypot(b[0] - a[0], b[1] - a[1]) || 1
  return {
    points: run,
    middle: mid.p,
    out: [(b[1] - a[1]) / length, -(b[0] - a[0]) / length],
  }
}

/**
 * The curve joining two sewn edges across the table, from the middle of
 * one (`a`, the way out of its piece there `aOut`) to the middle of the
 * other: it leaves each edge outwards, as a thread pulled from one edge to
 * the other would. A cubic Bézier's points, mm; none where the two middles
 * meet.
 */
export function joinCurve(a: P, aOut: P, b: P, bOut: P): [P, P, P, P] | null {
  const apart = Math.hypot(b[0] - a[0], b[1] - a[1])
  if (apart < 1) return null
  const reach = Math.min(Math.max(apart * 0.35, 20), 150)
  return [
    a,
    [a[0] + aOut[0] * reach, a[1] + aOut[1] * reach],
    [b[0] + bOut[0] * reach, b[1] + bOut[1] * reach],
    b,
  ]
}

/**
 * The ways an edge leaves its corners, in piece coordinates (unit
 * vectors): along it from its start, and back along it from its end.
 */
export function edgeEnds(piece: Piece, edgeId: string): { start: P; end: P } | null {
  const points = edgePoints(piece, edgeId)
  if (!points || points.length < 2) return null
  const way = (from: P, to: P): P => {
    const length = Math.hypot(to[0] - from[0], to[1] - from[1]) || 1
    return [(to[0] - from[0]) / length, (to[1] - from[1]) / length]
  }
  const at = (k: number) => points[Math.max(0, Math.min(points.length - 1, k))]!
  return { start: way(at(0), at(1)), end: way(at(points.length - 1), at(points.length - 2)) }
}

/**
 * The corner at the end of an edge, in piece coordinates: where it is, and
 * out of the piece there (a unit vector, halfway between the two edges'
 * ways out).
 */
export function cornerOf(piece: Piece, edgeId: string): { at: P; out: P } | null {
  const edges = piece.outline.edges
  const i = edges.findIndex((e) => e.id === edgeId)
  const next = edges[(i + 1) % edges.length]
  const [a, b] = [edgeEnds(piece, edgeId), next && edgeEnds(piece, next.id)]
  const at = edges[i]?.segments.at(-1)?.to
  if (!a || !b || !at) return null
  // Out of a piece drawn clockwise is to the left of the way it runs.
  const into = [-a.end[0], -a.end[1]]
  const outA: P = [into[1]!, -into[0]!]
  const outB: P = [b.start[1], -b.start[0]]
  const [x, y] = [outA[0] + outB[0], outA[1] + outB[1]]
  const length = Math.hypot(x, y)
  return { at: [at[0], at[1]], out: length > 1e-6 ? [x / length, y / length] : outA }
}
