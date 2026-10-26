import type { Piece, SizeMeasures } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import { cutFor, sizesOnTable, suggest } from './sizes.ts'

/** A size measured round the figure: `round` the garment at each level, mm. */
const size = (name: string, round: Record<string, number | null>): SizeMeasures => ({
  size: name,
  girths: Object.entries(round).map(([level, garment]) => ({
    level,
    garment,
    body: { bust: 900, underbust: 720, waist: 700, hip: 960 }[level] ?? 0,
  })),
})

// A corset in sizes E, F and G: 40 mm bigger round each level each size.
const measured = [
  size('E', { bust: 920, underbust: 760, waist: 640, hip: null }),
  size('F', { bust: 960, underbust: 800, waist: 680, hip: null }),
  size('G', { bust: 1000, underbust: 840, waist: 720, hip: null }),
]

describe('the size to cut at each level', () => {
  it('suggests the size nearest the ease wanted, the underbust too', () => {
    // Fitted: 60 mm bigger round the bust (960), 40 under it (760) and at
    // the waist (740).
    expect(suggest(measured, 'fitted')).toEqual({ bust: 'F', underbust: 'E', waist: 'G' })
  })

  it('cuts a measurement between the two sizes either side', () => {
    // A 730 mm waist, close (60 mm smaller): 670 round, three quarters of
    // the way from E to F.
    expect(cutFor(measured, 'waist', 730, 'close')).toEqual({
      size: 'E',
      between: 0.75,
      next: 'F',
    })
  })

  it('cuts in a size where the measurement is on its line', () => {
    expect(cutFor(measured, 'waist', 740, 'close')).toEqual({ size: 'F', between: 0, next: null })
  })

  it('cuts in the nearest size where the measurement is outside them all', () => {
    expect(cutFor(measured, 'waist', 900, 'close')).toEqual({ size: 'G', between: 0, next: null })
    expect(cutFor(measured, 'waist', 500, 'close')).toEqual({ size: 'E', between: 0, next: null })
  })

  it('cuts nowhere the garment doesn’t reach', () => {
    expect(cutFor(measured, 'hip', 960, 'close')).toBeNull()
  })
})

/** A square piece `side` mm across. */
const square = (id: string, name: string, side: number) =>
  ({
    id,
    name,
    outline: {
      start: [0, 0],
      edges: [
        {
          id: 'e1',
          segments: [
            { kind: 'line', to: [side, 0] },
            { kind: 'line', to: [side, side] },
            { kind: 'line', to: [0, side] },
            { kind: 'line', to: [0, 0] },
          ],
        },
      ],
    },
  }) as unknown as Piece

describe('a pattern’s sizes on the table', () => {
  it('are found by name, smallest first, each named without its size', () => {
    const found = sizesOnTable([
      square('a', 'Front (G)', 120),
      square('b', 'Front (E)', 100),
      square('c', 'Back (E)', 90),
      square('d', 'Front (F)', 110),
      square('e', 'Back (G)', 110),
      square('f', 'Back (F)', 100),
      square('g', 'Pocket', 50),
    ])
    expect(found?.sizes.map((s) => s.size)).toEqual(['E', 'F', 'G'])
    expect(found?.sizes[0]!.pieces.map((p) => [p.id, p.name])).toEqual([
      ['b', 'Front'],
      ['c', 'Back'],
    ])
    expect(found?.sizeOf.get('a')).toBe('G')
    expect(found?.sizeOf.has('g')).toBe(false)
  })

  it('are none where only one size is there, or the sizes share no piece', () => {
    expect(sizesOnTable([square('a', 'Front (G)', 100), square('b', 'Back', 100)])).toBeNull()
    expect(sizesOnTable([square('a', 'Front (G)', 100), square('b', 'Back (F)', 100)])).toBeNull()
  })
})
