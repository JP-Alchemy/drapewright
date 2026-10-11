// The sim worker: the engine's 3D side (the avatar, the 3D view, and the
// project's garment draped on the avatar), off the main thread. The 3D
// module and the body file are loaded the first time it is asked for
// anything, then kept.

import { Avatar, AvatarView, type DrapeProgress, type Draping } from '@drapewright/engine/avatar'
import bodyUrl from '../../../assets/body/adult.dwbody?url'
import {
  type DrapeUpdate,
  SimError,
  type SimMethods,
  type SimNote,
  type SimRequest,
  type SimResponse,
  type SimUpdate,
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

/** Time spent draping each frame, ms: as much as leaves the view smooth. */
const DRAPE_BUDGET = 12
/** How often the page hears how a drape is going, ms. */
const UPDATE_EVERY = 200

/** The drape in progress, asked for by request `id`. */
interface Drape {
  id: number
  draping: Draping
  leftOut: string[]
  finish: (last: DrapeUpdate | null) => void
  told: { stage: string; at: number }
}
let drape: Drape | null = null
/** Counts drapes asked for and taken off: a drape overtaken while starting is dropped. */
let drapes = 0
/** The request being answered, while its handler starts. */
let asking = 0

const done = (p: DrapeProgress) => p.stage === 'settled' || p.stage === 'stopped'

/** Stops the drape in progress, its request answered with `last`. */
function stopDrape(last: DrapeUpdate | null) {
  if (!drape) return
  const { draping, finish } = drape
  drape = null
  draping.free()
  finish(last)
}

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
    if (drape) moveDrape(drape)
    view?.draw()
  })
}

/** Moves the drape on for this frame and shows its cloth. */
function moveDrape(d: Drape) {
  const start = performance.now()
  let progress: DrapeProgress
  do progress = d.draping.step()
  while (!done(progress) && performance.now() - start < DRAPE_BUDGET)
  view?.showCloth(d.draping)
  const update: DrapeUpdate = { ...progress, leftOut: d.leftOut }
  const now = performance.now()
  if (done(progress)) {
    stopDrape(update)
    return
  }
  if (progress.stage !== d.told.stage || now - d.told.at >= UPDATE_EVERY) {
    d.told = { stage: progress.stage, at: now }
    self.postMessage({ drape: d.id, update } satisfies SimUpdate)
  }
  redraw()
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
    // A new figure: the garment no longer fits it.
    drapes++
    stopDrape(null)
    view.hideCloth()
    view.show(a, settings)
    redraw()
    return null
  },
  detachView: () => {
    attachment++
    drapes++
    stopDrape(null)
    view = null
    return null
  },
  drape: async (request) => {
    const id = asking
    const mine = ++drapes
    const a = await loadAvatar()
    if (mine !== drapes) return null
    if (!view) throw new SimError('the view is not open', 'noView')
    stopDrape(null)
    let draping: Draping
    try {
      draping = a.drape(request)
    } catch (error) {
      throw new SimError(error instanceof Error ? error.message : String(error), 'cannotDrape')
    }
    return new Promise<DrapeUpdate | null>((finish) => {
      drape = { id, draping, leftOut: draping.leftOut, finish, told: { stage: '', at: 0 } }
      redraw()
    })
  },
  undrape: () => {
    drapes++
    stopDrape(null)
    view?.hideCloth()
    redraw()
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
    asking = id
    const answer = handler(...args)
    asking = 0
    response = { id, ok: true, result: await answer }
  } catch (error) {
    asking = 0
    response = {
      id,
      ok: false,
      message: error instanceof Error ? error.message : String(error),
      failure: error instanceof SimError ? error.failure : undefined,
    }
  }
  self.postMessage(response)
}
