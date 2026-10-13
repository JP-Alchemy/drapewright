// The pick tool: points tapped on a piece to place an alteration. A slash
// and spread takes where its slash starts and where it hinges (both on the
// edge) and then the part that turns; a bust adjustment takes the bust
// point, then where the armhole hinges and where the side seam is slashed
// (both on the edge); a pivot takes the dart to move (unless the piece has
// only one), then where on the edge it goes; a swayback, shoulder, bust at
// the side, waist or hip adjustment takes two points on the edge; truing up takes the edge
// tapped. Points on the edge snap to it.

import type { Alteration, Piece } from '@drapewright/engine'
import type { ToolPointer } from '@drapewright/input'
import type { PickFor, Picking } from '../state/session.ts'
import { type Tool, type ToolContext, toPiece } from './tool.ts'

/** For each kind, which of its points are on the edge (the rest inside the piece). */
export const ON_EDGE: Record<PickFor, boolean[]> = {
  spread: [true, true, false],
  bust: [false, true, true],
  pivot: [true],
  swayback: [true, true],
  shoulder: [true, true],
  bustSide: [true, true],
  waist: [true, true],
  hip: [true, true],
  trueUp: [],
}

/** Whether an edge runs within 6 mm of the line between its ends: one to true straight. */
export function nearlyStraight(piece: Piece, edgeId: string): boolean {
  const edges = piece.outline.edges
  const i = edges.findIndex((e) => e.id === edgeId)
  const edge = edges[i]
  if (!edge) return false
  const start = i === 0 ? piece.outline.start : edges[i - 1]!.segments.at(-1)!.to
  const end = edge.segments.at(-1)!.to
  const [dx, dy] = [end[0] - start[0], end[1] - start[1]]
  const length = Math.hypot(dx, dy) || 1
  // A curve keeps within its control points.
  const points = edge.segments.flatMap((s) => ('c1' in s ? [s.c1, s.c2, s.to] : [s.to]))
  return points.every(([x, y]) => Math.abs((x - start[0]) * dy - (y - start[1]) * dx) / length < 6)
}

/** A step making `change`. */
const step = (change: Alteration['change']): Alteration => ({ id: '', on: true, change })

/** A press moving less than this, CSS pixels, is a tap. */
const TAP_SLOP = 8

/** The alteration the points place, once all are tapped: 2 cm to start. */
export function placed(picking: Picking): Alteration | null {
  const points = picking.points
  if (points.length < ON_EDGE[picking.for].length) return null
  const [a, b, c] = points as [[number, number], [number, number], [number, number]]
  if (picking.for === 'pivot') {
    if (!picking.dart) return null
    return { id: '', on: true, change: { type: 'pivot', dart: picking.dart, to: a, share: 1 } }
  }
  switch (picking.for) {
    case 'swayback':
      return step({ type: 'swayback', centre: a, side: b, by: 15 })
    case 'shoulder':
      return step({ type: 'shoulder', shoulder: a, armhole: b, by: 10 })
    case 'bustSide':
      return step({ type: 'bustSide', at: a, to: b, by: 10 })
    case 'waist':
      return step({ type: 'waist', at: a, to: b, by: 10 })
    case 'hip':
      return step({ type: 'hip', at: a, to: b, by: 10 })
  }
  if (picking.for === 'spread') {
    const along = [b[0] - a[0], b[1] - a[1]] as const
    // Right of the slash, going along it from where it starts (y down).
    const right = (c[0] - a[0]) * -along[1] + (c[1] - a[1]) * along[0]
    return {
      id: '',
      on: true,
      change: { type: 'spread', from: a, hinge: b, by: 20, turns: right > 0 ? 'right' : 'left' },
    }
  }
  return {
    id: '',
    on: true,
    change: { type: 'fullBust', apex: a, armhole: b, side: c, by: 20 },
  }
}

/** The point a tap gives on the piece being picked on, if it gives one. */
function tapped(p: ToolPointer, ctx: ToolContext, picking: Picking): [number, number] | null {
  const piece = ctx.session.engine.document().pieces.find((q) => q.id === picking.piece)
  const hit = ctx.hit(p)
  if (!piece || !hit || hit.piece !== piece.id) return null
  const at = ctx.world(p)
  if (!ON_EDGE[picking.for][picking.points.length]) return toPiece(piece, at)
  if (hit.kind !== 'edge' && hit.kind !== 'corner') return null
  return ctx.session.engine.snapToEdge(piece.id, hit.edge, at.x, at.y)
}

export function pickTool(): Tool {
  let pressed: ToolPointer | null = null
  return {
    down(p) {
      pressed = p
    },
    move() {},
    up(p, ctx) {
      const start = pressed
      pressed = null
      const picking = ctx.session.ui.get().picking
      if (!start || !picking || Math.hypot(p.x - start.x, p.y - start.y) > TAP_SLOP) return
      if (picking.for === 'trueUp') {
        // The edge tapped: trued straight if it nearly is, else smooth.
        const hit = ctx.hit(p)
        const piece = ctx.session.engine.document().pieces.find((q) => q.id === picking.piece)
        if (!piece || hit?.piece !== piece.id || (hit.kind !== 'edge' && hit.kind !== 'corner'))
          return
        ctx.session.setPicking(null)
        ctx.session.dispatch({
          type: 'addAlteration',
          piece: piece.id,
          alteration: step({
            type: 'trueUp',
            edge: hit.edge,
            straight: nearlyStraight(piece, hit.edge),
          }),
        })
        return
      }
      if (picking.for === 'pivot' && !picking.dart) {
        // Which dart: the one tapped.
        const hit = ctx.hit(p)
        if (hit?.piece !== picking.piece || (hit.kind !== 'edge' && hit.kind !== 'corner')) return
        const at = ctx.world(p)
        const dart = ctx.session.engine.dartAt(picking.piece, hit.edge, at.x, at.y)
        if (dart) ctx.session.setPicking({ ...picking, dart })
        return
      }
      const point = tapped(p, ctx, picking)
      if (!point) return
      const next = { ...picking, points: [...picking.points, point] }
      const alteration = placed(next)
      if (alteration) {
        ctx.session.setPicking(null)
        ctx.session.dispatch({ type: 'addAlteration', piece: picking.piece, alteration })
      } else {
        ctx.session.setPicking(next)
      }
    },
    cancel() {
      pressed = null
    },
  }
}
