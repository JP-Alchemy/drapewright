// Projector mode: the pieces projected true size onto fabric on the
// cutting table, from a projector above it. The projector is calibrated
// first: four corners dragged onto a rectangle marked on the mat, which
// gives the scale and straightens a projector that is not square on. Kept
// for this device and this screen size.

import { useEffect, useRef, useState } from 'react'
import { Button, Group, Input, Label, NumberField, Switch } from 'react-aria-components'
import {
  type Homography,
  type Pt,
  invert,
  mapPoint,
  solveHomography,
} from '../canvas/homography.ts'
import { drawCalibration, drawProjection } from '../canvas/projector.ts'
import { sceneBounds } from '../canvas/renderer.ts'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'

/** A rectangle on the mat (mm), and where its corners show on the screen. */
interface Calibration {
  width: number
  height: number
  /** Top left, top right, bottom right, bottom left: CSS px. */
  corners: [Pt, Pt, Pt, Pt]
  /** The screen it was made on, CSS px: another must be calibrated again. */
  screen: [number, number]
}

const STORAGE = 'drapewright.projector'
/** A corner within this of a press is the one dragged, CSS px. */
const GRAB = 48

function loadCalibration(width: number, height: number): Calibration | null {
  try {
    const c = JSON.parse(localStorage.getItem(STORAGE) ?? 'null') as Calibration | null
    return c && c.screen[0] === width && c.screen[1] === height ? c : null
  } catch {
    return null
  }
}

function saveCalibration(c: Calibration) {
  try {
    localStorage.setItem(STORAGE, JSON.stringify(c))
  } catch {
    // Not kept (private browsing): calibrated again next time.
  }
}

/** A rectangle `width` by `height` mm, square on in the middle of the screen. */
function startingCalibration(screen: [number, number], width = 400, height = 300): Calibration {
  const s = Math.min((screen[0] * 0.7) / width, (screen[1] * 0.7) / height)
  const [x0, y0] = [(screen[0] - width * s) / 2, (screen[1] - height * s) / 2]
  const [x1, y1] = [x0 + width * s, y0 + height * s]
  return {
    width,
    height,
    corners: [
      [x0, y0],
      [x1, y0],
      [x1, y1],
      [x0, y1],
    ],
    screen,
  }
}

/** Mat mm to the screen, by the calibration. */
function mapOf(c: Calibration): Homography | null {
  return solveHomography(
    [
      [0, 0],
      [c.width, 0],
      [c.width, c.height],
      [0, c.height],
    ],
    c.corners,
  )
}

/** Drags on the projection: a corner being calibrated, or the pattern being slid. */
type Drag =
  { kind: 'corner'; index: number } | { kind: 'slide'; from: Pt; offset: Pt; back: Homography }

export function ProjectorView({ session }: { session: Session }) {
  const { revision } = useEngineState(session)
  const host = useRef<HTMLDivElement>(null)
  const canvas = useRef<HTMLCanvasElement>(null)
  const screen = (): [number, number] => [window.innerWidth, window.innerHeight]
  const [size, setSize] = useState(screen)
  const [cal, setCal] = useState<Calibration>(
    () => loadCalibration(...screen()) ?? startingCalibration(screen()),
  )
  const [calibrating, setCalibrating] = useState(() => loadCalibration(...screen()) === null)
  // The table point at the rectangle's top left corner: the pieces' top left.
  const [offset, setOffset] = useState<Pt>(() => {
    const b = sceneBounds(session.scene())
    return b ? [b.x0, b.y0] : [0, 0]
  })
  const [stitching, setStitching] = useState(true)
  const drag = useRef<Drag | null>(null)
  const map = mapOf(cal)

  // Full screen, as a projector shows it; leaving full screen closes it.
  useEffect(() => {
    const el = host.current
    let entered = false
    el?.requestFullscreen?.().then(
      () => (entered = true),
      () => {},
    )
    const onChange = () => {
      if (entered && !document.fullscreenElement) session.showProjector(false)
    }
    // A screen of another size: its own calibration, or calibrated again.
    const onResize = () => {
      const now = screen()
      setSize(now)
      const kept = loadCalibration(...now)
      setCal((c) =>
        c.screen[0] === now[0] && c.screen[1] === now[1]
          ? c
          : (kept ?? startingCalibration(now, c.width, c.height)),
      )
      if (!kept) setCalibrating(true)
    }
    const onKey = (e: KeyboardEvent) => e.key === 'Escape' && session.showProjector(false)
    document.addEventListener('fullscreenchange', onChange)
    window.addEventListener('resize', onResize)
    window.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('fullscreenchange', onChange)
      window.removeEventListener('resize', onResize)
      window.removeEventListener('keydown', onKey)
      if (document.fullscreenElement) void document.exitFullscreen().catch(() => {})
    }
  }, [session])

  useEffect(() => {
    const c = canvas.current
    const ctx = c?.getContext('2d')
    if (!c || !ctx || !map) return
    const dpr = window.devicePixelRatio || 1
    c.width = Math.round(size[0] * dpr)
    c.height = Math.round(size[1] * dpr)
    const box = { width: size[0], height: size[1], dpr }
    if (calibrating) drawCalibration(ctx, box, map, cal, cal.corners)
    else drawProjection(ctx, box, session.scene(), { map, offset }, { stitching })
  }, [size, cal, map, calibrating, offset, stitching, revision, session])

  const point = (e: React.PointerEvent): Pt => [e.clientX, e.clientY]
  const down = (e: React.PointerEvent) => {
    const p = point(e)
    if (calibrating) {
      const nearest = cal.corners
        .map((c, index) => ({ index, d: Math.hypot(c[0] - p[0], c[1] - p[1]) }))
        .sort((a, b) => a.d - b.d)[0]!
      if (nearest.d > GRAB) return
      drag.current = { kind: 'corner', index: nearest.index }
    } else {
      const back = map && invert(map)
      if (!back) return
      drag.current = { kind: 'slide', from: mapPoint(back, p), offset, back }
    }
    e.currentTarget.setPointerCapture(e.pointerId)
  }
  const move = (e: React.PointerEvent) => {
    const d = drag.current
    if (!d) return
    const p = point(e)
    if (d.kind === 'corner') {
      const corners = [...cal.corners] as Calibration['corners']
      corners[d.index] = p
      if (mapOf({ ...cal, corners })) setCal({ ...cal, corners })
    } else {
      // The table point first pressed stays under the finger.
      const at = mapPoint(d.back, p)
      setOffset([d.offset[0] + d.from[0] - at[0], d.offset[1] + d.from[1] - at[1]])
    }
  }
  const up = () => {
    if (drag.current?.kind === 'corner') saveCalibration(cal)
    drag.current = null
  }
  const text = t.projector
  const resize = (width: number, height: number) => {
    if (!(width > 0 && height > 0)) return
    const next = { ...cal, width, height }
    setCal(next)
    saveCalibration(next)
  }
  return (
    <div className="projector" ref={host} role="dialog" aria-label={text.heading}>
      <canvas
        ref={canvas}
        className="projector-canvas"
        role="img"
        aria-label={calibrating ? text.calibrationLabel : text.projectionLabel}
        onPointerDown={down}
        onPointerMove={move}
        onPointerUp={up}
        onPointerCancel={up}
      />
      {/* Calibrating, in the middle, clear of the corners near the edges. */}
      <div className={calibrating ? 'projector-bar centred' : 'projector-bar'}>
        {calibrating ? (
          <>
            <strong>{text.calibrate}</strong>
            <p>{text.calibrateHow}</p>
            <div className="projector-size">
              <SizeField
                label={text.width}
                value={cal.width / 10}
                onChange={(cm) => resize(cm * 10, cal.height)}
              />
              <SizeField
                label={text.height}
                value={cal.height / 10}
                onChange={(cm) => resize(cal.width, cm * 10)}
              />
            </div>
            <div className="projector-actions">
              <Button className="button" onPress={() => session.showProjector(false)}>
                {text.close}
              </Button>
              <Button
                className="button primary"
                onPress={() => {
                  saveCalibration(cal)
                  setCalibrating(false)
                }}
              >
                {text.done}
              </Button>
            </div>
          </>
        ) : (
          <>
            <p>{text.slide}</p>
            <Switch className="switch" isSelected={stitching} onChange={setStitching}>
              <span className="switch-track" aria-hidden="true" />
              {text.stitching}
            </Switch>
            <div className="projector-actions">
              <Button className="button" onPress={() => setCalibrating(true)}>
                {text.recalibrate}
              </Button>
              <Button className="button" onPress={() => session.showProjector(false)}>
                {text.close}
              </Button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}

/** A side of the calibration rectangle, cm. */
function SizeField({
  label,
  value,
  onChange,
}: {
  label: string
  value: number
  onChange(cm: number): void
}) {
  return (
    <NumberField
      className="field"
      value={value}
      minValue={5}
      maxValue={300}
      formatOptions={{ maximumFractionDigits: 1 }}
      onChange={(v) => Number.isFinite(v) && onChange(v)}
    >
      <Label>{label}</Label>
      <Group className="number-group with-unit">
        <Input className="number-input" />
        <span className="number-unit">cm</span>
      </Group>
    </NumberField>
  )
}
