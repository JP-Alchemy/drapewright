// The sim worker: the engine's 3D side (the avatar, the 3D view, and the
// project's garment draped on the avatar), off the main thread. The 3D
// module and the body file are loaded the first time it is asked for
// anything, then kept.

import {
  Avatar,
  type AvatarSettings,
  AvatarView,
  type DrapeProgress,
  type Draping,
} from '@drapewright/engine/avatar'
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
/** The canvas the view draws on, to take pictures of. */
let surface: OffscreenCanvas | null = null
/** Counts views started and stopped: a view overtaken while starting is dropped. */
let attachment = 0

/** Time spent draping each frame, ms: as much as leaves the view smooth. */
const DRAPE_BUDGET = 12
/**
 * Time spent each frame on a drape not shown till it settles, ms: more, as
 * the view does not change meanwhile (the page, on its own thread, stays
 * smooth).
 */
const QUIET_BUDGET = 40
/** How often the page hears how a drape is going, ms. */
const UPDATE_EVERY = 200

/** The drape in progress, asked for by request `id`. */
interface Drape {
  id: number
  draping: Draping
  /** Shown only once settled, the cloth shown before staying till then. */
  quietly: boolean
  leftOut: string[]
  inside: string[]
  unplaced: string[]
  lined: string[]
  buttons: number
  finish: (last: DrapeUpdate | null) => void
  told: { stage: string; at: number }
}
let drape: Drape | null = null
/** The drape last settled, kept to move into another pose. */
let kept: Draping | null = null
/** The figure shown, to show again in another pose. */
let figure: AvatarSettings | null = null
/** Counts drapes asked for and taken off: a drape overtaken while starting is dropped. */
let drapes = 0
/** The request being answered, while its handler starts. */
let asking = 0

const done = (p: DrapeProgress) => p.stage === 'settled' || p.stage === 'stopped'

/**
 * Stops the drape in progress, its request answered with `last`: settled,
 * it is kept to pose, the last kept let go.
 */
function stopDrape(last: DrapeUpdate | null) {
  forget()
  if (!drape) return
  const { draping, finish } = drape
  drape = null
  if (last) kept = draping
  else draping.free()
  finish(last)
}

/** Lets go of the drape kept to pose. */
function forget() {
  kept?.free()
  kept = null
}

/** Steps `draping` on as the drape asked for by request `id`, answering once done. */
function follow(id: number, draping: Draping, quietly: boolean): Promise<DrapeUpdate | null> {
  return new Promise<DrapeUpdate | null>((finish) => {
    drape = {
      id,
      draping,
      quietly,
      leftOut: draping.leftOut,
      inside: draping.inside,
      unplaced: draping.unplaced,
      lined: draping.lined,
      buttons: draping.buttons,
      finish,
      told: { stage: '', at: 0 },
    }
    redraw()
  })
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

/**
 * Moves the drape on for this frame and shows its cloth. On the graphics
 * card, as many frames as it can take ahead of what has come back.
 */
function moveDrape(d: Drape) {
  const start = performance.now()
  const budget = d.quietly ? QUIET_BUDGET : DRAPE_BUDGET
  let progress: DrapeProgress | null = null
  while (d.draping.ready() && performance.now() - start < budget) {
    progress = d.draping.step()
    if (done(progress)) break
  }
  // Waiting for the graphics card: again next frame.
  if (!progress) {
    redraw()
    return
  }
  if (!d.quietly || done(progress)) view?.showCloth(d.draping)
  const update: DrapeUpdate = {
    ...progress,
    leftOut: d.leftOut,
    inside: d.inside,
    unplaced: d.unplaced,
    lined: d.lined,
    buttons: d.buttons,
    card: d.draping.onCard,
    // Measured as placed, then again as it settles.
    girths: d.draping.girths,
  }
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
  measureSizes: async (request) => (await loadAvatar()).measureSizes(request),
  blendSizes: async (request) => (await loadAvatar()).blendSizes(request),
  fitSteps: async (request) => (await loadAvatar()).fitSteps(request),
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
      surface = canvas
      view.setLook(colours)
      redraw()
    }
    return created.backend
  },
  showAvatar: async (settings, pose) => {
    const a = await loadAvatar()
    if (!view) throw new SimError('the view is not open', 'noView')
    // A new figure: the garment no longer fits it.
    drapes++
    stopDrape(null)
    view.hideCloth()
    view.show(a, settings, pose)
    figure = settings
    redraw()
    return null
  },
  pose: async (pose) => {
    const id = asking
    // Being draped: it moves into the pose once settled.
    if (drape) {
      drape.draping.pose(pose)
      return null
    }
    const a = await loadAvatar()
    if (!view) throw new SimError('the view is not open', 'noView')
    if (kept) {
      const draping = kept
      kept = null
      draping.pose(pose)
      return follow(id, draping, false)
    }
    if (figure) view.show(a, figure, pose)
    redraw()
    return null
  },
  detachView: () => {
    attachment++
    drapes++
    stopDrape(null)
    view = null
    surface = null
    figure = null
    return null
  },
  drape: async (request, quietly) => {
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
    return follow(id, draping, quietly)
  },
  undrape: () => {
    drapes++
    stopDrape(null)
    view?.hideCloth()
    redraw()
    return null
  },
  pick: (x, y, pixelRatio) => view?.pick(x, y, pixelRatio) ?? null,
  // A picture is taken of a frame as soon as it is drawn, before the
  // browser shows it and starts the next.
  picture: async () => {
    if (!view || !surface) throw new SimError('the view is not open', 'noView')
    view.draw()
    return surface.convertToBlob({ type: 'image/png' })
  },
  turnaround: async (frames) => {
    if (!view || !surface) throw new SimError('the view is not open', 'noView')
    const pictures: Promise<Blob>[] = []
    for (let k = 0; k < frames; k++) {
      view.draw()
      pictures.push(surface.convertToBlob({ type: 'image/jpeg', quality: 0.92 }))
      view.spin((2 * Math.PI) / frames)
    }
    redraw()
    return Promise.all(pictures)
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
    case 'shows':
      view.setShows(note.shows)
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
