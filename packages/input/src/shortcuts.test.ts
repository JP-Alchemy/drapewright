import { describe, expect, it } from 'vitest'
import { type KeyInput, matchShortcut } from './shortcuts.ts'

const key = (k: string, mods: Partial<KeyInput> = {}): KeyInput => ({
  key: k,
  meta: false,
  ctrl: false,
  shift: false,
  alt: false,
  ...mods,
})

describe('shortcuts', () => {
  it('uses ⌘ on Apple and Ctrl elsewhere', () => {
    expect(matchShortcut(key('z', { meta: true }), 'apple')?.action).toBe('undo')
    expect(matchShortcut(key('z', { ctrl: true }), 'apple')).toBeNull()
    expect(matchShortcut(key('z', { ctrl: true }), 'other')?.action).toBe('undo')
    expect(matchShortcut(key('Z', { meta: true, shift: true }), 'apple')?.action).toBe('redo')
    expect(matchShortcut(key('y', { ctrl: true }), 'other')?.action).toBe('redo')
    expect(matchShortcut(key('y', { meta: true }), 'apple')).toBeNull()
  })

  it('maps single keys to tools and view actions', () => {
    expect(matchShortcut(key('v'), 'apple')?.action).toBe('tool.select')
    expect(matchShortcut(key('N', { shift: true }), 'other')).toBeNull()
    expect(matchShortcut(key('0'), 'apple')?.action).toBe('fitAll')
    expect(matchShortcut(key('1'), 'apple')?.action).toBe('actualSize')
    expect(matchShortcut(key('Backspace'), 'apple')?.action).toBe('delete')
  })

  it('nudges further with shift', () => {
    expect(matchShortcut(key('ArrowLeft'), 'apple')).toEqual({ action: 'nudgeLeft', big: false })
    expect(matchShortcut(key('ArrowDown', { shift: true }), 'apple')).toEqual({
      action: 'nudgeDown',
      big: true,
    })
  })
})
