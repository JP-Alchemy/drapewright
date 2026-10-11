import type { ImportNote } from '@drapewright/engine'

/** A note, and the pieces it is about (none for notes about the whole file). */
export interface NoteGroup {
  note: ImportNote
  pieces: string[]
}

/** Notes about pieces that say the same thing share this key. */
function sameAs(note: ImportNote): string | null {
  switch (note.kind) {
    case 'seamLineFound':
      return `seamLineFound ${note.allowance}`
    case 'cuttingLineDiffers':
    case 'outlineCrossesItself':
      return note.kind
    default:
      return null
  }
}

/**
 * Notes that say the same about several pieces, said once: an allowance
 * measured at the same width, cutting lines that differ (by the most any
 * does), outlines that cross themselves. Other notes stay as they are, and
 * the order is kept.
 */
export function groupNotes(notes: ImportNote[]): NoteGroup[] {
  const groups: NoteGroup[] = []
  const byKey = new Map<string, NoteGroup>()
  for (const note of notes) {
    const key = sameAs(note)
    if (key === null || !('piece' in note)) {
      groups.push({ note, pieces: [] })
      continue
    }
    const group = byKey.get(key)
    if (!group) {
      const first = { note, pieces: [note.piece] }
      byKey.set(key, first)
      groups.push(first)
      continue
    }
    group.pieces.push(note.piece)
    if (
      note.kind === 'cuttingLineDiffers' &&
      group.note.kind === 'cuttingLineDiffers' &&
      note.maxMm > group.note.maxMm
    ) {
      group.note = note
    }
  }
  return groups
}
