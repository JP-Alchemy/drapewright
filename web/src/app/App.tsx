import { loadEngine } from '@drapewright/engine'
import {
  Download,
  FolderOpen,
  Maximize2,
  Minus,
  Plus,
  Projector,
  Redo2,
  Rotate3d,
  Undo2,
} from 'lucide-react'
import { type CSSProperties, useEffect, useRef, useState } from 'react'
import { Button, ToggleButton } from 'react-aria-components'
import { PatternCanvas } from '../canvas/PatternCanvas.tsx'
import { actualSize, zoomAt } from '../canvas/viewport.ts'
import { useEngineState } from '../state/hooks.ts'
import { fileName, saveFile } from '../platform/save-file.ts'
import { loadScreenScale } from '../platform/screen.ts'
import { PATTERN_FILE_TYPES, Session } from '../state/session.ts'
import { useStore } from '../state/store.ts'
import {
  type Opened,
  PROJECT_FILE_TYPES,
  type ProjectSummary,
  Projects,
  isProjectFile,
} from '../storage/projects.ts'
import { t } from '../strings/en.ts'
import { IoClient } from '../workers/io-client.ts'
import { AvatarPane } from './AvatarPane.tsx'
import { ProjectorView } from './ProjectorView.tsx'
import { ContextMenu } from './ContextMenu.tsx'
import { DeviceDialog } from './DeviceDialog.tsx'
import { FeedbackDialog } from './FeedbackDialog.tsx'
import { DraftDialog } from './DraftDialog.tsx'
import { ImportDialog } from './ImportDialog.tsx'
import { MeasurementsDialog } from './MeasurementsDialog.tsx'
import { PrintDialog } from './PrintDialog.tsx'
import { LayoutDialog } from './LayoutDialog.tsx'
import { PhotoDialog } from './PhotoDialog.tsx'
import { SidePanel } from './SidePanel.tsx'
import { SAMPLES, type SampleName, isSampleName, sampleFile } from './samples.ts'
import { useAvatarLayout } from './avatar-layout.ts'
import { sideLabel } from './labels.ts'
import { type ProjectActions, ProjectsDialog } from './ProjectsDialog.tsx'
import { ScreenDialog } from './ScreenDialog.tsx'
import { Toolbar } from './Toolbar.tsx'
import { attachShortcuts } from './shortcuts.ts'

function SaveStatus({ projects }: { projects: Projects }) {
  const status = useStore(projects.status)
  const [text, title] =
    status.state === 'saving'
      ? [t.projects.saving, t.projects.saving]
      : status.state === 'saved'
        ? [t.projects.saved, t.projects.savedLong]
        : status.state === 'failed'
          ? [t.projects.notSaved, t.projects.notSavedLong(status.message)]
          : ['', '']
  return (
    <span
      className="save-status"
      data-state={status.state}
      title={title}
      role="status"
      data-testid="save-status"
    >
      {text}
    </span>
  )
}

function TopBar({
  session,
  projects,
  actions,
  onOpen,
}: {
  session: Session
  projects: Projects
  actions: ProjectActions
  onOpen: () => void
}) {
  const state = useEngineState(session)
  const { avatar } = useStore(session.ui)
  const backup = async () => {
    const name = session.engine.document().meta.name
    try {
      session.syncView()
      await projects.flush()
      const bytes = await projects.backup(session.engine)
      const file = fileName(name, 'drapewright')
      saveFile(bytes, file, 'application/octet-stream')
      session.toast(t.projects.backedUp(file))
    } catch (error) {
      session.notify({
        kind: 'error',
        text: t.projects.backupFailed(error instanceof Error ? error.message : String(error)),
      })
    }
  }
  return (
    <header className="topbar">
      <div className="brand">
        <img src="/icon.svg" alt="" width={28} height={28} />
        <ProjectsDialog session={session} actions={actions} />
        <SaveStatus projects={projects} />
      </div>
      <Button className="button" onPress={onOpen}>
        <FolderOpen size={18} aria-hidden="true" />
        <span className="button-label">{t.actions.open}</span>
      </Button>
      <MeasurementsDialog session={session} />
      <ToggleButton
        className="button"
        isSelected={avatar}
        onChange={(open) => session.showAvatar(open)}
        aria-description={t.avatar.openHint}
      >
        <Rotate3d size={18} aria-hidden="true" />
        <span className="button-label">{t.avatar.open}</span>
      </ToggleButton>
      <PrintDialog session={session} />
      <LayoutDialog session={session} />
      <Button
        className="button"
        onPress={() => session.showProjector(true)}
        aria-description={t.projector.openHint}
      >
        <Projector size={18} aria-hidden="true" />
        <span className="button-label minor">{t.projector.open}</span>
      </Button>
      <Button className="button" onPress={backup} aria-description={t.projects.backupHint}>
        <Download size={18} aria-hidden="true" />
        <span className="button-label minor">{t.projects.backup}</span>
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
      <FeedbackDialog />
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
        onPress={() => {
          session.setViewport(actualSize)
          if (!session.ui.get().screenMeasured) session.toast(t.table.notCalibrated)
        }}
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
      <ScreenDialog session={session} />
    </div>
  )
}

function Workspace({
  session,
  projects,
  actions,
}: {
  session: Session
  projects: Projects
  actions: ProjectActions
}) {
  const fileInput = useRef<HTMLInputElement>(null)
  const { busy, notice, toast, tip, sewing, sewingOnto, avatar, projector } = useStore(session.ui)
  // The avatar beside (or under) the table, or floating over it.
  const avatarLayout = useAvatarLayout()
  const docked = avatar && !avatarLayout.floating
  useEngineState(session)
  const empty = session.engine.document().pieces.length === 0
  const open = () => fileInput.current?.click()

  useEffect(() => attachShortcuts(session, () => fileInput.current?.click()), [session])

  return (
    <div className="app">
      <TopBar session={session} projects={projects} actions={actions} onOpen={open} />
      <input
        ref={fileInput}
        type="file"
        accept={`${PATTERN_FILE_TYPES},${PROJECT_FILE_TYPES},image/*`}
        hidden
        data-testid="pattern-file"
        onChange={(e) => {
          const file = e.currentTarget.files?.[0]
          e.currentTarget.value = ''
          // A backup opens as a project of its own; anything else is imported.
          if (file && isProjectFile(file)) void actions.restore(file)
          else if (file) session.openFile(file)
        }}
      />
      <PhotoDialog session={session} />
      <div
        className={docked ? 'workspace with-avatar' : 'workspace'}
        style={
          docked
            ? ({
                '--table-fr': `${(1 - avatarLayout.share).toFixed(3)}fr`,
                '--avatar-fr': `${avatarLayout.share.toFixed(3)}fr`,
              } as CSSProperties)
            : undefined
        }
      >
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
              <div className="samples" role="group" aria-labelledby="samples-heading">
                <p id="samples-heading" className="muted small">
                  {t.samples.heading}
                </p>
                {(Object.keys(SAMPLES) as SampleName[]).map((name) => (
                  <Button
                    key={name}
                    className="sample"
                    onPress={() => void actions.openSample(name)}
                  >
                    <span className="sample-title">{t.samples.titles[name]}</span>
                    <span className="muted small">{t.samples.about[name]}</span>
                  </Button>
                ))}
                <Button className="sample" onPress={() => session.showDrafting(true)}>
                  <span className="sample-title">{t.samples.titles.freesewing}</span>
                  <span className="muted small">{t.samples.about.freesewing}</span>
                </Button>
                <p className="muted small">
                  {t.samples.credit}{' '}
                  <a href={SAMPLES.dress.licence} target="_blank" rel="noopener">
                    {t.samples.licence}
                  </a>
                </p>
              </div>
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
          {sewing && (
            <div className="status-bar" role="status">
              <span>
                {t.seams.sewingHint(
                  sideLabel(session.engine.document(), {
                    ...sewing,
                    line: null,
                    from: null,
                    to: null,
                    mirror: false,
                  }),
                )}
              </span>
              <Button className="link-button" onPress={() => session.cancelSewing()}>
                {t.seams.cancel}
              </Button>
            </div>
          )}
          {sewingOnto && (
            <div className="status-bar" role="status">
              <span>
                {t.seams.sewingOntoHint(
                  session.engine.document().pieces.find((p) => p.id === sewingOnto)?.name ?? '',
                )}
              </span>
              <Button className="link-button" onPress={() => session.cancelSewing()}>
                {t.seams.cancel}
              </Button>
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
        {avatar && <AvatarPane session={session} />}
        <SidePanel session={session} />
      </div>
      <ImportDialog session={session} />
      {projector && <ProjectorView session={session} />}
      <DraftDialog session={session} />
    </div>
  )
}

interface Current {
  session: Session
  /** Changes with every project opened, so the workspace starts afresh. */
  generation: number
}

const reason = (e: unknown) => (e instanceof Error ? e.message : String(e))

/** Which project is open, and moving between projects. */
class Opener implements ProjectActions {
  #current: Current

  constructor(
    readonly projects: Projects,
    first: Opened & { failed?: string },
    private readonly onChange: (current: Current) => void,
  ) {
    const session = show(projects, first)
    if (first.failed) session.notify({ kind: 'error', text: t.projects.openFailed(first.failed) })
    this.#current = { session, generation: 1 }
  }

  get current(): Current {
    return this.#current
  }

  #replace(opened: Opened, message: string) {
    const session = show(this.projects, opened)
    session.toast(message)
    this.#current = { session, generation: this.#current.generation + 1 }
    this.onChange(this.#current)
  }

  #fail(text: string) {
    this.#current.session.notify({ kind: 'error', text })
  }

  list() {
    return this.projects.list()
  }

  create() {
    this.#replace({ engine: this.projects.create(), issues: [] }, t.projects.new)
  }

  async open(id: string) {
    try {
      await this.projects.flush()
      const opened = await this.projects.open(id)
      this.projects.status.set({ state: 'saved' })
      this.#replace(opened, t.projects.opened(opened.engine.document().meta.name))
    } catch (e) {
      this.#fail(t.projects.openFailed(reason(e)))
    }
  }

  async restore(file: File) {
    try {
      const opened = await this.projects.restore(new Uint8Array(await file.arrayBuffer()))
      this.projects.status.set({ state: 'saved' })
      this.#replace(opened, t.projects.restored(opened.engine.document().meta.name))
    } catch (e) {
      this.#fail(t.projects.restoreFailed(reason(e)))
    }
  }

  async openSample(name: SampleName) {
    try {
      const file = await sampleFile(name)
      if (SAMPLES[name].kind === 'pattern') {
        await this.#current.session.openPatternFile(file)
        return
      }
      await this.projects.flush()
      const title = t.samples.names[name]
      const opened = await this.projects.restore(new Uint8Array(await file.arrayBuffer()), {
        name: title,
      })
      this.projects.status.set({ state: 'saved' })
      this.#replace(opened, t.projects.opened(title))
      // Sewn already: the figure, ready to drape it.
      this.#current.session.showAvatar(true)
      this.#current.session.notify({ kind: 'info', text: t.samples.opened(title) })
    } catch (e) {
      this.#fail(t.samples.failed(reason(e)))
    }
  }

  async remove(project: ProjectSummary) {
    try {
      await this.projects.delete(project.id)
      this.#current.session.toast(t.projects.deleted(project.name))
    } catch (e) {
      this.#fail(reason(e))
    }
  }
}

/**
 * The page's projects, and the one opened first: made once, however often
 * the effect asking for them runs (twice, as React checks in development).
 */
let starting: Promise<{ projects: Projects; first: Awaited<ReturnType<Projects['openLatest']>> }>
const start = () =>
  (starting ??= loadEngine().then(async () => {
    const projects = new Projects(new IoClient())
    return { projects, first: await projects.openLatest() }
  }))

export function App() {
  const [opener, setOpener] = useState<Opener | null>(null)
  const [current, setCurrent] = useState<Current | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    start()
      .then(({ projects, first }) => {
        if (cancelled) return
        const o = new Opener(projects, first, setCurrent)
        setOpener(o)
        setCurrent(o.current)
        // A sample asked for in the address (the site's "try it" link):
        // opened once, and taken out of the address.
        const url = new URL(location.href)
        const asked = url.searchParams.get('sample')
        if (isSampleName(asked)) {
          url.searchParams.delete('sample')
          history.replaceState(null, '', url)
          void o.openSample(asked)
        }
      })
      .catch((e: unknown) => setError(String(e)))
    return () => {
      cancelled = true
    }
  }, [])

  // Saves the open project as it changes; the next project takes over.
  useEffect(
    () =>
      opener && current
        ? opener.projects.autosave(current.session.engine, () => current.session.syncView())
        : undefined,
    [opener, current],
  )

  if (error) return <p className="boot-message">{t.device.engineFailed(error)}</p>
  if (!opener || !current) return <p className="boot-message">{t.device.engineLoading}</p>
  return (
    <Workspace
      key={current.generation}
      session={current.session}
      projects={opener.projects}
      actions={opener}
    />
  )
}

/** A session for an opened project, with any load problems shown. */
function show(projects: Projects, opened: Opened): Session {
  const session = new Session(opened.engine, projects.io, loadScreenScale(), projects)
  const problems = opened.issues.length
  if (problems > 0) session.notify({ kind: 'info', text: t.projects.needsChecking(problems) })
  // Opt-in handle for debugging and end-to-end tests: add ?debug to the URL.
  if (import.meta.env.DEV || new URLSearchParams(location.search).has('debug')) {
    Object.assign(window, { drapewright: session })
  }
  return session
}
