import type { EngineState } from '@soom/engine'
import { useSyncExternalStore } from 'react'
import type { Session } from './session.ts'

/** Re-renders after every edit, undo and redo; returns undo/redo availability. */
export function useEngineState(session: Session): EngineState {
  return useSyncExternalStore(session.engine.subscribe, session.engine.getState)
}
