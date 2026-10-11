import type { AvatarFit, AvatarRequest } from '@drapewright/engine'

/** What the sim worker can be asked: the avatar now; the view and the drape to come. */
export interface SimRequest {
  id: number
  method: 'fitAvatar'
  request: AvatarRequest
}

/** Failures the app explains in its own words. */
export type SimFailure = 'noBodyFile'

export type SimResponse =
  | { id: number; ok: true; result: AvatarFit }
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
