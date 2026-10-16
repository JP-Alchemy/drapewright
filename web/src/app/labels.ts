// How pieces' edges, seam sides and measurements are named in the app.

import type { Document, Figure, MeasurementKey, Piece, Seam, SeamSide } from '@drapewright/engine'
import { t } from '../strings/en.ts'

/** A measurement's name and how to take it, for this figure. */
export function measurementText(key: MeasurementKey, figure: Figure): readonly [string, string] {
  return key === 'bust' && figure === 'man' ? t.measure.chest : t.measure.names[key]
}

/** "Side seam"; "the hem" or "the fold" when unnamed; else "edge 3". */
export function edgeLabel(piece: Piece, edgeId: string): string {
  const i = piece.outline.edges.findIndex((e) => e.id === edgeId)
  const edge = piece.outline.edges[i]
  if (edge?.name) return edge.name
  if (edge?.kind === 'hem') return t.inspector.theHem
  if (edge?.kind === 'fold') return t.inspector.theFold
  return t.inspector.edgeNumber(i + 1)
}

/** "side seam", or "part of side seam" where a seam sews only part of it. */
function sewnEdge(piece: Piece, side: SeamSide): string {
  const edge = edgeLabel(piece, side.edge)
  return side.from != null || side.to != null ? t.seams.partOf(edge) : edge
}

/** "Front · side seam", or "Back, other half · centre back". */
export function sideLabel(doc: Document, side: SeamSide): string {
  const piece = doc.pieces.find((p) => p.id === side.piece)
  if (!piece) return '?'
  const name = side.mirror ? t.seams.otherHalf(piece.name) : piece.name
  return `${name} · ${sewnEdge(piece, side)}`
}

/**
 * A seam as listed under one of its pieces: the piece it is sewn to
 * ("Back", or "Back, other half"), and the edges sewn, this piece's first
 * ("side seam ↔ side seam").
 */
export function seamUnder(doc: Document, piece: string, seam: Seam): { to: string; edges: string } {
  const here = [seam.a, seam.b].filter((side) => side.piece === piece)
  const near = here.find((side) => !side.mirror) ?? here[0] ?? seam.a
  const far = near === seam.a ? seam.b : seam.a
  const edge = (side: SeamSide) => {
    const on = doc.pieces.find((p) => p.id === side.piece)
    const label = on ? sewnEdge(on, side) : '?'
    return side.mirror && side === near ? t.seams.halfEdge(label) : label
  }
  const other = doc.pieces.find((p) => p.id === far.piece)
  const to = !other ? '?' : far.mirror ? t.seams.otherHalf(other.name) : other.name
  return { to, edges: t.seams.between(edge(near), edge(far)) }
}
