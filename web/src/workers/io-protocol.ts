import type { Jobs } from '@drapewright/engine'
import type { StorageJobs } from '../storage/worker-store.ts'

/** Everything the IO worker can do: engine jobs and project storage. */
export type IoHandlers = Jobs & StorageJobs

export type IoMethod = keyof IoHandlers

export interface IoRequest<M extends IoMethod = IoMethod> {
  id: number
  method: M
  args: Parameters<IoHandlers[M]>
}

export type IoResponse =
  { id: number; ok: true; result: unknown } | { id: number; ok: false; message: string }
