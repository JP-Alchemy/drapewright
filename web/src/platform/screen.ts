// The screen's true size, measured once per device and kept in this
// browser's own storage. It is a convenience of this device, not part of
// any project. Without it, 100% assumes the web's nominal 96 px to the inch.

const KEY = 'drapewright.screen.devicePxPerMm'
/** The same setting from before the app was renamed (it was called Soom). */
const EARLIER_KEY = 'soom.screen.devicePxPerMm'

/**
 * Saved as device pixels per millimetre, which do not change when the
 * browser is zoomed; CSS pixels do.
 */
export function loadScreenScale(): number | null {
  try {
    const saved = Number(localStorage.getItem(KEY) ?? localStorage.getItem(EARLIER_KEY))
    const cssPxPerMm = saved / window.devicePixelRatio
    return Number.isFinite(cssPxPerMm) && cssPxPerMm >= 1 && cssPxPerMm <= 20 ? cssPxPerMm : null
  } catch {
    return null
  }
}

/** Saves a measurement in CSS px per mm, or forgets it (`null`). */
export function saveScreenScale(cssPxPerMm: number | null): void {
  try {
    if (cssPxPerMm === null) localStorage.removeItem(KEY)
    else localStorage.setItem(KEY, String(cssPxPerMm * window.devicePixelRatio))
  } catch {
    // Storage is blocked (private browsing): the measurement lasts this visit.
  }
}
