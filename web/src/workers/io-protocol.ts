import type { Jobs } from '@soom/engine'

export type IoMethod = keyof Jobs

export interface IoRequest<M extends IoMethod = IoMethod> {
  id: number
  method: M
  args: Parameters<Jobs[M]>
}

export type IoResponse =
  { id: number; ok: true; result: unknown } | { id: number; ok: false; message: string }
