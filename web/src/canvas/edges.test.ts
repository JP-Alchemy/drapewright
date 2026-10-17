import { describe, expect, it } from 'vitest'
import type { Piece } from '@drapewright/engine'
import { cornerOf, edgeEnds, joinCurve, stretch } from './edges.ts'

describe('stretch', () => {
  // The top of a square drawn clockwise on screen, left to right.
  const top: [number, number][] = [
    [0, 0],
    [50, 0],
    [100, 0],
  ]

  it('takes part of a run of points, and finds its middle', () => {
    const s = stretch(top, 0.2, 0.6)
    expect(s.points[0]).toEqual([20, 0])
    expect(s.points.at(-1)).toEqual([60, 0])
    expect(s.points).toContainEqual([50, 0])
    expect(s.middle).toEqual([40, 0])
  })

  it('points out of a piece drawn clockwise', () => {
    // Up the screen, away from the square below.
    expect(stretch(top, 0, 1).out).toEqual([0, -1])
  })

  it('takes the whole run, either way round', () => {
    const s = stretch(top, 1, 0)
    expect(s.points[0]).toEqual([0, 0])
    expect(s.points.at(-1)).toEqual([100, 0])
  })
})

describe('joinCurve', () => {
  it('leaves each edge outwards, further for edges further apart', () => {
    // A front's right side facing a back's left side, 300 mm apart.
    const c = joinCurve([0, 0], [1, 0], [300, 0], [-1, 0])!
    expect(c[0]).toEqual([0, 0])
    expect(c[1]).toEqual([105, 0])
    expect(c[2]).toEqual([195, 0])
    expect(c[3]).toEqual([300, 0])
  })

  it('reaches out at least 20 mm, and at most 150 mm', () => {
    expect(joinCurve([0, 0], [0, -1], [10, 0], [0, -1])![1]).toEqual([0, -20])
    expect(joinCurve([0, 0], [0, 1], [2000, 0], [0, 1])![2]).toEqual([2000, 150])
  })

  it('joins nothing where the middles meet', () => {
    expect(joinCurve([5, 5], [1, 0], [5.5, 5], [-1, 0])).toBeNull()
  })
})

describe('corners', () => {
  // A square 100 mm a side, drawn clockwise on screen from its top left.
  const square = {
    outline: {
      start: [0, 0],
      edges: [
        { id: 'top', segments: [{ to: [100, 0] }] },
        { id: 'right', segments: [{ to: [100, 100] }] },
        { id: 'bottom', segments: [{ to: [0, 100] }] },
        { id: 'left', segments: [{ to: [0, 0] }] },
      ],
    },
  } as unknown as Piece

  it('knows which way each edge leaves its corners', () => {
    expect(edgeEnds(square, 'top')).toEqual({ start: [1, 0], end: [-1, 0] })
  })

  it('points out of the piece at a corner, between its edges', () => {
    const c = cornerOf(square, 'top')!
    expect(c.at).toEqual([100, 0])
    expect(c.out[0]).toBeCloseTo(Math.SQRT1_2)
    expect(c.out[1]).toBeCloseTo(-Math.SQRT1_2)
  })
})
