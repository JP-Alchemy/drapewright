import type { DisplayUnits } from '@drapewright/engine'
import { Minus, Plus } from 'lucide-react'
import { useId, useState } from 'react'
import { Button, Group, Input, Label, NumberField, Text } from 'react-aria-components'
import { t } from '../strings/en.ts'
import { fromDisplay, lengthFormat, lengthStep, toDisplay } from '../strings/units.ts'

interface Props {
  label: string
  /** Millimetres; `null` where things chosen together differ. */
  value: number | null
  units: DisplayUnits
  onChange(mm: number): void
  /** Steps each one's own length (mm), where they differ. */
  onStep?(mm: number): void
  description?: string
  min?: number
}

/** A length entered in the user's units and reported in millimetres. */
export function LengthField({
  label,
  value,
  units,
  onChange,
  onStep,
  description,
  min = 0,
}: Props) {
  if (value === null) {
    const step = fromDisplay(lengthStep(units), units)
    return (
      <MixedNumberField
        label={label}
        step={step}
        onStep={(mm) => onStep?.(mm)}
        onSet={(mm) => onChange(Math.max(min, mm))}
        parse={(text) => {
          const v = Number.parseFloat(text.replace(',', '.'))
          return Number.isFinite(v) ? fromDisplay(v, units) : null
        }}
      />
    )
  }
  return (
    <NumberField
      className="field"
      value={toDisplay(value, units)}
      minValue={toDisplay(min, units)}
      step={lengthStep(units)}
      formatOptions={lengthFormat(units)}
      onChange={(v) => {
        if (Number.isFinite(v)) onChange(fromDisplay(v, units))
      }}
    >
      <Label>{label}</Label>
      <Group className="number-group">
        <Button slot="decrement" className="step-button">
          <Minus size={16} aria-hidden="true" />
        </Button>
        <Input className="number-input" />
        <Button slot="increment" className="step-button">
          <Plus size={16} aria-hidden="true" />
        </Button>
      </Group>
      {description && (
        <Text slot="description" className="muted small">
          {description}
        </Text>
      )}
    </NumberField>
  )
}

/**
 * A number that things chosen together hold different values of, shown as
 * mixed. Its buttons step each one's own value by `step`; a number typed in
 * (read by `parse`) sets them all to it.
 */
export function MixedNumberField({
  label,
  step,
  onStep,
  onSet,
  parse,
}: {
  label: string
  step: number
  onStep(by: number): void
  onSet(value: number): void
  parse(text: string): number | null
}) {
  const id = useId()
  const [draft, setDraft] = useState('')
  const commit = () => {
    const value = parse(draft)
    if (value !== null) onSet(value)
    setDraft('')
  }
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <div className="number-group">
        <Button
          className="step-button"
          aria-label={t.inspector.decrease(label)}
          onPress={() => onStep(-step)}
        >
          <Minus size={16} aria-hidden="true" />
        </Button>
        <input
          id={id}
          className="number-input"
          value={draft}
          placeholder={t.inspector.mixed}
          inputMode="decimal"
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => e.key === 'Enter' && commit()}
        />
        <Button
          className="step-button"
          aria-label={t.inspector.increase(label)}
          onPress={() => onStep(step)}
        >
          <Plus size={16} aria-hidden="true" />
        </Button>
      </div>
    </div>
  )
}
