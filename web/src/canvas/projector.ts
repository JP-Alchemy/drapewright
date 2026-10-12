// Projecting the pieces onto fabric on the cutting table, true size: bright
// lines on black (which projects no light), through the projector's map of
// the mat. And the rectangle a projector is calibrated by.

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

/** The pieces on the fabric: cutting lines, and if wanted stitching lines, with notches, grainlines and names. */
export function drawProjection(
  ctx: CanvasRenderingContext2D,
  size: { width: number; height: number; dpr: number },
  pieces: ScenePiece[],
  projection: Projection,
  options: { stitching: boolean },
) {
  ctx.setTransform(size.dpr, 0, 0, size.dpr, 0, 0)
  ctx.fillStyle = '#000000'
  ctx.fillRect(0, 0, size.width, size.height)
  ctx.lineJoin = 'round'
  ctx.lineCap = 'round'
  for (const sp of pieces) {
    const at = pieceToScreen(sp, projection)
    const { view, piece } = sp
    if (options.stitching) {
      ctx.beginPath()
      polyline(ctx, view.stitching, at)
      ctx.setLineDash([10, 6])
      ctx.strokeStyle = COLOURS.stitch
      ctx.lineWidth = 1.5
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
    ctx.strokeStyle = COLOURS.cut
    ctx.lineWidth = 3
    ctx.stroke()
    const g = piece.grainline
    if (g) {
      const [a, b] = [at(g.from[0], g.from[1]), at(g.to[0], g.to[1])]
      const length = Math.hypot(b[0] - a[0], b[1] - a[1])
      if (length > 0) {
        const [ux, uy] = [(b[0] - a[0]) / length, (b[1] - a[1]) / length]
        const head = Math.min(18, length / 4)
        ctx.beginPath()
        ctx.moveTo(...a)
        ctx.lineTo(...b)
        const arrow = ([x, y]: Pt, dx: number, dy: number) => {
          ctx.moveTo(x - dx * head - dy * head * 0.45, y - dy * head + dx * head * 0.45)
          ctx.lineTo(x, y)
          ctx.lineTo(x - dx * head + dy * head * 0.45, y - dy * head - dx * head * 0.45)
        }
        arrow(b, ux, uy)
        if (!g.oneWay) arrow(a, -ux, -uy)
        ctx.strokeStyle = COLOURS.grain
        ctx.lineWidth = 2
        ctx.stroke()
      }
    }
    // Its name in the middle, to know which piece it is.
    const [x0, y0, x1, y1] = view.outlineBounds
    const [cx, cy] = at((x0 + x1) / 2, (y0 + y1) / 2)
    ctx.font = '600 18px system-ui, sans-serif'
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillStyle = COLOURS.label
    ctx.fillText(piece.name, cx, cy)
  }
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
