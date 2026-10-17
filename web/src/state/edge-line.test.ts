import type { Document, Seam } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import { edgeLine } from './edge-line.ts'

/** A piece with edges of these ids and kinds (their shapes don't matter here). */
const piece = (id: string, edges: [string, string][]) => ({
  id,
  outline: { start: [0, 0], edges: edges.map(([e, kind]) => ({ id: e, kind, segments: [] })) },
})

const seam = (
  a: [string, string],
  b: [string, string],
  { reversed = false, mirror = false, to = null as number | null } = {},
): Seam => ({
  id: `${a.join('')}-${b.join('')}`,
  name: '',
  a: { piece: a[0], edge: a[1], from: null, to, mirror: false },
  b: { piece: b[0], edge: b[1], from: null, to: null, mirror },
  reversed,
  ease: 'even',
  confirmed: true,
})

// A skirt: its front cut on the fold (waist, side, hem, fold), its back cut
// as a pair (waist, side, hem, centre back), sewn at the sides and at the
// centre back to its other half.
const skirt = (sides: Seam = seam(['front', 'side'], ['back', 'side'])) =>
  ({
    pieces: [
      piece('front', [
        ['waist', 'seam'],
        ['side', 'seam'],
        ['hem', 'hem'],
        ['fold', 'fold'],
      ]),
      piece('back', [
        ['waist', 'seam'],
        ['side', 'seam'],
        ['hem', 'hem'],
        ['cb', 'seam'],
      ]),
    ],
    seams: [sides, seam(['back', 'cb'], ['back', 'cb'], { mirror: true })],
  }) as unknown as Pick<Document, 'pieces' | 'seams'>

const ids = (line: { piece: string; edge: string }[]) => line.map((r) => `${r.piece} ${r.edge}`)

describe('the line an edge is part of', () => {
  it('goes round a skirt’s hem across its side seams', () => {
    expect(ids(edgeLine(skirt(), { piece: 'front', edge: 'hem' }))).toEqual([
      'back hem',
      'front hem',
    ])
    // From the back's, the same edges.
    expect(ids(edgeLine(skirt(), { piece: 'back', edge: 'hem' })).sort()).toEqual([
      'back hem',
      'front hem',
    ])
  })

  it('goes round its waist the same way, not onto the side seams', () => {
    expect(ids(edgeLine(skirt(), { piece: 'front', edge: 'waist' }))).toEqual([
      'front waist',
      'back waist',
    ])
  })

  it('stops where the side seam is sewn the other way round, at the waist end', () => {
    // Sewn start to end, the front's hem meets the back's waist.
    const crossed = skirt(seam(['front', 'side'], ['back', 'side'], { reversed: true }))
    expect(ids(edgeLine(crossed, { piece: 'front', edge: 'hem' }))).toEqual([
      'back waist',
      'front hem',
    ])
  })

  it('stops at a seam that does not reach the corner', () => {
    // The side sewn only down to 300 mm of its 600.
    const open = skirt(seam(['front', 'side'], ['back', 'side'], { to: 300 }))
    const length = () => 600
    expect(ids(edgeLine(open, { piece: 'front', edge: 'hem' }, length))).toEqual(['front hem'])
  })

  it('goes round an armhole across the side and shoulder seams', () => {
    const bodice = {
      pieces: [
        piece('front', [
          ['neck', 'seam'],
          ['shoulder', 'seam'],
          ['armhole', 'seam'],
          ['side', 'seam'],
          ['waist', 'seam'],
          ['cf', 'fold'],
        ]),
        piece('back', [
          ['neck', 'seam'],
          ['shoulder', 'seam'],
          ['armhole', 'seam'],
          ['side', 'seam'],
          ['waist', 'seam'],
          ['cb', 'fold'],
        ]),
      ],
      seams: [
        seam(['front', 'shoulder'], ['back', 'shoulder']),
        seam(['front', 'side'], ['back', 'side']),
      ],
    } as unknown as Pick<Document, 'pieces' | 'seams'>
    expect(ids(edgeLine(bodice, { piece: 'front', edge: 'armhole' }))).toEqual([
      'front armhole',
      'back armhole',
    ])
  })
})
