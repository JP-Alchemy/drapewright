import type {
  AvatarFit,
  AvatarPose,
  AvatarRequest,
  AvatarSettings,
  ClothReading,
  DrapeRequest,
  SeenEdge,
  Blended,
  BlendRequest,
  FitRequest,
  FitSteps,
  SizeMeasures,
  SizesRequest,
} from '@drapewright/engine'
import type { ViewColours } from '@drapewright/engine/avatar'
import {
  type DrapeUpdate,
  SimError,
  type SimMethod,
  type SimMethods,
  type SimNote,
  type SimOutline,
  type SimRequest,
  type SimResponse,
  type SimUpdate,
} from './sim-protocol.ts'

/**
 * Calls the sim worker as promises. The worker (and the 3D module and body
 * file it loads) starts with the first call, so the 2D app never waits for
 * it; if it fails to start, the next call starts it afresh.
 */
class SimClient {
  #worker: Worker | undefined
  readonly #pending = new Map<
    number,
    { resolve: (v: unknown) => void; reject: (e: Error) => void }
  >()
  #nextId = 1
  /** Recent fits by what they were fitted to: the same person is not fitted twice. */
  readonly #fits = new Map<string, Promise<AvatarFit>>()
  /** The drape asked for last, by request, and who hears how it goes. */
  #drape: { id: number; onUpdate: (update: DrapeUpdate) => void } | null = null
  /** Who hears the pieces' edges as seen, while editing on the figure. */
  #outline: ((outline: SeenEdge[] | null) => void) | null = null

  #start(): Worker {
    if (this.#worker) return this.#worker
    const worker = new Worker(new URL('./sim.worker.ts', import.meta.url), { type: 'module' })
    worker.onmessage = (event: MessageEvent<SimResponse | SimUpdate | SimOutline>) => {
      const response = event.data
      if ('outline' in response) {
        this.#outline?.(response.outline)
        return
      }
      if ('update' in response) {
        if (response.drape === this.#drape?.id) this.#drape.onUpdate(response.update)
        return
      }
      const pending = this.#pending.get(response.id)
      if (!pending) return
      this.#pending.delete(response.id)
      if (response.ok) pending.resolve(response.result)
      else pending.reject(new SimError(response.message, response.failure))
    }
    worker.onerror = (event) => {
      event.preventDefault()
      for (const pending of this.#pending.values()) {
        pending.reject(new Error(event.message || 'the 3D side stopped'))
      }
      this.#pending.clear()
      worker.terminate()
      this.#worker = undefined
    }
    this.#worker = worker
    return worker
  }

  #call<M extends SimMethod>(
    method: M,
    args: Parameters<SimMethods[M]>,
    transfer: Transferable[] = [],
    id = this.#nextId++,
  ): Promise<ReturnType<SimMethods[M]>> {
    const request: SimRequest<M> = { id, method, args }
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve: resolve as (v: unknown) => void, reject })
      this.#start().postMessage(request, transfer)
    })
  }

  /** Fits the avatar to a person's measurements. */
  fitAvatar(request: AvatarRequest): Promise<AvatarFit> {
    const key = JSON.stringify(request)
    let fit = this.#fits.get(key)
    if (!fit) {
      fit = this.#call('fitAvatar', [request])
      // Not kept if it failed, to try again.
      fit.catch(() => this.#fits.delete(key))
      this.#fits.set(key, fit)
      if (this.#fits.size > 20) this.#fits.delete(this.#fits.keys().next().value!)
    }
    return fit
  }

  /** Sews each size of a pattern and measures it on the avatar. */
  measureSizes(request: SizesRequest): Promise<SizeMeasures[]> {
    return this.#call('measureSizes', [request])
  }

  /** The view as it is, as a PNG. */
  picture(): Promise<Blob> {
    return this.#call('picture', [])
  }

  /** The view turned once round the figure in `frames` steps, each a JPEG. */
  turnaround(frames: number): Promise<Blob[]> {
    return this.#call('turnaround', [frames])
  }

  /** Cuts a pattern in a size at each level, blended between them. */
  blendSizes(request: BlendRequest): Promise<Blended> {
    return this.#call('blendSizes', [request])
  }

  /** The steps altering a garment at its side seams to fit. */
  fitSteps(request: FitRequest): Promise<FitSteps> {
    return this.#call('fitSteps', [request])
  }

  /**
   * Hears the pieces' edges as seen in the view after each frame drawn
   * (`pixelRatio` device pixels to a CSS pixel), until called with null.
   */
  outline(hear: ((outline: SeenEdge[] | null) => void) | null, pixelRatio = 1): void {
    this.#outline = hear
    this.note({ note: 'outline', pixelRatio: hear ? pixelRatio : null })
  }

  /** Hands the canvas to the worker for the 3D view; how it draws. */
  attachView(canvas: OffscreenCanvas, colours: ViewColours): Promise<string> {
    return this.#call('attachView', [canvas, colours], [canvas])
  }

  showAvatar(settings: AvatarSettings, pose: AvatarPose = 'standing'): Promise<null> {
    return this.#call('showAvatar', [settings, pose])
  }

  detachView(): Promise<null> {
    return this.#call('detachView', [])
  }

  /**
   * Drapes the project's garment on the avatar in the view; `onUpdate`
   * hears how it goes. Its last update once it is done, or null if it was
   * taken off (or another drape asked for) first. `quietly`, the cloth
   * shown stays until the new one has settled.
   */
  drape(
    request: DrapeRequest,
    onUpdate: (update: DrapeUpdate) => void,
    quietly = false,
  ): Promise<DrapeUpdate | null> {
    const id = this.#nextId++
    this.#drape = { id, onUpdate }
    return this.#call('drape', [request, quietly], [], id)
  }

  /**
   * Moves the avatar into `pose`, and any garment on it. A garment draped
   * and settled moves as a drape again: `onUpdate` hears how it goes, and
   * its last update answers once settled. Otherwise null: a garment being
   * draped moves once settled, its drape answering.
   */
  pose(pose: AvatarPose, onUpdate?: (update: DrapeUpdate) => void): Promise<DrapeUpdate | null> {
    const id = this.#nextId++
    if (onUpdate) this.#drape = { id, onUpdate }
    return this.#call('pose', [pose], [], id)
  }

  /** Takes the garment off. */
  undrape(): Promise<null> {
    this.#drape = null
    return this.#call('undrape', [])
  }

  /** The cloth seen at `x`, `y` CSS pixels from the view's top left, if any. */
  pick(x: number, y: number, pixelRatio: number): Promise<ClothReading | null> {
    return this.#call('pick', [x, y, pixelRatio])
  }

  /** A change to the view that needs no answer. */
  note(note: SimNote): void {
    this.#worker?.postMessage(note)
  }
}

export const sim = new SimClient()
