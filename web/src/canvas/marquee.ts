// Choosing pieces by dragging a box across the table, as files are chosen
// in a folder: every piece the box touches (its paper, to the cutting line)
// is chosen, whether or not the box takes in all of it.

import type { Piece } from '@drapewright/engine'
import type { Point } from './viewport.ts'

/** What the box needs of a piece: where it lies, and its cutting line. */
export interface Boxable {
  piece: Pick<Piece, 'id' | 'placement'>
  /** The cutting line, `x0, y0, x1, y1, …` in the piece's own mm, closed. */
  view: { cutting: number[] }
}

/** The pieces a box from corner `a` to corner `b` (work-table mm) touches. */
export function piecesTouching(pieces: readonly Boxable[], a: Point, b: Point): string[] {
  const box = {
    x0: Math.min(a.x, b.x),
    y0: Math.min(a.y, b.y),
    x1: Math.max(a.x, b.x),
    y1: Math.max(a.y, b.y),
  }
  return pieces
    .filter(({ piece, view }) => touches(onTable(view.cutting, piece.placement), box))
    .map(({ piece }) => piece.id)
}

interface Box {
  x0: number
  y0: number
  x1: number
  y1: number
}

/** A piece's outline where it lies on the table. */
function onTable(flat: number[], placement: Piece['placement']): Point[] {
  const angle = (placement.rotation * Math.PI) / 180
  const cos = Math.cos(angle)
  const sin = Math.sin(angle)
  const points: Point[] = []
  for (let i = 0; i + 1 < flat.length; i += 2) {
    const x = flat[i]!
    const y = flat[i + 1]!
    points.push({ x: placement.x + x * cos - y * sin, y: placement.y + x * sin + y * cos })
  }
  return points
}

function touches(outline: Point[], box: Box): boolean {
  if (outline.length < 3) return false
  // Nowhere near: past the box on some side.
  if (
    outline.every((p) => p.x < box.x0) ||
    outline.every((p) => p.x > box.x1) ||
    outline.every((p) => p.y < box.y0) ||
    outline.every((p) => p.y > box.y1)
  ) {
    return false
  }
  // Part of the outline in the box, the box inside the piece, or the
  // outline crossing the box's sides.
  if (outline.some((p) => inBox(p, box))) return true
  if (inside({ x: box.x0, y: box.y0 }, outline)) return true
  const corners: Point[] = [
    { x: box.x0, y: box.y0 },
    { x: box.x1, y: box.y0 },
    { x: box.x1, y: box.y1 },
    { x: box.x0, y: box.y1 },
  ]
  for (let i = 0; i < outline.length; i++) {
    const p = outline[i]!
    const q = outline[(i + 1) % outline.length]!
    for (let k = 0; k < 4; k++) {
      if (crosses(p, q, corners[k]!, corners[(k + 1) % 4]!)) return true
    }
  }
  return false
}

const inBox = (p: Point, box: Box) =>
  p.x >= box.x0 && p.x <= box.x1 && p.y >= box.y0 && p.y <= box.y1

/** Whether `p` is inside the outline (even–odd). */
function inside(p: Point, outline: Point[]): boolean {
  let within = false
  for (let i = 0, j = outline.length - 1; i < outline.length; j = i++) {
    const a = outline[i]!
    const b = outline[j]!
    if (a.y > p.y !== b.y > p.y && p.x < ((b.x - a.x) * (p.y - a.y)) / (b.y - a.y) + a.x) {
      within = !within
    }
  }
  return within
}

/** Whether segments `ab` and `cd` cross or touch. */
function crosses(a: Point, b: Point, c: Point, d: Point): boolean {
  const o1 = turn(a, b, c)
  const o2 = turn(a, b, d)
  const o3 = turn(c, d, a)
  const o4 = turn(c, d, b)
  if (o1 * o2 < 0 && o3 * o4 < 0) return true
  return (
    (o1 === 0 && along(a, b, c)) ||
    (o2 === 0 && along(a, b, d)) ||
    (o3 === 0 && along(c, d, a)) ||
    (o4 === 0 && along(c, d, b))
  )
}

/** Which way `p → q → r` turns: positive one way, negative the other, 0 in line. */
const turn = (p: Point, q: Point, r: Point) => (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x)

/** Whether `r`, in line with `p` and `q`, lies between them. */
const along = (p: Point, q: Point, r: Point) =>
  r.x >= Math.min(p.x, q.x) &&
  r.x <= Math.max(p.x, q.x) &&
  r.y >= Math.min(p.y, q.y) &&
  r.y <= Math.max(p.y, q.y)
