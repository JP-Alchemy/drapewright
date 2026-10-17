// The line an edge is part of, round the garment: a skirt's hem is its
// front's hem and its back's, met at the side seams. From each end of the
// edge the line goes past the corner, across the seam sewing the edge that
// meets it there, and on along the edge beside that seam's other side, as
// Blender's edge loops go round a mesh. It stops at a fold (beyond, the
// same line mirrored), at an edge sewn to nothing there, or once round.

import type { Document, SeamSide } from '@drapewright/engine'

export interface EdgeRef {
  piece: string
  edge: string
}

type End = 'start' | 'end'

const other = (end: End): End => (end === 'start' ? 'end' : 'start')

/** Sewn as far as its `end`: a seam that stops short of it is not crossed there. */
function sewnTo(side: SeamSide, end: End, length: (side: SeamSide) => number | null): boolean {
  if (end === 'start') return side.from === null || side.from <= 1
  if (side.to === null) return true
  const whole = length(side)
  return whole !== null && side.to >= whole - 1
}

/**
 * Past the corner at the `leaving` end of edge `at`: the edge the line goes
 * on along, across the seam sewing the edge that meets it there, and the
 * end of it the line arrives at. None at a fold, or where that edge is not
 * sewn as far as the corner.
 */
function across(
  doc: Pick<Document, 'pieces' | 'seams'>,
  at: EdgeRef,
  leaving: End,
  length: (side: SeamSide) => number | null,
): { ref: EdgeRef; arrives: End } | null {
  const edges = doc.pieces.find((p) => p.id === at.piece)?.outline.edges
  const i = edges?.findIndex((e) => e.id === at.edge) ?? -1
  if (!edges || i < 0 || edges.length < 2) return null
  const n = edges.length
  const meeting = edges[leaving === 'end' ? (i + 1) % n : (i + n - 1) % n]!
  if (meeting.kind === 'fold') return null
  const meetingEnd = other(leaving)
  for (const seam of doc.seams.filter((s) => s.confirmed)) {
    for (const [mine, theirs] of [
      [seam.a, seam.b],
      [seam.b, seam.a],
    ] as const) {
      if (mine.piece !== at.piece || mine.edge !== meeting.id) continue
      if (!sewnTo(mine, meetingEnd, length)) continue
      // `a`'s start is sewn to `b`'s start, or its end where reversed.
      const theirEnd = seam.reversed ? other(meetingEnd) : meetingEnd
      if (!sewnTo(theirs, theirEnd, length)) continue
      const beside = doc.pieces.find((p) => p.id === theirs.piece)?.outline.edges
      const j = beside?.findIndex((e) => e.id === theirs.edge) ?? -1
      if (!beside || j < 0) continue
      const m = beside.length
      // The edge before it ends where it starts; the one after starts
      // where it ends.
      const next = beside[theirEnd === 'start' ? (j + m - 1) % m : (j + 1) % m]!
      return {
        ref: { piece: theirs.piece, edge: next.id },
        arrives: theirEnd === 'start' ? 'end' : 'start',
      }
    }
  }
  return null
}

/**
 * The edges along the line `from` is part of, in order round it, `from`
 * among them. `length` gives a seam side's edge's length (mm), for seams
 * sewing only part of one.
 */
export function edgeLine(
  doc: Pick<Document, 'pieces' | 'seams'>,
  from: EdgeRef,
  length: (side: SeamSide) => number | null = () => null,
): EdgeRef[] {
  const key = (r: EdgeRef) => `${r.piece} ${r.edge}`
  const line: EdgeRef[] = [from]
  const seen = new Set([key(from)])
  // Off its end one way, off its start the other.
  for (const way of ['end', 'start'] as const) {
    let at = from
    let leaving: End = way
    for (let k = 0; k < 500; k++) {
      const next = across(doc, at, leaving, length)
      if (!next || seen.has(key(next.ref))) break
      seen.add(key(next.ref))
      if (way === 'end') line.push(next.ref)
      else line.unshift(next.ref)
      at = next.ref
      leaving = other(next.arrives)
    }
  }
  return line
}
