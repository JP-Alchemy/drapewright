import { type CornerStyle, type Piece, previewAllowance } from '@soom/engine'
import { useMemo } from 'react'
import { Label, Radio, RadioGroup } from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'

const STYLES: CornerStyle[] = [
  'auto',
  'mitre',
  'squareThis',
  'squareNext',
  'mirrorThis',
  'mirrorNext',
]

type V = [number, number]

const sub = (a: V, b: V): V => [a[0] - b[0], a[1] - b[1]]
const len = (a: V) => Math.hypot(a[0], a[1])
const norm = (a: V): V => {
  const l = len(a)
  return l > 0 ? [a[0] / l, a[1] / l] : [0, 0]
}

/** Directions arriving at and leaving the corner at the end of edge `i`. */
function cornerFrame(piece: Piece, i: number): { corner: V; into: V; out: V } | null {
  const edges = piece.outline.edges
  const edge = edges[i]
  const next = edges[(i + 1) % edges.length]
  const last = edge?.segments.at(-1)
  const first = next?.segments[0]
  if (!edge || !next || !last || !first) return null
  const corner = last.to
  const prevPoint: V =
    edge.segments.length > 1
      ? edge.segments.at(-2)!.to
      : i === 0
        ? piece.outline.start
        : edges[i - 1]!.segments.at(-1)!.to
  const intoFrom = 'c2' in last ? (len(sub(corner, last.c2)) > 1e-9 ? last.c2 : last.c1) : prevPoint
  const outTo = 'c1' in first ? (len(sub(first.c1, corner)) > 1e-9 ? first.c1 : first.c2) : first.to
  return { corner, into: norm(sub(corner, intoFrom)), out: norm(sub(outTo, corner)) }
}

function withCorner(piece: Piece, i: number, style: CornerStyle): Piece {
  return {
    ...piece,
    outline: {
      ...piece.outline,
      edges: piece.outline.edges.map((e, k) => (k === i ? { ...e, endCorner: style } : e)),
    },
  }
}

interface Thumb {
  style: CornerStyle
  cutting: string
  stitching: string
  possible: boolean
  /** Gives the same cutting line as a pointed corner (to 0.1 mm). */
  likePointed: boolean
}

/** Whether two cutting lines are the same to within `tolerance` mm. */
function sameLine(a: number[], b: number[], tolerance = 0.1): boolean {
  return a.length === b.length && a.every((v, i) => Math.abs(v - b[i]!) <= tolerance)
}

/**
 * Pictures of this very corner with each shape, turned so the corner points
 * up: the edge arriving at the corner is always on the left.
 */
function thumbnails(
  piece: Piece,
  i: number,
  reach: number,
): { thumbs: Thumb[]; viewBox: string } | null {
  const frame = cornerFrame(piece, i)
  if (!frame) return null
  const { corner, into, out } = frame
  // Outward normals (clockwise outlines: left of travel on screen).
  const na: V = [into[1], -into[0]]
  const nb: V = [out[1], -out[0]]
  const bisector = norm([na[0] + nb[0], na[1] + nb[1]])
  const turn = -Math.PI / 2 - Math.atan2(bisector[1], bisector[0])
  const [c, s] = [Math.cos(turn), Math.sin(turn)]
  const place = (x: number, y: number): string => {
    const dx = x - corner[0]
    const dy = y - corner[1]
    return `${(c * dx - s * dy).toFixed(3)},${(s * dx + c * dy).toFixed(3)}`
  }
  const path = (flat: number[]) => {
    const pts: string[] = []
    for (let k = 0; k + 1 < flat.length; k += 2) pts.push(place(flat[k]!, flat[k + 1]!))
    return pts.length ? `M${pts.join('L')}Z` : ''
  }
  const edgeId = piece.outline.edges[i]!.id
  const previews = STYLES.map((style) => previewAllowance(withCorner(piece, i, style)))
  const pointed = previews[STYLES.indexOf('mitre')]!.cutting
  const thumbs = STYLES.map((style, k) => {
    const preview = previews[k]!
    return {
      style,
      cutting: path(preview.cutting),
      stitching: path(preview.stitching),
      possible: !preview.notes.some((n) => n.kind === 'cornerFallback' && n.edge === edgeId),
      likePointed: style !== 'mitre' && sameLine(preview.cutting, pointed),
    }
  })
  // Centre on the allowance just outside the corner, where the shapes differ.
  const r = reach
  const lift = r * 0.35
  return { thumbs, viewBox: `${-r} ${-lift - r} ${2 * r} ${2 * r}` }
}

export function CornerChooser({
  session,
  piece,
  edgeId,
  left,
  right,
}: {
  session: Session
  piece: Piece
  edgeId: string
  /** Names of the edges arriving at and leaving the corner. */
  left: string
  right: string
}) {
  const i = piece.outline.edges.findIndex((e) => e.id === edgeId)
  const view = session.scenePiece(piece.id)?.view
  const widths = view
    ? [view.edges[i]?.allowance ?? 0, view.edges[(i + 1) % view.edges.length]?.allowance ?? 0]
    : [15, 15]
  const reach = Math.max(10, 1.6 * Math.max(...widths))
  const result = useMemo(() => (i < 0 ? null : thumbnails(piece, i, reach)), [piece, i, reach])
  if (i < 0 || !result) return null
  const current = piece.outline.edges[i]!.endCorner

  return (
    <RadioGroup
      className="corner-choice"
      value={current}
      onChange={(style) =>
        session.dispatch({
          type: 'setCornerStyle',
          piece: piece.id,
          edge: edgeId,
          style: style as CornerStyle,
        })
      }
    >
      <Label className="group-label">{t.corner.heading}</Label>
      <div className="corner-grid">
        {result.thumbs.map((th) => (
          <Radio
            key={th.style}
            value={th.style}
            className="corner-card"
            data-impossible={!th.possible || undefined}
          >
            <svg viewBox={result.viewBox} className="corner-thumb" aria-hidden="true">
              <path d={th.cutting} className="thumb-cut" />
              <path d={th.stitching} className="thumb-stitch" />
            </svg>
            <span className="corner-name">{t.corner.style(th.style)}</span>
            {!th.possible ? (
              <span className="muted small">{t.corner.notHere}</span>
            ) : (
              th.likePointed && <span className="muted small">{t.corner.likePointed}</span>
            )}
          </Radio>
        ))}
      </div>
      <p className="muted small">{t.corner.sides(left, right)}</p>
    </RadioGroup>
  )
}
