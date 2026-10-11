// The notch tool: tap an edge to add a notch there; drag a notch to slide
// it along its edge; tap a notch to cycle single → double → triple.

import type { Notch, NotchKind, Piece } from '@drapewright/engine'
import type { ToolPointer } from '@drapewright/input'
import { TUNING } from '@drapewright/input'
import { formatLength, t } from '../strings/en.ts'
import type { Tool, ToolContext } from './tool.ts'

const NEXT_KIND: Record<NotchKind, NotchKind> = {
  single: 'double',
  double: 'triple',
  triple: 'single',
}

type State =
  | { kind: 'idle' }
  | {
      kind: 'press'
      piece: string
      notch: string
      screen: { x: number; y: number }
      pointer: ToolPointer['kind']
      /** Created by this press: a tap should not also cycle its kind. */
      created: boolean
      dragging: boolean
      gesture: number
    }

let gestures = 0

/** Where along an edge a pointer is, mm from the edge start. */
function alongEdge(ctx: ToolContext, p: ToolPointer, piece: Piece, edgeId: string): number | null {
  const w = ctx.world(p)
  return ctx.session.engine.nearestOnEdge(piece.id, edgeId, w.x, w.y)
}

export function notchTool(): Tool {
  let state: State = { kind: 'idle' }

  const notchOf = (ctx: ToolContext, pieceId: string, notchId: string) => {
    const piece = ctx.session.engine.document().pieces.find((p) => p.id === pieceId)
    const notch = piece?.notches.find((n) => n.id === notchId)
    return piece && notch ? { piece, notch } : null
  }

  return {
    down(p, ctx) {
      const { session } = ctx
      const hit = ctx.hit(p)
      session.showTip(null)
      if (hit?.kind === 'notch') {
        session.select([hit.piece], { kind: 'notch', piece: hit.piece, notch: hit.notch })
        state = {
          kind: 'press',
          piece: hit.piece,
          notch: hit.notch,
          screen: { x: p.x, y: p.y },
          pointer: p.kind,
          created: false,
          dragging: false,
          gesture: ++gestures,
        }
        return
      }
      if (hit?.kind !== 'edge') {
        if (hit) session.select([hit.piece])
        state = { kind: 'idle' }
        return
      }
      const piece = session.engine.document().pieces.find((x) => x.id === hit.piece)
      const view = session.scenePiece(hit.piece)?.view
      const length = view?.edges.find((e) => e.id === hit.edge)?.length
      if (!piece || length === undefined) return
      // Measure from the nearer corner, so it stays put if the far end moves.
      const fromEnd = hit.along > length / 2
      const at = fromEnd ? length - hit.along : hit.along
      const before = new Set(piece.notches.map((n) => n.id))
      const ok = session.dispatch({
        type: 'addNotch',
        piece: piece.id,
        edge: hit.edge,
        at,
        from: fromEnd ? 'end' : 'start',
        kind: 'single',
      })
      if (!ok) return
      const added = session.engine
        .document()
        .pieces.find((x) => x.id === piece.id)
        ?.notches.find((n) => !before.has(n.id))
      if (!added) return
      session.select([piece.id], { kind: 'notch', piece: piece.id, notch: added.id })
      state = {
        kind: 'press',
        piece: piece.id,
        notch: added.id,
        screen: { x: p.x, y: p.y },
        pointer: p.kind,
        created: true,
        dragging: false,
        gesture: ++gestures,
      }
    },

    move(p, ctx) {
      if (state.kind !== 'press') return
      if (!state.dragging) {
        const travel = Math.hypot(p.x - state.screen.x, p.y - state.screen.y)
        if (travel <= TUNING.dragSlop[state.pointer]) return
        state.dragging = true
      }
      const found = notchOf(ctx, state.piece, state.notch)
      if (!found) return
      const { piece, notch } = found
      const along = alongEdge(ctx, p, piece, notch.edge)
      const length = ctx.session
        .scenePiece(piece.id)
        ?.view.edges.find((e) => e.id === notch.edge)?.length
      if (along === null || length === undefined) return
      const at = notch.from === 'end' ? length - along : along
      const moved: Notch = { ...notch, at: Math.max(0, Math.min(length, at)) }
      // A whole drag is one undo step.
      ctx.session.dispatch(
        { type: 'setNotch', piece: piece.id, notch: moved },
        { coalesce: `notch-drag:${state.gesture}` },
      )
      const units = ctx.session.engine.document().settings.displayUnits
      ctx.session.showTip({
        x: p.x,
        y: p.y,
        text: t.notch.fromCorner(formatLength(moved.at, units)),
      })
    },

    up(_p, ctx) {
      if (state.kind === 'press' && !state.dragging && !state.created) {
        const found = notchOf(ctx, state.piece, state.notch)
        if (found) {
          ctx.session.dispatch({
            type: 'setNotch',
            piece: found.piece.id,
            notch: { ...found.notch, kind: NEXT_KIND[found.notch.kind] },
          })
        }
      }
      ctx.session.showTip(null)
      state = { kind: 'idle' }
    },

    cancel(ctx) {
      ctx.session.showTip(null)
      state = { kind: 'idle' }
    },

    hover(p, ctx) {
      ctx.session.setHover(p ? ctx.hit(p) : null)
    },
  }
}
