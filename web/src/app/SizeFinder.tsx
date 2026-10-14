// Which size of a pattern in many sizes fits someone: each size sewn and
// measured on their figure, round the bust, waist and hip, and the size
// whose ease comes nearest to how it should fit suggested at each. Where
// they differ, the pattern can be cut in each level's size, blended
// between them.

import type {
  AllowanceAnswer,
  Blended,
  ImportResult,
  LevelSize,
  ScaleOption,
  SizeMeasures,
  SizePieces,
} from '@drapewright/engine'
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
import { FITS, type Fit, LEVELS, type Level } from './fit.ts'

const text = t.sizing
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
  ready,
  onChoose,
  onBlend,
}: {
  session: Session
  result: ImportResult
  /** Each piece's size, by id; pieces for every size have none. */
  sizeOf: Map<string, string>
  scale: ScaleOption
  allowance: AllowanceAnswer | null
  leftOut: ReadonlySet<string>
  /** Whether the import's questions are answered, so pieces can be added. */
  ready: boolean
  onChoose(size: string): void
  /** The pattern blended, cut in `at`'s size at each level. */
  onBlend(blended: Blended, at: LevelSize[]): void
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
  // The size to cut at each level, where it isn't the one suggested.
  const [picked, setPicked] = useState<Partial<Record<Level, string>>>({})
  const [blending, setBlending] = useState(false)
  const cutId = useId()
  const set = people.find((p) => p.id === person) ?? people[0]!

  // Every size's pieces, as they would be added (scale and allowance
  // answered, named as in the file), and the figure shaped.
  const prepare = async () => {
    const finished = await session.io.call(
      'finishImport',
      result,
      result.sizes.names,
      scale,
      allowance,
    )
    // A piece left out in the size shown is left out of every size.
    const named = new Map(result.pieces.map((p) => [p.id, p.name]))
    const unwanted = new Set([...leftOut].map((id) => named.get(id)))
    const sizes: SizePieces[] = result.sizes.names.map((size) => ({
      size,
      pieces: finished.pieces
        .map((p) => ({ ...p, name: named.get(p.id) ?? p.name }))
        .filter((p) => {
          const own = sizeOf.get(p.id)
          return !unwanted.has(p.name) && (own === undefined || own === size)
        }),
    }))
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
  const levels = LEVELS.filter((level) =>
    (measured ?? []).some((m) => m.girths.some((g) => g.level === level && g.garment !== null)),
  )
  const named = LEVELS.flatMap((level) =>
    suggested[level] ? [text.at(t.avatar.levels[level] ?? level, suggested[level])] : [],
  )
  // The size to cut at each level the garment reaches.
  const cut: LevelSize[] = levels.flatMap((level) => {
    const size = picked[level] ?? suggested[level]
    return size ? [{ level, size }] : []
  })
  const one = cut.every((c) => c.size === cut[0]?.size) ? cut[0]?.size : undefined
  const sewn = (measured ?? []).filter((m) => m.girths.length > 0).map((m) => m.size)

  const blend = async () => {
    setBlending(true)
    setFailed(null)
    try {
      const { sizes, settings } = await prepare()
      onBlend(await sim.blendSizes({ sizes, settings, at: cut }), cut)
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
          {cut.length > 0 && (
            <div className="size-cut" role="group" aria-labelledby={cutId}>
              <p className="group-label" id={cutId}>
                {text.cutAt}
              </p>
              <div className="size-finder-row">
                {cut.map(({ level, size }) => (
                  <Select
                    key={level}
                    className="field compact"
                    selectedKey={size}
                    onSelectionChange={(key) => setPicked({ ...picked, [level]: String(key) })}
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
                      </ListBox>
                    </Popover>
                  </Select>
                ))}
                {one ? (
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
              {!one && <p className="muted small">{text.blendHow}</p>}
            </div>
          )}
        </>
      )}
    </section>
  )
}
