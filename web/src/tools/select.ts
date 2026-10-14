// The select tool: tap to select a piece or one of its edges; drag to move
// pieces on the table; drag across the table from where there is no piece
// to choose every piece the box touches, as files in a folder; hold an edge
// to see its length. Shift (or Command, or Ctrl) adds to what is chosen.

import type { ToolPointer } from '@drapewright/input'
import { TUNING } from '@drapewright/input'
import { piecesTouching } from '../canvas/marquee.ts'
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
  | {
      kind: 'table'
      start: Point
      screen: Point
      pointer: ToolPointer['kind']
      /** Chosen before the press, to go back to if it is called off. */
      before: string[]
      /** Kept chosen whatever the box touches (adding to them). */
      base: string[]
      /** Dragging the box (past a tap's slop). */
      boxing: boolean
    }

/** Shift, Command or Ctrl held: add to what is chosen rather than replace it. */
const adding = (p: ToolPointer) => p.modifiers.shift || p.modifiers.meta || p.modifiers.ctrl

const same = (a: ReadonlySet<string>, b: ReadonlySet<string>) =>
  a.size === b.size && [...a].every((id) => b.has(id))

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
        const before = [...ui.selection]
        const base = adding(p) ? before : []
        if (!adding(p)) session.select([])
        state = {
          kind: 'table',
          start: ctx.world(p),
          screen: { x: p.x, y: p.y },
          pointer: p.kind,
          before,
          base,
          boxing: false,
        }
        return
      }
      const piece = hit.piece
      let ids: string[]
      if (adding(p)) {
        const next = new Set(ui.selection)
        if (next.has(piece)) next.delete(piece)
        else next.add(piece)
        ids = [...next]
        session.select(ids, null)
      } else if (hit.kind === 'edge' || hit.kind === 'corner') {
        // An edge or corner belongs to one piece: select just that piece.
        ids = [piece]
        session.select(ids, { kind: hit.kind, piece, edge: hit.edge })
      } else if (hit.kind === 'notch') {
        ids = [piece]
        session.select(ids, { kind: 'notch', piece, notch: hit.notch })
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
        narrowTo: ids.length > 1 && !adding(p) ? piece : null,
      }
    },

    move(p, ctx) {
      if (state.kind === 'table') {
        const travel = Math.hypot(p.x - state.screen.x, p.y - state.screen.y)
        if (!state.boxing && travel <= TUNING.dragSlop[state.pointer]) return
        state.boxing = true
        const { session } = ctx
        const to = ctx.world(p)
        session.setMarquee({ from: state.start, to })
        const chosen = new Set([...state.base, ...piecesTouching(session.scene(), state.start, to)])
        if (!same(chosen, session.ui.get().selection)) session.select(chosen)
        return
      }
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
      if (state.kind === 'table') ctx.session.setMarquee(null)
      else if (state.kind === 'drag') finishDrag(ctx, ctx.world(p))
      // A tap (no drag) on one piece of a group selects just that piece.
      else if (state.kind === 'press' && state.narrowTo) ctx.session.select([state.narrowTo])
      state = { kind: 'idle' }
    },

    cancel(ctx) {
      // A box called off (a second finger down, say) leaves the choice as it was.
      if (state.kind === 'table') {
        ctx.session.setMarquee(null)
        ctx.session.select(state.before)
      }
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
