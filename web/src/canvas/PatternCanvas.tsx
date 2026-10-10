import { useEffect, useRef } from 'react'
import type { Session } from '../state/session.ts'
import { draw, readTheme } from './renderer.ts'
import { type Point, panBy, scaleOf, toWorld, zoomAt } from './viewport.ts'

/** Pointer travel (CSS px) before a press becomes a drag. */
const DRAG_SLOP = 4
/** Hit radius on screen, CSS px. */
const HIT_RADIUS = 8

interface SafariGestureEvent extends UIEvent {
  scale: number
  clientX: number
  clientY: number
}

/**
 * The work table. Draws on demand (once per animation frame at most) when
 * the project or the view changes.
 */
export function PatternCanvas({ session }: { session: Session }) {
  const canvasRef = useRef<HTMLCanvasElement>(null)

  useEffect(() => {
    const canvas = canvasRef.current
    const ctx = canvas?.getContext('2d')
    if (!canvas || !ctx) return
    let theme = readTheme(canvas)
    let frame = 0

    const render = () => {
      frame = 0
      const { viewport, selection } = session.ui.get()
      const dpr = window.devicePixelRatio || 1
      const w = Math.max(1, Math.round(viewport.width * dpr))
      const h = Math.max(1, Math.round(viewport.height * dpr))
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w
        canvas.height = h
      }
      draw(ctx, viewport, dpr, { pieces: session.scene(), selection }, theme)
    }
    const invalidate = () => {
      frame ||= requestAnimationFrame(render)
    }
    const unsubscribeUi = session.ui.subscribe(invalidate)
    const unsubscribeEngine = session.engine.subscribe(invalidate)

    const resize = new ResizeObserver(([entry]) => {
      if (!entry) return
      const { width, height } = entry.contentRect
      session.setViewport((v) => ({ ...v, width, height }))
    })
    resize.observe(canvas)

    const scheme = matchMedia('(prefers-color-scheme: dark)')
    const onScheme = () => {
      theme = readTheme(canvas)
      invalidate()
    }
    scheme.addEventListener('change', onScheme)

    const local = (e: { clientX: number; clientY: number }): Point => {
      const r = canvas.getBoundingClientRect()
      return { x: e.clientX - r.left, y: e.clientY - r.top }
    }

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

    // Safari reports trackpad pinches as gesture events.
    let gestureScale = 1
    const onGestureStart = (e: Event) => {
      e.preventDefault()
      gestureScale = 1
    }
    const onGestureChange = (e: Event) => {
      e.preventDefault()
      const g = e as SafariGestureEvent
      const factor = g.scale / gestureScale
      gestureScale = g.scale
      session.setViewport((v) => zoomAt(v, local(g), factor))
    }

    // Press and drag pans; a press without a drag selects.
    let press: { id: number; start: Point; last: Point; dragging: boolean } | null = null
    const onPointerDown = (e: PointerEvent) => {
      if (e.button !== 0 && e.pointerType === 'mouse') return
      canvas.setPointerCapture(e.pointerId)
      const p = local(e)
      press = { id: e.pointerId, start: p, last: p, dragging: false }
    }
    const onPointerMove = (e: PointerEvent) => {
      if (!press || e.pointerId !== press.id) return
      const p = local(e)
      if (!press.dragging && Math.hypot(p.x - press.start.x, p.y - press.start.y) > DRAG_SLOP)
        press.dragging = true
      if (press.dragging) {
        const { x, y } = press.last
        session.setViewport((v) => panBy(v, p.x - x, p.y - y))
      }
      press.last = p
    }
    const onPointerUp = (e: PointerEvent) => {
      if (!press || e.pointerId !== press.id) return
      if (!press.dragging) {
        const v = session.ui.get().viewport
        const w = toWorld(v, press.start)
        const hit = session.engine.hitTest(w.x, w.y, HIT_RADIUS / scaleOf(v))
        session.select(hit ? [hit.piece] : [])
      }
      press = null
    }
    const onPointerCancel = () => {
      press = null
    }

    canvas.addEventListener('wheel', onWheel, { passive: false })
    canvas.addEventListener('gesturestart', onGestureStart)
    canvas.addEventListener('gesturechange', onGestureChange)
    canvas.addEventListener('pointerdown', onPointerDown)
    canvas.addEventListener('pointermove', onPointerMove)
    canvas.addEventListener('pointerup', onPointerUp)
    canvas.addEventListener('pointercancel', onPointerCancel)
    invalidate()

    return () => {
      cancelAnimationFrame(frame)
      unsubscribeUi()
      unsubscribeEngine()
      resize.disconnect()
      scheme.removeEventListener('change', onScheme)
      canvas.removeEventListener('wheel', onWheel)
      canvas.removeEventListener('gesturestart', onGestureStart)
      canvas.removeEventListener('gesturechange', onGestureChange)
      canvas.removeEventListener('pointerdown', onPointerDown)
      canvas.removeEventListener('pointermove', onPointerMove)
      canvas.removeEventListener('pointerup', onPointerUp)
      canvas.removeEventListener('pointercancel', onPointerCancel)
    }
  }, [session])

  return (
    <canvas
      ref={canvasRef}
      className="pattern-canvas"
      role="img"
      aria-label="Pattern pieces on the cutting table"
    />
  )
}
