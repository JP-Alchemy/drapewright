import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { IoClient, type IoWorker } from './io-client.ts'
import type { IoRequest } from './io-protocol.ts'

/** A stand-in worker: it records what it is sent, and is answered or failed by hand. */
class FakeWorker implements IoWorker {
  sent: IoRequest[] = []
  terminated = false
  onmessage: IoWorker['onmessage'] = null
  onerror: IoWorker['onerror'] = null
  postMessage(request: IoRequest) {
    this.sent.push(request)
  }
  terminate() {
    this.terminated = true
  }
  answer(id: number, result: unknown) {
    this.onmessage?.call(this as never, { data: { id, ok: true, result } } as MessageEvent)
  }
  fail(message = 'blocked') {
    this.onerror?.call(this as never, { message, preventDefault() {} } as ErrorEvent)
  }
}

describe('the file worker client', () => {
  let workers: FakeWorker[]
  const client = () =>
    new IoClient(() => {
      const w = new FakeWorker()
      workers.push(w)
      return w
    })

  beforeEach(() => {
    workers = []
    vi.useFakeTimers()
  })
  afterEach(() => vi.useRealTimers())

  it('starts a worker that failed to load again, and sends it what was waiting', async () => {
    const io = client()
    const status = io.call('storageStatus')
    workers[0]!.fail()
    expect(workers[0]!.terminated).toBe(true)
    await vi.advanceTimersByTimeAsync(300)
    expect(workers).toHaveLength(2)
    const request = workers[1]!.sent[0]!
    expect(request.method).toBe('storageStatus')
    workers[1]!.answer(request.id, { kind: 'opfs', persisted: false })
    await expect(status).resolves.toEqual({ kind: 'opfs', persisted: false })
  })

  it('gives up after failing to load three times, and says so', async () => {
    const io = client()
    const status = io.call('storageStatus')
    const failed = expect(status).rejects.toThrow('blocked')
    for (let i = 0; i < 3; i++) {
      workers.at(-1)!.fail()
      await vi.advanceTimersByTimeAsync(300)
    }
    await failed
    expect(workers).toHaveLength(3)
    // And every call after it fails at once, rather than waiting for ever.
    await expect(io.call('storageStatus')).rejects.toThrow('blocked')
    expect(workers).toHaveLength(3)
  })

  it('fails what was waiting on a worker that stopped mid-job, and starts afresh', async () => {
    const io = client()
    const first = io.call('storageStatus')
    workers[0]!.answer(workers[0]!.sent[0]!.id, { kind: 'opfs', persisted: true })
    await first
    const second = io.call('storageStatus')
    workers[0]!.fail('out of memory')
    await expect(second).rejects.toThrow('out of memory')
    // Not sent again: it may have been half done.
    expect(workers).toHaveLength(2)
    expect(workers[1]!.sent).toHaveLength(0)
  })
})
