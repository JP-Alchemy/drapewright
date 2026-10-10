// Mapping between the work table (millimetres, y down) and the screen (CSS
// pixels). Pure functions over a plain state object, so they are easy to
// test and to keep in a store.

export interface Point {
  x: number
  y: number
}

export interface Rect {
  x0: number
  y0: number
  x1: number
  y1: number
}

export interface ViewportState {
  /** Size of the canvas, CSS px. */
  width: number
  height: number
  /** Work-table point at the middle of the canvas, mm. */
  cx: number
  cy: number
  /** Multiple of true size (1 = actual size on a calibrated screen). */
  zoom: number
  /** CSS px per mm at zoom 1: the screen calibration. */
  pxPerMm: number
}

/** The web's nominal 96 CSS px per inch, used until the screen is calibrated. */
export const NOMINAL_PX_PER_MM = 96 / 25.4

export const MIN_ZOOM = 0.02
export const MAX_ZOOM = 20

export const initialViewport: ViewportState = {
  width: 0,
  height: 0,
  cx: 0,
  cy: 0,
  zoom: 0.5,
  pxPerMm: NOMINAL_PX_PER_MM,
}

/** CSS px per mm at the current zoom. */
export const scaleOf = (v: ViewportState): number => v.pxPerMm * v.zoom

export function toScreen(v: ViewportState, p: Point): Point {
  const s = scaleOf(v)
  return { x: (p.x - v.cx) * s + v.width / 2, y: (p.y - v.cy) * s + v.height / 2 }
}

export function toWorld(v: ViewportState, p: Point): Point {
  const s = scaleOf(v)
  return { x: (p.x - v.width / 2) / s + v.cx, y: (p.y - v.height / 2) / s + v.cy }
}

const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z))

/** Zooms by `factor`, keeping the work-table point under `screen` in place. */
export function zoomAt(v: ViewportState, screen: Point, factor: number): ViewportState {
  const anchor = toWorld(v, screen)
  const zoom = clampZoom(v.zoom * factor)
  const s = v.pxPerMm * zoom
  return {
    ...v,
    zoom,
    cx: anchor.x - (screen.x - v.width / 2) / s,
    cy: anchor.y - (screen.y - v.height / 2) / s,
  }
}

/** Moves the view so content follows a drag of (dx, dy) screen px. */
export function panBy(v: ViewportState, dx: number, dy: number): ViewportState {
  const s = scaleOf(v)
  return { ...v, cx: v.cx - dx / s, cy: v.cy - dy / s }
}

/** Shows all of `r`, centred, with `padding` CSS px around it. */
export function fit(v: ViewportState, r: Rect, padding = 48): ViewportState {
  const w = Math.max(r.x1 - r.x0, 1)
  const h = Math.max(r.y1 - r.y0, 1)
  const availW = Math.max(v.width - 2 * padding, 1)
  const availH = Math.max(v.height - 2 * padding, 1)
  const zoom = clampZoom(Math.min(availW / w, availH / h) / v.pxPerMm)
  return { ...v, zoom, cx: (r.x0 + r.x1) / 2, cy: (r.y0 + r.y1) / 2 }
}

/** Shows true size (zoom 1) around the current centre. */
export function actualSize(v: ViewportState): ViewportState {
  return { ...v, zoom: 1 }
}

/** The part of the work table on screen. */
export function visibleWorld(v: ViewportState): Rect {
  const a = toWorld(v, { x: 0, y: 0 })
  const b = toWorld(v, { x: v.width, y: v.height })
  return { x0: a.x, y0: a.y, x1: b.x, y1: b.y }
}
