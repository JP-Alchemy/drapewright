import type { AvatarFit, AvatarRequest, AvatarSettings } from '@drapewright/engine'
import type { ViewColours } from '@drapewright/engine/avatar'

/** What the sim worker can be asked, and what each answers. */
export interface SimMethods {
  /** Fits the avatar to a person's measurements. */
  fitAvatar(request: AvatarRequest): AvatarFit
  /** Starts the 3D view on a canvas handed over by the page; how it draws. */
  attachView(canvas: OffscreenCanvas, colours: ViewColours): string
  /** Shows the avatar with these settings in the view. */
  showAvatar(settings: AvatarSettings): null
  /** Stops the view (its canvas is gone). */
  detachView(): null
}

export type SimMethod = keyof SimMethods

export interface SimRequest<M extends SimMethod = SimMethod> {
  id: number
  method: M
  args: Parameters<SimMethods[M]>
}

/** Changes to the view that need no answer: sent as often as the pointer moves. */
export type SimNote =
  | { note: 'resize'; width: number; height: number }
  | { note: 'look'; colours: ViewColours }
  | { note: 'turn'; dx: number; dy: number }
  | { note: 'zoom'; factor: number }
  | { note: 'slide'; dx: number; dy: number; pixelRatio: number }
  | { note: 'reset' }

/** Failures the app explains in its own words. */
export type SimFailure = 'noBodyFile' | 'noView'

export type SimResponse =
  | { id: number; ok: true; result: unknown }
  | { id: number; ok: false; message: string; failure?: SimFailure }

/** A failure from the sim worker. */
export class SimError extends Error {
  constructor(
    message: string,
    readonly failure?: SimFailure,
  ) {
    super(message)
  }
}
