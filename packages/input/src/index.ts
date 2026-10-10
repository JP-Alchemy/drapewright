// Framework-free input handling for Soom: pointer routing with palm
// rejection and gestures, and keyboard shortcuts.

export {
  type FingerEditing,
  type InputHandlers,
  InputRouter,
  type Modifiers,
  type PointerKind,
  type RawPointer,
  type Scheduler,
  type ToolPointer,
  TUNING,
} from './router.ts'
export {
  type Action,
  type KeyInput,
  type Match,
  type Platform,
  matchShortcut,
  platformOf,
  shortcutLabel,
} from './shortcuts.ts'

/** Screen-space hit radius per pointer, CSS px: 44 pt targets for fingers. */
export function hitRadius(kind: 'pen' | 'touch' | 'mouse'): number {
  return kind === 'touch' ? 22 : kind === 'pen' ? 10 : 6
}
