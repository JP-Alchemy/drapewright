// What the avatar is fitted to, the same wherever it is fitted.

import {
  type AvatarRequest,
  type AvatarSettings,
  type MeasurementSet,
  measurementVocabulary,
} from '@drapewright/engine'

/** The avatar's age when a person's is not known. */
export const STANDARD_AGE = 35

/** The figure shown before anyone's measurements. */
export const STANDARD_FIGURE: AvatarSettings = {
  woman: 1,
  age: STANDARD_AGE,
  muscle: 0.5,
  weight: 0.5,
  height: 0.5,
  proportions: 0.5,
  cup: 0.5,
  firmness: 0.5,
  local: {},
}

/** Within this, mm, the avatar matches a measurement. */
export const CLOSE_MM = 10

/**
 * A person's measurements as the avatar is fitted to them. A man's figure
 * leaves out the bust's shape, as the form does; the seamstress's own
 * measurements go too, to be reported as not taken.
 */
export function avatarRequest(set: MeasurementSet): AvatarRequest {
  const taken = new Set<string>(
    measurementVocabulary()
      .filter((m) => set.figure === 'woman' || !m.womenOnly)
      .map((m) => m.key),
  )
  const values = Object.fromEntries(
    Object.entries(set.values).filter(([k]) => taken.has(k) || k.startsWith('x-')),
  )
  return { woman: set.figure === 'woman', age: set.age ?? STANDARD_AGE, values }
}
