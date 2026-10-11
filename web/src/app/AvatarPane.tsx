import type { ViewColours } from '@drapewright/engine/avatar'
import { ChevronDown, RotateCcw, X } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import {
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Select,
  SelectValue,
} from 'react-aria-components'
import { attachOrbitInput } from '../canvas/orbit-input.ts'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'
import { sim } from '../workers/sim-client.ts'
import { SimError } from '../workers/sim-protocol.ts'
import { CLOSE_MM, STANDARD_FIGURE, avatarRequest } from './avatar.ts'

/** "rgb(239, 233, 223)" as sRGB parts, 0 to 1. */
function rgb(css: string): [number, number, number] {
  const [r = 0, g = 0, b = 0] = css.match(/[\d.]+/g)?.map(Number) ?? []
  return [r / 255, g / 255, b / 255]
}

/** The view's colours, from the page's theme. */
function colours(el: HTMLElement): ViewColours {
  const style = getComputedStyle(el)
  return { background: rgb(style.backgroundColor), figure: rgb(style.color) }
}

type Status =
  | { kind: 'starting' }
  | { kind: 'fitting' }
  | { kind: 'standard' }
  | { kind: 'fitted'; close: number; of: number }
  | { kind: 'failed'; text: string }

function failure(error: unknown): string {
  if (error instanceof SimError && error.failure === 'noBodyFile')
    return t.measure.avatar.noBodyFile
  if (error instanceof SimError && error.failure === 'noView') return t.avatar.noView
  return t.measure.avatar.failed(error instanceof Error ? error.message : String(error))
}

/** The avatar in 3D, fitted to someone's measurements, beside the table. */
export function AvatarPane({ session }: { session: Session }) {
  useEngineState(session)
  const { avatarPerson } = useStore(session.ui)
  const sets = session.engine.document().measurementSets
  const set = sets.find((s) => s.id === avatarPerson) ?? sets[0] ?? null
  const host = useRef<HTMLDivElement>(null)
  const [backend, setBackend] = useState<string | null>(null)
  const [status, setStatus] = useState<Status>({ kind: 'starting' })

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
      (error: unknown) => live && setStatus({ kind: 'failed', text: failure(error) }),
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
  useEffect(() => {
    if (backend === null) return
    let live = true
    const show = async () => {
      if (!measured) {
        await sim.showAvatar(STANDARD_FIGURE)
        if (live) setStatus({ kind: 'standard' })
        return
      }
      setStatus({ kind: 'fitting' })
      const fit = await sim.fitAvatar(request)
      await sim.showAvatar(fit.settings)
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
  }, [backend, key])

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
    <section className="avatar-pane" aria-label={text.heading}>
      <div className="avatar-head">
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
      <div
        ref={host}
        className="avatar-view"
        role="img"
        tabIndex={0}
        aria-label={text.viewLabel(measured ? (set?.name ?? null) : null)}
      />
      {backend && <span className="avatar-backend muted small">{text.drawnWith(backend)}</span>}
    </section>
  )
}
