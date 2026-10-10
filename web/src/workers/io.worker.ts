// The IO worker: imports and exports run here, on their own instance of the
// engine's WebAssembly module, so a big file never freezes the screen.

import { jobs, loadEngine, type Jobs } from '@soom/engine'
import type { IoRequest, IoResponse } from './io-protocol.ts'

const ready = loadEngine()

self.onmessage = async (event: MessageEvent<IoRequest>) => {
  const { id, method, args } = event.data
  let response: IoResponse
  try {
    await ready
    const job = jobs[method] as (...a: unknown[]) => ReturnType<Jobs[typeof method]>
    response = { id, ok: true, result: job(...args) }
  } catch (error) {
    response = { id, ok: false, message: error instanceof Error ? error.message : String(error) }
  }
  self.postMessage(response)
}
