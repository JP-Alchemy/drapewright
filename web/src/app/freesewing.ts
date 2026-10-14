// FreeSewing's designs (freesewing.org; MIT, © Joost De Cock and
// contributors), drafted in the browser to someone's measurements and
// opened as a pattern. Each drafted piece is written as FreeSewing writes
// its own SVG (its stitching and cutting lines, grainline, fold, notches,
// and how many to cut), for the SVG import to read as it reads FreeSewing's
// files. A design is loaded only when it is drafted.
//
// Measurements the person hasn't taken are estimated from FreeSewing's
// standard sizes: the size nearest them, in proportion to what they have
// measured.

import type { MeasurementSet } from '@drapewright/engine'
import { measurementVocabulary } from '@drapewright/engine'

export const DESIGNS = [
  'teagan',
  'sasha',
  'penelope',
  'sandy',
  'cathrin',
  'bella',
  'simone',
  'diana',
] as const
export type DesignName = (typeof DESIGNS)[number]

const LOADERS: Record<DesignName, () => Promise<Record<string, unknown>>> = {
  teagan: () => import('@freesewing/teagan'),
  sasha: () => import('@freesewing/sasha'),
  penelope: () => import('@freesewing/penelope'),
  sandy: () => import('@freesewing/sandy'),
  cathrin: () => import('@freesewing/cathrin'),
  bella: () => import('@freesewing/bella'),
  simone: () => import('@freesewing/simone'),
  diana: () => import('@freesewing/diana'),
}

/**
 * Design options set here rather than left at FreeSewing's defaults. Sasha's
 * pockets are drafted into the side front as a bag behind the panel below
 * it, which can't yet be sewn or draped: it is drafted without them.
 */
const OPTIONS: Partial<Record<DesignName, Record<string, unknown>>> = {
  sasha: { withPocket: false },
}

/**
 * Corrections to a design as it is drafted, where FreeSewing's pieces
 * don't go together. Sasha widens its front neckline along the shoulder but
 * not its back's, leaving the back shoulder longer than the front's by as
 * much (36 mm at its default width): the back neckline is widened by as
 * much too, keeping the design's own ease in the back shoulder.
 */
const CORRECTIONS: Partial<Record<DesignName, (pattern: FsPattern) => void>> = {
  sasha(pattern) {
    let widened = 0
    pattern.on('postPartDraft', (p) => {
      const front = p.parts[0]?.['sasha.frontInside']
      if (p.activePart !== 'sasha.frontInside' || !front) return
      widened = front.points.hps!.dist(front.points.neckShoulder!)
    })
    pattern.on('prePartDraft', (p) => {
      const back = p.parts[0]?.['sasha.backInside']
      if (p.activePart !== 'sasha.backInside' || !back) return
      back.points.hps = back.points.hps!.shiftTowards(back.points.shoulder!, widened)
    })
  },
}

// What is used of FreeSewing's objects.
interface FsPoint {
  x: number
  y: number
  dist(that: FsPoint): number
  shiftTowards(that: FsPoint, distance: number): FsPoint
}
interface FsPath {
  hidden: boolean
  attributes: { get(key: string): string | false }
  asPathstring(): string
  bbox(): { topLeft: FsPoint; bottomRight: FsPoint }
  /** Each of its lines and curves, as a path of its own. */
  divide(): FsPath[]
  shiftFractionAlong(fraction: number, stepsPerMm?: number): FsPoint
  length(): number
  start(): FsPoint
  end(): FsPoint
}
interface FsPart {
  hidden: boolean
  points: Record<string, FsPoint>
  paths: Record<string, FsPath>
  snippets: Record<string, { def: string; anchor: FsPoint }>
}
interface FsCut {
  cutOnFold?: FsPoint[]
  /** Where its grainline comes from: `cutonfold` when the fold bracket is it. */
  grainOrigin?: string
  materials?: Record<string, { cut: number; identical: boolean; onFold: boolean }[]>
}
interface FsPattern {
  draft(): FsPattern
  /** Calls `method` at a point of drafting (before or after each part, say). */
  on(hook: 'prePartDraft' | 'postPartDraft', method: (pattern: FsPattern) => void): FsPattern
  /** The part being drafted, by name. */
  activePart: string
  getConfig(): { measurements?: string[]; optionalMeasurements?: string[] }
  parts: Record<string, FsPart>[]
  setStores: { cutlist?: Record<string, FsCut> }[]
  config: { measurements: string[]; optionalMeasurements: string[] }
}
type FsDesign = new (settings: Record<string, unknown>) => FsPattern

async function designOf(name: DesignName): Promise<FsDesign> {
  const mod = await LOADERS[name]()
  const exported = name.charAt(0).toUpperCase() + name.slice(1)
  return mod[exported] as FsDesign
}

/** The measurements a design needs, and those it uses if they are there. */
export async function needs(name: DesignName): Promise<{ required: string[]; optional: string[] }> {
  const Design = await designOf(name)
  // Its configuration, without drafting it (which, with no measurements,
  // FreeSewing would log as errors).
  const config = new Design({}).getConfig()
  return {
    required: config.measurements ?? [],
    optional: config.optionalMeasurements ?? [],
  }
}

/** Someone's measurements by FreeSewing's names, mm (as Drapewright keeps them). */
export function freesewingMeasurements(set: MeasurementSet): Record<string, number> {
  const out: Record<string, number> = {}
  for (const info of measurementVocabulary()) {
    const value = set.values[info.key]
    if (info.freesewing && typeof value === 'number' && value > 0) out[info.freesewing] = value
  }
  return out
}

/** FreeSewing's measurements taken round the body (the rest are lengths, or angles). */
const AROUND = new Set([
  'ankle',
  'biceps',
  'chest',
  'head',
  'heel',
  'highBust',
  'hips',
  'knee',
  'neck',
  'seat',
  'underbust',
  'upperLeg',
  'waist',
  'wrist',
  'bustFront',
  'highBustFront',
  'seatBack',
  'waistBack',
])
const ANGLES = new Set(['shoulderSlope'])

/**
 * Each of `wanted` the person hasn't measured, estimated from FreeSewing's
 * standard size nearest them: in proportion to what they have measured
 * round the body (or along it, for a length).
 */
export async function estimated(
  have: Record<string, number>,
  wanted: string[],
  woman: boolean,
): Promise<Record<string, number>> {
  const missing = wanted.filter((m) => !(m in have))
  if (missing.length === 0) return {}
  const models = (await import('@freesewing/models')) as Record<string, unknown>
  const group = woman ? 'cisFemaleAdult' : 'cisMaleAdult'
  const sizes = ((models.sizes as Record<string, number[]>)[group] ?? []).map(
    (n) => models[`${group}${n}`] as Record<string, number>,
  )
  // The size nearest round the chest (or the seat, or the waist).
  const by = ['chest', 'seat', 'waist', 'neck'].find((m) => m in have)
  const size = sizes.reduce((best, s) =>
    by && Math.abs((s[by] ?? 0) - have[by]!) < Math.abs((best[by] ?? 0) - have[by]!) ? s : best,
  )
  const ratio = (kind: (m: string) => boolean) => {
    const known = Object.keys(have).filter((m) => kind(m) && (size[m] ?? 0) > 0)
    if (known.length === 0) return 1
    return known.reduce((sum, m) => sum + have[m]! / size[m]!, 0) / known.length
  }
  const around = ratio((m) => AROUND.has(m))
  const along = ratio((m) => !AROUND.has(m) && !ANGLES.has(m))
  const out: Record<string, number> = {}
  for (const m of missing) {
    const v = size[m]
    if (v === undefined) continue
    out[m] = ANGLES.has(m) ? v : Math.round(v * (AROUND.has(m) ? around : along))
  }
  return out
}

/**
 * The stretches of a piece's stitching line that are hems: where
 * FreeSewing's allowance runs at least twice as far outside it as a
 * seam's, as deep all along (a French seam's is half as deep again, a
 * hem's two or three times), but not along its fold, which has none. And
 * the hems' allowance, mm.
 */
function hemsOf(
  seam: FsPath,
  allowance: FsPath[],
  sa: number,
  fold: FsPoint[],
): { hems: FsPath[]; hemAllowance: number | null } {
  if (allowance.length === 0 || sa <= 0) return { hems: [], hemAllowance: null }
  const finite = (p: FsPoint | undefined): p is FsPoint =>
    p !== undefined && Number.isFinite(p.x) && Number.isFinite(p.y)
  // The allowance lines, sampled every few millimetres, as short lines.
  const lines: [FsPoint, FsPoint][] = allowance.flatMap((path) =>
    path.divide().flatMap((part) => {
      const n = Math.max(2, Math.ceil(part.length() / 3))
      const points = Array.from({ length: n + 1 }, (_, k) =>
        part.shiftFractionAlong(k / n, 1),
      ).filter(finite)
      return points.slice(1).map((p, k): [FsPoint, FsPoint] => [points[k]!, p])
    }),
  )
  const away = (p: FsPoint) => {
    let best = Infinity
    for (const [q, r] of lines) {
      const [dx, dy] = [r.x - q.x, r.y - q.y]
      const l = dx * dx + dy * dy
      const t = l > 0 ? Math.max(0, Math.min(1, ((p.x - q.x) * dx + (p.y - q.y) * dy) / l)) : 0
      best = Math.min(best, Math.hypot(q.x + t * dx - p.x, q.y + t * dy - p.y))
    }
    return best
  }
  const [a, b] = fold
  const onFold = (p: FsPoint) => {
    if (!a || !b) return false
    const [dx, dy] = [b.x - a.x, b.y - a.y]
    const l = Math.hypot(dx, dy) || 1
    return Math.abs((p.x - a.x) * dy - (p.y - a.y) * dx) / l < 0.5
  }
  const hems: FsPath[] = []
  const widths: number[] = []
  for (const part of seam.divide()) {
    const [from, to] = [part.start(), part.end()]
    if (!finite(from) || !finite(to) || (onFold(from) && onFold(to))) continue
    // As deep all along: a dart's leg, which the allowance bridges, is far
    // from it at its tip and near at its mouth.
    const depths = [0.25, 0.5, 0.75].map((f) => {
      const at = part.shiftFractionAlong(f, 1)
      return finite(at) ? away(at) : Infinity
    })
    const [least, most] = [Math.min(...depths), Math.max(...depths)]
    if (least >= 2 * sa && most < 1.3 * least) {
      hems.push(part)
      widths.push(depths[1]!)
    }
  }
  widths.sort((x, y) => x - y)
  const middle = widths[Math.floor(widths.length / 2)]
  return { hems, hemAllowance: middle === undefined ? null : Math.round(middle) }
}

/** `f`, or no hems where it fails: a draft never fails for its hems. */
const safely =
  (f: typeof hemsOf) =>
  (...args: Parameters<typeof hemsOf>): ReturnType<typeof hemsOf> => {
    try {
      return f(...args)
    } catch {
      return { hems: [], hemAllowance: null }
    }
  }

const escape = (s: string) =>
  s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;')

/**
 * A design drafted to `measurements` (FreeSewing's names, mm) with `sa`
 * mm of seam allowance, as an SVG in FreeSewing's way.
 */
export async function draftSvg(
  name: DesignName,
  measurements: Record<string, number>,
  sa = 10,
): Promise<string> {
  const Design = await designOf(name)
  const pattern = new Design({
    measurements,
    sa,
    complete: true,
    paperless: false,
    options: OPTIONS[name] ?? {},
  })
  CORRECTIONS[name]?.(pattern)
  pattern.draft()
  const parts = pattern.parts[0] ?? {}
  const cutlist = pattern.setStores[0]?.cutlist ?? {}
  const groups: string[] = []
  let x = 0
  let height = 0
  let count = 0
  for (const [id, part] of Object.entries(parts)) {
    if (part.hidden) continue
    const shown = Object.entries(part.paths).filter(([, p]) => !p.hidden)
    const classOf = (p: FsPath) => String(p.attributes.get('class') || '')
    // Its stitching line: FreeSewing's `seam`, or else a closed line in
    // the fabric's style that isn't the allowance (a hem drawn on its own
    // is in the fabric's style too).
    const fabric = (p: FsPath) => classOf(p).split(' ').includes('fabric')
    const seam =
      shown.find(([k]) => k === 'seam') ??
      shown.find(
        ([, p]) =>
          fabric(p) && !classOf(p).split(' ').includes('sa') && p.asPathstring().endsWith('z'),
      )
    if (!seam) continue
    const grain = shown.find(([k]) => k.includes('grainline'))
    const fold = shown.find(([k]) => k.includes('cutonfold'))
    const outline = [seam[1]]
    // FreeSewing's allowance, in one line or several.
    const allowanceLines = shown
      .filter(([k, p]) => k === 'sa' || classOf(p).split(' ').includes('sa'))
      .map(([, p]) => p)
    const { hems, hemAllowance } = safely(hemsOf)(
      seam[1],
      allowanceLines,
      sa,
      cutlist[id]?.cutOnFold ?? [],
    )
    const boxes = outline.map((p) => p.bbox())
    const x0 = Math.min(...boxes.map((b) => b.topLeft.x))
    const y0 = Math.min(...boxes.map((b) => b.topLeft.y))
    const x1 = Math.max(...boxes.map((b) => b.bottomRight.x))
    const y1 = Math.max(...boxes.map((b) => b.bottomRight.y))
    const cut = cutlist[id]
    // The main fabric's, where it is cut from several.
    const materials = Object.entries(cut?.materials ?? {})
    const [material, how] = materials.find(([m]) => m === 'fabric') ??
      materials[0] ?? ['fabric', []]
    const first = how[0] ?? { cut: 1, identical: false, onFold: false }
    const onFold = first.onFold || (cut?.cutOnFold?.length ?? 0) > 0
    const said = `Cut ${first.cut}${!first.identical && first.cut >= 2 ? ' mirrored' : ''}${onFold ? ' on fold' : ''} from ${material}`
    const line = (d: string, attrs: string) =>
      `<path d="${d}" fill="none" stroke="#000000" stroke-width="0.5" ${attrs}/>`
    // Ids as FreeSewing numbers them, which say nothing: the piece is
    // named by its group.
    const next = () => `fs-${++count}`
    const body = [
      // The stitching line, saying how much allowance to add: Drapewright
      // draws the cutting line itself (FreeSewing's loops at some inner
      // corners).
      line(
        seam[1].asPathstring(),
        `id="${next()}" class="fabric" data-allowance="${sa}"${hemAllowance === null ? '' : ` data-hem-allowance="${hemAllowance}"`}`,
      ),
      // Its hems, where FreeSewing's allowance is deeper, so that they are
      // known for hems.
      ...hems.map((h) => line(h.asPathstring(), `id="${next()}" data-text="hem"`)),
      grain ? line(grain[1].asPathstring(), `id="${next()}" data-text="grainline"`) : '',
      // A fold's bracket is its grainline too, when it says so.
      fold
        ? line(
            fold[1].asPathstring(),
            `id="${next()}" data-text="cutonfold${cut?.grainOrigin === 'cutonfold' || !grain ? ' grainline' : ''}"`,
          )
        : '',
      ...Object.values(part.snippets)
        .filter((s) => s.def === 'notch' || s.def === 'bnotch')
        .map((s) => `<use href="#${s.def}" x="${s.anchor.x}" y="${s.anchor.y}"/>`),
      `<text x="${(x0 + x1) / 2}" y="${(y0 + y1) / 2}" font-size="8">${escape(said)}</text>`,
    ]
    groups.push(
      `<g id="fs-stack-${escape(id)}-part-${escape(id)}" transform="translate(${x - x0} ${-y0})">${body.join('')}</g>`,
    )
    x += x1 - x0 + 50
    height = Math.max(height, y1 - y0)
  }
  if (groups.length === 0) throw new Error('FreeSewing drafted no pieces.')
  const width = Math.max(x - 50, 1)
  height = Math.max(height, 1)
  return [
    `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"`,
    ` width="${width}mm" height="${height}mm" viewBox="0 0 ${width} ${height}">`,
    `<defs><g id="notch"><circle r="1.5" fill="#000000"/></g>`,
    `<g id="bnotch"><circle r="1.5" fill="#000000"/><circle cx="4" r="1.5" fill="#000000"/></g></defs>`,
    ...groups,
    `</svg>`,
  ].join('')
}
