import type { AvatarFit, AvatarRequest } from '@drapewright/engine'
import { SimError, type SimRequest, type SimResponse } from './sim-protocol.ts'

/**
 * Calls the sim worker as promises. The worker (and the 3D module and body
 * file it loads) starts with the first call, so the 2D app never waits for
 * it; if it fails to start, the next call starts it afresh.
 */
class SimClient {
  #worker: Worker | undefined
  readonly #pending = new Map<
    number,
    { resolve: (fit: AvatarFit) => void; reject: (e: Error) => void }
  >()
  #nextId = 1

  #start(): Worker {
    if (this.#worker) return this.#worker
    const worker = new Worker(new URL('./sim.worker.ts', import.meta.url), { type: 'module' })
    worker.onmessage = (event: MessageEvent<SimResponse>) => {
      const response = event.data
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

  /** Fits the avatar to a person's measurements. */
  fitAvatar(request: AvatarRequest): Promise<AvatarFit> {
    const id = this.#nextId++
    const message: SimRequest = { id, method: 'fitAvatar', request }
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve, reject })
      this.#start().postMessage(message)
    })
  }
}

export const sim = new SimClient()
