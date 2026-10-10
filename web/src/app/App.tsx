import { Engine, loadEngine } from '@soom/engine'
import { FolderOpen, Maximize2, Minus, Plus, Redo2, Undo2 } from 'lucide-react'
import { useEffect, useRef, useState } from 'react'
import { Button } from 'react-aria-components'
import { PatternCanvas } from '../canvas/PatternCanvas.tsx'
import { actualSize, zoomAt } from '../canvas/viewport.ts'
import { useEngineState } from '../state/hooks.ts'
import { PATTERN_FILE_TYPES, Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import { t } from '../strings/en.ts'
import { IoClient } from '../workers/io-client.ts'
import { ContextMenu } from './ContextMenu.tsx'
import { DeviceDialog } from './DeviceDialog.tsx'
import { ImportDialog } from './ImportDialog.tsx'
import { Inspector } from './Inspector.tsx'
import { PieceList } from './PieceList.tsx'
import { Toolbar } from './Toolbar.tsx'
import { attachShortcuts } from './shortcuts.ts'

function TopBar({ session, onOpen }: { session: Session; onOpen: () => void }) {
  const state = useEngineState(session)
  const name = session.engine.document().meta.name
  return (
    <header className="topbar">
      <div className="brand">
        <img src="/icon.svg" alt="" width={28} height={28} />
        <span className="project-name">{name}</span>
      </div>
      <Button className="button" onPress={onOpen}>
        <FolderOpen size={18} aria-hidden="true" />
        {t.actions.open}
      </Button>
      <div className="spacer" />
      <Button
        className="icon-button"
        aria-label={state.undoLabel ? t.actions.undoThing(state.undoLabel) : t.actions.undo}
        isDisabled={!state.canUndo}
        onPress={() => session.undo()}
      >
        <Undo2 size={20} aria-hidden="true" />
      </Button>
      <Button
        className="icon-button"
        aria-label={state.redoLabel ? t.actions.redoThing(state.redoLabel) : t.actions.redo}
        isDisabled={!state.canRedo}
        onPress={() => session.redo()}
      >
        <Redo2 size={20} aria-hidden="true" />
      </Button>
      <DeviceDialog />
    </header>
  )
}

function ZoomControls({ session }: { session: Session }) {
  const { viewport } = useStore(session.ui)
  const centre = { x: viewport.width / 2, y: viewport.height / 2 }
  return (
    <div className="zoom-controls" role="toolbar" aria-label="Zoom">
      <Button
        className="icon-button"
        aria-label={t.actions.zoomOut}
        onPress={() => session.setViewport((v) => zoomAt(v, centre, 1 / 1.5))}
      >
        <Minus size={18} aria-hidden="true" />
      </Button>
      <Button
        className="zoom-label"
        aria-label={t.actions.actualSize}
        onPress={() => session.setViewport(actualSize)}
      >
        {t.table.zoomLabel(viewport.zoom)}
      </Button>
      <Button
        className="icon-button"
        aria-label={t.actions.zoomIn}
        onPress={() => session.setViewport((v) => zoomAt(v, centre, 1.5))}
      >
        <Plus size={18} aria-hidden="true" />
      </Button>
      <Button
        className="icon-button"
        aria-label={t.actions.fitAll}
        onPress={() => session.fitAll()}
      >
        <Maximize2 size={18} aria-hidden="true" />
      </Button>
    </div>
  )
}

function Workspace({ session }: { session: Session }) {
  const fileInput = useRef<HTMLInputElement>(null)
  const { busy, notice, toast, tip } = useStore(session.ui)
  useEngineState(session)
  const empty = session.engine.document().pieces.length === 0
  const open = () => fileInput.current?.click()

  useEffect(() => attachShortcuts(session, () => fileInput.current?.click()), [session])

  return (
    <div className="app">
      <TopBar session={session} onOpen={open} />
      <input
        ref={fileInput}
        type="file"
        accept={PATTERN_FILE_TYPES}
        hidden
        data-testid="pattern-file"
        onChange={(e) => {
          const file = e.currentTarget.files?.[0]
          e.currentTarget.value = ''
          if (file) void session.openPatternFile(file)
        }}
      />
      <div className="workspace">
        <main className="table" aria-label="Cutting table">
          <PatternCanvas session={session} />
          {empty && (
            <div className="empty-state">
              <p>{t.table.empty}</p>
              <p className="muted small">{t.table.emptyHint}</p>
              <Button className="button primary" onPress={open}>
                <FolderOpen size={18} aria-hidden="true" />
                {t.actions.open}
              </Button>
            </div>
          )}
          <Toolbar session={session} />
          <ZoomControls session={session} />
          <ContextMenu session={session} onOpen={open} />
          {toast && (
            <div className="toast" role="status">
              {toast}
            </div>
          )}
          {tip && (
            <div className="tip" role="status" style={{ left: tip.x, top: tip.y }}>
              {tip.text}
            </div>
          )}
          {busy && (
            <div className="status-bar" role="status">
              {busy}
            </div>
          )}
          {notice && (
            <div
              className={`status-bar ${notice.kind}`}
              role={notice.kind === 'error' ? 'alert' : 'status'}
            >
              <span>{notice.text}</span>
              <Button className="link-button" onPress={() => session.notify(null)}>
                {t.actions.close}
              </Button>
            </div>
          )}
        </main>
        <aside className="side-panel">
          <PieceList session={session} />
          <Inspector session={session} />
        </aside>
      </div>
      <ImportDialog session={session} />
    </div>
  )
}

export function App() {
  const [session, setSession] = useState<Session | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    loadEngine()
      .then(() => {
        if (cancelled) return
        const session = new Session(Engine.create(t.untitled), new IoClient())
        // Opt-in handle for debugging and end-to-end tests: add ?debug to the URL.
        if (import.meta.env.DEV || new URLSearchParams(location.search).has('debug')) {
          Object.assign(window, { soom: session })
        }
        setSession(session)
      })
      .catch((e: unknown) => setError(String(e)))
    return () => {
      cancelled = true
    }
  }, [])

  if (error) return <p className="boot-message">{t.device.engineFailed(error)}</p>
  if (!session) return <p className="boot-message">{t.device.engineLoading}</p>
  return <Workspace session={session} />
}
