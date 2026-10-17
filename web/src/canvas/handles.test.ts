import type { EdgeView } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import { edgeHandles, handleAt, handleGlyph, towardHandles } from './handles.ts'
import type { ViewportState } from './viewport.ts'

/** 1 px a mm, the table's origin at the view's top left, 800 by 600 px. */
const view: ViewportState = { width: 800, height: 600, cx: 400, cy: 300, zoom: 1, pxPerMm: 1 }
const unturned = { transformPoint: (p: { x: number; y: number }) => p }

const edge = (middle: [number, number], normal: [number, number]): EdgeView => ({
  id: 'e',
  name: '',
  kind: 'seam',
  length: 200,
  allowance: 10,
  start: [0, 0],
  end: [0, 0],
  middle,
  middleNormal: normal,
})

describe('edge handles', () => {
  it('sit either side of a hem’s length, below it: extend on the left', () => {
    const h = edgeHandles('p', edge([300, 400], [0, 1]), unturned, view, '20.0 cm')
    // The label 10 mm of allowance and 14 px beyond the hem.
    expect(h.pill).toEqual({ x: 300, y: 424 })
    expect(h.out.y).toBeCloseTo(1)
    expect(h.extend.y).toBeCloseTo(424)
    expect(h.extrude.y).toBeCloseTo(424)
    expect(h.extend.x).toBeLessThan(300 - h.width / 2)
    expect(h.extrude.x).toBeGreaterThan(300 + h.width / 2)
  })

  it('sit above and below the length of an upright edge: extend above', () => {
    const h = edgeHandles('p', edge([300, 300], [-1, 0]), unturned, view, '20.0 cm')
    expect(h.pill).toEqual({ x: 276, y: 300 })
    expect(h.extend.x).toBeCloseTo(276)
    expect(h.extend.y).toBeLessThan(300 - 11)
    expect(h.extrude.y).toBeGreaterThan(300 + 11)
  })

  it('stay on the view for an edge at its side', () => {
    const h = edgeHandles('p', edge([300, 595], [0, 1]), unturned, view, '20.0 cm')
    expect(h.pill.y).toBeLessThanOrEqual(600 - 13)
    expect(h.extend.y + 13).toBeLessThanOrEqual(600)
  })

  it('are found under the pointer, and the way out to them keeps the edge', () => {
    const h = edgeHandles('p', edge([300, 400], [0, 1]), unturned, view, '20.0 cm')
    expect(handleAt(h, h.extend)).toBe('extend')
    expect(handleAt(h, { x: h.extrude.x + 5, y: h.extrude.y - 5 })).toBe('extrude')
    expect(handleAt(h, h.pill)).toBeNull()
    expect(towardHandles(h, { x: 300, y: 410 })).toBe(true)
    expect(towardHandles(h, { x: h.extend.x, y: 415 })).toBe(true)
    expect(towardHandles(h, { x: 300, y: 300 })).toBe(false)
    expect(towardHandles(h, { x: 100, y: 410 })).toBe(false)
  })

  it('are drawn as an arrow out of the edge, its sides flaring or square', () => {
    const sides = (kind: 'extend' | 'extrude') => handleGlyph(kind).slice(-2)
    const [left] = sides('extend')
    expect(left![1]![0]).toBeLessThan(left![0]![0])
    const [square] = sides('extrude')
    expect(square![1]![0]).toBe(square![0]![0])
  })
})
