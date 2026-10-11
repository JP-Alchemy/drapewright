// The avatar and the 3D view: the engine's 3D side (`pkg-3d/`, built by
// `pnpm wasm`). Only the sim worker imports this entry, so the 2D app never
// loads it.

import init, { Avatar as AvatarHandle, View as ViewHandle } from '../pkg-3d/drapewright_wasm_3d.js'
import type { AvatarFit, AvatarRequest, AvatarSettings } from './types'

export type { AvatarFit, AvatarRequest, AvatarSettings } from './types'

let ready: Promise<void> | undefined
const loaded = () => (ready ??= init().then(() => undefined))

/** Each avatar's handle, for the view to draw it. */
const handles = new WeakMap<Avatar, AvatarHandle>()

/** The avatar's body, loaded and ready to fit. */
export class Avatar {
  private constructor(handle: AvatarHandle) {
    handles.set(this, handle)
  }

  /** Loads the 3D module (once) and the avatar from a body file's bytes. */
  static async load(body: Uint8Array): Promise<Avatar> {
    await loaded()
    return new Avatar(new AvatarHandle(body))
  }

  /** Fits the avatar to a person's measurements. */
  fit(request: AvatarRequest): AvatarFit {
    return handles.get(this)!.fit(request) as AvatarFit
  }
}

/** The view's colours, as on the page: sRGB, each part 0 to 1. */
export interface ViewColours {
  background: [number, number, number]
  figure: [number, number, number]
}

/** The 3D view, drawing on a canvas handed over by the page. */
export class AvatarView {
  readonly #handle: ViewHandle

  private constructor(handle: ViewHandle) {
    this.#handle = handle
  }

  /** Starts drawing on `canvas`, already at its size in device pixels. */
  static async create(canvas: OffscreenCanvas): Promise<AvatarView> {
    await loaded()
    return new AvatarView(await ViewHandle.create(canvas))
  }

  /** How it draws: "WebGPU" or "WebGL2". */
  get backend(): string {
    return this.#handle.backend
  }

  resize(width: number, height: number): void {
    this.#handle.resize(width, height)
  }

  setLook(colours: ViewColours): void {
    this.#handle.setLook(colours)
  }

  /** Shows the avatar shaped by these settings; framed whole the first time. */
  show(avatar: Avatar, settings: AvatarSettings): void {
    this.#handle.show(handles.get(avatar)!, settings)
  }

  /** Turns round the figure for a drag of `dx`, `dy` CSS pixels. */
  turn(dx: number, dy: number): void {
    this.#handle.turn(dx, dy)
  }

  zoom(factor: number): void {
    this.#handle.zoom(factor)
  }

  /** Slides the view for a drag of `dx`, `dy` CSS pixels. */
  slide(dx: number, dy: number, pixelRatio: number): void {
    this.#handle.slide(dx, dy, pixelRatio)
  }

  /** Back to the figure seen whole from the front. */
  reset(): void {
    this.#handle.reset()
  }

  draw(): void {
    this.#handle.draw()
  }
}
