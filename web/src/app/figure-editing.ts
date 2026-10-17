// Editing on the figure: while it is on, the sim worker tells the page the
// pieces' edges as it draws them, and they are drawn over the 3D view.
// Hovering an edge shows it, its length and its handles, on the figure and
// on the table alike; a tap chooses it; a handle dragged moves it, as on the
// table. The garment drapes again once the pattern stops changing.

import type { EdgeMove, SeenEdge } from '@drapewright/engine'
import { TUNING } from '@drapewright/input'
import { type RefObject, useEffect } from 'react'
import { handleAt, towardHandles } from '../canvas/handles.ts'
import { readTheme } from '../canvas/renderer.ts'
import type { Point } from '../canvas/viewport.ts'
import type { Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import {
  type EdgeMoving,
  beginEdgeMove,
  cancelEdgeMove,
  endEdgeMove,
  setEdgeMove,
} from '../tools/edge-move.ts'
import { sim } from '../workers/sim-client.ts'
import { copyOf, drawOverlay, edgeNear, mmDragged, seenHandles } from './avatar-edit.ts'

/** A handle on the figure being dragged. */
interface Drag {
  moving: EdgeMoving
  /** The copy of the edge its handle is beside, as seen when pressed. */
  copy: SeenEdge
  start: Point
  pointer: number
  kind: 'mouse' | 'pen' | 'touch'
  /** Past a tap's slop. */
  dragging: boolean
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
    // The copy of an edge under the pointer, the handle under it, a drag,
    // and what is said beside the pointer.
    let hovered: SeenEdge | null = null
    let active: { piece: string; edge: string; kind: EdgeMove } | null = null
    let drag: Drag | null = null
    let tip: { at: Point; text: string } | null = null
    /** The session's hovered edge was set from here. */
    let ours = false
    let frame = 0

    const size = () => ({ width: el.clientWidth, height: el.clientHeight })
    const label = (piece: string, edge: string) => {
      const ev = session.scenePiece(piece)?.view.edges.find((e) => e.id === edge)
      const units = session.engine.document().settings.displayUnits
      return ev ? formatLength(ev.length, units) : ''
    }
    /** The copy with the length and handles: dragged, hovered, else the chosen edge's. */
    const handled = () => {
      const part = session.ui.get().part
      return (
        drag?.copy ??
        hovered ??
        (part?.kind === 'edge' ? copyOf(seen.current ?? [], part.piece, part.edge) : null)
      )
    }
    const layout = () => {
      const copy = handled()
      if (!copy) return null
      const text = label(copy.piece, copy.edge)
      return { copy, label: text, layout: seenHandles(copy, text, size()) }
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
      const strong = [ui.hover, ui.part].flatMap((r) =>
        r?.kind === 'edge' ? [{ piece: r.piece, edge: r.edge }] : [],
      )
      const handles = layout()
      const moved = drag?.dragging ? drag.moving.by - drag.moving.base : 0
      drawOverlay(
        ctx,
        all,
        {
          strong,
          handles,
          active:
            active && handles?.copy.piece === active.piece && handles.copy.edge === active.edge
              ? { kind: active.kind, dragging: drag !== null }
              : null,
          ghost:
            drag && moved !== 0
              ? {
                  copy: drag.copy,
                  offset: { x: drag.copy.out[0] * moved, y: drag.copy.out[1] * moved },
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
      // The copy hovered, as now seen.
      if (hovered) {
        const { piece, edge, middle } = hovered
        const was = { x: hovered.points[2 * middle]!, y: hovered.points[2 * middle + 1]! }
        hovered = outline ? copyOf(outline, piece, edge, was) : null
      }
      invalidate()
    }, window.devicePixelRatio || 1)
    const unsubscribeUi = session.ui.subscribe(invalidate)
    const unsubscribeEngine = session.engine.subscribe(invalidate)

    const local = (e: PointerEvent): Point => {
      const r = el.getBoundingClientRect()
      return { x: e.clientX - r.left, y: e.clientY - r.top }
    }
    const setHover = (e: SeenEdge | null) => {
      if (e) {
        session.setHover({ kind: 'edge', piece: e.piece, edge: e.edge, along: 0, distance: 0 })
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

    // Ahead of the view's own turning: a handle pressed is held here.
    const onDown = (e: PointerEvent) => {
      const p = local(e)
      const h = layout()
      const kind = h && handleAt(h.layout, p)
      const piece = h && session.engine.document().pieces.find((x) => x.id === h.copy.piece)
      if (!h || !kind || !piece) return
      e.stopImmediatePropagation()
      e.preventDefault()
      try {
        el.setPointerCapture(e.pointerId)
      } catch {
        // Not an active pointer (synthetic events): capture is only a nicety.
      }
      drag = {
        moving: beginEdgeMove(session, piece, h.copy.edge, kind),
        copy: h.copy,
        start: p,
        pointer: e.pointerId,
        kind: e.pointerType === 'pen' ? 'pen' : e.pointerType === 'touch' ? 'touch' : 'mouse',
        dragging: false,
      }
      active = { piece: piece.id, edge: h.copy.edge, kind }
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
        const mm = mmDragged(drag.copy, p.x - drag.start.x, p.y - drag.start.y)
        say({ at: p, text: setEdgeMove(session, drag.moving, mm) })
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
      active = null
      tip = null
      // On an edge; or on the way from one to its handles.
      const near = edgeNear(seen.current ?? [], p)
      if (near) {
        hovered = near
        setHover(near)
      } else if (!(hovered && h && h.copy === hovered && towardHandles(h.layout, p))) {
        hovered = null
        setHover(null)
      }
      invalidate()
    }
    const onEnd = (cancelled: boolean) => (e: PointerEvent) => {
      if (!drag || e.pointerId !== drag.pointer) return
      e.stopImmediatePropagation()
      if (cancelled) cancelEdgeMove(session, drag.moving)
      else endEdgeMove(session, drag.moving)
      drag = null
      active = null
      say(null)
    }
    const onUp = onEnd(false)
    const onCancel = onEnd(true)
    const onLeave = () => {
      if (drag) return
      hovered = null
      active = null
      setHover(null)
      say(null)
    }
    // With the debugging handle (`?debug`), for the tests: the edges seen
    // and the handles shown.
    const handle = window as unknown as { drapewright?: unknown; drapewrightFigure?: unknown }
    if (handle.drapewright) {
      handle.drapewrightFigure = {
        seen: () => seen.current,
        handles: () => layout()?.layout ?? null,
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
      if (drag) cancelEdgeMove(session, drag.moving)
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
