// Turning pieces so their grainline runs straight up and down the table, as
// a pattern drawn a little askew on its sheet comes in with each piece's
// grain a few degrees off. Placement only: a piece's shape never changes.

import type { Command, Piece } from '@drapewright/engine'

/** What straightening needs of a piece: its grain, where it lies, its cutting line. */
export interface Straightenable {
  piece: Pick<Piece, 'id' | 'name' | 'placement' | 'grainline'>
  /** The cutting line, `x0, y0, x1, y1, …` in the piece's own mm, closed. */
  view: { cutting: number[] }
}

/** Turns under this, in degrees, are none: the piece is straight already. */
const STRAIGHT = 0.01

/**
 * The turn (degrees, clockwise) that brings a piece's grainline upright on
 * the table, the nearer of the two ways up: between -90 and 90. `null` for a
 * piece with no grainline.
 */
export function uprightTurn(piece: Straightenable['piece']): number | null {
  const g = piece.grainline
  if (!g) return null
  const [fx, fy] = g.from
  const [tx, ty] = g.to
  if (fx === tx && fy === ty) return null
  const along = (Math.atan2(ty - fy, tx - fx) * 180) / Math.PI + piece.placement.rotation
  return within90(90 - along)
}

/** The same turn, give or take half a turn, between -90 and 90. */
function within90(degrees: number): number {
  const d = (((degrees % 180) + 270) % 180) - 90
  return Math.abs(d) < STRAIGHT ? 0 : d
}

/**
 * A turn as the field shows it: either way from as drawn, more than -180 up
 * to 180 (a piece straightened 3° back reads -3, not 357), to a tenth of a
 * degree.
 */
export function shownTurn(rotation: number): number {
  const r = Math.round(((((rotation % 360) + 540) % 360) - 180) * 10) / 10
  return r === -180 ? 180 : r === 0 ? 0 : r
}

export interface Straightening {
  /** The turns, one undo step together. */
  commands: Command[]
  /** Names of the pieces with no grainline to straighten to. */
  noGrain: string[]
  /** How many were straight already. */
  straight: number
}

/**
 * Turns each piece upright on its grainline. Pieces laid over each other
 * (a pattern's sizes, nested as printed) turn about their shared middle, so
 * they stay nested; any other piece turns about its own.
 */
export function straighten(pieces: readonly Straightenable[]): Straightening {
  const { turns, noGrain, straight } = plan(pieces)
  const commands: Command[] = turns.map(({ item, degrees, about }) =>
    about
      ? { type: 'rotatePiece', piece: item.piece.id, degrees, about }
      : { type: 'rotatePiece', piece: item.piece.id, degrees },
  )
  return { commands, noGrain, straight }
}

/** A piece drawn this far askew, or less, was meant to be upright: a bias piece is not. */
export const ASKEW = 10

/** Whether a piece's grainline is a little off upright (not straight, not on the bias). */
export function askew(piece: Straightenable['piece']): boolean {
  const turn = uprightTurn(piece)
  return turn !== null && turn !== 0 && Math.abs(turn) <= ASKEW
}

/**
 * Pieces not yet on the table (as found in a file), those drawn a little
 * askew placed upright on their grainline, stacks turned together as
 * `straighten` turns them. The others are as they were.
 */
export function straightenedPieces<P extends Straightenable['piece']>(
  pieces: readonly P[],
  points: (piece: P) => number[],
): P[] {
  const items = pieces.filter(askew).map((piece) => ({ piece, view: { cutting: points(piece) } }))
  const placed = new Map<string, Piece['placement']>()
  for (const { item, degrees, about } of plan(items).turns) {
    const box = tableBox(item)
    const [px, py] = about ?? [(box.x0 + box.x1) / 2, (box.y0 + box.y1) / 2]
    placed.set(item.piece.id, turnedAbout(item.piece.placement, degrees, px, py))
  }
  return pieces.map((p) => {
    const placement = placed.get(p.id)
    return placement ? { ...p, placement } : p
  })
}

/** A placement turned `degrees` clockwise about a point on the table. */
function turnedAbout(
  placement: Piece['placement'],
  degrees: number,
  px: number,
  py: number,
): Piece['placement'] {
  const a = (degrees * Math.PI) / 180
  const cos = Math.cos(a)
  const sin = Math.sin(a)
  const dx = placement.x - px
  const dy = placement.y - py
  return {
    x: px + dx * cos - dy * sin,
    y: py + dx * sin + dy * cos,
    rotation: (((placement.rotation + degrees) % 360) + 360) % 360,
  }
}

interface Turn {
  item: Straightenable
  degrees: number
  /** About this point on the table; or, with none, the piece's own middle. */
  about: [number, number] | null
}

function plan(pieces: readonly Straightenable[]): {
  turns: Turn[]
  noGrain: string[]
  straight: number
} {
  const noGrain: string[] = []
  let straight = 0
  const turning: { item: Straightenable; turn: number; box: Box }[] = []
  for (const item of pieces) {
    const turn = uprightTurn(item.piece)
    if (turn === null) noGrain.push(item.piece.name)
    else if (turn === 0) straight++
    else turning.push({ item, turn, box: tableBox(item) })
  }
  const turns: Turn[] = []
  for (const group of stacks(turning)) {
    const [first] = group
    if (!first) continue
    if (group.length === 1) {
      turns.push({ item: first.item, degrees: first.turn, about: null })
      continue
    }
    const box = group.map((g) => g.box).reduce(union)
    const about: [number, number] = [(box.x0 + box.x1) / 2, (box.y0 + box.y1) / 2]
    for (const { item, turn } of group) {
      // All of a stack the same way up: half a turn round from the first's
      // choice is the other end of the same grain.
      const degrees = Math.abs(turn - first.turn) > 90 ? turn - 180 * Math.sign(turn) : turn
      turns.push({ item, degrees, about })
    }
  }
  return { turns, noGrain, straight }
}

interface Box {
  x0: number
  y0: number
  x1: number
  y1: number
}

function tableBox({ piece, view }: Straightenable): Box {
  const a = (piece.placement.rotation * Math.PI) / 180
  const cos = Math.cos(a)
  const sin = Math.sin(a)
  const box = { x0: Infinity, y0: Infinity, x1: -Infinity, y1: -Infinity }
  for (let i = 0; i + 1 < view.cutting.length; i += 2) {
    const x = view.cutting[i]!
    const y = view.cutting[i + 1]!
    const tx = piece.placement.x + x * cos - y * sin
    const ty = piece.placement.y + x * sin + y * cos
    box.x0 = Math.min(box.x0, tx)
    box.y0 = Math.min(box.y0, ty)
    box.x1 = Math.max(box.x1, tx)
    box.y1 = Math.max(box.y1, ty)
  }
  return box
}

const union = (a: Box, b: Box): Box => ({
  x0: Math.min(a.x0, b.x0),
  y0: Math.min(a.y0, b.y0),
  x1: Math.max(a.x1, b.x1),
  y1: Math.max(a.y1, b.y1),
})

/** One piece's middle inside the other's box: laid over each other, not beside. */
function stacked(a: Box, b: Box): boolean {
  const inside = (p: Box, q: Box) => {
    const x = (p.x0 + p.x1) / 2
    const y = (p.y0 + p.y1) / 2
    return x > q.x0 && x < q.x1 && y > q.y0 && y < q.y1
  }
  return inside(a, b) || inside(b, a)
}

/** The pieces in groups laid over each other, in the order given. */
function stacks<T extends { box: Box }>(items: T[]): T[][] {
  const group = items.map((_, i) => i)
  const root = (i: number): number => (group[i] === i ? i : (group[i] = root(group[i]!)))
  for (let i = 0; i < items.length; i++) {
    for (let j = i + 1; j < items.length; j++) {
      if (stacked(items[i]!.box, items[j]!.box)) group[root(j)] = root(i)
    }
  }
  const out = new Map<number, T[]>()
  items.forEach((item, i) => {
    const r = root(i)
    out.set(r, [...(out.get(r) ?? []), item])
  })
  return [...out.values()]
}
