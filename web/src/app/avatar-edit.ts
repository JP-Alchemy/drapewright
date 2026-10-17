// Editing on the figure: the pieces' edges drawn over the 3D view as the
// sim worker last saw them, the edge under the pointer found, its length
// and handles laid out as on the table, and a drag of a handle turned into
// how far the edge moves.

import type { EdgeMove, SeenEdge } from '@drapewright/engine'
import {
  type CornerHandle,
  type EdgeHandles,
  HANDLE_RADIUS,
  cornerHandle,
  handlesBeside,
  unit,
} from '../canvas/handles.ts'
import { type Theme, drawHandle, drawPill } from '../canvas/renderer.ts'
import type { Point } from '../canvas/viewport.ts'

/** Within this of an edge, CSS px, the pointer is on it. */
export const NEAR_EDGE = 10
/** From an edge's middle to its length's label, out of the piece, CSS px. */
const LABEL_OUT = 28
/**
 * The least an edge's way out may show on the view, CSS px a mm: one seen
 * end on (a hem from below) is dragged as if it showed this much, not
 * flung by the slightest move.
 */
const LEAST_OUT = 0.2

const at = (e: SeenEdge, i: number): Point => ({ x: e.points[2 * i]!, y: e.points[2 * i + 1]! })

/** The runs of an edge's points that face the eye. */
export function shownRuns(e: SeenEdge): Point[][] {
  const runs: Point[][] = []
  let run: Point[] = []
  e.shown.forEach((shown, i) => {
    if (shown) run.push(at(e, i))
    else if (run.length > 0) {
      runs.push(run)
      run = []
    }
  })
  if (run.length > 0) runs.push(run)
  return runs.filter((r) => r.length > 1)
}

function toSegment(p: Point, a: Point, b: Point): number {
  const [dx, dy] = [b.x - a.x, b.y - a.y]
  const n2 = dx * dx + dy * dy
  const t = n2 > 0 ? Math.min(Math.max(((p.x - a.x) * dx + (p.y - a.y) * dy) / n2, 0), 1) : 0
  return Math.hypot(p.x - (a.x + dx * t), p.y - (a.y + dy * t))
}

/** How far `p` is from the part of an edge facing the eye, CSS px. */
export function distanceTo(e: SeenEdge, p: Point): number {
  let best = Infinity
  for (const run of shownRuns(e)) {
    for (let i = 1; i < run.length; i++) best = Math.min(best, toSegment(p, run[i - 1]!, run[i]!))
  }
  return best
}

/** The edge seen nearest `p`, if one is within `within` px. */
export function edgeNear(seen: SeenEdge[], p: Point, within = NEAR_EDGE): SeenEdge | null {
  let best: { e: SeenEdge; d: number } | null = null
  for (const e of seen) {
    const d = distanceTo(e, p)
    if (d <= within && (!best || d < best.d)) best = { e, d }
  }
  return best?.e ?? null
}

/** How long the part of an edge facing the eye is on the view, CSS px. */
function shownLength(e: SeenEdge): number {
  let total = 0
  for (const run of shownRuns(e)) {
    for (let i = 1; i < run.length; i++) {
      total += Math.hypot(run[i]!.x - run[i - 1]!.x, run[i]!.y - run[i - 1]!.y)
    }
  }
  return total
}

/**
 * Which of a piece edge's copies on the figure (a pair's two, a fold's two
 * halves) its handles go beside: the one nearest `near`, else the one seen
 * longest. Its middle must face the eye.
 */
export function copyOf(
  seen: SeenEdge[],
  piece: string,
  edge: string,
  near?: Point,
): SeenEdge | null {
  const copies = seen.filter((e) => e.piece === piece && e.edge === edge && e.shown[e.middle])
  const key = (e: SeenEdge) => (near ? distanceTo(e, near) : -shownLength(e))
  return copies.reduce<SeenEdge | null>((a, b) => (a && key(a) <= key(b) ? a : b), null)
}

/** An edge's length label and handles on the view: beside its middle, out of the piece. */
export function seenHandles(
  e: SeenEdge,
  label: string,
  size: { width: number; height: number },
): EdgeHandles {
  const middle = at(e, e.middle)
  const out = unit(e.out[0], e.out[1])
  const pill = { x: middle.x + out.x * LABEL_OUT, y: middle.y + out.y * LABEL_OUT }
  return handlesBeside(e.piece, e.edge, middle, out, pill, label, size)
}

/** How far a drag of `dx`, `dy` CSS px goes out of the piece at the edge's middle, mm. */
export function mmDragged(e: SeenEdge, dx: number, dy: number): number {
  const [ox, oy] = e.out
  const shows = Math.hypot(ox, oy)
  if (shows === 0) return 0
  // Along the way out as the view shows it, a mm for each `shows` px.
  const along = (dx * ox + dy * oy) / shows
  return along / Math.max(shows, LEAST_OUT)
}

/** Within this of a corner, CSS px, the pointer is on it. */
export const NEAR_CORNER = 9
/**
 * A corner facing the eye less squarely than this (at the figure's side)
 * is not moved on the figure: which way a drag means is unclear there.
 */
export const FACING_ENOUGH = 0.4

/**
 * A corner seen: where one edge's copy ends and the next edge's (on the
 * same copy of the piece) starts.
 */
export interface SeenCorner {
  piece: string
  /** The edge it ends. */
  edge: string
  at: Point
  /** The two edges' copies meeting there. */
  into: SeenEdge
  from: SeenEdge
  /** How squarely the cloth there faces the eye: 1 straight on, 0 edge on. */
  facing: number
}

/** The corners seen, each where two edges meet: `next` names the edge after one, on its piece. */
export function seenCorners(
  seen: SeenEdge[],
  next: (piece: string, edge: string) => string | null,
): SeenCorner[] {
  const corners: SeenCorner[] = []
  for (const into of seen) {
    const last = into.shown.length - 1
    const after = next(into.piece, into.edge)
    if (!into.shown[last] || after === null) continue
    const end = at(into, last)
    const from = seen.find((f) => {
      if (f.piece !== into.piece || f.edge !== after || !f.shown[0]) return false
      const start = at(f, 0)
      return Math.hypot(start.x - end.x, start.y - end.y) < 2
    })
    if (from) {
      const facing = Math.min(into.facing[1], from.facing[0])
      corners.push({ piece: into.piece, edge: into.edge, at: end, into, from, facing })
    }
  }
  return corners
}

/**
 * A corner wins over an edge under the pointer only this much further from
 * it, CSS px: on a short edge seen small, its middle is near its corners.
 */
const CORNER_BIAS = 4

/** What is under the pointer: a corner, if it is as near as the nearest edge; else that edge. */
export function underPointer(
  seen: SeenEdge[],
  corners: SeenCorner[],
  p: Point,
): { corner: SeenCorner } | { edge: SeenEdge } | null {
  const corner = cornerNear(corners, p)
  const edge = edgeNear(seen, p)
  if (corner) {
    const toCorner = Math.hypot(corner.at.x - p.x, corner.at.y - p.y)
    if (!edge || toCorner <= distanceTo(edge, p) + CORNER_BIAS) return { corner }
  }
  return edge ? { edge } : null
}

/** The corner seen nearest `p`, if one is within `within` px. */
export function cornerNear(
  corners: SeenCorner[],
  p: Point,
  within = NEAR_CORNER,
): SeenCorner | null {
  let best: { c: SeenCorner; d: number } | null = null
  for (const c of corners) {
    const d = Math.hypot(c.at.x - p.x, c.at.y - p.y)
    if (d <= within && (!best || d < best.d)) best = { c, d }
  }
  return best?.c ?? null
}

/** A corner's handle on the view: out from it, away from both its edges. */
export function seenCornerHandle(
  c: SeenCorner,
  size: { width: number; height: number },
): CornerHandle {
  const a = unit(c.into.ends[1][0], c.into.ends[1][1])
  const b = unit(c.from.ends[0][0], c.from.ends[0][1])
  const [x, y] = [-(a.x + b.x), -(a.y + b.y)]
  const out = Math.hypot(x, y) > 1e-3 ? unit(x, y) : { x: a.y, y: -a.x }
  return cornerHandle(c.piece, c.edge, c.at, out, size)
}

/**
 * How a move of a corner on its piece shows on the view: the view's px for
 * each mm across the piece, and down it (`jx`, `jy`). From the two ways its
 * edges leave it, on the view (px each mm, back along the one it ends,
 * along the next) and on the piece (`ways`, unit vectors the same ways).
 * None where they show as one way (seen edge on).
 */
function cornerJacobian(c: SeenCorner, ways: [Point, Point]) {
  const [s1, s2] = [c.into.ends[1], c.from.ends[0]]
  const [t1, t2] = ways
  const det = t1.x * t2.y - t2.x * t1.y
  if (Math.abs(det) < 1e-3) return null
  // A move v on the piece is a mm along t1 and b along t2 (v = T (a, b)),
  // and shows as a s1 + b s2: J = S T⁻¹.
  const inverse = [
    [t2.y / det, -t2.x / det],
    [-t1.y / det, t1.x / det],
  ] as const
  const jx = {
    x: s1[0] * inverse[0][0] + s2[0] * inverse[1][0],
    y: s1[1] * inverse[0][0] + s2[1] * inverse[1][0],
  }
  const jy = {
    x: s1[0] * inverse[0][1] + s2[0] * inverse[1][1],
    y: s1[1] * inverse[0][1] + s2[1] * inverse[1][1],
  }
  return { jx, jy }
}

/**
 * A way on the piece seen at less than this of the best-seen way is
 * taken as seen this much: dragging along it moves the corner no faster.
 */
const LEAST_SEEN = 0.3

/**
 * How far a drag of `d` px on the view moves a corner across and down its
 * piece, mm. Ways seen end on (at the side of the figure, the waist running
 * towards the eye) are damped, so a drag moves the corner the way it shows,
 * not flung along a way it hardly shows.
 */
export function cornerDragged(c: SeenCorner, ways: [Point, Point], d: Point): Point | null {
  const j = cornerJacobian(c, ways)
  if (!j) return null
  const { jx, jy } = j
  // Its singular values and ways on the piece: JᵀJ's eigenvalues and
  // eigenvectors.
  const [a, b, e] = [
    jx.x * jx.x + jx.y * jx.y,
    jx.x * jy.x + jx.y * jy.y,
    jy.x * jy.x + jy.y * jy.y,
  ]
  const half = (a + e) / 2
  const root = Math.sqrt(Math.max(half * half - (a * e - b * b), 0))
  const [l1, l2] = [half + root, Math.max(half - root, 0)]
  if (l1 <= 1e-12) return null
  const v1 = Math.abs(b) > 1e-12 ? unit(b, l1 - a) : a >= e ? { x: 1, y: 0 } : { x: 0, y: 1 }
  const v2 = { x: -v1.y, y: v1.x }
  const best = Math.sqrt(l1)
  let move = { x: 0, y: 0 }
  for (const [v, l] of [
    [v1, l1],
    [v2, l2],
  ] as const) {
    const sigma = Math.sqrt(l)
    // Along v on the piece: on the view, J v.
    const seen = { x: jx.x * v.x + jy.x * v.y, y: jx.y * v.x + jy.y * v.y }
    const along = sigma > 1e-9 ? (seen.x * d.x + seen.y * d.y) / sigma : 0
    const mm = along / Math.max(sigma, LEAST_SEEN * best)
    move = { x: move.x + v.x * mm, y: move.y + v.y * mm }
  }
  return move
}

/** Where on the view a corner moved `v` (mm across and down its piece) shows, from where it is. */
export function cornerShown(c: SeenCorner, ways: [Point, Point], v: Point): Point | null {
  const j = cornerJacobian(c, ways)
  if (!j) return null
  return { x: j.jx.x * v.x + j.jy.x * v.y, y: j.jx.y * v.x + j.jy.y * v.y }
}

/** What the overlay shows besides the edges. */
export interface Overlay {
  /** Piece edges drawn stronger: hovered or chosen, on the figure or on the table. */
  strong: { piece: string; edge: string }[]
  /** Corners drawn stronger, by the edge each ends. */
  strongCorners: { piece: string; edge: string }[]
  /** A corner's handle, and whether it is under the pointer or dragged. */
  corner: { handle: CornerHandle; active: boolean } | null
  /** While dragging a corner: where it is going. */
  cornerGhost: { from: Point; to: Point } | null
  /** The copy with its length and handles, and the length. */
  handles: { copy: SeenEdge; label: string; layout: EdgeHandles } | null
  /** The handle under the pointer or dragged, and whether dragged. */
  active: { kind: EdgeMove; dragging: boolean } | null
  /** While dragging: the edge where it is going, `by` px off, dashed. */
  ghost: { copy: SeenEdge; offset: Point } | null
  /** Said beside the pointer. */
  tip: { at: Point; text: string } | null
}

/** Draws the pieces' edges and their corners over the view, and what `o` adds. */
export function drawOverlay(
  ctx: CanvasRenderingContext2D,
  seen: SeenEdge[],
  o: Overlay,
  theme: Theme,
): void {
  const strong = (e: SeenEdge) => o.strong.some((s) => s.piece === e.piece && s.edge === e.edge)
  const line = (runs: Point[][], offset: Point = { x: 0, y: 0 }) => {
    ctx.beginPath()
    for (const run of runs) {
      run.forEach((p, i) =>
        i === 0
          ? ctx.moveTo(p.x + offset.x, p.y + offset.y)
          : ctx.lineTo(p.x + offset.x, p.y + offset.y),
      )
    }
    ctx.stroke()
  }
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'
  // Each edge, then its corners.
  for (const e of seen) {
    const runs = shownRuns(e)
    ctx.strokeStyle = 'rgba(255, 255, 255, 0.7)'
    ctx.lineWidth = 3
    line(runs)
    ctx.strokeStyle = strong(e) ? theme.select : theme.seam
    ctx.lineWidth = strong(e) ? 3 : 1.5
    line(runs)
  }
  const strongAt = (e: SeenEdge, i: number) =>
    o.strongCorners.some(
      (c) => c.piece === e.piece && i === e.shown.length - 1 && c.edge === e.edge,
    )
  for (const e of seen) {
    for (const i of [0, e.shown.length - 1]) {
      if (!e.shown[i]) continue
      const p = at(e, i)
      const strongHere = strongAt(e, i)
      ctx.beginPath()
      ctx.arc(p.x, p.y, strongHere ? 5 : 3, 0, Math.PI * 2)
      ctx.fillStyle = strongHere ? theme.select : theme.paper
      ctx.fill()
      ctx.strokeStyle = strongHere ? theme.paper : theme.seam
      ctx.lineWidth = 1.25
      ctx.stroke()
    }
  }
  if (o.cornerGhost) {
    const { from, to } = o.cornerGhost
    ctx.setLineDash([6, 4])
    ctx.strokeStyle = theme.select
    ctx.lineWidth = 2
    ctx.beginPath()
    ctx.moveTo(from.x, from.y)
    ctx.lineTo(to.x, to.y)
    ctx.stroke()
    ctx.setLineDash([])
    ctx.beginPath()
    ctx.arc(to.x, to.y, 5, 0, Math.PI * 2)
    ctx.fillStyle = theme.select
    ctx.fill()
  }
  if (o.ghost) {
    ctx.setLineDash([6, 4])
    ctx.strokeStyle = theme.select
    ctx.lineWidth = 2
    line(shownRuns(o.ghost.copy), o.ghost.offset)
    ctx.setLineDash([])
  }
  if (o.handles) {
    const h = o.handles.layout
    drawPill(ctx, o.handles.label, h.pill.x, h.pill.y, theme)
    for (const kind of ['extend', 'extrude'] as const) {
      drawHandle(ctx, kind, h[kind], h.out, o.active?.kind === kind, theme)
    }
  }
  if (o.corner) {
    const h = o.corner.handle
    drawHandle(ctx, 'corner', h.at, h.out, o.corner.active, theme)
  }
  if (o.tip) drawPill(ctx, o.tip.text, o.tip.at.x, o.tip.at.y - HANDLE_RADIUS - 16, theme)
}
