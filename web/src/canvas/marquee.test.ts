import { describe, expect, it } from 'vitest'
import { type Boxable, piecesTouching } from './marquee.ts'

/** A piece with outline `points` (its own mm), placed at `x, y` and turned. */
const piece = (id: string, points: number[], x = 0, y = 0, rotation = 0): Boxable => ({
  piece: { id, placement: { x, y, rotation } },
  view: { cutting: points },
})

/** A 100 × 50 mm rectangle. */
const RECT = [0, 0, 100, 0, 100, 50, 0, 50]
/** A right triangle, its long side from (100, 0) to (0, 100). */
const TRIANGLE = [0, 0, 100, 0, 0, 100]

const box = (x0: number, y0: number, x1: number, y1: number) =>
  [
    { x: x0, y: y0 },
    { x: x1, y: y1 },
  ] as const

describe('piecesTouching', () => {
  it('chooses the pieces the box takes in, and those it only touches', () => {
    const pieces = [piece('a', RECT), piece('b', RECT, 200), piece('c', RECT, 400)]
    // Over all of a and part of b.
    expect(piecesTouching(pieces, ...box(-10, -10, 250, 20))).toEqual(['a', 'b'])
    // Dragged the other way: the same.
    expect(piecesTouching(pieces, ...box(250, 20, -10, -10))).toEqual(['a', 'b'])
    // Between pieces: none.
    expect(piecesTouching(pieces, ...box(120, -10, 180, 80))).toEqual([])
  })

  it('goes by the outline, not the piece’s bounds', () => {
    const pieces = [piece('t', TRIANGLE)]
    // In the bounds, past the long side: not touching.
    expect(piecesTouching(pieces, ...box(80, 80, 120, 120))).toEqual([])
    // Across the long side, with no corner of the triangle in the box.
    expect(piecesTouching(pieces, ...box(40, 40, 70, 70))).toEqual(['t'])
  })

  it('takes a piece where it lies: moved and turned', () => {
    // Turned a quarter clockwise about its own origin, at (500, 0): it lies
    // from x 450 to 500, y 0 to 100.
    const pieces = [piece('r', RECT, 500, 0, 90)]
    expect(piecesTouching(pieces, ...box(440, 90, 455, 95))).toEqual(['r'])
    expect(piecesTouching(pieces, ...box(510, 0, 520, 10))).toEqual([])
    expect(piecesTouching(pieces, ...box(0, 0, 100, 50))).toEqual([])
  })

  it('chooses a piece the box is wholly inside', () => {
    expect(piecesTouching([piece('a', RECT)], ...box(10, 10, 20, 20))).toEqual(['a'])
  })
})
