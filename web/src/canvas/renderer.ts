// Draws the work table: a cutting mat with a true-size centimetre grid and
// the pieces laid on it like pattern paper.

import type { Piece, PieceView } from '@soom/engine'
import type { PiecePaths } from './paths.ts'
import { type ViewportState, scaleOf, toScreen, visibleWorld } from './viewport.ts'

export interface ScenePiece {
  piece: Piece
  view: PieceView
  paths: PiecePaths
}

export interface Scene {
  pieces: ScenePiece[]
  selection: ReadonlySet<string>
}

export interface Theme {
  mat: string
  matLine: string
  matLineMajor: string
  paper: string
  ink: string
  stitch: string
  select: string
  label: string
  labelHalo: string
}

/** Reads the palette from CSS custom properties, so light and dark follow the app. */
export function readTheme(el: Element): Theme {
  const css = getComputedStyle(el)
  const v = (name: string, fallback: string) => css.getPropertyValue(name).trim() || fallback
  return {
    mat: v('--mat', '#2f5d50'),
    matLine: v('--mat-line', '#3b6d5f'),
    matLineMajor: v('--mat-line-major', '#57907f'),
    paper: v('--paper', '#fbf7f0'),
    ink: v('--ink-on-paper', '#1f2a2e'),
    stitch: v('--accent', '#b4532a'),
    select: v('--select', '#2563eb'),
    label: v('--ink-on-paper', '#1f2a2e'),
    labelHalo: v('--paper', '#fbf7f0'),
  }
}

function drawGrid(ctx: CanvasRenderingContext2D, v: ViewportState, theme: Theme) {
  const s = scaleOf(v)
  const r = visibleWorld(v)
  const lines = (step: number, color: string, width: number) => {
    ctx.beginPath()
    for (let x = Math.floor(r.x0 / step) * step; x <= r.x1; x += step) {
      ctx.moveTo(x, r.y0)
      ctx.lineTo(x, r.y1)
    }
    for (let y = Math.floor(r.y0 / step) * step; y <= r.y1; y += step) {
      ctx.moveTo(r.x0, y)
      ctx.lineTo(r.x1, y)
    }
    ctx.strokeStyle = color
    ctx.lineWidth = width / s
    ctx.stroke()
  }
  // 1 cm squares, bolder every 5 cm; coarser when zoomed far out.
  if (10 * s >= 6) lines(10, theme.matLine, 1)
  if (50 * s >= 6) lines(50, theme.matLineMajor, 1.25)
  else lines(500, theme.matLineMajor, 1.25)
}

function placement(piece: Piece): DOMMatrix {
  const { x, y, rotation } = piece.placement
  return new DOMMatrix().translateSelf(x, y).rotateSelf(rotation)
}

function drawGrainline(ctx: CanvasRenderingContext2D, piece: Piece, s: number, theme: Theme) {
  const g = piece.grainline
  if (!g) return
  const [ax, ay] = g.from
  const [bx, by] = g.to
  const len = Math.hypot(bx - ax, by - ay)
  if (len === 0) return
  const ux = (bx - ax) / len
  const uy = (by - ay) / len
  const head = Math.min(6, len / 4)
  ctx.beginPath()
  ctx.moveTo(ax, ay)
  ctx.lineTo(bx, by)
  const arrow = (tx: number, ty: number, dx: number, dy: number) => {
    ctx.moveTo(tx - dx * head - dy * head * 0.45, ty - dy * head + dx * head * 0.45)
    ctx.lineTo(tx, ty)
    ctx.lineTo(tx - dx * head + dy * head * 0.45, ty - dy * head - dx * head * 0.45)
  }
  arrow(bx, by, ux, uy)
  if (!g.oneWay) arrow(ax, ay, -ux, -uy)
  ctx.strokeStyle = theme.ink
  ctx.lineWidth = 1.25 / s
  ctx.lineCap = 'round'
  ctx.lineJoin = 'round'
  ctx.stroke()
}

export function draw(
  ctx: CanvasRenderingContext2D,
  v: ViewportState,
  dpr: number,
  scene: Scene,
  theme: Theme,
) {
  const s = scaleOf(v)
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.fillStyle = theme.mat
  ctx.fillRect(0, 0, v.width, v.height)

  // World transform: mm → device pixels.
  const world = new DOMMatrix([
    dpr * s,
    0,
    0,
    dpr * s,
    dpr * (v.width / 2 - v.cx * s),
    dpr * (v.height / 2 - v.cy * s),
  ])
  ctx.setTransform(world)
  drawGrid(ctx, v, theme)

  for (const { piece, paths } of scene.pieces) {
    const selected = scene.selection.has(piece.id)
    ctx.setTransform(world.multiply(placement(piece)))
    ctx.fillStyle = theme.paper
    ctx.fill(paths.cut)
    ctx.lineJoin = 'round'
    ctx.strokeStyle = selected ? theme.select : theme.ink
    ctx.lineWidth = (selected ? 2.5 : 1.25) / s
    ctx.stroke(paths.cut)
    ctx.setLineDash([6 / s, 4 / s])
    ctx.strokeStyle = theme.stitch
    ctx.lineWidth = 1 / s
    ctx.stroke(paths.stitch)
    ctx.setLineDash([])
    drawGrainline(ctx, piece, s, theme)
  }

  // Names in screen space, so they stay readable at any zoom.
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.font = '600 13px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  for (const { piece, view } of scene.pieces) {
    const [x0, y0, x1, y1] = view.bounds
    const m = placement(piece).transformPoint(new DOMPoint((x0 + x1) / 2, (y0 + y1) / 2))
    const p = toScreen(v, { x: m.x, y: m.y })
    const widthPx = (x1 - x0) * s
    if (widthPx < 40) continue
    ctx.lineWidth = 4
    ctx.strokeStyle = theme.labelHalo
    ctx.strokeText(piece.name, p.x, p.y, widthPx - 8)
    ctx.fillStyle = theme.label
    ctx.fillText(piece.name, p.x, p.y, widthPx - 8)
  }
}

/** Bounds of all pieces on the work table, mm. */
export function sceneBounds(
  pieces: ScenePiece[],
): { x0: number; y0: number; x1: number; y1: number } | null {
  let r: { x0: number; y0: number; x1: number; y1: number } | null = null
  for (const { piece, view } of pieces) {
    const [x0, y0, x1, y1] = view.bounds
    const m = placement(piece)
    for (const [x, y] of [
      [x0, y0],
      [x1, y0],
      [x1, y1],
      [x0, y1],
    ] as const) {
      const p = m.transformPoint(new DOMPoint(x, y))
      r = r
        ? {
            x0: Math.min(r.x0, p.x),
            y0: Math.min(r.y0, p.y),
            x1: Math.max(r.x1, p.x),
            y1: Math.max(r.y1, p.y),
          }
        : { x0: p.x, y0: p.y, x1: p.x, y1: p.y }
    }
  }
  return r
}
