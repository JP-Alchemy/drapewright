// Draws the work table: a cutting mat with a true-size centimetre grid and
// the pieces laid on it like pattern paper.

import type { Hit, Piece, PieceView, Seam } from '@drapewright/engine'
import type { DisplayUnits } from '@drapewright/engine'
import type { GrainPreview, MovePreview, PartRef, Picking } from '../state/session.ts'
import { cutSummary } from '../app/PieceDetails.tsx'
import { formatLength } from '../strings/en.ts'
import { edgePoints, stretch } from './edges.ts'
import { type PiecePaths, addSegment } from './paths.ts'
import { type ViewportState, scaleOf, toScreen, visibleWorld } from './viewport.ts'

export interface ScenePiece {
  piece: Piece
  view: PieceView
  paths: PiecePaths
}

export interface Scene {
  pieces: ScenePiece[]
  selection: ReadonlySet<string>
  part?: PartRef | null
  units?: DisplayUnits
  hover?: Hit | null
  preview?: MovePreview | null
  grainPreview?: GrainPreview | null
  /** Points tapped so far to place an alteration. */
  picking?: Picking | null
  seams?: SeamMark[]
}

/** One side of a seam, as drawn: its number, beside its edge. */
export interface SeamMark {
  number: number
  piece: string
  edge: string
  /** The piece's other half (drawn on the piece itself, marked ′). */
  mirror: boolean
  /** The picked seam, or an edge waiting to be sewn. */
  strong: boolean
  /** Sewing only part of the edge: from and to how far along it, mm. */
  from?: number | null
  to?: number | null
}

/** Marks for every seam side, numbered as the seam list numbers them. */
export function seamMarks(
  seams: Seam[],
  picked: string | null,
  sewing: { piece: string; edge: string } | null,
): SeamMark[] {
  const marks: SeamMark[] = seams.flatMap((seam, i) => {
    // Sewn to its own other half: one edge, one mark.
    const self = seam.a.piece === seam.b.piece && seam.a.edge === seam.b.edge
    return (self ? [seam.a] : [seam.a, seam.b]).map((side) => ({
      number: i + 1,
      piece: side.piece,
      edge: side.edge,
      mirror: side.mirror,
      strong: seam.id === picked,
      from: side.from,
      to: side.to,
    }))
  })
  if (sewing) marks.push({ number: 0, ...sewing, mirror: false, strong: true })
  return marks
}

export interface Theme {
  mat: string
  matLine: string
  matLineMajor: string
  paper: string
  ink: string
  stitch: string
  select: string
  seam: string
  /** Alterations' lines. */
  accent: string
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
    seam: v('--seam', '#7c3aed'),
    accent: v('--accent', '#b4532a'),
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

function placement(piece: Piece, preview?: MovePreview | null): DOMMatrix {
  const { x, y, rotation } = piece.placement
  const moving = preview?.ids.has(piece.id)
  return new DOMMatrix()
    .translateSelf(x + (moving ? preview!.dx : 0), y + (moving ? preview!.dy : 0))
    .rotateSelf(rotation)
}

/** One edge of a piece's outline, in piece coordinates. */
function edgePath(piece: Piece, edgeId: string): Path2D | null {
  const edges = piece.outline.edges
  const i = edges.findIndex((e) => e.id === edgeId)
  if (i < 0) return null
  const prev = i === 0 ? null : edges[i - 1]?.segments.at(-1)
  const [sx, sy] = prev ? prev.to : piece.outline.start
  const path = new Path2D()
  path.moveTo(sx, sy)
  for (const seg of edges[i]!.segments) addSegment(path, seg)
  return path
}

/** A run of points as an open path. */
function linePath(points: [number, number][]): Path2D {
  const path = new Path2D()
  points.forEach(([x, y], k) => (k === 0 ? path.moveTo(x, y) : path.lineTo(x, y)))
  return path
}

/** The corner where `edgeId` ends, in piece coordinates. */
function cornerPoint(piece: Piece, edgeId: string): [number, number] | null {
  const edge = piece.outline.edges.find((e) => e.id === edgeId)
  return edge?.segments.at(-1)?.to ?? null
}

/** Notch marks: the slit (or V) at the cutting line, and a faint line across the allowance. */
function drawNotches(ctx: CanvasRenderingContext2D, view: PieceView, s: number, theme: Theme) {
  if (view.notches.length === 0) return
  ctx.beginPath()
  for (const m of view.notches) {
    for (const [x0, y0, x1, y1] of m.lines) {
      ctx.moveTo(x0, y0)
      ctx.lineTo(x1, y1)
    }
  }
  ctx.strokeStyle = theme.ink
  ctx.lineWidth = 1.5 / s
  ctx.lineCap = 'round'
  ctx.stroke()
  ctx.beginPath()
  for (const m of view.notches) {
    ctx.moveTo(m.onOutline[0], m.onOutline[1])
    ctx.lineTo(m.onCut[0], m.onCut[1])
  }
  ctx.strokeStyle = theme.stitch
  ctx.lineWidth = 1 / s
  ctx.stroke()
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

  for (const sp of scene.pieces) {
    const { piece, paths } = sp
    const selected = scene.selection.has(piece.id)
    ctx.setTransform(world.multiply(placement(piece, scene.preview)))
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
    ctx.strokeStyle = theme.ink
    ctx.setLineDash([8 / s, 3 / s, 2 / s, 3 / s])
    ctx.stroke(paths.dashedLines)
    ctx.setLineDash([])
    ctx.stroke(paths.lines)
    ctx.stroke(paths.marks)
    ctx.fillStyle = theme.ink
    ctx.fill(paths.dots)
    drawNotches(ctx, sp.view, s, theme)
    drawGrainline(ctx, piece, s, theme)
    // Altered: as drawn, faintly, and where it was altered.
    if (paths.drawn) {
      ctx.setLineDash([2 / s, 3 / s])
      ctx.strokeStyle = theme.stitch
      ctx.lineWidth = 1 / s
      ctx.stroke(paths.drawn)
    }
    if (paths.alterLines) {
      ctx.setLineDash([10 / s, 4 / s])
      ctx.strokeStyle = theme.accent
      ctx.lineWidth = 1.5 / s
      ctx.stroke(paths.alterLines)
    }
    ctx.setLineDash([])
  }

  // Points tapped to place an alteration, and the slashes between them.
  const picking = scene.picking
  const pickPiece = picking && scene.pieces.find((p) => p.piece.id === picking.piece)
  if (picking && pickPiece && picking.points.length > 0) {
    ctx.setTransform(world.multiply(placement(pickPiece.piece, scene.preview)))
    const [first, ...rest] = picking.points
    ctx.strokeStyle = theme.accent
    ctx.fillStyle = theme.accent
    ctx.lineWidth = 1.5 / s
    ctx.setLineDash([10 / s, 4 / s])
    // A slash runs from its start to its hinge; a bust adjustment's from
    // the bust point to each of the others.
    const lines =
      picking.for === 'spread' ? (rest[0] ? [[first!, rest[0]]] : []) : rest.map((p) => [first!, p])
    for (const [a, b] of lines) {
      ctx.beginPath()
      ctx.moveTo(a![0], a![1])
      ctx.lineTo(b![0], b![1])
      ctx.stroke()
    }
    ctx.setLineDash([])
    for (const [x, y] of picking.points) {
      ctx.beginPath()
      ctx.arc(x, y, 4 / s, 0, Math.PI * 2)
      ctx.fill()
    }
  }

  // A grainline being drawn.
  const grain = scene.grainPreview
  const grainPiece = grain && scene.pieces.find((p) => p.piece.id === grain.piece)
  if (grain && grainPiece) {
    ctx.setTransform(world.multiply(placement(grainPiece.piece, scene.preview)))
    ctx.setLineDash([5 / s, 4 / s])
    drawGrainline(
      ctx,
      { ...grainPiece.piece, grainline: { from: grain.from, to: grain.to, oneWay: false } },
      s,
      {
        ...theme,
        ink: theme.select,
      },
    )
    ctx.setLineDash([])
  }

  // Sewn edges: a band along the edge (or the part of it sewn) and the
  // seam's number inside it.
  const marks = scene.seams ?? []
  /** The part of its edge a mark sews, where it sews only part of it. */
  const partOf = (mark: SeamMark) => {
    if (mark.from == null && mark.to == null) return null
    const sp = scene.pieces.find((p) => p.piece.id === mark.piece)
    const ev = sp?.view.edges.find((e) => e.id === mark.edge)
    const points = sp && edgePoints(sp.piece, mark.edge)
    if (!ev || !points || ev.length <= 0) return null
    return stretch(points, (mark.from ?? 0) / ev.length, (mark.to ?? ev.length) / ev.length)
  }
  for (const mark of marks) {
    const sp = scene.pieces.find((p) => p.piece.id === mark.piece)
    const part = partOf(mark)
    const path = part ? linePath(part.points) : sp && edgePath(sp.piece, mark.edge)
    if (!sp || !path) continue
    ctx.setTransform(world.multiply(placement(sp.piece, scene.preview)))
    ctx.strokeStyle = mark.strong ? theme.select : theme.seam
    ctx.globalAlpha = mark.strong ? 0.9 : 0.6
    ctx.lineWidth = (mark.strong ? 5 : 3.5) / s
    ctx.lineCap = 'round'
    ctx.stroke(path)
    ctx.globalAlpha = 1
  }

  // Hovered and selected edges and corners, over everything else.
  const highlight = (pieceId: string, edgeId: string, corner: boolean, strong: boolean) => {
    const sp = scene.pieces.find((p) => p.piece.id === pieceId)
    if (!sp) return
    ctx.setTransform(world.multiply(placement(sp.piece, scene.preview)))
    ctx.strokeStyle = theme.select
    ctx.globalAlpha = strong ? 1 : 0.55
    if (edgeId.startsWith('notch:')) {
      const id = edgeId.slice('notch:'.length)
      ctx.lineWidth = (strong ? 4 : 3) / s
      ctx.lineCap = 'round'
      for (const m of sp.view.notches.filter((n) => n.notch === id)) {
        ctx.beginPath()
        for (const [x0, y0, x1, y1] of m.lines) {
          ctx.moveTo(x0, y0)
          ctx.lineTo(x1, y1)
        }
        ctx.moveTo(m.onOutline[0], m.onOutline[1])
        ctx.lineTo(m.onCut[0], m.onCut[1])
        ctx.stroke()
      }
    } else if (corner) {
      const c = cornerPoint(sp.piece, edgeId)
      if (c) {
        ctx.beginPath()
        ctx.arc(c[0], c[1], (strong ? 6 : 5) / s, 0, Math.PI * 2)
        ctx.lineWidth = 2 / s
        ctx.stroke()
      }
    } else {
      const path = edgePath(sp.piece, edgeId)
      if (path) {
        ctx.lineWidth = (strong ? 4 : 3) / s
        ctx.lineCap = 'round'
        ctx.stroke(path)
      }
    }
    ctx.globalAlpha = 1
  }
  const hover = scene.hover
  if (hover && (hover.kind === 'edge' || hover.kind === 'corner')) {
    highlight(hover.piece, hover.edge, hover.kind === 'corner', false)
  }
  if (hover?.kind === 'notch') highlight(hover.piece, `notch:${hover.notch}`, false, false)
  const part = scene.part
  if (part?.kind === 'notch') highlight(part.piece, `notch:${part.notch}`, false, true)
  else if (part) highlight(part.piece, part.edge, part.kind === 'corner', true)

  // Lengths of the hovered and selected edges, just outside the cutting line.
  const lengthLabels: { piece: string; edge: string }[] = []
  if (hover?.kind === 'edge') lengthLabels.push(hover)
  if (part?.kind === 'edge') lengthLabels.push(part)
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  for (const ref of lengthLabels) {
    const sp = scene.pieces.find((p) => p.piece.id === ref.piece)
    const ev = sp?.view.edges.find((e) => e.id === ref.edge)
    if (!sp || !ev) continue
    const out = ev.allowance + 14 / s
    const local = new DOMPoint(
      ev.middle[0] + ev.middleNormal[0] * out,
      ev.middle[1] + ev.middleNormal[1] * out,
    )
    const m = placement(sp.piece, scene.preview).transformPoint(local)
    const at = toScreen(v, { x: m.x, y: m.y })
    drawPill(ctx, formatLength(ev.length, scene.units ?? 'cm'), at.x, at.y, theme)
  }

  // Seam numbers, just inside each sewn edge.
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  for (const mark of marks) {
    if (mark.number === 0) continue
    const sp = scene.pieces.find((p) => p.piece.id === mark.piece)
    const ev = sp?.view.edges.find((e) => e.id === mark.edge)
    if (!sp || !ev) continue
    const inward = 16 / s
    const part = partOf(mark)
    const [middle, out] = part ? [part.middle, part.out] : [ev.middle, ev.middleNormal]
    const local = new DOMPoint(middle[0] - out[0] * inward, middle[1] - out[1] * inward)
    const m = placement(sp.piece, scene.preview).transformPoint(local)
    const at = toScreen(v, { x: m.x, y: m.y })
    drawBadge(ctx, `${mark.number}${mark.mirror ? '′' : ''}`, at.x, at.y, mark.strong, theme)
  }

  // Names in screen space, so they stay readable at any zoom.
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.font = '600 13px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  for (const { piece, view } of scene.pieces) {
    const [x0, y0, x1, y1] = view.bounds
    const m = placement(piece, scene.preview).transformPoint(
      new DOMPoint((x0 + x1) / 2, (y0 + y1) / 2),
    )
    const p = toScreen(v, { x: m.x, y: m.y })
    const widthPx = (x1 - x0) * s
    if (widthPx < 40) continue
    ctx.font = '600 13px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
    ctx.lineWidth = 4
    ctx.strokeStyle = theme.labelHalo
    ctx.strokeText(piece.name, p.x, p.y, widthPx - 8)
    ctx.fillStyle = theme.label
    ctx.fillText(piece.name, p.x, p.y, widthPx - 8)
    const summary = cutSummary(piece)
    if (summary && (y1 - y0) * s > 60) {
      ctx.font = '12px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
      ctx.strokeText(summary, p.x, p.y + 17, widthPx - 8)
      ctx.fillText(summary, p.x, p.y + 17, widthPx - 8)
    }
  }
}

/** A seam's number in a round badge. */
function drawBadge(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number,
  strong: boolean,
  theme: Theme,
) {
  ctx.font = '700 12px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  const w = Math.max(20, ctx.measureText(text).width + 10)
  ctx.beginPath()
  ctx.roundRect(x - w / 2, y - 10, w, 20, 10)
  ctx.fillStyle = strong ? theme.select : theme.seam
  ctx.fill()
  ctx.fillStyle = '#fff'
  ctx.fillText(text, x, y + 0.5)
}

function drawPill(ctx: CanvasRenderingContext2D, text: string, x: number, y: number, theme: Theme) {
  ctx.font = '600 12px -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif'
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  const w = ctx.measureText(text).width + 14
  const h = 22
  ctx.beginPath()
  ctx.roundRect(x - w / 2, y - h / 2, w, h, h / 2)
  ctx.fillStyle = theme.select
  ctx.fill()
  ctx.fillStyle = '#fff'
  ctx.fillText(text, x, y + 0.5)
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
