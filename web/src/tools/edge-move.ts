// Dragging an edge out or in by the handles beside its length, on the
// table or on the figure: extended (the edges either side carry on the way
// they run) or extruded (a strip added with square sides). The drag is an
// alteration step, shown as it goes, and one undo step. Dragging the same
// edge the same way again, while that is the piece's last step, changes
// that step rather than adding another.

import type { Alteration, Change, EdgeMove, Piece } from '@drapewright/engine'
import type { ToolPointer } from '@drapewright/input'
import { TUNING } from '@drapewright/input'
import { type EdgeHandles, edgeHandles, handleAt } from '../canvas/handles.ts'
import { placement } from '../canvas/renderer.ts'
import type { Point } from '../canvas/viewport.ts'
import type { EdgeRef, Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import type { ToolContext } from './tool.ts'

/** The handles shown: beside the hovered edge's length, and the chosen edge's. */
export function shownHandles(session: Session): EdgeHandles[] {
  const ui = session.ui.get()
  if (ui.tool !== 'select') return []
  const units = session.engine.document().settings.displayUnits
  const refs = [ui.hover, ui.part].flatMap((r) => (r?.kind === 'edge' ? [r] : []))
  return refs.flatMap((ref) => {
    const sp = session.scenePiece(ref.piece)
    const ev = sp?.view.edges.find((e) => e.id === ref.edge)
    if (!sp || !ev) return []
    return [
      edgeHandles(
        sp.piece.id,
        ev,
        placement(sp.piece),
        ui.viewport,
        formatLength(ev.length, units),
      ),
    ]
  })
}

/** The handle shown at a screen point, if any. */
export function handleUnder(
  session: Session,
  p: Point,
): { handles: EdgeHandles; kind: EdgeMove } | null {
  for (const handles of shownHandles(session)) {
    const kind = handleAt(handles, p)
    if (kind) return { handles, kind }
  }
  return null
}

/** The step a drag of this edge this way changes: the piece's last, if it moves the same edge the same way. */
export function lastStep(piece: Piece, edge: string, mode: EdgeMove): Alteration | null {
  const last = piece.alterations.at(-1)
  const change = last?.change
  return last?.on && change?.type === 'moveEdge' && change.edge === edge && change.mode === mode
    ? last
    : null
}

/**
 * An edge being moved, on the table or on the figure: the step its drag
 * sets as it goes.
 */
export interface EdgeMoving {
  piece: string
  edge: string
  mode: EdgeMove
  /** How far the step moved the edge before, and moves it now, mm. */
  base: number
  by: number
  /** The step being changed; whether this move added it. */
  step: string | null
  added: boolean
  /** Why it was last refused, if it was. */
  refused: string | null
  gesture: number
}

let gestures = 0

/**
 * Starts moving an edge, choosing it (unless `choose` is false). The step
 * changed is its piece's last, if that moves this edge this way; else one
 * is added once it moves. Edges moved together share a `gesture`: one undo
 * step.
 */
export function beginEdgeMove(
  session: Session,
  piece: Piece,
  edge: string,
  mode: EdgeMove,
  { choose = true, gesture = ++gestures }: { choose?: boolean; gesture?: number } = {},
): EdgeMoving {
  const last = lastStep(piece, edge, mode)
  const base = last?.change.type === 'moveEdge' ? last.change.by : 0
  if (choose) session.select([piece.id], { kind: 'edge', piece: piece.id, edge })
  return {
    piece: piece.id,
    edge,
    mode,
    base,
    by: base,
    step: last?.id ?? null,
    added: false,
    refused: null,
    gesture,
  }
}

/**
 * Starts moving `edge`, and with it every edge chosen with it: all of them
 * if it is one of several chosen, else it alone (chosen).
 */
export function beginEdgesMove(session: Session, edge: EdgeRef, mode: EdgeMove): EdgeMoving[] {
  const doc = session.engine.document()
  const piece = (id: string) => doc.pieces.find((p) => p.id === id)
  const chosen = session.chosenEdges()
  const together =
    chosen.length > 1 && chosen.some((e) => e.piece === edge.piece && e.edge === edge.edge)
  if (!together) {
    const p = piece(edge.piece)
    return p ? [beginEdgeMove(session, p, edge.edge, mode)] : []
  }
  const gesture = ++gestures
  // The one dragged first: its amount is the one said.
  const order = [edge, ...chosen.filter((e) => e.piece !== edge.piece || e.edge !== edge.edge)]
  return order.flatMap((e) => {
    const p = piece(e.piece)
    return p ? [beginEdgeMove(session, p, e.edge, mode, { choose: false, gesture })] : []
  })
}

/** Each edge moved `mm` further out than it was: what to say, as `setEdgeMove` (and how many). */
export function setEdgesMove(session: Session, movings: EdgeMoving[], mm: number): string {
  const said = movings.map((m) => setEdgeMove(session, m, mm))
  const refused = movings.find((m) => m.refused)?.refused
  if (refused) return refused
  return movings.length > 1 ? t.edgeMove.many(said[0]!, movings.length) : (said[0] ?? '')
}

/**
 * The edge moved `mm` further out than it was (in, less than 0), to the
 * nearest millimetre or tenth of an inch as lengths are shown: what to
 * say beside the pointer, how far it has moved or why it goes no further.
 */
export function setEdgeMove(session: Session, moving: EdgeMoving, mm: number): string {
  const units = session.engine.document().settings.displayUnits
  const unit = units === 'in' ? 2.54 : 1
  const by = Math.round((moving.base + mm) / unit) * unit
  const said = () =>
    t.edgeMove.tip(moving.mode, moving.by, formatLength(Math.abs(moving.by), units))
  if (by === moving.by) return said()
  const change: Change = { type: 'moveEdge', edge: moving.edge, by, mode: moving.mode }
  const options = { coalesce: `edge-move:${moving.gesture}` }
  // Straight to the engine: a move too far is said beside the pointer,
  // not as a notice each time.
  const result = moving.step
    ? session.engine.dispatch(
        {
          type: 'setAlteration',
          piece: moving.piece,
          alteration: { id: moving.step, on: true, change },
        },
        options,
      )
    : session.engine.dispatch(
        { type: 'addAlteration', piece: moving.piece, alteration: { id: '', on: true, change } },
        options,
      )
  if (result.status === 'rejected') {
    moving.refused = result.error.message
    return result.error.message
  }
  moving.refused = null
  if (!moving.step) {
    const piece = session.engine.document().pieces.find((x) => x.id === moving.piece)
    moving.step = piece?.alterations.at(-1)?.id ?? null
    moving.added = true
  }
  moving.by = by
  return said()
}

/** Let go: a step left moving nothing is taken away. */
export function endEdgeMove(session: Session, moving: EdgeMoving): void {
  if (moving.step && moving.by === 0) {
    session.engine.dispatch(
      { type: 'deleteAlteration', piece: moving.piece, alteration: moving.step },
      { coalesce: `edge-move:${moving.gesture}` },
    )
  }
}

/** Called off (a second finger down, say): the step back as it was. */
export function cancelEdgeMove(session: Session, moving: EdgeMoving): void {
  const options = { coalesce: `edge-move:${moving.gesture}` }
  if (moving.step && moving.added) {
    session.engine.dispatch(
      { type: 'deleteAlteration', piece: moving.piece, alteration: moving.step },
      options,
    )
  } else if (moving.step && moving.by !== moving.base) {
    const change: Change = {
      type: 'moveEdge',
      edge: moving.edge,
      by: moving.base,
      mode: moving.mode,
    }
    session.engine.dispatch(
      {
        type: 'setAlteration',
        piece: moving.piece,
        alteration: { id: moving.step, on: true, change },
      },
      options,
    )
  }
}

/** A handle on the table being dragged. */
export interface EdgeDrag {
  /** The edge dragged, first, and those chosen with it. */
  movings: EdgeMoving[]
  /** Where the press was, on the table and on screen. */
  start: Point
  screen: Point
  pointer: ToolPointer['kind']
  /** Out of the piece at the edge's middle, on the table: a unit vector. */
  out: Point
  /** Past a tap's slop. */
  dragging: boolean
}

/** Takes hold of an edge's handle on the table. */
export function startEdgeDrag(
  ctx: ToolContext,
  p: ToolPointer,
  handles: EdgeHandles,
  mode: EdgeMove,
): EdgeDrag | null {
  const { session } = ctx
  const sp = session.scenePiece(handles.piece)
  const ev = sp?.view.edges.find((e) => e.id === handles.edge)
  if (!sp || !ev) return null
  const m = placement(sp.piece)
  const [mx, my] = ev.middle
  const a = m.transformPoint(new DOMPoint(mx, my))
  const b = m.transformPoint(new DOMPoint(mx + ev.middleNormal[0], my + ev.middleNormal[1]))
  const n = Math.hypot(b.x - a.x, b.y - a.y) || 1
  const movings = beginEdgesMove(session, { piece: sp.piece.id, edge: handles.edge }, mode)
  session.setEdgeHandle({ piece: sp.piece.id, edge: handles.edge, kind: mode, dragging: true })
  session.showTip(null)
  return {
    movings,
    start: ctx.world(p),
    screen: { x: p.x, y: p.y },
    pointer: p.kind,
    out: { x: (b.x - a.x) / n, y: (b.y - a.y) / n },
    dragging: false,
  }
}

/** The edge moved as far as the pointer has gone out (or in) from where it was pressed. */
export function moveEdgeDrag(ctx: ToolContext, p: ToolPointer, drag: EdgeDrag): void {
  if (!drag.dragging) {
    const travel = Math.hypot(p.x - drag.screen.x, p.y - drag.screen.y)
    if (travel <= TUNING.dragSlop[drag.pointer]) return
    drag.dragging = true
  }
  const w = ctx.world(p)
  const mm = (w.x - drag.start.x) * drag.out.x + (w.y - drag.start.y) * drag.out.y
  const text = setEdgesMove(ctx.session, drag.movings, mm)
  ctx.session.showTip({ x: p.x, y: p.y, text })
}

/** Lets go of the handle. */
export function endEdgeDrag(ctx: ToolContext, drag: EdgeDrag): void {
  for (const m of drag.movings) endEdgeMove(ctx.session, m)
  ctx.session.setEdgeHandle(null)
  ctx.session.showTip(null)
}

/** The drag called off: the step back as it was. */
export function cancelEdgeDrag(ctx: ToolContext, drag: EdgeDrag): void {
  for (const m of drag.movings) cancelEdgeMove(ctx.session, m)
  ctx.session.setEdgeHandle(null)
  ctx.session.showTip(null)
}
