import { useEffect, useRef } from 'react'
import type { Session } from '../state/session.ts'
import { grainlineTool } from '../tools/grainline.ts'
import { notchTool } from '../tools/notch.ts'
import { selectTool } from '../tools/select.ts'
import { attachInput } from './input.ts'
import { draw, readTheme } from './renderer.ts'

/**
 * The work table. Draws on demand (once per animation frame at most) when
 * the project or the view changes; input goes through @drapewright/input.
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
      const ui = session.ui.get()
      const { viewport } = ui
      const dpr = window.devicePixelRatio || 1
      const w = Math.max(1, Math.round(viewport.width * dpr))
      const h = Math.max(1, Math.round(viewport.height * dpr))
      if (canvas.width !== w || canvas.height !== h) {
        canvas.width = w
        canvas.height = h
      }
      draw(
        ctx,
        viewport,
        dpr,
        {
          pieces: session.scene(),
          selection: ui.selection,
          part: ui.part,
          units: session.engine.document().settings.displayUnits,
          hover: ui.hover,
          preview: ui.preview,
          grainPreview: ui.grainPreview,
        },
        theme,
      )
    }
    const invalidate = () => {
      frame ||= requestAnimationFrame(render)
    }
    const unsubscribeUi = session.ui.subscribe(invalidate)
    const unsubscribeEngine = session.engine.subscribe(invalidate)

    // Measure now as well as on resize: a page that starts hidden gets no
    // resize callbacks until it is shown.
    const measured = canvas.getBoundingClientRect()
    session.setViewport((v) => ({ ...v, width: measured.width, height: measured.height }))
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

    const input = attachInput(canvas, session, {
      select: selectTool(),
      notch: notchTool(),
      grainline: grainlineTool(),
    })
    invalidate()

    return () => {
      cancelAnimationFrame(frame)
      unsubscribeUi()
      unsubscribeEngine()
      resize.disconnect()
      scheme.removeEventListener('change', onScheme)
      input.detach()
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
