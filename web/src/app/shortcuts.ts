// Keyboard shortcuts for the whole window. Ignored while typing in a field.

import { matchShortcut, platformOf } from '@soom/input'
import { actualSize, zoomAt } from '../canvas/viewport.ts'
import type { Session } from '../state/session.ts'

/** Nudge distance, mm: one press, and with shift. */
const NUDGE = { small: 1, big: 10 }

/** Input types where keys type text (radios, checkboxes and switches do not). */
const TEXT_INPUTS = new Set(['text', 'search', 'number', 'email', 'password', 'url', 'tel'])

export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  if (target instanceof HTMLInputElement) return TEXT_INPUTS.has(target.type)
  return target.isContentEditable || target.tagName === 'TEXTAREA' || target.tagName === 'SELECT'
}

export function attachShortcuts(session: Session, onOpen: () => void): () => void {
  const platform = platformOf(navigator.userAgent)
  const onKey = (e: KeyboardEvent) => {
    if (isTyping(e.target)) return
    const match = matchShortcut(
      { key: e.key, meta: e.metaKey, ctrl: e.ctrlKey, shift: e.shiftKey, alt: e.altKey },
      platform,
    )
    if (!match) return
    const step = match.big ? NUDGE.big : NUDGE.small
    const v = session.ui.get().viewport
    const centre = { x: v.width / 2, y: v.height / 2 }
    switch (match.action) {
      case 'undo':
        session.undo()
        break
      case 'redo':
        session.redo()
        break
      case 'delete':
        session.deleteSelection()
        break
      case 'escape':
        if (session.ui.get().menu) session.openMenu(null)
        else session.select([])
        break
      case 'open':
        onOpen()
        break
      case 'selectAll':
        session.select(session.engine.document().pieces.map((p) => p.id))
        break
      case 'zoomIn':
        session.setViewport((s) => zoomAt(s, centre, 1.5))
        break
      case 'zoomOut':
        session.setViewport((s) => zoomAt(s, centre, 1 / 1.5))
        break
      case 'fitAll':
        session.fitAll()
        break
      case 'actualSize':
        session.setViewport(actualSize)
        break
      case 'nudgeLeft':
        session.nudge(-step, 0)
        break
      case 'nudgeRight':
        session.nudge(step, 0)
        break
      case 'nudgeUp':
        session.nudge(0, -step)
        break
      case 'nudgeDown':
        session.nudge(0, step)
        break
      case 'tool.select':
        session.setTool('select')
        break
      default:
        // Tools that arrive in later steps.
        return
    }
    e.preventDefault()
  }
  window.addEventListener('keydown', onKey)
  return () => window.removeEventListener('keydown', onKey)
}
