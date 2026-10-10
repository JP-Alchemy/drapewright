import { engineVersion, loadEngine } from '@soom/engine'
import { useEffect, useState } from 'react'
import { type Capabilities, detectCapabilities } from '../platform/capabilities.ts'

interface CheckRow {
  label: string
  detail: string
  ok: boolean
  /** Shown instead of `detail` when the check fails. */
  fallback: string
}

function checkRows(caps: Capabilities): CheckRow[] {
  return [
    {
      label: 'Saves on this device',
      detail: 'Projects stay private, in this browser’s own storage.',
      ok: caps.opfs,
      fallback: 'This browser cannot store projects. Use Back up often.',
    },
    {
      label: 'Works without internet',
      detail: 'After this first visit the app opens offline.',
      ok: caps.serviceWorker,
      fallback: 'Offline use is not available in this browser.',
    },
    {
      label: '3D draping',
      detail: 'Your graphics chip can run the fabric simulation.',
      ok: caps.webgpu,
      fallback: '3D will show saved drapes only. Try the latest Chrome or Safari.',
    },
    {
      label: 'Fast mode',
      detail: 'Cross-origin isolated, for multi-core work.',
      ok: caps.crossOriginIsolated,
      fallback: 'The page is not served with isolation headers.',
    },
  ]
}

export function App() {
  const [caps, setCaps] = useState<Capabilities | null>(null)
  const [version, setVersion] = useState<string | null>(null)
  const [engineError, setEngineError] = useState<string | null>(null)

  useEffect(() => {
    void detectCapabilities().then(setCaps)
    loadEngine()
      .then(() => setVersion(engineVersion()))
      .catch((err: unknown) => setEngineError(String(err)))
  }, [])

  return (
    <div className="app">
      <header className="topbar">
        <div className="brand">
          <img src="/icon.svg" alt="" width={28} height={28} />
          <span>Soom</span>
        </div>
      </header>

      <main className="table" aria-label="Cutting table">
        <section className="card" aria-labelledby="device-title">
          <h1 id="device-title">This device</h1>
          {caps === null ? (
            <p className="muted">Checking…</p>
          ) : (
            <ul className="checks">
              {checkRows(caps).map((row) => (
                <li key={row.label} data-ok={row.ok}>
                  <span className="check-mark" aria-hidden="true">
                    {row.ok ? '✓' : '–'}
                  </span>
                  <span>
                    <strong>{row.label}</strong>
                    <span className="muted">{row.ok ? row.detail : row.fallback}</span>
                  </span>
                </li>
              ))}
            </ul>
          )}
          <p className="muted small" data-testid="engine-status">
            {engineError
              ? `Pattern engine failed to load: ${engineError}`
              : version
                ? `Pattern engine ${version}`
                : 'Loading pattern engine…'}
          </p>
        </section>
      </main>
    </div>
  )
}
