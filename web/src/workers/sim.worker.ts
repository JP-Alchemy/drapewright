// The sim worker: the engine's 3D side (the avatar and the 3D view now; the
// drape to come), off the main thread. The 3D module and the body file are
// loaded the first time it is asked for anything, then kept.

import { Avatar, AvatarView } from '@drapewright/engine/avatar'
import bodyUrl from '../../../assets/body/adult.dwbody?url'
import {
  SimError,
  type SimMethods,
  type SimNote,
  type SimRequest,
  type SimResponse,
} from './sim-protocol.ts'

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

let view: AvatarView | null = null
/** Counts views started and stopped: a view overtaken while starting is dropped. */
let attachment = 0
let pending = false

/** Draws once before the next frame, however many changes come first. */
function redraw() {
  if (pending || !view) return
  pending = true
  const frame =
    typeof self.requestAnimationFrame === 'function'
      ? (fn: () => void) => self.requestAnimationFrame(fn)
      : (fn: () => void) => setTimeout(fn, 16)
  frame(() => {
    pending = false
    view?.draw()
  })
}

const handlers: { [M in keyof SimMethods]: (...args: Parameters<SimMethods[M]>) => unknown } = {
  fitAvatar: async (request) => (await loadAvatar()).fit(request),
  attachView: async (canvas, colours) => {
    const mine = ++attachment
    let created: AvatarView
    try {
      created = await AvatarView.create(canvas)
    } catch (error) {
      throw new SimError(String(error), 'noView')
    }
    if (mine === attachment) {
      view = created
      view.setLook(colours)
      redraw()
    }
    return created.backend
  },
  showAvatar: async (settings) => {
    const a = await loadAvatar()
    if (!view) throw new SimError('the view is not open', 'noView')
    view.show(a, settings)
    redraw()
    return null
  },
  detachView: () => {
    attachment++
    view = null
    return null
  },
}

function apply(note: SimNote) {
  if (!view) return
  switch (note.note) {
    case 'resize':
      view.resize(note.width, note.height)
      break
    case 'look':
      view.setLook(note.colours)
      break
    case 'turn':
      view.turn(note.dx, note.dy)
      break
    case 'zoom':
      view.zoom(note.factor)
      break
    case 'slide':
      view.slide(note.dx, note.dy, note.pixelRatio)
      break
    case 'reset':
      view.reset()
      break
  }
  redraw()
}

self.onmessage = async (event: MessageEvent<SimRequest | SimNote>) => {
  const message = event.data
  if ('note' in message) {
    apply(message)
    return
  }
  const { id, method, args } = message
  let response: SimResponse
  try {
    const handler = handlers[method] as (...a: unknown[]) => unknown
    response = { id, ok: true, result: await handler(...args) }
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
