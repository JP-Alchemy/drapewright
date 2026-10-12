// The IO worker: imports, exports and project storage run here, on their
// own instance of the engine's WebAssembly module, so a big file never
// freezes the screen and OPFS can be written synchronously.

import { jobs, loadEngine } from '@drapewright/engine'
import { storageJobs } from '../storage/worker-store.ts'
import type { IoHandlers, IoRequest, IoResponse } from './io-protocol.ts'

const ready = loadEngine()
const handlers: IoHandlers = { ...jobs, ...storageJobs }

/**
 * Storage jobs run one at a time (others alongside them): OPFS lets only one
 * job at a time have a file open, so a save and a backup of the same project
 * would otherwise fail.
 */
const storage = new Set<string>(Object.keys(storageJobs))
let stored: Promise<unknown> = Promise.resolve()

self.onmessage = async (event: MessageEvent<IoRequest>) => {
  const { id, method, args } = event.data
  const run = async () => {
    await ready
    const handler = handlers[method] as (...a: unknown[]) => unknown
    return handler(...args)
  }
  let response: IoResponse
  try {
    const result = storage.has(method) ? await (stored = stored.then(run, run)) : await run()
    response = { id, ok: true, result }
  } catch (error) {
    response = { id, ok: false, message: error instanceof Error ? error.message : String(error) }
  }
  self.postMessage(response)
}
