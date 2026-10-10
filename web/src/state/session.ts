// One open project and everything around it: the engine, the IO worker,
// UI state, and the cache of drawable paths per piece.

import type { Engine, ImportResult, Piece, ScaleOption } from '@soom/engine'
import { buildPaths } from '../canvas/paths.ts'
import { type ScenePiece, sceneBounds } from '../canvas/renderer.ts'
import { type ViewportState, fit, initialViewport } from '../canvas/viewport.ts'
import { t } from '../strings/en.ts'
import type { IoClient } from '../workers/io-client.ts'
import { type Store, createStore } from './store.ts'

export interface PendingImport {
  fileName: string
  result: ImportResult
}

export interface Notice {
  kind: 'info' | 'error'
  text: string
}

export interface UiState {
  selection: ReadonlySet<string>
  viewport: ViewportState
  pendingImport: PendingImport | null
  busy: string | null
  notice: Notice | null
}

/** Space left between imported pieces and what is already on the table, mm. */
const IMPORT_GAP = 50

export class Session {
  readonly ui: Store<UiState>
  readonly #scene = new Map<string, ScenePiece>()

  constructor(
    readonly engine: Engine,
    readonly io: IoClient,
  ) {
    this.ui = createStore<UiState>({
      selection: new Set(),
      viewport: initialViewport,
      pendingImport: null,
      busy: null,
      notice: null,
    })
    engine.subscribe((changes) => {
      for (const id of [...changes.changed, ...changes.removed]) this.#scene.delete(id)
      if (changes.removed.length > 0) {
        this.ui.set((s) => ({
          ...s,
          selection: new Set([...s.selection].filter((id) => !changes.removed.includes(id))),
        }))
      }
    })
  }

  /** Every piece with its derived view and canvas paths, in drawing order. */
  scene(): ScenePiece[] {
    return this.engine.document().pieces.map((piece) => {
      let entry = this.#scene.get(piece.id)
      if (!entry) {
        const view = this.engine.pieceView(piece.id)
        if (!view) throw new Error(`No view for piece ${piece.id}`)
        entry = { piece, view, paths: buildPaths(piece, view) }
        this.#scene.set(piece.id, entry)
      }
      return entry
    })
  }

  setViewport(update: (v: ViewportState) => ViewportState): void {
    this.ui.set((s) => ({ ...s, viewport: update(s.viewport) }))
  }

  fitAll(): void {
    const bounds = sceneBounds(this.scene())
    if (bounds) this.setViewport((v) => fit(v, bounds))
  }

  select(ids: Iterable<string>): void {
    this.ui.set((s) => ({ ...s, selection: new Set(ids) }))
  }

  notify(notice: Notice | null): void {
    this.ui.set((s) => ({ ...s, notice }))
  }

  async openPatternFile(file: File): Promise<void> {
    if (!file.name.toLowerCase().endsWith('.svg') && file.type !== 'image/svg+xml') {
      this.notify({ kind: 'error', text: t.importing.unsupported(file.name) })
      return
    }
    this.ui.set((s) => ({ ...s, busy: t.importing.reading(file.name), notice: null }))
    try {
      const bytes = new Uint8Array(await file.arrayBuffer())
      const result = await this.io.call('importSvg', bytes, file.name)
      this.ui.set((s) => ({ ...s, pendingImport: { fileName: file.name, result } }))
    } catch (error) {
      this.notify({
        kind: 'error',
        text: t.importing.failed(error instanceof Error ? error.message : String(error)),
      })
    } finally {
      this.ui.set((s) => ({ ...s, busy: null }))
    }
  }

  cancelImport(): void {
    this.ui.set((s) => ({ ...s, pendingImport: null }))
  }

  /** Adds the pending import's pieces at the chosen scale, beside what is already there. */
  async confirmImport(scale: ScaleOption): Promise<void> {
    const pending = this.ui.get().pendingImport
    if (!pending) return
    let { result } = pending
    if (!result.scale.certain || scale.mmPerUnit !== result.scale.chosen.mmPerUnit) {
      result = await this.io.call('rescaleImport', result, scale, true)
    }
    const pieces = this.#besideExisting(result.pieces)
    const outcome = this.engine.dispatch({ type: 'addPieces', pieces })
    this.ui.set((s) => ({ ...s, pendingImport: null }))
    if (outcome.status === 'rejected') {
      this.notify({ kind: 'error', text: outcome.error.message })
      return
    }
    this.select(outcome.changes.added)
    this.fitAll()
  }

  #besideExisting(pieces: Piece[]): Piece[] {
    const existing = sceneBounds(this.scene())
    if (!existing || pieces.length === 0) return pieces
    const minX = Math.min(...pieces.flatMap((p) => outlineXs(p)))
    const minY = Math.min(...pieces.flatMap((p) => outlineYs(p)))
    const dx = existing.x1 + IMPORT_GAP - minX
    const dy = existing.y0 - minY
    return pieces.map((p) => ({
      ...p,
      placement: { ...p.placement, x: p.placement.x + dx, y: p.placement.y + dy },
    }))
  }
}

function outlinePoints(p: Piece): [number, number][] {
  return [
    p.outline.start,
    ...p.outline.edges.flatMap((e) =>
      e.segments.flatMap((s) => ('c1' in s ? [s.c1, s.c2, s.to] : [s.to])),
    ),
  ]
}

const outlineXs = (p: Piece) => outlinePoints(p).map(([x]) => x)
const outlineYs = (p: Piece) => outlinePoints(p).map(([, y]) => y)

/** Approximate size of a piece's outline (control points included), mm. */
export function outlineSize(p: Piece): { width: number; height: number } {
  const xs = outlineXs(p)
  const ys = outlineYs(p)
  return { width: Math.max(...xs) - Math.min(...xs), height: Math.max(...ys) - Math.min(...ys) }
}
