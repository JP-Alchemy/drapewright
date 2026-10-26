// Which size of a pattern in many sizes fits someone: each size sewn and
// measured on their figure, round the bust, underbust, waist and hip, and
// the size whose ease comes nearest to how it should fit suggested at each.
// Where they differ, the pattern can be cut in each level's size (or to a
// measurement there, between two sizes), blended between them.

import type { Blended, LevelSize, SizeMeasures, SizePieces } from '@drapewright/engine'
import { Check, ChevronDown } from 'lucide-react'
import { useId, useState } from 'react'
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
import { withExample } from './example.ts'
import { FITS, type Fit, SIZE_LEVELS, type SizeLevel } from './fit.ts'
import { LengthField } from './LengthField.tsx'
import { type Cut, type CutAt, cutFor, suggest } from './sizes.ts'

const text = t.sizing
/** The choice, among a level's sizes, of cutting it to a measurement. */
const MEASUREMENT = '\u0000measurement'

/** How one level is cut: chosen as a size, or as the body's girth there (mm). */
interface LevelCut {
  level: SizeLevel
  choice: string | number
  at: Cut
  body: number | null
}

export function SizeFinder({
  session,
  sizePieces,
  ready,
  onChoose,
  onBlend,
}: {
  session: Session
  /** Every size's pieces, smallest first, as they would be cut. */
  sizePieces(): Promise<SizePieces[]>
  /** Whether the pieces can be added yet. */
  ready: boolean
  /** Cuts the pattern in one size; none where that isn't offered. */
  onChoose?(size: string): void
  /** The pattern blended, cut as `at` says at each level. */
  onBlend(blended: Blended, at: CutAt[]): void
}) {
  const doc = session.engine.document()
  const units = doc.settings.displayUnits
  // Each person measured, then the example: a size for the standard
  // figure before anyone is measured.
  const people = withExample(doc.measurementSets)
  const [person, setPerson] = useState(people[0]!.id)
  const [fit, setFit] = useState<Fit>('fitted')
  const [measured, setMeasured] = useState<SizeMeasures[] | null>(null)
  const [measuring, setMeasuring] = useState(false)
  const [failed, setFailed] = useState<string | null>(null)
  // The size to cut at each level, where it isn't the one suggested, or
  // the body's girth there (mm) to cut to.
  const [picked, setPicked] = useState<Partial<Record<SizeLevel, string | number>>>({})
  const [blending, setBlending] = useState(false)
  const cutId = useId()
  const set = people.find((p) => p.id === person) ?? people[0]!

  // Every size's pieces, and the figure shaped.
  const prepare = async () => {
    const sizes = await sizePieces()
    const fitted = await sim.fitAvatar(avatarRequest(set))
    return { sizes, settings: fitted.settings }
  }

  const measure = async () => {
    setMeasuring(true)
    setFailed(null)
    try {
      setMeasured(await sim.measureSizes(await prepare()))
      setPicked({})
    } catch (error) {
      setFailed(text.failed(error instanceof Error ? error.message : String(error)))
    } finally {
      setMeasuring(false)
    }
  }

  const suggested = measured ? suggest(measured, fit) : {}
  // Only the levels the garment reaches in some size (a skirt has no bust).
  const levels = SIZE_LEVELS.filter((level) =>
    (measured ?? []).some((m) => m.girths.some((g) => g.level === level && g.garment !== null)),
  )
  const levelName = (level: SizeLevel) => t.avatar.levels[level] ?? level
  const named = SIZE_LEVELS.flatMap((level) =>
    suggested[level] ? [text.at(levelName(level), suggested[level])] : [],
  )
  // The body round each level, as measured on the figure.
  const bodyAt = (level: SizeLevel) =>
    (measured ?? []).flatMap((m) => m.girths).find((g) => g.level === level)?.body ?? 0
  // How each level the garment reaches is cut: in a size, or to the
  // measurement asked for, between two.
  const cut = levels.flatMap((level): LevelCut[] => {
    const choice = picked[level] ?? suggested[level]
    if (choice === undefined) return []
    if (typeof choice === 'string') {
      return [{ level, choice, at: { size: choice, between: 0, next: null }, body: null }]
    }
    const at = cutFor(measured ?? [], level, choice, fit)
    return at ? [{ level, choice, at, body: choice }] : []
  })
  const toBlend: LevelSize[] = cut.map((c) => ({
    level: c.level,
    size: c.at.size,
    between: c.at.between,
  }))
  const said: CutAt[] = cut.map((c) => ({
    level: c.level,
    said:
      c.body === null
        ? text.at(levelName(c.level), c.at.size)
        : text.measuredAt(levelName(c.level), formatLength(c.body, units), c.at.size, c.at.next),
  }))
  const one =
    cut.length > 0 && cut.every((c) => c.at.next === null && c.at.size === cut[0]!.at.size)
      ? cut[0]!.at.size
      : undefined
  const sewn = (measured ?? []).filter((m) => m.girths.length > 0).map((m) => m.size)
  // The garment round a level cut to a measurement.
  const garmentAt = (level: SizeLevel, body: number) => body + FITS[fit][level]

  const blend = async () => {
    setBlending(true)
    setFailed(null)
    try {
      const { sizes, settings } = await prepare()
      onBlend(await sim.blendSizes({ sizes, settings, at: toBlend }), said)
    } catch (error) {
      setFailed(text.blendFailed(error instanceof Error ? error.message : String(error)))
    } finally {
      setBlending(false)
    }
  }

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
                  {onChoose && (
                    <th scope="col">
                      <span className="visually-hidden">{text.chooseColumn}</span>
                    </th>
                  )}
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
                    {onChoose && (
                      <td>
                        <Button className="link-button" onPress={() => onChoose(m.size)}>
                          {text.choose(m.size)}
                        </Button>
                      </td>
                    )}
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          <p className="muted small">{text.how}</p>
          {cut.length > 0 && (
            <div className="size-cut" role="group" aria-labelledby={cutId}>
              <p className="group-label" id={cutId}>
                {text.cutAt}
              </p>
              <div className="size-finder-row">
                {cut.map(({ level, choice }) => (
                  <Select
                    key={level}
                    className="field compact"
                    selectedKey={typeof choice === 'string' ? choice : MEASUREMENT}
                    onSelectionChange={(key) =>
                      setPicked({
                        ...picked,
                        [level]: key === MEASUREMENT ? Math.round(bodyAt(level)) : String(key),
                      })
                    }
                  >
                    <Label>{text.columns[level]}</Label>
                    <Button className="select-button">
                      <SelectValue />
                      <ChevronDown size={16} aria-hidden="true" />
                    </Button>
                    <Popover className="popover">
                      <ListBox className="menu">
                        {sewn.map((n) => (
                          <ListBoxItem key={n} id={n} className="menu-item">
                            {n}
                          </ListBoxItem>
                        ))}
                        <ListBoxItem
                          key={MEASUREMENT}
                          id={MEASUREMENT}
                          textValue={text.byMeasurement}
                          className="menu-item"
                        >
                          {text.byMeasurement}
                        </ListBoxItem>
                      </ListBox>
                    </Popover>
                  </Select>
                ))}
                {one && onChoose ? (
                  <Button className="button" onPress={() => onChoose(one)}>
                    {text.choose(one)}
                  </Button>
                ) : (
                  <Button
                    className="button"
                    isDisabled={blending || !ready}
                    onPress={() => void blend()}
                  >
                    {blending ? text.blending : text.blend}
                  </Button>
                )}
              </div>
              {cut.flatMap(({ level, at, body }) =>
                body === null
                  ? []
                  : [
                      <LengthField
                        key={level}
                        label={text.bodyRound(levelName(level))}
                        value={body}
                        units={units}
                        min={1}
                        onChange={(mm) => setPicked({ ...picked, [level]: mm })}
                        description={text.cutBetween(
                          t.fitting.fitNames[fit] ?? fit,
                          formatLength(garmentAt(level, body), units),
                          at.size,
                          at.next,
                        )}
                      />,
                    ],
              )}
              {!(one && onChoose) && <p className="muted small">{text.blendHow}</p>}
            </div>
          )}
        </>
      )}
    </section>
  )
}
