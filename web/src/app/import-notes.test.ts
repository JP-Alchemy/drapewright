import type { ImportNote } from '@drapewright/engine'
import { describe, expect, it } from 'vitest'
import { t } from '../strings/en.ts'
import { groupNotes } from './import-notes.ts'

const seam = (piece: string, allowance: number): ImportNote => ({
  kind: 'seamLineFound',
  piece,
  allowance,
})
const differs = (piece: string, maxMm: number): ImportNote => ({
  kind: 'cuttingLineDiffers',
  piece,
  maxMm,
})

describe('import notes', () => {
  it('say the same about several pieces once', () => {
    const groups = groupNotes([
      seam('a', 10),
      differs('a', 9.3),
      seam('b', 10),
      differs('b', 9.8),
      seam('c', 15),
      { kind: 'smallShapesIgnored', count: 15 },
    ])
    expect(groups.map((g) => [g.note.kind, g.pieces])).toEqual([
      ['seamLineFound', ['a', 'b']],
      ['cuttingLineDiffers', ['a', 'b']],
      ['seamLineFound', ['c']],
      ['smallShapesIgnored', []],
    ])
    // By the most any piece differs.
    expect(groups[1]?.note).toEqual(differs('b', 9.8))
  })

  it('name the pieces, or all of them', () => {
    expect(t.importing.note(seam('a', 10), ['Front'], 7, 'cm')).toBe(
      'Front: found both the cutting line and the stitching line, so the allowance (1.0 cm) was measured from the file.',
    )
    const cut = differs('a', 9.8)
    expect(t.importing.note(cut, ['Front', 'Back', 'Sleeve'], 3, 'cm')).toMatch(
      /^All 3 pieces: the file’s cutting lines differ from Drapewright’s by up to 10 mm,/,
    )
    expect(t.importing.note(cut, ['Front', 'Back'], 3, 'cm')).toMatch(/^Front and Back: /)
    expect(t.importing.note(cut, ['Front', 'Back'], 2, 'cm')).toMatch(/^Both pieces: /)
  })
})
