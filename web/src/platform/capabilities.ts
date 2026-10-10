// What this browser and device can do. Checked once at start-up and shown in
// the "This device" panel, so a missing capability is explained up front
// rather than discovered halfway through a task.

export interface Capabilities {
  /** COOP/COEP in effect: SharedArrayBuffer and WASM threads are available. */
  crossOriginIsolated: boolean
  /** WebGPU adapter available: 3D draping can run. */
  webgpu: boolean
  /** Origin private file system: projects can be saved on this device. */
  opfs: boolean
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

async function hasOpfs(): Promise<boolean> {
  try {
    await navigator.storage.getDirectory()
    return true
  } catch {
    return false
  }
}

export async function detectCapabilities(): Promise<Capabilities> {
  const [webgpu, opfs] = await Promise.all([hasWebGpu(), hasOpfs()])
  return {
    crossOriginIsolated: globalThis.crossOriginIsolated === true,
    webgpu,
    opfs,
    serviceWorker: 'serviceWorker' in navigator,
  }
}
