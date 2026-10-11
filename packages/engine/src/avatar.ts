// The avatar: the engine's 3D side (`pkg-3d/`, built by `pnpm wasm`). Only
// the sim worker imports this entry, so the 2D app never loads it.

import init, { Avatar as AvatarHandle } from '../pkg-3d/drapewright_wasm_3d.js'
import type { AvatarFit, AvatarRequest } from './types'

export type { AvatarFit, AvatarRequest, AvatarSettings } from './types'

let ready: Promise<void> | undefined

/** The avatar's body, loaded and ready to fit. */
export class Avatar {
  readonly #handle: AvatarHandle

  private constructor(handle: AvatarHandle) {
    this.#handle = handle
  }

  /** Loads the 3D module (once) and the avatar from a body file's bytes. */
  static async load(body: Uint8Array): Promise<Avatar> {
    ready ??= init().then(() => undefined)
    await ready
    return new Avatar(new AvatarHandle(body))
  }

  /** Fits the avatar to a person's measurements. */
  fit(request: AvatarRequest): AvatarFit {
    return this.#handle.fit(request) as AvatarFit
  }
}
