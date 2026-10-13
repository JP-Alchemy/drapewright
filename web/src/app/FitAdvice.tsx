// How the garment on the avatar fits, against how it should: too tight or
// too loose round the bust, waist and hip, and its side seams let out or
// taken in to fit, as steps on its pieces. Altered, the garment is draped
// again (it follows the pattern), and says how it fits then.

import type { AvatarSettings, DisplayUnits, Girth } from '@drapewright/engine'
import { ChevronDown } from 'lucide-react'
import { useState } from 'react'
import {
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Select,
  SelectValue,
} from 'react-aria-components'
import { remember, remembered } from '../platform/remember.ts'
import type { Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import { sim } from '../workers/sim-client.ts'
import { FITS, type Fit, isFit, wanted } from './fit.ts'

const text = t.fitting
const FIT_KEY = 'drapewright.fit'
/** Off by less than this round the body, it fits, mm. */
const CLOSE_ENOUGH = 10

export function FitAdvice({
  session,
  girths,
  figure,
  units,
}: {
  session: Session
  girths: Girth[]
  /** The figure the garment is on. */
  figure(): AvatarSettings | null
  units: DisplayUnits
}) {
  const [fit, setFit] = useState<Fit>(() => {
    const kept = remembered<unknown>(FIT_KEY, 'fitted')
    return isFit(kept) ? kept : 'fitted'
  })
  const [altering, setAltering] = useState(false)
  // Each level the garment reaches: how much bigger round it should be.
  const off = girths.flatMap((g) => {
    const by = wanted(g, fit)
    return by === null ? [] : [{ level: g.level, by }]
  })
  if (off.length === 0) return null
  const name = (level: string) => t.avatar.levels[level] ?? level
  const length = (mm: number) => formatLength(Math.abs(mm), units)
  const wrong = off.filter((o) => Math.abs(o.by) >= CLOSE_ENOUGH)
  const right = off.filter((o) => Math.abs(o.by) < CLOSE_ENOUGH)
  const said =
    wrong.length === 0
      ? text.fits(
          text.fitNames[fit] ?? fit,
          right.map((o) => name(o.level)),
        )
      : text.off(
          text.fitNames[fit] ?? fit,
          wrong.map((o) =>
            o.by > 0
              ? text.tight(length(o.by), name(o.level))
              : text.loose(length(o.by), name(o.level)),
          ),
          right.map((o) => name(o.level)),
        )

  const alterToFit = async () => {
    const settings = figure()
    if (!settings) return
    setAltering(true)
    try {
      const doc = session.engine.document()
      const fitted = await sim.fitSteps({
        pieces: doc.pieces,
        seams: doc.seams,
        settings,
        // Levels that fit stay as they are: the changes taper to them.
        change: off.map((o) => ({ level: o.level, by: Math.abs(o.by) < CLOSE_ENOUGH ? 0 : o.by })),
      })
      // One undo step.
      const together = `fit-${Date.now()}`
      for (const step of fitted.steps) {
        session.dispatch(
          { type: 'addAlteration', piece: step.piece, alteration: step.alteration },
          { coalesce: together },
        )
      }
      const done = wrong.filter((o) => !fitted.missed.includes(o.level))
      const pieces = new Set(fitted.steps.map((s) => s.piece))
      const names = doc.pieces.filter((p) => pieces.has(p.id)).map((p) => p.name)
      session.notify({
        kind: fitted.steps.length > 0 ? 'info' : 'error',
        text: [
          fitted.steps.length > 0
            ? text.altered(
                done.map((o) =>
                  o.by > 0
                    ? text.letOut(length(o.by), name(o.level))
                    : text.takenIn(length(o.by), name(o.level)),
                ),
                names,
              )
            : text.nothingAltered,
          ...fitted.missed.map((level) => text.missed(name(level))),
        ].join(' '),
      })
    } catch (error) {
      session.notify({
        kind: 'error',
        text: text.failed(error instanceof Error ? error.message : String(error)),
      })
    } finally {
      setAltering(false)
    }
  }

  return (
    <div className="fit-advice" role="group" aria-label={text.heading}>
      <div className="avatar-drape-row">
        <Select
          className="avatar-fabric"
          selectedKey={fit}
          onSelectionChange={(f) => {
            if (!isFit(f)) return
            setFit(f)
            remember(FIT_KEY, f)
          }}
        >
          <Label>{text.heading}</Label>
          <Button className="select-button">
            <SelectValue />
            <ChevronDown size={16} aria-hidden="true" />
          </Button>
          <Popover className="popover">
            <ListBox className="menu">
              {(Object.keys(FITS) as Fit[]).map((f) => (
                <ListBoxItem key={f} id={f} className="menu-item">
                  {t.sizing.fits[f]}
                </ListBoxItem>
              ))}
            </ListBox>
          </Popover>
        </Select>
        {wrong.length > 0 && (
          <Button className="button" isDisabled={altering} onPress={() => void alterToFit()}>
            {altering ? text.altering : text.alter}
          </Button>
        )}
      </div>
      <p className="small fit-said" aria-live="polite">
        {said}
      </p>
    </div>
  )
}
