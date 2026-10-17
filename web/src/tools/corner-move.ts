// Moving a corner by the handle beside it, on the table or on the figure:
// the corner moves, the two edges meeting there drawn to it. The drag is an
// alteration step, shown as it goes, and one undo step. Dragging the same
// corner again, while that is the piece's last step, changes that step
// rather than adding another.

import type { Alteration, Change, Piece } from '@drapewright/engine'
import type { ToolPointer } from '@drapewright/input'
import { TUNING } from '@drapewright/input'
import { type CornerHandle, onCornerHandle, pieceCornerHandle } from '../canvas/handles.ts'
import { placement } from '../canvas/renderer.ts'
import type { Point } from '../canvas/viewport.ts'
import type { Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import type { ToolContext } from './tool.ts'

/** Whether a step moves the corner at the end of `edge`. */
function movesCorner(piece: Piece, change: Change, edge: string): boolean {
  if (change.type !== 'moveCorner') return false
  const edges = piece.outline.edges
  const next = edges[(edges.findIndex((e) => e.id === edge) + 1) % edges.length]?.id
  return change.start ? change.edge === next : change.edge === edge
}

/** The piece's steps moving the corner at the end of `edge`. */
export function cornerSteps(piece: Piece, edge: string): Alteration[] {
  return piece.alterations.filter((a) => movesCorner(piece, a.change, edge))
}

/** "2.0 cm right, 1.0 cm up", or nothing. */
export function cornerMoves(by: [number, number], units: Parameters<typeof formatLength>[1]) {
  const [across, down] = by
  return [
    ...(Math.abs(across) >= 0.05
      ? [t.alter.cornerAcross(across > 0, formatLength(Math.abs(across), units))]
      : []),
    ...(Math.abs(down) >= 0.05
      ? [t.alter.cornerDown(down > 0, formatLength(Math.abs(down), units))]
      : []),
  ]
}

/** A corner being moved, on the table or on the figure: the step its drag sets as it goes. */
export interface CornerMoving {
  piece: string
  /** The edge it ends. */
  edge: string
  /** The step's own way of naming the corner (a mirrored piece's runs the other way round). */
  target: { edge: string; start: boolean }
  /** How far the step moved it before, and moves it now, mm across and down. */
  base: [number, number]
  by: [number, number]
  step: string | null
  added: boolean
  gesture: number
}

let gestures = 0

/** Starts moving a corner, choosing it. */
export function beginCornerMove(session: Session, piece: Piece, edge: string): CornerMoving {
  const last = piece.alterations.at(-1)
  const change = last?.on && movesCorner(piece, last.change, edge) ? last.change : null
  const moved = change?.type === 'moveCorner' ? change : null
  session.select([piece.id], { kind: 'corner', piece: piece.id, edge })
  const base: [number, number] = moved ? [moved.by[0], moved.by[1]] : [0, 0]
  return {
    piece: piece.id,
    edge,
    target: moved ? { edge: moved.edge, start: moved.start } : { edge, start: false },
    base,
    by: base,
    step: moved ? last!.id : null,
    added: false,
    gesture: ++gestures,
  }
}

/**
 * The corner moved `d` (mm across and down the piece) further than it was,
 * to the nearest millimetre or tenth of an inch: what to say beside the
 * pointer, how far it has moved or why it goes no further.
 */
export function setCornerMove(session: Session, moving: CornerMoving, d: Point): string {
  const units = session.engine.document().settings.displayUnits
  const unit = units === 'in' ? 2.54 : 1
  const snap = (mm: number) => Math.round(mm / unit) * unit
  const by: [number, number] = [snap(moving.base[0] + d.x), snap(moving.base[1] + d.y)]
  const said = () => t.cornerMove.tip(cornerMoves(moving.by, units))
  if (by[0] === moving.by[0] && by[1] === moving.by[1]) return said()
  const change: Change = { type: 'moveCorner', ...moving.target, by }
  const options = { coalesce: `corner-move:${moving.gesture}` }
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
  if (result.status === 'rejected') return result.error.message
  if (!moving.step) {
    const piece = session.engine.document().pieces.find((x) => x.id === moving.piece)
    moving.step = piece?.alterations.at(-1)?.id ?? null
    moving.added = true
  }
  moving.by = by
  return said()
}

/** Let go: a step left moving nothing is taken away. */
export function endCornerMove(session: Session, moving: CornerMoving): void {
  if (moving.step && moving.by[0] === 0 && moving.by[1] === 0) {
    session.engine.dispatch(
      { type: 'deleteAlteration', piece: moving.piece, alteration: moving.step },
      { coalesce: `corner-move:${moving.gesture}` },
    )
  }
}

/** Called off: the step back as it was. */
export function cancelCornerMove(session: Session, moving: CornerMoving): void {
  const options = { coalesce: `corner-move:${moving.gesture}` }
  if (moving.step && moving.added) {
    session.engine.dispatch(
      { type: 'deleteAlteration', piece: moving.piece, alteration: moving.step },
      options,
    )
  } else if (moving.step) {
    const change: Change = { type: 'moveCorner', ...moving.target, by: moving.base }
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

/** The corner handles shown on the table: beside the hovered corner and the chosen one. */
export function shownCorners(session: Session): CornerHandle[] {
  const ui = session.ui.get()
  if (ui.tool !== 'select') return []
  const refs = [ui.hover, ui.part].flatMap((r) => (r?.kind === 'corner' ? [r] : []))
  return refs.flatMap((ref) => {
    const sp = session.scenePiece(ref.piece)
    const h = sp && pieceCornerHandle(sp.piece, ref.edge, placement(sp.piece), ui.viewport)
    return h ? [h] : []
  })
}

/** The corner handle shown at a screen point, if any. */
export function cornerUnder(session: Session, p: Point): CornerHandle | null {
  return shownCorners(session).find((h) => onCornerHandle(h, p)) ?? null
}

/** A corner's handle on the table being dragged. */
export interface CornerDrag {
  moving: CornerMoving
  /** Where the press was, on the table and on screen. */
  start: Point
  screen: Point
  pointer: ToolPointer['kind']
  /** How the piece is turned on the table, degrees. */
  rotation: number
  dragging: boolean
}

/** Takes hold of a corner's handle on the table. */
export function startCornerDrag(
  ctx: ToolContext,
  p: ToolPointer,
  handle: CornerHandle,
): CornerDrag | null {
  const { session } = ctx
  const piece = session.engine.document().pieces.find((x) => x.id === handle.piece)
  if (!piece) return null
  const moving = beginCornerMove(session, piece, handle.edge)
  session.setEdgeHandle({ piece: piece.id, edge: handle.edge, kind: 'corner', dragging: true })
  session.showTip(null)
  return {
    moving,
    start: ctx.world(p),
    screen: { x: p.x, y: p.y },
    pointer: p.kind,
    rotation: piece.placement.rotation,
    dragging: false,
  }
}

/** The corner moved as far as the pointer has gone, in the piece's own directions. */
export function moveCornerDrag(ctx: ToolContext, p: ToolPointer, drag: CornerDrag): void {
  if (!drag.dragging) {
    const travel = Math.hypot(p.x - drag.screen.x, p.y - drag.screen.y)
    if (travel <= TUNING.dragSlop[drag.pointer]) return
    drag.dragging = true
  }
  const w = ctx.world(p)
  const turn = (-drag.rotation * Math.PI) / 180
  const [dx, dy] = [w.x - drag.start.x, w.y - drag.start.y]
  const d = {
    x: dx * Math.cos(turn) - dy * Math.sin(turn),
    y: dx * Math.sin(turn) + dy * Math.cos(turn),
  }
  const text = setCornerMove(ctx.session, drag.moving, d)
  ctx.session.showTip({ x: p.x, y: p.y, text })
}

/** Lets go of the handle. */
export function endCornerDrag(ctx: ToolContext, drag: CornerDrag): void {
  endCornerMove(ctx.session, drag.moving)
  ctx.session.setEdgeHandle(null)
  ctx.session.showTip(null)
}

/** The drag called off: the step back as it was. */
export function cancelCornerDrag(ctx: ToolContext, drag: CornerDrag): void {
  cancelCornerMove(ctx.session, drag.moving)
  ctx.session.setEdgeHandle(null)
  ctx.session.showTip(null)
}
