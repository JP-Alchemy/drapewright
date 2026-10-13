// Projecting the pieces onto fabric on the cutting table, true size: bright
// lines on black (which projects no light), through the projector's map of
// the mat. And the rectangle a projector is calibrated by.

import type { Layout } from '@drapewright/engine'
import { type Homography, type Pt, mapPoint } from './homography.ts'
import type { ScenePiece } from './renderer.ts'

/** Where the table is on the screen: its mm, from `offset` on, through `map`. */
export interface Projection {
  /** Mat mm (from the calibrated rectangle's top left corner) to screen CSS px. */
  map: Homography
  /** The table point at the rectangle's top left corner, mm. */
  offset: Pt
}

const COLOURS = {
  cut: '#ffffff',
  /** Pieces not being cut now, when one is. */
  dim: '#5a5a5a',
  edge: '#ff8a4c',
  stitch: '#5ee1ff',
  grain: '#ffe066',
  label: '#ffffff',
  rectangle: '#ffffff',
  grid: '#4a4a4a',
  handle: '#ff8a4c',
}

/** A piece's own mm to the screen, through where it lies on the table. */
function pieceToScreen(sp: ScenePiece, projection: Projection): (x: number, y: number) => Pt {
  const { x, y, rotation } = sp.piece.placement
  const m = new DOMMatrix().translateSelf(x, y).rotateSelf(rotation)
  const [ox, oy] = projection.offset
  return (px, py) => {
    const p = m.transformPoint(new DOMPoint(px, py))
    return mapPoint(projection.map, [p.x - ox, p.y - oy])
  }
}

/** Lines through flat `x0, y0, x1, y1, …` points, closed. */
function polyline(ctx: CanvasRenderingContext2D, flat: number[], at: (x: number, y: number) => Pt) {
  for (let i = 0; i + 1 < flat.length; i += 2) {
    const [x, y] = at(flat[i]!, flat[i + 1]!)
    if (i === 0) ctx.moveTo(x, y)
    else ctx.lineTo(x, y)
  }
  ctx.closePath()
}

/** How the pieces are drawn: stitching lines too, lines thick for dark fabric, one piece picked out (by index). */
export interface Look {
  stitching: boolean
  thick: boolean
  focus: number | null
}

/** A grainline from `a` to `b` on the screen, arrowed at `b` (and at `a` unless one way). */
function grainArrow(ctx: CanvasRenderingContext2D, a: Pt, b: Pt, oneWay: boolean) {
  const length = Math.hypot(b[0] - a[0], b[1] - a[1])
  if (length === 0) return
  const [ux, uy] = [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
  const head = Math.min(18, length / 4)
  ctx.moveTo(...a)
  ctx.lineTo(...b)
  const arrow = ([x, y]: Pt, dx: number, dy: number) => {
    ctx.moveTo(x - dx * head - dy * head * 0.45, y - dy * head + dx * head * 0.45)
    ctx.lineTo(x, y)
    ctx.lineTo(x - dx * head + dy * head * 0.45, y - dy * head - dx * head * 0.45)
  }
  arrow(b, ux, uy)
  if (!oneWay) arrow(a, -ux, -uy)
}

/** The pieces on the fabric: cutting lines, and if wanted stitching lines, with notches, grainlines and names. */
export function drawProjection(
  ctx: CanvasRenderingContext2D,
  size: { width: number; height: number; dpr: number },
  pieces: ScenePiece[],
  projection: Projection,
  look: Look,
) {
  ctx.setTransform(size.dpr, 0, 0, size.dpr, 0, 0)
  ctx.fillStyle = '#000000'
  ctx.fillRect(0, 0, size.width, size.height)
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'
  const weight = look.thick ? 2 : 1
  pieces.forEach((sp, k) => {
    const dim = look.focus !== null && look.focus !== k
    const at = pieceToScreen(sp, projection)
    const { view, piece } = sp
    if (look.stitching && !dim) {
      ctx.beginPath()
      polyline(ctx, view.stitching, at)
      ctx.setLineDash([10, 6])
      ctx.strokeStyle = COLOURS.stitch
      ctx.lineWidth = 1.5 * weight
      ctx.stroke()
      ctx.setLineDash([])
    }
    ctx.beginPath()
    polyline(ctx, view.cutting, at)
    for (const notch of view.notches) {
      for (const [x0, y0, x1, y1] of notch.lines) {
        ctx.moveTo(...at(x0, y0))
        ctx.lineTo(...at(x1, y1))
      }
    }
    ctx.strokeStyle = dim ? COLOURS.dim : COLOURS.cut
    ctx.lineWidth = 3 * weight
    ctx.stroke()
    const g = piece.grainline
    if (g && !dim) {
      ctx.beginPath()
      grainArrow(ctx, at(g.from[0], g.from[1]), at(g.to[0], g.to[1]), g.oneWay)
      ctx.strokeStyle = COLOURS.grain
      ctx.lineWidth = 2 * weight
      ctx.stroke()
    }
    // Its name in the middle, to know which piece it is.
    const [x0, y0, x1, y1] = view.outlineBounds
    const [cx, cy] = at((x0 + x1) / 2, (y0 + y1) / 2)
    label(ctx, piece.name, cx, cy, dim, look.thick)
  })
}

function label(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  dim: boolean,
  thick: boolean,
) {
  ctx.font = `600 ${thick ? 26 : 18}px system-ui, sans-serif`
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillStyle = dim ? COLOURS.dim : COLOURS.label
  ctx.fillText(text, x, y)
}

/**
 * The cutting layout on the fabric: its fold (or a selvage) along the top
 * and its other edge along the bottom, its cut end, and the pieces as they
 * lie on it. `faceDown` is said under the name of a piece cut face down.
 */
export function drawLayout(
  ctx: CanvasRenderingContext2D,
  size: { width: number; height: number; dpr: number },
  layout: Layout,
  projection: Projection,
  look: Look,
  faceDown: string,
) {
  ctx.setTransform(size.dpr, 0, 0, size.dpr, 0, 0)
  ctx.fillStyle = '#000000'
  ctx.fillRect(0, 0, size.width, size.height)
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'
  const weight = look.thick ? 2 : 1
  const [ox, oy] = projection.offset
  const at = (x: number, y: number) => mapPoint(projection.map, [x - ox, y - oy])
  // The fabric's edges, past its ends.
  const across = layout.folded ? layout.width / 2 : layout.width
  const end = layout.length + 2000
  ctx.beginPath()
  ctx.moveTo(...at(-2000, 0))
  ctx.lineTo(...at(end, 0))
  ctx.moveTo(...at(-2000, across))
  ctx.lineTo(...at(end, across))
  ctx.setLineDash(layout.folded ? [16, 10] : [])
  ctx.strokeStyle = COLOURS.edge
  ctx.lineWidth = 2 * weight
  ctx.stroke()
  ctx.setLineDash([])
  // Where the fabric is cut straight, to start.
  ctx.beginPath()
  ctx.moveTo(...at(0, 0))
  ctx.lineTo(...at(0, across))
  ctx.stroke()
  layout.placed.forEach((p, k) => {
    const dim = look.focus !== null && look.focus !== k
    ctx.beginPath()
    for (const ring of p.rings) {
      ring.forEach(([x, y], i) => (i === 0 ? ctx.moveTo(...at(x, y)) : ctx.lineTo(...at(x, y))))
      ctx.closePath()
    }
    ctx.strokeStyle = dim ? COLOURS.dim : COLOURS.cut
    ctx.lineWidth = 3 * weight
    ctx.stroke()
    if (!dim) {
      ctx.beginPath()
      grainArrow(ctx, at(...p.grain[0]), at(...p.grain[1]), false)
      ctx.strokeStyle = COLOURS.grain
      ctx.lineWidth = 2 * weight
      ctx.stroke()
    }
    const xs = p.rings.flat().map((q) => q[0])
    const ys = p.rings.flat().map((q) => q[1])
    const [cx, cy] = at(
      (Math.min(...xs) + Math.max(...xs)) / 2,
      (Math.min(...ys) + Math.max(...ys)) / 2,
    )
    label(ctx, p.name, cx, cy, dim, look.thick)
    if (p.mirrored) label(ctx, faceDown, cx, cy + (look.thick ? 30 : 22), dim, look.thick)
  })
}

/** The calibration rectangle, `width` by `height` mm, with a 10 cm grid and its corners to drag. */
export function drawCalibration(
  ctx: CanvasRenderingContext2D,
  size: { width: number; height: number; dpr: number },
  map: Homography,
  rectangle: { width: number; height: number },
  corners: readonly Pt[],
) {
  ctx.setTransform(size.dpr, 0, 0, size.dpr, 0, 0)
  ctx.fillStyle = '#000000'
  ctx.fillRect(0, 0, size.width, size.height)
  const line = (a: Pt, b: Pt) => {
    ctx.moveTo(...mapPoint(map, a))
    ctx.lineTo(...mapPoint(map, b))
  }
  ctx.beginPath()
  for (let x = 100; x < rectangle.width; x += 100) line([x, 0], [x, rectangle.height])
  for (let y = 100; y < rectangle.height; y += 100) line([0, y], [rectangle.width, y])
  ctx.strokeStyle = COLOURS.grid
  ctx.lineWidth = 1.5
  ctx.stroke()
  ctx.beginPath()
  ctx.moveTo(...corners[0]!)
  for (const c of corners.slice(1)) ctx.lineTo(...c)
  ctx.closePath()
  ctx.strokeStyle = COLOURS.rectangle
  ctx.lineWidth = 3
  ctx.stroke()
  for (const [x, y] of corners) {
    ctx.beginPath()
    ctx.arc(x, y, 14, 0, Math.PI * 2)
    ctx.strokeStyle = COLOURS.handle
    ctx.lineWidth = 3
    ctx.stroke()
    ctx.beginPath()
    ctx.moveTo(x - 22, y)
    ctx.lineTo(x + 22, y)
    ctx.moveTo(x, y - 22)
    ctx.lineTo(x, y + 22)
    ctx.lineWidth = 1.5
    ctx.stroke()
  }
}
