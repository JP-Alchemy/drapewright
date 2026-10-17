import type { SeenEdge } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import {
  copyOf,
  cornerDragged,
  cornerNear,
  cornerShown,
  edgeNear,
  mmDragged,
  seenCornerHandle,
  seenCorners,
  seenHandles,
  shownRuns,
  underPointer,
} from './avatar-edit.ts'

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
    ends: [
      [1, 0],
      [-1, 0],
    ],
    facing: [1, 1],
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

  // A piece's bottom right corner seen straight on: its right side comes
  // down to (300, 400), its hem goes left from there. Half a px a mm.
  const side: SeenEdge = {
    ...seenAcross('p1', 'right', [300, 300], 300),
    points: [300, 300, 300, 350, 300, 400],
    shown: [true, true, true],
    middle: 1,
    ends: [
      [0, 0.5],
      [0, -0.5],
    ],
  }
  const hem: SeenEdge = {
    ...seenAcross('p1', 'hem', [300, 100], 400),
    ends: [
      [-0.5, 0],
      [0.5, 0],
    ],
  }
  const next = (_piece: string, edge: string) => (edge === 'right' ? 'hem' : null)

  it('finds corners where one edge ends and the next begins', () => {
    const corners = seenCorners([side, hem], next)
    expect(corners).toHaveLength(1)
    expect(corners[0]).toMatchObject({ piece: 'p1', edge: 'right', at: { x: 300, y: 400 } })
    expect(cornerNear(corners, { x: 305, y: 404 })?.edge).toBe('right')
    expect(cornerNear(corners, { x: 320, y: 400 })).toBeNull()
  })

  it('puts a corner’s handle out of the piece, away from both its edges', () => {
    const [corner] = seenCorners([side, hem], next)
    const h = seenCornerHandle(corner!, { width: 800, height: 600 })
    expect(h.at.x).toBeGreaterThan(300)
    expect(h.at.y).toBeGreaterThan(400)
  })

  it('turns a drag of a corner into a move across and down its piece, and back', () => {
    const [corner] = seenCorners([side, hem], next)
    // On the piece: back up its right side, and left along its hem.
    const ways: [{ x: number; y: number }, { x: number; y: number }] = [
      { x: 0, y: -1 },
      { x: -1, y: 0 },
    ]
    const moved = cornerDragged(corner!, ways, { x: 10, y: 5 })!
    expect(moved.x).toBeCloseTo(20)
    expect(moved.y).toBeCloseTo(10)
    const shown = cornerShown(corner!, ways, moved)!
    expect(shown.x).toBeCloseTo(10)
    expect(shown.y).toBeCloseTo(5)
  })

  it('moves a corner little along a way seen end on, rather than flinging it', () => {
    // Its hem runs almost straight at the eye: a tenth of a px a mm.
    const [corner] = seenCorners(
      [
        side,
        {
          ...hem,
          ends: [
            [-0.05, 0],
            [0.05, 0],
          ],
        },
      ],
      next,
    )
    const ways: [{ x: number; y: number }, { x: number; y: number }] = [
      { x: 0, y: -1 },
      { x: -1, y: 0 },
    ]
    // Dragged across: as if seen at 0.3 of the side's half a px a mm.
    const moved = cornerDragged(corner!, ways, { x: 3, y: 0 })!
    expect(moved.x).toBeCloseTo(20)
    // Dragged down the side seam: exactly.
    expect(cornerDragged(corner!, ways, { x: 0, y: 5 })!.y).toBeCloseTo(10)
  })

  it('takes a corner under the pointer only if it is as near as the edge', () => {
    const corners = seenCorners([side, hem], next)
    // On the corner: the corner.
    expect(underPointer([side, hem], corners, { x: 301, y: 399 })).toMatchObject({
      corner: { edge: 'right' },
    })
    // On the hem, 7 px from the corner: the hem.
    expect(underPointer([side, hem], corners, { x: 293, y: 400 })).toMatchObject({
      edge: { edge: 'hem' },
    })
  })
})
