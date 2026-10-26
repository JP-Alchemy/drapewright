// The size to cut at each level of the figure: the size whose ease comes
// nearest to the fit wanted, or, for a measurement, a cut between the lines
// of the two sizes it falls between. And a pattern's sizes found on the
// table, brought in side by side ("Front (8)", "Front (10)"), to blend.

import type { Blended, Piece, SizeMeasures, SizePieces } from '@drapewright/engine'
import { t } from '../strings/en.ts'
import { FITS, type Fit, SIZE_LEVELS, type SizeLevel } from './fit.ts'

/** At each level, the size whose ease comes nearest to what `fit` wants. */
export function suggest(measured: SizeMeasures[], fit: Fit): Partial<Record<SizeLevel, string>> {
  const out: Partial<Record<SizeLevel, string>> = {}
  for (const level of SIZE_LEVELS) {
    let best: { size: string; off: number } | null = null
    for (const m of measured) {
      const g = m.girths.find((x) => x.level === level)
      if (!g || g.garment === null) continue
      const off = Math.abs(g.garment - g.body - FITS[fit][level])
      if (!best || off < best.off) best = { size: m.size, off }
    }
    if (best) out[level] = best.size
  }
  return out
}

/** Where a level is cut: in `size`, or `between` (0 to 1) of the way on to `next`. */
export interface Cut {
  size: string
  between: number
  next: string | null
}

/**
 * Where to cut at `level` to fit a body `body` mm round there as `fit`
 * wants: between the two sizes either side of it there, or in the nearest
 * size where it is outside them all. None where no size reaches the level.
 */
export function cutFor(
  measured: SizeMeasures[],
  level: SizeLevel,
  body: number,
  fit: Fit,
): Cut | null {
  const want = body + FITS[fit][level]
  const round = measured.map((m) => m.girths.find((g) => g.level === level)?.garment ?? null)
  for (let k = 0; k + 1 < measured.length; k++) {
    const [a, b] = [round[k], round[k + 1]]
    if (a == null || b == null || a === b) continue
    const s = (want - a) / (b - a)
    if (s < 0 || s > 1) continue
    if (s > 0.995) return { size: measured[k + 1]!.size, between: 0, next: null }
    if (s < 0.005) return { size: measured[k]!.size, between: 0, next: null }
    return { size: measured[k]!.size, between: s, next: measured[k + 1]!.size }
  }
  let nearest: { k: number; off: number } | null = null
  round.forEach((g, k) => {
    if (g != null && (!nearest || Math.abs(g - want) < nearest.off))
      nearest = { k, off: Math.abs(g - want) }
  })
  const k = (nearest as { k: number } | null)?.k
  return k === undefined ? null : { size: measured[k]!.size, between: 0, next: null }
}

/** A level and how it is cut, said as in a blended piece's notes. */
export interface CutAt {
  level: SizeLevel
  said: string
}

/**
 * The pieces blended, ready to add: those blended between sizes say so in
 * their notes, in place of the size they were; and what to tell of it.
 * `sizeOf` gives a piece's size by id, where it has one.
 */
export function blendedPieces(
  blended: Blended,
  at: CutAt[],
  sizeOf: (id: string) => string | undefined,
): { pieces: Piece[]; said: string } {
  const parts = at.map((c) => c.said)
  // Blended: those of one size each, not cut in one size.
  const oneSize = new Set(blended.notes.map((n) => n.piece))
  const mixed = (p: Piece) => sizeOf(p.id) !== undefined && !oneSize.has(p.name)
  // Its notes say how it's blended, not the size it was (a CAD file's
  // "Size 38").
  const notes = (p: Piece) => {
    const was = `Size ${sizeOf(p.id)}`
    const kept = p.notes
      .split('. ')
      .filter((part) => part && part !== was)
      .join('. ')
    const note = t.sizing.blendedNote(parts)
    return kept ? `${kept}\n${note}` : note
  }
  const levelName = (c?: CutAt) => (c ? (t.avatar.levels[c.level] ?? c.level) : '')
  const [top, bottom] = [levelName(at[0]), levelName(at[at.length - 1])]
  return {
    pieces: blended.pieces.map((p) => (mixed(p) ? { ...p, notes: notes(p) } : p)),
    said: [
      t.sizing.blended(blended.pieces.filter(mixed).length, parts),
      ...blended.notes.map((n) => t.sizing.oneSize(n.piece, n.size, n.why, top, bottom)),
    ].join(' '),
  }
}

/** A piece's name and size, as a pattern's sizes are named side by side. */
const SIZED = /^(.*\S) \(([^()]+)\)$/

/**
 * A pattern's sizes on the table, smallest first: pieces named alike but
 * for their sizes ("Front (8)", "Front (10)"), each size's pieces named
 * without it. None unless two sizes or more share a piece.
 */
export function sizesOnTable(
  pieces: Piece[],
): { sizes: SizePieces[]; sizeOf: Map<string, string> } | null {
  const bySize = new Map<string, Piece[]>()
  const sizeOf = new Map<string, string>()
  for (const p of pieces) {
    const m = SIZED.exec(p.name)
    if (!m) continue
    const [, name, size] = m as unknown as [string, string, string]
    sizeOf.set(p.id, size)
    bySize.set(size, [...(bySize.get(size) ?? []), { ...p, name }])
  }
  // The pieces every size has, by name.
  const names = [...bySize.values()].map((ps) => new Set(ps.map((p) => p.name)))
  const shared = [...(names[0] ?? [])].filter((n) => names.every((s) => s.has(n)))
  if (bySize.size < 2 || shared.length === 0) return null
  // Smallest first: by the area of the pieces they share.
  const area = (ps: Piece[]) =>
    ps.filter((p) => shared.includes(p.name)).reduce((sum, p) => sum + Math.abs(shoelace(p)), 0)
  const sizes = [...bySize.entries()]
    .map(([size, ps]) => ({ size, pieces: ps, area: area(ps) }))
    .sort((a, b) => a.area - b.area)
    .map(({ size, pieces }) => ({ size, pieces }))
  return { sizes, sizeOf }
}

/** The area inside a piece's outline, its curves taken by their ends, mm². */
function shoelace(p: Piece): number {
  const points: [number, number][] = [
    p.outline.start,
    ...p.outline.edges.flatMap((e) => e.segments.map((s) => s.to)),
  ]
  let sum = 0
  points.forEach(([x0, y0], i) => {
    const [x1, y1] = points[(i + 1) % points.length]!
    sum += x0 * y1 - x1 * y0
  })
  return sum / 2
}
