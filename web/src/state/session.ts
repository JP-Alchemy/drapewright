// One open project and everything around it: the engine, the IO worker,
// UI state, and the cache of drawable paths per piece.

import type {
  AllowanceAnswer,
  Command,
  DispatchOptions,
  Engine,
  FabricPreset,
  Hit,
  ImportResult,
  Piece,
  ScaleOption,
} from '@drapewright/engine'
import { buildPaths } from '../canvas/paths.ts'
import { type ScenePiece, sceneBounds } from '../canvas/renderer.ts'
import { NOMINAL_PX_PER_MM, type ViewportState, fit, initialViewport } from '../canvas/viewport.ts'
import { t } from '../strings/en.ts'
import { saveScreenScale } from '../platform/screen.ts'
import type { Projects } from '../storage/projects.ts'
import type { IoClient } from '../workers/io-client.ts'
import { type Store, createStore } from './store.ts'

export type ToolId = 'select' | 'notch' | 'grainline' | 'pick'

/** What points are tapped on a piece for: an alteration placed by them. */
export type PickFor =
  'spread' | 'bust' | 'pivot' | 'swayback' | 'shoulder' | 'waist' | 'hip' | 'trueUp'

/** Points being tapped on a piece (its own coordinates), and what for. */
export interface Picking {
  piece: string
  for: PickFor
  points: [number, number][]
  /** A pivot's dart, once known. */
  dart?: string
}

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
  /** The file itself, kept with the project once the pieces are added. */
  bytes: Uint8Array
  /** A PDF's pages she chose to read (from 1); null for those read by default. */
  pages: number[] | null
}

type Importer = 'importSvg' | 'importDxf' | 'importPdf'

/** The pattern files Drapewright reads, by file extension and media type. */
const PATTERN_FORMATS: {
  importer: Importer
  extensions: string[]
  types: string[]
}[] = [
  { importer: 'importSvg', extensions: ['.svg'], types: ['image/svg+xml'] },
  {
    importer: 'importDxf',
    extensions: ['.dxf'],
    types: ['image/vnd.dxf', 'image/x-dxf', 'application/dxf'],
  },
  { importer: 'importPdf', extensions: ['.pdf'], types: ['application/pdf'] },
]

/** For a file input's `accept`. */
export const PATTERN_FILE_TYPES = PATTERN_FORMATS.flatMap((f) => [
  ...f.extensions,
  ...f.types,
]).join(',')

function importerFor(file: File): Importer | null {
  const name = file.name.toLowerCase()
  const format = PATTERN_FORMATS.find(
    (f) => f.extensions.some((e) => name.endsWith(e)) || f.types.includes(file.type),
  )
  return format?.importer ?? null
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
  /** Points being tapped for an alteration, with the pick tool. */
  picking: Picking | null
  viewport: ViewportState
  pendingImport: PendingImport | null
  busy: string | null
  notice: Notice | null
  menu: MenuState | null
  tip: Tip | null
  /** Brief feedback, e.g. after a two-finger undo. */
  toast: string | null
  /** Whether this screen's true size has been measured (see `setScreenScale`). */
  screenMeasured: boolean
  /** The seam picked in the seam list, drawn stronger. */
  seam: string | null
  /** An edge waiting for the edge it is to be sewn to: the next edge tapped. */
  sewing: { piece: string; edge: string } | null
  /** The 3D view of the avatar is open beside the table. */
  avatar: boolean
  /** Projecting the pieces onto fabric, full screen. */
  projector: boolean
  /** Whose measurements the avatar is fitted to: a measurement set's id. */
  avatarPerson: string | null
  /** The fabric the garment is draped on the avatar in: ready made, or one's own (by id). */
  drapeFabric: FabricPreset | string
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
    screenScale: number | null = null,
    readonly projects: Projects | null = null,
  ) {
    this.ui = createStore<UiState>({
      tool: 'select',
      selection: new Set(),
      part: null,
      hover: null,
      preview: null,
      grainPreview: null,
      picking: null,
      viewport: { ...initialViewport, pxPerMm: screenScale ?? NOMINAL_PX_PER_MM },
      pendingImport: null,
      busy: null,
      notice: null,
      menu: null,
      tip: null,
      toast: null,
      screenMeasured: screenScale !== null,
      seam: null,
      sewing: null,
      avatar: false,
      projector: false,
      avatarPerson: null,
      drapeFabric: 'poplin',
    })
    // Back where the project was left; a project saved without a view
    // (the engine's default) shows all its pieces instead.
    const view = engine.viewState()
    const unset = view.center[0] === 0 && view.center[1] === 0 && view.zoom === 0.5
    if (!unset) {
      this.ui.set((st) => ({
        ...st,
        viewport: { ...st.viewport, cx: view.center[0], cy: view.center[1], zoom: view.zoom },
      }))
    } else if (engine.document().pieces.length > 0) {
      this.#fitPending = true
    }
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
    if (ui.seam && !doc.seams.some((s) => s.id === ui.seam)) {
      this.ui.set((s) => ({ ...s, seam: null }))
    }
    const sewing = ui.sewing
    if (sewing && !doc.pieces.some((p) => p.id === sewing.piece)) {
      this.ui.set((s) => ({ ...s, sewing: null }))
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

  /** Puts the current view into the project, so it opens where it was left. */
  syncView(): void {
    const { viewport: v, selection } = this.ui.get()
    this.engine.setViewState({ center: [v.cx, v.cy], zoom: v.zoom, selection: [...selection] })
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
    this.ui.set((s) => ({
      ...s,
      tool,
      preview: null,
      grainPreview: null,
      hover: null,
      tip: null,
      picking: tool === 'pick' ? s.picking : null,
    }))
  }

  /** Starts tapping points on a piece to place an alteration. */
  startPicking(piece: string, kind: PickFor): void {
    this.select([piece])
    // A piece with one dart: that is the dart to move.
    const darts = this.engine.document().pieces.find((p) => p.id === piece)?.darts ?? []
    const dart = kind === 'pivot' && darts.length === 1 ? darts[0]!.id : undefined
    this.ui.set((s) => ({ ...s, tool: 'pick', picking: { piece, for: kind, points: [], dart } }))
  }

  /** The points tapped so far, as they change. */
  setPicking(picking: Picking | null): void {
    this.ui.set((s) => ({ ...s, picking, tool: picking ? 'pick' : 'select' }))
  }

  setGrainPreview(grainPreview: GrainPreview | null): void {
    this.ui.set((s) => ({ ...s, grainPreview }))
  }

  select(ids: Iterable<string>, part: PartRef | null = null): void {
    if (this.ui.get().sewing && part?.kind === 'edge') this.#sewTo(part.piece, part.edge)
    this.ui.set((s) => ({ ...s, selection: new Set(ids), part }))
  }

  selectSeam(seam: string | null): void {
    this.ui.set((s) => ({ ...s, seam }))
  }

  /** Waits for the edge to sew this one to: the next edge tapped. */
  startSewing(piece: string, edge: string): void {
    this.ui.set((s) => ({ ...s, sewing: { piece, edge } }))
  }

  cancelSewing(): void {
    this.ui.set((s) => ({ ...s, sewing: null }))
  }

  /**
   * Sews the waiting edge to this one, the right way round. The same edge
   * again, on a piece cut as a pair, is sewn to its other half.
   */
  #sewTo(piece: string, edge: string): void {
    const from = this.ui.get().sewing
    if (!from) return
    this.cancelSewing()
    const doc = this.engine.document()
    const same = from.piece === piece && from.edge === edge
    const pair = doc.pieces
      .find((p) => p.id === piece)
      ?.cut.some((c) => c.quantity >= 2 && c.mirrored !== false)
    if (same && !pair) return
    const a = { piece: from.piece, edge: from.edge, from: null, to: null, mirror: false }
    const b = { piece, edge, from: null, to: null, mirror: same }
    const reversed = !same && this.engine.sewnReversed(a, b)
    const sewn = this.dispatch({
      type: 'addSeams',
      seams: [{ id: '', name: '', a, b, reversed, ease: 'even', confirmed: true }],
    })
    if (sewn) {
      this.selectSeam(this.engine.document().seams.at(-1)?.id ?? null)
      this.toast(t.seams.sewn)
    }
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

  /**
   * Sets how many CSS px make a millimetre on this screen (`null`: back to
   * the nominal 96 to the inch). The zoom stays, so 100% is true size.
   */
  setScreenScale(pxPerMm: number | null): void {
    saveScreenScale(pxPerMm)
    this.ui.set((s) => ({
      ...s,
      viewport: { ...s.viewport, pxPerMm: pxPerMm ?? NOMINAL_PX_PER_MM },
      screenMeasured: pxPerMm !== null,
    }))
  }

  /** Projects the pieces onto fabric, full screen, or stops. */
  showProjector(open: boolean): void {
    this.ui.set((s) => ({ ...s, projector: open }))
  }

  /** Opens or closes the 3D view of the avatar. */
  showAvatar(open: boolean): void {
    this.ui.set((s) => ({ ...s, avatar: open }))
  }

  /** Fits the avatar to this person's measurements. */
  chooseAvatarPerson(id: string | null): void {
    this.ui.set((s) => ({ ...s, avatarPerson: id }))
  }

  /** Drapes the garment on the avatar in this fabric from now on. */
  chooseDrapeFabric(fabric: FabricPreset | string): void {
    this.ui.set((s) => ({ ...s, drapeFabric: fabric }))
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
    const importer = importerFor(file)
    if (!importer) {
      this.notify({ kind: 'error', text: t.importing.unsupported(file.name) })
      return
    }
    this.ui.set((s) => ({ ...s, busy: t.importing.reading(file.name), notice: null }))
    try {
      const bytes = new Uint8Array(await file.arrayBuffer())
      const result = await this.io.call(importer, bytes, file.name)
      this.ui.set((s) => ({
        ...s,
        pendingImport: { fileName: file.name, result, bytes, pages: null },
      }))
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

  /**
   * Reads the pending PDF again, only the pages she chose (numbered from 1),
   * or all of them as at first (null): when a page of pieces was left out,
   * or a page of instructions taken for one.
   */
  async readPages(pages: number[] | null): Promise<void> {
    const pending = this.ui.get().pendingImport
    if (!pending) return
    this.ui.set((s) => ({ ...s, busy: t.importing.reading(pending.fileName) }))
    try {
      const result = await this.io.call('importPdf', pending.bytes, pending.fileName, pages)
      this.ui.set((s) => ({ ...s, pendingImport: { ...pending, result, pages } }))
    } catch (error) {
      this.notify({
        kind: 'error',
        text: t.importing.failed(error instanceof Error ? error.message : String(error)),
      })
    } finally {
      this.ui.set((s) => ({ ...s, busy: null }))
    }
  }

  /**
   * Adds the pending import's pieces beside what is already there: the
   * sizes she chose, at the scale she confirmed and, when she was asked,
   * with her answer about seam allowance.
   */
  async confirmImport(
    scale: ScaleOption,
    allowance: AllowanceAnswer | null,
    sizes: string[] | null = null,
    keep: string[] | null = null,
  ): Promise<void> {
    const pending = this.ui.get().pendingImport
    if (!pending) return
    let result: ImportResult
    try {
      result = await this.io.call('finishImport', pending.result, sizes, scale, allowance)
    } catch (error) {
      this.notify({
        kind: 'error',
        text: t.importing.failed(error instanceof Error ? error.message : String(error)),
      })
      return
    }
    // Pieces she left out (by id, which the answers don't change) stay out.
    const chosen = keep ? result.pieces.filter((p) => keep.includes(p.id)) : result.pieces
    const pieces = this.#besideExisting(chosen)
    const outcome = this.engine.dispatch({ type: 'addPieces', pieces })
    this.ui.set((s) => ({ ...s, pendingImport: null }))
    if (outcome.status === 'rejected') {
      this.notify({ kind: 'error', text: outcome.error.message })
      return
    }
    this.projects?.keepSource(
      this.engine.document().meta.id,
      result.sha256,
      pending.fileName,
      pending.bytes,
    )
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
