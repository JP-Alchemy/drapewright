import {
  type DisplayUnits,
  type Figure,
  type MeasurementInfo,
  type MeasurementKey,
  type MeasurementSet,
  measurementVocabulary,
} from '@drapewright/engine'
import { PersonStanding, Plus } from 'lucide-react'
import { useState } from 'react'
import {
  Button,
  Dialog,
  DialogTrigger,
  Group,
  Heading,
  Input,
  Label,
  Modal,
  ModalOverlay,
  NumberField,
  Radio,
  RadioGroup,
  Text,
  TextField,
} from 'react-aria-components'
import { useEngineState } from '../state/hooks.ts'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { fromDisplay, lengthFormat, lengthStep, toDisplay } from '../strings/units.ts'
import { AvatarFitter } from './AvatarFitter.tsx'
import { CommitTextField } from './Inspector.tsx'
import { measurementText } from './labels.ts'

/** Today in the device's time zone, as `YYYY-MM-DD`. */
function today(): string {
  return new Date().toLocaleDateString('en-CA')
}

/** Whether a value is outside the usual range for adults (wrong units, most likely). */
export function isUnusual(info: MeasurementInfo, mm: number): boolean {
  return mm < info.usual[0] || mm > info.usual[1]
}

/** The Measurements button, and behind it each person's measurements. */
export function MeasurementsDialog({ session }: { session: Session }) {
  return (
    <DialogTrigger>
      <Button className="button">
        <PersonStanding size={18} aria-hidden="true" />
        <span className="button-label">{t.measure.open}</span>
      </Button>
      <ModalOverlay isDismissable className="modal-overlay">
        <Modal className="modal wide">
          <Dialog className="dialog">
            {({ close }) => <MeasurementsForm session={session} close={close} />}
          </Dialog>
        </Modal>
      </ModalOverlay>
    </DialogTrigger>
  )
}

function MeasurementsForm({ session, close }: { session: Session; close: () => void }) {
  useEngineState(session)
  const doc = session.engine.document()
  const sets = doc.measurementSets
  const [chosen, setChosen] = useState<string | null>(sets[0]?.id ?? null)
  const set = sets.find((s) => s.id === chosen) ?? sets[0] ?? null

  const add = (name: string) => {
    const added = session.dispatch({
      type: 'addMeasurementSet',
      set: {
        id: '',
        name,
        figure: 'woman',
        revision: 1,
        takenOn: today(),
        age: null,
        values: {},
        previous: [],
      },
    })
    if (added) setChosen(session.engine.document().measurementSets.at(-1)?.id ?? null)
  }

  return (
    <>
      <Heading slot="title">{t.measure.heading}</Heading>
      <p className="muted">{t.measure.intro}</p>
      {set === null ? (
        <div className="measure-empty">
          <p>{t.measure.empty}</p>
          <Button className="button primary" onPress={() => add(t.measure.defaultName)}>
            <Plus size={18} aria-hidden="true" />
            {t.measure.addFirst}
          </Button>
        </div>
      ) : (
        <>
          <div className="measure-people">
            {sets.length > 1 && (
              <RadioGroup
                className="segmented"
                orientation="horizontal"
                aria-label={t.measure.people}
                value={set.id}
                onChange={setChosen}
              >
                <div className="segments">
                  {sets.map((s) => (
                    <Radio key={s.id} value={s.id} className="segment">
                      {s.name}
                    </Radio>
                  ))}
                </div>
              </RadioGroup>
            )}
            <Button className="button" onPress={() => add(t.measure.newName(sets.length + 1))}>
              <Plus size={18} aria-hidden="true" />
              {t.measure.add}
            </Button>
          </div>
          <SetEditor
            key={set.id}
            session={session}
            set={set}
            units={doc.settings.displayUnits}
            onDeleted={() => setChosen(null)}
          />
        </>
      )}
      <div className="dialog-actions">
        <Button className="button primary" onPress={close}>
          {t.measure.done}
        </Button>
      </div>
    </>
  )
}

function SetEditor({
  session,
  set,
  units,
  onDeleted,
}: {
  session: Session
  set: MeasurementSet
  units: DisplayUnits
  onDeleted(): void
}) {
  const [confirming, setConfirming] = useState(false)
  const update = (change: Partial<MeasurementSet>) =>
    session.dispatch(
      { type: 'setMeasurementSet', set: { ...set, ...change } },
      { coalesce: `measure:${set.id}` },
    )
  const setValue = (key: MeasurementKey, mm: number | null) => {
    const values = { ...set.values }
    if (mm === null) delete values[key]
    // A tenth of a millimetre is finer than any tape.
    else values[key] = Math.round(mm * 10) / 10
    update({ values })
  }
  const vocabulary = measurementVocabulary().filter((m) => set.figure === 'woman' || !m.womenOnly)
  const groups = Object.keys(t.measure.groups) as (keyof typeof t.measure.groups)[]

  return (
    <div className="measure-set">
      <div className="measure-head">
        <CommitTextField
          key={`name-${set.name}`}
          label={t.measure.name}
          value={set.name}
          onCommit={(name) => name && update({ name })}
        />
        <RadioGroup
          className="segmented"
          orientation="horizontal"
          value={set.figure}
          onChange={(figure) => update({ figure: figure as Figure })}
        >
          <Label className="group-label">{t.measure.figure}</Label>
          <div className="segments">
            <Radio value="woman" className="segment">
              {t.measure.figures.woman}
            </Radio>
            <Radio value="man" className="segment">
              {t.measure.figures.man}
            </Radio>
          </div>
        </RadioGroup>
        <TextField
          className="field"
          type="date"
          value={set.takenOn ?? ''}
          onChange={(date) => update({ takenOn: date || null })}
        >
          <Label>{t.measure.takenOn}</Label>
          <Input className="text-input" />
        </TextField>
      </div>
      <div className="measure-revision">
        <span className="muted small">
          {t.measure.revision(set.revision, set.takenOn)}
          {set.previous.length > 0 && ` · ${t.measure.earlier(set.previous.length)}`}
        </span>
        <Button
          className="button"
          aria-description={t.measure.againHint}
          onPress={() => session.dispatch({ type: 'remeasureSet', id: set.id, takenOn: today() })}
        >
          {t.measure.again}
        </Button>
      </div>
      {groups.map((group) => {
        const items = vocabulary.filter((m) => m.group === group)
        return (
          <section key={group} className="measure-group" aria-labelledby={`measure-${group}`}>
            <h3 id={`measure-${group}`} className="group-label">
              {t.measure.groups[group]}
            </h3>
            <div className="measure-grid">
              {items.map((info) => (
                <MeasurementField
                  key={info.key}
                  info={info}
                  figure={set.figure}
                  value={set.values[info.key]}
                  units={units}
                  onChange={(mm) => setValue(info.key, mm)}
                />
              ))}
            </div>
          </section>
        )
      })}
      <AvatarFitter
        set={set}
        vocabulary={vocabulary}
        units={units}
        onAge={(age) => update({ age })}
      />
      <div className="measure-delete">
        {confirming ? (
          <>
            <span>{t.measure.removeConfirm(set.name)}</span>
            <Button
              className="button danger"
              onPress={() => {
                if (session.dispatch({ type: 'deleteMeasurementSet', id: set.id })) onDeleted()
              }}
            >
              {t.measure.remove}
            </Button>
            <Button className="button" onPress={() => setConfirming(false)}>
              {t.screen.cancel}
            </Button>
          </>
        ) : (
          <Button className="button danger" onPress={() => setConfirming(true)}>
            {t.measure.remove}
          </Button>
        )}
      </div>
    </div>
  )
}

/** One measurement, in the user's units; empty when not taken. */
function MeasurementField({
  info,
  figure,
  value,
  units,
  onChange,
}: {
  info: MeasurementInfo
  figure: Figure
  value: number | undefined
  units: DisplayUnits
  onChange(mm: number | null): void
}) {
  const [name, howTo] = measurementText(info.key, figure)
  const unusual = value !== undefined && isUnusual(info, value)
  return (
    <NumberField
      className="field compact"
      value={value === undefined ? Number.NaN : toDisplay(value, units)}
      minValue={0}
      step={lengthStep(units)}
      formatOptions={lengthFormat(units)}
      onChange={(v) => onChange(Number.isFinite(v) && v > 0 ? fromDisplay(v, units) : null)}
    >
      <Label>{name}</Label>
      <Group className="number-group">
        <Input className="number-input" />
      </Group>
      <Text slot="description" className="muted small">
        {unusual && <span className="measure-warning">{t.measure.unusual(name)} </span>}
        {howTo}
      </Text>
    </NumberField>
  )
}
