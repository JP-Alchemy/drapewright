// An example person: the standard figure the avatar shows before anyone's
// measurements are added, measured as a tape would measure it (written by
// the body's tests, crates/body/tests/fit.rs). Built in rather than kept in
// a project: there for reference, to see what each measurement is about,
// and to find a size or draft a design to before measuring anyone.

import type { Figure, MeasurementSet } from '@drapewright/engine'
import { t } from '../strings/en.ts'
import standard from './example-measurements.json'

export const EXAMPLE_ID = 'example'

export const EXAMPLE: MeasurementSet = {
  id: EXAMPLE_ID,
  name: t.measure.example,
  figure: standard.figure as Figure,
  revision: 1,
  takenOn: null,
  age: standard.age,
  values: standard.values,
  previous: [],
}

/** The people to choose from: each one measured, then the example. */
export function withExample(sets: readonly MeasurementSet[]): MeasurementSet[] {
  return [...sets, EXAMPLE]
}

export const isExample = (set: MeasurementSet) => set.id === EXAMPLE_ID
