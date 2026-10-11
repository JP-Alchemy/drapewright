import type {
  AvatarFit,
  AvatarRequest,
  AvatarSettings,
  ClothReading,
  DrapeRequest,
} from '@drapewright/engine'
import type { ViewColours } from '@drapewright/engine/avatar'
import {
  type DrapeUpdate,
  SimError,
  type SimMethod,
  type SimMethods,
  type SimNote,
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

  #start(): Worker {
    if (this.#worker) return this.#worker
    const worker = new Worker(new URL('./sim.worker.ts', import.meta.url), { type: 'module' })
    worker.onmessage = (event: MessageEvent<SimResponse | SimUpdate>) => {
      const response = event.data
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

  /** Hands the canvas to the worker for the 3D view; how it draws. */
  attachView(canvas: OffscreenCanvas, colours: ViewColours): Promise<string> {
    return this.#call('attachView', [canvas, colours], [canvas])
  }

  showAvatar(settings: AvatarSettings): Promise<null> {
    return this.#call('showAvatar', [settings])
  }

  detachView(): Promise<null> {
    return this.#call('detachView', [])
  }

  /**
   * Drapes the project's garment on the avatar in the view; `onUpdate`
   * hears how it goes. Its last update once it is done, or null if it was
   * taken off (or another drape asked for) first.
   */
  drape(
    request: DrapeRequest,
    onUpdate: (update: DrapeUpdate) => void,
  ): Promise<DrapeUpdate | null> {
    const id = this.#nextId++
    this.#drape = { id, onUpdate }
    return this.#call('drape', [request], [], id)
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
