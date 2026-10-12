import { describe, expect, it } from 'vitest'
import { invert, mapPoint, solveHomography, type Pt } from './homography.ts'

const square: [Pt, Pt, Pt, Pt] = [
  [0, 0],
  [500, 0],
  [500, 300],
  [0, 300],
]

describe('homography', () => {
  it('maps each corner where it was asked to', () => {
    // A projector tilted: the far side of the mat narrower.
    const corners: [Pt, Pt, Pt, Pt] = [
      [120, 80],
      [1180, 60],
      [1100, 700],
      [180, 690],
    ]
    const m = solveHomography(square, corners)!
    for (let i = 0; i < 4; i++) {
      const [x, y] = mapPoint(m, square[i]!)
      expect(x).toBeCloseTo(corners[i]![0], 6)
      expect(y).toBeCloseTo(corners[i]![1], 6)
    }
    // And back again.
    const back = invert(m)!
    const [x, y] = mapPoint(back, mapPoint(m, [250, 150]))
    expect(x).toBeCloseTo(250, 6)
    expect(y).toBeCloseTo(150, 6)
  })

  it('is plain scaling for a square-on projector', () => {
    const m = solveHomography(
      square,
      square.map(([x, y]) => [x * 2 + 10, y * 2 + 20] as const) as unknown as [Pt, Pt, Pt, Pt],
    )!
    expect(mapPoint(m, [100, 100])[0]).toBeCloseTo(210, 9)
    expect(mapPoint(m, [100, 100])[1]).toBeCloseTo(220, 9)
    expect(m[6]).toBeCloseTo(0, 12)
    expect(m[7]).toBeCloseTo(0, 12)
  })

  it('cannot be found from points on a line', () => {
    const line: [Pt, Pt, Pt, Pt] = [
      [0, 0],
      [1, 1],
      [2, 2],
      [3, 3],
    ]
    expect(solveHomography(square, line)).toBeNull()
  })
})
