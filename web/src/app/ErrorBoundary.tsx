import { Component, type ReactNode } from 'react'
import { t } from '../strings/en.ts'

interface State {
  error: Error | null
}

/** Shows a calm message instead of a blank screen if a view fails. */
export class ErrorBoundary extends Component<{ children: ReactNode }, State> {
  override state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  override render() {
    if (!this.state.error) return this.props.children
    return (
      <div className="boot-message" role="alert">
        <div className="card">
          <h1>{t.crash.heading}</h1>
          <p>{t.crash.body}</p>
          <p className="muted small">{this.state.error.message}</p>
          <button className="button primary" type="button" onClick={() => location.reload()}>
            {t.crash.reload}
          </button>
        </div>
      </div>
    )
  }
}
