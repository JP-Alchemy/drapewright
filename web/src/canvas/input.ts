// Connects the canvas element's DOM events to the input router and the
// active tool. Everything gesture-related is decided in @drapewright/input; this
// file only translates events.

import {
  type InputHandlers,
  InputRouter,
  type PointerKind,
  type RawPointer,
  hitRadius,
} from '@drapewright/input'
import { isTyping } from '../app/shortcuts.ts'
import type { PartRef, Session, ToolId } from '../state/session.ts'
import type { Tool } from '../tools/tool.ts'
import { toolContext } from '../tools/tool.ts'
import { type Point, panBy, scaleOf, toWorld, zoomAt } from './viewport.ts'

interface SafariGestureEvent extends UIEvent {
  scale: number
  clientX: number
  clientY: number
}

const kindOf = (e: PointerEvent): PointerKind =>
  e.pointerType === 'pen' ? 'pen' : e.pointerType === 'touch' ? 'touch' : 'mouse'

/** Opens the context menu for whatever is at a screen point. */
export function openMenuAt(session: Session, x: number, y: number, kind: PointerKind): void {
  const v = session.ui.get().viewport
  const w = toWorld(v, { x, y })
  const hit = session.engine.hitTest(w.x, w.y, hitRadius(kind) / scaleOf(v))
  if (hit) {
    const part: PartRef | null =
      hit.kind === 'edge' || hit.kind === 'corner'
        ? { kind: hit.kind, piece: hit.piece, edge: hit.edge }
        : hit.kind === 'notch'
          ? { kind: 'notch', piece: hit.piece, notch: hit.notch }
          : null
    session.select([hit.piece], part)
  }
  session.openMenu({ x, y, hit })
}

export function attachInput(
  canvas: HTMLCanvasElement,
  session: Session,
  tools: Record<ToolId, Tool>,
): { router: InputRouter; detach: () => void } {
  const ctx = toolContext(session)
  const tool = () => tools[session.ui.get().tool]

  const handlers: InputHandlers = {
    pan: (dx, dy) => session.setViewport((v) => panBy(v, dx, dy)),
    zoom: (cx, cy, factor) => session.setViewport((v) => zoomAt(v, { x: cx, y: cy }, factor)),
    undo: () => session.undo(true),
    redo: () => session.redo(true),
    toolDown: (p) => tool().down(p, ctx),
    toolMove: (p) => tool().move(p, ctx),
    toolUp: (p) => tool().up(p, ctx),
    toolCancel: () => tool().cancel(ctx),
    hover: (p) => tool().hover?.(p, ctx),
    contextMenu: (x, y, kind) => openMenuAt(session, x, y, kind),
    hold: (p) => tool().hold?.(p, ctx),
  }
  const router = new InputRouter(handlers, {
    after(ms, fn) {
      const id = setTimeout(fn, ms)
      return () => clearTimeout(id)
    },
  })

  const local = (e: { clientX: number; clientY: number }): Point => {
    const r = canvas.getBoundingClientRect()
    return { x: e.clientX - r.left, y: e.clientY - r.top }
  }
  const raw = (phase: RawPointer['phase'], e: PointerEvent): RawPointer => ({
    phase,
    id: e.pointerId,
    kind: kindOf(e),
    ...local(e),
    time: e.timeStamp,
    button: e.button,
    pressure: e.pressure,
    width: e.width,
    height: e.height,
    shift: e.shiftKey,
    alt: e.altKey,
    meta: e.metaKey,
    ctrl: e.ctrlKey,
  })

  let touches = 0
  const onPointerDown = (e: PointerEvent) => {
    if (e.pointerType === 'touch') touches++
    session.openMenu(null)
    try {
      canvas.setPointerCapture(e.pointerId)
    } catch {
      // Not an active pointer (synthetic events): capture is only a nicety.
    }
    router.handle(raw('down', e))
  }
  const onPointerMove = (e: PointerEvent) => {
    if (e.buttons === 0 && e.pointerType !== 'touch') {
      router.handle(raw('hover', e))
      return
    }
    // Coalesced events keep fast Pencil strokes smooth.
    const events = typeof e.getCoalescedEvents === 'function' ? e.getCoalescedEvents() : []
    for (const c of events.length > 0 ? events : [e]) router.handle(raw('move', c))
  }
  const onPointerEnd = (phase: 'up' | 'cancel') => (e: PointerEvent) => {
    if (e.pointerType === 'touch') touches = Math.max(0, touches - 1)
    router.handle(raw(phase, e))
  }
  const onPointerUp = onPointerEnd('up')
  const onPointerCancel = onPointerEnd('cancel')
  const onPointerLeave = (e: PointerEvent) => {
    if (e.buttons === 0) tool().hover?.(null, ctx)
  }
  const onContextMenu = (e: Event) => e.preventDefault()

  // Trackpad and mouse wheel: pinch or ⌘/Ctrl+scroll zooms, scroll pans.
  const onWheel = (e: WheelEvent) => {
    e.preventDefault()
    const p = local(e)
    if (e.ctrlKey || e.metaKey) {
      const factor = Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.01))
      session.setViewport((v) => zoomAt(v, p, factor))
    } else {
      const [dx, dy] = e.shiftKey ? [e.deltaY, e.deltaX] : [e.deltaX, e.deltaY]
      session.setViewport((v) => panBy(v, -dx, -dy))
    }
  }

  // Safari reports Mac trackpad pinches as gesture events. On iPad it also
  // sends them alongside touch pointers, which the router already handles.
  let gestureScale = 1
  const onGestureStart = (e: Event) => {
    e.preventDefault()
    gestureScale = 1
  }
  const onGestureChange = (e: Event) => {
    e.preventDefault()
    if (touches > 0) return
    const g = e as SafariGestureEvent
    const factor = g.scale / gestureScale
    gestureScale = g.scale
    session.setViewport((v) => zoomAt(v, local(g), factor))
  }

  const onBlur = () => router.reset()
  const onVisibility = () => {
    if (document.hidden) router.reset()
  }
  // Holding the space bar turns a mouse drag into a pan.
  const onSpace = (down: boolean) => (e: KeyboardEvent) => {
    if (e.code !== 'Space' || isTyping(e.target)) return
    e.preventDefault()
    router.setPanModifier(down)
  }
  const onSpaceDown = onSpace(true)
  const onSpaceUp = onSpace(false)
  window.addEventListener('keydown', onSpaceDown)
  window.addEventListener('keyup', onSpaceUp)

  canvas.addEventListener('pointerdown', onPointerDown)
  canvas.addEventListener('pointermove', onPointerMove)
  canvas.addEventListener('pointerup', onPointerUp)
  canvas.addEventListener('pointercancel', onPointerCancel)
  canvas.addEventListener('pointerleave', onPointerLeave)
  canvas.addEventListener('contextmenu', onContextMenu)
  canvas.addEventListener('wheel', onWheel, { passive: false })
  canvas.addEventListener('gesturestart', onGestureStart)
  canvas.addEventListener('gesturechange', onGestureChange)
  window.addEventListener('blur', onBlur)
  document.addEventListener('visibilitychange', onVisibility)

  return {
    router,
    detach() {
      canvas.removeEventListener('pointerdown', onPointerDown)
      canvas.removeEventListener('pointermove', onPointerMove)
      canvas.removeEventListener('pointerup', onPointerUp)
      canvas.removeEventListener('pointercancel', onPointerCancel)
      canvas.removeEventListener('pointerleave', onPointerLeave)
      canvas.removeEventListener('contextmenu', onContextMenu)
      canvas.removeEventListener('wheel', onWheel)
      canvas.removeEventListener('gesturestart', onGestureStart)
      canvas.removeEventListener('gesturechange', onGestureChange)
      window.removeEventListener('blur', onBlur)
      document.removeEventListener('visibilitychange', onVisibility)
      window.removeEventListener('keydown', onSpaceDown)
      window.removeEventListener('keyup', onSpaceUp)
    },
  }
}
