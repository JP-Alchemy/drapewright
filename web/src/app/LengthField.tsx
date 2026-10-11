import type { DisplayUnits } from '@drapewright/engine'
import { Minus, Plus } from 'lucide-react'
import { Button, Group, Input, Label, NumberField, Text } from 'react-aria-components'
import { fromDisplay, lengthFormat, lengthStep, toDisplay } from '../strings/units.ts'

interface Props {
  label: string
  /** Millimetres. */
  value: number
  units: DisplayUnits
  onChange(mm: number): void
  description?: string
  min?: number
}

/** A length entered in the user's units and reported in millimetres. */
export function LengthField({ label, value, units, onChange, description, min = 0 }: Props) {
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
