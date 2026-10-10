// One open project and everything around it: the engine, the IO worker,
// UI state, and the cache of drawable paths per piece.

import type {
  Command,
  DispatchOptions,
  Engine,
  Hit,
  ImportResult,
  Piece,
  ScaleOption,
} from '@soom/engine'
import { buildPaths } from '../canvas/paths.ts'
import { type ScenePiece, sceneBounds } from '../canvas/renderer.ts'
import { type ViewportState, fit, initialViewport } from '../canvas/viewport.ts'
import { t } from '../strings/en.ts'
import type { IoClient } from '../workers/io-client.ts'
import { type Store, createStore } from './store.ts'

export type ToolId = 'select' | 'notch' | 'grainline'

/** A selected edge, the corner at the end of an edge, or a notch. */
export type PartRef =
  | { kind: 'edge' | 'corner'; piece: string; edge: string }
  | { kind: 'notch'; piece: string; notch: string }

/** A grainline being drawn, in the piece's own coordinates. */
export interface GrainPreview {
  piece: string
  from: [number, number]
  to: [number, number]
}

export interface PendingImport {
  fileName: string
  result: ImportResult
}

export interface Notice {
  kind: 'info' | 'error'
  text: string
}

/** Pieces being dragged, drawn offset until the drag ends. */
export interface MovePreview {
  ids: ReadonlySet<string>
  dx: number
  dy: number
}

/** A short measurement shown next to the pointer, screen px. */
export interface Tip {
  x: number
  y: number
  text: string
}

export interface MenuState {
  x: number
  y: number
  hit: Hit | null
}

export interface UiState {
  tool: ToolId
  selection: ReadonlySet<string>
  /** The selected edge or corner, shown in the inspector. */
  part: PartRef | null
  hover: Hit | null
  preview: MovePreview | null
  grainPreview: GrainPreview | null
  viewport: ViewportState
  pendingImport: PendingImport | null
  busy: string | null
  notice: Notice | null
  menu: MenuState | null
  tip: Tip | null
  /** Brief feedback, e.g. after a two-finger undo. */
  toast: string | null
}

/** Space left between imported pieces and what is already on the table, mm. */
const IMPORT_GAP = 50

const TOAST_MS = 1400

export class Session {
  readonly ui: Store<UiState>
  readonly #scene = new Map<string, ScenePiece>()
  #toastTimer: ReturnType<typeof setTimeout> | undefined
  #fitPending = false

  constructor(
    readonly engine: Engine,
    readonly io: IoClient,
  ) {
    this.ui = createStore<UiState>({
      tool: 'select',
      selection: new Set(),
      part: null,
      hover: null,
      preview: null,
      grainPreview: null,
      viewport: initialViewport,
      pendingImport: null,
      busy: null,
      notice: null,
      menu: null,
      tip: null,
      toast: null,
    })
    engine.subscribe((changes) => {
      // Print settings (notch style) change how every piece is drawn.
      if (changes.document) this.#scene.clear()
      for (const id of [...changes.changed, ...changes.removed]) this.#scene.delete(id)
      this.#dropStaleSelection()
    })
  }

  /** Forgets selected pieces and edges that no longer exist. */
  #dropStaleSelection() {
    const doc = this.engine.document()
    const ui = this.ui.get()
    const alive = new Set(doc.pieces.map((p) => p.id))
    const selection = [...ui.selection].filter((id) => alive.has(id))
    const part = ui.part
    const partPiece = part && doc.pieces.find((p) => p.id === part.piece)
    const partAlive =
      !!partPiece &&
      !!part &&
      (part.kind === 'notch'
        ? partPiece.notches.some((n) => n.id === part.notch)
        : partPiece.outline.edges.some((e) => e.id === part.edge))
    if (selection.length !== ui.selection.size || (ui.part && !partAlive)) {
      this.ui.set((s) => ({
        ...s,
        selection: new Set(selection),
        part: partAlive ? s.part : null,
      }))
    }
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

  scenePiece(id: string): ScenePiece | undefined {
    return this.scene().find((s) => s.piece.id === id)
  }

  setViewport(update: (v: ViewportState) => ViewportState): void {
    this.ui.set((s) => ({ ...s, viewport: update(s.viewport) }))
    const v = this.ui.get().viewport
    if (this.#fitPending && v.width > 0 && v.height > 0) {
      this.#fitPending = false
      this.fitAll()
    }
  }

  /** Shows every piece. Waits for the table to have a size if it has none yet. */
  fitAll(): void {
    const v = this.ui.get().viewport
    if (v.width <= 0 || v.height <= 0) {
      this.#fitPending = true
      return
    }
    const bounds = sceneBounds(this.scene())
    if (bounds) this.setViewport((s) => fit(s, bounds))
  }

  setTool(tool: ToolId): void {
    this.ui.set((s) => ({ ...s, tool, preview: null, grainPreview: null, hover: null, tip: null }))
  }

  setGrainPreview(grainPreview: GrainPreview | null): void {
    this.ui.set((s) => ({ ...s, grainPreview }))
  }

  select(ids: Iterable<string>, part: PartRef | null = null): void {
    this.ui.set((s) => ({ ...s, selection: new Set(ids), part }))
  }

  setHover(hover: Hit | null): void {
    const current = this.ui.get().hover
    if (JSON.stringify(current) === JSON.stringify(hover)) return
    this.ui.set((s) => ({ ...s, hover }))
  }

  setPreview(preview: MovePreview | null): void {
    this.ui.set((s) => ({ ...s, preview }))
  }

  showTip(tip: Tip | null): void {
    this.ui.set((s) => ({ ...s, tip }))
  }

  openMenu(menu: MenuState | null): void {
    this.ui.set((s) => ({ ...s, menu }))
  }

  notify(notice: Notice | null): void {
    this.ui.set((s) => ({ ...s, notice }))
  }

  toast(text: string): void {
    clearTimeout(this.#toastTimer)
    this.ui.set((s) => ({ ...s, toast: text }))
    this.#toastTimer = setTimeout(() => this.ui.set((s) => ({ ...s, toast: null })), TOAST_MS)
  }

  /** Runs a command; shows its plain-language reason if it is refused. */
  dispatch(command: Command, options?: DispatchOptions): boolean {
    const result = this.engine.dispatch(command, options)
    if (result.status === 'rejected') {
      this.notify({ kind: 'error', text: result.error.message })
      return false
    }
    return true
  }

  undo(feedback = false): void {
    const label = this.engine.state.undoLabel
    if (this.engine.undo() && feedback && label) this.toast(t.actions.undoThing(label))
  }

  redo(feedback = false): void {
    const label = this.engine.state.redoLabel
    if (this.engine.redo() && feedback && label) this.toast(t.actions.redoThing(label))
  }

  /** Moves the selected pieces; a run of nudges is one undo step. */
  nudge(dx: number, dy: number): void {
    const ids = [...this.ui.get().selection].sort()
    if (ids.length === 0) return
    this.dispatch({ type: 'movePieces', ids, dx, dy }, { coalesce: `nudge:${ids.join(',')}` })
  }

  deleteSelection(): void {
    const ids = [...this.ui.get().selection]
    if (ids.length > 0) this.dispatch({ type: 'deletePieces', ids })
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
