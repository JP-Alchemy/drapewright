// Editing on the figure: the pieces' edges drawn over the 3D view as the
// sim worker last saw them, the edge under the pointer found, its length
// and handles laid out as on the table, and a drag of a handle turned into
// how far the edge moves.

import type { EdgeMove, SeenEdge } from '@drapewright/engine'
import { type EdgeHandles, HANDLE_RADIUS, handlesBeside, unit } from '../canvas/handles.ts'
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

/** What the overlay shows besides the edges. */
export interface Overlay {
  /** Piece edges drawn stronger: hovered or chosen, on the figure or on the table. */
  strong: { piece: string; edge: string }[]
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
  for (const e of seen) {
    for (const i of [0, e.shown.length - 1]) {
      if (!e.shown[i]) continue
      const p = at(e, i)
      ctx.beginPath()
      ctx.arc(p.x, p.y, 3, 0, Math.PI * 2)
      ctx.fillStyle = theme.paper
      ctx.fill()
      ctx.strokeStyle = theme.seam
      ctx.lineWidth = 1.25
      ctx.stroke()
    }
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
  if (o.tip) drawPill(ctx, o.tip.text, o.tip.at.x, o.tip.at.y - HANDLE_RADIUS - 16, theme)
}
