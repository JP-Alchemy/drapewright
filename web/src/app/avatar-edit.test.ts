import type { SeenEdge } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import { copyOf, edgeNear, mmDragged, seenHandles, shownRuns } from './avatar-edit.ts'

/** A straight edge seen from (x0, y) to (x1, y), its points 10 px apart, `hidden` ones turned away. */
function seenAcross(
  piece: string,
  edge: string,
  [x0, x1]: [number, number],
  y: number,
  hidden: number[] = [],
): SeenEdge {
  const n = Math.round(Math.abs(x1 - x0) / 10) + 1
  const points = Array.from({ length: n }, (_, i) => [x0 + ((x1 - x0) * i) / (n - 1), y]).flat()
  return {
    piece,
    edge,
    kind: 'hem',
    points,
    shown: Array.from({ length: n }, (_, i) => !hidden.includes(i)),
    middle: Math.floor(n / 2),
    out: [0, 0.5],
    length: 300,
  }
}

describe('editing on the figure', () => {
  const front = seenAcross('p1', 'e4', [100, 300], 400)
  const back = seenAcross('p2', 'e9', [100, 300], 200, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10])

  it('draws only what faces the eye, in runs', () => {
    expect(shownRuns(front)).toHaveLength(1)
    const broken = seenAcross('p1', 'e4', [100, 300], 400, [10])
    expect(shownRuns(broken).map((r) => r.length)).toEqual([10, 10])
    expect(shownRuns(back).map((r) => r.length)).toEqual([10])
  })

  it('finds the edge under the pointer, not one turned away', () => {
    expect(edgeNear([front, back], { x: 200, y: 405 })?.edge).toBe('e4')
    expect(edgeNear([front, back], { x: 150, y: 202 })).toBeNull()
    expect(edgeNear([front, back], { x: 200, y: 300 })).toBeNull()
  })

  it('puts the handles on the copy nearest the pointer, or the one seen longest', () => {
    const left = seenAcross('p1', 'e4', [100, 200], 400)
    const right = seenAcross('p1', 'e4', [300, 500], 400)
    expect(copyOf([left, right], 'p1', 'e4', { x: 120, y: 400 })).toBe(left)
    expect(copyOf([left, right], 'p1', 'e4')).toBe(right)
    expect(copyOf([back], 'p2', 'e9')).toBeNull()
  })

  it('lays the length and handles out beyond the edge, out of the piece', () => {
    const h = seenHandles(front, '30.0 cm', { width: 800, height: 600 })
    expect(h.middle).toEqual({ x: 200, y: 400 })
    expect(h.pill.y).toBeGreaterThan(400)
    expect(h.extend.x).toBeLessThan(h.pill.x)
    expect(h.extrude.x).toBeGreaterThan(h.pill.x)
  })

  it('turns a drag out of the piece into millimetres', () => {
    // Half a pixel a mm, out down the view.
    expect(mmDragged(front, 0, 10)).toBeCloseTo(20)
    expect(mmDragged(front, 7, -5)).toBeCloseTo(-10)
    // Seen end on, a drag goes no faster than at 0.2 px a mm.
    const endOn = { ...front, out: [0, 0.01] as [number, number] }
    expect(mmDragged(endOn, 0, 10)).toBeCloseTo(50)
  })
})
