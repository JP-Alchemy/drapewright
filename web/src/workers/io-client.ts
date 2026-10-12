import type { IoHandlers, IoMethod, IoRequest, IoResponse } from './io-protocol.ts'

/** The worker, as the client needs it (a stand-in in tests). */
export type IoWorker = Pick<Worker, 'postMessage' | 'terminate' | 'onmessage' | 'onerror'>

/** Starts the IO worker. */
const startWorker = (): IoWorker =>
  new Worker(new URL('./io.worker.ts', import.meta.url), { type: 'module' })

/** Times a worker that failed to load is started again before calls fail. */
const RESTARTS = 2
/** How long before starting it again, ms. */
const RESTART_AFTER = 250

/**
 * Calls the IO worker's jobs as promises. A worker that fails to load (as
 * can happen while a new version of the app takes over from the old) is
 * started again, and the calls waiting on it sent again: it never ran them.
 */
export class IoClient {
  #worker: IoWorker
  readonly #pending = new Map<
    number,
    { request: IoRequest; resolve: (v: unknown) => void; reject: (e: Error) => void }
  >()
  #nextId = 1
  #restarts = RESTARTS
  /** Set once the worker could not be started: every call fails with it. */
  #failed: Error | null = null

  constructor(readonly start: () => IoWorker = startWorker) {
    this.#worker = this.#begin()
  }

  #begin(): IoWorker {
    const worker = this.start()
    // Answered: started, so a later failure is a job's, not loading.
    let answered = false
    worker.onmessage = (event: MessageEvent<IoResponse>) => {
      answered = true
      const response = event.data
      const pending = this.#pending.get(response.id)
      if (!pending) return
      this.#pending.delete(response.id)
      if (response.ok) pending.resolve(response.result)
      else pending.reject(new Error(response.message))
    }
    worker.onerror = (event: ErrorEvent) => {
      event.preventDefault?.()
      worker.terminate()
      if (!answered && this.#restarts > 0) {
        this.#restarts--
        setTimeout(() => {
          this.#worker = this.#begin()
          for (const { request } of this.#pending.values()) this.#worker.postMessage(request)
        }, RESTART_AFTER)
        return
      }
      // Failed while doing a job, or could not be started: what waits on
      // it fails. One that ran is started afresh for later calls.
      const error = new Error(event.message || 'The file worker stopped.')
      for (const { reject } of this.#pending.values()) reject(error)
      this.#pending.clear()
      if (answered) this.#worker = this.#begin()
      else this.#failed = error
    }
    return worker
  }

  call<M extends IoMethod>(
    method: M,
    ...args: Parameters<IoHandlers[M]>
  ): Promise<Awaited<ReturnType<IoHandlers[M]>>> {
    const id = this.#nextId++
    const request: IoRequest<M> = { id, method, args }
    if (this.#failed) return Promise.reject(this.#failed)
    return new Promise((resolve, reject) => {
      this.#pending.set(id, {
        request: request as IoRequest,
        resolve: resolve as (v: unknown) => void,
        reject,
      })
      this.#worker.postMessage(request)
    })
  }
}
