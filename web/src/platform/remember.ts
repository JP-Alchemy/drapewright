// Small preferences kept on this device: how the side panel is split,
// which details are folded away. None are kept in a private window, or
// where storage is blocked; the defaults then serve.

/** A preference kept on this device, or `fallback`. */
export function remembered<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key)
    return raw === null ? fallback : (JSON.parse(raw) as T)
  } catch {
    return fallback
  }
}

/** Keeps a preference on this device, if it can be kept. */
export function remember(key: string, value: unknown): void {
  try {
    localStorage.setItem(key, JSON.stringify(value))
  } catch {
    // Not kept: the default serves next time.
  }
}
