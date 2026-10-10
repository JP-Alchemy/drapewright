// Converting lengths between millimetres (stored) and the user's units.

import type { DisplayUnits } from '@soom/engine'

const MM_PER: Record<DisplayUnits, number> = { mm: 1, cm: 10, in: 25.4 }

export const toDisplay = (mm: number, units: DisplayUnits): number => mm / MM_PER[units]
export const fromDisplay = (value: number, units: DisplayUnits): number => value * MM_PER[units]

/** Number formatting for a length field in these units. */
export function lengthFormat(units: DisplayUnits): Intl.NumberFormatOptions {
  const unit = { mm: 'millimeter', cm: 'centimeter', in: 'inch' }[units]
  return {
    style: 'unit',
    unit,
    unitDisplay: 'short',
    maximumFractionDigits: units === 'mm' ? 1 : 3,
  }
}

/** Sensible step for a length field: 1 mm, 0.1 cm (1 mm), 1/16 in. */
export function lengthStep(units: DisplayUnits): number {
  return { mm: 1, cm: 0.1, in: 0.0625 }[units]
}
