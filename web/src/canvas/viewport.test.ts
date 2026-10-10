import { describe, expect, it } from 'vitest'
import {
  type ViewportState,
  NOMINAL_PX_PER_MM,
  fit,
  panBy,
  scaleOf,
  toScreen,
  toWorld,
  zoomAt,
} from './viewport.ts'

const v: ViewportState = {
  width: 800,
  height: 600,
  cx: 100,
  cy: 50,
  zoom: 1,
  pxPerMm: NOMINAL_PX_PER_MM,
}

describe('viewport', () => {
  it('maps world to screen and back', () => {
    const p = { x: 123.4, y: -56.7 }
    const back = toWorld(v, toScreen(v, p))
    expect(back.x).toBeCloseTo(p.x, 9)
    expect(back.y).toBeCloseTo(p.y, 9)
    expect(toScreen(v, { x: 100, y: 50 })).toEqual({ x: 400, y: 300 })
  })

  it('at zoom 1, 25.4 mm spans 96 CSS px before calibration', () => {
    const a = toScreen(v, { x: 0, y: 0 })
    const b = toScreen(v, { x: 25.4, y: 0 })
    expect(b.x - a.x).toBeCloseTo(96, 9)
  })

  it('zooming keeps the point under the pointer still', () => {
    const pointer = { x: 620, y: 140 }
    const before = toWorld(v, pointer)
    const z = zoomAt(v, pointer, 2.5)
    const after = toWorld(z, pointer)
    expect(z.zoom).toBe(2.5)
    expect(after.x).toBeCloseTo(before.x, 9)
    expect(after.y).toBeCloseTo(before.y, 9)
  })

  it('panning moves content with the drag', () => {
    const p = { x: 10, y: 20 }
    const before = toScreen(v, p)
    const after = toScreen(panBy(v, 30, -12), p)
    expect(after.x - before.x).toBeCloseTo(30, 9)
    expect(after.y - before.y).toBeCloseTo(-12, 9)
  })

  it('fits a rectangle inside the padding', () => {
    const f = fit(v, { x0: 0, y0: 0, x1: 1000, y1: 500 }, 50)
    const s = scaleOf(f)
    expect(1000 * s).toBeCloseTo(700, 6)
    expect(f.cx).toBe(500)
    expect(f.cy).toBe(250)
  })
})
