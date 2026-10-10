// What this browser and device can do. Checked once at start-up and shown in
// the "This device" panel, so a missing capability is explained up front
// rather than discovered halfway through a task.

export interface Capabilities {
  /** COOP/COEP in effect: SharedArrayBuffer and WASM threads are available. */
  crossOriginIsolated: boolean
  /** WebGPU adapter available: 3D draping can run. */
  webgpu: boolean
  /** Projects can be saved on this device (OPFS, or IndexedDB where it is not usable). */
  storage: boolean
  /** The browser has promised not to clear this site's storage on its own. */
  persisted: boolean
  /** Service workers: the app can work offline after the first visit. */
  serviceWorker: boolean
}

interface GpuNavigator {
  gpu?: { requestAdapter(): Promise<unknown> }
}

async function hasWebGpu(): Promise<boolean> {
  const gpu = (navigator as Navigator & GpuNavigator).gpu
  if (!gpu) return false
  try {
    return (await gpu.requestAdapter()) != null
  } catch {
    return false
  }
}

async function hasStorage(): Promise<boolean> {
  try {
    await navigator.storage.getDirectory()
    return true
  } catch {
    return 'indexedDB' in globalThis
  }
}

async function isPersisted(): Promise<boolean> {
  try {
    return (await navigator.storage?.persisted?.()) ?? false
  } catch {
    return false
  }
}

export async function detectCapabilities(): Promise<Capabilities> {
  const [webgpu, storage, persisted] = await Promise.all([hasWebGpu(), hasStorage(), isPersisted()])
  return {
    crossOriginIsolated: globalThis.crossOriginIsolated === true,
    webgpu,
    storage,
    persisted,
    serviceWorker: 'serviceWorker' in navigator,
  }
}
