import type { Command } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import {
  type Straightenable,
  askew,
  shownTurn,
  straighten,
  straightenedPieces,
  uprightTurn,
} from './straighten.ts'

/** A grain `tilt` degrees clockwise of straight down the piece, as drawn. */
const grain = (tilt: number) => {
  const a = ((90 + tilt) * Math.PI) / 180
  return {
    from: [50, 50] as [number, number],
    to: [50 + 300 * Math.cos(a), 50 + 300 * Math.sin(a)] as [number, number],
    oneWay: false,
  }
}

/** A 100 × 400 mm piece placed at `x, y`, turned, its grain tilted as drawn. */
const piece = (
  id: string,
  tilt: number | null,
  { x = 0, y = 0, rotation = 0, size = 0 } = {},
): Straightenable => ({
  piece: {
    id,
    name: id,
    placement: { x, y, rotation },
    grainline: tilt === null ? null : grain(tilt),
  },
  // A size `size` mm larger all round, as nested sizes are.
  view: { cutting: [-size, -size, 100 + size, -size, 100 + size, 400 + size, -size, 400 + size] },
})

const turns = (commands: Command[]) =>
  commands.map((c) =>
    c.type === 'rotatePiece' ? { piece: c.piece, degrees: c.degrees, about: c.about } : null,
  )

describe('uprightTurn', () => {
  it('turns a grain drawn a few degrees off back upright, the nearer way', () => {
    expect(uprightTurn(piece('a', 3).piece)).toBeCloseTo(-3)
    expect(uprightTurn(piece('a', -4.5).piece)).toBeCloseTo(4.5)
    // Drawn upright but turned on the table: back by that turn.
    expect(uprightTurn(piece('a', 0, { rotation: 7 }).piece)).toBeCloseTo(-7)
    // Nearly upside down is nearer upright the other way up.
    expect(uprightTurn(piece('a', 0, { rotation: 178 }).piece)).toBeCloseTo(2)
    // A quarter turned, either way up.
    expect(Math.abs(uprightTurn(piece('a', 0, { rotation: 90 }).piece)!)).toBeCloseTo(90)
    expect(uprightTurn(piece('a', 0).piece)).toBe(0)
    expect(uprightTurn(piece('a', 0, { rotation: 180 }).piece)).toBe(0)
    expect(uprightTurn(piece('a', null).piece)).toBeNull()
  })

  it('tells a piece drawn askew from one on the bias', () => {
    expect(askew(piece('a', 3).piece)).toBe(true)
    expect(askew(piece('a', 0).piece)).toBe(false)
    expect(askew(piece('a', 45).piece)).toBe(false)
    expect(askew(piece('a', null).piece)).toBe(false)
  })
})

describe('straighten', () => {
  it('turns each piece apart about its own middle, and says which have no grain', () => {
    const result = straighten([
      piece('a', 3),
      piece('b', -2, { x: 300 }),
      piece('c', null, { x: 600 }),
      piece('d', 0, { x: 900 }),
    ])
    expect(result.noGrain).toEqual(['c'])
    expect(result.straight).toBe(1)
    const [a, b] = turns(result.commands)
    expect(result.commands).toHaveLength(2)
    expect(a?.piece).toBe('a')
    expect(a?.degrees).toBeCloseTo(-3)
    expect(a?.about).toBeUndefined()
    expect(b?.degrees).toBeCloseTo(2)
  })

  it('turns sizes laid over each other together, about their shared middle', () => {
    const result = straighten([
      piece('small', 3),
      piece('medium', 3, { size: 5 }),
      piece('large', 3.2, { size: 10 }),
      piece('apart', 3, { x: 500 }),
    ])
    const [small, medium, large, apart] = turns(result.commands)
    expect(small?.about).toEqual([50, 200])
    expect(medium?.about).toEqual([50, 200])
    expect(large?.about).toEqual([50, 200])
    expect(large?.degrees).toBeCloseTo(-3.2)
    expect(apart?.about).toBeUndefined()
  })

  it('keeps a stack the same way up where one size would turn the other way', () => {
    const result = straighten([
      piece('a', 0, { rotation: 89 }),
      piece('b', 0, { rotation: 91, size: 5 }),
    ])
    const [a, b] = turns(result.commands)
    expect(a?.degrees).toBeCloseTo(-89)
    expect(b?.degrees).toBeCloseTo(-91)
  })
})

describe('straightenedPieces', () => {
  it('places pieces drawn askew upright, leaving bias and straight pieces be', () => {
    const corners = [0, 0, 100, 0, 100, 400, 0, 400]
    const pieces = [piece('a', 3), piece('bias', 45, { x: 300 }), piece('b', 0, { x: 600 })].map(
      (p) => p.piece,
    )
    const [a, bias, b] = straightenedPieces(pieces, () => corners)
    expect(uprightTurn(a!)).toBe(0)
    expect(shownTurn(a!.placement.rotation)).toBe(-3)
    // Turned about its middle: the middle stays where it was.
    const r = (a!.placement.rotation * Math.PI) / 180
    const mx = a!.placement.x + 50 * Math.cos(r) - 200 * Math.sin(r)
    const my = a!.placement.y + 50 * Math.sin(r) + 200 * Math.cos(r)
    expect(mx).toBeCloseTo(50)
    expect(my).toBeCloseTo(200)
    expect(bias).toBe(pieces[1])
    expect(b).toBe(pieces[2])
  })
})

describe('shownTurn', () => {
  it('shows turns either way from as drawn, to a tenth of a degree', () => {
    expect(shownTurn(357)).toBe(-3)
    expect(shownTurn(359.99)).toBe(0)
    expect(shownTurn(723.44)).toBe(3.4)
    expect(shownTurn(180)).toBe(180)
    expect(shownTurn(270)).toBe(-90)
  })
})
