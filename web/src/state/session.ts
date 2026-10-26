// One open project and everything around it: the engine, the IO worker,
// UI state, and the cache of drawable paths per piece.

import type {
  AllowanceAnswer,
  Change,
  Command,
  CutInstruction,
  DispatchOptions,
  EdgeMove,
  Engine,
  FabricPreset,
  Hit,
  ImportResult,
  PhotoRequest,
  Piece,
  ScaleOption,
  Seam,
  SeamSide,
  Traced,
} from '@drapewright/engine'
import { buildPaths } from '../canvas/paths.ts'
import { type ScenePiece, sceneBounds, sidesBounds } from '../canvas/renderer.ts'
import {
  NOMINAL_PX_PER_MM,
  type ViewportState,
  fit,
  initialViewport,
  reveal,
} from '../canvas/viewport.ts'
import { t } from '../strings/en.ts'
import { saveScreenScale } from '../platform/screen.ts'
import type { Projects } from '../storage/projects.ts'
import type { IoClient } from '../workers/io-client.ts'
import { type EdgeRef, edgeLine } from './edge-line.ts'
import { type Store, createStore } from './store.ts'

export type { EdgeRef } from './edge-line.ts'

export type ToolId = 'select' | 'notch' | 'grainline' | 'pick'

/** What points are tapped on a piece for: an alteration placed by them. */
export type PickFor =
  | 'spread'
  | 'bust'
  | 'pivot'
  | 'swayback'
  | 'shoulder'
  | 'bustSide'
  | 'waist'
  | 'hip'
  | 'trueUp'
  | 'moveEdge'

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

/** One of the handles beside an edge's length, to drag the edge by. */
export interface EdgeHandleRef {
  piece: string
  /** The edge, or for a corner the edge it ends. */
  edge: string
  kind: EdgeMove | 'corner'
  /** Being dragged, not only under the pointer. */
  dragging: boolean
}

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

/** A box dragged across the table to choose the pieces it touches, mm. */
export interface Marquee {
  from: { x: number; y: number }
  to: { x: number; y: number }
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
  /** Pressed on one of several chosen pieces: the menu is for all of them. */
  pieces?: string[]
}

export interface UiState {
  tool: ToolId
  selection: ReadonlySet<string>
  /** The selected edge or corner, shown in the inspector. */
  part: PartRef | null
  hover: Hit | null
  /** The edge handle under the pointer, or being dragged. */
  edgeHandle: EdgeHandleRef | null
  /**
   * Other edges chosen with the part's (an edge), on its piece or others:
   * moved together with it.
   */
  moreEdges: EdgeRef[]
  preview: MovePreview | null
  marquee: Marquee | null
  grainPreview: GrainPreview | null
  /** Points being tapped for an alteration, with the pick tool. */
  picking: Picking | null
  viewport: ViewportState
  pendingImport: PendingImport | null
  /** A photo of a paper pattern being read. */
  photo: File | null
  busy: string | null
  notice: Notice | null
  menu: MenuState | null
  tip: Tip | null
  /** Brief feedback, e.g. after a two-finger undo. */
  toast: string | null
  /** Whether this screen's true size has been measured (see `setScreenScale`). */
  screenMeasured: boolean
  /**
   * The seam picked (under a piece in the list, or from an edge it sews):
   * its edges drawn stronger and joined by a line across the table.
   */
  seam: string | null
  /** A suggested seam shown on the table, its edges joined by a dashed line. */
  suggestion: Seam | null
  /** An edge waiting for the edge it is to be sewn to: the next edge tapped. */
  sewing: { piece: string; edge: string } | null
  /** A piece (a pocket) waiting to be sewn onto a line on another: the next tapped. */
  sewingOnto: string | null
  /** The 3D view of the avatar is open beside the table. */
  avatar: boolean
  /** Projecting the pieces onto fabric, full screen. */
  projector: boolean
  /** Choosing a FreeSewing design to draft. */
  drafting: boolean
  /** Whose measurements the avatar is fitted to: a measurement set's id. */
  avatarPerson: string | null
  /** The fabric the garment is draped on the avatar in: ready made, or one's own (by id). */
  drapeFabric: FabricPreset | string
}

/** Space left between imported pieces and what is already on the table, mm. */
const IMPORT_GAP = 50

const TOAST_MS = 1400

/** What a piece that says nothing about cutting is cut as. */
const MAIN_ONCE: CutInstruction = { material: 'main', quantity: 1, mirrored: null, onFold: null }

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
      edgeHandle: null,
      moreEdges: [],
      preview: null,
      marquee: null,
      grainPreview: null,
      picking: null,
      viewport: { ...initialViewport, pxPerMm: screenScale ?? NOMINAL_PX_PER_MM },
      pendingImport: null,
      photo: null,
      busy: null,
      notice: null,
      menu: null,
      tip: null,
      toast: null,
      screenMeasured: screenScale !== null,
      seam: null,
      suggestion: null,
      sewing: null,
      sewingOnto: null,
      avatar: false,
      projector: false,
      drafting: false,
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
    const there = (r: EdgeRef) =>
      doc.pieces.find((p) => p.id === r.piece)?.outline.edges.some((e) => e.id === r.edge) ?? false
    const moreEdges = ui.moreEdges.filter(there)
    if (
      selection.length !== ui.selection.size ||
      (ui.part && !partAlive) ||
      moreEdges.length !== ui.moreEdges.length
    ) {
      this.ui.set((s) => ({
        ...s,
        selection: new Set(selection),
        part: partAlive ? s.part : null,
        moreEdges: partAlive ? moreEdges : [],
      }))
    }
    if (ui.seam && !doc.seams.some((s) => s.id === ui.seam)) {
      this.ui.set((s) => ({ ...s, seam: null }))
    }
    const shown = ui.suggestion
    const edgeAlive = (side: SeamSide) =>
      doc.pieces.some(
        (p) =>
          p.id === side.piece &&
          (side.line
            ? p.lines.some((l) => l.id === side.line)
            : p.outline.edges.some((e) => e.id === side.edge)),
      )
    if (shown && !(edgeAlive(shown.a) && edgeAlive(shown.b))) {
      this.ui.set((s) => ({ ...s, suggestion: null }))
    }
    const sewing = ui.sewing
    if (sewing && !doc.pieces.some((p) => p.id === sewing.piece)) {
      this.ui.set((s) => ({ ...s, sewing: null }))
    }
    const onto = ui.sewingOnto
    if (onto && !doc.pieces.some((p) => p.id === onto)) {
      this.ui.set((s) => ({ ...s, sewingOnto: null }))
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
      marquee: null,
      grainPreview: null,
      hover: null,
      edgeHandle: null,
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
    if (this.ui.get().sewing && part?.kind === 'edge' && this.#sewTo(part.piece, part.edge)) return
    const selection = new Set(ids)
    const [only] = selection.size === 1 ? selection : []
    const seams = this.engine.document().seams
    this.ui.set((s) => {
      const picked = seams.find((seam) => seam.id === s.seam)
      return {
        ...s,
        selection,
        part,
        moreEdges: [],
        // The picked seam stays while one of its pieces is chosen on its
        // own, none of its parts chosen: moved, say, to lie by the other.
        seam: picked && part === null && only !== undefined && sews(picked, only) ? s.seam : null,
        suggestion: selection.size > 0 ? s.suggestion : null,
      }
    })
  }

  /** The edges chosen: the part's, if an edge, then the others chosen with it. */
  chosenEdges(): EdgeRef[] {
    const { part, moreEdges } = this.ui.get()
    return part?.kind === 'edge' ? [{ piece: part.piece, edge: part.edge }, ...moreEdges] : []
  }

  /**
   * Chooses these edges together, their pieces with them: `primary` the
   * one whose details show (and whose handles show on the table).
   */
  chooseEdges(edges: EdgeRef[], primary: EdgeRef): void {
    const same = (a: EdgeRef) => a.piece === primary.piece && a.edge === primary.edge
    const others = edges.filter(
      (e, i) => !same(e) && edges.findIndex((f) => f.piece === e.piece && f.edge === e.edge) === i,
    )
    this.select([primary.piece, ...others.map((e) => e.piece)], { kind: 'edge', ...primary })
    this.ui.set((s) => ({ ...s, moreEdges: others }))
  }

  /** Adds an edge to those chosen; one chosen already, takes it away. */
  toggleEdge(edge: EdgeRef): void {
    const same = (a: EdgeRef) => a.piece === edge.piece && a.edge === edge.edge
    const chosen = this.chosenEdges()
    if (!chosen.some(same)) {
      this.chooseEdges([...chosen, edge], edge)
      return
    }
    const rest = chosen.filter((e) => !same(e))
    if (rest.length > 0) this.chooseEdges(rest, rest.at(-1)!)
    else this.select([edge.piece])
  }

  /** Chooses the line round the garment that `edge` is part of; `adding` it to those chosen. */
  chooseLine(edge: EdgeRef, adding = false): void {
    const length = (side: { piece: string; edge: string }) =>
      this.scenePiece(side.piece)?.view.edges.find((e) => e.id === side.edge)?.length ?? null
    const line = edgeLine(this.engine.document(), edge, length)
    this.chooseEdges(adding ? [...this.chosenEdges(), ...line] : line, edge)
  }

  /**
   * Picks a seam on one of its pieces (`piece`; else the piece chosen, if
   * it is one of them; else its first side's), which is chosen: its edges
   * are drawn joined, and the view moves if it must to show them both.
   * Null puts the picked seam down.
   */
  selectSeam(seam: string | null, piece?: string): void {
    const picked = this.engine.document().seams.find((s) => s.id === seam)
    if (!picked) {
      this.ui.set((s) => ({ ...s, seam: null }))
      return
    }
    const { selection } = this.ui.get()
    const [only] = selection.size === 1 ? selection : []
    const on = piece ?? (only !== undefined && sews(picked, only) ? only : picked.a.piece)
    this.ui.set((s) => ({
      ...s,
      selection: new Set([on]),
      part: null,
      seam: picked.id,
      suggestion: null,
    }))
    this.#show([picked.a, picked.b])
  }

  /**
   * Shows a suggested seam on the table, its edges joined by a dashed line,
   * moving the view if it must to show them both; null stops showing it.
   */
  showSuggestion(suggestion: Seam | null): void {
    this.ui.set((s) => ({ ...s, suggestion, seam: suggestion ? null : s.seam }))
    if (suggestion) this.#show([suggestion.a, suggestion.b])
  }

  /** Moves the view, if it must, to show the edges these sides of a seam sew. */
  #show(sides: SeamSide[]): void {
    const r = sidesBounds(this.scene(), sides)
    if (r) this.setViewport((v) => reveal(v, r))
  }

  /** Waits for the edge to sew this one to: the next edge tapped. */
  startSewing(piece: string, edge: string): void {
    this.ui.set((s) => ({ ...s, sewing: { piece, edge } }))
  }

  cancelSewing(): void {
    this.ui.set((s) => ({ ...s, sewing: null, sewingOnto: null }))
  }

  /** Waits for the line on another piece to sew this one (a pocket) onto: the next tapped. */
  startSewingOnto(piece: string): void {
    this.ui.set((s) => ({ ...s, sewing: null, sewingOnto: piece }))
  }

  /**
   * Sews the waiting piece onto the line nearest `world` (work-table mm)
   * on `piece`, as it best fits there, all its seams one change, and picks
   * the first to show it sewn. Tapping the waiting piece itself waits on.
   * Whether a piece was waiting.
   */
  sewOntoAt(piece: string, world: { x: number; y: number }): boolean {
    const pocket = this.ui.get().sewingOnto
    if (!pocket) return false
    if (piece === pocket) return true
    this.cancelSewing()
    const doc = this.engine.document()
    const name = doc.pieces.find((p) => p.id === pocket)?.name ?? ''
    const line = this.engine.nearestLine(piece, world.x, world.y)
    const seams = line ? this.engine.pocketSeams(pocket, piece, line) : null
    if (!line) {
      this.toast(t.seams.noLine)
    } else if (!seams || seams.length === 0) {
      this.toast(t.seams.notItsShape(name))
    } else if (this.dispatch({ type: 'addSeams', seams })) {
      const first = this.engine.document().seams.at(-seams.length)
      this.selectSeam(first?.id ?? null, pocket)
      this.toast(t.seams.sewnOn(name))
    }
    return true
  }

  /**
   * Sews the waiting edge to this one, the right way round, and picks the
   * seam to show it sewn. The same edge again, on a piece cut as a pair, is
   * sewn to its other half. Whether it was sewn.
   */
  #sewTo(piece: string, edge: string): boolean {
    const from = this.ui.get().sewing
    if (!from) return false
    this.cancelSewing()
    const doc = this.engine.document()
    const same = from.piece === piece && from.edge === edge
    const pair = doc.pieces
      .find((p) => p.id === piece)
      ?.cut.some((c) => c.quantity >= 2 && c.mirrored !== false)
    if (same && !pair) return false
    const a = {
      piece: from.piece,
      edge: from.edge,
      line: null,
      from: null,
      to: null,
      mirror: false,
    }
    const b = { piece, edge, line: null, from: null, to: null, mirror: same }
    const reversed = !same && this.engine.sewnReversed(a, b)
    const sewn = this.dispatch({
      type: 'addSeams',
      seams: [{ id: '', name: '', a, b, reversed, ease: 'even', confirmed: true }],
    })
    if (sewn) {
      this.selectSeam(this.engine.document().seams.at(-1)?.id ?? null, piece)
      this.toast(t.seams.sewn)
    }
    return sewn
  }

  setHover(hover: Hit | null): void {
    const current = this.ui.get().hover
    if (JSON.stringify(current) === JSON.stringify(hover)) return
    this.ui.set((s) => ({ ...s, hover }))
  }

  setEdgeHandle(edgeHandle: EdgeHandleRef | null): void {
    const current = this.ui.get().edgeHandle
    if (JSON.stringify(current) === JSON.stringify(edgeHandle)) return
    this.ui.set((s) => ({ ...s, edgeHandle }))
  }

  setPreview(preview: MovePreview | null): void {
    this.ui.set((s) => ({ ...s, preview }))
  }

  setMarquee(marquee: Marquee | null): void {
    this.ui.set((s) => ({ ...s, marquee }))
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

  /** Opens or closes the choice of a FreeSewing design to draft. */
  showDrafting(open: boolean): void {
    this.ui.set((s) => ({ ...s, drafting: open }))
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

  /**
   * Adds the same alteration to each of `pieces`, in one undo step, and
   * says so.
   */
  alterEach(pieces: string[], change: Change): void {
    const step = { coalesce: `alter-each:${Date.now()}` }
    for (const piece of pieces) {
      this.dispatch(
        { type: 'addAlteration', piece, alteration: { id: '', on: true, change } },
        step,
      )
    }
    this.toast(t.alter.everyPieceDone(pieces.length))
  }

  /**
   * Sets pieces drawn for one side of the body and cut once (a pattern
   * that draws each panel once) to cut as mirrored pairs, in one undo
   * step, and says so.
   */
  cutOneSideAsPairs(): void {
    const doc = this.engine.document()
    const pieces = this.engine
      .drawnForOneSide()
      .flatMap((id) => doc.pieces.filter((p) => p.id === id))
    if (pieces.length === 0) return
    const step = { coalesce: `pairs:${Date.now()}` }
    for (const piece of pieces) {
      const cut = piece.cut.length > 0 ? piece.cut : [{ ...MAIN_ONCE }]
      this.dispatch(
        {
          type: 'setCut',
          piece: piece.id,
          cut: cut.map((c) => ({ ...c, quantity: 2, mirrored: true })),
        },
        step,
      )
    }
    this.notify({ kind: 'info', text: t.seams.cutAsPairs(pieces.map((p) => p.name)) })
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

  /** Several commands as one undo step; if any is rejected, none is done. */
  dispatchAll(commands: Command[]): boolean {
    const result = this.engine.dispatchAll(commands)
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

  /** Turns each chosen piece about its own middle, together as one undo step. */
  turnSelection(degrees: number): void {
    const ids = [...this.ui.get().selection].sort()
    if (ids.length > 0) {
      this.dispatchAll(ids.map((piece) => ({ type: 'rotatePiece', piece, degrees })))
    }
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

  /**
   * Opens a pattern file, or a photo of a paper pattern (an image that is
   * not a pattern file: an SVG is both).
   */
  openFile(file: File): void {
    if (!importerFor(file) && file.type.startsWith('image/')) this.openPhoto(file)
    else void this.openPatternFile(file)
  }

  /** Opens a photo of a paper pattern piece, to find the piece in it. */
  openPhoto(file: File): void {
    this.ui.set((s) => ({ ...s, photo: file, notice: null }))
  }

  closePhoto(): void {
    this.ui.set((s) => ({ ...s, photo: null }))
  }

  /** Finds a pattern piece in a photo's pixels (RGBA). */
  tracePhoto(
    rgba: Uint8Array,
    width: number,
    height: number,
    bytes: Uint8Array,
    request: PhotoRequest,
  ): Promise<Traced> {
    return this.io.call('tracePhoto', rgba, width, height, bytes, request)
  }

  /**
   * Imports the piece found in a photo, as any pattern file is imported,
   * called `name` (if any).
   */
  usePhoto(traced: Traced, name: string, fileName: string, bytes: Uint8Array): void {
    const result = {
      ...traced.result,
      pieces: traced.result.pieces.map((p) => (name ? { ...p, name } : p)),
    }
    this.ui.set((s) => ({
      ...s,
      photo: null,
      pendingImport: { fileName, result, bytes, pages: null },
    }))
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
    this.addImported(chosen)
  }

  /**
   * Adds pieces made from the file being imported (its sizes blended, say)
   * as its pieces, and closes the import.
   */
  addImported(chosen: Piece[]): boolean {
    const pending = this.ui.get().pendingImport
    if (!pending) return false
    const pieces = this.#besideExisting(chosen)
    const outcome = this.engine.dispatch({ type: 'addPieces', pieces })
    this.ui.set((s) => ({ ...s, pendingImport: null }))
    if (outcome.status === 'rejected') {
      this.notify({ kind: 'error', text: outcome.error.message })
      return false
    }
    this.projects?.keepSource(
      this.engine.document().meta.id,
      pending.result.sha256,
      pending.fileName,
      pending.bytes,
    )
    this.select(outcome.changes.added)
    this.fitAll()
    return true
  }

  /**
   * Adds pieces cut from those on the table (its sizes blended, say)
   * beside them, and chooses them.
   */
  addBeside(pieces: Piece[]): boolean {
    const outcome = this.engine.dispatch({
      type: 'addPieces',
      pieces: this.#besideExisting(pieces),
    })
    if (outcome.status === 'rejected') {
      this.notify({ kind: 'error', text: outcome.error.message })
      return false
    }
    this.select(outcome.changes.added)
    this.fitAll()
    return true
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

/** Whether a seam sews an edge of this piece. */
const sews = (seam: Seam, piece: string) => seam.a.piece === piece || seam.b.piece === piece

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
