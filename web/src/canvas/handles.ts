// The two handles beside an edge's length, to drag the edge out or in:
// one extends it (the edges either side carry on the way they run), the
// other extrudes it (a strip added, its sides square to the edge). Laid
// out in screen space, so the renderer and the select tool agree on where
// they are.

import type { EdgeMove, EdgeView } from '@drapewright/engine'
import { type Point, type ViewportState, scaleOf, toScreen } from './viewport.ts'

/** A handle's radius, CSS px. */
export const HANDLE_RADIUS = 13
/** Pressed this near a handle's middle, it is taken, CSS px. */
const GRAB_RADIUS = HANDLE_RADIUS + 5
/** Between the length label and each handle, CSS px. */
const GAP = 6
/** Kept this far inside the table's view, CSS px. */
const EDGE_ROOM = 4
/** How far beyond the edge's allowance its length is shown, CSS px. */
export const PILL_OUT = 14
export const PILL_HEIGHT = 22
export const PILL_FONT =
  '600 12px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'

let measurer: OffscreenCanvasRenderingContext2D | null | undefined

/** How wide the length label is for `text`, CSS px. */
export function pillWidth(text: string): number {
  if (measurer === undefined) {
    try {
      measurer =
        typeof OffscreenCanvas === 'undefined' ? null : new OffscreenCanvas(1, 1).getContext('2d')
    } catch {
      measurer = null
    }
    if (measurer) measurer.font = PILL_FONT
  }
  // Without a canvas to measure on (tests), about 7 px a letter.
  return (measurer ? measurer.measureText(text).width : text.length * 7) + 14
}

export interface EdgeHandles {
  piece: string
  edge: string
  /** The edge's middle and its length label's middle, screen px. */
  middle: Point
  pill: Point
  /** The length label's width, px. */
  width: number
  /** Out of the piece at the edge's middle, as a unit vector on screen. */
  out: Point
  /** Where each handle is, screen px: extend on the left (or above), extrude on the right. */
  extend: Point
  extrude: Point
}

/** A piece's own coordinates to the table's: its placement (a DOMMatrix will do). */
export interface ToTable {
  transformPoint(p: Point): Point
}

const unit = (x: number, y: number): Point => {
  const n = Math.hypot(x, y) || 1
  return { x: x / n, y: y / n }
}

/**
 * Where an edge's length label and handles are on screen. `toTable` takes
 * the piece's own coordinates to the table's (its placement); `label` is
 * the length shown.
 */
export function edgeHandles(
  piece: string,
  edge: EdgeView,
  toTable: ToTable,
  v: ViewportState,
  label: string,
): EdgeHandles {
  const s = scaleOf(v)
  const screen = (x: number, y: number) => {
    const w = toTable.transformPoint({ x, y })
    return toScreen(v, { x: w.x, y: w.y })
  }
  const [mx, my] = edge.middle
  const [nx, ny] = edge.middleNormal
  const reach = edge.allowance + PILL_OUT / s
  const middle = screen(mx, my)
  const tip = screen(mx + nx, my + ny)
  const out = unit(tip.x - middle.x, tip.y - middle.y)
  // Along the edge either side of the label, clear of it.
  const along = { x: -out.y, y: out.x }
  const width = pillWidth(label)
  const away =
    (Math.abs(along.x) * width) / 2 + (Math.abs(along.y) * PILL_HEIGHT) / 2 + GAP + HANDLE_RADIUS
  // Kept on the table's view, so an edge near its side can still be
  // taken hold of.
  const half = {
    x: Math.max(width / 2, Math.abs(along.x) * away + HANDLE_RADIUS) + EDGE_ROOM,
    y: Math.max(PILL_HEIGHT / 2, Math.abs(along.y) * away + HANDLE_RADIUS) + EDGE_ROOM,
  }
  const within = (z: number, room: number, size: number) =>
    size > 2 * room ? Math.min(Math.max(z, room), size - room) : z
  const beyond = screen(mx + nx * reach, my + ny * reach)
  const pill = { x: within(beyond.x, half.x, v.width), y: within(beyond.y, half.y, v.height) }
  const a = { x: pill.x - along.x * away, y: pill.y - along.y * away }
  const b = { x: pill.x + along.x * away, y: pill.y + along.y * away }
  const aFirst = a.x < b.x - 0.5 || (Math.abs(a.x - b.x) <= 0.5 && a.y < b.y)
  return {
    piece,
    edge: edge.id,
    middle,
    pill,
    width,
    out,
    extend: aFirst ? a : b,
    extrude: aFirst ? b : a,
  }
}

/** The handle at a screen point, if any. */
export function handleAt(h: EdgeHandles, p: Point): EdgeMove | null {
  const near = (q: Point) => Math.hypot(p.x - q.x, p.y - q.y) <= GRAB_RADIUS
  if (near(h.extend)) return 'extend'
  if (near(h.extrude)) return 'extrude'
  return null
}

/**
 * Whether a screen point is on the way from the edge to its handles: the
 * band from the edge's middle out past the label, as wide as the handles
 * reach. Moving through it, the edge stays shown so the handles can be
 * reached.
 */
export function towardHandles(h: EdgeHandles, p: Point): boolean {
  const [dx, dy] = [h.pill.x - h.middle.x, h.pill.y - h.middle.y]
  const length = Math.hypot(dx, dy)
  const [ux, uy] = length > 0 ? [dx / length, dy / length] : [h.out.x, h.out.y]
  const [px, py] = [p.x - h.middle.x, p.y - h.middle.y]
  const outward = px * ux + py * uy
  const across = Math.abs(px * -uy + py * ux)
  const half = Math.hypot(h.extend.x - h.pill.x, h.extend.y - h.pill.y) + GRAB_RADIUS
  return outward >= -GRAB_RADIUS && outward <= length + GRAB_RADIUS && across <= half
}

/**
 * The handle's picture, in its own frame: x along the edge, y out of the
 * piece, about 20 px across. An arrow out of the edge, and the sides
 * either side carrying on: flaring as they run (extend), or square
 * (extrude).
 */
export function handleGlyph(kind: EdgeMove): [number, number][][] {
  const sides: [number, number][][] =
    kind === 'extend'
      ? [
          [
            [-5, -5],
            [-8.5, 6],
          ],
          [
            [5, -5],
            [8.5, 6],
          ],
        ]
      : [
          [
            [-6, -5],
            [-6, 6],
          ],
          [
            [6, -5],
            [6, 6],
          ],
        ]
  return [
    // The edge.
    [
      [-7, -5],
      [7, -5],
    ],
    // The arrow out of it.
    [
      [0, -2],
      [0, 6],
    ],
    [
      [-2.5, 3.5],
      [0, 6.5],
      [2.5, 3.5],
    ],
    ...sides,
  ]
}
