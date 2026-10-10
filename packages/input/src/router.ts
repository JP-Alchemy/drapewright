// The input router: turns raw pointer events from pen, touch and mouse
// into navigation (pan, zoom), commands (undo, redo, context menu) and one
// stream of tool events. It has no DOM and no framework: the app feeds it
// plain objects and it calls back, so every rule here is unit-tested.
//
// Rules (docs/proposal.md §3):
// * Pencil edits; fingers navigate and select. Until a Pencil is seen, one
//   finger may edit too (iPads without a Pencil, phones).
// * While a pen is down, and briefly after, touches are palms: ignored.
// * Two fingers pan and pinch. A quick two-finger tap is undo, three is redo.
// * Holding still opens the context menu (touch) or shows measurements (pen).

export type PointerKind = 'pen' | 'touch' | 'mouse'

export interface RawPointer {
  phase: 'down' | 'move' | 'up' | 'cancel' | 'hover'
  id: number
  kind: PointerKind
  /** CSS px, relative to the canvas. */
  x: number
  y: number
  /** Milliseconds, from any monotonic clock. */
  time: number
  /** For `down`: 0 primary, 1 middle, 2 secondary. */
  button?: number
  pressure?: number
  /** Contact size for touches, CSS px. */
  width?: number
  height?: number
  shift?: boolean
  alt?: boolean
  meta?: boolean
  ctrl?: boolean
}

export interface Modifiers {
  shift: boolean
  alt: boolean
  meta: boolean
  ctrl: boolean
}

export interface ToolPointer {
  id: number
  kind: PointerKind
  x: number
  y: number
  pressure: number
  time: number
  modifiers: Modifiers
}

export interface InputHandlers {
  /** Move the view by screen px. */
  pan(dx: number, dy: number): void
  /** Zoom by `factor` about the screen point (cx, cy). */
  zoom(cx: number, cy: number, factor: number): void
  undo(): void
  redo(): void
  toolDown(p: ToolPointer): void
  toolMove(p: ToolPointer): void
  toolUp(p: ToolPointer): void
  /** Whatever the tool was doing must be abandoned (palm, gesture, cancel). */
  toolCancel(): void
  /** A mouse or hovering pen moved with no button down; `null` when it left. */
  hover(p: ToolPointer | null): void
  contextMenu(x: number, y: number, kind: PointerKind): void
  /** A pen or mouse button held still: show measurements. */
  hold(p: ToolPointer): void
}

export interface Scheduler {
  /** Calls `fn` after `ms`; returns a function that cancels it. */
  after(ms: number, fn: () => void): () => void
}

export type FingerEditing = 'auto' | 'always' | 'never'

export const TUNING = {
  /** Travel before a press becomes a drag, CSS px. */
  dragSlop: { pen: 3, touch: 10, mouse: 4 } as Record<PointerKind, number>,
  /** A multi-finger tap: everything down and up again within this, ms. */
  tapMaxMs: 300,
  /** …moving no finger further than this, CSS px. */
  tapSlop: 10,
  longPressMs: 500,
  holdMs: 350,
  /** Touches this soon after the pen lifts are palms, ms. */
  palmWindowMs: 300,
  /** Touches with a contact this large are palms once a pen is known, CSS px. */
  palmContact: 40,
} as const

type Mode =
  | { kind: 'idle' }
  | { kind: 'tool'; id: number; pointerKind: PointerKind; start: RawPointer; moved: boolean }
  | { kind: 'pan'; id: number; x: number; y: number }
  | { kind: 'pendingTouch'; id: number; start: RawPointer; longPressed: boolean }
  | { kind: 'gesture' }

interface Touch {
  x: number
  y: number
  startX: number
  startY: number
}

const modifiers = (e: RawPointer): Modifiers => ({
  shift: !!e.shift,
  alt: !!e.alt,
  meta: !!e.meta,
  ctrl: !!e.ctrl,
})

const toTool = (e: RawPointer): ToolPointer => ({
  id: e.id,
  kind: e.kind,
  x: e.x,
  y: e.y,
  pressure: e.pressure ?? (e.kind === 'pen' ? 0.5 : 1),
  time: e.time,
  modifiers: modifiers(e),
})

export class InputRouter {
  #mode: Mode = { kind: 'idle' }
  readonly #touches = new Map<number, Touch>()
  readonly #ignored = new Set<number>()
  #penSeen = false
  #penDown = false
  #lastPenUp = Number.NEGATIVE_INFINITY
  #panModifier = false
  #cancelTimer: (() => void) | null = null
  #gesture = { start: 0, maxTouches: 0, maxTravel: 0, cx: 0, cy: 0, spread: 0 }

  constructor(
    private readonly handlers: InputHandlers,
    private readonly scheduler: Scheduler,
    private fingerEditing: FingerEditing = 'auto',
  ) {}

  /** Whether a Pencil has been used in this session. */
  get penSeen(): boolean {
    return this.#penSeen
  }

  setFingerEditing(mode: FingerEditing): void {
    this.fingerEditing = mode
  }

  /** Space bar held: a mouse drag pans instead of editing. */
  setPanModifier(held: boolean): void {
    this.#panModifier = held
  }

  #fingersEdit(): boolean {
    return this.fingerEditing === 'always' || (this.fingerEditing === 'auto' && !this.#penSeen)
  }

  #startTimer(ms: number, fn: () => void) {
    this.#stopTimer()
    this.#cancelTimer = this.scheduler.after(ms, () => {
      this.#cancelTimer = null
      fn()
    })
  }

  #stopTimer() {
    this.#cancelTimer?.()
    this.#cancelTimer = null
  }

  handle(e: RawPointer): void {
    if (this.#ignored.has(e.id)) {
      if (e.phase === 'up' || e.phase === 'cancel') this.#ignored.delete(e.id)
      return
    }
    switch (e.kind) {
      case 'pen':
        return this.#pen(e)
      case 'mouse':
        return this.#mouse(e)
      case 'touch':
        return this.#touch(e)
    }
  }

  /** Abandons everything in progress (window lost focus, app hidden…). */
  reset(): void {
    this.#stopTimer()
    if (this.#mode.kind === 'tool') this.handlers.toolCancel()
    this.#mode = { kind: 'idle' }
    for (const id of this.#touches.keys()) this.#ignored.add(id)
    this.#touches.clear()
  }

  // ── Pen ────────────────────────────────────────────────────────────────

  #pen(e: RawPointer) {
    const mode = this.#mode
    switch (e.phase) {
      case 'hover':
        if (mode.kind === 'idle') this.handlers.hover(toTool(e))
        return
      case 'down': {
        this.#penSeen = true
        this.#penDown = true
        // Anything the hand was doing is a palm: drop it.
        if (mode.kind === 'tool') this.handlers.toolCancel()
        for (const id of this.#touches.keys()) this.#ignored.add(id)
        this.#touches.clear()
        this.#stopTimer()
        this.#mode = { kind: 'tool', id: e.id, pointerKind: 'pen', start: e, moved: false }
        this.handlers.toolDown(toTool(e))
        this.#startTimer(TUNING.holdMs, () => {
          const m = this.#mode
          if (m.kind === 'tool' && m.id === e.id && !m.moved) this.handlers.hold(toTool(m.start))
        })
        return
      }
      case 'move':
        if (mode.kind === 'tool' && mode.id === e.id) {
          if (
            !mode.moved &&
            Math.hypot(e.x - mode.start.x, e.y - mode.start.y) > TUNING.dragSlop.pen
          ) {
            mode.moved = true
            this.#stopTimer()
          }
          this.handlers.toolMove(toTool(e))
        } else if (mode.kind === 'idle') {
          this.handlers.hover(toTool(e))
        }
        return
      case 'up':
      case 'cancel':
        this.#penDown = false
        this.#lastPenUp = e.time
        this.#stopTimer()
        if (mode.kind === 'tool' && mode.id === e.id) {
          if (e.phase === 'up') this.handlers.toolUp(toTool(e))
          else this.handlers.toolCancel()
          this.#mode = { kind: 'idle' }
        }
        return
    }
  }

  // ── Mouse ──────────────────────────────────────────────────────────────

  #mouse(e: RawPointer) {
    const mode = this.#mode
    switch (e.phase) {
      case 'hover':
        if (mode.kind === 'idle') this.handlers.hover(toTool(e))
        return
      case 'down':
        if (mode.kind !== 'idle') return
        if (e.button === 2) {
          this.handlers.contextMenu(e.x, e.y, 'mouse')
          return
        }
        if (e.button === 1 || this.#panModifier) {
          this.#mode = { kind: 'pan', id: e.id, x: e.x, y: e.y }
          return
        }
        this.#mode = { kind: 'tool', id: e.id, pointerKind: 'mouse', start: e, moved: false }
        this.handlers.toolDown(toTool(e))
        this.#startTimer(TUNING.holdMs, () => {
          const m = this.#mode
          if (m.kind === 'tool' && m.id === e.id && !m.moved) this.handlers.hold(toTool(m.start))
        })
        return
      case 'move':
        if (mode.kind === 'pan' && mode.id === e.id) {
          this.handlers.pan(e.x - mode.x, e.y - mode.y)
          mode.x = e.x
          mode.y = e.y
        } else if (mode.kind === 'tool' && mode.id === e.id) {
          if (
            !mode.moved &&
            Math.hypot(e.x - mode.start.x, e.y - mode.start.y) > TUNING.dragSlop.mouse
          ) {
            mode.moved = true
            this.#stopTimer()
          }
          this.handlers.toolMove(toTool(e))
        } else if (mode.kind === 'idle') {
          this.handlers.hover(toTool(e))
        }
        return
      case 'up':
      case 'cancel':
        this.#stopTimer()
        if (mode.kind === 'tool' && mode.id === e.id) {
          if (e.phase === 'up') this.handlers.toolUp(toTool(e))
          else this.handlers.toolCancel()
        }
        if ((mode.kind === 'tool' || mode.kind === 'pan') && mode.id === e.id)
          this.#mode = { kind: 'idle' }
        return
    }
  }

  // ── Touch ──────────────────────────────────────────────────────────────

  #isPalm(e: RawPointer): boolean {
    if (this.#penDown || e.time - this.#lastPenUp < TUNING.palmWindowMs) return true
    const contact = Math.max(e.width ?? 0, e.height ?? 0)
    return this.#penSeen && contact > TUNING.palmContact
  }

  #centroid(): { cx: number; cy: number; spread: number } {
    let cx = 0
    let cy = 0
    for (const t of this.#touches.values()) {
      cx += t.x
      cy += t.y
    }
    const n = Math.max(this.#touches.size, 1)
    cx /= n
    cy /= n
    let spread = 0
    for (const t of this.#touches.values()) spread += Math.hypot(t.x - cx, t.y - cy)
    return { cx, cy, spread: spread / n }
  }

  #rebaseGesture() {
    const { cx, cy, spread } = this.#centroid()
    Object.assign(this.#gesture, { cx, cy, spread })
  }

  #touch(e: RawPointer) {
    const mode = this.#mode
    switch (e.phase) {
      case 'hover':
        return
      case 'down': {
        if (this.#isPalm(e)) {
          this.#ignored.add(e.id)
          return
        }
        this.#touches.set(e.id, { x: e.x, y: e.y, startX: e.x, startY: e.y })
        const n = this.#touches.size
        if (n === 1 && mode.kind === 'idle') {
          this.#gesture = {
            start: e.time,
            maxTouches: 1,
            maxTravel: 0,
            cx: e.x,
            cy: e.y,
            spread: 0,
          }
          const pending: Mode = { kind: 'pendingTouch', id: e.id, start: e, longPressed: false }
          this.#mode = pending
          this.#startTimer(TUNING.longPressMs, () => {
            if (this.#mode === pending) {
              pending.longPressed = true
              this.handlers.contextMenu(e.x, e.y, 'touch')
            }
          })
          return
        }
        if (n >= 2) {
          this.#stopTimer()
          if (mode.kind === 'tool' && mode.pointerKind === 'touch') this.handlers.toolCancel()
          if (mode.kind !== 'gesture') {
            if (mode.kind !== 'pendingTouch') this.#gesture.start = e.time
            this.#mode = { kind: 'gesture' }
          }
          this.#gesture.maxTouches = Math.max(this.#gesture.maxTouches, n)
          this.#rebaseGesture()
        }
        return
      }
      case 'move': {
        const touch = this.#touches.get(e.id)
        if (!touch) return
        touch.x = e.x
        touch.y = e.y
        this.#gesture.maxTravel = Math.max(
          this.#gesture.maxTravel,
          Math.hypot(e.x - touch.startX, e.y - touch.startY),
        )
        if (mode.kind === 'pendingTouch' && mode.id === e.id) {
          if (Math.hypot(e.x - mode.start.x, e.y - mode.start.y) <= TUNING.dragSlop.touch) return
          this.#stopTimer()
          if (mode.longPressed) {
            // A menu is open; moving does not start anything else.
            return
          }
          if (this.#fingersEdit()) {
            this.#mode = {
              kind: 'tool',
              id: e.id,
              pointerKind: 'touch',
              start: mode.start,
              moved: true,
            }
            this.handlers.toolDown(toTool(mode.start))
            this.handlers.toolMove(toTool(e))
          } else {
            this.#mode = { kind: 'pan', id: e.id, x: e.x, y: e.y }
            this.handlers.pan(e.x - mode.start.x, e.y - mode.start.y)
          }
        } else if (mode.kind === 'tool' && mode.id === e.id) {
          this.handlers.toolMove(toTool(e))
        } else if (mode.kind === 'pan' && mode.id === e.id) {
          this.handlers.pan(e.x - mode.x, e.y - mode.y)
          mode.x = e.x
          mode.y = e.y
        } else if (mode.kind === 'gesture') {
          const before = this.#gesture
          const { cx, cy, spread } = this.#centroid()
          this.handlers.pan(cx - before.cx, cy - before.cy)
          if (before.spread > 0 && spread > 0 && this.#touches.size >= 2) {
            this.handlers.zoom(cx, cy, spread / before.spread)
          }
          Object.assign(this.#gesture, { cx, cy, spread })
        }
        return
      }
      case 'up':
      case 'cancel': {
        if (!this.#touches.delete(e.id)) return
        if (mode.kind === 'pendingTouch' && mode.id === e.id) {
          this.#stopTimer()
          this.#mode = { kind: 'idle' }
          if (e.phase === 'up' && !mode.longPressed) {
            // A tap: the tool sees a press and release in place.
            this.handlers.toolDown(toTool(mode.start))
            this.handlers.toolUp(toTool({ ...e, x: mode.start.x, y: mode.start.y }))
          }
        } else if (mode.kind === 'tool' && mode.id === e.id) {
          if (e.phase === 'up') this.handlers.toolUp(toTool(e))
          else this.handlers.toolCancel()
          this.#mode = { kind: 'idle' }
        } else if (mode.kind === 'pan' && mode.id === e.id) {
          this.#mode = { kind: 'idle' }
        } else if (mode.kind === 'gesture') {
          if (this.#touches.size > 0) {
            this.#rebaseGesture()
            return
          }
          this.#mode = { kind: 'idle' }
          const g = this.#gesture
          const quick =
            e.phase === 'up' && e.time - g.start <= TUNING.tapMaxMs && g.maxTravel <= TUNING.tapSlop
          if (quick && g.maxTouches === 2) this.handlers.undo()
          else if (quick && g.maxTouches === 3) this.handlers.redo()
        }
        return
      }
    }
  }
}
