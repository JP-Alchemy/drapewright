// The select tool: tap to select a piece or one of its edges; drag to move
// pieces on the table; hold an edge to see its length.

import type { ToolPointer } from '@soom/input'
import { TUNING } from '@soom/input'
import type { Point } from '../canvas/viewport.ts'
import { formatLength } from '../strings/en.ts'
import type { Tool, ToolContext } from './tool.ts'

type State =
  | { kind: 'idle' }
  | {
      kind: 'press'
      start: Point
      screen: Point
      ids: string[]
      pointer: ToolPointer['kind']
      /** The pressed piece, when it was part of a larger selection. */
      narrowTo: string | null
    }
  | { kind: 'drag'; start: Point; ids: string[] }

export function selectTool(): Tool {
  let state: State = { kind: 'idle' }

  const finishDrag = (ctx: ToolContext, end: Point | null) => {
    if (state.kind !== 'drag') return
    const { start, ids } = state
    ctx.session.setPreview(null)
    if (end && (end.x !== start.x || end.y !== start.y)) {
      ctx.session.dispatch({ type: 'movePieces', ids, dx: end.x - start.x, dy: end.y - start.y })
    }
  }

  return {
    down(p, ctx) {
      const { session } = ctx
      const hit = ctx.hit(p)
      const ui = session.ui.get()
      session.showTip(null)
      if (!hit) {
        if (!p.modifiers.shift) session.select([])
        state = { kind: 'idle' }
        return
      }
      const piece = hit.piece
      let ids: string[]
      if (p.modifiers.shift) {
        const next = new Set(ui.selection)
        if (next.has(piece)) next.delete(piece)
        else next.add(piece)
        ids = [...next]
        session.select(ids, null)
      } else if (hit.kind === 'edge' || hit.kind === 'corner') {
        // An edge or corner belongs to one piece: select just that piece.
        ids = [piece]
        session.select(ids, { kind: hit.kind, piece, edge: hit.edge })
      } else {
        // Keep a group selected so it can be dragged together.
        ids = ui.selection.has(piece) ? [...ui.selection] : [piece]
        session.select(ids, null)
      }
      state = {
        kind: 'press',
        start: ctx.world(p),
        screen: { x: p.x, y: p.y },
        ids,
        pointer: p.kind,
        narrowTo: ids.length > 1 && !p.modifiers.shift ? piece : null,
      }
    },

    move(p, ctx) {
      if (state.kind === 'press') {
        const travel = Math.hypot(p.x - state.screen.x, p.y - state.screen.y)
        if (travel <= TUNING.dragSlop[state.pointer] || state.ids.length === 0) return
        state = { kind: 'drag', start: state.start, ids: state.ids }
      }
      if (state.kind === 'drag') {
        const w = ctx.world(p)
        ctx.session.setPreview({
          ids: new Set(state.ids),
          dx: w.x - state.start.x,
          dy: w.y - state.start.y,
        })
      }
    },

    up(p, ctx) {
      if (state.kind === 'drag') finishDrag(ctx, ctx.world(p))
      // A tap (no drag) on one piece of a group selects just that piece.
      else if (state.kind === 'press' && state.narrowTo) ctx.session.select([state.narrowTo])
      state = { kind: 'idle' }
    },

    cancel(ctx) {
      finishDrag(ctx, null)
      state = { kind: 'idle' }
    },

    hover(p, ctx) {
      ctx.session.setHover(p ? ctx.hit(p) : null)
    },

    hold(p, ctx) {
      const hit = ctx.hit(p)
      if (hit?.kind !== 'edge') return
      const scene = ctx.session.scenePiece(hit.piece)
      const edge = scene?.view.edges.find((e) => e.id === hit.edge)
      if (!edge) return
      const units = ctx.session.engine.document().settings.displayUnits
      const name = edge.name || 'Edge'
      ctx.session.showTip({ x: p.x, y: p.y, text: `${name}: ${formatLength(edge.length, units)}` })
    },
  }
}
