import type { Piece, PieceView, Segment } from '@soom/engine'

/** Canvas paths for one piece, in piece coordinates (mm). */
export interface PiecePaths {
  /** The cut piece: filled as paper and stroked as the cutting line. */
  cut: Path2D
  /** The stitching line, drawn dashed. */
  stitch: Path2D
  /** Lines inside the piece: plain, and dashed (fold, placement and stitching lines). */
  lines: Path2D
  dashedLines: Path2D
  /** Marks: outlined (drill holes, circles, crosses, buttonholes) and filled (dots). */
  marks: Path2D
  dots: Path2D
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

const DASHED_LINES = new Set(['fold', 'placement', 'stitch'])

/** Marks as the printed pattern draws them, with the same default sizes (mm). */
function markPaths(piece: Piece): { marks: Path2D; dots: Path2D } {
  const marks = new Path2D()
  const dots = new Path2D()
  const circle = (path: Path2D, x: number, y: number, r: number) => {
    path.moveTo(x + r, y)
    path.arc(x, y, r, 0, Math.PI * 2)
  }
  const line = (x0: number, y0: number, x1: number, y1: number) => {
    marks.moveTo(x0, y0)
    marks.lineTo(x1, y1)
  }
  for (const m of piece.marks) {
    const [x, y] = m.at
    switch (m.kind) {
      case 'drill': {
        const r = (m.size ?? 3) / 2
        circle(marks, x, y, r)
        line(x - r * 1.6, y, x + r * 1.6, y)
        line(x, y - r * 1.6, x, y + r * 1.6)
        break
      }
      case 'dot':
        circle(dots, x, y, (m.size ?? 3) / 2)
        break
      case 'circle':
      case 'button':
        circle(marks, x, y, (m.size ?? 6) / 2)
        break
      case 'cross': {
        const r = (m.size ?? 6) / 2
        line(x - r, y - r, x + r, y + r)
        line(x - r, y + r, x + r, y - r)
        break
      }
      case 'buttonhole': {
        const half = (m.length ?? 20) / 2
        const a = (m.angle * Math.PI) / 180
        const [dx, dy] = [Math.cos(a), Math.sin(a)]
        line(x - dx * half, y - dy * half, x + dx * half, y + dy * half)
        for (const end of [-half, half]) {
          line(
            x + dx * end + dy * 2,
            y + dy * end - dx * 2,
            x + dx * end - dy * 2,
            y + dy * end + dx * 2,
          )
        }
        break
      }
    }
  }
  return { marks, dots }
}

/**
 * The outline keeps its exact curves; the derived line is the engine's
 * polyline (within 0.002 mm of the true curve).
 */
export function buildPaths(piece: Piece, view: PieceView): PiecePaths {
  const exact = outlinePath(piece)
  const lines = new Path2D()
  const dashedLines = new Path2D()
  for (const l of piece.lines) {
    const path = DASHED_LINES.has(l.kind) ? dashedLines : lines
    path.moveTo(l.start[0], l.start[1])
    for (const seg of l.segments) addSegment(path, seg)
  }
  const outlines = view.outlineIsCutting
    ? { cut: exact, stitch: polyline(view.stitching) }
    : { cut: polyline(view.cutting), stitch: exact }
  return { ...outlines, lines, dashedLines, ...markPaths(piece) }
}
