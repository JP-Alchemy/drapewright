import type {
  AvatarPose,
  AvatarSettings,
  ClothReading,
  ClothShows,
  DisplayUnits,
  Document,
  DrapeRequest,
  FabricPreset,
  Measured,
  SeenEdge,
} from '@drapewright/engine'
import type { ViewColours } from '@drapewright/engine/avatar'
import {
  Camera,
  ChevronDown,
  Film,
  PanelBottom,
  PanelRight,
  PencilRuler,
  PersonStanding,
  PictureInPicture2,
  RotateCcw,
  X,
} from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Label,
  Header,
  ListBox,
  ListBoxItem,
  ListBoxSection,
  Popover,
  Radio,
  RadioGroup,
  Switch,
  Select,
  SelectValue,
  ToggleButton,
} from 'react-aria-components'
import type { Modifiers } from '@drapewright/input'
import { attachOrbitInput } from '../canvas/orbit-input.ts'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { formatLength, t } from '../strings/en.ts'
import { sim } from '../workers/sim-client.ts'
import { type DrapeUpdate, SimError } from '../workers/sim-protocol.ts'
import { CLOSE_MM, STANDARD_FIGURE, avatarRequest } from './avatar.ts'
import { DockEdge, useDocksBelow, useFloatingBox } from './AvatarFrame.tsx'
import { setAvatarLayout, useAvatarLayout } from './avatar-layout.ts'
import { FabricDialog } from './FabricDialog.tsx'
import { FitAdvice } from './FitAdvice.tsx'
import { chooseOnFigure, useFigureEditing } from './figure-editing.ts'
import { canRecord, video } from './turnaround.ts'
import { fileName, saveBlob } from '../platform/save-file.ts'

/** "rgb(239, 233, 223)" as sRGB parts, 0 to 1. */
function rgb(css: string): [number, number, number] {
  const [r = 0, g = 0, b = 0] = css.match(/[\d.]+/g)?.map(Number) ?? []
  return [r / 255, g / 255, b / 255]
}

/** The view's colours, from the page's theme: the cloth's from a swatch in it. */
function colours(el: HTMLElement): ViewColours {
  const style = getComputedStyle(el)
  const swatch = el.querySelector('.avatar-cloth-swatch')
  return {
    background: rgb(style.backgroundColor),
    figure: rgb(style.color),
    cloth: rgb(swatch ? getComputedStyle(swatch).color : 'rgb(91, 120, 150)'),
  }
}

/** The fabrics, in the order offered. */
const FABRICS = Object.keys(t.avatar.fabrics) as FabricPreset[]

/** What the cloth can be shown as, in the order offered. */
const SHOWS = Object.keys(t.avatar.showsAs) as ClothShows[]
/** The poses, in the order offered. */
const POSES = Object.keys(t.avatar.poses) as AvatarPose[]
/** The heatmaps' scales, as the view draws them (crates/wasm-3d/src/drape.rs). */
const PULL_SCALE = 100
const ROOM_SCALE = 40

/** What the cloth is like where it was tapped. */
function readingText(reading: ClothReading, units: DisplayUnits): string {
  const text = t.avatar
  const stretch = `${(reading.stretch * 100).toFixed(1)}%`
  const room = reading.room < 1 ? null : formatLength(reading.room, units)
  return text.reading(text.pulled(reading.pull), Math.round(reading.pull), stretch, room)
}

type Status =
  | { kind: 'starting' }
  | { kind: 'fitting' }
  | { kind: 'standard' }
  | { kind: 'fitted'; close: number; of: number }
  | { kind: 'failed'; text: string }

/**
 * The garment on the avatar: off, being draped, draped (in `fabric`), or
 * draped and being draped again out of sight to follow a change.
 */
type Drape =
  | { kind: 'off' }
  | { kind: 'draping'; fabric: string; update: DrapeUpdate | null }
  | { kind: 'draped'; fabric: string; update: DrapeUpdate }
  | { kind: 'following'; fabric: string; update: DrapeUpdate }
  | { kind: 'failed'; text: string }

/** How long after the pattern last changed a garment on the avatar follows it, ms. */
const FOLLOW_AFTER = 400

/** The fabric chosen (ready made, or one's own by id): what to drape in, and what it is called. */
function fabricOf(
  doc: Document,
  chosen: string,
): { preset: FabricPreset; measured?: Measured; name: string } {
  const own = doc.fabrics.find((f) => f.id === chosen)
  if (own) return { preset: own.like, measured: own.measured, name: own.name }
  const preset = (chosen in t.avatar.fabrics ? chosen : 'poplin') as FabricPreset
  return { preset, name: t.avatar.fabrics[preset].toLowerCase() }
}

/**
 * What the garment on the avatar is made of: its pieces' shapes and
 * sewing, and the fabric `chosen`.
 */
function garmentKey(doc: Document, chosen: string): string {
  const { preset, measured } = fabricOf(doc, chosen)
  return JSON.stringify([
    doc.pieces.map((p) => [p.id, p.name, p.outline, p.allowance, p.cut, p.grainline, p.darts]),
    doc.seams.filter((s) => s.confirmed),
    preset,
    measured,
  ])
}

/** The fabric list's actions: measuring a fabric, or changing the one chosen. */
const MEASURE = 'measure'
const CHANGE = 'change'

function failure(error: unknown): string {
  if (error instanceof SimError && error.failure === 'noBodyFile')
    return t.measure.avatar.noBodyFile
  if (error instanceof SimError && error.failure === 'noView') return t.avatar.noView
  if (error instanceof SimError && error.failure === 'cannotDrape')
    return t.avatar.cannotDrape(error.message)
  return t.measure.avatar.failed(error instanceof Error ? error.message : String(error))
}

/**
 * How big the garment on the figure is round its bust, waist and hip, and
 * the ease there: what a sewist measures of a finished garment.
 */
function garmentText(drape: Drape, units: DisplayUnits): string {
  if (drape.kind !== 'draping' && drape.kind !== 'draped') return ''
  const text = t.avatar
  const parts = (drape.update?.girths ?? []).flatMap((g) => {
    if (g.garment === null) return []
    const ease = g.garment - g.body
    const by = formatLength(Math.abs(ease), units)
    return [
      text.girthAt(
        text.levels[g.level] ?? g.level,
        formatLength(g.garment, units),
        Math.abs(ease) < 1 ? text.noEase : ease > 0 ? text.easeMore(by) : text.easeLess(by),
      ),
    ]
  })
  return parts.length > 0 ? text.garmentSize(parts) : ''
}

/**
 * What the drape bar says of the garment on the avatar; `named`, the
 * pieces' names by id.
 */
function drapeText(drape: Drape, named: (ids: string[]) => string[]): string {
  const text = t.avatar
  switch (drape.kind) {
    case 'off':
      return ''
    case 'failed':
      return drape.text
    case 'following':
      return text.following
    case 'draping':
    case 'draped': {
      const update = drape.update
      const fabric = drape.fabric
      const said =
        drape.kind === 'draping'
          ? update?.stage === 'settling'
            ? text.settling
            : update?.stage === 'posing'
              ? text.posing
              : text.sewing
          : update?.stage === 'stopped'
            ? text.stillMoving(fabric)
            : text.draped(fabric)
      const left = update?.leftOut.length ?? 0
      const inside = update?.inside.length ?? 0
      const unplaced = update?.unplaced.length ?? 0
      const lined = named(update?.lined ?? [])
      return [
        said,
        left > 0 ? text.leftOut(left) : '',
        inside > 0 ? text.inside(inside) : '',
        unplaced > 0 ? text.unplaced(unplaced) : '',
        lined.length > 0 ? text.lined(lined) : '',
      ]
        .filter(Boolean)
        .join(' ')
    }
  }
}

/** The avatar in 3D, fitted to someone's measurements, beside the table. */
export function AvatarPane({ session }: { session: Session }) {
  const { revision } = useEngineState(session)
  const { avatarPerson, drapeFabric } = useStore(session.ui)
  const sets = session.engine.document().measurementSets
  const set = sets.find((s) => s.id === avatarPerson) ?? sets[0] ?? null
  const host = useRef<HTMLDivElement>(null)
  // Beside (or under) the table, or floating over it as a window.
  const pane = useRef<HTMLElement>(null)
  const { floating } = useAvatarLayout()
  const below = useDocksBelow()
  const frame = useFloatingBox(pane)
  const [backend, setBackend] = useState<string | null>(null)
  // The view could not start: the avatar is still fitted and reported.
  const [viewFailed, setViewFailed] = useState<string | null>(null)
  const [status, setStatus] = useState<Status>({ kind: 'starting' })
  // The figure in the view, to drape on.
  const shown = useRef<AvatarSettings | null>(null)
  const [drape, setDrape] = useState<Drape>({ kind: 'off' })
  // How the figure stands; any garment on it moves with it.
  const [pose, setPose] = useState<AvatarPose>('standing')
  const posed = useRef<AvatarPose>('standing')
  // Worn open, its buttons undone.
  const [open, setOpen] = useState(false)
  const opened = useRef(false)
  // Counts drapes asked for: an answer for one overtaken is dropped.
  const drapes = useRef(0)
  // The garment last draped, to follow changes to it.
  const draped = useRef<string | null>(null)
  const [shows, setShows] = useState<ClothShows>('fabric')
  // The cloth where it was last tapped.
  const [reading, setReading] = useState<ClothReading | null>(null)
  // Editing on the figure: the pieces' edges drawn over it, as last seen.
  const [editing, setEditing] = useState(false)
  const overlay = useRef<HTMLCanvasElement>(null)
  const seen = useRef<SeenEdge[] | null>(null)
  // Editing, an edge or a corner tapped on the figure is chosen: whether one was.
  const chooseEdge = useRef<(x: number, y: number, keys: Modifiers) => boolean>(() => false)
  useEffect(() => {
    chooseEdge.current = (x, y, keys) => chooseOnFigure(session, seen.current, { x, y }, keys)
  }, [session])
  // A fabric being measured, or the one chosen being changed.
  const [measuring, setMeasuring] = useState<typeof MEASURE | typeof CHANGE | null>(null)
  const units = session.engine.document().settings.displayUnits
  // A picture or a video being made of the view.
  const [saving, setSaving] = useState<'picture' | 'video' | null>(null)
  const keep = async (what: 'picture' | 'video') => {
    setSaving(what)
    const name = session.engine.document().meta.name
    try {
      if (what === 'picture') {
        const file = fileName(t.avatar.onTheFigure(name), 'png')
        saveBlob(await sim.picture(), file)
        session.toast(t.avatar.saved(file))
      } else {
        // Once round in three seconds.
        const made = await video(await sim.turnaround(72))
        const file = fileName(t.avatar.onTheFigure(name), made.extension)
        saveBlob(made.blob, file)
        session.toast(t.avatar.saved(file))
      }
    } catch (error) {
      session.notify({
        kind: 'error',
        text: t.avatar.notSaved(error instanceof Error ? error.message : String(error)),
      })
    } finally {
      setSaving(null)
    }
  }

  // The canvas, handed to the sim worker to draw on. Made here rather than
  // by React: a canvas can be handed over only once, and effects may run
  // twice.
  useEffect(() => {
    const el = host.current
    if (!el) return
    const canvas = document.createElement('canvas')
    canvas.className = 'avatar-canvas'
    el.append(canvas)
    const pixels = () => {
      const box = el.getBoundingClientRect()
      const ratio = window.devicePixelRatio || 1
      return [
        Math.max(1, Math.round(box.width * ratio)),
        Math.max(1, Math.round(box.height * ratio)),
      ] as const
    }
    ;[canvas.width, canvas.height] = pixels()
    let live = true
    sim.attachView(canvas.transferControlToOffscreen(), colours(el)).then(
      (drawnWith) => live && setBackend(drawnWith),
      (error: unknown) => live && setViewFailed(failure(error)),
    )
    const resize = new ResizeObserver(() => {
      const [width, height] = pixels()
      sim.note({ note: 'resize', width, height })
    })
    resize.observe(el)
    const scheme = matchMedia('(prefers-color-scheme: dark)')
    const onScheme = () => sim.note({ note: 'look', colours: colours(el) })
    scheme.addEventListener('change', onScheme)
    const detachInput = attachOrbitInput(el, {
      turn: (dx, dy) => sim.note({ note: 'turn', dx, dy }),
      zoom: (factor) => sim.note({ note: 'zoom', factor }),
      slide: (dx, dy) =>
        sim.note({ note: 'slide', dx, dy, pixelRatio: window.devicePixelRatio || 1 }),
      reset: () => sim.note({ note: 'reset' }),
      tap: (x, y, keys) => {
        if (chooseEdge.current(x, y, keys)) return
        sim.pick(x, y, window.devicePixelRatio || 1).then(
          (found) => live && setReading(found),
          () => live && setReading(null),
        )
      },
    })
    return () => {
      live = false
      resize.disconnect()
      scheme.removeEventListener('change', onScheme)
      detachInput()
      canvas.remove()
      void sim.detachView()
    }
  }, [])

  // Whose figure: fitted to their measurements, or the standard figure.
  const request = set ? avatarRequest(set) : null
  const measured = request !== null && Object.keys(request.values).length > 0
  const key = measured ? JSON.stringify(request) : 'standard'
  const ready = backend !== null || viewFailed !== null
  useEffect(() => {
    if (!ready) return
    let live = true
    const drawing = backend !== null
    const show = async () => {
      // A new figure: the garment comes off it.
      drapes.current++
      shown.current = null
      setDrape({ kind: 'off' })
      setReading(null)
      if (!measured) {
        if (drawing) await sim.showAvatar(STANDARD_FIGURE, posed.current)
        if (live) {
          shown.current = STANDARD_FIGURE
          setStatus({ kind: 'standard' })
        }
        return
      }
      setStatus({ kind: 'fitting' })
      const fit = await sim.fitAvatar(request)
      if (drawing) await sim.showAvatar(fit.settings, posed.current)
      if (live) shown.current = fit.settings
      const residuals = Object.values(fit.residuals)
      const close = residuals.filter((r) => Math.abs(r) <= CLOSE_MM).length
      if (live) setStatus({ kind: 'fitted', close, of: residuals.length })
    }
    show().catch((error: unknown) => live && setStatus({ kind: 'failed', text: failure(error) }))
    return () => {
      live = false
    }
    // `key` stands for the request.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready, backend, key])

  /**
   * Drapes the project's garment on the figure shown, in `fabric`. To
   * follow a change, `quietly`: the cloth shown stays until the new one has
   * settled.
   */
  const startDrape = (chosen: string, quietly = false) => {
    const settings = shown.current
    if (!settings) return
    const doc = session.engine.document()
    const mine = ++drapes.current
    draped.current = garmentKey(doc, chosen)
    const { preset, measured, name: fabric } = fabricOf(doc, chosen)
    if (!doc.seams.some((s) => s.confirmed)) {
      if (quietly) void sim.undrape()
      setDrape({ kind: 'failed', text: t.avatar.nothingSewn })
      return
    }
    const request: DrapeRequest = {
      pieces: doc.pieces,
      seams: doc.seams,
      fabric: preset,
      ...(measured ? { measured } : {}),
      settings,
      pose: posed.current,
      open: opened.current,
    }
    setDrape((was) =>
      quietly && (was.kind === 'draped' || was.kind === 'following')
        ? { kind: 'following', fabric, update: was.update }
        : { kind: 'draping', fabric, update: null },
    )
    setReading(null)
    sim
      .drape(
        request,
        (update) => {
          if (mine === drapes.current && !quietly) setDrape({ kind: 'draping', fabric, update })
        },
        quietly,
      )
      .then(
        (last) => {
          if (mine !== drapes.current) return
          setDrape(last ? { kind: 'draped', fabric, update: last } : { kind: 'off' })
        },
        (error: unknown) => {
          if (mine !== drapes.current) return
          // What was shown no longer fits the pattern: taken off.
          if (quietly) void sim.undrape()
          setDrape({ kind: 'failed', text: failure(error) })
        },
      )
  }
  // A garment on the avatar follows the pattern: changed, it is draped
  // again a moment after the last change.
  useEffect(() => {
    const following =
      drape.kind === 'draped' || drape.kind === 'following' || drape.kind === 'draping'
    if (!following || garmentKey(session.engine.document(), drapeFabric) === draped.current) return
    const timer = setTimeout(() => startDrape(drapeFabric, drape.kind !== 'draping'), FOLLOW_AFTER)
    return () => clearTimeout(timer)
    // Followed as the pattern changes; `startDrape` is the latest render's.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [revision, drape.kind])
  // The figure moved into `to`: a garment draped moves with it as a drape
  // again; one being draped, once settled.
  const choosePose = (to: AvatarPose) => {
    setPose(to)
    posed.current = to
    if (drape.kind !== 'draped') {
      sim.pose(to).catch((error: unknown) => setDrape({ kind: 'failed', text: failure(error) }))
      return
    }
    const mine = ++drapes.current
    const { fabric } = drape
    setDrape({ kind: 'draping', fabric, update: { ...drape.update, stage: 'posing' } })
    setReading(null)
    sim
      .pose(to, (update) => {
        if (mine === drapes.current) setDrape({ kind: 'draping', fabric, update })
      })
      .then(
        (last) => {
          if (mine !== drapes.current) return
          setDrape(last ? { kind: 'draped', fabric, update: last } : { kind: 'off' })
        },
        (error: unknown) => {
          if (mine === drapes.current) setDrape({ kind: 'failed', text: failure(error) })
        },
      )
  }
  const takeOff = () => {
    drapes.current++
    setDrape({ kind: 'off' })
    setReading(null)
    void sim.undrape()
  }
  const show = (as: ClothShows) => {
    setShows(as)
    setReading(null)
    sim.note({ note: 'shows', shows: as })
  }
  const on = drape.kind === 'draping' || drape.kind === 'draped' || drape.kind === 'following'
  useFigureEditing(session, editing && on && backend !== null, host, overlay, seen)
  const legend =
    shows === 'pull'
      ? t.avatar.pullLegend
      : shows === 'room'
        ? t.avatar.roomLegend(formatLength(ROOM_SCALE, units))
        : null
  const chooseFabric = (fabric: string) => {
    if (fabric === MEASURE || fabric === CHANGE) {
      setMeasuring(fabric)
      return
    }
    session.chooseDrapeFabric(fabric)
    // On the avatar already: draped again in the new fabric.
    if (on) startDrape(fabric)
  }
  const doc = session.engine.document()
  const ownChosen = doc.fabrics.find((f) => f.id === drapeFabric) ?? null
  const canDrape = backend !== null && (status.kind === 'standard' || status.kind === 'fitted')
  // What the drape is doing, or what was read where tapped.
  const named = (ids: string[]) =>
    ids.flatMap((id) => doc.pieces.find((p) => p.id === id)?.name ?? [])
  const drapeSaid = on && reading ? readingText(reading, units) : drapeText(drape, named)
  const garmentSaid = garmentText(drape, units)

  const text = t.avatar
  const said =
    status.kind === 'fitting'
      ? text.fitting
      : status.kind === 'standard'
        ? text.standard
        : status.kind === 'fitted'
          ? text.fitted(status.close, status.of)
          : status.kind === 'failed'
            ? status.text
            : text.starting
  return (
    <section
      ref={pane}
      className={floating ? 'avatar-pane floating' : 'avatar-pane'}
      style={floating ? frame.style : undefined}
      aria-label={text.heading}
    >
      {!floating && <DockEdge pane={pane} />}
      <div className="avatar-head" onPointerDown={floating ? frame.moveFromHead : undefined}>
        {floating && frame.grip}
        {sets.length > 1 && set ? (
          <Select
            className="field avatar-person"
            selectedKey={set.id}
            onSelectionChange={(id) => session.chooseAvatarPerson(String(id))}
          >
            <Label className="visually-hidden">{text.person}</Label>
            <Button className="select-button">
              <SelectValue>{() => set.name}</SelectValue>
              <ChevronDown size={16} aria-hidden="true" />
            </Button>
            <Popover className="popover">
              <ListBox className="menu">
                {sets.map((s) => (
                  <ListBoxItem key={s.id} id={s.id} className="menu-item">
                    {s.name}
                  </ListBoxItem>
                ))}
              </ListBox>
            </Popover>
          </Select>
        ) : (
          <strong className="avatar-who">{set?.name ?? text.heading}</strong>
        )}
        <span
          className={status.kind === 'failed' ? 'measure-warning small' : 'muted small'}
          role="status"
        >
          {said}
        </span>
        <div className="spacer" />
        {backend && (
          // How the figure stands: any garment on it moves with it.
          <Select
            className="avatar-pose"
            selectedKey={pose}
            isDisabled={!canDrape}
            onSelectionChange={(p) => choosePose(p as AvatarPose)}
          >
            <Label className="visually-hidden">{text.pose}</Label>
            <Button className="select-button">
              <PersonStanding size={16} aria-hidden="true" />
              <SelectValue>{() => text.poses[pose]}</SelectValue>
              <ChevronDown size={16} aria-hidden="true" />
            </Button>
            <Popover className="popover">
              <ListBox className="menu">
                {POSES.map((p) => (
                  <ListBoxItem key={p} id={p} className="menu-item">
                    {text.poses[p]}
                  </ListBoxItem>
                ))}
              </ListBox>
            </Popover>
          </Select>
        )}
        <Button
          className="icon-button"
          aria-label={floating ? (below ? text.dockBelow : text.dockBeside) : text.float}
          onPress={() => setAvatarLayout({ floating: !floating })}
        >
          {!floating ? (
            <PictureInPicture2 size={18} aria-hidden="true" />
          ) : below ? (
            <PanelBottom size={18} aria-hidden="true" />
          ) : (
            <PanelRight size={18} aria-hidden="true" />
          )}
        </Button>
        {backend && (
          <>
            <Button
              className="icon-button"
              aria-label={text.savePicture}
              isDisabled={saving !== null}
              onPress={() => void keep('picture')}
            >
              <Camera size={18} aria-hidden="true" />
            </Button>
            <Button
              className="icon-button"
              aria-label={saving === 'video' ? text.makingVideo : text.saveVideo}
              isDisabled={saving !== null || !canRecord()}
              onPress={() => void keep('video')}
            >
              <Film size={18} aria-hidden="true" />
            </Button>
          </>
        )}
        <Button
          className="icon-button"
          aria-label={text.reset}
          onPress={() => sim.note({ note: 'reset' })}
        >
          <RotateCcw size={18} aria-hidden="true" />
        </Button>
        <Button
          className="icon-button"
          aria-label={text.close}
          onPress={() => session.showAvatar(false)}
        >
          <X size={18} aria-hidden="true" />
        </Button>
      </div>
      <div className="avatar-stage">
        <div
          ref={host}
          className="avatar-view"
          role="img"
          tabIndex={0}
          aria-label={viewFailed ?? text.viewLabel(measured ? (set?.name ?? null) : null)}
        >
          <span className="avatar-cloth-swatch" aria-hidden="true" />
          <canvas ref={overlay} className="avatar-overlay" aria-hidden="true" />
          {viewFailed && <p className="avatar-cannot">{viewFailed}</p>}
          {backend && (
            <span className="avatar-backend muted small">
              {'update' in drape && drape.update?.card
                ? text.drapedWith(backend)
                : text.drawnWith(backend)}
            </span>
          )}
        </div>
        {backend && (
          // Over the view (taps go through to it), so that the view keeps
          // its size: what the drape is doing or what was read where
          // tapped, and a heatmap's key.
          <div className="avatar-notes">
            {on && legend && (
              <div
                className="avatar-legend small"
                role="img"
                aria-label={text.legend(text.showsAs[shows], legend[0]!, legend[1]!)}
              >
                <span>{legend[0]}</span>
                <span
                  className={`avatar-legend-bar ${shows}`}
                  title={`0–${shows === 'pull' ? `${PULL_SCALE} N/m` : formatLength(ROOM_SCALE, units)}`}
                />
                <span>{legend[1]}</span>
              </div>
            )}
            {garmentSaid && <p className="avatar-say small avatar-ease">{garmentSaid}</p>}
            {editing && on && <p className="avatar-say small">{text.editHint}</p>}
            <p
              className={`avatar-say small${drape.kind === 'failed' ? ' measure-warning' : ''}${drapeSaid ? '' : ' quiet'}`}
              role="status"
            >
              {drapeSaid}
            </p>
          </div>
        )}
      </div>
      {backend && (
        <div className="avatar-drape" role="group" aria-label={text.drapeGroup}>
          <div className="avatar-drape-row">
            <Select
              className="avatar-fabric"
              selectedKey={drapeFabric}
              onSelectionChange={(f) => chooseFabric(String(f))}
            >
              <Label>{text.fabric}</Label>
              <Button className="select-button">
                <SelectValue>
                  {() => ownChosen?.name ?? text.fabrics[drapeFabric as FabricPreset] ?? ''}
                </SelectValue>
                <ChevronDown size={16} aria-hidden="true" />
              </Button>
              <Popover className="popover">
                <ListBox className="menu">
                  {FABRICS.map((f) => (
                    <ListBoxItem key={f} id={f} className="menu-item">
                      {text.fabrics[f]}
                    </ListBoxItem>
                  ))}
                  {doc.fabrics.length > 0 && (
                    <ListBoxSection>
                      <Header className="menu-header">{text.yourFabrics}</Header>
                      {doc.fabrics.map((f) => (
                        <ListBoxItem key={f.id} id={f.id} className="menu-item">
                          {f.name}
                        </ListBoxItem>
                      ))}
                    </ListBoxSection>
                  )}
                  <ListBoxSection>
                    <ListBoxItem id={MEASURE} className="menu-item action">
                      {text.measureFabric}
                    </ListBoxItem>
                    {ownChosen && (
                      <ListBoxItem id={CHANGE} className="menu-item action">
                        {text.changeFabric(ownChosen.name)}
                      </ListBoxItem>
                    )}
                  </ListBoxSection>
                </ListBox>
              </Popover>
            </Select>
            <FabricDialog
              session={session}
              fabric={measuring === CHANGE ? ownChosen : null}
              isOpen={measuring !== null}
              onClose={() => setMeasuring(null)}
              onSaved={(id) => {
                setMeasuring(null)
                // Once the form has gone. A change to the fabric chosen is
                // followed like any other.
                if (id !== drapeFabric) setTimeout(() => chooseFabric(id), 0)
              }}
            />
            {drape.kind === 'draping' ? (
              <Button className="button" onPress={takeOff}>
                {text.stop}
              </Button>
            ) : (
              <Button
                className={on ? 'button' : 'button primary'}
                isDisabled={!canDrape}
                onPress={() => startDrape(drapeFabric)}
              >
                {on ? text.drapeAgain : text.drape}
              </Button>
            )}
            {(drape.kind === 'draped' || drape.kind === 'following') && (
              <Button className="button" onPress={takeOff}>
                {text.takeOff}
              </Button>
            )}
            <ToggleButton
              className="button toggle"
              isSelected={editing && on}
              isDisabled={!on}
              onChange={setEditing}
            >
              <PencilRuler size={16} aria-hidden="true" />
              {text.edit}
            </ToggleButton>
            {on && (open || (drape.update?.buttons ?? 0) > 0) && (
              <Switch
                className="switch"
                isSelected={!open}
                onChange={(doneUp) => {
                  setOpen(!doneUp)
                  opened.current = !doneUp
                  startDrape(drapeFabric)
                }}
              >
                <span className="switch-track" aria-hidden="true" />
                {text.doneUp}
              </Switch>
            )}
          </div>
          {drape.kind === 'draped' && drape.update && (
            <FitAdvice
              session={session}
              girths={drape.update.girths}
              figure={() => shown.current}
              units={units}
            />
          )}
          {/* Always there, so that the bar keeps its height: until something
              is draped, there is nothing to show. */}
          <RadioGroup
            className="segmented avatar-shows"
            orientation="horizontal"
            aria-label={text.shows}
            value={shows}
            isDisabled={!on}
            onChange={(as) => show(as as ClothShows)}
          >
            <div className="segments">
              {SHOWS.map((as) => (
                <Radio key={as} value={as} className="segment">
                  {text.showsAs[as]}
                </Radio>
              ))}
            </div>
          </RadioGroup>
        </div>
      )}
      {floating && frame.corner}
    </section>
  )
}
