import type { IoHandlers, IoMethod, IoRequest, IoResponse } from './io-protocol.ts'

/** Calls the IO worker's jobs as promises. */
export class IoClient {
  readonly #worker = new Worker(new URL('./io.worker.ts', import.meta.url), { type: 'module' })
  readonly #pending = new Map<
    number,
    { resolve: (v: unknown) => void; reject: (e: Error) => void }
  >()
  #nextId = 1

  constructor() {
    this.#worker.onmessage = (event: MessageEvent<IoResponse>) => {
      const response = event.data
      const pending = this.#pending.get(response.id)
      if (!pending) return
      this.#pending.delete(response.id)
      if (response.ok) pending.resolve(response.result)
      else pending.reject(new Error(response.message))
    }
  }

  call<M extends IoMethod>(
    method: M,
    ...args: Parameters<IoHandlers[M]>
  ): Promise<Awaited<ReturnType<IoHandlers[M]>>> {
    const id = this.#nextId++
    const request: IoRequest<M> = { id, method, args }
    return new Promise((resolve, reject) => {
      this.#pending.set(id, { resolve: resolve as (v: unknown) => void, reject })
      this.#worker.postMessage(request)
    })
  }
}
