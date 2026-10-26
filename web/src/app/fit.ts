// How a garment should fit: the ease wanted round the bust, underbust,
// waist and hip, for choosing a size and for altering a garment to fit.

import type { Girth } from '@drapewright/engine'

/** The levels a garment's fit is told of, top down. */
export const LEVELS = ['bust', 'waist', 'hip'] as const
export type Level = (typeof LEVELS)[number]

/**
 * The levels a size is chosen at, top down: the underbust too, where a
 * bodice or corset is often a size smaller than at the bust.
 */
export const SIZE_LEVELS = ['bust', 'underbust', 'waist', 'hip'] as const
export type SizeLevel = (typeof SIZE_LEVELS)[number]

/** How a garment should fit: the ease wanted at each level, mm. */
export const FITS = {
  fitted: { bust: 60, underbust: 40, waist: 40, hip: 60 },
  relaxed: { bust: 120, underbust: 120, waist: 120, hip: 120 },
  close: { bust: -20, underbust: -40, waist: -60, hip: -20 },
  // A knit's negative ease: about 5% smaller than the body all round.
  stretch: { bust: -40, underbust: -35, waist: -30, hip: -40 },
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
