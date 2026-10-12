// How pieces' edges, seam sides and measurements are named in the app.

import type { Document, Figure, MeasurementKey, Piece, SeamSide } from '@drapewright/engine'
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

/** "Front · side seam", or "Back, other half · centre back". */
export function sideLabel(doc: Document, side: SeamSide): string {
  const piece = doc.pieces.find((p) => p.id === side.piece)
  if (!piece) return '?'
  const name = side.mirror ? t.seams.otherHalf(piece.name) : piece.name
  const edge = edgeLabel(piece, side.edge)
  const part = side.from != null || side.to != null
  return `${name} · ${part ? t.seams.partOf(edge) : edge}`
}
