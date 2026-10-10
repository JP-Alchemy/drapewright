// The grainline tool: drag inside a piece to draw its grainline. It snaps
// to 15° steps of the piece's own axes; hold Shift for any angle.

import type { Piece } from '@soom/engine'
import type { ToolPointer } from '@soom/input'
import type { Tool, ToolContext } from './tool.ts'

/** Shortest grainline worth keeping, mm. */
const MIN_LENGTH = 10
const SNAP = (15 * Math.PI) / 180

type State = { kind: 'idle' } | { kind: 'drawing'; piece: string; from: [number, number] }

/** A work-table point in a piece's own coordinates. */
function toPiece(piece: Piece, world: { x: number; y: number }): [number, number] {
  const { x, y, rotation } = piece.placement
  const p = new DOMMatrix()
    .translateSelf(x, y)
    .rotateSelf(rotation)
    .inverse()
    .transformPoint(new DOMPoint(world.x, world.y))
  return [p.x, p.y]
}

export function grainlineTool(): Tool {
  let state: State = { kind: 'idle' }

  const pieceOf = (ctx: ToolContext, id: string) =>
    ctx.session.engine.document().pieces.find((p) => p.id === id)

  const end = (ctx: ToolContext, p: ToolPointer) => {
    if (state.kind !== 'drawing') return null
    const piece = pieceOf(ctx, state.piece)
    if (!piece) return null
    const [fx, fy] = state.from
    const [tx, ty] = toPiece(piece, ctx.world(p))
    const length = Math.hypot(tx - fx, ty - fy)
    if (p.modifiers.shift || length === 0)
      return { piece, to: [tx, ty] as [number, number], length }
    const angle = Math.round(Math.atan2(ty - fy, tx - fx) / SNAP) * SNAP
    const to: [number, number] = [fx + length * Math.cos(angle), fy + length * Math.sin(angle)]
    return { piece, to, length }
  }

  return {
    down(p, ctx) {
      const hit = ctx.hit(p)
      if (!hit) {
        state = { kind: 'idle' }
        return
      }
      const piece = pieceOf(ctx, hit.piece)
      if (!piece) return
      ctx.session.select([piece.id])
      state = { kind: 'drawing', piece: piece.id, from: toPiece(piece, ctx.world(p)) }
    },

    move(p, ctx) {
      const e = end(ctx, p)
      if (!e || state.kind !== 'drawing') return
      ctx.session.setGrainPreview({ piece: e.piece.id, from: state.from, to: e.to })
    },

    up(p, ctx) {
      const e = end(ctx, p)
      ctx.session.setGrainPreview(null)
      if (e && state.kind === 'drawing' && e.length >= MIN_LENGTH) {
        ctx.session.dispatch({
          type: 'setGrainline',
          piece: e.piece.id,
          grainline: { from: state.from, to: e.to, oneWay: e.piece.grainline?.oneWay ?? false },
        })
      }
      state = { kind: 'idle' }
    },

    cancel(ctx) {
      ctx.session.setGrainPreview(null)
      state = { kind: 'idle' }
    },
  }
}
