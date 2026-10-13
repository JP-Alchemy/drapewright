// Which size of a pattern in many sizes fits someone: each size sewn and
// measured on their figure, round the bust, waist and hip, and the size
// whose ease comes nearest to how it should fit suggested at each.

import type { AllowanceAnswer, ImportResult, ScaleOption, SizeMeasures } from '@drapewright/engine'
import { Check, ChevronDown } from 'lucide-react'
import { useState } from 'react'
import {
  Button,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
} from 'react-aria-components'
import type { Session } from '../state/session.ts'
import { formatLength, t } from '../strings/en.ts'
import { sim } from '../workers/sim-client.ts'
import { avatarRequest } from './avatar.ts'

const text = t.sizing
const LEVELS = ['bust', 'waist', 'hip'] as const

/** How a garment should fit: the ease wanted at each level, mm. */
export const FITS = {
  fitted: { bust: 60, waist: 40, hip: 60 },
  relaxed: { bust: 120, waist: 120, hip: 120 },
  close: { bust: -20, waist: -60, hip: -20 },
} as const
export type Fit = keyof typeof FITS

/** At each level, the size whose ease comes nearest to what `fit` wants. */
export function suggest(measured: SizeMeasures[], fit: Fit): Partial<Record<string, string>> {
  const out: Partial<Record<string, string>> = {}
  for (const level of LEVELS) {
    let best: { size: string; off: number } | null = null
    for (const m of measured) {
      const g = m.girths.find((x) => x.level === level)
      if (!g || g.garment === null) continue
      const off = Math.abs(g.garment - g.body - FITS[fit][level])
      if (!best || off < best.off) best = { size: m.size, off }
    }
    if (best) out[level] = best.size
  }
  return out
}

export function SizeFinder({
  session,
  result,
  sizeOf,
  scale,
  allowance,
  leftOut,
  onChoose,
}: {
  session: Session
  result: ImportResult
  /** Each piece's size, by id; pieces for every size have none. */
  sizeOf: Map<string, string>
  scale: ScaleOption
  allowance: AllowanceAnswer | null
  leftOut: ReadonlySet<string>
  onChoose(size: string): void
}) {
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  const people = doc.measurementSets
  const [person, setPerson] = useState(people[0]?.id ?? null)
  const [fit, setFit] = useState<Fit>('fitted')
  const [measured, setMeasured] = useState<SizeMeasures[] | null>(null)
  const [measuring, setMeasuring] = useState(false)
  const [failed, setFailed] = useState<string | null>(null)
  const set = people.find((p) => p.id === person) ?? people[0]

  if (!set) {
    return (
      <section className="size-finder" aria-labelledby="size-finder-heading">
        <p className="group-label" id="size-finder-heading">
          {text.heading}
        </p>
        <p className="muted small">{text.noPeople}</p>
      </section>
    )
  }

  const measure = async () => {
    setMeasuring(true)
    setFailed(null)
    try {
      // Every size, as it would be added: scale and allowance answered.
      const finished = await session.io.call(
        'finishImport',
        result,
        result.sizes.names,
        scale,
        allowance,
      )
      const sizes = result.sizes.names.map((size) => ({
        size,
        pieces: finished.pieces.filter((p) => {
          const own = sizeOf.get(p.id)
          return !leftOut.has(p.id) && (own === undefined || own === size)
        }),
      }))
      const fitted = await sim.fitAvatar(avatarRequest(set))
      setMeasured(await sim.measureSizes({ sizes, settings: fitted.settings }))
    } catch (error) {
      setFailed(text.failed(error instanceof Error ? error.message : String(error)))
    } finally {
      setMeasuring(false)
    }
  }

  const suggested = measured ? suggest(measured, fit) : {}
  // Only the levels the garment reaches in some size (a skirt has no bust).
  const levels = LEVELS.filter((level) =>
    (measured ?? []).some((m) => m.girths.some((g) => g.level === level && g.garment !== null)),
  )
  const named = LEVELS.flatMap((level) =>
    suggested[level] ? [text.at(t.avatar.levels[level] ?? level, suggested[level])] : [],
  )

  return (
    <section className="size-finder" aria-labelledby="size-finder-heading">
      <p className="group-label" id="size-finder-heading">
        {text.heading}
      </p>
      <div className="size-finder-row">
        {people.length > 1 && (
          <Select
            className="field compact"
            selectedKey={set.id}
            onSelectionChange={(id) => {
              setPerson(String(id))
              setMeasured(null)
            }}
          >
            <Label>{text.person}</Label>
            <Button className="select-button">
              <SelectValue>{() => set.name}</SelectValue>
              <ChevronDown size={16} aria-hidden="true" />
            </Button>
            <Popover className="popover">
              <ListBox className="menu">
                {people.map((p) => (
                  <ListBoxItem key={p.id} id={p.id} className="menu-item">
                    {p.name}
                  </ListBoxItem>
                ))}
              </ListBox>
            </Popover>
          </Select>
        )}
        <RadioGroup
          className="segmented"
          orientation="horizontal"
          value={fit}
          onChange={(f) => setFit(f as Fit)}
        >
          <Label className="group-label">{text.fit}</Label>
          <div className="segments">
            {(Object.keys(FITS) as Fit[]).map((f) => (
              <Radio key={f} value={f} className="segment">
                {text.fits[f]}
              </Radio>
            ))}
          </div>
        </RadioGroup>
      </div>
      <Button className="button" isDisabled={measuring} onPress={() => void measure()}>
        {measuring ? text.measuring : text.measure(set.name)}
      </Button>
      {failed && <p className="measure-warning small">{failed}</p>}
      {measured && (
        <>
          {named.length > 0 && (
            <p className="small" role="status">
              {text.suggested(set.name, named)}
            </p>
          )}
          <div className="size-table-wrap">
            <table className="size-table">
              <thead>
                <tr>
                  <th scope="col">{text.size}</th>
                  {levels.map((level) => (
                    <th key={level} scope="col">
                      {text.columns[level]}
                    </th>
                  ))}
                  <th scope="col">
                    <span className="visually-hidden">{text.chooseColumn}</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                {measured.map((m) => (
                  <tr key={m.size}>
                    <th scope="row">{m.size}</th>
                    {m.girths.length === 0 ? (
                      <td colSpan={levels.length} className="muted small">
                        {text.notSewn}
                      </td>
                    ) : (
                      levels.map((level) => {
                        const g = m.girths.find((x) => x.level === level)
                        if (!g || g.garment === null) return <td key={level}>–</td>
                        const ease = g.garment - g.body
                        const mine = suggested[level] === m.size
                        return (
                          <td key={level} className={mine ? 'suggested' : undefined}>
                            {mine && <Check size={14} aria-label={text.suggestedHere} />}
                            {formatLength(g.garment, units)}
                            <span className="muted small">
                              {' '}
                              {text.ease(
                                ease >= 0 ? '+' : '−',
                                formatLength(Math.abs(ease), units),
                              )}
                            </span>
                          </td>
                        )
                      })
                    )}
                    <td>
                      <Button className="link-button" onPress={() => onChoose(m.size)}>
                        {text.choose(m.size)}
                      </Button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="muted small">{text.how}</p>
        </>
      )}
    </section>
  )
}
