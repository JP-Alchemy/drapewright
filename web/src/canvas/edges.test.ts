import { describe, expect, it } from 'vitest'
import { stretch } from './edges.ts'

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
