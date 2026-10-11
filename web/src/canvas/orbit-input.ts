// Turning, zooming and sliding the 3D view, through the same input router
// as the cutting table: one finger, the Pencil or the mouse turns the
// figure; two fingers slide it and pinch to zoom; the wheel zooms; a
// double tap, or 0, shows it whole again; the arrow keys turn it. A tap
// that doesn't move points at what is there.

import { InputRouter, type RawPointer, type ToolPointer } from '@drapewright/input'
import { isTyping } from '../app/shortcuts.ts'
import { rawPointer } from './input.ts'

export interface Orbiting {
  /** Turn for a drag of `dx`, `dy` CSS pixels. */
  turn(dx: number, dy: number): void
  zoom(factor: number): void
  /** Slide for a drag of `dx`, `dy` CSS pixels. */
  slide(dx: number, dy: number): void
  reset(): void
  /** A tap at `x`, `y` CSS pixels from the view's top left. */
  tap?(x: number, y: number): void
}

interface SafariGestureEvent extends UIEvent {
  scale: number
}

/** A key's step, CSS pixels of drag. */
const KEY_STEP = 24
/** A press moving less than this, CSS pixels, is a tap. */
const TAP_SLOP = 6

export function attachOrbitInput(el: HTMLElement, view: Orbiting): () => void {
  let last: ToolPointer | null = null
  // Where a press began, and whether it has moved off it: a tap if not.
  let pressed: { x: number; y: number; moved: boolean } | null = null
  const router = new InputRouter(
    {
      pan: (dx, dy) => view.slide(dx, dy),
      zoom: (_x, _y, factor) => view.zoom(factor),
      undo: () => {},
      redo: () => {},
      toolDown: (p) => {
        last = p
        pressed = { x: p.x, y: p.y, moved: false }
      },
      toolMove: (p) => {
        if (last) view.turn(p.x - last.x, p.y - last.y)
        last = p
        if (pressed && Math.hypot(p.x - pressed.x, p.y - pressed.y) > TAP_SLOP) pressed.moved = true
      },
      toolUp: () => {
        if (pressed && !pressed.moved) view.tap?.(pressed.x, pressed.y)
        last = null
        pressed = null
      },
      toolCancel: () => {
        last = null
        pressed = null
      },
      hover: () => {},
      contextMenu: () => {},
      hold: () => {},
    },
    {
      after(ms, fn) {
        const id = setTimeout(fn, ms)
        return () => clearTimeout(id)
      },
    },
    // Here one finger always turns, Pencil or not.
    'always',
  )
  const raw = (phase: RawPointer['phase'], e: PointerEvent) =>
    rawPointer(phase, e, el.getBoundingClientRect())

  let touches = 0
  const onPointerDown = (e: PointerEvent) => {
    if (e.pointerType === 'touch') touches++
    try {
      el.setPointerCapture(e.pointerId)
    } catch {
      // Not an active pointer (synthetic events): capture is only a nicety.
    }
    router.handle(raw('down', e))
  }
  const onPointerMove = (e: PointerEvent) => {
    if (e.buttons === 0 && e.pointerType !== 'touch') return
    router.handle(raw('move', e))
  }
  const onPointerEnd = (phase: 'up' | 'cancel') => (e: PointerEvent) => {
    if (e.pointerType === 'touch') touches = Math.max(0, touches - 1)
    router.handle(raw(phase, e))
  }
  const onPointerUp = onPointerEnd('up')
  const onPointerCancel = onPointerEnd('cancel')
  const onWheel = (e: WheelEvent) => {
    e.preventDefault()
    view.zoom(Math.exp(-e.deltaY * (e.deltaMode === 1 ? 0.05 : 0.002)))
  }
  // Mac trackpad pinches in Safari; on iPad the router has the touches.
  let gestureScale = 1
  const onGestureStart = (e: Event) => {
    e.preventDefault()
    gestureScale = 1
  }
  const onGestureChange = (e: Event) => {
    e.preventDefault()
    if (touches > 0) return
    const g = e as SafariGestureEvent
    view.zoom(g.scale / gestureScale)
    gestureScale = g.scale
  }
  const onDoubleClick = () => view.reset()
  const onKey = (e: KeyboardEvent) => {
    if (isTyping(e.target)) return
    const turns: Record<string, [number, number]> = {
      ArrowLeft: [-KEY_STEP, 0],
      ArrowRight: [KEY_STEP, 0],
      ArrowUp: [0, -KEY_STEP],
      ArrowDown: [0, KEY_STEP],
    }
    const turn = turns[e.key]
    if (turn) view.turn(...turn)
    else if (e.key === '+' || e.key === '=') view.zoom(1.15)
    else if (e.key === '-' || e.key === '_') view.zoom(1 / 1.15)
    else if (e.key === '0') view.reset()
    else return
    e.preventDefault()
  }
  const onBlur = () => router.reset()
  const onContextMenu = (e: Event) => e.preventDefault()

  el.addEventListener('pointerdown', onPointerDown)
  el.addEventListener('pointermove', onPointerMove)
  el.addEventListener('pointerup', onPointerUp)
  el.addEventListener('pointercancel', onPointerCancel)
  el.addEventListener('wheel', onWheel, { passive: false })
  el.addEventListener('gesturestart', onGestureStart)
  el.addEventListener('gesturechange', onGestureChange)
  el.addEventListener('dblclick', onDoubleClick)
  el.addEventListener('keydown', onKey)
  el.addEventListener('contextmenu', onContextMenu)
  window.addEventListener('blur', onBlur)
  return () => {
    el.removeEventListener('pointerdown', onPointerDown)
    el.removeEventListener('pointermove', onPointerMove)
    el.removeEventListener('pointerup', onPointerUp)
    el.removeEventListener('pointercancel', onPointerCancel)
    el.removeEventListener('wheel', onWheel)
    el.removeEventListener('gesturestart', onGestureStart)
    el.removeEventListener('gesturechange', onGestureChange)
    el.removeEventListener('dblclick', onDoubleClick)
    el.removeEventListener('keydown', onKey)
    el.removeEventListener('contextmenu', onContextMenu)
    window.removeEventListener('blur', onBlur)
  }
}
