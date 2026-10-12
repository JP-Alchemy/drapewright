import { type CSSProperties, useRef, useState } from 'react'
import { remember, remembered } from '../platform/remember.ts'
import type { Session } from '../state/session.ts'
import { t } from '../strings/en.ts'
import { Inspector } from './Inspector.tsx'
import { PieceList } from './PieceList.tsx'
import { SeamList } from './SeamList.tsx'

const SPLIT_KEY = 'drapewright.sidePanel.split'
/** The lists' share of the side panel's height: at first, least and most. */
const SPLIT = { first: 0.45, least: 0.15, most: 0.85 }
const KEY_STEP = 0.05

const clamp = (share: number) =>
  Number.isFinite(share) ? Math.min(SPLIT.most, Math.max(SPLIT.least, share)) : SPLIT.first

/**
 * The side panel: the pieces and seams above, the details of what is
 * selected below, each scrolling on its own, so picking the next piece
 * keeps the list where it was and shows the piece's details from the top.
 * The bar between them moves: drag it, or focus it and use the arrow keys;
 * double-click puts it back.
 */
export function SidePanel({ session }: { session: Session }) {
  const panel = useRef<HTMLElement>(null)
  const dragging = useRef(false)
  const [split, setSplit] = useState(() => clamp(remembered(SPLIT_KEY, SPLIT.first)))
  const keep = (share: (now: number) => number) =>
    setSplit((now) => {
      const next = clamp(share(now))
      remember(SPLIT_KEY, next)
      return next
    })
  const style = { '--split': `${split * 100}%` } as CSSProperties
  return (
    <aside ref={panel} className="side-panel" style={style}>
      <div className="side-lists">
        <PieceList session={session} />
        <SeamList session={session} />
      </div>
      <div
        role="separator"
        aria-orientation="horizontal"
        aria-label={t.sidePanel.resize}
        aria-valuemin={Math.round(SPLIT.least * 100)}
        aria-valuemax={Math.round(SPLIT.most * 100)}
        aria-valuenow={Math.round(split * 100)}
        tabIndex={0}
        className="side-split"
        onPointerDown={(e) => {
          e.currentTarget.setPointerCapture(e.pointerId)
          dragging.current = true
        }}
        onPointerMove={(e) => {
          const box = panel.current?.getBoundingClientRect()
          if (dragging.current && box && box.height > 0) {
            setSplit(clamp((e.clientY - box.top) / box.height))
          }
        }}
        onPointerUp={() => {
          dragging.current = false
          remember(SPLIT_KEY, split)
        }}
        onPointerCancel={() => {
          dragging.current = false
        }}
        onDoubleClick={() => keep(() => SPLIT.first)}
        onKeyDown={(e) => {
          const to: Record<string, (now: number) => number> = {
            ArrowUp: (now) => now - KEY_STEP,
            ArrowDown: (now) => now + KEY_STEP,
            Home: () => SPLIT.least,
            End: () => SPLIT.most,
          }
          const next = to[e.key]
          if (next === undefined) return
          e.preventDefault()
          keep(next)
        }}
      />
      <div className="side-details">
        <Inspector session={session} />
      </div>
    </aside>
  )
}
