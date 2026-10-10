import type { Piece, PieceView, Segment } from '@soom/engine'

/** Canvas paths for one piece, in piece coordinates (mm). */
export interface PiecePaths {
  /** The cut piece: filled as paper and stroked as the cutting line. */
  cut: Path2D
  /** The stitching line, drawn dashed. */
  stitch: Path2D
}

function polyline(flat: number[]): Path2D {
  const path = new Path2D()
  for (let i = 0; i + 1 < flat.length; i += 2) {
    if (i === 0) path.moveTo(flat[i]!, flat[i + 1]!)
    else path.lineTo(flat[i]!, flat[i + 1]!)
  }
  path.closePath()
  return path
}

function outlinePath(piece: Piece): Path2D {
  const path = new Path2D()
  const [sx, sy] = piece.outline.start
  path.moveTo(sx, sy)
  for (const edge of piece.outline.edges) {
    for (const seg of edge.segments) addSegment(path, seg)
  }
  path.closePath()
  return path
}

export function addSegment(path: Path2D, seg: Segment): void {
  if ('c1' in seg)
    path.bezierCurveTo(seg.c1[0], seg.c1[1], seg.c2[0], seg.c2[1], seg.to[0], seg.to[1])
  else path.lineTo(seg.to[0], seg.to[1])
}

/**
 * The outline keeps its exact curves; the derived line is the engine's
 * polyline (within 0.002 mm of the true curve).
 */
export function buildPaths(piece: Piece, view: PieceView): PiecePaths {
  const exact = outlinePath(piece)
  return view.outlineIsCutting
    ? { cut: exact, stitch: polyline(view.stitching) }
    : { cut: polyline(view.cutting), stitch: exact }
}
