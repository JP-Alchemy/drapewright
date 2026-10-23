// The avatar, the 3D view and the drape: the engine's 3D side (`pkg-3d/`,
// built by `pnpm wasm`). Only the sim worker imports this entry, so the 2D
// app never loads it.

import init, {
  Avatar as AvatarHandle,
  Draping as DrapingHandle,
  View as ViewHandle,
} from '../pkg-3d/drapewright_wasm_3d.js'
import type {
  AvatarFit,
  AvatarPose,
  AvatarRequest,
  AvatarSettings,
  Blended,
  BlendRequest,
  ClothReading,
  ClothShows,
  DrapeProgress,
  DrapeRequest,
  FitRequest,
  FitSteps,
  Girth,
  SeenEdge,
  SizeMeasures,
  SizesRequest,
} from './types'

export type {
  AvatarFit,
  AvatarPose,
  AvatarRequest,
  AvatarSettings,
  BlendNote,
  Blended,
  BlendRequest,
  ClothReading,
  ClothShows,
  DrapeProgress,
  DrapeRequest,
  DrapeStage,
  FabricPreset,
  FitRequest,
  FitSteps,
  Girth,
  SeenEdge,
  SizeMeasures,
  SizesRequest,
} from './types'

let ready: Promise<void> | undefined
const loaded = () => (ready ??= init().then(() => undefined))

/** Each avatar's handle, for the view to draw it. */
const handles = new WeakMap<Avatar, AvatarHandle>()
/** Each drape's handle, for the view to draw its cloth. */
const drapings = new WeakMap<Draping, DrapingHandle>()

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

  /**
   * Sews each size of a pattern and measures it on the avatar shaped by
   * the request's settings: round the bust, waist and hip, and the ease.
   */
  measureSizes(request: SizesRequest): SizeMeasures[] {
    return handles.get(this)!.measureSizes(request) as SizeMeasures[]
  }

  /**
   * The steps letting out or taking in a garment's side seams to change
   * it round the bust, waist and hip by as much as the request asks.
   */
  fitSteps(request: FitRequest): FitSteps {
    return handles.get(this)!.fitSteps(request) as FitSteps
  }

  /**
   * Cuts a pattern in the size the request chooses at each of the bust,
   * waist and hip, blended smoothly between them up and down the figure.
   * Throws if the size blended from can't be sewn round the body.
   */
  blendSizes(request: BlendRequest): Blended {
    return handles.get(this)!.blendSizes(request) as Blended
  }

  /**
   * Starts draping the project's garment on the avatar. Throws if it
   * cannot be: nothing sewn yet, or a piece that cannot be meshed.
   */
  drape(request: DrapeRequest): Draping {
    const draping = Object.create(Draping.prototype) as Draping
    drapings.set(draping, handles.get(this)!.drape(request))
    return draping
  }
}

/** A drape in progress on the avatar, moved on a frame at a time. */
export class Draping {
  private constructor() {}

  /** The pieces left out, sewn to nothing, by id. */
  get leftOut(): string[] {
    return drapings.get(this)!.leftOut
  }

  /** The facings and linings sewn to nothing, by id: left out, as they go inside. */
  get inside(): string[] {
    return drapings.get(this)!.inside
  }

  /** The facings and linings sewn, but with nothing to lie on inside the rest, by id: left out. */
  get unplaced(): string[] {
    return drapings.get(this)!.unplaced
  }

  /** The facings and linings draped inside the rest, by id. */
  get lined(): string[] {
    return drapings.get(this)!.lined
  }

  /** The pieces sewn onto others (patch pockets) left out, by id: sewn onto a piece not draped, or onto a lining. */
  get off(): string[] {
    return drapings.get(this)!.off
  }

  /** The pieces sewn onto others left out as they couldn't be laid on the piece they are sewn onto, by id. */
  get notLaid(): string[] {
    return drapings.get(this)!.notLaid
  }

  /** How many buttons the garment has, done up or not. */
  get buttons(): number {
    return drapings.get(this)!.buttons
  }

  /** The garment's size round the body at the figure's bust, waist and hip. */
  get girths(): Girth[] {
    return drapings.get(this)!.girths as Girth[]
  }

  /**
   * Moves the body into `pose` once the garment has settled (at once if it
   * has), the garment with it: stepped on as a drape is.
   */
  pose(pose: AvatarPose): void {
    drapings.get(this)!.pose(pose)
  }

  /**
   * Whether a frame can be taken now: on the graphics card, not until it
   * has caught up, as the page's frames go by.
   */
  ready(): boolean {
    return drapings.get(this)!.ready()
  }

  /** Whether the cloth is moved on on the graphics card. */
  get onCard(): boolean {
    return drapings.get(this)!.onCard
  }

  /** Moves the drape on a frame (1/60 s of the cloth's time). */
  step(): DrapeProgress {
    return drapings.get(this)!.step() as DrapeProgress
  }

  /** Frees the drape in the 3D module; the view keeps the cloth it shows. */
  free(): void {
    drapings.get(this)?.free()
    drapings.delete(this)
  }
}

/** The view's colours, as on the page: sRGB, each part 0 to 1. */
export interface ViewColours {
  background: [number, number, number]
  figure: [number, number, number]
  cloth: [number, number, number]
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

  /** Shows the avatar shaped by these settings in `pose`; framed whole the first time. */
  show(avatar: Avatar, settings: AvatarSettings, pose: AvatarPose = 'standing'): void {
    this.#handle.show(handles.get(avatar)!, settings, pose)
  }

  /** Shows a drape's cloth as it is now, and its body as it has moved. */
  showCloth(draping: Draping): void {
    const handle = drapings.get(draping)
    if (handle) this.#handle.showCloth(handle)
  }

  /** Takes the cloth off. */
  hideCloth(): void {
    this.#handle.hideCloth()
  }

  /** Shows the cloth in its fabric's colour, or as a heatmap. */
  setShows(shows: ClothShows): void {
    this.#handle.setShows(shows)
  }

  /** The cloth seen at `x`, `y` CSS pixels from the view's top left, if any. */
  pick(x: number, y: number, pixelRatio: number): ClothReading | null {
    return this.#handle.pick(x, y, pixelRatio) as ClothReading | null
  }

  /** The pieces' edges on the cloth as seen now; null with no cloth on. */
  outline(pixelRatio: number): SeenEdge[] | null {
    return this.#handle.outline(pixelRatio) as SeenEdge[] | null
  }

  /** Turns round the figure for a drag of `dx`, `dy` CSS pixels. */
  turn(dx: number, dy: number): void {
    this.#handle.turn(dx, dy)
  }

  /** Turns round the figure by `radians`, as on a turntable. */
  spin(radians: number): void {
    this.#handle.spin(radians)
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
