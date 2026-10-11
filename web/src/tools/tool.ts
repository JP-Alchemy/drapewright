// Tools are small state machines. They receive one stream of pointer
// events from the input router and respond with previews (transient, never
// in undo) or commands (one undo step each). They never change the document
// directly, so a cancelled gesture can never leave half an edit behind.

import type { Hit } from '@drapewright/engine'
import { type ToolPointer, hitRadius } from '@drapewright/input'
import type { Point } from '../canvas/viewport.ts'
import { scaleOf, toWorld } from '../canvas/viewport.ts'
import type { Session } from '../state/session.ts'

export interface ToolContext {
  session: Session
  /** Work-table position of a pointer, mm. */
  world(p: ToolPointer): Point
  /** What is under a pointer, with a hit radius suited to the pointer. */
  hit(p: ToolPointer): Hit | null
}

export interface Tool {
  down(p: ToolPointer, ctx: ToolContext): void
  move(p: ToolPointer, ctx: ToolContext): void
  up(p: ToolPointer, ctx: ToolContext): void
  cancel(ctx: ToolContext): void
  hover?(p: ToolPointer | null, ctx: ToolContext): void
  /** Pen or mouse held still. */
  hold?(p: ToolPointer, ctx: ToolContext): void
}

export function toolContext(session: Session): ToolContext {
  return {
    session,
    world(p) {
      return toWorld(session.ui.get().viewport, p)
    },
    hit(p) {
      const v = session.ui.get().viewport
      const w = toWorld(v, p)
      return session.engine.hitTest(w.x, w.y, hitRadius(p.kind) / scaleOf(v))
    },
  }
}
