// The IO worker: imports, exports and project storage run here, on their
// own instance of the engine's WebAssembly module, so a big file never
// freezes the screen and OPFS can be written synchronously.

import { jobs, loadEngine } from '@drapewright/engine'
import { storageJobs } from '../storage/worker-store.ts'
import type { IoHandlers, IoRequest, IoResponse } from './io-protocol.ts'

const ready = loadEngine()
const handlers: IoHandlers = { ...jobs, ...storageJobs }

self.onmessage = async (event: MessageEvent<IoRequest>) => {
  const { id, method, args } = event.data
  let response: IoResponse
  try {
    await ready
    const handler = handlers[method] as (...a: unknown[]) => unknown
    response = { id, ok: true, result: await handler(...args) }
  } catch (error) {
    response = { id, ok: false, message: error instanceof Error ? error.message : String(error) }
  }
  self.postMessage(response)
}
