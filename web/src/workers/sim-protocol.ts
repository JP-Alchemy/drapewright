import type {
  AvatarFit,
  AvatarPose,
  AvatarRequest,
  AvatarSettings,
  ClothReading,
  ClothShows,
  DrapeProgress,
  DrapeRequest,
  Girth,
  Blended,
  BlendRequest,
  FitRequest,
  FitSteps,
  SizeMeasures,
  SizesRequest,
} from '@drapewright/engine'
import type { ViewColours } from '@drapewright/engine/avatar'

/** How a drape is going, as the sim worker tells the page. */
export interface DrapeUpdate extends DrapeProgress {
  /** The pieces left out, sewn to nothing, by id. */
  leftOut: string[]
  /** The facings and linings sewn to nothing, by id: left out, as they go inside. */
  inside: string[]
  /** The facings and linings draped inside the rest, by id. */
  lined: string[]
  /** How many buttons the garment has, done up or not. */
  buttons: number
  /** Whether the cloth is moved on on the graphics card. */
  card: boolean
  /** The garment's size round the body at the figure's bust, waist and hip. */
  girths: Girth[]
}

/** What the sim worker can be asked, and what each answers. */
export interface SimMethods {
  /** Fits the avatar to a person's measurements. */
  fitAvatar(request: AvatarRequest): AvatarFit
  /** Sews each size of a pattern and measures it on the avatar. */
  measureSizes(request: SizesRequest): SizeMeasures[]
  /** The view as it is, as a PNG. */
  picture(): Blob
  /** The view turned once round the figure in `frames` steps, each a JPEG. */
  turnaround(frames: number): Blob[]
  /** Cuts a pattern in a size at each level, blended between them. */
  blendSizes(request: BlendRequest): Blended
  /** The steps altering a garment at its side seams to fit. */
  fitSteps(request: FitRequest): FitSteps
  /** Starts the 3D view on a canvas handed over by the page; how it draws. */
  attachView(canvas: OffscreenCanvas, colours: ViewColours): string
  /** Shows the avatar with these settings in `pose` in the view, taking off any garment. */
  showAvatar(settings: AvatarSettings, pose: AvatarPose): null
  /** Stops the view (its canvas is gone). */
  detachView(): null
  /**
   * Drapes the project's garment on the avatar in the view, telling the
   * page how it goes (`SimUpdate`s). Answers its last update once it is
   * done, or null if it was taken off first. `quietly`, the cloth shown
   * stays until the new one has settled.
   */
  drape(request: DrapeRequest, quietly: boolean): DrapeUpdate | null
  /**
   * Moves the avatar into `pose`. A garment being draped moves with it once
   * settled, its drape answering (null now). A garment draped and settled
   * moves with it as a drape again, telling the page how it goes under this
   * request, answering its last update once settled. With no garment, the
   * avatar is shown in the pose (null).
   */
  pose(pose: AvatarPose): DrapeUpdate | null
  /** Takes the garment off. */
  undrape(): null
  /** The cloth seen at `x`, `y` CSS pixels from the view's top left, if any. */
  pick(x: number, y: number, pixelRatio: number): ClothReading | null
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
  | { note: 'shows'; shows: ClothShows }

/** What the worker tells the page unasked: how the drape asked for by request `drape` is going. */
export interface SimUpdate {
  drape: number
  update: DrapeUpdate
}

/** Failures the app explains in its own words. */
export type SimFailure = 'noBodyFile' | 'noView' | 'cannotDrape'

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
