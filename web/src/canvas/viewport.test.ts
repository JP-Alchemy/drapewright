import { describe, expect, it } from 'vitest'
import {
  type ViewportState,
  NOMINAL_PX_PER_MM,
  fit,
  panBy,
  reveal,
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

  it('shows a rectangle already on screen as it is', () => {
    expect(reveal(v, { x0: 50, y0: 0, x1: 150, y1: 100 }, 50)).toBe(v)
  })

  it('moves the least to show a rectangle off screen, at the same zoom', () => {
    // Off to the right and a little low: it comes in just inside the padding.
    const r = reveal(v, { x0: 400, y0: 100, x1: 450, y1: 160 }, 50)
    expect(r.zoom).toBe(1)
    expect(toScreen(r, { x: 450, y: 160 }).x).toBeCloseTo(750, 6)
    expect(toScreen(r, { x: 450, y: 160 }).y).toBeCloseTo(550, 6)
    // The other way, up and to the left.
    const l = reveal(v, { x0: -300, y0: -200, x1: -250, y1: -150 }, 50)
    expect(toScreen(l, { x: -300, y: -200 }).x).toBeCloseTo(50, 6)
    expect(toScreen(l, { x: -300, y: -200 }).y).toBeCloseTo(50, 6)
  })

  it('zooms out only as far as a rectangle needs to fit, never in', () => {
    const wide = reveal(v, { x0: 0, y0: 0, x1: 2000, y1: 10 }, 50)
    expect(wide.zoom).toBeLessThan(1)
    expect(toScreen(wide, { x: 0, y: 0 }).x).toBeCloseTo(50, 6)
    expect(toScreen(wide, { x: 2000, y: 0 }).x).toBeCloseTo(750, 6)
    // A speck far away is shown at the zoom it was.
    expect(reveal(v, { x0: 5000, y0: 5000, x1: 5001, y1: 5001 }, 50).zoom).toBe(1)
  })
})
