// How pieces' edges and seam sides are named in the app.

import type { Document, Piece, SeamSide } from '@drapewright/engine'
import { t } from '../strings/en.ts'

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
  return `${name} · ${edgeLabel(piece, side.edge)}`
}
