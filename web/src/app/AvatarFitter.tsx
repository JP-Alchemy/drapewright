import type {
  AvatarFit,
  DisplayUnits,
  MeasureKind,
  MeasurementInfo,
  MeasurementSet,
} from '@drapewright/engine'
import { useState } from 'react'
import { Button, Group, Input, Label, NumberField, Text } from 'react-aria-components'
import { formatLength, t } from '../strings/en.ts'
import { sim } from '../workers/sim-client.ts'
import { SimError } from '../workers/sim-protocol.ts'
import { CLOSE_MM as CLOSE, avatarRequest } from './avatar.ts'
import { measurementText } from './labels.ts'

/** “0.4 cm larger”, “1.2 cm shorter”, or “Same” when they read the same. */
function difference(
  measured: number,
  avatar: number,
  kind: MeasureKind,
  units: DisplayUnits,
): string {
  const text = t.measure.avatar
  if (formatLength(measured, units) === formatLength(avatar, units)) return text.same
  const mm = avatar - measured
  const by = formatLength(Math.abs(mm), units)
  if (kind === 'around') return mm > 0 ? text.larger(by) : text.smaller(by)
  return mm > 0 ? text.longer(by) : text.shorter(by)
}

interface Fitted {
  fit: AvatarFit
  /** What it was fitted to: figure, age and measurements. */
  key: string
  values: Record<string, number>
}

/** Fits the avatar to a person's measurements, and says how close it comes. */
export function AvatarFitter({
  set,
  vocabulary,
  units,
  onAge,
}: {
  set: MeasurementSet
  /** The measurements taken for this figure. */
  vocabulary: MeasurementInfo[]
  units: DisplayUnits
  onAge(age: number | null): void
}) {
  const text = t.measure.avatar
  const [busy, setBusy] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)
  const [fitted, setFitted] = useState<Fitted | null>(null)

  const request = avatarRequest(set)
  const { values } = request
  const key = JSON.stringify(request)
  const empty = Object.keys(values).length === 0

  const fit = async () => {
    setBusy(true)
    setFailure(null)
    try {
      const result = await sim.fitAvatar(request)
      setFitted({ fit: result, key, values })
    } catch (error) {
      setFailure(
        error instanceof SimError && error.failure === 'noBodyFile'
          ? text.noBodyFile
          : text.failed(error instanceof Error ? error.message : String(error)),
      )
    } finally {
      setBusy(false)
    }
  }

  return (
    <section className="measure-group avatar-fit" aria-labelledby="measure-avatar">
      <h3 id="measure-avatar" className="group-label">
        {text.heading}
      </h3>
      <p className="muted small">{text.intro}</p>
      <div className="avatar-controls">
        <NumberField
          className="field compact avatar-age"
          value={set.age ?? Number.NaN}
          minValue={1}
          maxValue={120}
          step={1}
          onChange={(v) => onAge(Number.isFinite(v) && v > 0 ? Math.round(v) : null)}
        >
          <Label>{text.age}</Label>
          <Group className="number-group">
            <Input className="number-input" />
          </Group>
          <Text slot="description" className="muted small">
            {text.ageHint}
          </Text>
        </NumberField>
        <Button className="button primary" isDisabled={empty} isPending={busy} onPress={fit}>
          {busy ? text.fitting : fitted ? text.again : text.fit}
        </Button>
      </div>
      {empty && <p className="muted small">{text.need}</p>}
      <div aria-live="polite">
        {failure !== null && <p className="measure-warning">{failure}</p>}
        {fitted && (
          <Report
            fitted={fitted}
            changed={fitted.key !== key}
            set={set}
            vocabulary={vocabulary}
            units={units}
          />
        )}
      </div>
    </section>
  )
}

function Report({
  fitted,
  changed,
  set,
  vocabulary,
  units,
}: {
  fitted: Fitted
  changed: boolean
  set: MeasurementSet
  vocabulary: MeasurementInfo[]
  units: DisplayUnits
}) {
  const text = t.measure.avatar
  const { fit, values } = fitted
  const rows = vocabulary.filter((m) => fit.residuals[m.key] !== undefined)
  const close = rows.filter((m) => Math.abs(fit.residuals[m.key] ?? 0) <= CLOSE).length
  const notTaken = Object.keys(values).filter((k) => fit.residuals[k] === undefined)
  return (
    <>
      {changed && <p className="measure-warning">{text.changed}</p>}
      <p>{text.summary(close, rows.length)}</p>
      <div className="avatar-report-wrap">
        <table className="avatar-report">
          <caption className="visually-hidden">{text.report}</caption>
          <thead>
            <tr>
              {text.columns.map((c) => (
                <th key={c} scope="col">
                  {c}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((m) => {
              const measured = values[m.key] ?? 0
              const avatar = fit.measured[m.key] ?? 0
              return (
                <tr key={m.key} className={Math.abs(avatar - measured) > CLOSE ? 'far' : undefined}>
                  <th scope="row">{measurementText(m.key, set.figure)[0]}</th>
                  <td>{formatLength(measured, units)}</td>
                  <td>{formatLength(avatar, units)}</td>
                  <td>{difference(measured, avatar, m.kind, units)}</td>
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
      {notTaken.length > 0 && (
        <p className="muted small">{text.notTaken(notTaken.map((k) => k.slice(2)).join(', '))}</p>
      )}
    </>
  )
}
