// Editing on the figure: while it is on, the sim worker tells the page the
// pieces' edges as it draws them, and they are drawn over the 3D view, a dot
// at each corner. Hovering an edge or a corner shows it, on the figure and
// on the table alike, with its handles: an edge's length and the handles to
// extend or extrude it, a corner's handle to move it. A tap chooses it; a
// handle dragged moves it, as on the table. The garment drapes again once
// the pattern stops changing.

import type { SeenEdge } from '@drapewright/engine'
import { type Modifiers, TUNING } from '@drapewright/input'
import { type RefObject, useEffect } from 'react'
import { edgeEnds } from '../canvas/edges.ts'
import {
  type HandleKind,
  handleAt,
  onCornerHandle,
  towardCorner,
  towardHandles,
} from '../canvas/handles.ts'
import { readTheme } from '../canvas/renderer.ts'
import type { Point } from '../canvas/viewport.ts'
import type { Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import {
  type CornerMoving,
  beginCornerMove,
  cancelCornerMove,
  endCornerMove,
  setCornerMove,
} from '../tools/corner-move.ts'
import {
  type EdgeMoving,
  beginEdgesMove,
  cancelEdgeMove,
  endEdgeMove,
  setEdgesMove,
} from '../tools/edge-move.ts'
import { sim } from '../workers/sim-client.ts'
import {
  FACING_ENOUGH,
  type SeenCorner,
  copyOf,
  cornerDragged,
  cornerShown,
  drawOverlay,
  mmDragged,
  seenCornerHandle,
  seenCorners,
  seenHandles,
  underPointer,
} from './avatar-edit.ts'

/** Where a drag began, by which pointer, and whether past a tap's slop. */
interface Pressed {
  start: Point
  pointer: number
  kind: 'mouse' | 'pen' | 'touch'
  dragging: boolean
}

/** A handle on the figure being dragged: an edge's, or a corner's. */
type Drag =
  | (Pressed & { of: 'edge'; movings: EdgeMoving[]; copy: SeenEdge })
  | (Pressed & {
      of: 'corner'
      moving: CornerMoving
      corner: SeenCorner
      /** The corner's edges' ways from it, on the piece. */
      ways: [Point, Point]
    })

/** The edge after `edge` on its piece, if the piece is there. */
function nextEdge(session: Session, piece: string, edge: string): string | null {
  const edges = session.engine.document().pieces.find((p) => p.id === piece)?.outline.edges
  if (!edges) return null
  const i = edges.findIndex((e) => e.id === edge)
  return i < 0 ? null : edges[(i + 1) % edges.length]!.id
}

/** The ways the edges at the corner ending `edge` leave it, on the piece: back along it, along the next. */
function cornerWays(session: Session, piece: string, edge: string): [Point, Point] | null {
  const p = session.engine.document().pieces.find((x) => x.id === piece)
  const next = nextEdge(session, piece, edge)
  const [a, b] = [p && edgeEnds(p, edge), p && next ? edgeEnds(p, next) : null]
  if (!a || !b) return null
  return [
    { x: a.end[0], y: a.end[1] },
    { x: b.start[0], y: b.start[1] },
  ]
}

/** The corners seen, by the edges seen. */
function cornersOf(session: Session, seen: SeenEdge[] | null): SeenCorner[] {
  return seen ? seenCorners(seen, (piece, edge) => nextEdge(session, piece, edge)) : []
}

/**
 * What a tap at `p` chooses while editing on the figure: a corner there,
 * else an edge; with Shift, the edge added to those chosen (or taken
 * away); with Ctrl or Alt (Option), the line round the garment it is part
 * of. Whether it chose one.
 */
export function chooseOnFigure(
  session: Session,
  seen: SeenEdge[] | null,
  p: Point,
  keys: Modifiers = { shift: false, alt: false, meta: false, ctrl: false },
): boolean {
  const under = seen ? underPointer(seen, cornersOf(session, seen), p) : null
  if (under && 'corner' in under) {
    const { piece, edge } = under.corner
    session.select([piece], { kind: 'corner', piece, edge })
  } else if (under) {
    const { piece, edge } = under.edge
    if (keys.ctrl || keys.alt) session.chooseLine({ piece, edge }, keys.shift)
    else if (keys.shift) session.toggleEdge({ piece, edge })
    else session.select([piece], { kind: 'edge', piece, edge })
  }
  return under !== null
}

/**
 * Edits on the figure in the view `host` while `on`, drawing on `overlay`
 * (over the view, letting the pointer through). `seen` is kept up to date
 * with the edges as last seen, for taps to choose by.
 */
export function useFigureEditing(
  session: Session,
  on: boolean,
  host: RefObject<HTMLElement | null>,
  overlay: RefObject<HTMLCanvasElement | null>,
  seen: { current: SeenEdge[] | null },
): void {
  useEffect(() => {
    const el = host.current
    const canvas = overlay.current
    const stage = el?.parentElement
    const ctx = canvas?.getContext('2d')
    if (!on || !el || !canvas || !stage || !ctx) return
    const theme = readTheme(canvas)
    // The corners seen; what is under the pointer (an edge's copy, or a
    // corner), the handle under it, a drag, and what is said beside the
    // pointer.
    let corners: SeenCorner[] = []
    let hovered: { edge: SeenEdge } | { corner: SeenCorner } | null = null
    let active: { piece: string; edge: string; kind: HandleKind } | null = null
    let drag: Drag | null = null
    let tip: { at: Point; text: string } | null = null
    /** The session's hovered edge or corner was set from here. */
    let ours = false
    let frame = 0

    const size = () => ({ width: el.clientWidth, height: el.clientHeight })
    const label = (piece: string, edge: string) => {
      const ev = session.scenePiece(piece)?.view.edges.find((e) => e.id === edge)
      const units = session.engine.document().settings.displayUnits
      return ev ? formatLength(ev.length, units) : ''
    }
    /** The edge's copy with the length and handles: dragged, hovered, else the chosen edge's. */
    const handled = () => {
      if (drag) return drag.of === 'edge' ? drag.copy : null
      if (hovered && 'edge' in hovered) return hovered.edge
      const part = session.ui.get().part
      return part?.kind === 'edge' ? copyOf(seen.current ?? [], part.piece, part.edge) : null
    }
    const layout = () => {
      const copy = handled()
      if (!copy) return null
      const text = label(copy.piece, copy.edge)
      return { copy, label: text, layout: seenHandles(copy, text, size()) }
    }
    /** The corner with a handle: dragged, hovered, else the chosen corner. */
    const cornerHandled = () => {
      if (drag) return drag.of === 'corner' ? drag.corner : null
      if (hovered && 'corner' in hovered) return hovered.corner
      const part = session.ui.get().part
      return part?.kind === 'corner'
        ? (corners.find((c) => c.piece === part.piece && c.edge === part.edge) ?? null)
        : null
    }
    /** Its handle, if it faces the eye squarely enough to be moved. */
    const cornerLayout = () => {
      const corner = cornerHandled()
      return corner && corner.facing >= FACING_ENOUGH
        ? { corner, handle: seenCornerHandle(corner, size()) }
        : null
    }

    const paint = () => {
      frame = 0
      const dpr = window.devicePixelRatio || 1
      const { width, height } = size()
      const [w, h] = [Math.max(1, Math.round(width * dpr)), Math.max(1, Math.round(height * dpr))]
      if (canvas.width !== w || canvas.height !== h) [canvas.width, canvas.height] = [w, h]
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
      ctx.clearRect(0, 0, width, height)
      const all = seen.current
      if (!all) return
      const ui = session.ui.get()
      const refs = (kind: 'edge' | 'corner') => [
        ...[ui.hover, ui.part].flatMap((r) =>
          r?.kind === kind ? [{ piece: r.piece, edge: r.edge }] : [],
        ),
        ...(kind === 'edge' ? ui.moreEdges : []),
      ]
      const handles = layout()
      const corner = cornerLayout()
      const first = drag?.of === 'edge' ? drag.movings[0] : undefined
      const moved = first && drag?.dragging ? first.by - first.base : 0
      const cornerTo =
        drag?.of === 'corner' && drag.dragging
          ? cornerShown(drag.corner, drag.ways, {
              x: drag.moving.by[0] - drag.moving.base[0],
              y: drag.moving.by[1] - drag.moving.base[1],
            })
          : null
      const activeOn = (piece: string, edge: string) =>
        active && active.piece === piece && active.edge === edge ? active.kind : null
      const edgeKind = handles ? activeOn(handles.copy.piece, handles.copy.edge) : null
      drawOverlay(
        ctx,
        all,
        {
          strong: refs('edge'),
          strongCorners: refs('corner'),
          handles,
          active:
            edgeKind && edgeKind !== 'corner' ? { kind: edgeKind, dragging: drag !== null } : null,
          ghost:
            drag?.of === 'edge' && moved !== 0
              ? {
                  copy: drag.copy,
                  offset: { x: drag.copy.out[0] * moved, y: drag.copy.out[1] * moved },
                }
              : null,
          corner: corner
            ? {
                handle: corner.handle,
                active: activeOn(corner.corner.piece, corner.corner.edge) === 'corner',
              }
            : null,
          cornerGhost:
            drag?.of === 'corner' && cornerTo
              ? {
                  from: drag.corner.at,
                  to: { x: drag.corner.at.x + cornerTo.x, y: drag.corner.at.y + cornerTo.y },
                }
              : null,
          tip,
        },
        theme,
      )
    }
    const invalidate = () => {
      frame ||= requestAnimationFrame(paint)
    }

    sim.outline((outline) => {
      seen.current = outline
      corners = cornersOf(session, outline)
      // What is hovered, as now seen.
      if (hovered && 'edge' in hovered) {
        const { piece, edge, middle } = hovered.edge
        const was = { x: hovered.edge.points[2 * middle]!, y: hovered.edge.points[2 * middle + 1]! }
        const copy = outline ? copyOf(outline, piece, edge, was) : null
        hovered = copy ? { edge: copy } : null
      } else if (hovered && 'corner' in hovered) {
        const was = hovered.corner
        const corner = corners
          .filter((c) => c.piece === was.piece && c.edge === was.edge)
          .sort(
            (a, b) =>
              Math.hypot(a.at.x - was.at.x, a.at.y - was.at.y) -
              Math.hypot(b.at.x - was.at.x, b.at.y - was.at.y),
          )[0]
        hovered = corner ? { corner } : null
      }
      invalidate()
    }, window.devicePixelRatio || 1)
    const unsubscribeUi = session.ui.subscribe(invalidate)
    const unsubscribeEngine = session.engine.subscribe(invalidate)

    const local = (e: PointerEvent): Point => {
      const r = el.getBoundingClientRect()
      return { x: e.clientX - r.left, y: e.clientY - r.top }
    }
    const setHover = (to: { edge: SeenEdge } | { corner: SeenCorner } | null) => {
      hovered = to
      if (to && 'edge' in to) {
        const { piece, edge } = to.edge
        session.setHover({ kind: 'edge', piece, edge, along: 0, distance: 0 })
        ours = true
      } else if (to) {
        session.setHover({ kind: 'corner', piece: to.corner.piece, edge: to.corner.edge })
        ours = true
      } else if (ours) {
        session.setHover(null)
        ours = false
      }
    }
    const say = (next: { at: Point; text: string } | null) => {
      tip = next
      invalidate()
    }
    const pressed = (e: PointerEvent, p: Point): Pressed => ({
      start: p,
      pointer: e.pointerId,
      kind: e.pointerType === 'pen' ? 'pen' : e.pointerType === 'touch' ? 'touch' : 'mouse',
      dragging: false,
    })
    const hold = (e: PointerEvent) => {
      e.stopImmediatePropagation()
      e.preventDefault()
      try {
        el.setPointerCapture(e.pointerId)
      } catch {
        // Not an active pointer (synthetic events): capture is only a nicety.
      }
    }

    // Ahead of the view's own turning: a handle pressed is held here.
    const onDown = (e: PointerEvent) => {
      const p = local(e)
      const h = layout()
      const kind = h && handleAt(h.layout, p)
      const piece = h && session.engine.document().pieces.find((x) => x.id === h.copy.piece)
      if (h && kind && piece) {
        hold(e)
        drag = {
          ...pressed(e, p),
          of: 'edge',
          movings: beginEdgesMove(session, { piece: piece.id, edge: h.copy.edge }, kind),
          copy: h.copy,
        }
        active = { piece: piece.id, edge: h.copy.edge, kind }
        say(null)
        return
      }
      const c = cornerLayout()
      const ways = c && cornerWays(session, c.corner.piece, c.corner.edge)
      const owner = c && session.engine.document().pieces.find((x) => x.id === c.corner.piece)
      // A corner seen edge on can't be told which way it goes: left be.
      if (!c || !ways || !owner || !onCornerHandle(c.handle, p)) return
      if (!cornerDragged(c.corner, ways, { x: 1, y: 0 })) return
      hold(e)
      drag = {
        ...pressed(e, p),
        of: 'corner',
        moving: beginCornerMove(session, owner, c.corner.edge),
        corner: c.corner,
        ways,
      }
      active = { piece: owner.id, edge: c.corner.edge, kind: 'corner' }
      say(null)
    }
    const onMove = (e: PointerEvent) => {
      const p = local(e)
      if (drag && e.pointerId === drag.pointer) {
        e.stopImmediatePropagation()
        if (!drag.dragging) {
          if (Math.hypot(p.x - drag.start.x, p.y - drag.start.y) <= TUNING.dragSlop[drag.kind]) {
            return
          }
          drag.dragging = true
        }
        const [dx, dy] = [p.x - drag.start.x, p.y - drag.start.y]
        if (drag.of === 'edge') {
          say({ at: p, text: setEdgesMove(session, drag.movings, mmDragged(drag.copy, dx, dy)) })
        } else {
          const d = cornerDragged(drag.corner, drag.ways, { x: dx, y: dy })
          if (d) say({ at: p, text: setCornerMove(session, drag.moving, d) })
        }
        return
      }
      if (drag || e.buttons !== 0 || e.pointerType === 'touch') return
      // Over a handle: what dragging it does.
      const h = layout()
      const kind = h && handleAt(h.layout, p)
      if (h && kind) {
        active = { piece: h.copy.piece, edge: h.copy.edge, kind }
        say({ at: h.layout[kind], text: t.edgeMove.handle[kind] })
        return
      }
      const c = cornerLayout()
      if (c && onCornerHandle(c.handle, p)) {
        active = { piece: c.corner.piece, edge: c.corner.edge, kind: 'corner' }
        say({ at: c.handle.at, text: t.cornerMove.handle })
        return
      }
      active = null
      tip = null
      // On a corner, or an edge; or on the way from one to its handles.
      const under = underPointer(seen.current ?? [], corners, p)
      const corner = under && 'corner' in under ? under.corner : null
      const edge = under && 'edge' in under ? under.edge : null
      const onTheWay =
        (hovered &&
          'edge' in hovered &&
          h &&
          h.copy === hovered.edge &&
          towardHandles(h.layout, p)) ||
        (hovered &&
          'corner' in hovered &&
          c &&
          c.corner === hovered.corner &&
          towardCorner(c.handle, p))
      if (corner) {
        setHover({ corner })
        // Seen from the side: say how to move it.
        if (corner.facing < FACING_ENOUGH) tip = { at: corner.at, text: t.cornerMove.turn }
      } else if (edge) setHover({ edge })
      else if (!onTheWay) setHover(null)
      invalidate()
    }
    const onEnd = (cancelled: boolean) => (e: PointerEvent) => {
      if (!drag || e.pointerId !== drag.pointer) return
      e.stopImmediatePropagation()
      if (drag.of === 'edge') {
        for (const m of drag.movings) {
          if (cancelled) cancelEdgeMove(session, m)
          else endEdgeMove(session, m)
        }
      } else if (cancelled) cancelCornerMove(session, drag.moving)
      else endCornerMove(session, drag.moving)
      drag = null
      active = null
      say(null)
    }
    const onUp = onEnd(false)
    const onCancel = onEnd(true)
    const onLeave = () => {
      if (drag) return
      active = null
      setHover(null)
      say(null)
    }
    // With the debugging handle (`?debug`), for the tests: the edges and
    // corners seen, and the handles shown.
    const handle = window as unknown as { drapewright?: unknown; drapewrightFigure?: unknown }
    if (handle.drapewright) {
      handle.drapewrightFigure = {
        seen: () => seen.current,
        corners: () =>
          corners.map((c) => ({ piece: c.piece, edge: c.edge, at: c.at, facing: c.facing })),
        handles: () => layout()?.layout ?? null,
        cornerHandle: () => cornerLayout()?.handle ?? null,
      }
    }
    const capture = { capture: true }
    stage.addEventListener('pointerdown', onDown, capture)
    stage.addEventListener('pointermove', onMove, capture)
    stage.addEventListener('pointerup', onUp, capture)
    stage.addEventListener('pointercancel', onCancel, capture)
    el.addEventListener('pointerleave', onLeave)
    invalidate()
    return () => {
      stage.removeEventListener('pointerdown', onDown, capture)
      stage.removeEventListener('pointermove', onMove, capture)
      stage.removeEventListener('pointerup', onUp, capture)
      stage.removeEventListener('pointercancel', onCancel, capture)
      el.removeEventListener('pointerleave', onLeave)
      cancelAnimationFrame(frame)
      delete handle.drapewrightFigure
      if (drag?.of === 'edge') for (const m of drag.movings) cancelEdgeMove(session, m)
      else if (drag) cancelCornerMove(session, drag.moving)
      sim.outline(null)
      unsubscribeUi()
      unsubscribeEngine()
      seen.current = null
      setHover(null)
      ctx.setTransform(1, 0, 0, 1, 0, 0)
      ctx.clearRect(0, 0, canvas.width, canvas.height)
    }
  }, [session, on, host, overlay, seen])
}
