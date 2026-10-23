// The select tool: tap to select a piece or one of its edges; drag to move
// pieces on the table; drag across the table from where there is no piece
// to choose every piece the box touches, as files in a folder; hold an edge
// to see its length; drag the handles beside an edge's length to move the
// edge out or in (every edge chosen with it, too), or beside a corner to
// move it. Shift (or Command, or Ctrl) adds a piece to what is chosen. On
// an edge, Shift adds the edge to the edges chosen (or takes it away), and
// Ctrl or Alt (Option) chooses the line round the garment it is part of,
// as Blender chooses an edge loop; with Shift as well, adding it.

import type { ToolPointer } from '@drapewright/input'
import { TUNING } from '@drapewright/input'
import { HANDLE_RADIUS, towardCorner, towardHandles } from '../canvas/handles.ts'
import { piecesTouching } from '../canvas/marquee.ts'
import type { Point } from '../canvas/viewport.ts'
import { formatLength, t } from '../strings/en.ts'
import {
  type CornerDrag,
  cancelCornerDrag,
  cornerUnder,
  endCornerDrag,
  moveCornerDrag,
  shownCorners,
  startCornerDrag,
} from './corner-move.ts'
import {
  type EdgeDrag,
  cancelEdgeDrag,
  endEdgeDrag,
  handleUnder,
  moveEdgeDrag,
  shownHandles,
  startEdgeDrag,
} from './edge-move.ts'
import type { Tool, ToolContext } from './tool.ts'

type State =
  | { kind: 'idle' }
  | { kind: 'edge'; drag: EdgeDrag }
  | { kind: 'corner'; drag: CornerDrag }
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
      const handle = handleUnder(session, p)
      const drag = handle && startEdgeDrag(ctx, p, handle.handles, handle.kind)
      if (drag) {
        state = { kind: 'edge', drag }
        return
      }
      const corner = cornerUnder(session, p)
      const moving = corner && startCornerDrag(ctx, p, corner)
      if (moving) {
        state = { kind: 'corner', drag: moving }
        return
      }
      const hit = ctx.hit(p)
      const ui = session.ui.get()
      session.showTip(null)
      // A pocket waiting to be sewn on: onto the line tapped.
      if (hit && session.sewOntoAt(hit.piece, ctx.world(p))) {
        state = { kind: 'idle' }
        return
      }
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
      if (hit.kind === 'edge' && (p.modifiers.ctrl || p.modifiers.alt)) {
        session.chooseLine({ piece, edge: hit.edge }, p.modifiers.shift)
        state = { kind: 'idle' }
        return
      }
      if (hit.kind === 'edge' && p.modifiers.shift) {
        session.toggleEdge({ piece, edge: hit.edge })
        state = { kind: 'idle' }
        return
      }
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
      if (state.kind === 'edge') {
        moveEdgeDrag(ctx, p, state.drag)
        return
      }
      if (state.kind === 'corner') {
        moveCornerDrag(ctx, p, state.drag)
        return
      }
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
      if (state.kind === 'edge') endEdgeDrag(ctx, state.drag)
      else if (state.kind === 'corner') endCornerDrag(ctx, state.drag)
      else if (state.kind === 'table') ctx.session.setMarquee(null)
      else if (state.kind === 'drag') finishDrag(ctx, ctx.world(p))
      // A tap (no drag) on one piece of a group selects just that piece.
      else if (state.kind === 'press' && state.narrowTo) ctx.session.select([state.narrowTo])
      state = { kind: 'idle' }
    },

    cancel(ctx) {
      if (state.kind === 'edge') cancelEdgeDrag(ctx, state.drag)
      if (state.kind === 'corner') cancelCornerDrag(ctx, state.drag)
      // A box called off (a second finger down, say) leaves the choice as it was.
      if (state.kind === 'table') {
        ctx.session.setMarquee(null)
        ctx.session.select(state.before)
      }
      finishDrag(ctx, null)
      state = { kind: 'idle' }
    },

    hover(p, ctx) {
      const { session } = ctx
      if (!p) {
        session.setHover(null)
        if (session.ui.get().edgeHandle) {
          session.setEdgeHandle(null)
          session.showTip(null)
        }
        return
      }
      // Over a handle: say what dragging it does.
      const handle = handleUnder(session, p)
      if (handle) {
        const { handles, kind } = handle
        const at = handles[kind]
        session.setEdgeHandle({ piece: handles.piece, edge: handles.edge, kind, dragging: false })
        session.showTip({ x: at.x, y: at.y - HANDLE_RADIUS, text: t.edgeMove.handle[kind] })
        return
      }
      const corner = cornerUnder(session, p)
      if (corner) {
        const { at } = corner
        session.setEdgeHandle({
          piece: corner.piece,
          edge: corner.edge,
          kind: 'corner',
          dragging: false,
        })
        session.showTip({ x: at.x, y: at.y - HANDLE_RADIUS, text: t.cornerMove.handle })
        return
      }
      if (session.ui.get().edgeHandle) {
        session.setEdgeHandle(null)
        session.showTip(null)
      }
      // On the way from an edge to its handles, over nothing else, the
      // edge stays shown.
      const hit = ctx.hit(p)
      const hover = session.ui.get().hover
      const clear = !hit || (hit.kind === 'piece' && hit.piece === hover?.piece)
      const onTheWay =
        clear &&
        ((hover?.kind === 'edge' &&
          shownHandles(session).some(
            (h) => h.piece === hover.piece && h.edge === hover.edge && towardHandles(h, p),
          )) ||
          (hover?.kind === 'corner' &&
            shownCorners(session).some(
              (h) => h.piece === hover.piece && h.edge === hover.edge && towardCorner(h, p),
            )))
      if (!onTheWay) session.setHover(hit)
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
