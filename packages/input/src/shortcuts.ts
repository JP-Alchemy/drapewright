// Keyboard shortcuts, platform-aware: ⌘ on Apple devices, Ctrl elsewhere.

export type Action =
  | 'undo'
  | 'redo'
  | 'delete'
  | 'escape'
  | 'open'
  | 'selectAll'
  | 'zoomIn'
  | 'zoomOut'
  | 'fitAll'
  | 'actualSize'
  | 'nudgeLeft'
  | 'nudgeRight'
  | 'nudgeUp'
  | 'nudgeDown'
  | 'tool.select'
  | 'tool.notch'
  | 'tool.grainline'
  | 'tool.measure'

export type Platform = 'apple' | 'other'

export interface KeyInput {
  key: string
  meta: boolean
  ctrl: boolean
  shift: boolean
  alt: boolean
}

export interface Match {
  action: Action
  /** Shift held with a nudge: move further. */
  big: boolean
}

export function platformOf(userAgent: string): Platform {
  return /Mac|iPhone|iPad/.test(userAgent) ? 'apple' : 'other'
}

/** The action for a key press, or `null`. */
export function matchShortcut(e: KeyInput, platform: Platform): Match | null {
  const mod = platform === 'apple' ? e.meta : e.ctrl
  const other = platform === 'apple' ? e.ctrl : e.meta
  const key = e.key.length === 1 ? e.key.toLowerCase() : e.key
  const hit = (action: Action, big = false): Match => ({ action, big })
  if (other || e.alt) return null

  if (mod) {
    if (key === 'z') return hit(e.shift ? 'redo' : 'undo')
    if (key === 'y' && platform === 'other') return hit('redo')
    if (key === 'o') return hit('open')
    if (key === 'a') return hit('selectAll')
    if (key === '=' || key === '+') return hit('zoomIn')
    if (key === '-') return hit('zoomOut')
    return null
  }

  switch (key) {
    case 'Backspace':
    case 'Delete':
      return hit('delete')
    case 'Escape':
      return hit('escape')
    case '+':
    case '=':
      return hit('zoomIn')
    case '-':
      return hit('zoomOut')
    case '0':
      return hit('fitAll')
    case '1':
      return hit('actualSize')
    case 'ArrowLeft':
      return hit('nudgeLeft', e.shift)
    case 'ArrowRight':
      return hit('nudgeRight', e.shift)
    case 'ArrowUp':
      return hit('nudgeUp', e.shift)
    case 'ArrowDown':
      return hit('nudgeDown', e.shift)
  }
  if (e.shift) return null
  switch (key) {
    case 'v':
      return hit('tool.select')
    case 'n':
      return hit('tool.notch')
    case 'g':
      return hit('tool.grainline')
    case 'm':
      return hit('tool.measure')
  }
  return null
}

/** How to show a shortcut in a tooltip: "⌘Z", "Ctrl+Z". */
export function shortcutLabel(action: Action, platform: Platform): string | null {
  const mod = platform === 'apple' ? '⌘' : 'Ctrl+'
  const labels: Partial<Record<Action, string>> = {
    undo: `${mod}Z`,
    redo: platform === 'apple' ? '⇧⌘Z' : 'Ctrl+Y',
    open: `${mod}O`,
    selectAll: `${mod}A`,
    delete: platform === 'apple' ? '⌫' : 'Delete',
    zoomIn: '+',
    zoomOut: '−',
    fitAll: '0',
    actualSize: '1',
    'tool.select': 'V',
    'tool.notch': 'N',
    'tool.grainline': 'G',
    'tool.measure': 'M',
  }
  return labels[action] ?? null
}
