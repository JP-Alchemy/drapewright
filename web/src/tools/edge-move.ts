// Dragging an edge out or in by the handles beside its length: extended
// (the edges either side carry on the way they run) or extruded (a strip
// added with square sides). The drag is an alteration step, shown on the
// table as it goes, and one undo step. Dragging the same edge the same way
// again, while that is the piece's last step, changes that step rather
// than adding another.

import type { Alteration, Change, EdgeMove, Piece } from '@drapewright/engine'
import type { ToolPointer } from '@drapewright/input'
import { TUNING } from '@drapewright/input'
import { type EdgeHandles, edgeHandles, handleAt } from '../canvas/handles.ts'
import { placement } from '../canvas/renderer.ts'
import type { Point } from '../canvas/viewport.ts'
import type { Session } from '../state/session.ts'
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

export interface EdgeDrag {
  piece: string
  edge: string
  mode: EdgeMove
  /** Where the press was, on the table and on screen. */
  start: Point
  screen: Point
  pointer: ToolPointer['kind']
  /** Out of the piece at the edge's middle, on the table: a unit vector. */
  out: Point
  /** How far the step moved the edge before the drag, and moves it now, mm. */
  base: number
  by: number
  /** The step being changed; whether this drag added it. */
  step: string | null
  added: boolean
  moving: boolean
  gesture: number
}

let gestures = 0

/** Takes hold of an edge's handle. */
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
  const last = lastStep(sp.piece, handles.edge, mode)
  const base = last?.change.type === 'moveEdge' ? last.change.by : 0
  session.select([sp.piece.id], { kind: 'edge', piece: sp.piece.id, edge: handles.edge })
  session.setEdgeHandle({ piece: sp.piece.id, edge: handles.edge, kind: mode, dragging: true })
  session.showTip(null)
  return {
    piece: sp.piece.id,
    edge: handles.edge,
    mode,
    start: ctx.world(p),
    screen: { x: p.x, y: p.y },
    pointer: p.kind,
    out: { x: (b.x - a.x) / n, y: (b.y - a.y) / n },
    base,
    by: base,
    step: last?.id ?? null,
    added: false,
    moving: false,
    gesture: ++gestures,
  }
}

/** The edge moved as far as the pointer has gone out (or in) from where it was pressed. */
export function moveEdgeDrag(ctx: ToolContext, p: ToolPointer, drag: EdgeDrag): void {
  if (!drag.moving) {
    const travel = Math.hypot(p.x - drag.screen.x, p.y - drag.screen.y)
    if (travel <= TUNING.dragSlop[drag.pointer]) return
    drag.moving = true
  }
  const { session } = ctx
  const units = session.engine.document().settings.displayUnits
  // To the nearest millimetre, or tenth of an inch, as lengths are shown.
  const unit = units === 'in' ? 2.54 : 1
  const w = ctx.world(p)
  const dragged = (w.x - drag.start.x) * drag.out.x + (w.y - drag.start.y) * drag.out.y
  const by = Math.round((drag.base + dragged) / unit) * unit
  const tip = (text: string) => session.showTip({ x: p.x, y: p.y, text })
  if (by === drag.by) {
    tip(t.edgeMove.tip(drag.mode, by, formatLength(Math.abs(by), units)))
    return
  }
  const change: Change = { type: 'moveEdge', edge: drag.edge, by, mode: drag.mode }
  const options = { coalesce: `edge-move:${drag.gesture}` }
  // Straight to the engine: a move too far is said beside the pointer,
  // not as a notice each time.
  const result = drag.step
    ? session.engine.dispatch(
        {
          type: 'setAlteration',
          piece: drag.piece,
          alteration: { id: drag.step, on: true, change },
        },
        options,
      )
    : session.engine.dispatch(
        { type: 'addAlteration', piece: drag.piece, alteration: { id: '', on: true, change } },
        options,
      )
  if (result.status === 'rejected') {
    tip(result.error.message)
    return
  }
  if (!drag.step) {
    const piece = session.engine.document().pieces.find((x) => x.id === drag.piece)
    drag.step = piece?.alterations.at(-1)?.id ?? null
    drag.added = true
  }
  drag.by = by
  tip(t.edgeMove.tip(drag.mode, by, formatLength(Math.abs(by), units)))
}

/** Lets go: a step left moving nothing is taken away. */
export function endEdgeDrag(ctx: ToolContext, drag: EdgeDrag): void {
  const { session } = ctx
  if (drag.step && drag.by === 0) {
    session.engine.dispatch(
      { type: 'deleteAlteration', piece: drag.piece, alteration: drag.step },
      { coalesce: `edge-move:${drag.gesture}` },
    )
  }
  session.setEdgeHandle(null)
  session.showTip(null)
}

/** Called off (a second finger down, say): the step back as it was. */
export function cancelEdgeDrag(ctx: ToolContext, drag: EdgeDrag): void {
  const { session } = ctx
  const options = { coalesce: `edge-move:${drag.gesture}` }
  if (drag.step && drag.added) {
    session.engine.dispatch(
      { type: 'deleteAlteration', piece: drag.piece, alteration: drag.step },
      options,
    )
  } else if (drag.step && drag.by !== drag.base) {
    const change: Change = { type: 'moveEdge', edge: drag.edge, by: drag.base, mode: drag.mode }
    session.engine.dispatch(
      { type: 'setAlteration', piece: drag.piece, alteration: { id: drag.step, on: true, change } },
      options,
    )
  }
  session.setEdgeHandle(null)
  session.showTip(null)
}
