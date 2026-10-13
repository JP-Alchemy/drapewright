// How a garment should fit: the ease wanted round the bust, waist and hip,
// for choosing a size and for altering a garment to fit.

import type { Girth } from '@drapewright/engine'

/** The levels a garment is measured round, top down. */
export const LEVELS = ['bust', 'waist', 'hip'] as const
export type Level = (typeof LEVELS)[number]

/** How a garment should fit: the ease wanted at each level, mm. */
export const FITS = {
  fitted: { bust: 60, waist: 40, hip: 60 },
  relaxed: { bust: 120, waist: 120, hip: 120 },
  close: { bust: -20, waist: -60, hip: -20 },
} as const
export type Fit = keyof typeof FITS

export const isFit = (f: unknown): f is Fit => typeof f === 'string' && Object.hasOwn(FITS, f)

const isLevel = (level: string): level is Level => (LEVELS as readonly string[]).includes(level)

/**
 * How much bigger round a garment should be at a level to fit as `fit`
 * wants (smaller where less than 0), mm; none where it isn't measured
 * there.
 */
export function wanted(g: Girth, fit: Fit): number | null {
  if (g.garment === null || !isLevel(g.level)) return null
  return FITS[fit][g.level] - (g.garment - g.body)
}
