// Projector mode: the pieces projected true size onto fabric on the
// cutting table, from a projector above it, as they lie on the table or
// as the cutting layout lays them on the fabric, all of them or one at a
// time. The projector is calibrated first: four corners dragged onto a
// rectangle marked on the mat, which gives the scale and straightens a
// projector that is not square on. Kept on this device for each screen it
// is made on, and used again whenever the projector opens there. Corners
// and the pattern can be moved a step at a time, from buttons or the arrow
// keys, where a projector shows a finger's drag late.

import type { Layout } from '@drapewright/engine'
import { ArrowDown, ArrowLeft, ArrowRight, ArrowUp, ChevronLeft, ChevronRight } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Group,
  Input,
  Label,
  NumberField,
  Radio,
  RadioGroup,
  Switch,
} from 'react-aria-components'
import {
  type Homography,
  type Pt,
  invert,
  mapPoint,
  solveHomography,
} from '../canvas/homography.ts'
import { drawCalibration, drawLayout, drawProjection } from '../canvas/projector.ts'
import { type ScenePiece, sceneBounds } from '../canvas/renderer.ts'
import { remember, remembered } from '../platform/remember.ts'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { keptChoice, mainMaterial, widthName } from './LayoutDialog.tsx'
import { isTyping } from './shortcuts.ts'

/** A rectangle on the mat (mm), and where its corners show on the screen. */
interface Calibration {
  width: number
  height: number
  /**
   * Top left, top right, bottom right, bottom left: CSS px on the screen
   * (not the window), so that it holds in full screen or not.
   */
  corners: [Pt, Pt, Pt, Pt]
  /** The screen it was made on, CSS px: another must be calibrated again. */
  screen: [number, number]
}

/** The calibration last made (before each screen kept its own). */
const STORAGE = 'drapewright.projector'
/** Each screen's calibration, by its size ("1920x1080"). */
const CALIBRATIONS = 'drapewright.projector.calibrations'
/** How far a step moves a corner or the pattern, mm. */
const STEP = 'drapewright.projector.step'
const STEPS = [1, 5, 10] as const
/** What is projected, and how. */
const SHOWS = 'drapewright.projector.shows'
const THICK = 'drapewright.projector.thick'

type Shows = 'table' | 'layout'

/** A piece's extent on the table, mm. */
function onTable(sp: ScenePiece): [number, number, number, number] {
  const { x, y, rotation } = sp.piece.placement
  const m = new DOMMatrix().translateSelf(x, y).rotateSelf(rotation)
  const [x0, y0, x1, y1] = sp.view.outlineBounds
  const corners = [
    [x0, y0],
    [x1, y0],
    [x1, y1],
    [x0, y1],
  ].map(([px, py]) => m.transformPoint(new DOMPoint(px, py)))
  const xs = corners.map((p) => p.x)
  const ys = corners.map((p) => p.y)
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]
}

/** A laid out piece's extent on the fabric, mm. */
function onFabric(layout: Layout, k: number): [number, number, number, number] {
  const points = layout.placed[k]!.rings.flat()
  const xs = points.map((p) => p[0])
  const ys = points.map((p) => p[1])
  return [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)]
}
/** A corner within this of a press is the one dragged, CSS px. */
const GRAB = 48

/** The screen's size, CSS px. */
const display = (): [number, number] => [window.screen.width, window.screen.height]

/**
 * Where the page's top left corner is on the screen, CSS px: nowhere but
 * the corner in full screen; in a window, past the browser's frame and its
 * bar along the top (outer less inner size: the frame as wide all round).
 */
function pageOrigin(): Pt {
  if (document.fullscreenElement) return [0, 0]
  const side = Math.max(0, (window.outerWidth - window.innerWidth) / 2)
  const top = Math.max(0, window.outerHeight - window.innerHeight - side)
  return [window.screenX + side, window.screenY + top]
}

/** The calibration's corners where they are on the page, its top left at `origin`. */
function onPage(c: Calibration, origin: Pt): Calibration {
  const at = ([x, y]: Pt): Pt => [x - origin[0], y - origin[1]]
  return { ...c, corners: c.corners.map(at) as Calibration['corners'] }
}

const screenKey = (width: number, height: number) => `${width}x${height}`

function savedCalibrations(): Record<string, Calibration> {
  try {
    return (JSON.parse(localStorage.getItem(CALIBRATIONS) ?? 'null') ?? {}) as Record<
      string,
      Calibration
    >
  } catch {
    return {}
  }
}

/** The calibration kept for a screen of this size, if any. */
function loadCalibration(width: number, height: number): Calibration | null {
  const kept = savedCalibrations()[screenKey(width, height)]
  if (kept) return kept
  try {
    const c = JSON.parse(localStorage.getItem(STORAGE) ?? 'null') as Calibration | null
    return c && c.screen[0] === width && c.screen[1] === height ? c : null
  } catch {
    return null
  }
}

/** Keeps a calibration for its screen, beside those of other screens. */
function saveCalibration(c: Calibration) {
  try {
    const all = savedCalibrations()
    all[screenKey(...c.screen)] = c
    localStorage.setItem(CALIBRATIONS, JSON.stringify(all))
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

/** A rectangle in the middle of the page to start from, its corners on the screen. */
function startingOnScreen(width?: number, height?: number): Calibration {
  const o = pageOrigin()
  const c = startingCalibration([window.innerWidth, window.innerHeight], width, height)
  return { ...onPage(c, [-o[0], -o[1]]), screen: display() }
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
  const page = (): [number, number] => [window.innerWidth, window.innerHeight]
  const [size, setSize] = useState(page)
  const [origin, setOrigin] = useState(pageOrigin)
  const [cal, setCal] = useState<Calibration>(
    () => loadCalibration(...display()) ?? startingOnScreen(),
  )
  const [calibrating, setCalibrating] = useState(() => loadCalibration(...display()) === null)
  // The calibration as it is on this page, wherever the window is.
  const shown = onPage(cal, origin)
  // The table point at the rectangle's top left corner: the pieces' top left.
  const [offset, setOffset] = useState<Pt>(() => {
    const b = sceneBounds(session.scene())
    return b ? [b.x0, b.y0] : [0, 0]
  })
  const [stitching, setStitching] = useState(true)
  const [thick, setThick] = useState(() => remembered(THICK, false))
  const [shows, setShows] = useState<Shows>(() =>
    remembered<Shows>(SHOWS, 'table') === 'layout' ? 'layout' : 'table',
  )
  const [layout, setLayout] = useState<Layout | null>(null)
  // The piece being cut, picked out of the rest; none: all of them.
  const [focus, setFocus] = useState<number | null>(null)
  const drag = useRef<Drag | null>(null)
  const map = mapOf(shown)
  // The corner the step buttons move: the last dragged or chosen.
  const [chosen, setChosen] = useState(0)
  const [stepMm, setStepMm] = useState<number>(() => {
    const kept = remembered<number>(STEP, 1)
    return (STEPS as readonly number[]).includes(kept) ? kept : 1
  })
  /**
   * Steps (`dx`, `dy`: how many, each way) of the chosen corner, while
   * calibrating, or of the pattern on the fabric.
   */
  const nudge = (dx: number, dy: number) => {
    if (calibrating) {
      // The rectangle's scale on the screen, px a mm, along its top and side.
      const [a, b, , d] = cal.corners
      const across = Math.hypot(b[0] - a[0], b[1] - a[1]) / cal.width
      const down = Math.hypot(d[0] - a[0], d[1] - a[1]) / cal.height
      const px = (stepMm * (across + down)) / 2
      const corners = [...cal.corners] as Calibration['corners']
      const [x, y] = corners[chosen]!
      corners[chosen] = [x + dx * px, y + dy * px]
      const next = { ...cal, corners }
      if (!mapOf(next)) return
      setCal(next)
      saveCalibration(next)
    } else {
      setOffset(([x, y]) => [x - dx * stepMm, y - dy * stepMm])
    }
  }
  // The arrow keys step it too, from the newest `nudge`.
  const nudgeRef = useRef(nudge)
  useEffect(() => {
    nudgeRef.current = nudge
  })

  // Full screen, as a projector shows it. Leaving full screen leaves the
  // projector open, to go back into it: Safari leaves it as soon as a
  // measurement is typed, and an iPad as its keyboard comes up.
  const [full, setFull] = useState(false)
  const fullScreen = () =>
    host.current?.requestFullscreen?.().then(
      () => setFull(true),
      () => {},
    )
  useEffect(() => {
    host.current?.requestFullscreen?.().then(
      () => setFull(true),
      () => {},
    )
    const onChange = () => {
      setFull(document.fullscreenElement !== null)
      setOrigin(pageOrigin())
    }
    // The window resized (in or out of full screen) or moved: the
    // calibration holds, on the screen. Another screen: its own
    // calibration, or calibrated again.
    const onResize = () => {
      setSize([window.innerWidth, window.innerHeight])
      setOrigin(pageOrigin())
      const now = display()
      setCal((c) => {
        if (c.screen[0] === now[0] && c.screen[1] === now[1]) return c
        const kept = loadCalibration(...now)
        if (!kept) setCalibrating(true)
        return kept ?? startingOnScreen(c.width, c.height)
      })
    }
    // A window moved fires nothing: looked for now and then.
    const moved = window.setInterval(() => {
      const o = pageOrigin()
      setOrigin((was) => (was[0] === o[0] && was[1] === o[1] ? was : o))
    }, 500)
    // Escape closes it, but not while a measurement is being typed; the
    // arrow keys step the corner or the pattern (with Shift, ten steps).
    const arrows: Record<string, Pt> = {
      ArrowLeft: [-1, 0],
      ArrowRight: [1, 0],
      ArrowUp: [0, -1],
      ArrowDown: [0, 1],
    }
    const onKey = (e: KeyboardEvent) => {
      if (isTyping(e.target)) return
      if (e.key === 'Escape') session.showProjector(false)
      const by = arrows[e.key]
      if (!by) return
      e.preventDefault()
      const times = e.shiftKey ? 10 : 1
      nudgeRef.current(by[0] * times, by[1] * times)
    }
    document.addEventListener('fullscreenchange', onChange)
    window.addEventListener('resize', onResize)
    window.addEventListener('keydown', onKey)
    return () => {
      document.removeEventListener('fullscreenchange', onChange)
      window.removeEventListener('resize', onResize)
      window.clearInterval(moved)
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
    const look = { stitching, thick, focus }
    if (calibrating) drawCalibration(ctx, box, map, shown, shown.corners, chosen)
    else if (shows === 'layout' && layout)
      drawLayout(ctx, box, layout, { map, offset }, look, t.layout.faceDown)
    else drawProjection(ctx, box, session.scene(), { map, offset }, look)
  }, [
    size,
    shown,
    map,
    calibrating,
    chosen,
    offset,
    stitching,
    thick,
    focus,
    shows,
    layout,
    revision,
    session,
  ])

  // The cutting layout, on the fabric last chosen for it, as the pattern
  // changes.
  useEffect(() => {
    if (shows !== 'layout') return
    let current = true
    const doc = session.engine.document()
    const choice = keptChoice(doc.settings.displayUnits)
    const materials = [...new Set(doc.pieces.flatMap((p) => p.cut.map((c) => c.material)))]
    void session.io
      .call('layOut', { pieces: doc.pieces, material: mainMaterial(materials), ...choice })
      .then(
        (l) => current && setLayout(l),
        () => current && setLayout(null),
      )
    return () => {
      current = false
    }
  }, [shows, revision, session])

  const count = shows === 'layout' ? (layout?.placed.length ?? 0) : session.scene().length
  const extent = (k: number) =>
    shows === 'layout' && layout ? onFabric(layout, k) : onTable(session.scene()[k]!)
  /** Picks out the next (or previous) piece, slid to the middle of the rectangle. */
  const step = (by: number) => {
    if (count === 0) return
    const k = focus === null ? (by > 0 ? 0 : count - 1) : (focus + by + count) % count
    setFocus(k)
    const [x0, y0, x1, y1] = extent(k)
    setOffset([(x0 + x1) / 2 - cal.width / 2, (y0 + y1) / 2 - cal.height / 2])
  }
  const showAs = (next: Shows) => {
    setShows(next)
    remember(SHOWS, next)
    setFocus(null)
    // The fabric's corner a little in from the rectangle's; or the pieces'.
    const b = sceneBounds(session.scene())
    setOffset(next === 'layout' ? [-50, -50] : b ? [b.x0, b.y0] : [0, 0])
  }
  const focusName =
    focus === null
      ? null
      : shows === 'layout'
        ? layout?.placed[focus]?.name
        : session.scene()[focus]?.piece.name

  const point = (e: React.PointerEvent): Pt => [e.clientX, e.clientY]
  const down = (e: React.PointerEvent) => {
    const p = point(e)
    if (calibrating) {
      const nearest = shown.corners
        .map((c, index) => ({ index, d: Math.hypot(c[0] - p[0], c[1] - p[1]) }))
        .sort((a, b) => a.d - b.d)[0]!
      if (nearest.d > GRAB) return
      drag.current = { kind: 'corner', index: nearest.index }
      setChosen(nearest.index)
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
      // Kept on the screen: where it is on the page, plus where the page is.
      const corners = [...cal.corners] as Calibration['corners']
      corners[d.index] = [p[0] + origin[0], p[1] + origin[1]]
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
            <RadioGroup
              className="segmented"
              orientation="horizontal"
              aria-label={text.corner}
              value={String(chosen)}
              onChange={(v) => setChosen(Number(v))}
            >
              <Label className="group-label">{text.corner}</Label>
              <div className="segments">
                {text.corners.map((name, k) => (
                  <Radio key={name} value={String(k)} className="segment">
                    {name}
                  </Radio>
                ))}
              </div>
            </RadioGroup>
            <StepPad
              label={text.moveCorner(text.corners[chosen]!)}
              step={stepMm}
              onStep={(mm) => {
                setStepMm(mm)
                remember(STEP, mm)
              }}
              onNudge={nudge}
            />
            <div className="projector-actions">
              <Button className="button" onPress={() => session.showProjector(false)}>
                {text.close}
              </Button>
              {!full && (
                <Button className="button" onPress={() => void fullScreen()}>
                  {text.fullScreen}
                </Button>
              )}
              <Button
                className="button primary"
                onPress={() => {
                  saveCalibration(cal)
                  setCalibrating(false)
                  session.toast(text.savedNow)
                }}
              >
                {text.done}
              </Button>
            </div>
          </>
        ) : (
          <>
            <p>{text.slide}</p>
            <RadioGroup
              className="segmented"
              orientation="horizontal"
              aria-label={text.shows}
              value={shows}
              onChange={(v) => showAs(v as Shows)}
            >
              <div className="segments">
                <Radio value="table" className="segment">
                  {text.asOnTable}
                </Radio>
                <Radio value="layout" className="segment">
                  {text.asLaidOut}
                </Radio>
              </div>
            </RadioGroup>
            {shows === 'layout' && layout && (
              <p>
                {text.layFabric(
                  layout.folded,
                  widthName(layout.width, session.engine.document().settings.displayUnits),
                )}
              </p>
            )}
            <div className="projector-pieces" role="group" aria-label={text.pieces}>
              <Button className="icon-button" aria-label={text.previous} onPress={() => step(-1)}>
                <ChevronLeft size={20} aria-hidden="true" />
              </Button>
              <span className="projector-piece" aria-live="polite">
                {focusName && focus !== null
                  ? text.onePiece(focusName, focus + 1, count)
                  : text.allPieces}
              </span>
              <Button className="icon-button" aria-label={text.next} onPress={() => step(1)}>
                <ChevronRight size={20} aria-hidden="true" />
              </Button>
              {focus !== null && (
                <Button className="button" onPress={() => setFocus(null)}>
                  {text.showAll}
                </Button>
              )}
            </div>
            <StepPad
              label={text.movePattern}
              step={stepMm}
              onStep={(mm) => {
                setStepMm(mm)
                remember(STEP, mm)
              }}
              onNudge={nudge}
            />
            <p className="small">{text.saved}</p>
            {shows === 'table' && (
              <Switch className="switch" isSelected={stitching} onChange={setStitching}>
                <span className="switch-track" aria-hidden="true" />
                {text.stitching}
              </Switch>
            )}
            <Switch
              className="switch"
              isSelected={thick}
              onChange={(on) => {
                setThick(on)
                remember(THICK, on)
              }}
            >
              <span className="switch-track" aria-hidden="true" />
              {text.thick}
            </Switch>
            <div className="projector-actions">
              <Button className="button" onPress={() => setCalibrating(true)}>
                {text.recalibrate}
              </Button>
              {!full && (
                <Button className="button" onPress={() => void fullScreen()}>
                  {text.fullScreen}
                </Button>
              )}
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

/**
 * Buttons that move something a step at a time (the arrow keys do too),
 * and how big a step is: for a projector that shows a drag late.
 */
function StepPad({
  label,
  step,
  onStep,
  onNudge,
}: {
  label: string
  step: number
  onStep(mm: number): void
  onNudge(dx: number, dy: number): void
}) {
  const text = t.projector
  const arrow = (name: string, dx: number, dy: number, icon: React.ReactNode) => (
    <Button
      className="icon-button"
      aria-label={name}
      onPress={() => onNudge(dx, dy)}
      style={{ gridColumn: 2 + dx, gridRow: 2 + dy }}
    >
      {icon}
    </Button>
  )
  return (
    <div className="projector-steps" role="group" aria-label={label}>
      <span className="group-label">{label}</span>
      <div className="step-pad">
        {arrow(text.up, 0, -1, <ArrowUp size={20} aria-hidden="true" />)}
        {arrow(text.left, -1, 0, <ArrowLeft size={20} aria-hidden="true" />)}
        {arrow(text.right, 1, 0, <ArrowRight size={20} aria-hidden="true" />)}
        {arrow(text.down, 0, 1, <ArrowDown size={20} aria-hidden="true" />)}
      </div>
      <RadioGroup
        className="segmented"
        orientation="horizontal"
        aria-label={text.stepSize}
        value={String(step)}
        onChange={(v) => onStep(Number(v))}
      >
        <div className="segments">
          {STEPS.map((mm) => (
            <Radio key={mm} value={String(mm)} className="segment">
              {text.stepOf(mm)}
            </Radio>
          ))}
        </div>
      </RadioGroup>
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
