// The sim worker: the engine's 3D side (the avatar now; the view and the
// drape to come), off the main thread. The 3D module and the body file are
// loaded the first time it is asked for anything, then kept.

import { Avatar } from '@drapewright/engine/avatar'
import bodyUrl from '../../../assets/body/adult.dwbody?url'
import { SimError, type SimRequest, type SimResponse } from './sim-protocol.ts'

let avatar: Promise<Avatar> | undefined

function loadAvatar(): Promise<Avatar> {
  const noBodyFile = (why: string) =>
    new SimError(`the body file is not here (${why})`, 'noBodyFile')
  avatar ??= fetch(bodyUrl)
    .then(
      (response) => {
        if (!response.ok) throw noBodyFile(String(response.status))
        return response.arrayBuffer()
      },
      (error: unknown) => {
        throw noBodyFile(String(error))
      },
    )
    .then((bytes) => Avatar.load(new Uint8Array(bytes)))
  // Try again next time rather than keep a failure (offline on first use).
  avatar.catch(() => (avatar = undefined))
  return avatar
}

self.onmessage = async (event: MessageEvent<SimRequest>) => {
  const { id, request } = event.data
  let response: SimResponse
  try {
    response = { id, ok: true, result: (await loadAvatar()).fit(request) }
  } catch (error) {
    response = {
      id,
      ok: false,
      message: error instanceof Error ? error.message : String(error),
      failure: error instanceof SimError ? error.failure : undefined,
    }
  }
  self.postMessage(response)
}
