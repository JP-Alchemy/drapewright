// The avatar's frame: the edge that sizes it while it sits beside (or
// under) the table, and the title bar, grip and corner that move and size
// it while it floats over the table.

import { GripHorizontal } from 'lucide-react'
import {
  type CSSProperties,
  type KeyboardEvent,
  type PointerEvent as ReactPointerEvent,
  type ReactNode,
  type RefObject,
  useEffect,
  useState,
  useSyncExternalStore,
} from 'react'
import { t } from '../strings/en.ts'
import { type Box, SHARE, fitBox, setAvatarLayout, useAvatarLayout } from './avatar-layout.ts'

const text = t.avatar
/** How far a key moves the docked edge (a share), or the window (px). */
const STEP = 0.05
const NUDGE = 20

const UPRIGHT = '(max-width: 1100px)'

/** Whether the avatar docks under the table, as app.css lays it out on a tablet held upright. */
export function useDocksBelow(): boolean {
  return useSyncExternalStore(
    (changed) => {
      const query = matchMedia(UPRIGHT)
      query.addEventListener('change', changed)
      return () => query.removeEventListener('change', changed)
    },
    () => matchMedia(UPRIGHT).matches,
  )
}

/** The room the avatar floats in (the workspace), and the table's width in it. */
function useRoom(pane: RefObject<HTMLElement | null>) {
  const [room, setRoom] = useState({ width: 0, height: 0, table: 0 })
  useEffect(() => {
    const space = pane.current?.parentElement
    if (!space) return
    const measure = () =>
      setRoom({
        width: space.clientWidth,
        height: space.clientHeight,
        table: space.querySelector<HTMLElement>('.table')?.clientWidth ?? space.clientWidth,
      })
    measure()
    const observer = new ResizeObserver(measure)
    observer.observe(space)
    return () => observer.disconnect()
  }, [pane])
  return room
}

/**
 * Follows a pointer from `e` until it lifts: `move` with how far it has
 * gone (px), then `done`.
 */
function follow(
  e: ReactPointerEvent<HTMLElement>,
  move: (dx: number, dy: number) => void,
  done: () => void,
) {
  const el = e.currentTarget
  const [x0, y0] = [e.clientX, e.clientY]
  el.setPointerCapture(e.pointerId)
  const onMove = (ev: PointerEvent) => move(ev.clientX - x0, ev.clientY - y0)
  const onEnd = () => {
    el.removeEventListener('pointermove', onMove)
    el.removeEventListener('pointerup', onEnd)
    el.removeEventListener('pointercancel', onEnd)
    done()
  }
  el.addEventListener('pointermove', onMove)
  el.addEventListener('pointerup', onEnd)
  el.addEventListener('pointercancel', onEnd)
}

/** Keeps where the avatar is now, at the end of a drag. */
const keep = () => setAvatarLayout({})

/** Arrow keys as a step across and down; other keys `null`. */
function arrows(e: KeyboardEvent): [number, number] | null {
  const by: Record<string, [number, number]> = {
    ArrowLeft: [-1, 0],
    ArrowRight: [1, 0],
    ArrowUp: [0, -1],
    ArrowDown: [0, 1],
  }
  return by[e.key] ?? null
}

/**
 * The edge between the table and the docked avatar: drag it (or focus it
 * and use the arrow keys) to give either more room; double-click puts it
 * back halfway.
 */
export function DockEdge({ pane }: { pane: RefObject<HTMLElement | null> }) {
  const { share } = useAvatarLayout()
  const below = useDocksBelow()
  return (
    <div
      role="separator"
      aria-orientation={below ? 'horizontal' : 'vertical'}
      aria-label={text.resizeDocked}
      aria-valuemin={Math.round(SHARE.least * 100)}
      aria-valuemax={Math.round(SHARE.most * 100)}
      aria-valuenow={Math.round(share * 100)}
      tabIndex={0}
      className={below ? 'avatar-edge below' : 'avatar-edge beside'}
      onPointerDown={(e) => {
        const el = pane.current
        const table = el?.parentElement?.querySelector<HTMLElement>('.table')
        if (!el || !table || (e.pointerType === 'mouse' && e.button !== 0)) return
        // The table and the avatar share the room from the table's start to
        // the avatar's end, which stays put while the edge moves.
        const p = el.getBoundingClientRect()
        const tb = table.getBoundingClientRect()
        const [start, end] = below ? [tb.top, p.bottom] : [tb.left, p.right]
        const at = below ? e.clientY : e.clientX
        follow(
          e,
          (dx, dy) =>
            setAvatarLayout({ share: (end - (at + (below ? dy : dx))) / (end - start) }, false),
          keep,
        )
      }}
      onDoubleClick={() => setAvatarLayout({ share: SHARE.first })}
      onKeyDown={(e) => {
        const step = arrows(e)
        if (!step) return
        e.preventDefault()
        // Towards the avatar's end makes it smaller.
        setAvatarLayout({ share: share - (below ? step[1] : step[0]) * STEP })
      }}
    />
  )
}

/**
 * The floating avatar's place and size, fitted to the room it floats in:
 * at first a window at the table's right, as tall as fits.
 */
export function useFloatingBox(pane: RefObject<HTMLElement | null>): {
  style: CSSProperties | undefined
  /** Starts moving the window from its title bar (but not from its controls). */
  moveFromHead: (e: ReactPointerEvent<HTMLElement>) => void
  grip: ReactNode
  corner: ReactNode
} {
  const { box: kept } = useAvatarLayout()
  const room = useRoom(pane)
  const ready = room.width > 0
  const first = (): Box => {
    const width = Math.min(440, Math.max(320, room.table * 0.45))
    return { width, height: Math.min(680, room.height - 32), x: room.table - width - 16, y: 16 }
  }
  const box = ready ? fitBox(kept ?? first(), room.width, room.height) : null
  const place = (next: Box, keepIt = true) =>
    setAvatarLayout({ box: fitBox(next, room.width, room.height) }, keepIt)

  const move = (e: ReactPointerEvent<HTMLElement>) => {
    if (!box || (e.pointerType === 'mouse' && e.button !== 0)) return
    const start = box
    follow(e, (dx, dy) => place({ ...start, x: start.x + dx, y: start.y + dy }, false), keep)
  }
  const size = (e: ReactPointerEvent<HTMLElement>) => {
    if (!box || (e.pointerType === 'mouse' && e.button !== 0)) return
    e.stopPropagation()
    const start = box
    follow(
      e,
      (dx, dy) => place({ ...start, width: start.width + dx, height: start.height + dy }, false),
      keep,
    )
  }
  return {
    style: box
      ? { left: box.x, top: box.y, width: box.width, height: box.height }
      : { visibility: 'hidden' },
    moveFromHead: (e) => {
      const target = e.target as Element
      if (target.closest('button, input, select, a, [role="button"], [role="separator"]')) return
      move(e)
    },
    grip: (
      <div
        role="button"
        tabIndex={0}
        aria-label={text.moveWindow}
        className="avatar-grip"
        onPointerDown={move}
        onKeyDown={(e) => {
          const step = arrows(e)
          if (!step || !box) return
          e.preventDefault()
          place({ ...box, x: box.x + step[0] * NUDGE, y: box.y + step[1] * NUDGE })
        }}
      >
        <GripHorizontal size={18} aria-hidden="true" />
      </div>
    ),
    corner: (
      <div
        role="button"
        tabIndex={0}
        aria-label={text.resizeWindow}
        className="avatar-corner"
        onPointerDown={size}
        onKeyDown={(e) => {
          const step = arrows(e)
          if (!step || !box) return
          e.preventDefault()
          place({
            ...box,
            width: box.width + step[0] * NUDGE,
            height: box.height + step[1] * NUDGE,
          })
        }}
      />
    ),
  }
}
