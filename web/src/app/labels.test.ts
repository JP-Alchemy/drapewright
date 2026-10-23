import type { Document, Piece, Seam } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import { lineLabel, seamUnder, sideLabel } from './labels.ts'

/** A piece with edges of these ids and these lines (their shapes don't matter here). */
const piece = (id: string, name: string, edges: string[], lines: [string, string | null][] = []) =>
  ({
    id,
    name,
    outline: { start: [0, 0], edges: edges.map((e) => ({ id: e, name: '', kind: 'seam' })) },
    lines: lines.map(([l, label]) => ({ id: l, kind: 'placement', label, segments: [] })),
  }) as unknown as Piece

// A skirt front with a pocket's placement drawn on it, and the pocket.
const doc = {
  pieces: [
    piece(
      'front',
      'Skirt front',
      ['fw', 'fs'],
      [
        ['fp', 'Pocket placement'],
        ['fl', null],
      ],
    ),
    piece('pocket', 'Patch pocket', ['pt', 'pr', 'pb', 'pl']),
  ],
} as unknown as Document

// The pocket's right side sewn onto part of the placement.
const onto: Seam = {
  id: 's1',
  name: 'Patch pocket',
  a: { piece: 'pocket', edge: 'pr', line: null, from: null, to: null, mirror: false },
  b: { piece: 'front', edge: '', line: 'fp', from: 130, to: 270, mirror: false },
  reversed: false,
  ease: 'even',
  confirmed: true,
}

describe('a pocket sewn onto its placement', () => {
  it('names the line by its label, or as a line drawn on the piece', () => {
    expect(lineLabel(doc.pieces[0]!, 'fp')).toBe('Pocket placement')
    expect(lineLabel(doc.pieces[0]!, 'fl')).toBe('line drawn on it')
  })

  it('names the side on the line as part of it', () => {
    expect(sideLabel(doc, onto.b)).toBe('Skirt front · part of Pocket placement')
    expect(sideLabel(doc, onto.a)).toBe('Patch pocket · edge 2')
  })

  it('lists the seam under either piece', () => {
    expect(seamUnder(doc, 'pocket', onto)).toEqual({
      to: 'Skirt front',
      edges: 'edge 2 ↔ part of Pocket placement',
    })
    expect(seamUnder(doc, 'front', onto).to).toBe('Patch pocket')
  })
})
