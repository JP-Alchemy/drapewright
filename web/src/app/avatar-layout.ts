// Where the avatar is: docked beside the table (under it on a tablet held
// upright), taking a share of their room; or floating over the table as a
// window. Kept on this device.

import { useSyncExternalStore } from 'react'
import { remember, remembered } from '../platform/remember.ts'

/** A floating window's place and size, px from the workspace's top left. */
export interface Box {
  x: number
  y: number
  width: number
  height: number
}

export interface AvatarLayout {
  floating: boolean
  /** The avatar's share of the room it splits with the table, docked. */
  share: number
  /** Where it floats; `null` until it has floated. */
  box: Box | null
}

const KEY = 'drapewright.avatar.layout'
export const SHARE = { first: 0.5, least: 0.25, most: 0.75 }
/** The smallest a floating avatar gets. */
export const LEAST = { width: 280, height: 320 }

const clampShare = (share: unknown) =>
  typeof share === 'number' && Number.isFinite(share)
    ? Math.min(SHARE.most, Math.max(SHARE.least, share))
    : SHARE.first

const isBox = (box: unknown): box is Box =>
  typeof box === 'object' &&
  box !== null &&
  ['x', 'y', 'width', 'height'].every((k) => Number.isFinite((box as Record<string, unknown>)[k]))

function load(): AvatarLayout {
  const kept = remembered<Partial<AvatarLayout> | null>(KEY, null)
  return {
    floating: kept?.floating === true,
    share: clampShare(kept?.share),
    box: isBox(kept?.box) ? kept.box : null,
  }
}

let layout = load()
const listeners = new Set<() => void>()

/**
 * Changes where the avatar is. `keep`: false while a drag is under way, so
 * only where it ends up is kept.
 */
export function setAvatarLayout(change: Partial<AvatarLayout>, keep = true): void {
  layout = { ...layout, ...change, share: clampShare(change.share ?? layout.share) }
  if (keep) remember(KEY, layout)
  for (const listener of listeners) listener()
}

export function useAvatarLayout(): AvatarLayout {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    () => layout,
  )
}

/** `box` moved and sized to fit a room `width` × `height`. */
export function fitBox(box: Box, width: number, height: number): Box {
  const w = Math.min(Math.max(box.width, LEAST.width), Math.max(LEAST.width, width))
  const h = Math.min(Math.max(box.height, LEAST.height), Math.max(LEAST.height, height))
  return {
    width: w,
    height: h,
    x: Math.min(Math.max(box.x, 0), Math.max(0, width - w)),
    y: Math.min(Math.max(box.y, 0), Math.max(0, height - h)),
  }
}
